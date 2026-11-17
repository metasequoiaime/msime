//! 原生设置经账号 C 入口清除，实际数据与屏障必须一起验证。

use super::*;

fn reset(harness: &Harness) -> Value {
    call(
        &harness.source,
        json!({"operation":"learning_reset_settings", "options":harness.options}),
    )
}

pub(super) fn seed(harness: &Harness) -> LearningSyncState {
    let local = state(4, FUTURE);
    msime_engine::merge_learning_sync_state(&harness.paths, false, &local, 0).unwrap();
    local
}

#[test]
fn settings_c_reset_covers_observed_time_without_network_or_keys() {
    let harness = Harness::new();
    seed(&harness);
    progress(&harness)
        .observe(account(&harness), FUTURE + 8)
        .unwrap();
    let mut expired = harness.storage.load().unwrap().unwrap();
    expired.expires_at_unix_ms = 0;
    harness.storage.save(&expired).unwrap();
    assert_eq!(
        reset(&harness),
        json!({"ok":true,"value":{"reset":true,"collections_requeued":0}})
    );
    assert_eq!(harness.exported().records().len(), 0);
    assert_eq!(
        progress(&harness)
            .read(account(&harness))
            .unwrap()
            .reset_floor,
        FUTURE + 8
    );
    assert_eq!(floor(&harness), (FUTURE + 8) as i64);
    assert!(harness.storage.load_learning_keystore().unwrap().is_none());
    assert!(harness.server.state.lock().unwrap().trace.is_empty());
}

#[test]
fn settings_c_reset_only_falls_back_when_explicitly_signed_out() {
    let harness = Harness::new();
    seed(&harness);
    harness.storage.clear().unwrap();
    assert_eq!(
        reset(&harness),
        json!({"ok":true,"value":{"reset":true,"collections_requeued":0}})
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
fn settings_c_reset_rejects_corrupt_account_and_barrier_without_clearing() {
    for corrupt_account in [true, false] {
        let harness = Harness::new();
        let local = seed(&harness);
        if corrupt_account {
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
        assert_eq!(
            reset(&harness),
            json!({"ok":false,"error":"dictionary_reset_rejected"})
        );
        assert_eq!(harness.exported(), local);
        assert!(harness.server.state.lock().unwrap().trace.is_empty());
    }
}

#[test]
fn settings_c_reset_keeps_success_and_barrier_when_collection_requeue_fails() {
    let harness = Harness::new();
    seed(&harness);
    std::fs::write(
        harness._root.path().join("DictionaryCollections"),
        b"synthetic-blocked",
    )
    .unwrap();
    let result = reset(&harness);
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["value"]["reset"], true);
    assert!(result["value"]["collections_requeued"].is_null());
    assert!(result["value"]["collections_error"].is_string());
    assert_eq!(harness.exported().records().len(), 0);
    assert_eq!(
        progress(&harness)
            .read(account(&harness))
            .unwrap()
            .reset_floor,
        FUTURE
    );
    assert!(harness.server.state.lock().unwrap().trace.is_empty());
}

#[test]
fn settings_c_reset_rejects_unknown_fields_and_busy_keeps_original_records() {
    let harness = Harness::new();
    let local = seed(&harness);
    for extra in ["login", "action", "extra"] {
        let mut request = json!({"operation":"learning_reset_settings","options":harness.options});
        request[extra] = json!(true);
        assert_eq!(
            call(&harness.source, request),
            json!({"ok":false,"error":"account_invalid"})
        );
        assert_eq!(harness.exported(), local);
    }
    let _lease = msime_client_services::dictionary::access::DictionaryAccess::try_session(
        &harness.paths.user_data,
        &harness.paths.dictionaries,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        reset(&harness),
        json!({"ok":false,"error":"dictionary_busy"})
    );
    assert_eq!(harness.exported(), local);
    assert!(!harness
        .paths
        .user_data
        .join(msime_client_services::dictionary::quiesce::LEASE_NAME)
        .exists());
}
