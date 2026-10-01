use super::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpListener;

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
    assert_eq!(crash.message, "std::runtime_error: bad state in data.bin");
    assert_eq!(
        crash.stack,
        "msime-server.exe+0x1a2b\nmsime_host_api.dll+0x77"
    );
    assert!(!crash.stack.contains("Alice"));
    assert!(crash.is_valid());
    assert!(session_crash.id.starts_with("session-"));
    assert!(crash.id.starts_with("crash-"));
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
    assert_eq!(crash.message, "SIGSEGV");
    assert_eq!(
        crash.stack,
        "libmsime_host_api.so(+0x1f2e)[0x7f00]\nlibc.so.6(+0x3c050)[0x7f01]\n\u{fffd}"
    );
}

#[test]
fn record_crash_without_a_session_writes_nothing() {
    let (_directory, store) = store();
    assert!(!store.record_crash("boom", "").unwrap());
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

#[test]
fn a_legacy_queue_loses_its_download_events_and_short_ids() {
    let (_directory, store) = store();
    std::fs::create_dir_all(store.directory()).unwrap();
    std::fs::write(
        store.directory().join(QUEUE_FILE),
        serde_json::to_vec(&serde_json::json!([
            {"id": "1f2e3d4c5b6a7980", "kind": "download", "platform": "linux", "version": "0.4.0"},
            {"id": "abc123", "kind": "crash", "platform": "linux", "version": "0.4.0", "message": "std::terminate"},
            {"id": "0123456789abcdef0123", "kind": "crash", "platform": "windows", "version": "0.4.0", "message": "std::terminate"},
            "garbage",
            {"id": "0123456789abcdef9999", "kind": "crash", "platform": "linux", "version": "0.4.0"}
        ]))
        .unwrap(),
    )
    .unwrap();
    let install_id = store.install_id().unwrap();
    let queue = store.queued().unwrap();
    assert_eq!(queue.len(), 2);
    assert!(queue.iter().all(|event| event.kind == TelemetryKind::Crash));
    assert!(queue.iter().all(|event| event.id.len() >= 16));
    assert!(queue.iter().all(|event| event.install_id == install_id));
    assert_eq!(queue[1].id, "0123456789abcdef0123");
    // The regenerated id was written back, so every later attempt sends the same one.
    assert_eq!(store.queued().unwrap(), queue);
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
