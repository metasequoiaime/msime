use super::*;

fn login(session: u128) -> AccountLoginBinding {
    AccountLoginBinding {
        user_id: "synthetic-account".into(),
        session_id: uuid::Uuid::from_u128(session),
    }
}

#[test]
fn missing_schedule_is_disabled_and_old_progress_is_preserved() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    store.observe("synthetic-account", 103).unwrap();
    let file = store.directory.join(STATE_FILE);
    let before = std::fs::read(&file).unwrap();
    assert!(!store.schedule_status(&login(1)).unwrap().enabled);
    assert_eq!(std::fs::read(&file).unwrap(), before);
    assert_eq!(
        store.reserve_schedule(&login(1), 1000, true).unwrap(),
        Err(SkipReason::Disabled)
    );
    assert_eq!(store.read("synthetic-account").unwrap().observed_max, 103);
}

#[test]
fn restart_and_clock_rollback_do_not_reset_the_hour_and_manual_bypasses_only_that_hour() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    store.set_schedule_enabled(&login(1), true).unwrap();
    assert!(store
        .reserve_schedule(&login(1), 1000, true)
        .unwrap()
        .is_ok());
    let store = StateStore::new(root.path());
    assert_eq!(
        store.reserve_schedule(&login(1), 500, true).unwrap(),
        Err(SkipReason::Throttled)
    );
    assert!(store
        .reserve_schedule(&login(1), 500, false)
        .unwrap()
        .is_ok());
    assert_eq!(
        store
            .reserve_schedule(&login(1), 999 + AUTOMATIC_INTERVAL_MS, true)
            .unwrap(),
        Err(SkipReason::Throttled)
    );
    assert!(store
        .reserve_schedule(&login(1), 1001, false)
        .unwrap()
        .is_ok());
    assert_eq!(
        store
            .reserve_schedule(&login(1), 1000 + AUTOMATIC_INTERVAL_MS, true)
            .unwrap(),
        Err(SkipReason::Throttled)
    );
    assert!(store
        .reserve_schedule(&login(1), 1001 + AUTOMATIC_INTERVAL_MS, true)
        .unwrap()
        .is_ok());
}

#[test]
fn relogin_and_toggle_preserve_backoff_and_quota_and_revoke_old_consent() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    store.set_schedule_enabled(&login(1), true).unwrap();
    let (revision, _) = store
        .reserve_schedule(&login(1), 1000, true)
        .unwrap()
        .unwrap();
    store
        .restrict_schedule(&login(1), Some(9000), true)
        .unwrap();
    store.set_schedule_enabled(&login(1), false).unwrap();
    store.set_schedule_enabled(&login(1), true).unwrap();
    assert!(!store.schedule_authorized(&login(1), revision).unwrap());
    assert_eq!(
        store.reserve_schedule(&login(1), 2000, false).unwrap(),
        Err(SkipReason::Backoff)
    );
    assert!(!store.schedule_status(&login(2)).unwrap().enabled);
    store.set_schedule_enabled(&login(2), true).unwrap();
    let status = store.schedule_status(&login(2)).unwrap();
    assert_eq!(status.retry_after_ms, 9000);
    assert!(status.upload_paused);
    let resumed = store.resume_schedule_upload(&login(2)).unwrap();
    assert!(!resumed.upload_paused);
    assert_eq!(resumed.retry_after_ms, 9000);
    assert_eq!(resumed.automatic_after_ms, status.automatic_after_ms);
    assert_eq!(
        store.reserve_schedule(&login(2), 2000, false).unwrap(),
        Err(SkipReason::Backoff)
    );
}

#[test]
fn concurrent_process_handles_reserve_only_one_automatic_round() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    store.set_schedule_enabled(&login(1), true).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let mut threads = Vec::new();
    for _ in 0..8 {
        let store = store.clone();
        let barrier = barrier.clone();
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            store
                .reserve_schedule(&login(1), 1000, true)
                .unwrap()
                .is_ok()
        }));
    }
    assert_eq!(
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .filter(|success| *success)
            .count(),
        1
    );
}

#[test]
fn corrupted_schedule_is_preserved_and_cannot_be_enabled() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    store.set_schedule_enabled(&login(1), true).unwrap();
    let file = store.directory.join(STATE_FILE);
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    value["accounts"]["synthetic-account"]["schedule"]["enabled"] = serde_json::json!("true");
    let corrupt = serde_json::to_vec(&value).unwrap();
    std::fs::write(&file, &corrupt).unwrap();
    assert_eq!(
        store.set_schedule_enabled(&login(1), true),
        Err(StateError::Invalid)
    );
    assert_eq!(std::fs::read(&file).unwrap(), corrupt);
}

#[test]
fn retry_deadline_only_increases_and_saturated_time_cannot_wrap_into_an_allowed_round() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    store.set_schedule_enabled(&login(1), true).unwrap();
    store
        .restrict_schedule(&login(1), Some(u64::MAX), false)
        .unwrap();
    store.restrict_schedule(&login(1), Some(1), false).unwrap();
    assert_eq!(
        store
            .reserve_schedule(&login(1), u64::MAX - 1, false)
            .unwrap(),
        Err(SkipReason::Backoff)
    );
    assert_eq!(
        store.reserve_schedule(&login(1), u64::MAX, false).unwrap(),
        Err(SkipReason::Backoff)
    );
    let other = AccountLoginBinding {
        user_id: "synthetic-other".into(),
        ..login(1)
    };
    store.set_schedule_enabled(&other, true).unwrap();
    assert!(store
        .reserve_schedule(&other, u64::MAX, true)
        .unwrap()
        .is_ok());
    assert_eq!(
        store.schedule_status(&other).unwrap().automatic_after_ms,
        u64::MAX
    );
    assert_eq!(
        store.reserve_schedule(&other, u64::MAX, true).unwrap(),
        Err(SkipReason::Throttled)
    );
}

#[test]
fn lifecycle_cancellation_preserves_authorization_and_limits_but_invalidates_the_old_lease() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    assert!(!store.cancel_schedule(&login(1)).unwrap().enabled);
    assert!(!store.directory.join(STATE_FILE).exists());
    store.set_schedule_enabled(&login(1), true).unwrap();
    let (revision, _) = store
        .reserve_schedule(&login(1), 1000, true)
        .unwrap()
        .unwrap();
    store
        .restrict_schedule(&login(1), Some(9000), true)
        .unwrap();
    let before = store.schedule_status(&login(1)).unwrap();
    assert_eq!(store.cancel_schedule(&login(1)).unwrap(), before);
    assert!(!store.schedule_authorized(&login(1), revision).unwrap());
    let reopened = StateStore::new(root.path());
    assert_eq!(reopened.schedule_status(&login(1)).unwrap(), before);
    assert_eq!(
        reopened.reserve_schedule(&login(1), 2000, false).unwrap(),
        Err(SkipReason::Backoff)
    );
    assert_eq!(
        reopened.reserve_schedule(&login(1), 10000, true).unwrap(),
        Err(SkipReason::Throttled)
    );
}
