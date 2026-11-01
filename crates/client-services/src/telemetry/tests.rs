use super::*;
use std::cell::RefCell;
use std::collections::VecDeque;
#[cfg(unix)]
use std::io::Write as _;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpListener;
use std::sync::{Arc, Barrier};

/// 2026-10-01T12:00:00Z.
fn noon() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_790_856_000)
}

fn app() -> TelemetryApp {
    TelemetryApp::new("linux", "0.50.0").unwrap()
}

fn store() -> (tempfile::TempDir, TelemetryStore) {
    let directory = tempfile::tempdir().unwrap();
    let store = TelemetryStore::new(directory.path().join("state"));
    (directory, store)
}

#[cfg(unix)]
#[test]
fn an_in_progress_native_record_waits_for_the_writer_to_release_its_lock() {
    let (_directory, store) = store();
    let capture = store.prepare_crash_capture(&app(), noon()).unwrap();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&capture.crash_record_path)
        .unwrap();
    let held = crate::file_lock::ScopedFileLock::exclusive(file).unwrap();
    let mut writer = held.file();
    writer
        .write_all(capture.crash_record_header.as_bytes())
        .unwrap();
    writer.write_all(b"Synthetic native fault\n").unwrap();
    assert!(read_crash_file(&capture.crash_record_path).is_none());
    let next = store.begin_session(&app(), noon()).unwrap();
    assert_eq!(next.crashes, 0);
    assert!(capture.crash_record_path.exists());
    writer.write_all(b"synthetic-module+0x1234\n").unwrap();
    drop(held);
    assert_eq!(store.begin_session(&app(), noon()).unwrap().crashes, 1);
    let queue = store.read_queue().unwrap();
    let crash = queue
        .iter()
        .find(|event| event.kind == TelemetryKind::Crash)
        .unwrap();
    assert_eq!(crash.stack, "synthetic-module+0x1234");
    assert!(!capture.crash_record_path.exists());
}

#[test]
fn an_empty_native_record_is_not_promoted_to_an_unknown_crash() {
    let (_directory, store) = store();
    let capture = store.prepare_crash_capture(&app(), noon()).unwrap();
    std::fs::write(&capture.crash_record_path, b"").unwrap();
    assert_eq!(store.begin_session(&app(), noon()).unwrap().crashes, 0);
    assert!(capture.crash_record_path.exists());
    std::fs::write(&capture.crash_record_path, b"Synthetic native fault\n").unwrap();
    assert_eq!(store.begin_session(&app(), noon()).unwrap().crashes, 1);
}

#[test]
fn a_stale_unreadable_native_record_is_removed_without_a_crash_event() {
    let (_directory, store) = store();
    let capture = store.prepare_crash_capture(&app(), noon()).unwrap();
    let file = File::create(&capture.crash_record_path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(8 * 24 * 60 * 60))
        .unwrap();
    drop(file);
    store.record_active(&app(), noon()).unwrap();
    assert!(!capture.crash_record_path.exists());
    assert!(!store
        .read_queue()
        .unwrap()
        .iter()
        .any(|event| event.kind == TelemetryKind::Crash));
}

/// Answers each send with the next scripted delivery and records the bodies.
struct ScriptedSender {
    answers: RefCell<VecDeque<Delivery>>,
    bodies: RefCell<Vec<serde_json::Value>>,
}

impl ScriptedSender {
    fn new(answers: impl IntoIterator<Item = Delivery>) -> Self {
        Self {
            answers: RefCell::new(answers.into_iter().collect()),
            bodies: RefCell::new(Vec::new()),
        }
    }

    fn ids(&self) -> Vec<String> {
        self.bodies
            .borrow()
            .iter()
            .map(|body| body["id"].as_str().unwrap().to_owned())
            .collect()
    }
}

impl TelemetrySender for ScriptedSender {
    fn send(&self, body: &[u8]) -> Delivery {
        self.bodies
            .borrow_mut()
            .push(serde_json::from_slice(body).unwrap());
        self.answers
            .borrow_mut()
            .pop_front()
            .unwrap_or(Delivery::Accepted)
    }
}

#[test]
fn platform_aliases_map_to_the_canonical_ids() {
    for (alias, canonical) in [
        ("win", "windows"),
        ("Windows", "windows"),
        ("mac", "macos"),
        ("darwin", "macos"),
        ("ipados", "ios"),
        ("harmonyos", "harmony"),
        ("ohos", "harmony"),
        ("linux", "linux"),
        ("android", "android"),
    ] {
        assert_eq!(canonical_platform(alias), Some(canonical), "{alias}");
    }
    assert_eq!(canonical_platform("beos"), None);
    assert!(TelemetryApp::new("web", "1.0").is_err());
    assert!(TelemetryApp::new("linux", "").is_err());
    assert!(TelemetryApp::new("linux", "1.0\n2").is_err());
    assert!(TelemetryApp::new("linux", &"9".repeat(65)).is_err());
}

#[test]
fn the_install_id_is_random_stable_and_well_formed() {
    let (_directory, store) = store();
    let first = store.install_id().unwrap();
    assert!(valid_install_id(&first), "{first}");
    assert_eq!(store.install_id().unwrap(), first);
    let (_other, other) = self::store();
    assert_ne!(other.install_id().unwrap(), first);
    assert!(!valid_install_id("short"));
    assert!(!valid_install_id("has space in it!!"));
}

#[test]
fn a_start_queues_one_active_per_utc_day_with_the_deterministic_id() {
    let (_directory, store) = store();
    let install_id = store.install_id().unwrap();
    store.begin_session(&app(), noon()).unwrap();
    store.end_session().unwrap();
    store
        .begin_session(&app(), noon() + Duration::from_secs(3600))
        .unwrap();
    store
        .record_active(&app(), noon() + Duration::from_secs(7200))
        .unwrap();
    let active: Vec<_> = store
        .queued()
        .unwrap()
        .into_iter()
        .filter(|event| event.kind == TelemetryKind::Active)
        .collect();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, format!("active-{install_id}-20261001"));
    assert_eq!(active[0].install_id, install_id);
    assert_eq!(active[0].platform, "linux");
    assert_eq!(active[0].version, "0.50.0");

    store
        .record_active(&app(), noon() + Duration::from_secs(13 * 3600))
        .unwrap();
    let days: Vec<_> = store
        .queued()
        .unwrap()
        .into_iter()
        .filter(|event| event.kind == TelemetryKind::Active)
        .map(|event| event.id)
        .collect();
    assert_eq!(
        days,
        [
            format!("active-{install_id}-20261001"),
            format!("active-{install_id}-20261002")
        ]
    );
}

#[test]
fn a_normal_end_queues_session_and_a_bare_leftover_marker_is_not_a_crash() {
    let (_directory, store) = store();
    store.begin_session(&app(), noon()).unwrap();
    assert!(store.end_session().unwrap());
    assert!(!store.end_session().unwrap(), "the marker is gone");
    let kinds = |store: &TelemetryStore| {
        store
            .queued()
            .unwrap()
            .into_iter()
            .map(|event| event.kind)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        kinds(&store),
        [TelemetryKind::Active, TelemetryKind::Session]
    );

    // The process is killed (no end, no crash record): the next start reports nothing for it.
    let start = store.begin_session(&app(), noon()).unwrap();
    assert!(!start.previous_session_crashed);
    let next = store.begin_session(&app(), noon()).unwrap();
    assert!(!next.previous_session_crashed);
    assert_eq!(next.crashes, 0);
    assert_eq!(
        kinds(&store),
        [TelemetryKind::Active, TelemetryKind::Session]
    );
}

#[test]
fn a_crash_record_turns_the_leftover_session_into_session_crash_and_a_crash() {
    let (_directory, store) = store();
    let start = store
        .begin_session(&TelemetryApp::new("windows", "0.50.1").unwrap(), noon())
        .unwrap();
    assert!(start.crash_record_path.parent().unwrap().is_dir());
    assert!(store
        .record_crash(
            "std::runtime_error: bad state in C:\\Users\\Alice Smith\\AppData\\Local\\MSIME\\data.bin",
            "msime-server.exe+0x1a2b\r\nC:\\Users\\Alice Smith\\AppData\\Local\\MSIME\\msime_host_api.dll+0x77\r\n"
        )
        .unwrap());
    // A second record for the same session (the SIGABRT after a terminate handler) keeps the first.
    assert!(!store.record_crash("SIGABRT", "").unwrap());

    let upgraded = TelemetryApp::new("windows", "0.50.2").unwrap();
    let next = store.begin_session(&upgraded, noon()).unwrap();
    assert!(next.previous_session_crashed);
    assert_eq!(next.crashes, 1);
    assert!(!start.crash_record_path.exists());
    let queue = store.queued().unwrap();
    let session_crash = queue
        .iter()
        .find(|event| event.kind == TelemetryKind::SessionCrash)
        .unwrap();
    let crash = queue
        .iter()
        .find(|event| event.kind == TelemetryKind::Crash)
        .unwrap();
    // Both belong to the version that crashed, not the one that reports them.
    assert_eq!(session_crash.version, "0.50.1");
    assert_eq!(crash.version, "0.50.1");
    assert_eq!(crash.platform, "windows");
    assert_eq!(
        crash.message.lines().next(),
        Some("std::runtime_error: bad state in data.bin")
    );
    assert_eq!(
        crash.stack,
        "msime-server.exe+0x1a2b\nmsime_host_api.dll+0x77"
    );
    assert!(!crash.stack.contains("Alice"));
    assert!(crash.is_valid());
    assert!(session_crash.id.starts_with("session-"));
    assert!(crash.id.starts_with("crash-"));
}

/// 同一目录里两个宿主进程各自的会话（同一用户的两个图形会话、或启动时被重复拉起的宿主）：标记只记着最后开始的那个。
#[test]
fn concurrent_sessions_in_one_directory_keep_their_own_crashes_and_ends() {
    let (_directory, store) = store();
    let older = TelemetryApp::new("linux", "0.50.0").unwrap();
    let newer = TelemetryApp::new("linux", "0.51.0").unwrap();
    let first = store.begin_session(&older, noon()).unwrap();
    let second = store.begin_session(&newer, noon()).unwrap();
    assert_ne!(first.session_id, second.session_id);
    assert_eq!(
        first
            .crash_record_path
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap(),
        first.session_id
    );

    // 第一个进程 terminate：记录写给它自己的会话，随后 SIGABRT 再写同一个文件会被拒绝。
    assert!(store
        .record_crash_for(&first.session_id, "std::terminate: boom", "")
        .unwrap());
    assert!(first.crash_record_path.is_file());
    assert!(!second.crash_record_path.exists());
    assert!(!store
        .record_crash_for(&first.session_id, "SIGABRT", "")
        .unwrap());

    // 第二个进程还在运行时有第三个进程启动：只为崩溃的第一个会话补报一次，版本是它自己的。
    let third = store.begin_session(&newer, noon()).unwrap();
    assert!(third.previous_session_crashed);
    assert_eq!(third.crashes, 1);
    let queue = store.queued().unwrap();
    let crashed: Vec<_> = queue
        .iter()
        .filter(|event| event.kind == TelemetryKind::SessionCrash)
        .collect();
    assert_eq!(crashed.len(), 1);
    assert_eq!(crashed[0].id, format!("session-{}", first.session_id));
    assert_eq!(crashed[0].version, "0.50.0");
    let crash = queue
        .iter()
        .find(|event| event.kind == TelemetryKind::Crash)
        .unwrap();
    assert_eq!(crash.id, format!("crash-{}", first.session_id));
    assert_eq!(crash.version, "0.50.0");

    // 第二个进程正常结束：报它自己的 session，第三个进程的标记不动。
    assert!(store.end_session_by_id(&second.session_id).unwrap());
    assert!(!store.end_session_by_id(&second.session_id).unwrap());
    assert_eq!(store.read_marker().unwrap().id, third.session_id);
    // 崩溃过的会话已经关闭，不能再报成正常结束。
    assert!(!store.end_session_by_id(&first.session_id).unwrap());

    // 第三个进程之后被信号打断，记录照样由下一次启动认领，即使中间又有进程开始过。
    let fourth = store.begin_session(&newer, noon()).unwrap();
    std::fs::write(&third.crash_record_path, b"SIGSEGV: segmentation fault\n").unwrap();
    let fifth = store.begin_session(&newer, noon()).unwrap();
    assert!(fifth.previous_session_crashed);
    assert!(store.end_session_by_id(&fourth.session_id).unwrap());
    let ids: Vec<_> = store
        .queued()
        .unwrap()
        .into_iter()
        .filter(|event| event.kind != TelemetryKind::Active)
        .map(|event| (event.kind, event.id))
        .collect();
    assert_eq!(
        ids,
        [
            (
                TelemetryKind::SessionCrash,
                format!("session-{}", first.session_id)
            ),
            (TelemetryKind::Crash, format!("crash-{}", first.session_id)),
            (
                TelemetryKind::Session,
                format!("session-{}", second.session_id)
            ),
            (
                TelemetryKind::SessionCrash,
                format!("session-{}", third.session_id)
            ),
            (TelemetryKind::Crash, format!("crash-{}", third.session_id)),
            (
                TelemetryKind::Session,
                format!("session-{}", fourth.session_id)
            ),
        ]
    );
    // 不按 id 结束（旧宿主）仍结束标记里的会话。
    assert!(store.end_session().unwrap());
    assert!(store.read_marker().is_none());
    assert!(!store.end_session_by_id(&fifth.session_id).unwrap());
}

/// iOS 的 MetricKit 诊断在 App 下次打开时才送到，那时崩溃的键盘进程早已不在：按进程号对回它当时未结束的会话，一次崩溃只得到一条 `crash` 和一条 `session_crash`。
#[test]
fn a_diagnosed_crash_belongs_to_the_open_session_of_its_process_once() {
    let (_directory, store) = store();
    let ios = |version: &str| TelemetryApp::new("ios", version).unwrap();
    // 进程 101 展示过两次键盘：第一次正常收起，第二次展示期间崩溃。另一个键盘进程 202 的会话还开着。
    let ended = store
        .begin_session_in_process(&ios("0.52.0"), noon(), 101)
        .unwrap();
    assert!(store.end_session_by_id(&ended.session_id).unwrap());
    let crashed = store
        .begin_session_in_process(&ios("0.52.0"), noon(), 101)
        .unwrap();
    let other = store
        .begin_session_in_process(&ios("0.52.0"), noon(), 202)
        .unwrap();

    assert_eq!(
        store
            .record_diagnosed_crash(
                Some(101),
                "Crash exception 10 code 0 signal 6",
                "0 MSIMEKeyboardExtension + 1"
            )
            .unwrap(),
        DiagnosedCrash::Session
    );
    assert!(crashed.crash_record_path.is_file());
    assert!(!other.crash_record_path.exists());
    // 同一次崩溃再送一次：保留第一份。
    assert_eq!(
        store
            .record_diagnosed_crash(Some(101), "Crash exception 10 code 0 signal 6", "")
            .unwrap(),
        DiagnosedCrash::Duplicate
    );

    // 进程 303 的键盘弹出时补报，版本是崩溃的那个会话的。
    let next = store
        .begin_session_in_process(&ios("0.53.0"), noon(), 303)
        .unwrap();
    assert!(next.previous_session_crashed);
    assert_eq!(next.crashes, 1);
    // 认领之后同一进程的诊断仍被认出已经报过。
    assert_eq!(
        store
            .record_diagnosed_crash(Some(101), "Crash exception 10 code 0 signal 6", "")
            .unwrap(),
        DiagnosedCrash::Duplicate
    );

    // 进程 404 的信号处理函数已经写下记录，MetricKit 的那份不再写，记录被认领前后都一样。
    let handled = store
        .begin_session_in_process(&ios("0.53.0"), noon(), 404)
        .unwrap();
    std::fs::write(&handled.crash_record_path, b"Fatal signal SIGSEGV\n").unwrap();
    assert_eq!(
        store
            .record_diagnosed_crash(Some(404), "Crash exception 1 code 0 signal 11", "")
            .unwrap(),
        DiagnosedCrash::Duplicate
    );
    store
        .begin_session_in_process(&ios("0.53.0"), noon(), 505)
        .unwrap();
    assert_eq!(
        store
            .record_diagnosed_crash(Some(404), "Crash exception 1 code 0 signal 11", "")
            .unwrap(),
        DiagnosedCrash::Duplicate
    );

    // 没有未结束会话的进程（App 自身、收起后才被结束的键盘），以及系统强制结束：只报 crash。
    assert_eq!(
        store
            .record_diagnosed_crash(Some(101_000), "Crash exception 10 code 0 signal 6", "")
            .unwrap(),
        DiagnosedCrash::Outside
    );
    assert_eq!(
        store
            .record_diagnosed_crash(
                None,
                "Killed exception 10 code 0 signal 9 Namespace RUNNINGBOARD, Code 0xdead10cc",
                ""
            )
            .unwrap(),
        DiagnosedCrash::Outside
    );
    let last = store
        .begin_session_in_process(&ios("0.53.0"), noon(), 606)
        .unwrap();
    assert!(!last.previous_session_crashed);
    assert_eq!(last.crashes, 2);

    let queue = store.queued().unwrap();
    let session_crashes: Vec<_> = queue
        .iter()
        .filter(|event| event.kind == TelemetryKind::SessionCrash)
        .map(|event| (event.id.clone(), event.version.clone()))
        .collect();
    assert_eq!(
        session_crashes,
        [
            (
                format!("session-{}", crashed.session_id),
                "0.52.0".to_owned()
            ),
            (
                format!("session-{}", handled.session_id),
                "0.53.0".to_owned()
            ),
        ]
    );
    let crashes: Vec<_> = queue
        .iter()
        .filter(|event| event.kind == TelemetryKind::Crash)
        .map(|event| event.message.lines().next().unwrap())
        .collect();
    assert_eq!(crashes.len(), 4);
    assert!(crashes.contains(&"Crash exception 10 code 0 signal 6"));
    assert!(crashes.contains(&"Fatal signal SIGSEGV"));
    assert!(crashes
        .contains(&"Killed exception 10 code 0 signal 9 Namespace RUNNINGBOARD, Code 0xdead10cc"));
    assert!(queue
        .iter()
        .any(|event| event.id == format!("crash-{}", crashed.session_id)));
}

#[test]
fn a_diagnosed_crash_takes_the_newest_session_of_a_reused_process_id() {
    let (_directory, store) = store();
    // 进程 7 的会话被 SIGKILL 结束、只留下标记；进程号之后被新的键盘进程重用。
    let killed = store.begin_session_in_process(&app(), noon(), 7).unwrap();
    let later = store
        .begin_session_in_process(&app(), noon() + Duration::from_secs(60), 7)
        .unwrap();
    assert_eq!(
        store
            .record_diagnosed_crash(Some(7), "SIGSEGV", "")
            .unwrap(),
        DiagnosedCrash::Session
    );
    assert!(later.crash_record_path.is_file());
    assert!(!killed.crash_record_path.exists());
}

#[test]
fn clear_forgets_the_crashed_processes() {
    let (_directory, store) = store();
    let start = store.begin_session_in_process(&app(), noon(), 9).unwrap();
    std::fs::write(&start.crash_record_path, b"SIGSEGV\n").unwrap();
    store.begin_session_in_process(&app(), noon(), 10).unwrap();
    assert_eq!(store.read_state().crashed_processes, [9]);
    store.clear().unwrap();
    assert!(store.read_state().crashed_processes.is_empty());
}

#[test]
fn a_marker_left_by_an_older_version_is_still_closed_by_its_crash_record() {
    let (_directory, store) = store();
    std::fs::create_dir_all(store.directory().join(CRASH_DIRECTORY)).unwrap();
    let previous = "6f1c2a7e-0d35-4c0b-9a4e-2f8d1b3c4e5f";
    std::fs::write(
        store.directory().join(SESSION_FILE),
        serde_json::to_vec(&serde_json::json!({
            "id": previous, "platform": "linux", "version": "0.49.0", "started_at_unix_ms": 1
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        store
            .directory()
            .join(CRASH_DIRECTORY)
            .join(format!("{previous}.{CRASH_EXTENSION}")),
        b"SIGSEGV\n",
    )
    .unwrap();
    let start = store.begin_session(&app(), noon()).unwrap();
    assert!(start.previous_session_crashed);
    let crashed = store
        .queued()
        .unwrap()
        .into_iter()
        .find(|event| event.kind == TelemetryKind::SessionCrash)
        .unwrap();
    assert_eq!(crashed.id, format!("session-{previous}"));
    assert_eq!(crashed.version, "0.49.0");
}

#[test]
fn only_the_newest_open_sessions_are_remembered() {
    let (_directory, store) = store();
    let oldest = store.begin_session(&app(), noon()).unwrap();
    for _ in 0..MAX_OPEN_SESSIONS {
        store.begin_session(&app(), noon()).unwrap();
    }
    // 被挤出的会话之后留下的记录只报 crash，不再报 session_crash。
    std::fs::write(&oldest.crash_record_path, b"SIGSEGV\n").unwrap();
    let next = store.begin_session(&app(), noon()).unwrap();
    assert!(!next.previous_session_crashed);
    assert_eq!(next.crashes, 1);
    assert_eq!(store.read_state().open_sessions.len(), MAX_OPEN_SESSIONS);
}

#[test]
fn record_crash_for_refuses_anything_but_a_session_id() {
    let (_directory, store) = store();
    assert_eq!(
        store.record_crash_for("../escape", "boom", ""),
        Err(TelemetryError::Invalid)
    );
}

#[test]
fn crash_file_read_reserves_capped_file_size() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("synthetic.crash");
    let contents = vec![b'x'; 128];
    std::fs::write(&path, &contents).unwrap();

    let bytes = read_crash_file(&path).unwrap();
    assert_eq!(bytes, contents);
    assert_eq!(bytes.capacity(), 128);
}

#[test]
fn a_record_a_signal_handler_wrote_raw_is_read_too() {
    let (_directory, store) = store();
    let start = store.begin_session(&app(), noon()).unwrap();
    std::fs::write(
        &start.crash_record_path,
        b"SIGSEGV\n/home/alice/.local/lib/libmsime_host_api.so(+0x1f2e)[0x7f00]\n/usr/lib/x86_64-linux-gnu/libc.so.6(+0x3c050)[0x7f01]\n\xff\n",
    )
    .unwrap();
    store.begin_session(&app(), noon()).unwrap();
    let crash = store
        .queued()
        .unwrap()
        .into_iter()
        .find(|event| event.kind == TelemetryKind::Crash)
        .unwrap();
    assert_eq!(crash.message.lines().next(), Some("SIGSEGV"));
    assert_eq!(
        crash.stack,
        "libmsime_host_api.so(+0x1f2e)[0x7f00]\nlibc.so.6(+0x3c050)[0x7f01]\n\u{fffd}"
    );
}

#[test]
fn crash_records_beyond_one_batch_survive_for_the_next_collection() {
    let (_directory, store) = store();
    store.begin_session(&app(), noon()).unwrap();
    let directory = store.directory().join(CRASH_DIRECTORY);
    for index in 0..MAX_CRASH_RECORDS_PER_START + 3 {
        let path = directory.join(format!("{}.crash", Uuid::new_v4()));
        std::fs::write(path, format!("synthetic crash {index}\nmodule.so+0x12\n")).unwrap();
    }
    let first = store.begin_session(&app(), noon()).unwrap();
    assert_eq!(first.crashes, MAX_CRASH_RECORDS_PER_START);
    assert_eq!(crash_record_paths(&directory).unwrap().len(), 3);
    let second = store.begin_session(&app(), noon()).unwrap();
    assert_eq!(second.crashes, 3);
    assert!(crash_record_paths(&directory).unwrap().is_empty());
    assert_eq!(
        store
            .queued()
            .unwrap()
            .iter()
            .filter(|event| event.kind == TelemetryKind::Crash)
            .count(),
        MAX_CRASH_RECORDS_PER_START + 3
    );
}

#[test]
fn a_failed_queue_write_keeps_the_original_crash_record() {
    let (_directory, store) = store();
    let start = store.begin_session(&app(), noon()).unwrap();
    std::fs::write(
        &start.crash_record_path,
        "synthetic crash\nmodule.so+0x12\n",
    )
    .unwrap();
    let queue_path = store.directory().join(QUEUE_FILE);
    std::fs::remove_file(&queue_path).unwrap();
    std::fs::create_dir(&queue_path).unwrap();
    assert_eq!(
        store.begin_session(&app(), noon()),
        Err(TelemetryError::Storage)
    );
    assert!(start.crash_record_path.is_file());
    std::fs::remove_dir(queue_path).unwrap();
    assert_eq!(store.begin_session(&app(), noon()).unwrap().crashes, 1);
}

#[test]
fn flushing_without_a_keyboard_session_collects_standalone_crash_records() {
    let (_directory, store) = store();
    store.record_active(&app(), noon()).unwrap();
    let directory = store.directory().join(CRASH_DIRECTORY);
    std::fs::create_dir_all(&directory).unwrap();
    let record = directory.join(format!("{}.crash", Uuid::new_v4()));
    std::fs::write(&record, "synthetic settings crash\nsettings+0x12\n").unwrap();
    let sender = ScriptedSender::new([]);
    store.flush_with(&app(), &sender, noon()).unwrap();
    assert!(sender
        .bodies
        .borrow()
        .iter()
        .any(|body| body["kind"] == "crash"));
    assert!(!sender
        .bodies
        .borrow()
        .iter()
        .any(|body| body["kind"] == "session_crash"));
    assert!(!record.exists());
    assert!(!store.directory().join(SESSION_FILE).exists());
}

#[test]
fn retrying_a_standalone_crash_after_marker_write_failure_keeps_its_event_id() {
    let (_directory, store) = store();
    store.record_active(&app(), noon()).unwrap();
    let directory = store.directory().join(CRASH_DIRECTORY);
    std::fs::create_dir_all(&directory).unwrap();
    let record = directory.join("harmony-123-456.crash");
    std::fs::write(&record, "synthetic delayed crash\nmodule.so+0x12\n").unwrap();
    let marker = store.directory().join(SESSION_FILE);
    std::fs::create_dir(&marker).unwrap();
    assert_eq!(
        store.begin_session(&app(), noon()),
        Err(TelemetryError::Storage)
    );
    assert!(record.is_file());
    let first = store
        .queued()
        .unwrap()
        .into_iter()
        .find(|event| event.kind == TelemetryKind::Crash)
        .unwrap();
    std::fs::remove_dir(&marker).unwrap();
    store.begin_session(&app(), noon()).unwrap();
    let crashes: Vec<_> = store
        .queued()
        .unwrap()
        .into_iter()
        .filter(|event| event.kind == TelemetryKind::Crash)
        .collect();
    assert_eq!(crashes.len(), 1);
    assert_eq!(crashes[0].id, first.id);
}

#[test]
fn collecting_a_crashed_session_does_not_resurrect_its_legacy_marker() {
    let (_directory, store) = store();
    let start = store.begin_session(&app(), noon()).unwrap();
    std::fs::write(
        &start.crash_record_path,
        "synthetic crash\nmodule.so+0x12\n",
    )
    .unwrap();
    store.record_active(&app(), noon()).unwrap();
    assert!(store.read_state().open_sessions.is_empty());
    assert!(!store.directory().join(SESSION_FILE).exists());
    store.record_active(&app(), noon()).unwrap();
    assert!(store.read_state().open_sessions.is_empty());
    assert!(!store.end_session().unwrap());
    let events = store.queued().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == TelemetryKind::SessionCrash)
            .count(),
        1
    );
    assert!(!events
        .iter()
        .any(|event| event.kind == TelemetryKind::Session));
}

#[test]
fn record_crash_without_a_session_writes_nothing() {
    let (_directory, store) = store();
    assert!(!store.record_crash("boom", "").unwrap());
}

#[test]
fn independent_capture_preserves_the_crashing_build_after_an_upgrade() {
    let (_directory, store) = store();
    let previous = TelemetryApp::new("android", "1.2.3")
        .unwrap()
        .with_component("settings")
        .unwrap();
    let capture = store.prepare_crash_capture(&previous, noon()).unwrap();
    assert!(!store.directory().join(SESSION_FILE).exists());
    assert!(store.read_state().open_sessions.is_empty());
    let mut context = CrashContext::from_header(&capture.crash_record_header).unwrap();
    context.revision = "1234567890abcdef1234567890abcdef12345678".to_owned();
    context.run = "100".to_owned();
    context.attempt = "2".to_owned();
    context.arch = "aarch64".to_owned();
    std::fs::write(
        &capture.crash_record_path,
        format!("{}SIGSEGV\nlibsynthetic.so+0x42\n", context.record_header()),
    )
    .unwrap();
    let upgraded = TelemetryApp::new("android", "2.0.0").unwrap();
    store.record_active(&upgraded, noon()).unwrap();
    let queue = store.queued().unwrap();
    let crash = queue
        .iter()
        .find(|event| event.kind == TelemetryKind::Crash)
        .unwrap();
    assert_eq!(crash.version, "1.2.3");
    assert_eq!(crash.message.lines().next(), Some("SIGSEGV"));
    let metadata = crash
        .message
        .lines()
        .nth(1)
        .unwrap()
        .strip_prefix("[msime-crash-v1] ")
        .unwrap();
    assert_eq!(
        serde_json::from_str::<CrashContext>(metadata).unwrap(),
        context
    );
    assert_eq!(crash.stack, "libsynthetic.so+0x42");
    assert!(crash.is_valid());
    assert!(!queue.iter().any(|event| matches!(
        event.kind,
        TelemetryKind::Session | TelemetryKind::SessionCrash
    )));
    assert!(!store.end_session_by_id(&capture.session_id).unwrap());
}

#[test]
fn crash_identity_is_bounded_and_refuses_paths_and_control_characters() {
    let (_directory, store) = store();
    assert!(app().with_component("/synthetic/private").is_err());
    let start = store.prepare_crash_capture(&app(), noon()).unwrap();
    let mut context = CrashContext::from_header(&start.crash_record_header).unwrap();
    context.revision = "/synthetic/private".to_owned();
    assert!(CrashContext::from_header(&context.record_header()).is_none());
    assert_eq!(
        store.record_crash_with_context(&start.session_id, Some(&context), "SIGSEGV", ""),
        Err(TelemetryError::Invalid)
    );
    context.revision = "synthetic-revision".to_owned();
    let summary = "错".repeat(MAX_MESSAGE_CHARS + 1);
    let message = context.message(&summary);
    assert_eq!(message.chars().count(), MAX_MESSAGE_CHARS);
    assert_eq!(message.lines().count(), 2);
    assert!(!message.lines().next().unwrap().is_empty());
}

#[test]
fn malformed_crash_identity_is_not_attributed_to_the_collecting_build() {
    let (_directory, store) = store();
    let capture = store.prepare_crash_capture(&app(), noon()).unwrap();
    std::fs::write(
        &capture.crash_record_path,
        "MSIME-CRASH/1 {\"version\":\"1.2.3\"}\nSIGSEGV\nsynthetic.so+0x42\n",
    )
    .unwrap();
    store.record_active(&app(), noon()).unwrap();
    assert!(capture.crash_record_path.exists());
    assert!(!store
        .queued()
        .unwrap()
        .iter()
        .any(|event| event.kind == TelemetryKind::Crash));
}

#[test]
fn damaged_records_do_not_starve_valid_reports_after_the_batch_limit() {
    let (_directory, store) = store();
    let capture = store.prepare_crash_capture(&app(), noon()).unwrap();
    let directory = capture.crash_record_path.parent().unwrap();
    for index in 0..MAX_CRASH_RECORDS_PER_START {
        std::fs::write(
            directory.join(format!("000-damaged-{index}.crash")),
            "MSIME-CRASH/1 {}\nSIGSEGV\n",
        )
        .unwrap();
    }
    store
        .record_crash_for(&capture.session_id, "synthetic valid crash", "")
        .unwrap();
    store.record_active(&app(), noon()).unwrap();
    assert!(store.queued().unwrap().iter().any(|event| {
        event.kind == TelemetryKind::Crash && event.message == "synthetic valid crash"
    }));
    assert_eq!(
        std::fs::read_dir(directory).unwrap().count(),
        MAX_CRASH_RECORDS_PER_START
    );
}

#[test]
fn independent_capture_does_not_replace_another_process_session() {
    let (_directory, store) = store();
    let input = store.begin_session(&app(), noon()).unwrap();
    let capture = store
        .prepare_crash_capture(&app().with_component("panel").unwrap(), noon())
        .unwrap();
    assert_eq!(store.read_marker().unwrap().id, input.session_id);
    assert!(!store.end_session_by_id(&capture.session_id).unwrap());
    assert_eq!(store.read_marker().unwrap().id, input.session_id);
    assert!(store.end_session_by_id(&input.session_id).unwrap());
}

#[test]
fn messages_and_stacks_are_cleaned_and_bounded() {
    assert_eq!(
        strip_directories("at /Users/bob/src/main.rs:12"),
        "at main.rs:12"
    );
    assert_eq!(strip_directories("~/lib/x.so"), "x.so");
    assert_eq!(
        strip_directories("msime.dll!foo+0x12 (D:\\a\\b\\c.cpp:3)"),
        "msime.dll!foo+0x12 (c.cpp:3)"
    );
    assert_eq!(strip_directories("\\\\server\\share\\x.dll"), "x.dll");
    assert_eq!(
        strip_directories("std::ratio<1/1000>"),
        "std::ratio<1/1000>"
    );
    assert_eq!(strip_directories("no paths here"), "no paths here");
    // URLs in Foundation error descriptions carry the home directory after the scheme's colon.
    assert_eq!(
        strip_directories("NSURL=file:///Users/bob/Library/MSIME/x.json"),
        "NSURL=file:x.json"
    );
    assert_eq!(
        strip_directories("error:/home/bob/.local/lib/libmsime.so"),
        "error:libmsime.so"
    );
    assert_eq!(strip_directories("std::vector::at"), "std::vector::at");

    let message = clean_message(&format!("{}\u{7}", "错".repeat(1200)));
    assert_eq!(message.chars().count(), MAX_MESSAGE_CHARS);
    assert!(!message.contains('\u{7}'));

    let line = format!("frame {}", "界".repeat(100));
    let stack = clean_stack(&vec![line.as_str(); 400].join("\n"));
    assert!(stack.len() <= MAX_STACK_BYTES);
    assert!(stack.chars().count() <= MAX_STACK_CHARS);
    assert!(
        stack.lines().all(|kept| kept == line),
        "cut at a line boundary"
    );

    let single = clean_stack(&"x".repeat(20_000));
    assert_eq!(single.len(), MAX_STACK_BYTES);

    let ascii = clean_stack(&vec!["0123456789"; 3000].join("\n"));
    assert!(ascii.chars().count() <= MAX_STACK_CHARS);
    assert!(ascii.len() <= MAX_STACK_BYTES);
}

#[test]
fn the_whole_crash_event_stays_under_the_body_limit() {
    let event = TelemetryEvent {
        id: Uuid::new_v4().hyphenated().to_string(),
        kind: TelemetryKind::Crash,
        platform: "linux".into(),
        version: "1".repeat(64),
        message: clean_message(&"\u{1F600}".repeat(5000)),
        stack: clean_stack(&"\"\t\\".repeat(20_000)),
        artifact: String::new(),
        channel: String::new(),
        install_id: "a".repeat(64),
    };
    assert!(event.is_valid());
    assert!(serde_json::to_vec(&event).unwrap().len() <= MAX_EVENT_BYTES);
}

#[test]
fn flush_sends_oldest_first_drops_rejections_and_keeps_failures() {
    let (_directory, store) = store();
    store.begin_session(&app(), noon()).unwrap();
    store.end_session().unwrap();
    store.begin_session(&app(), noon()).unwrap();
    store.end_session().unwrap();
    let queued: Vec<_> = store.queued().unwrap().into_iter().map(|e| e.id).collect();
    assert_eq!(queued.len(), 3);

    // Accepted, rejected (dropped for good), then a network failure stops the flush.
    let sender = ScriptedSender::new([Delivery::Accepted, Delivery::Rejected, Delivery::Failed]);
    let report = store.flush(&sender, noon()).unwrap();
    assert_eq!(sender.ids(), queued);
    assert_eq!(
        report,
        FlushReport {
            sent: 1,
            dropped: 1,
            remaining: 1,
            deferred: false
        }
    );
    // The kept event is retried with the same id.
    let retry = ScriptedSender::new([Delivery::Accepted]);
    assert_eq!(store.flush(&retry, noon()).unwrap().sent, 1);
    assert_eq!(retry.ids(), [queued[2].clone()]);
    assert!(store.queued().unwrap().is_empty());
    assert!(!store.directory().join(QUEUE_FILE).exists());
}

#[test]
fn a_retry_after_defers_every_flush_until_it_passes() {
    let (_directory, store) = store();
    store.begin_session(&app(), noon()).unwrap();
    let limited = ScriptedSender::new([Delivery::RetryAfter(Some(Duration::from_secs(120)))]);
    let report = store.flush(&limited, noon()).unwrap();
    assert_eq!(report.remaining, 1);

    let idle = ScriptedSender::new([]);
    let deferred = store
        .flush(&idle, noon() + Duration::from_secs(60))
        .unwrap();
    assert!(deferred.deferred);
    assert!(idle.ids().is_empty());

    let later = ScriptedSender::new([]);
    let report = store
        .flush(&later, noon() + Duration::from_secs(121))
        .unwrap();
    assert_eq!(report.sent, 1);
    assert_eq!(later.ids().len(), 1);
}

#[test]
fn a_retry_deadline_beyond_the_maximum_window_does_not_block_after_clock_rewind() {
    let (_directory, store) = store();
    let future = noon() + Duration::from_secs(7 * 24 * 60 * 60);
    store.begin_session(&app(), future).unwrap();
    let limited = ScriptedSender::new([Delivery::RetryAfter(Some(Duration::from_secs(120)))]);
    store.flush(&limited, future).unwrap();
    let queued = store.queued().unwrap();

    let reopened = TelemetryStore::new(store.directory());
    let retry = ScriptedSender::new([Delivery::Accepted]);
    let report = reopened.flush(&retry, noon()).unwrap();
    assert!(!report.deferred, "回拨后的旧截止时间不能挡住队列");
    assert_eq!(report.sent, queued.len());
    assert_eq!(retry.ids().len(), queued.len());
    assert_eq!(reopened.read_state().retry_after_unix_ms, 0);
}

#[test]
fn a_clock_rewind_replaces_an_invalid_deadline_with_the_new_retry_after() {
    let (_directory, store) = store();
    let future = noon() + Duration::from_secs(7 * 24 * 60 * 60);
    store.begin_session(&app(), future).unwrap();
    let limited = ScriptedSender::new([Delivery::RetryAfter(Some(Duration::from_secs(120)))]);
    store.flush(&limited, future).unwrap();

    let reopened = TelemetryStore::new(store.directory());
    let limited_again = ScriptedSender::new([Delivery::RetryAfter(Some(Duration::from_secs(120)))]);
    let report = reopened.flush(&limited_again, noon()).unwrap();
    assert!(!report.deferred, "失效的退避必须允许一次新请求");
    assert_eq!(limited_again.ids().len(), 1);
    let idle = ScriptedSender::new([]);
    assert!(
        reopened
            .flush(&idle, noon() + Duration::from_secs(60))
            .unwrap()
            .deferred
    );
    assert!(idle.ids().is_empty());
    let ready = ScriptedSender::new([Delivery::Accepted]);
    assert_eq!(
        reopened
            .flush(&ready, noon() + Duration::from_secs(120))
            .unwrap()
            .sent,
        1
    );
}

#[test]
fn a_retry_after_keeps_the_maximum_window_and_retries_at_its_boundary() {
    let (_directory, store) = store();
    store.begin_session(&app(), noon()).unwrap();
    let limited = ScriptedSender::new([Delivery::RetryAfter(Some(Duration::MAX))]);
    store.flush(&limited, noon()).unwrap();
    let idle = ScriptedSender::new([]);
    assert!(store.flush(&idle, noon()).unwrap().deferred);
    assert!(
        store
            .flush(&idle, noon() + MAX_RETRY_AFTER - Duration::from_millis(1))
            .unwrap()
            .deferred
    );
    assert!(idle.ids().is_empty());
    let ready = ScriptedSender::new([Delivery::Accepted]);
    assert_eq!(
        store.flush(&ready, noon() + MAX_RETRY_AFTER).unwrap().sent,
        1
    );
}

struct PausingSender {
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
    delivery: Delivery,
}

impl TelemetrySender for PausingSender {
    fn send(&self, _body: &[u8]) -> Delivery {
        self.entered.wait();
        self.release.wait();
        self.delivery
    }
}

#[test]
fn a_concurrent_flush_does_not_clear_another_flush_retry_deadline() {
    let (_directory, store) = store();
    store.begin_session(&app(), noon()).unwrap();

    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let first = store.clone();
    let first_sender = PausingSender {
        entered: entered.clone(),
        release: release.clone(),
        delivery: Delivery::Accepted,
    };
    let first_thread = std::thread::spawn(move || first.flush(&first_sender, noon()).unwrap());

    entered.wait();
    let second_sender = ScriptedSender::new([Delivery::RetryAfter(Some(Duration::from_secs(120)))]);
    let second = store.clone();
    let second_report = second.flush(&second_sender, noon()).unwrap();
    assert_eq!(second_report.remaining, 1);

    release.wait();
    first_thread.join().unwrap();

    let idle = ScriptedSender::new([]);
    let deferred = store
        .flush(&idle, noon() + Duration::from_secs(60))
        .unwrap();
    assert!(
        deferred.deferred,
        "a concurrent success must not erase Retry-After"
    );
    assert!(idle.ids().is_empty());
}

#[test]
fn an_earlier_flush_preserves_a_later_maximum_retry_deadline() {
    let (_directory, store) = store();
    store.begin_session(&app(), noon()).unwrap();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let first = store.clone();
    let first_sender = PausingSender {
        entered: entered.clone(),
        release: release.clone(),
        delivery: Delivery::Accepted,
    };
    let first_thread = std::thread::spawn(move || first.flush(&first_sender, noon()).unwrap());

    entered.wait();
    let later = noon() + Duration::from_secs(1);
    let limited = ScriptedSender::new([Delivery::RetryAfter(Some(Duration::MAX))]);
    store.flush(&limited, later).unwrap();
    let deadline = store.read_state().retry_after_unix_ms;
    release.wait();
    first_thread.join().unwrap();

    assert_eq!(store.read_state().retry_after_unix_ms, deadline);
    let idle = ScriptedSender::new([]);
    assert!(store.flush(&idle, later).unwrap().deferred);
    assert!(idle.ids().is_empty());
}

#[test]
fn telemetry_queue_capacity_is_bounded_by_the_queue_limit() {
    assert_eq!(super::telemetry_queue_capacity(0), 0);
    assert_eq!(
        super::telemetry_queue_capacity(MAX_QUEUED_EVENTS),
        MAX_QUEUED_EVENTS
    );
    assert_eq!(
        super::telemetry_queue_capacity(MAX_QUEUED_EVENTS + 10),
        MAX_QUEUED_EVENTS
    );
}

#[test]
fn crash_record_paths_reserve_one_start_batch() {
    let directory = tempfile::tempdir().unwrap();
    let empty = super::crash_record_paths(directory.path()).unwrap();
    assert!(empty.is_empty());
    assert_eq!(empty.capacity(), 0);
    std::fs::write(directory.path().join("synthetic.crash"), b"fixture").unwrap();
    let records = super::crash_record_paths(directory.path()).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records.capacity(), super::MAX_CRASH_RECORDS_PER_START);
}

#[test]
fn the_queue_keeps_the_newest_sixty_four() {
    let (_directory, store) = store();
    let start = noon();
    for day in 0..70u64 {
        store
            .record_active(&app(), start + Duration::from_secs(day * 86_400))
            .unwrap();
    }
    let queue = store.queued().unwrap();
    assert_eq!(queue.len(), MAX_QUEUED_EVENTS);
    assert!(queue[0].id.ends_with("20261007"));
}

#[cfg(unix)]
#[test]
fn telemetry_state_symlink_is_ignored() {
    use std::os::unix::fs::symlink;

    let (directory, store) = store();
    let external = directory.path().join("external-state.json");
    let external_state = serde_json::json!({
        "install_id": "external-install-id-1234",
        "active_day": "20261001",
        "retry_after_unix_ms": 0
    });
    let external_bytes = serde_json::to_vec(&external_state).unwrap();
    std::fs::write(&external, &external_bytes).unwrap();
    std::fs::create_dir_all(store.directory()).unwrap();
    symlink(&external, store.directory().join(STATE_FILE)).unwrap();

    // 外部状态文件不能成为本地安装 id 的来源，也不能被修改。
    let install_id = store.install_id().unwrap();
    assert_ne!(install_id, "external-install-id-1234");
    assert_eq!(std::fs::read(&external).unwrap(), external_bytes);
}

#[cfg(unix)]
#[test]
fn telemetry_queue_symlink_is_ignored() {
    use std::os::unix::fs::symlink;

    let (directory, store) = store();
    let external = directory.path().join("external-queue.json");
    let event = TelemetryEvent {
        id: "0123456789abcdef0123456789abcdef".to_owned(),
        kind: TelemetryKind::Session,
        platform: "linux".to_owned(),
        version: "0.50.0".to_owned(),
        message: String::new(),
        stack: String::new(),
        artifact: String::new(),
        channel: String::new(),
        install_id: "external-install-id-1234".to_owned(),
    };
    let external_bytes = serde_json::to_vec(&vec![event]).unwrap();
    std::fs::write(&external, &external_bytes).unwrap();
    std::fs::create_dir_all(store.directory()).unwrap();
    symlink(&external, store.directory().join(QUEUE_FILE)).unwrap();

    // 外部队列不能被读取，原文件也不能被替换或删除。
    assert!(store.queued().unwrap().is_empty());
    assert!(
        std::fs::symlink_metadata(store.directory().join(QUEUE_FILE))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read(&external).unwrap(), external_bytes);
}

#[cfg(unix)]
#[test]
fn a_symlinked_previous_crash_marker_is_not_counted() {
    use std::os::unix::fs::symlink;

    let (directory, store) = store();
    let first = store.begin_session(&app(), noon()).unwrap();
    let external = directory.path().join("external-crash-record.crash");
    std::fs::write(&external, b"external crash\nexternal stack\n").unwrap();
    symlink(&external, &first.crash_record_path).unwrap();

    let next = store.begin_session(&app(), noon()).unwrap();
    // 外部 crash 文件不能让上一会话被标记为崩溃，也不能被删除。
    assert!(!next.previous_session_crashed);
    assert_eq!(next.crashes, 0);
    assert_eq!(
        std::fs::read(&external).unwrap(),
        b"external crash\nexternal stack\n"
    );
}

#[cfg(unix)]
#[test]
fn a_symlinked_session_marker_is_ignored() {
    use std::os::unix::fs::symlink;

    let (directory, store) = store();
    let external = directory.path().join("external-session.json");
    let marker = serde_json::json!({
        "id": "01234567-89ab-cdef-0123-456789abcdef",
        "platform": "linux",
        "version": "0.50.0",
        "started_at_unix_ms": 1790856000000u64
    });
    let external_bytes = serde_json::to_vec(&marker).unwrap();
    std::fs::write(&external, &external_bytes).unwrap();
    std::fs::create_dir_all(store.directory()).unwrap();
    symlink(&external, store.directory().join(SESSION_FILE)).unwrap();

    // 外部 session marker 不能被当作本地运行中的会话。
    assert!(store.read_marker().is_none());
    assert_eq!(std::fs::read(&external).unwrap(), external_bytes);
}

#[cfg(unix)]
#[test]
fn a_symlinked_crash_file_is_not_read() {
    use std::os::unix::fs::symlink;

    let (directory, _store) = store();
    let external = directory.path().join("external.crash");
    let link = directory.path().join("linked.crash");
    std::fs::write(&external, b"external crash\nexternal stack\n").unwrap();
    symlink(&external, &link).unwrap();

    // crash 叶子是外部链接时必须拒绝读取。
    assert!(read_crash_file(&link).is_none());
    assert_eq!(
        std::fs::read(&external).unwrap(),
        b"external crash\nexternal stack\n"
    );
}

#[test]
fn clear_drops_everything_but_the_install_id() {
    let (_directory, store) = store();
    let install_id = store.install_id().unwrap();
    store.begin_session(&app(), noon()).unwrap();
    store.record_crash("boom", "").unwrap();
    store.clear().unwrap();
    assert!(store.queued().unwrap().is_empty());
    assert!(!store.end_session().unwrap());
    assert!(!store.directory().join(CRASH_DIRECTORY).exists());
    assert_eq!(store.install_id().unwrap(), install_id);
    // The day's active is queued again once reporting is back on.
    store.record_active(&app(), noon()).unwrap();
    assert_eq!(store.queued().unwrap().len(), 1);
}

#[test]
fn a_relative_directory_is_refused() {
    let store = TelemetryStore::new("relative/telemetry");
    assert_eq!(store.install_id(), Err(TelemetryError::Storage));
}

#[test]
fn statuses_map_to_the_retry_rules() {
    assert_eq!(
        delivery_for_status(StatusCode::ACCEPTED, None),
        Delivery::Accepted
    );
    assert_eq!(
        delivery_for_status(StatusCode::BAD_REQUEST, None),
        Delivery::Rejected
    );
    assert_eq!(
        delivery_for_status(StatusCode::TOO_MANY_REQUESTS, Some(Duration::from_secs(9))),
        Delivery::RetryAfter(Some(Duration::from_secs(9)))
    );
    assert_eq!(
        delivery_for_status(StatusCode::SERVICE_UNAVAILABLE, None),
        Delivery::RetryAfter(None)
    );
    assert_eq!(
        delivery_for_status(StatusCode::BAD_GATEWAY, None),
        Delivery::Failed
    );
    assert_eq!(
        delivery_for_status(StatusCode::UNAUTHORIZED, None),
        Delivery::Failed
    );
}

fn serve(responses: Vec<&'static str>) -> (String, std::sync::mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for response in responses {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut head = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
                if line == "\r\n" {
                    break;
                }
                head.push_str(&line);
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            sender
                .send(format!("{head}\n{}", String::from_utf8(body).unwrap()))
                .unwrap();
            std::io::Write::write_all(reader.get_mut(), response.as_bytes()).unwrap();
        }
    });
    (origin, receiver)
}

#[test]
fn the_backend_sender_posts_anonymously_and_reads_retry_after() {
    let (origin, requests) = serve(vec![
        "HTTP/1.1 202 Accepted\r\nContent-Length: 17\r\nConnection: close\r\n\r\n{\"accepted\":true}",
        "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 30\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    ]);
    let client = BackendAccountClient::loopback(&origin).unwrap();
    assert_eq!(client.send(b"{\"id\":\"x\"}"), Delivery::Accepted);
    let request = requests.recv().unwrap();
    assert!(request.starts_with("POST /v1/telemetry/events HTTP/1.1"));
    assert!(request
        .to_ascii_lowercase()
        .contains("content-type: application/json"));
    assert!(!request.to_ascii_lowercase().contains("authorization"));
    assert!(request.ends_with("{\"id\":\"x\"}"));
    assert_eq!(
        client.send(b"{}"),
        Delivery::RetryAfter(Some(Duration::from_secs(30)))
    );
    assert_eq!(client.send(b"{}"), Delivery::Rejected);
    assert_eq!(
        BackendAccountClient::loopback("http://127.0.0.1:9")
            .unwrap()
            .send(b"{}"),
        Delivery::Failed
    );
}

#[cfg(unix)]
#[test]
fn finished_scope_releases_inherited_lock() {
    let root = tempfile::tempdir().unwrap();
    let store = TelemetryStore::new(root.path());
    let held = store.lock().unwrap();
    let inherited = crate::file_lock::testing::duplicate(&held);
    crate::file_lock::testing::assert_finished_scope_releases_lock(
        held,
        inherited,
        &root.path().join(LOCK_FILE),
    );
}

#[test]
fn a_full_crash_queue_keeps_later_records_on_disk_until_delivery_frees_space() {
    let (_directory, store) = store();
    let mut last = None;
    for _ in 0..=MAX_QUEUED_EVENTS {
        let capture = store.prepare_crash_capture(&app(), noon()).unwrap();
        store
            .record_crash_for(
                &capture.session_id,
                "Synthetic failure",
                "libsynthetic.so+0x1",
            )
            .unwrap();
        store.record_active(&app(), noon()).unwrap();
        last = Some(capture.crash_record_path);
    }
    let last = last.unwrap();
    assert_eq!(store.queued().unwrap().len(), MAX_QUEUED_EVENTS);
    assert!(store
        .queued()
        .unwrap()
        .iter()
        .all(|event| event.kind == TelemetryKind::Crash));
    assert!(last.is_file());
    store
        .record_active(&app(), noon() + Duration::from_secs(86400))
        .unwrap();
    assert!(last.is_file(), "新的日活不能挤掉待发崩溃");
    let sender = ScriptedSender::new([]);
    assert_eq!(
        store.flush(&sender, noon()).unwrap().sent,
        MAX_QUEUED_EVENTS
    );
    store.record_active(&app(), noon()).unwrap();
    assert!(!last.exists());
    assert_eq!(
        store
            .queued()
            .unwrap()
            .iter()
            .filter(|event| event.kind == TelemetryKind::Crash)
            .count(),
        1
    );
}

#[test]
fn delayed_system_diagnostics_keep_a_stable_event_identity() {
    let (_directory, store) = store();
    let capture = store.prepare_crash_capture(&app(), noon()).unwrap();
    let context = CrashContext::from_header(&capture.crash_record_header).unwrap();
    let record = || {
        store
            .record_diagnosed_crash_with_context(
                None,
                Some(&context),
                Some("synthetic-123-456"),
                "Synthetic system diagnostic",
                "libsynthetic.so+0x1",
            )
            .unwrap()
    };
    assert_eq!(record(), DiagnosedCrash::Outside);
    assert_eq!(record(), DiagnosedCrash::Duplicate);
    store.record_active(&app(), noon()).unwrap();
    let first = store
        .queued()
        .unwrap()
        .into_iter()
        .find(|event| event.kind == TelemetryKind::Crash)
        .unwrap();
    assert_eq!(record(), DiagnosedCrash::Duplicate);
    store.record_active(&app(), noon()).unwrap();
    let crashes: Vec<_> = store
        .queued()
        .unwrap()
        .into_iter()
        .filter(|event| event.kind == TelemetryKind::Crash)
        .collect();
    assert_eq!(crashes, [first]);
}

#[test]
fn system_diagnostics_stay_received_after_delivery_and_restart() {
    let (directory, store) = store();
    let capture = store.prepare_crash_capture(&app(), noon()).unwrap();
    let context = CrashContext::from_header(&capture.crash_record_header).unwrap();
    assert!(store
        .record_diagnosed_capture(&capture.session_id, &context, "Synthetic App fault", "")
        .unwrap());
    store.record_active(&app(), noon()).unwrap();
    let sender = ScriptedSender::new([]);
    store.flush(&sender, noon()).unwrap();
    assert!(store.queued().unwrap().is_empty());
    let restarted = TelemetryStore::new(directory.path().join("state"));
    assert!(!restarted
        .record_diagnosed_capture(&capture.session_id, &context, "Synthetic App fault", "")
        .unwrap());
    restarted.record_active(&app(), noon()).unwrap();
    assert!(restarted.queued().unwrap().is_empty());
    assert!(!capture.crash_record_path.exists());
    assert_eq!(
        sender
            .ids()
            .iter()
            .filter(|id| id.starts_with("crash-"))
            .count(),
        1
    );

    // 即使另一个旧进程重建了同名现场文件，收件端也不能再次排队。
    fs::write(
        &capture.crash_record_path,
        format!("{}Synthetic replay\n", capture.crash_record_header),
    )
    .unwrap();
    restarted.record_active(&app(), noon()).unwrap();
    assert!(restarted.queued().unwrap().is_empty());
    assert!(!capture.crash_record_path.exists());

    let identity = "synthetic-ANR-history";
    assert_eq!(
        restarted
            .record_diagnosed_crash_with_context(
                None,
                Some(&context),
                Some(identity),
                "Hang Android ANR",
                ""
            )
            .unwrap(),
        DiagnosedCrash::Outside
    );
    restarted.record_active(&app(), noon()).unwrap();
    restarted.flush(&sender, noon()).unwrap();
    let again = TelemetryStore::new(directory.path().join("state"));
    assert_eq!(
        again
            .record_diagnosed_crash_with_context(
                None,
                Some(&context),
                Some(identity),
                "Hang Android ANR",
                ""
            )
            .unwrap(),
        DiagnosedCrash::Duplicate
    );
    again.record_active(&app(), noon()).unwrap();
    assert!(again.queued().unwrap().is_empty());
}

#[test]
fn received_crash_receipts_require_durable_queue_and_are_bounded() {
    let (_directory, store) = store();
    let capture = store.prepare_crash_capture(&app(), noon()).unwrap();
    let context = CrashContext::from_header(&capture.crash_record_header).unwrap();
    assert!(store
        .record_diagnosed_capture(&capture.session_id, &context, "Synthetic fault", "")
        .unwrap());
    // 强制队列发布失败：收件失败不能提前保存收据或删除原记录。
    let queue = store.directory.join(QUEUE_FILE);
    fs::remove_file(&queue).unwrap();
    fs::create_dir(&queue).unwrap();
    assert!(store.record_active(&app(), noon()).is_err());
    assert!(store.read_state().received_crashes.is_empty());
    assert!(capture.crash_record_path.exists());
    fs::remove_dir(&queue).unwrap();
    store.record_active(&app(), noon()).unwrap();
    assert_eq!(
        store.read_state().received_crashes,
        [Uuid::parse_str(&capture.session_id).unwrap()]
    );
    let sender = ScriptedSender::new([]);
    for _ in 0..MAX_RECEIVED_CRASHES {
        let next = store.prepare_crash_capture(&app(), noon()).unwrap();
        assert!(store
            .record_diagnosed_capture(&next.session_id, &context, "Synthetic next fault", "")
            .unwrap());
        store.record_active(&app(), noon()).unwrap();
        store.flush(&sender, noon()).unwrap();
    }
    let state = store.read_state();
    assert_eq!(state.received_crashes.len(), MAX_RECEIVED_CRASHES);
    assert!(!state
        .received_crashes
        .contains(&Uuid::parse_str(&capture.session_id).unwrap()));
    assert!(
        fs::metadata(store.directory.join(STATE_FILE))
            .unwrap()
            .len()
            < MAX_SMALL_FILE_BYTES
    );
    store.clear().unwrap();
    assert!(store.read_state().received_crashes.is_empty());
}

#[test]
fn maximum_sessions_and_receipts_fit_the_state_read_limit() {
    let (_directory, store) = store();
    let capture = store.prepare_crash_capture(&app(), noon()).unwrap();
    let mut context = CrashContext::from_header(&capture.crash_record_header).unwrap();
    context.version = "v".repeat(64);
    context.revision = "r".repeat(64);
    context.run = "n".repeat(64);
    context.attempt = "a".repeat(64);
    context.arch = "x".repeat(64);
    context.component = "c".repeat(64);
    context.consent_epoch = Some(Uuid::new_v4());
    assert!(context.is_valid());
    let marker = SessionMarker {
        id: capture.session_id,
        platform: context.platform.clone(),
        version: context.version.clone(),
        started_at_unix_ms: u64::MAX,
        pid: Some(u32::MAX),
        context: Some(context),
    };
    let mut state = store.read_state();
    state.open_sessions = vec![marker; MAX_OPEN_SESSIONS];
    state.received_crashes = (0..MAX_RECEIVED_CRASHES).map(|_| Uuid::new_v4()).collect();
    state.crashed_processes = vec![u32::MAX; MAX_CRASHED_PROCESSES];
    store.write_state(&state).unwrap();
    let length = fs::metadata(store.directory.join(STATE_FILE))
        .unwrap()
        .len();
    assert!(length > MAX_SMALL_FILE_BYTES && length < MAX_STATE_FILE_BYTES);
    assert_eq!(store.read_state().received_crashes, state.received_crashes);
    assert_eq!(store.read_state().open_sessions.len(), MAX_OPEN_SESSIONS);
}

#[test]
fn collecting_another_hosts_crashes_does_not_mark_it_active_today() {
    let (_directory, store) = store();
    let session = store.begin_session(&app(), noon()).unwrap();
    let sender = ScriptedSender::new([]);
    assert_eq!(store.flush(&sender, noon()).unwrap().sent, 1);
    let active_day = store.read_state().active_day;
    store
        .record_crash_for(
            &session.session_id,
            "Synthetic fatal error",
            "synthetic.so+0x123",
        )
        .unwrap();
    let collector = TelemetryApp::new("linux", "99.0.0")
        .unwrap()
        .with_component("desktop")
        .unwrap();
    store.collect_crashes(&collector).unwrap();
    let events = store.queued().unwrap();
    assert_eq!(events.len(), 2);
    assert!(events
        .iter()
        .any(|event| event.kind == TelemetryKind::Crash));
    assert!(events
        .iter()
        .any(|event| event.kind == TelemetryKind::SessionCrash));
    assert!(events.iter().all(|event| event.version == app().version()));
    assert_eq!(store.read_state().active_day, active_day);
    assert!(!store.directory.join(SESSION_FILE).exists());
    assert!(!session.crash_record_path.exists());
    assert_eq!(store.flush(&sender, noon()).unwrap().sent, 2);
    TelemetryStore::new(store.directory())
        .collect_crashes(&collector)
        .unwrap();
    assert!(store.queued().unwrap().is_empty());
}
