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
    unsafe { call(msime_client_account_harmony_settings_export, &request) }
}

fn apply_call(request: Value) -> Value {
    // SAFETY: 同上。
    unsafe { call(msime_client_account_harmony_settings_apply, &request) }
}

fn feedback() -> Value {
    json!({ "soundEnabled": true, "hapticsEnabled": false, "hapticStrength": "light" })
}

/// 合成的字段表，键和类型成对给出。
fn schema(fields: &[(&str, &str)]) -> Value {
    let fields: serde_json::Map<String, Value> = fields
        .iter()
        .map(|(key, kind)| ((*key).to_owned(), json!({ "type": kind })))
        .collect();
    json!({
        "fields": fields,
        "maximum_bytes": 1_048_576,
        "update_mode": "replace",
        "revision_required": true,
    })
}

#[test]
fn export_keeps_only_the_keys_the_schema_lists_and_merges_over_the_cloud() {
    let directory = tempfile::tempdir().unwrap();
    let result = export_call(json!({
        "preferences_directory": directory.path(),
        "feedback": feedback(),
        "schema": schema(&[
            ("input.schema", "string"),
            ("input.learning", "boolean"),
            ("platform.harmony.haptic_strength", "string"),
        ]),
        "cloud": { "revision": 3, "settings": {
            "input.schema": "wubi",
            "platform.ios.keyboard_skin": "rose",
        } },
    }));
    assert_eq!(result["ok"], true, "{result}");
    let settings = result["value"]["settings"].as_object().unwrap();
    assert_eq!(
        settings.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "input.learning",
            "input.schema",
            "platform.harmony.haptic_strength"
        ]
    );
    assert_eq!(settings["input.schema"], "quanpin");
    assert_eq!(settings["platform.harmony.haptic_strength"], "light");
    let merged = &result["value"]["merged"];
    assert_eq!(merged["revision"], 3);
    assert_eq!(merged["settings"]["input.schema"], "quanpin");
    // 别的设备写的键原样保留。
    assert_eq!(merged["settings"]["platform.ios.keyboard_skin"], "rose");
}

#[test]
fn export_rejects_an_empty_upload_and_bad_requests() {
    let directory = tempfile::tempdir().unwrap();
    // 服务端还没收录 `platform.harmony.*` 时只有 `input.*` 能传；一个都没有就不给宿主文档。
    let empty = export_call(json!({
        "preferences_directory": directory.path(),
        "feedback": feedback(),
        "schema": schema(&[("platform.android.sound_enabled", "boolean")]),
    }));
    assert_eq!(empty["error"], "account_unavailable", "{empty}");
    for request in [
        json!({ "preferences_directory": "relative", "schema": schema(&[("input.schema", "string")]) }),
        json!({ "preferences_directory": directory.path() }),
        json!({ "preferences_directory": directory.path(), "schema": schema(&[("input.schema", "string")]), "extra": 1 }),
        json!({ "preferences_directory": directory.path(), "schema": schema(&[("input.schema", "string")]), "feedback": { "soundEnabled": true } }),
        // 字节上限带小数的字段表不是合法的字段表。
        json!({ "preferences_directory": directory.path(), "schema": {
            "fields": {}, "maximum_bytes": 64.5, "update_mode": "replace", "revision_required": true,
        } }),
    ] {
        assert_eq!(
            export_call(request.clone())["error"],
            "account_invalid",
            "{request}"
        );
    }
    // 合并时写字段表里没有声明的类型：整份拒绝。
    let clash = export_call(json!({
        "preferences_directory": directory.path(),
        "schema": schema(&[("input.learning", "string")]),
        "cloud": { "revision": 1, "settings": {} },
    }));
    assert_eq!(clash["error"], "account_invalid", "{clash}");
}

#[test]
fn apply_saves_the_whole_snapshot_and_hands_the_feedback_back() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let before = store.load().unwrap();
    let result = apply_call(json!({
        "preferences_directory": directory.path(),
        "cloud": { "revision": 4, "settings": {
            "input.schema": "shuangpin",
            "input.learning": false,
            "platform.harmony.number_keypad_order": "calculator",
            "platform.harmony.sound_enabled": false,
            "platform.harmony.theme": "sepia",
            "platform.android.keyboard_layout": "nine_key",
        } },
        "schema": schema(&[
            ("input.schema", "string"),
            ("input.learning", "boolean"),
            ("platform.harmony.number_keypad_order", "string"),
            ("platform.harmony.sound_enabled", "boolean"),
            ("platform.harmony.theme", "string"),
            ("platform.android.keyboard_layout", "string"),
        ]),
        "feedback": feedback(),
    }));
    assert_eq!(result["ok"], true, "{result}");
    let value = &result["value"];
    // 保存的是整份快照：修订号按读到的那个加一，格式版本照旧。
    let saved = &value["preferences"];
    assert_eq!(saved["revision"], before.revision + 1);
    assert_eq!(
        saved["format_version"],
        serde_json::to_value(&before).unwrap()["format_version"]
    );
    assert_eq!(saved["preferences"]["scheme"], "shuangpin");
    assert_eq!(saved["preferences"]["learning"], false);
    assert_eq!(
        saved["preferences"]["touch_number_keypad_order"],
        "calculator"
    );
    // Android 的布局键不归鸿蒙。
    assert_eq!(
        saved["preferences"]["touch_keyboard_layout"],
        "twenty_six_key"
    );
    assert_eq!(store.load().unwrap().revision, before.revision + 1);
    assert_eq!(
        value["feedback"],
        json!({ "soundEnabled": false, "hapticsEnabled": false, "hapticStrength": "light" })
    );
    assert_eq!(value["skipped"], json!(["platform.harmony.theme"]));

    // 什么都不变时不写，也不交回反馈。
    let unchanged = apply_call(json!({
        "preferences_directory": directory.path(),
        "cloud": { "revision": 5, "settings": { "input.schema": "shuangpin" } },
        "schema": schema(&[("input.schema", "string")]),
    }));
    assert_eq!(unchanged["ok"], true, "{unchanged}");
    assert_eq!(
        unchanged["value"]["preferences"]["revision"],
        before.revision + 1
    );
    assert_eq!(unchanged["value"]["feedback"], Value::Null);
}

#[test]
fn apply_follows_the_edition_of_the_preferences_directory() {
    let directory = tempfile::tempdir().unwrap();
    let wubi = msime_client_core::edition::Edition::by_id("wubi").unwrap();
    wubi.record_in(directory.path()).unwrap();
    let result = apply_call(json!({
        "preferences_directory": directory.path(),
        "cloud": { "revision": 2, "settings": {
            "input.schema": "quanpin",
            "platform.harmony.keyboard_layout": "nine_key",
            "input.learning": false,
        } },
        "schema": schema(&[
            ("input.schema", "string"),
            ("platform.harmony.keyboard_layout", "string"),
            ("input.learning", "boolean"),
        ]),
    }));
    assert_eq!(result["ok"], true, "{result}");
    let preferences = &result["value"]["preferences"]["preferences"];
    assert_eq!(preferences["learning"], false);
    assert_eq!(preferences["touch_keyboard_layout"], "twenty_six_key");
    assert_eq!(preferences["scheme"], "wubi");
}

#[test]
fn apply_rejects_bad_requests_and_documents() {
    let directory = tempfile::tempdir().unwrap();
    let sound = json!({ "revision": 1, "settings": { "platform.harmony.sound_enabled": false } });
    let fields = schema(&[
        ("platform.harmony.sound_enabled", "boolean"),
        ("input.learning", "boolean"),
    ]);
    // 文档里有按键反馈而宿主没给当前值。
    let missing = apply_call(json!({
        "preferences_directory": directory.path(), "cloud": sound, "schema": fields,
    }));
    assert_eq!(missing["error"], "account_invalid", "{missing}");
    for cloud in [
        // 声明了的键带着别的类型来。
        json!({ "revision": 1, "settings": { "input.learning": 1 } }),
        // 带小数的修订号、不是标量的值。
        json!({ "revision": 2.5, "settings": {} }),
        json!({ "revision": 1, "settings": { "input.learning": null } }),
        json!({ "revision": 1, "settings": { "input.learning": [] } }),
        json!({ "revision": 1, "settings": { "input.learning": {} } }),
    ] {
        let result = apply_call(json!({
            "preferences_directory": directory.path(), "cloud": cloud, "schema": fields, "feedback": feedback(),
        }));
        assert_eq!(result["error"], "account_invalid", "{cloud}");
    }
    // 关掉修订号检查的字段表不可信。
    let mut unchecked = fields.clone();
    unchecked["revision_required"] = json!(false);
    let result = apply_call(json!({
        "preferences_directory": directory.path(),
        "cloud": { "revision": 1, "settings": { "input.learning": false } },
        "schema": unchecked,
    }));
    assert_eq!(result["error"], "account_unavailable", "{result}");
    // 偏好文件一次都没写过。
    assert!(!directory.path().join("preferences.json").exists());
}
