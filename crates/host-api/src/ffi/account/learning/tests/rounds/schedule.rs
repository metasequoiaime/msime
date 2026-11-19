//! 真实 C 入口、独立账号文件和回环 HTTP 验证产品调度边界。

use super::*;

fn request(harness: &Harness, operation: &str) -> Value {
    let mut value = json!({
        "operation": operation,
        "login": harness.login,
        "options": harness.options,
    });
    match operation {
        "learning_set_sync_enabled" => value["enabled"] = json!(true),
        "learning_sync_scheduled" => {
            value["device_name"] = json!("合成 A");
            value["automatic"] = json!(true);
        }
        _ => {}
    }
    value
}

fn run(harness: &Harness, operation: &str) -> Value {
    call(&harness.source, request(harness, operation))
}

#[test]
fn local_authorization_is_default_off_and_does_not_register_keys_or_refresh_tokens() {
    let harness = Harness::new();
    let mut saved = harness.storage.load().unwrap().unwrap();
    saved.expires_at_unix_ms = 0;
    harness.storage.save(&saved).unwrap();
    let before = serde_json::to_vec(&harness.storage.load().unwrap().unwrap()).unwrap();
    assert_eq!(
        run(&harness, "learning_schedule_status"),
        json!({"ok": true, "value": {
            "enabled": false, "automatic_after_ms": 0,
            "retry_after_ms": 0, "upload_paused": false,
        }})
    );
    assert!(!harness
        ._root
        .path()
        .join("cloud-sync/learning.json")
        .exists());
    assert_eq!(
        run(&harness, "learning_set_sync_enabled")["value"]["enabled"],
        true
    );
    assert_eq!(
        run(&harness, "learning_schedule_status")["value"]["enabled"],
        true
    );
    assert_eq!(
        serde_json::to_vec(&harness.storage.load().unwrap().unwrap()).unwrap(),
        before
    );
    assert!(harness.storage.load_learning_keystore().unwrap().is_none());
    assert!(harness.server.state.lock().unwrap().trace.is_empty());
}

#[test]
fn disabled_sync_skips_network_and_does_not_create_learning_keys() {
    let harness = Harness::new();
    let response = run(&harness, "learning_sync_scheduled");
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(response["value"]["skipped"], "disabled");
    assert!(response["value"]["round"].is_null());
    assert!(harness.storage.load_learning_keystore().unwrap().is_none());
    assert!(harness.server.state.lock().unwrap().trace.is_empty());
}

#[test]
fn local_learning_off_rejects_enabling_resuming_and_sync_but_allows_disabling() {
    let harness = Harness::new();
    assert_eq!(
        run(&harness, "learning_set_sync_enabled")["value"]["enabled"],
        true
    );
    for operation in [
        "learning_set_sync_enabled",
        "learning_resume_upload",
        "learning_sync_scheduled",
    ] {
        let mut value = request(&harness, operation);
        value["options"]["preferences"]["learning"] = json!(false);
        assert_eq!(
            call(&harness.source, value),
            json!({"ok": false, "error": "learning_disabled"})
        );
    }
    let mut value = request(&harness, "learning_set_sync_enabled");
    value["options"]["preferences"]["learning"] = json!(false);
    value["enabled"] = json!(false);
    assert_eq!(call(&harness.source, value)["value"]["enabled"], false);
    assert!(harness.storage.load_learning_keystore().unwrap().is_none());
    assert!(harness.server.state.lock().unwrap().trace.is_empty());
}

#[test]
fn schedule_requests_are_strict_and_read_only_hosts_cannot_authorize_or_run() {
    let harness = Harness::new();
    for operation in [
        "learning_schedule_status",
        "learning_set_sync_enabled",
        "learning_resume_upload",
        "learning_cancel_scheduled",
        "learning_sync_scheduled",
    ] {
        let valid = request(&harness, operation);
        let fields: Vec<_> = valid.as_object().unwrap().keys().cloned().collect();
        let mut cases = Vec::new();
        for field in fields {
            let mut invalid = valid.clone();
            invalid.as_object_mut().unwrap().remove(&field);
            cases.push(invalid);
        }
        for field in ["authorized", "content_id", "state_directory"] {
            let mut invalid = valid.clone();
            invalid[field] = json!(true);
            cases.push(invalid);
        }
        for field in ["automatic", "enabled"] {
            if valid.get(field).is_some() {
                let mut invalid = valid.clone();
                invalid[field] = json!("true");
                cases.push(invalid);
            }
        }
        for invalid in cases {
            assert_eq!(
                call(&harness.source, invalid),
                json!({"ok": false, "error": "account_invalid"})
            );
        }
        let mut readonly = valid;
        readonly["options"]["read_only_state"] = json!(true);
        assert_eq!(
            call(&harness.source, readonly),
            json!({"ok": false, "error": "learning_read_only"})
        );
    }
    assert!(!harness
        ._root
        .path()
        .join("cloud-sync/learning.json")
        .exists());
    assert!(harness.storage.load_learning_keystore().unwrap().is_none());
    assert!(harness.server.state.lock().unwrap().trace.is_empty());
}

#[test]
fn relogin_revokes_authorization_and_old_ui_cannot_reenable_it() {
    let harness = Harness::new();
    assert_eq!(run(&harness, "learning_set_sync_enabled")["ok"], true);
    let mut saved = harness.storage.load().unwrap().unwrap();
    saved.session_id = Some(msime_client_core::uuid::Uuid::new_v4());
    harness.storage.save(&saved).unwrap();
    let file = harness._root.path().join("cloud-sync/learning.json");
    let before = std::fs::read(&file).unwrap();
    for operation in [
        "learning_schedule_status",
        "learning_set_sync_enabled",
        "learning_resume_upload",
        "learning_cancel_scheduled",
        "learning_sync_scheduled",
    ] {
        assert_eq!(
            run(&harness, operation),
            json!({"ok": false, "error": "account_cancelled"})
        );
    }
    let mut status = request(&harness, "learning_schedule_status");
    status["login"] = binding(&harness.source);
    assert_eq!(call(&harness.source, status)["value"]["enabled"], false);
    assert_eq!(std::fs::read(&file).unwrap(), before);
    assert!(harness.storage.load_learning_keystore().unwrap().is_none());
    assert!(harness.server.state.lock().unwrap().trace.is_empty());
}

#[test]
fn automatic_and_manual_use_real_encryption_and_the_same_persisted_hour() {
    let harness = Harness::new();
    harness.remote(3);
    assert_eq!(run(&harness, "learning_set_sync_enabled")["ok"], true);
    harness.server.state.lock().unwrap().trace.clear();
    let first = run(&harness, "learning_sync_scheduled");
    assert_eq!(first["ok"], true, "{first}");
    assert_eq!(first["value"]["round"]["uploaded"], 1);
    assert_eq!(
        first["value"]["round"]["downloads"][0]["outcome"],
        "applied"
    );
    assert_eq!(harness.exported(), state(9, 100));
    let before = harness.server.state.lock().unwrap().trace.clone();
    let skipped = run(&harness, "learning_sync_scheduled");
    assert_eq!(skipped["value"]["skipped"], "throttled");
    assert_eq!(harness.server.state.lock().unwrap().trace, before);
    let mut manual = request(&harness, "learning_sync_scheduled");
    manual["automatic"] = json!(false);
    let result = call(&harness.source, manual);
    assert_eq!(result["value"]["round"]["uploaded"], 2, "{result}");
    let server = harness.server.state.lock().unwrap();
    assert!(!server
        .trace
        .iter()
        .any(|path| path.starts_with("PUT /v1/users/me/learning/devices/")));
    for envelope in &server.uploads {
        let keys = harness.keys();
        let opened = keys.open_blob(&keys.device_id(), envelope).unwrap();
        assert!(msime_engine::read_learning_sync(std::io::Cursor::new(opened.stream)).is_ok());
    }
    assert_eq!(server.uploads.len(), 2);
}

#[test]
fn lifecycle_cancel_with_learning_off_preserves_the_users_independent_switch_and_hour() {
    let harness = Harness::new();
    harness.ready();
    assert_eq!(run(&harness, "learning_set_sync_enabled")["ok"], true);
    let synced = run(&harness, "learning_sync_scheduled");
    assert_eq!(synced["ok"], true, "{synced}");
    let mut value = request(&harness, "learning_cancel_scheduled");
    value["options"]["preferences"]["learning"] = json!(false);
    assert_eq!(
        call(&harness.source, value)["value"],
        synced["value"]["status"]
    );
    let before = harness.server.state.lock().unwrap().trace.clone();
    assert_eq!(
        run(&harness, "learning_sync_scheduled")["value"]["skipped"],
        "throttled"
    );
    assert_eq!(harness.server.state.lock().unwrap().trace, before);
}

#[test]
fn unavailable_dictionary_generation_does_not_prevent_revoking_or_reading_local_consent() {
    let harness = Harness::new();
    assert_eq!(run(&harness, "learning_set_sync_enabled")["ok"], true);
    for operation in [
        "learning_schedule_status",
        "learning_cancel_scheduled",
        "learning_set_sync_enabled",
        "learning_sync_scheduled",
    ] {
        let mut value = request(&harness, operation);
        value["options"]["dictionaries"] = json!(harness
            .paths
            .user_data
            .join("dictionaries/synthetic-unavailable"));
        if operation == "learning_set_sync_enabled" {
            value["enabled"] = json!(false);
        }
        let response = call(&harness.source, value);
        if operation == "learning_sync_scheduled" {
            assert_eq!(
                response,
                json!({"ok": false, "error": "learning_invalid_options"})
            );
        } else {
            assert_eq!(response["ok"], true, "{response}");
            assert_eq!(
                response["value"]["enabled"],
                operation != "learning_set_sync_enabled"
            );
        }
    }
    assert!(harness.storage.load_learning_keystore().unwrap().is_none());
    assert!(harness.server.state.lock().unwrap().trace.is_empty());
}
