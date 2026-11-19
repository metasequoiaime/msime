//! 桌面实际入口复用真实账号、SQLite 与维护租约，失败不能走旧清除兜底。

use super::*;
use crate::settings_dictionary::request_with_account;
use msime_client_services::account::{BackendAccountClient, BackendAccountSession};

type DesktopSession = BackendAccountSession<BackendAccountClient, FileAccountSessionStorage>;

fn session(harness: &Harness) -> DesktopSession {
    BackendAccountSession::new(
        BackendAccountClient::loopback_with_agent(
            &TEST_ORIGIN.with_borrow(Clone::clone).unwrap(),
            "MSIME/Tests",
        )
        .unwrap(),
        FileAccountSessionStorage::new(harness._root.path(), AccountSessionFileLayout::Native),
    )
}

fn reset(harness: &Harness) -> Result<Value, &'static str> {
    request_with_account(
        &session(harness),
        &harness.options,
        &json!({"operation":"reset"}),
        || {},
    )
}

fn seed(harness: &Harness) -> LearningSyncState {
    let local = state(4, FUTURE);
    msime_engine::merge_learning_sync_state(&harness.paths, false, &local, 0).unwrap();
    local
}

#[test]
fn desktop_reset_binds_expired_login_without_network_or_keys() {
    let harness = Harness::new();
    seed(&harness);
    progress(&harness)
        .observe(account(&harness), FUTURE + 3)
        .unwrap();
    let mut saved = harness.storage.load().unwrap().unwrap();
    saved.expires_at_unix_ms = 0;
    harness.storage.save(&saved).unwrap();
    assert_eq!(
        reset(&harness).unwrap(),
        json!({"reset":true,"collections_requeued":0})
    );
    assert_eq!(harness.exported().records().len(), 0);
    assert_eq!(floor(&harness), (FUTURE + 3) as i64);
    assert_eq!(
        progress(&harness)
            .read(account(&harness))
            .unwrap()
            .reset_floor,
        FUTURE + 3
    );
    assert!(harness.storage.load_learning_keystore().unwrap().is_none());
    assert!(harness.server.state.lock().unwrap().trace.is_empty());
}

#[test]
fn desktop_signed_out_reset_keeps_legacy_shape_without_account_barrier() {
    let harness = Harness::new();
    seed(&harness);
    harness.storage.clear().unwrap();
    assert_eq!(
        reset(&harness).unwrap(),
        json!({"reset":true,"collections_requeued":0})
    );
    assert_eq!(harness.exported().records().len(), 0);
    assert!(!harness
        ._root
        .path()
        .join("cloud-sync/learning.json")
        .exists());
    assert!(harness.server.state.lock().unwrap().trace.is_empty());
}

#[test]
fn desktop_account_or_barrier_failure_keeps_learning_and_does_not_fallback() {
    for account_corrupt in [false, true] {
        let harness = Harness::new();
        let local = seed(&harness);
        if account_corrupt {
            std::fs::write(
                harness._root.path().join("account-session.json"),
                b"synthetic-corrupt",
            )
            .unwrap();
        } else {
            std::fs::write(
                harness._root.path().join("cloud-sync"),
                b"synthetic-blocked",
            )
            .unwrap();
        }
        assert_eq!(reset(&harness), Err("dictionary_reset_rejected"));
        assert_eq!(harness.exported(), local);
        assert!(harness.server.state.lock().unwrap().trace.is_empty());
    }
}

#[test]
fn desktop_busy_reset_announces_release_and_retains_data() {
    let harness = Harness::new();
    let local = seed(&harness);
    let lease = msime_client_services::dictionary::access::DictionaryAccess::try_session(
        &harness.paths.user_data,
        &harness.paths.dictionaries,
    )
    .unwrap()
    .unwrap();
    let mut announced = 0;
    assert_eq!(
        request_with_account(
            &session(&harness),
            &harness.options,
            &json!({"operation":"reset"}),
            || announced += 1
        ),
        Err("dictionary_busy")
    );
    assert_eq!(announced, 1);
    drop(lease);
    assert_eq!(harness.exported(), local);
    assert!(!harness
        ._root
        .path()
        .join("cloud-sync/learning.json")
        .exists());
    assert!(!harness
        .paths
        .user_data
        .join(msime_client_services::dictionary::quiesce::LEASE_NAME)
        .exists());
}

#[test]
fn desktop_invalid_context_or_action_never_clears_learning() {
    let harness = Harness::new();
    let local = seed(&harness);
    let mut read_only = harness.options.clone();
    read_only["read_only_state"] = json!(true);
    let mut wrong_generation = harness.options.clone();
    wrong_generation["dictionaries"] =
        json!(harness.paths.user_data.join("dictionaries/synthetic-wrong"));
    for options in [read_only, wrong_generation] {
        assert_eq!(
            request_with_account(
                &session(&harness),
                &options,
                &json!({"operation":"reset"}),
                || {}
            ),
            Err("dictionary_reset_rejected")
        );
        assert_eq!(harness.exported(), local);
    }
    assert_eq!(
        request_with_account(
            &session(&harness),
            &harness.options,
            &json!({"operation":"reset","extra":true}),
            || {}
        ),
        Err("dictionary_reset_rejected")
    );
    assert_eq!(harness.exported(), local);
}

#[test]
fn desktop_non_reset_does_not_load_account_or_change_dictionary_behavior() {
    let harness = Harness::new();
    std::fs::write(
        harness._root.path().join("account-session.json"),
        b"synthetic-corrupt",
    )
    .unwrap();
    let action = json!({"operation":"list","kind":"english","offset":0,"limit":10});
    let result = request_with_account(&session(&harness), &harness.options, &action, || {});
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        result,
        crate::settings_dictionary::request(&harness.options, &action, || {})
    );
}

#[test]
fn desktop_login_change_during_release_never_clears_under_old_binding() {
    let harness = Harness::new();
    let local = seed(&harness);
    let mut lease = msime_client_services::dictionary::access::DictionaryAccess::try_session(
        &harness.paths.user_data,
        &harness.paths.dictionaries,
    )
    .unwrap();
    let mut announced = 0;
    let result = request_with_account(
        &session(&harness),
        &harness.options,
        &json!({"operation":"reset"}),
        || {
            announced += 1;
            let mut saved = harness.storage.load().unwrap().unwrap();
            saved.session_id = Some(msime_client_core::uuid::Uuid::new_v4());
            harness.storage.save(&saved).unwrap();
            lease.take();
        },
    );
    assert_eq!(result, Err("dictionary_reset_rejected"));
    assert_eq!(announced, 1);
    assert_eq!(harness.exported(), local);
    assert!(!harness
        ._root
        .path()
        .join("cloud-sync/learning.json")
        .exists());
}
