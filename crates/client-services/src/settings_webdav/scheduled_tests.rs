//! 合成网盘与可控时钟执行真实调度、加密和事务路径。
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

struct TestClock(AtomicU64);
impl TestClock {
    fn new() -> Arc<Self> {
        Arc::new(Self(AtomicU64::new(1_000_000)))
    }
    fn advance(&self, elapsed: u64) {
        self.0.fetch_add(elapsed, Ordering::SeqCst);
    }
}
impl Environment for TestClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
    fn sleep(&self, duration: Duration) {
        self.advance(duration.as_millis() as u64);
    }
}

fn scheduled_service(
    root: &std::path::Path,
    drive: &Arc<Drive>,
    clock: &Arc<TestClock>,
) -> SettingsWebDav {
    let mut service = service(root, drive, "laptop");
    service.clock = clock.clone();
    service
}

#[test]
fn changes_debounce_across_restarts_and_local_only_fields_do_not_upload() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let clock = TestClock::new();
    let service = scheduled_service(root.path(), &drive, &clock);
    let settings = preferences(root.path(), "laptop");
    let first = service
        .run_scheduled(&settings, SettingsSyncTrigger::Manual)
        .unwrap();
    assert!(first.report.unwrap().unwrap().uploaded);
    assert!(!first.status.pending_changes);
    let requests = drive.0.lock().unwrap().requests.len();

    edit(&settings, |value| value.candidate_page_size = 9);
    let marked = service
        .run_scheduled(&settings, SettingsSyncTrigger::LocalChange)
        .unwrap();
    assert!(!marked.network && marked.status.pending_changes);
    assert_eq!(marked.next_check_ms, 60_000);
    assert_eq!(drive.0.lock().unwrap().requests.len(), requests);
    let restarted = scheduled_service(root.path(), &drive, &clock);
    clock.advance(59_999);
    assert!(
        !restarted
            .run_scheduled(&settings, SettingsSyncTrigger::Periodic)
            .unwrap()
            .network
    );
    clock.advance(1);
    let sent = restarted
        .run_scheduled(&settings, SettingsSyncTrigger::Periodic)
        .unwrap();
    assert!(sent.report.unwrap().unwrap().uploaded);
    assert!(!sent.status.pending_changes);

    let requests = drive.0.lock().unwrap().requests.len();
    edit(&settings, |value| {
        value.voice_input.asr_token = "synthetic-private-token".into()
    });
    let local = restarted
        .run_scheduled(&settings, SettingsSyncTrigger::LocalChange)
        .unwrap();
    assert!(!local.network && !local.status.pending_changes);
    assert_eq!(drive.0.lock().unwrap().requests.len(), requests);
}

#[test]
fn network_failures_persist_backoff_but_manual_retry_keeps_its_budget() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let clock = TestClock::new();
    let service = scheduled_service(root.path(), &drive, &clock);
    let settings = preferences(root.path(), "laptop");
    service
        .run_scheduled(&settings, SettingsSyncTrigger::Manual)
        .unwrap()
        .report
        .unwrap()
        .unwrap();
    drive.0.lock().unwrap().unauthorized = true;
    clock.advance(60_000);
    let failed = service
        .run_scheduled(&settings, SettingsSyncTrigger::Opened)
        .unwrap();
    assert!(failed.network);
    assert_eq!(failed.report.unwrap().unwrap_err(), "relay_unauthorized");
    assert_eq!(
        failed.status.last_error.as_deref(),
        Some("relay_unauthorized")
    );
    let requests = drive.0.lock().unwrap().requests.len();
    edit(&settings, |value| value.candidate_page_size = 9);
    clock.advance(60_000);
    let restarted = scheduled_service(root.path(), &drive, &clock);
    assert!(
        !restarted
            .run_scheduled(&settings, SettingsSyncTrigger::LocalChange)
            .unwrap()
            .network
    );
    assert_eq!(drive.0.lock().unwrap().requests.len(), requests);
    assert!(
        restarted
            .run_scheduled(&settings, SettingsSyncTrigger::Manual)
            .unwrap()
            .network
    );
    assert!(drive.0.lock().unwrap().requests.len() > requests);
}

#[test]
fn failed_requests_exhaust_the_persisted_budget_and_a_new_config_cannot_refill_it() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    drive.0.lock().unwrap().unauthorized = true;
    let clock = TestClock::new();
    let service = scheduled_service(root.path(), &drive, &clock);
    let settings = preferences(root.path(), "laptop");
    for _ in 0..65 {
        service
            .run_scheduled(&settings, SettingsSyncTrigger::Manual)
            .unwrap();
    }
    assert_eq!(drive.0.lock().unwrap().requests.len(), 60);
    let mut restarted = scheduled_service(root.path(), &drive, &clock);
    restarted.config = RelayConfig::webdav(
        "https://other.example.test/dav",
        "other-synthetic-user",
        "synthetic-password",
    )
    .unwrap();
    let limited = restarted
        .run_scheduled(&settings, SettingsSyncTrigger::Manual)
        .unwrap();
    assert!(!limited.network);
    assert!(limited.status.budget_wait_until.is_some());
    assert_eq!(drive.0.lock().unwrap().requests.len(), 60);
    clock.advance(30 * 60 * 1000);
    assert!(
        restarted
            .run_scheduled(&settings, SettingsSyncTrigger::Manual)
            .unwrap()
            .network
    );
    assert_eq!(drive.0.lock().unwrap().requests.len(), 61);
}

#[test]
fn every_request_is_privately_charged_before_the_transport_sees_it() {
    struct Observed {
        drive: Arc<Drive>,
        file: PathBuf,
    }
    impl DavHttp for Observed {
        fn send(&self, request: &DavRequest<'_>) -> Result<DavResponse, RelayError> {
            let bytes = std::fs::read(&self.file).unwrap();
            let state: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            let charged = state["schedule"]["requests"].as_array().unwrap().len();
            assert_eq!(charged, self.drive.0.lock().unwrap().requests.len() + 1);
            assert!(!String::from_utf8_lossy(&bytes).contains("synthetic-password"));
            self.drive.send(request)
        }
    }
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let clock = TestClock::new();
    let mut service = scheduled_service(root.path(), &drive, &clock);
    service.http = Arc::new(Observed {
        drive: drive.clone(),
        file: service.directory.join("settings-sync-schedule"),
    });
    let result = service
        .run_scheduled(
            &preferences(root.path(), "laptop"),
            SettingsSyncTrigger::Manual,
        )
        .unwrap();
    assert!(result.report.unwrap().is_ok());
    assert!(!drive.0.lock().unwrap().requests.is_empty());
}

#[test]
fn a_new_edit_during_upload_remains_pending_instead_of_becoming_the_acknowledged_baseline() {
    let root = tempfile::tempdir().unwrap();
    let drive = Arc::new(Drive::default());
    let clock = TestClock::new();
    let service = scheduled_service(root.path(), &drive, &clock);
    let settings = preferences(root.path(), "laptop");
    let concurrent = preferences(root.path(), "laptop");
    drive.0.lock().unwrap().append_hook = Some(Box::new(move || {
        edit(&concurrent, |value| value.candidate_page_size = 9);
    }));
    let result = service
        .run_scheduled(&settings, SettingsSyncTrigger::Manual)
        .unwrap();
    assert!(result.report.unwrap().is_ok());
    assert!(result.status.pending_changes);
    assert_eq!(settings.load().unwrap().preferences.candidate_page_size, 9);
    assert_eq!(result.next_check_ms, 60_000);
}
