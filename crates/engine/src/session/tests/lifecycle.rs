//! 会话自己的收尾（结束时写队列、交还日志连接）、候选修订号，以及跨线程使用后不留日志连接。

use super::*;
use crate::touch::TouchTuning;
use crate::types::OnlineQuery;
use crate::user_dictionary::journal::{cache_generation, parked_journals, thread_holds_journal};

#[test]
fn the_candidate_revision_moves_with_every_call_that_may_change_the_candidates() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    assert_eq!(session.candidate_revision(), 0);

    let mut last = session.candidate_revision();
    let mut moved = |session: &Session, call: &str| {
        let now = session.candidate_revision();
        assert!(now > last, "{call} left the revision at {now}");
        last = now;
    };
    session.character(b'n', false);
    moved(&session, "a key");
    session.character(b'i', false);
    moved(&session, "a second key");
    session.command(Command::Backspace);
    moved(&session, "a command");
    session.set_caret(None);
    moved(&session, "set_caret");
    type_text(&mut session, "ihao");
    moved(&session, "typing");
    select_word(&mut session, "拟好");
    moved(&session, "a selection");
    session.reset_context();
    moved(&session, "reset_context");
    session.set_dedicated_english(true);
    moved(&session, "set_dedicated_english");
}

#[test]
fn reads_and_setters_for_later_keys_leave_the_candidate_revision_alone() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    type_text(&mut session, "nihao");
    let revision = session.candidate_revision();

    let _ = session.snapshot();
    let _ = session.online_query();
    let _ = session.segment_raw_boundaries();
    session.set_clock(Clock::default());
    session.set_key_touch(None);
    session.set_key_geometry(None);
    session.set_touch_tuning(TouchTuning::default());
    session.set_chinese_punctuation_enabled(false);
    session.set_paired_punctuation_enabled(false);
    session.set_punctuation_lock(1).unwrap();
    session.set_rescoring_context("上文");

    assert_eq!(session.candidate_revision(), revision);
    // A stale online answer shows nothing, so it changes nothing either.
    assert!(!session.apply_online_candidates(
        &OnlineQuery::default(),
        &["云".to_owned()],
        CandidateSource::CloudSuggestion
    ));
    assert_eq!(session.candidate_revision(), revision);
}

/// The teardown the host session used to own: the queued context reaches the journal and this thread keeps no journal handle once the session is gone.
#[test]
fn dropping_a_session_writes_its_queue_and_releases_the_thread_journal() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    // Frequency learning writes the pick through this thread's journal connection.
    let mut session =
        fixture.session_with(|options| options.frequency.mode = FrequencyAdjustmentMode::Promote);
    let generation = cache_generation();
    type_text(&mut session, "nihao");
    select_word(&mut session, "拟好");
    // Another test closing every journal meanwhile takes the parked connection away.
    assert!(thread_holds_journal() || cache_generation() != generation);
    drop(session);
    assert!(!thread_holds_journal());
    let written: i64 = Connection::open(fixture.journal())
        .unwrap()
        .query_row(
            "SELECT count(*) FROM personal_bigram WHERE word='拟好'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(written, 1);
}

/// A session created and first used on one thread, then used and dropped on another while the first thread lives on: no thread keeps a connection to its journal afterwards.
#[test]
fn a_session_moved_to_another_thread_leaves_no_journal_connection_behind() {
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let options = fixture.options();
    let journal = fixture.journal();
    let (created_tx, created_rx) = std::sync::mpsc::channel();
    let (finish_tx, finish_rx) = std::sync::mpsc::channel::<()>();
    let creator = std::thread::spawn(move || {
        let mut session = Session::new(options).unwrap();
        type_text(&mut session, "nihao");
        created_tx.send(session).unwrap();
        finish_rx.recv().unwrap();
        thread_holds_journal()
    });
    let mut session = created_rx.recv().unwrap();
    let user = std::thread::spawn(move || {
        select_word(&mut session, "拟好");
        drop(session);
        thread_holds_journal()
    });
    assert!(
        !user.join().unwrap(),
        "the thread that dropped the session kept its journal"
    );
    assert_eq!(
        parked_journals(&journal),
        0,
        "the creating thread kept its journal after the session ended"
    );
    finish_tx.send(()).unwrap();
    assert!(!creator.join().unwrap());
}
