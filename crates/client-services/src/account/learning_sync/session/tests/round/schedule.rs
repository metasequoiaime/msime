//! 用真实密钥、持久文件和 SQLite 验证调度，而非只核对定时器常量。

use super::*;
use crate::account::learning_sync::state::{SkipReason, StateError};

fn enable(pair: &Pair) {
    pair.a
        .learning_set_sync_enabled(&pair.login, &pair.state, true)
        .unwrap();
}

#[test]
fn default_disabled_does_not_register_or_create_a_key_or_request_data() {
    let pair = Pair::new();
    let directory = pair._root.path().join("fresh");
    let storage = store(&directory);
    let fresh = session(&pair.server, &storage);
    let login = sign_in(&fresh);
    let requests = pair.server.lock().unwrap().requests.len();
    let context = RoundContext {
        state_directory: &directory,
        ..pair.context()
    };
    assert_eq!(
        fresh
            .learning_sync_scheduled(&login, "合成设备", &context, true)
            .unwrap()
            .skipped,
        Some(SkipReason::Disabled)
    );
    assert!(storage.load_learning_keystore().unwrap().is_none());
    assert_eq!(pair.server.lock().unwrap().requests.len(), requests);
    pair.edit_round(|round| assert!(round.trace.is_empty()));
}

#[test]
fn automatic_hour_survives_session_recreation_and_manual_can_sync_without_management_requests() {
    let pair = Pair::new();
    enable(&pair);
    let requests = pair.server.lock().unwrap().requests.len();
    let first = pair
        .a
        .learning_sync_scheduled(&pair.login, "合成 A", &pair.context(), true)
        .unwrap();
    assert!(first.round.unwrap().uploaded.is_some());
    pair.edit_round(|round| round.trace.clear());
    let reopened = session(&pair.server, &pair.storage_a);
    assert_eq!(
        reopened
            .learning_sync_scheduled(&pair.login, "合成 A", &pair.context(), true)
            .unwrap()
            .skipped,
        Some(SkipReason::Throttled)
    );
    pair.edit_round(|round| assert!(round.trace.is_empty()));
    let manual = reopened
        .learning_sync_scheduled(&pair.login, "合成 A", &pair.context(), false)
        .unwrap();
    assert!(manual.round.unwrap().upload_unchanged);
    assert_eq!(pair.server.lock().unwrap().requests.len(), requests);
}

#[test]
fn quota_pause_survives_restart_and_still_downloads_until_explicit_resume() {
    let pair = Pair::new();
    pair.remote(1, &[weight(9, 100)], "synthetic-v1");
    pair.edit_round(|round| {
        round.upload_status = Some(413);
        round.upload_error_body = br#"{"error":{"code":"learning_quota"}}"#.to_vec();
    });
    enable(&pair);
    let first = pair
        .a
        .learning_sync_scheduled(&pair.login, "合成 A", &pair.context(), false)
        .unwrap();
    assert!(first.status.upload_paused);
    let report = first.round.unwrap();
    assert!(matches!(
        report.upload_error,
        Some(RoundError::Http { status: 413, .. })
    ));
    assert_eq!(report.downloads[0].outcome, DownloadOutcome::Applied);
    pair.remote(2, &[weight(12, 200)], "synthetic-v1");
    pair.edit_round(|round| {
        round.upload_status = None;
        round.upload_error_body.clear();
        round.trace.clear();
    });
    let reopened = session(&pair.server, &pair.storage_a);
    let paused = reopened
        .learning_sync_scheduled(&pair.login, "合成 A", &pair.context(), false)
        .unwrap()
        .round
        .unwrap();
    assert!(paused.upload_paused);
    assert!(!paused.upload_unchanged);
    assert_eq!(paused.downloads[0].outcome, DownloadOutcome::Applied);
    assert_eq!(pair.exported(), state(&[weight(12, 200)]));
    pair.edit_round(|round| assert!(!round.trace.iter().any(|entry| entry.starts_with("upload:"))));
    reopened
        .learning_resume_upload(&pair.login, &pair.state)
        .unwrap();
    let resumed = reopened
        .learning_sync_scheduled(&pair.login, "合成 A", &pair.context(), false)
        .unwrap();
    assert!(!resumed.status.upload_paused);
    assert!(resumed.round.unwrap().uploaded.is_some());
}

#[test]
fn envelope_size_failure_does_not_pause_the_account_quota() {
    let pair = Pair::new();
    pair.edit_round(|round| {
        round.upload_status = Some(413);
        round.upload_error_body = br#"{"error":{"code":"learning_too_large"}}"#.to_vec();
    });
    enable(&pair);
    let result = pair
        .a
        .learning_sync_scheduled(&pair.login, "合成 A", &pair.context(), false)
        .unwrap();
    assert!(!result.status.upload_paused);
    assert!(result.round.unwrap().upload_error.is_some());
}

#[test]
fn download_backoff_survives_restart_and_cannot_be_bypassed_by_manual_or_toggle() {
    let pair = Pair::new();
    pair.remote(1, &[weight(9, 100)], "synthetic-v1");
    pair.edit_round(|round| round.download_status = Some(429));
    enable(&pair);
    assert!(matches!(
        pair.a
            .learning_sync_scheduled(&pair.login, "合成 A", &pair.context(), false),
        Err(RoundError::Http {
            status: 429,
            retry_after_seconds: Some(17)
        })
    ));
    pair.a
        .learning_set_sync_enabled(&pair.login, &pair.state, false)
        .unwrap();
    enable(&pair);
    pair.edit_round(|round| round.trace.clear());
    let reopened = session(&pair.server, &pair.storage_a);
    assert_eq!(
        reopened
            .learning_sync_scheduled(&pair.login, "合成 A", &pair.context(), false)
            .unwrap()
            .skipped,
        Some(SkipReason::Backoff)
    );
    pair.edit_round(|round| assert!(round.trace.is_empty()));
    assert_eq!(pair.progress(), RemoteProgress::default());
}

#[test]
fn disable_then_reenable_during_response_cancels_the_old_round_and_retains_response_limits() {
    for (status, kind) in [(429, "download:"), (413, "upload:")] {
        let pair = Pair::new();
        pair.remote(1, &[weight(9, 100)], "synthetic-v1");
        pair.edit_round(|round| {
            if status == 429 {
                round.download_status = Some(status);
            } else {
                round.upload_status = Some(status);
                round.upload_error_body = br#"{"error":{"code":"learning_quota"}}"#.to_vec();
            }
        });
        enable(&pair);
        let server = pair.server.clone();
        let storage = pair.storage_a.clone();
        let login = pair.login.clone();
        let directory = pair.state.clone();
        let running = pair.after_blob(kind, move || {
            let ui = session(&server, &storage);
            ui.learning_set_sync_enabled(&login, &directory, false)
                .unwrap();
            ui.learning_set_sync_enabled(&login, &directory, true)
                .unwrap();
        });
        assert!(matches!(
            running.learning_sync_scheduled(&pair.login, "合成 A", &pair.context(), false),
            Err(RoundError::Account(AccountError::Cancelled))
        ));
        let state = pair
            .a
            .learning_schedule_status(&pair.login, &pair.state)
            .unwrap();
        assert!(state.enabled);
        assert_eq!(state.upload_paused, status == 413);
        if status == 429 {
            assert!(state.retry_after_ms > 0);
        }
        assert_eq!(pair.progress(), RemoteProgress::default());
    }
}

#[test]
fn missing_server_approval_is_not_repaired_by_the_automatic_driver() {
    let pair = Pair::new();
    let own = pair.keys(&pair.storage_a).device_id();
    pair.server.lock().unwrap().wraps.remove(&own);
    let requests = pair.server.lock().unwrap().requests.len();
    enable(&pair);
    assert!(matches!(
        pair.a
            .learning_sync_scheduled(&pair.login, "合成 A", &pair.context(), true),
        Err(RoundError::NotReady)
    ));
    assert_eq!(pair.server.lock().unwrap().requests.len(), requests);
    pair.edit_round(|round| assert_eq!(round.trace, vec!["metadata"]));
}

#[test]
fn relogin_during_response_cancels_old_data_but_preserves_account_limits() {
    for (status, kind) in [(429, "download:"), (413, "upload:")] {
        let pair = Pair::new();
        pair.remote(1, &[weight(9, 100)], "synthetic-v1");
        pair.edit_round(|round| {
            if status == 429 {
                round.download_status = Some(status);
            } else {
                round.upload_status = Some(status);
                round.upload_error_body = br#"{"error":{"code":"learning_quota"}}"#.to_vec();
            }
        });
        enable(&pair);
        let server = pair.server.clone();
        let storage = pair.storage_a.clone();
        let running = pair.after_blob(kind, move || {
            sign_in(&session(&server, &storage));
        });
        assert!(matches!(
            running.learning_sync_scheduled(&pair.login, "合成 A", &pair.context(), false),
            Err(RoundError::Account(AccountError::Cancelled))
        ));
        let login = pair.a.login_binding().unwrap().unwrap();
        assert_ne!(login, pair.login);
        let state = pair
            .a
            .learning_schedule_status(&login, &pair.state)
            .unwrap();
        assert!(!state.enabled);
        assert_eq!(state.upload_paused, status == 413);
        if status == 429 {
            assert!(state.retry_after_ms > 0);
        }
        assert_eq!(pair.progress(), RemoteProgress::default());
    }
}

#[test]
fn stale_login_and_relative_paths_cannot_mutate_consent_or_start_a_round() {
    let pair = Pair::new();
    enable(&pair);
    let before = std::fs::read(pair.state.join("cloud-sync/learning.json")).unwrap();
    let new_login = sign_in(&pair.a);
    for result in [
        pair.a.learning_schedule_status(&pair.login, &pair.state),
        pair.a
            .learning_set_sync_enabled(&pair.login, &pair.state, false),
        pair.a.learning_resume_upload(&pair.login, &pair.state),
    ] {
        assert!(matches!(
            result,
            Err(RoundError::Account(AccountError::Cancelled))
        ));
    }
    let relative = std::path::Path::new("synthetic-relative-state");
    assert!(matches!(
        pair.a.learning_set_sync_enabled(&new_login, relative, true),
        Err(RoundError::State(StateError::Invalid))
    ));
    let context = RoundContext {
        state_directory: relative,
        ..pair.context()
    };
    assert!(matches!(
        pair.a
            .learning_sync_scheduled(&new_login, "合成 A", &context, true),
        Err(RoundError::State(StateError::Invalid))
    ));
    assert_eq!(
        std::fs::read(pair.state.join("cloud-sync/learning.json")).unwrap(),
        before
    );
    pair.edit_round(|round| assert!(round.trace.is_empty()));
}

#[test]
fn lifecycle_stop_cancels_inflight_merge_without_disabling_the_next_host_session() {
    let pair = Pair::new();
    pair.remote(1, &[weight(9, 100)], "synthetic-v1");
    enable(&pair);
    let server = pair.server.clone();
    let storage = pair.storage_a.clone();
    let login = pair.login.clone();
    let directory = pair.state.clone();
    let running = pair.after_blob("download:", move || {
        assert!(
            session(&server, &storage)
                .learning_cancel_scheduled(&login, &directory)
                .unwrap()
                .enabled
        );
    });
    assert!(matches!(
        running.learning_sync_scheduled(&pair.login, "合成 A", &pair.context(), true),
        Err(RoundError::Account(AccountError::Cancelled))
    ));
    assert_eq!(pair.progress(), RemoteProgress::default());
    assert_eq!(pair.exported().records().len(), 0);
    let reopened = session(&pair.server, &pair.storage_a);
    assert!(
        reopened
            .learning_schedule_status(&pair.login, &pair.state)
            .unwrap()
            .enabled
    );
    let resumed = reopened
        .learning_sync_scheduled(&pair.login, "合成 A", &pair.context(), false)
        .unwrap();
    assert_eq!(
        resumed.round.unwrap().downloads[0].outcome,
        DownloadOutcome::Applied
    );
    assert_eq!(pair.exported(), state(&[weight(9, 100)]));
}

#[test]
fn cancelling_during_initial_refresh_or_upload_401_refresh_prevents_the_next_data_request() {
    for expired in [false, true] {
        let pair = Pair::new();
        enable(&pair);
        if expired {
            let mut saved = pair.storage_a.load().unwrap().unwrap();
            saved.expires_at_unix_ms = 0;
            pair.storage_a.save(&saved).unwrap();
        }
        let directory = pair.state.clone();
        let login = pair.login.clone();
        pair.server.lock().unwrap().after_refresh = Some(Arc::new(move || {
            StateStore::new(&directory).cancel_schedule(&login).unwrap();
        }));
        let server = pair.server.clone();
        let running = pair.after_blob("metadata", move || {
            server.lock().unwrap().reject = Some("a".repeat(64));
        });
        assert!(matches!(
            running.learning_sync_scheduled(&pair.login, "合成 A", &pair.context(), true),
            Err(RoundError::Account(AccountError::Cancelled))
        ));
        let server = pair.server.lock().unwrap();
        assert_eq!(server.refreshes, 1);
        assert_eq!(server.blob_tokens.len(), if expired { 0 } else { 2 });
        assert!(!server
            .blob_tokens
            .iter()
            .any(|token| token == &"c".repeat(64)));
        let round = server.round.as_ref().unwrap();
        assert_eq!(round.trace, if expired { vec![] } else { vec!["metadata"] });
        assert!(round.blobs.is_empty());
    }
}
