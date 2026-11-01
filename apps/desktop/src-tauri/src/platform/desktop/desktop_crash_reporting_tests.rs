use super::*;
use msime_client_services::telemetry::{
    CrashContext, Delivery, TelemetryKind, CRASH_DIRECTORY, SESSION_FILE,
};
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Barrier};

#[cfg(target_os = "macos")]
#[test]
#[ignore = "由 macos_directory_uses_the_native_system_home 在独立进程中运行"]
fn macos_directory_child() {
    let root = PathBuf::from(std::env::var_os("CFFIXED_USER_HOME").unwrap());
    let mut expected = root.join("Library/Application Support/MSIME");
    let edition = crate::package_edition();
    if edition.id != "full" {
        expected = expected.join(&edition.id);
    }
    assert!(
        directory() == Some(expected.join("telemetry")),
        "设置与原生宿主必须使用同一个系统 Application Support 目录"
    );
    assert!(!root.join("Library").exists(), "解析目录不应创建队列");
}

#[cfg(target_os = "macos")]
#[test]
fn macos_directory_uses_the_native_system_home() {
    let root = tempfile::tempdir().unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "platform::desktop::desktop_crash_reporting::tests::macos_directory_child",
            "--ignored",
            "--nocapture",
        ])
        .env("CFFIXED_USER_HOME", root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn reporter(root: &Path) -> Reporter {
    Reporter {
        preferences: Arc::new(PreferencesStore::new(root.join("preferences"))),
        store: TelemetryStore::new(root.join("telemetry")),
        app: TelemetryApp::new("linux", "99.0.0")
            .unwrap()
            .with_component("desktop")
            .unwrap(),
        state: Mutex::new(State::default()),
    }
}

fn set_enabled(preferences: &PreferencesStore, enabled: bool) {
    let snapshot = preferences.load().unwrap();
    let mut changed = snapshot.preferences;
    changed.usage_reporting = enabled;
    preferences.save(snapshot.revision, changed).unwrap();
}

/// 全局故障处理器仅安装在隔离子进程，且不经过会创建网络发送线程的产品 start。
#[test]
#[ignore = "由 capture_consent_lifecycle 使用合成目录启动"]
#[allow(unsafe_code)]
fn child_process() {
    let root = PathBuf::from(std::env::var_os("MSIME_DESKTOP_CRASH_TEST_ROOT").unwrap());
    let mode = std::env::var("MSIME_DESKTOP_CRASH_TEST_MODE").unwrap();
    let reporter = reporter(&root);
    let mut state = State::default();
    unsafe { msime_native_crash_install() };
    reporter.refresh_capture(&mut state).unwrap();
    let first = state.capture.clone().unwrap();
    match mode.as_str() {
        "epoch-cycle" | "stale-late" => {
            set_enabled(&reporter.preferences, false);
            set_enabled(&reporter.preferences, true);
            if mode == "epoch-cycle" {
                let consent = reporter.refresh_capture(&mut state).unwrap();
                let current = state.capture.as_ref().unwrap();
                assert_ne!(current.1.session_id, first.1.session_id);
                assert_ne!(consent.epoch, first.0.epoch);
            } else {
                // 新宿主已准备新队列，旧进程尚未观察撤回，此时真实故障只能留下旧代次头。
                let consent = reporter
                    .preferences
                    .usage_reporting_consent()
                    .unwrap()
                    .unwrap();
                reporter
                    .preferences
                    .with_usage_reporting_consent(|_, _| {
                        reporter
                            .store
                            .activate_consent_epoch(consent.epoch)
                            .unwrap()
                            .prepare_crash_capture(&reporter.app, SystemTime::now())
                            .unwrap();
                    })
                    .unwrap();
            }
        }
        "unreadable" | "read-recovery" => {
            let path = reporter.preferences.directory().join("preferences.json");
            let saved = std::fs::read(&path).unwrap();
            std::fs::write(&path, b"{").unwrap();
            assert!(reporter.refresh_capture(&mut state).is_none());
            assert!(state.suspended);
            assert_eq!(state.capture.as_ref().unwrap().1, first.1);
            if mode == "read-recovery" {
                std::fs::write(&path, saved).unwrap();
                reporter.refresh_capture(&mut state).unwrap();
                assert!(!state.suspended);
                assert_eq!(state.capture.as_ref().unwrap().1, first.1);
            }
        }
        "prepared-off" | "prepared-cycle" => {
            state.suspend();
            set_enabled(&reporter.preferences, false);
            if mode == "prepared-cycle" {
                set_enabled(&reporter.preferences, true);
            }
            assert!(reporter.arm_capture(&mut state, first.0).is_none());
            assert!(state.suspended);
        }
        "off" => {
            set_enabled(&reporter.preferences, false);
            assert!(reporter.refresh_capture(&mut state).is_none());
            assert!(state.capture.is_none());
            assert!(state.cleared);
        }
        "on" => {}
        _ => panic!("unknown synthetic mode"),
    }
    std::process::abort();
}

#[test]
fn capture_consent_lifecycle() {
    for mode in [
        "on",
        "off",
        "epoch-cycle",
        "read-recovery",
        "unreadable",
        "stale-late",
        "prepared-off",
        "prepared-cycle",
    ] {
        let root = tempfile::tempdir().unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "platform::desktop::desktop_crash_reporting::tests::child_process",
                "--ignored",
                "--nocapture",
            ])
            .env("MSIME_DESKTOP_CRASH_TEST_ROOT", root.path())
            .env("MSIME_DESKTOP_CRASH_TEST_MODE", mode)
            .output()
            .unwrap();
        assert_eq!(
            output.status.signal(),
            Some(6),
            "{mode}: child must abort: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let reporter = reporter(root.path());
        let records: Vec<_> = std::fs::read_dir(reporter.store.directory().join(CRASH_DIRECTORY))
            .into_iter()
            .flatten()
            .flatten()
            .collect();
        let expected = !matches!(
            mode,
            "off" | "unreadable" | "prepared-off" | "prepared-cycle"
        );
        assert_eq!(
            records.len(),
            usize::from(expected),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !reporter.store.directory().join(SESSION_FILE).exists(),
            "{mode}"
        );
        if !expected {
            continue;
        }
        let record = std::fs::read_to_string(records[0].path()).unwrap();
        assert!(record.contains("SIGABRT"), "{mode}: {record}");
        let consent = reporter
            .preferences
            .usage_reporting_consent()
            .unwrap()
            .unwrap();
        let context = CrashContext::from_header(record.lines().next().unwrap()).unwrap();
        assert_eq!(
            context.consent_epoch == Some(consent.epoch),
            mode != "stale-late",
            "{mode}"
        );
        let bound = reporter
            .preferences
            .with_usage_reporting_consent(|_, _| {
                reporter
                    .store
                    .activate_consent_epoch(consent.epoch)
                    .unwrap()
            })
            .unwrap();
        bound
            .record_active(&reporter.app, SystemTime::now())
            .unwrap();
        let events = bound.queued().unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind == TelemetryKind::Crash)
                .count(),
            usize::from(mode != "stale-late"),
            "{mode}"
        );
        assert!(
            events
                .iter()
                .all(|event| !event.message.contains(&consent.epoch.to_string())),
            "{mode}"
        );
        assert!(
            events.iter().all(|event| !matches!(
                event.kind,
                TelemetryKind::Session | TelemetryKind::SessionCrash
            )),
            "{mode}"
        );
    }
}

struct PausedSender {
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
    calls: AtomicUsize,
}

impl TelemetrySender for PausedSender {
    fn send(&self, _body: &[u8]) -> Delivery {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            self.entered.wait();
            self.release.wait();
        }
        Delivery::Accepted
    }
}

fn blocked_send(reopen_while_blocked: bool) {
    let root = tempfile::tempdir().unwrap();
    let reporter = Arc::new(reporter(root.path()));
    let consent = reporter
        .preferences
        .usage_reporting_consent()
        .unwrap()
        .unwrap();
    let bound = reporter
        .preferences
        .with_usage_reporting_consent(|_, _| {
            reporter
                .store
                .activate_consent_epoch(consent.epoch)
                .unwrap()
        })
        .unwrap();
    let capture = bound
        .prepare_crash_capture(&reporter.app, SystemTime::now())
        .unwrap();
    let context = CrashContext::from_header(&capture.crash_record_header).unwrap();
    bound
        .record_crash_with_context(
            &capture.session_id,
            Some(&context),
            "synthetic fatal error",
            "synthetic.dll+0x1",
        )
        .unwrap();
    let sender = Arc::new(PausedSender {
        entered: Arc::new(Barrier::new(2)),
        release: Arc::new(Barrier::new(2)),
        calls: AtomicUsize::new(0),
    });
    let worker_reporter = Arc::clone(&reporter);
    let worker_sender = Arc::clone(&sender);
    let worker = std::thread::spawn(move || {
        worker_reporter.flush(
            &worker_reporter.store,
            consent,
            worker_sender.as_ref(),
            true,
        )
    });
    sender.entered.wait();
    let (written, observed) = mpsc::channel();
    let writer_reporter = Arc::clone(&reporter);
    let writer = std::thread::spawn(move || {
        set_enabled(&writer_reporter.preferences, false);
        if reopen_while_blocked {
            set_enabled(&writer_reporter.preferences, true);
        }
        written.send(()).unwrap();
    });
    let completed = observed.recv_timeout(Duration::from_secs(2));
    sender.release.wait();
    writer.join().unwrap();
    let report = worker.join().unwrap().unwrap();
    assert!(completed.is_ok(), "撤回不得等待网络释放");
    assert_eq!(sender.calls.load(Ordering::SeqCst), 1);
    assert_eq!(report.sent, 1);
    assert_eq!(report.remaining, 1);
    if !reopen_while_blocked {
        set_enabled(&reporter.preferences, true);
    }
    let reopened = reporter
        .preferences
        .usage_reporting_consent()
        .unwrap()
        .unwrap();
    assert_ne!(reopened.epoch, consent.epoch);
    assert!(reporter
        .flush(&reporter.store, consent, sender.as_ref(), true)
        .is_none());
    assert_eq!(sender.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn in_flight_send_does_not_block_revoke_or_send_next_event() {
    blocked_send(false);
    blocked_send(true);
}

#[derive(Default)]
struct RecordingSender(Mutex<Vec<serde_json::Value>>);

impl TelemetrySender for RecordingSender {
    fn send(&self, body: &[u8]) -> Delivery {
        self.0
            .lock()
            .unwrap()
            .push(serde_json::from_slice(body).unwrap());
        Delivery::Accepted
    }
}

#[test]
fn collecting_fcitx_crashes_does_not_create_an_unused_host_or_an_active_event() {
    let root = tempfile::tempdir().unwrap();
    let reporter = reporter(root.path());
    let consent = reporter
        .preferences
        .usage_reporting_consent()
        .unwrap()
        .unwrap();
    let store = TelemetryStore::new(root.path().join("fcitx5"));
    let sender = RecordingSender::default();
    assert!(reporter.flush(&store, consent, &sender, false).is_none());
    assert!(!store.directory().exists());
    assert!(sender.0.lock().unwrap().is_empty());

    let bound = reporter
        .preferences
        .with_usage_reporting_consent(|_, _| store.activate_consent_epoch(consent.epoch).unwrap())
        .unwrap();
    let original = TelemetryApp::new("linux", "0.1.0").unwrap();
    let previous = SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_856_000);
    let capture = bound.begin_session(&original, previous).unwrap();
    assert_eq!(bound.flush(&sender, previous).unwrap().sent, 1);
    sender.0.lock().unwrap().clear();
    bound
        .record_crash_for(
            &capture.session_id,
            "Synthetic fatal error",
            "synthetic.so+0x123",
        )
        .unwrap();
    assert_eq!(
        reporter
            .flush(&store, consent, &sender, false)
            .unwrap()
            .sent,
        2
    );
    let events = sender.0.lock().unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.iter().any(|event| event["kind"] == "crash"));
    assert!(events.iter().any(|event| event["kind"] == "session_crash"));
    assert!(events.iter().all(|event| event["version"] == "0.1.0"));
    drop(events);
    assert!(!capture.crash_record_path.exists());
    assert_eq!(
        reporter
            .flush(&store, consent, &sender, false)
            .unwrap()
            .sent,
        0
    );
    assert_eq!(sender.0.lock().unwrap().len(), 2);
}

#[test]
fn fcitx_only_settings_share_the_fcitx_install_id() {
    use msime_client_services::telemetry::STATE_FILE;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("msime");
    let fcitx = root.join("fcitx5");
    // 尚无任何宿主时仍落在 IBus 根目录，与原有行为一致。
    assert_eq!(linux_settings_store(root.clone()), root);

    let base = reporter(temp.path());
    let consent = base.preferences.usage_reporting_consent().unwrap().unwrap();
    let fcitx_store = TelemetryStore::new(fcitx.clone());
    let fcitx_install = base
        .preferences
        .with_usage_reporting_consent(|_, _| {
            fcitx_store
                .activate_consent_epoch(consent.epoch)
                .unwrap()
                .install_id()
                .unwrap()
        })
        .unwrap();
    assert!(fcitx.join(STATE_FILE).is_file());
    assert_eq!(linux_settings_store(root.clone()), fcitx);

    let reporter = Reporter {
        store: TelemetryStore::new(linux_settings_store(root.clone())),
        ..base
    };
    let sender = RecordingSender::default();
    assert!(reporter
        .flush(&reporter.store, consent, &sender, true)
        .is_some());
    assert!(
        !root.join(STATE_FILE).exists(),
        "只用 Fcitx5 时设置不得在 IBus 根目录另建安装 id"
    );
    let events = sender.0.lock().unwrap();
    assert!(events.iter().any(|event| event["kind"] == "active"));
    assert!(events
        .iter()
        .all(|event| event["install_id"] == fcitx_install.as_str()));
    drop(events);

    // 两套框架都已初始化时仍以 IBus 根目录为准。
    let ibus = TelemetryStore::new(root.clone());
    reporter
        .preferences
        .with_usage_reporting_consent(|_, _| {
            ibus.activate_consent_epoch(consent.epoch)
                .unwrap()
                .install_id()
                .unwrap()
        })
        .unwrap();
    assert_eq!(linux_settings_store(root.clone()), root);
}
