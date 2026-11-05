//! 本地语言选路和旧代次拒收通过真实会话 ABI 验证。

use super::*;

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
#[test]
fn local_translation_selection_never_exposes_cloud_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let mut preferences = Preferences {
        candidate_translations: true,
        translation_account: true,
        ..Preferences::default()
    };
    // 故意保留旧服务的选择，确认本地选择优先且不携带凭据。
    preferences.tencent_tmt.secret_id = "synthetic-id".into();
    preferences.tencent_tmt.secret_key = "synthetic-key".into();
    preferences.local_translation.enabled = true;
    preferences.local_translation.target_language = "yue".into();
    let handle = test_host_preferences(dir.path(), preferences.clone());
    read(msime_client_focus(handle, true));
    for byte in b"U4e2d" {
        read(msime_client_character(
            handle,
            *byte,
            byte.is_ascii_uppercase(),
        ));
    }
    let query = read(msime_client_translation_query(handle))["value"].clone();
    assert_eq!(query["provider"], "local");
    assert_eq!(query["target_language"], "yue");
    assert_eq!(query["target_languages"], json!(["yue"]));
    assert_eq!(query["translation_account"], false);
    for field in [
        "tencent_tmt",
        "niutrans",
        "custom_translation",
        "ai_assistant",
        "user_data",
    ] {
        assert!(query[field].is_null(), "{field}");
    }
    let generation = query["generation"].as_u64().unwrap();
    let payload = serde_json::to_vec(
        &json!([{"text": query["candidates"][0]["text"], "translation": "合成译文"}]),
    )
    .unwrap();
    preferences.local_translation.target_language = "ja".into();
    assert_eq!(update(handle, 1, &preferences)["ok"], true);
    let result = read(unsafe {
        msime_client_apply_translations(handle, generation, payload.as_ptr(), payload.len())
    });
    assert_eq!(result["value"]["applied"], false);
    let next = read(msime_client_translation_query(handle))["value"].clone();
    assert_eq!(next["target_language"], "ja");
    assert!(next["generation"].as_u64().unwrap() > generation);
    read(msime_client_destroy(handle));
}

#[test]
fn local_translation_settings_response_contains_no_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let call = |update: Value| {
        let request =
            serde_json::to_vec(&json!({"directory": dir.path(), "update": update})).unwrap();
        read(unsafe { msime_client_local_translation_settings(request.as_ptr(), request.len()) })
    };
    let initial = call(Value::Null);
    assert_eq!(initial["ok"], true);
    let revision = initial["value"]["revision"].as_u64().unwrap();
    let update = json!({"expected_revision": revision, "enabled": true, "target_language": "bo"});
    let saved = call(update.clone());
    assert_eq!(saved["ok"], true);
    assert_eq!(saved["value"].as_object().unwrap().len(), 2);
    assert_eq!(saved["value"]["local_translation"]["target_language"], "bo");
    assert_eq!(call(update)["ok"], false);
}

#[cfg(unix)]
#[test]
fn local_translation_cannot_escape_through_provider_socket() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(
        dir.path(),
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
    let path = dir.path().join("provider.sock");
    let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let request = json!({"generation": 1, "provider": "local", "target_language": "en", "candidates": ["合成文本"]}).to_string();
    let socket = path.to_str().unwrap();
    let result = read(unsafe {
        msime_client_translation_provider_request(
            request.as_ptr(),
            request.len(),
            socket.as_ptr(),
            socket.len(),
        )
    });
    assert_eq!(result["ok"], true);
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
}
