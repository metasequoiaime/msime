//! 同步 NAPI 的受限 C 入口必须保留真实清除屏障，且不能转成通用联网入口。

use super::*;
use crate::ffi::account::learning::reset_settings::msime_client_learning_reset_settings;

fn request(value: Value) -> Value {
    let bytes = serde_json::to_vec(&value).unwrap();
    // SAFETY: 请求在调用期间有效，返回字符串读取后按 C ABI 释放。
    unsafe {
        let raw = msime_client_learning_reset_settings(bytes.as_ptr(), bytes.len());
        let result = serde_json::from_str(CStr::from_ptr(raw).to_str().unwrap()).unwrap();
        msime_client_string_free(raw);
        result
    }
}

fn reset(harness: &Harness) -> Value {
    request(json!({"directory":harness._root.path(), "options":harness.options}))
}

#[test]
fn native_settings_reset_is_offline_and_persists_observed_floor_with_expired_login() {
    let harness = Harness::new();
    settings::seed(&harness);
    progress(&harness)
        .observe(account(&harness), FUTURE + 8)
        .unwrap();
    let mut expired = harness.storage.load().unwrap().unwrap();
    expired.expires_at_unix_ms = 0;
    harness.storage.save(&expired).unwrap();
    let saved = std::fs::read(harness._root.path().join("account-session.json")).unwrap();
    assert_eq!(
        reset(&harness),
        json!({"ok":true,"value":{"reset":true,"collections_requeued":0}})
    );
    assert_eq!(harness.exported().records().len(), 0);
    assert_eq!(floor(&harness), (FUTURE + 8) as i64);
    assert_eq!(
        progress(&harness)
            .read(account(&harness))
            .unwrap()
            .reset_floor,
        FUTURE + 8
    );
    assert_eq!(
        std::fs::read(harness._root.path().join("account-session.json")).unwrap(),
        saved
    );
    assert!(harness.storage.load_learning_keystore().unwrap().is_none());
    assert!(harness.server.state.lock().unwrap().trace.is_empty());
}

#[test]
fn native_settings_reset_preserves_data_on_account_barrier_or_busy_failure() {
    for failure in ["account", "barrier", "busy"] {
        let harness = Harness::new();
        let local = settings::seed(&harness);
        let mut lease = None;
        match failure {
            "account" => std::fs::write(
                harness._root.path().join("account-session.json"),
                b"synthetic-corrupt",
            )
            .unwrap(),
            "barrier" => std::fs::write(
                harness._root.path().join("cloud-sync"),
                b"synthetic-blocked",
            )
            .unwrap(),
            _ => {
                lease = msime_client_services::dictionary::access::DictionaryAccess::try_session(
                    &harness.paths.user_data,
                    &harness.paths.dictionaries,
                )
                .unwrap()
            }
        }
        assert_eq!(
            reset(&harness),
            json!({"ok":false,"error":if failure == "busy" {"dictionary_busy"} else {"dictionary_reset_rejected"}})
        );
        assert_eq!(harness.exported(), local);
        assert!(harness.server.state.lock().unwrap().trace.is_empty());
        drop(lease);
    }
}

#[test]
fn native_settings_reset_handles_signed_out_and_partial_collection_success() {
    for signed_out in [true, false] {
        let harness = Harness::new();
        settings::seed(&harness);
        if signed_out {
            harness.storage.clear().unwrap();
        } else {
            std::fs::write(
                harness._root.path().join("DictionaryCollections"),
                b"synthetic-blocked",
            )
            .unwrap();
        }
        let result = reset(&harness);
        assert_eq!(result["ok"], true, "{result}");
        assert_eq!(result["value"]["reset"], true);
        assert_eq!(harness.exported().records().len(), 0);
        if signed_out {
            assert_eq!(result["value"]["collections_requeued"], 0);
            assert!(!harness
                ._root
                .path()
                .join("cloud-sync/learning.json")
                .exists());
        } else {
            assert!(result["value"]["collections_error"].is_string());
            assert_eq!(
                progress(&harness)
                    .read(account(&harness))
                    .unwrap()
                    .reset_floor,
                FUTURE
            );
        }
        assert!(harness.storage.load_learning_keystore().unwrap().is_none());
        assert!(harness.server.state.lock().unwrap().trace.is_empty());
    }
}

#[test]
fn native_settings_reset_rejects_generic_account_actions_and_invalid_directory() {
    let harness = Harness::new();
    let local = settings::seed(&harness);
    for extra in ["action", "operation", "login", "extra", "session_store"] {
        let mut value = json!({"directory":harness._root.path(),"options":harness.options});
        value[extra] = json!({"operation":"learning_sync_once"});
        assert_eq!(
            request(value),
            json!({"ok":false,"error":"account_invalid"})
        );
        assert_eq!(harness.exported(), local);
    }
    assert_eq!(
        request(json!({"directory":"synthetic-relative","options":harness.options}))["ok"],
        false
    );
    assert_eq!(
        request(json!({"directory":harness._root.path()}))["ok"],
        false
    );
    assert_eq!(harness.exported(), local);
    assert!(harness.storage.load_learning_keystore().unwrap().is_none());
    assert!(harness.server.state.lock().unwrap().trace.is_empty());
}
