use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

fn fixture() -> (tempfile::TempDir, TelemetryStore, TelemetryApp) {
    let root = tempfile::tempdir().unwrap();
    let store = TelemetryStore::new(root.path().join("telemetry"));
    let app = TelemetryApp::new("ios", "0.1.0").unwrap();
    (root, store, app)
}

#[test]
fn rotation_preserves_installation_and_rejects_obsolete_store_operations() {
    let (_root, store, app) = fixture();
    let install = store.install_id().unwrap();
    let first_epoch = Uuid::new_v4();
    let first = store.activate_consent_epoch(first_epoch).unwrap();
    let session = first.begin_session(&app, SystemTime::now()).unwrap();
    first.end_session_by_id(&session.session_id).unwrap();
    let queued = first.queued().unwrap();
    assert_eq!(queued.len(), 2);
    let reopened = store.activate_consent_epoch(first_epoch).unwrap();
    assert_eq!(reopened.queued().unwrap(), queued);

    let second = store.activate_consent_epoch(Uuid::new_v4()).unwrap();
    assert!(second.queued().unwrap().is_empty());
    assert_eq!(second.install_id().unwrap(), install);
    assert_eq!(first.queued(), Err(TelemetryError::ConsentChanged));
    assert_eq!(first.clear(), Err(TelemetryError::ConsentChanged));
    assert_eq!(
        first.end_session_by_id(&session.session_id),
        Err(TelemetryError::ConsentChanged)
    );
    assert_eq!(
        first.begin_session(&app, SystemTime::now()),
        Err(TelemetryError::ConsentChanged)
    );
    assert!(second.queued().unwrap().is_empty());
}

#[test]
fn late_old_native_records_and_legacy_records_are_discarded() {
    let (_root, store, app) = fixture();
    let first = store.activate_consent_epoch(Uuid::new_v4()).unwrap();
    let old = first.begin_session(&app, SystemTime::now()).unwrap();
    let second = store.activate_consent_epoch(Uuid::new_v4()).unwrap();
    second.begin_session(&app, SystemTime::now()).unwrap();
    // 模拟旧进程在换代之后才完成的原生写盘；目录仍由新进程使用。
    std::fs::write(
        &old.crash_record_path,
        format!(
            "{}Synthetic old fault\nsynthetic+0x10\n",
            old.crash_record_header
        ),
    )
    .unwrap();
    let legacy = old
        .crash_record_path
        .with_file_name(format!("{}.crash", Uuid::new_v4()));
    std::fs::write(&legacy, "Synthetic legacy fault\nsynthetic+0x20\n").unwrap();
    second.record_active(&app, SystemTime::now()).unwrap();
    assert!(!old.crash_record_path.exists());
    assert!(!legacy.exists());
    assert!(second
        .queued()
        .unwrap()
        .iter()
        .all(|event| event.kind == TelemetryKind::Active));
}

#[test]
fn first_consent_binding_migrates_a_legacy_crash_record_once() {
    let (_root, store, app) = fixture();
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

    let bound = store.activate_consent_epoch(Uuid::new_v4()).unwrap();
    let start = bound.begin_session(&app, SystemTime::now()).unwrap();
    assert!(start.previous_session_crashed);
    assert!(bound
        .queued()
        .unwrap()
        .iter()
        .any(|event| event.kind == TelemetryKind::SessionCrash));
}

#[test]
fn same_epoch_restart_collects_native_fault_without_uploading_epoch() {
    let (_root, store, app) = fixture();
    let epoch = Uuid::new_v4();
    let first = store.activate_consent_epoch(epoch).unwrap();
    let capture = first.begin_session(&app, SystemTime::now()).unwrap();
    assert_eq!(
        CrashContext::from_header(&capture.crash_record_header)
            .unwrap()
            .consent_epoch,
        Some(epoch)
    );
    std::fs::write(
        &capture.crash_record_path,
        format!(
            "{}Synthetic current fault\nsynthetic+0x30\n",
            capture.crash_record_header
        ),
    )
    .unwrap();
    let restarted = store.activate_consent_epoch(epoch).unwrap();
    let next = restarted.begin_session(&app, SystemTime::now()).unwrap();
    assert_eq!(next.crashes, 1);
    assert!(next.previous_session_crashed);
    let events = restarted.queued().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == TelemetryKind::Crash)
            .count(),
        1
    );
    let body = serde_json::to_string(&events).unwrap();
    assert!(!body.contains("consent_epoch"));
    assert!(!body.contains(&epoch.to_string()));
    assert!(!capture.crash_record_path.exists());
}

#[test]
fn bound_writes_refuse_another_epoch_and_clear_invalidates_bindings() {
    let (_root, store, app) = fixture();
    let empty = store.activate_consent_epoch(Uuid::new_v4()).unwrap();
    assert!(empty.read_state().install_id.is_empty());
    empty.clear().unwrap();
    assert!(store.read_state().consent_required);
    assert_eq!(
        store.begin_session(&app, SystemTime::now()),
        Err(TelemetryError::ConsentChanged)
    );
    assert_eq!(empty.queued(), Err(TelemetryError::ConsentChanged));
    let first = store.activate_consent_epoch(Uuid::new_v4()).unwrap();
    let capture = first
        .prepare_crash_capture(&app, SystemTime::now())
        .unwrap();
    let mut wrong = CrashContext::from_header(&capture.crash_record_header).unwrap();
    wrong.consent_epoch = Some(Uuid::new_v4());
    assert_eq!(
        first.record_crash_with_context(&capture.session_id, Some(&wrong), "Synthetic", ""),
        Err(TelemetryError::ConsentChanged)
    );
    assert_eq!(
        first.record_crash_with_context(&capture.session_id, None, "Synthetic", ""),
        Err(TelemetryError::ConsentChanged)
    );
    assert!(!capture.crash_record_path.exists());
    let id = store.install_id().unwrap();
    first.clear().unwrap();
    assert_eq!(first.queued(), Err(TelemetryError::ConsentChanged));
    assert_eq!(store.install_id().unwrap(), id);
}

struct PausedSender {
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
    calls: Arc<AtomicUsize>,
    delivery: Delivery,
}

impl TelemetrySender for PausedSender {
    fn send(&self, _body: &[u8]) -> Delivery {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            self.entered.wait();
            self.release.wait();
        }
        self.delivery
    }
}

#[test]
fn in_flight_old_flush_cannot_change_new_queue_or_retry_state() {
    for delivery in [
        Delivery::Accepted,
        Delivery::RetryAfter(Some(Duration::from_secs(120))),
    ] {
        let (_root, store, app) = fixture();
        let first = store.activate_consent_epoch(Uuid::new_v4()).unwrap();
        let session = first.begin_session(&app, SystemTime::now()).unwrap();
        first.end_session_by_id(&session.session_id).unwrap();
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let calls = Arc::new(AtomicUsize::new(0));
        let sender = PausedSender {
            entered: entered.clone(),
            release: release.clone(),
            calls: calls.clone(),
            delivery,
        };
        let worker = std::thread::spawn(move || first.flush(&sender, SystemTime::now()));
        entered.wait();
        let second = store.activate_consent_epoch(Uuid::new_v4()).unwrap();
        second
            .begin_session(
                &TelemetryApp::new("ios", "0.2.0").unwrap(),
                SystemTime::now(),
            )
            .unwrap();
        let fresh = second.queued().unwrap();
        release.wait();
        assert_eq!(worker.join().unwrap(), Err(TelemetryError::ConsentChanged));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(second.queued().unwrap(), fresh);
        assert_eq!(second.read_state().retry_after_unix_ms, 0);
    }
}

#[cfg(unix)]
#[test]
fn bound_collection_waits_for_current_native_writer() {
    use std::io::Write;
    let (_root, store, app) = fixture();
    let bound = store.activate_consent_epoch(Uuid::new_v4()).unwrap();
    let capture = bound
        .prepare_crash_capture(&app, SystemTime::now())
        .unwrap();
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
    bound.record_active(&app, SystemTime::now()).unwrap();
    assert!(capture.crash_record_path.exists());
    writer
        .write_all(b"Synthetic fault\nsynthetic+0x40\n")
        .unwrap();
    drop(held);
    bound.record_active(&app, SystemTime::now()).unwrap();
    assert!(!capture.crash_record_path.exists());
    assert!(bound
        .queued()
        .unwrap()
        .iter()
        .any(|event| event.kind == TelemetryKind::Crash));
}

#[test]
fn bound_directory_rejects_unbound_normal_operations_but_keeps_session_ownership() {
    let (_root, store, app) = fixture();
    let bound = store.activate_consent_epoch(Uuid::new_v4()).unwrap();
    let session = bound.begin_session(&app, SystemTime::now()).unwrap();
    assert_eq!(
        store.begin_session(&app, SystemTime::now()),
        Err(TelemetryError::ConsentChanged)
    );
    assert_eq!(
        store.prepare_crash_capture(&app, SystemTime::now()),
        Err(TelemetryError::ConsentChanged)
    );
    assert_eq!(
        store.record_active(&app, SystemTime::now()),
        Err(TelemetryError::ConsentChanged)
    );
    assert_eq!(store.clear(), Err(TelemetryError::ConsentChanged));
    struct NeverSend;
    impl TelemetrySender for NeverSend {
        fn send(&self, _: &[u8]) -> Delivery {
            panic!("漏接入口不得发送");
        }
    }
    assert_eq!(
        store.flush(&NeverSend, SystemTime::now()),
        Err(TelemetryError::ConsentChanged)
    );
    assert_eq!(store.end_session(), Err(TelemetryError::ConsentChanged));
    assert!(store.end_session_by_id(&session.session_id).unwrap());
}
