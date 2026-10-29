//! A session's lifecycle across threads and the process-level shutdown, through the public API only.
//!
//! A host may create a session on one thread and use or drop it on another (a worker pool, the C ABI), while the creating thread lives on with the journal connection it cached. `reset_learned_data` must still replace the journal then, which on Windows means closing the connections of threads other than the caller: an open handle blocks the rename there, while Unix renames an open file, so the reset test only fails on Windows (CI runs it there). That a session's own teardown leaves no connection behind is checked by the unit test `a_session_moved_to_another_thread_leaves_no_journal_connection_behind`, which can see the parked connections. This binary runs apart from the unit tests, so `shutdown` here cannot take a schedule another test is reading.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use msime_engine::assets;
use msime_engine::{reset_learned_data, RuntimePaths, Session, SessionOptions};
use rusqlite::Connection;

const MAIN_SQL: &str = "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',200),('ni''hao','nh','拟好',100);";

/// Packaged and generation directories apart, as a reset requires, each with the two dictionaries every scheme reads.
fn runtime(root: &Path) -> RuntimePaths {
    let paths = RuntimePaths {
        resources: root.join("resources"),
        user_data: root.join("user"),
        cache: root.join("cache"),
        dictionaries: root.join("dictionaries"),
    };
    for directory in [&paths.resources, &paths.dictionaries] {
        std::fs::create_dir_all(directory).unwrap();
        Connection::open(directory.join(assets::MAIN_DICTIONARY))
            .unwrap()
            .execute_batch(MAIN_SQL)
            .unwrap();
        msime_engine::ensure_english_schema(&directory.join(assets::ENGLISH_DICTIONARY)).unwrap();
    }
    std::fs::create_dir_all(&paths.user_data).unwrap();
    std::fs::create_dir_all(&paths.cache).unwrap();
    paths
}

/// The tests here share the process-wide writer thread: a `shutdown` or a reset in one writes every store's queue, which would hide from another whether its writer started again.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

fn session(paths: &RuntimePaths) -> Session {
    let mut options = SessionOptions::new(paths.clone());
    // The fixture ships no helpcode table.
    options.helpcode = false;
    Session::new(options).unwrap()
}

/// Types `nihao` and picks 拟好, which reads the journal while the candidates are built and records a personal-context transition. A transition the writer thread could not take (it failed to start, or an earlier write failed) comes back as the pick's diagnostic.
fn pick(session: &mut Session) {
    for byte in b"nihao" {
        assert!(session.character(*byte, false).handled);
    }
    let index = session
        .snapshot()
        .candidates
        .iter()
        .position(|row| row.word == "拟好")
        .expect("拟好 is a candidate");
    let picked = session.select(index);
    assert_eq!(picked.commit.as_deref(), Some("拟好"));
    assert_eq!(picked.diagnostic, None);
}

/// The personal-context count the journal holds for picks of 拟好.
fn learned_pairs(journal: &Path) -> i64 {
    Connection::open(journal)
        .unwrap()
        .query_row(
            "SELECT coalesce(sum(count),0) FROM personal_bigram WHERE word='拟好'",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn a_session_used_and_dropped_on_another_thread_leaves_the_journal_free_to_reset() {
    let _serial = serial();
    let root = tempfile::tempdir().unwrap();
    let paths = runtime(root.path());
    let journal = paths.user(assets::USER_JOURNAL);

    let (created_tx, created_rx) = mpsc::channel();
    let (finish_tx, finish_rx) = mpsc::channel::<()>();
    let creator_paths = paths.clone();
    // The creating thread uses the session once, hands it over and stays alive, holding whatever journal connection it cached, while the reset runs.
    let creator = std::thread::spawn(move || {
        let mut created = session(&creator_paths);
        pick(&mut created);
        created_tx.send(created).unwrap();
        finish_rx.recv().unwrap();
    });
    let mut moved = created_rx.recv().unwrap();
    let user = std::thread::spawn(move || {
        pick(&mut moved);
        drop(moved);
    });
    user.join().unwrap();
    // The drop on the other thread wrote the queued context.
    assert!(learned_pairs(&journal) >= 1);

    reset_learned_data(&paths).unwrap();
    assert_eq!(
        learned_pairs(&journal),
        0,
        "the reset kept the learned data"
    );

    finish_tx.send(()).unwrap();
    creator.join().unwrap();
}

#[test]
fn shutdown_writes_the_queue_now_and_learning_resumes_after_it() {
    let _serial = serial();
    let root = tempfile::tempdir().unwrap();
    let paths = runtime(root.path());
    let journal = paths.user(assets::USER_JOURNAL);
    let mut live = session(&paths);

    // One transition is under a batch, so without shutdown it would wait about 2 s on the writer thread.
    pick(&mut live);
    msime_engine::shutdown();
    let first = learned_pairs(&journal);
    assert!(first >= 1, "shutdown did not write the queued pick");

    // The next record starts the writer again: its batch lands on its own, with nobody flushing.
    pick(&mut live);
    let deadline = Instant::now() + Duration::from_secs(30);
    while learned_pairs(&journal) <= first {
        assert!(
            Instant::now() < deadline,
            "no writer wrote the pick after shutdown"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let second = learned_pairs(&journal);

    // A second shutdown stops it again and still writes what is queued.
    pick(&mut live);
    msime_engine::shutdown();
    assert!(
        learned_pairs(&journal) > second,
        "the pick before the second shutdown was not written"
    );
    drop(live);
}

/// Several components of a host may each call `shutdown` from their own exit or unload hook while sessions still record. Every call returns, and every pick reaches its journal. Each recording thread has a data directory of its own: the writer thread and `shutdown` are process-wide either way.
#[test]
fn concurrent_shutdowns_and_records_all_finish() {
    const RECORDERS: usize = 3;
    const ROUNDS: usize = 40;
    const STOPPERS: usize = 2;
    let _serial = serial();
    let root = tempfile::tempdir().unwrap();
    let journals: Vec<_> = (0..RECORDERS)
        .map(|thread| {
            let paths = runtime(&root.path().join(thread.to_string()));
            (paths.user(assets::USER_JOURNAL), paths)
        })
        .collect();
    let recorders: Vec<_> = journals
        .iter()
        .map(|(_, paths)| {
            let paths = paths.clone();
            std::thread::spawn(move || {
                for _ in 0..ROUNDS {
                    // A fresh session each round: repeated picks in one session would learn the pair 拟好拟好 as a phrase, which this fixture's dictionary cannot store.
                    let mut live = session(&paths);
                    pick(&mut live);
                    msime_engine::shutdown();
                }
            })
        })
        .collect();
    let recording = Arc::new(AtomicBool::new(true));
    let stoppers: Vec<_> = (0..STOPPERS)
        .map(|_| {
            let recording = Arc::clone(&recording);
            std::thread::spawn(move || {
                while recording.load(Ordering::Acquire) {
                    msime_engine::shutdown();
                }
            })
        })
        .collect();
    let deadline = Instant::now() + Duration::from_secs(60);
    while !recorders.iter().all(std::thread::JoinHandle::is_finished) {
        assert!(Instant::now() < deadline, "a shutdown never returned");
        std::thread::sleep(Duration::from_millis(20));
    }
    recording.store(false, Ordering::Release);
    while !stoppers.iter().all(std::thread::JoinHandle::is_finished) {
        assert!(Instant::now() < deadline, "a shutdown never returned");
        std::thread::sleep(Duration::from_millis(20));
    }
    for thread in recorders.into_iter().chain(stoppers) {
        thread.join().unwrap();
    }
    for (journal, _) in &journals {
        assert!(
            learned_pairs(journal) >= ROUNDS as i64,
            "a pick recorded around a shutdown was lost"
        );
    }
}
