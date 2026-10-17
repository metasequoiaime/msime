use super::*;
use std::ffi::CStr;

unsafe fn call(
    function: unsafe extern "C" fn(*const u8, usize) -> *mut c_char,
    request: &Value,
) -> Value {
    let bytes = request.to_string();
    // SAFETY: 缓冲区在本次调用期间有效，返回的字符串在下面释放。
    unsafe {
        let raw = function(bytes.as_ptr(), bytes.len());
        let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
        msime_client_string_free(raw);
        serde_json::from_str(&text).unwrap()
    }
}

fn export_call(request: Value) -> Value {
    // SAFETY: `call` 让缓冲区在调用期间有效，并释放响应。
    unsafe { call(msime_client_account_ios_settings_export, &request) }
}

fn apply_call(request: Value) -> Value {
    // SAFETY: 同上。
    unsafe { call(msime_client_account_ios_settings_apply, &request) }
}

fn native() -> Value {
    json!({
        "inputScheme": "shuangpin",
        "traditionalChineseOutput": false,
        "soundEnabled": true,
        "hapticsEnabled": false,
        "hapticStrength": "medium",
        "englishSuggestions": true,
        "candidatePaletteFollowsDesktop": false,
        "inlinePreedit": false,
        "hapticsAvailable": true,
        "dictionaryLearning": true,
        "globalTheme": "shuishan",
        "customKeyboardSkin": null,
    })
}

/// 只收字符串键的合成字段表。
fn schema(keys: &[&str]) -> Value {
    let fields: serde_json::Map<String, Value> = keys
        .iter()
        .map(|key| ((*key).to_owned(), json!({ "type": "string" })))
        .collect();
    json!({
        "fields": fields,
        "maximum_bytes": 1_048_576,
        "update_mode": "replace",
        "revision_required": true,
    })
}

#[test]
fn export_keeps_only_the_keys_the_schema_lists() {
    let directory = tempfile::tempdir().unwrap();
    let result = export_call(json!({
        "preferences_directory": directory.path(),
        "ios_native": native(),
        "schema": schema(&["input.schema", "input.shuangpin_schema", "platform.ios.haptic_strength"]),
        "cloud": { "revision": 3, "settings": { "input.schema": "quanpin" } },
    }));
    assert_eq!(result["ok"], true, "{result}");
    let settings = result["value"]["settings"].as_object().unwrap();
    assert_eq!(
        settings.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "input.schema",
            "input.shuangpin_schema",
            "platform.ios.haptic_strength"
        ]
    );
    assert_eq!(settings["input.schema"], "shuangpin");
    assert_eq!(result["value"]["merged"]["revision"], 3);
    assert_eq!(
        result["value"]["merged"]["settings"]["input.schema"],
        "shuangpin"
    );
    // 隐私相关的键和 schema 外的主题键都不在结果里。
    assert!(settings.get("platform.ios.custom_theme_base").is_none());
}

#[test]
fn export_rejects_an_empty_upload_and_bad_requests() {
    let directory = tempfile::tempdir().unwrap();
    let empty = export_call(json!({
        "preferences_directory": directory.path(),
        "ios_native": native(),
        "schema": schema(&[]),
    }));
    assert_eq!(empty["error"], "account_unavailable");
    for request in [
        json!({ "preferences_directory": "relative", "ios_native": native(), "schema": schema(&["input.schema"]) }),
        json!({ "preferences_directory": directory.path(), "ios_native": native() }),
        json!({ "preferences_directory": directory.path(), "ios_native": native(), "schema": schema(&["input.schema"]), "extra": 1 }),
    ] {
        assert_eq!(
            export_call(request.clone())["error"],
            "account_invalid",
            "{request}"
        );
    }
    let mut broken = native();
    broken["hapticStrength"] = json!("loud");
    let invalid_native = export_call(json!({
        "preferences_directory": directory.path(),
        "ios_native": broken,
        "schema": schema(&["input.schema"]),
    }));
    assert_eq!(invalid_native["error"], "account_storage");
}

#[test]
fn apply_plans_the_native_values_and_commits_the_shared_preferences() {
    let directory = tempfile::tempdir().unwrap();
    let cloud = json!({
        "revision": 4,
        "settings": {
            "input.schema": "wubi",
            "input.wubi_schema": "wubi98",
            "platform.ios.sound_enabled": false,
            "platform.ios.global_theme": "ink",
        },
    });
    let planned = apply_call(json!({
        "preferences_directory": directory.path(),
        "cloud": cloud,
        "step": "plan",
        "ios_native": native(),
    }));
    assert_eq!(planned["ok"], true, "{planned}");
    let requested = &planned["value"]["ios_native"];
    assert_eq!(requested["inputScheme"], "wubi");
    assert_eq!(requested["soundEnabled"], false);
    assert_eq!(requested["globalTheme"], "ink");
    // 只算不写：共享偏好还没有文件。
    assert!(std::fs::read_dir(directory.path())
        .unwrap()
        .next()
        .is_none());

    let committed = apply_call(json!({
        "preferences_directory": directory.path(),
        "cloud": cloud,
        "step": "commit",
        "saved_native": requested,
    }));
    assert_eq!(committed["ok"], true, "{committed}");
    let preferences = &committed["value"]["preferences"]["preferences"];
    assert_eq!(preferences["scheme"], "wubi");
    assert_eq!(preferences["wubi_profile"], "wubi98");
    assert_eq!(preferences["global_theme"], "ink");
}

#[test]
fn apply_writes_the_account_wubi_profile_when_the_cloud_scheme_is_not_wubi() {
    // #8201 的决定 10：与 Android、桌面和鸿蒙相同，账号里的五笔版本有就写，云端方案是双拼也一样。
    let directory = tempfile::tempdir().unwrap();
    let cloud = json!({
        "revision": 5,
        "settings": { "input.schema": "shuangpin", "input.wubi_schema": "wubi98" },
    });
    let planned = apply_call(json!({
        "preferences_directory": directory.path(),
        "cloud": cloud,
        "step": "plan",
        "ios_native": native(),
    }));
    assert_eq!(planned["ok"], true, "{planned}");
    assert_eq!(planned["value"]["ios_native"]["inputScheme"], "shuangpin");
    let committed = apply_call(json!({
        "preferences_directory": directory.path(),
        "cloud": cloud,
        "step": "commit",
        "saved_native": planned["value"]["ios_native"],
    }));
    assert_eq!(committed["ok"], true, "{committed}");
    assert_eq!(
        committed["value"]["preferences"]["preferences"]["wubi_profile"],
        "wubi98"
    );
}

#[test]
fn apply_rejects_a_missing_native_value_and_an_invalid_cloud_document() {
    let directory = tempfile::tempdir().unwrap();
    let cloud = json!({ "revision": 1, "settings": { "input.schema": "quanpin" } });
    assert_eq!(
        apply_call(
            json!({ "preferences_directory": directory.path(), "cloud": cloud, "step": "plan" })
        )["error"],
        "account_invalid"
    );
    assert_eq!(
        apply_call(
            json!({ "preferences_directory": directory.path(), "cloud": cloud, "step": "commit" })
        )["error"],
        "account_invalid"
    );
    assert_eq!(
        apply_call(
            json!({ "preferences_directory": directory.path(), "cloud": cloud, "step": "later", "ios_native": native() })
        )["error"],
        "account_invalid"
    );
    // 应用不带字段表，与头文件写的相同：多带 schema 是不合法的请求，不是被忽略。
    assert_eq!(
        apply_call(
            json!({ "preferences_directory": directory.path(), "cloud": cloud, "step": "plan", "ios_native": native(), "schema": schema(&["input.schema"]) })
        )["error"],
        "account_invalid"
    );
    assert_eq!(
        apply_call(
            json!({ "preferences_directory": directory.path(), "cloud": cloud, "step": "plan", "ios_native": native() })
        )["ok"],
        true
    );
    let bad = json!({ "revision": 1, "settings": { "input.character_set": "klingon" } });
    assert_eq!(
        apply_call(
            json!({ "preferences_directory": directory.path(), "cloud": bad, "step": "plan", "ios_native": native() })
        )["error"],
        "account_invalid"
    );
}
