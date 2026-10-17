//! `msime_client_account` 的 `preferences_schema` 交给宿主的字段表是 camelCase（#8201 的 P5）：鸿蒙、iOS 和 Android 的设置导出与应用都能原样收下它，宿主不用改键名。

use super::harmony_settings::{
    msime_client_account_harmony_settings_apply, msime_client_account_harmony_settings_export,
};
use super::ios_settings::msime_client_account_ios_settings_export;
use super::*;
use msime_client_services::account::page::PreferenceSchemaResponse;
use msime_client_services::account::{AccountPreferenceField, AccountPreferenceSchema};
use std::collections::BTreeMap;
use std::ffi::CStr;

fn call(function: unsafe extern "C" fn(*const u8, usize) -> *mut c_char, request: Value) -> Value {
    let bytes = request.to_string();
    // SAFETY: 缓冲区在本次调用期间有效，返回的字符串在下面释放。
    unsafe {
        let raw = function(bytes.as_ptr(), bytes.len());
        let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
        msime_client_string_free(raw);
        serde_json::from_str(&text).unwrap()
    }
}

/// `preferences_schema` 的 value：服务端字段表经 [`PreferenceSchemaResponse`] 序列化，`maximumBytes`、`updateMode`、`revisionRequired` 是 camelCase。
fn page_schema(fields: &[(&str, &str)]) -> Value {
    let schema = AccountPreferenceSchema {
        fields: fields
            .iter()
            .map(|(key, kind)| {
                (
                    (*key).to_owned(),
                    AccountPreferenceField {
                        value_type: (*kind).to_owned(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>(),
        maximum_bytes: 1_048_576,
        update_mode: "replace".into(),
        revision_required: true,
    };
    let value = serde_json::to_value(PreferenceSchemaResponse::from(schema)).unwrap();
    assert!(value.get("maximumBytes").is_some(), "{value}");
    value
}

fn cloud() -> Value {
    json!({ "revision": 3, "settings": { "input.schema": "wubi", "platform.ios.keyboard_skin": "rose" } })
}

#[test]
fn the_harmony_export_and_apply_take_the_page_schema() {
    let directory = tempfile::tempdir().unwrap();
    let schema = page_schema(&[("input.schema", "string")]);
    let exported = call(
        msime_client_account_harmony_settings_export,
        json!({ "preferences_directory": directory.path(), "schema": schema, "cloud": cloud() }),
    );
    assert_eq!(exported["ok"], true, "{exported}");
    assert_eq!(exported["value"]["merged"]["revision"], 3);
    let applied = call(
        msime_client_account_harmony_settings_apply,
        json!({ "preferences_directory": directory.path(), "schema": schema, "cloud": cloud() }),
    );
    assert_eq!(applied["ok"], true, "{applied}");
}

#[test]
fn the_ios_export_takes_the_page_schema() {
    let directory = tempfile::tempdir().unwrap();
    let exported = call(
        msime_client_account_ios_settings_export,
        json!({
            "preferences_directory": directory.path(),
            "ios_native": {
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
            },
            "schema": page_schema(&[("input.schema", "string")]),
            "cloud": cloud(),
        }),
    );
    assert_eq!(exported["ok"], true, "{exported}");
    assert_eq!(exported["value"]["settings"]["input.schema"], "shuangpin");
    assert_eq!(exported["value"]["merged"]["revision"], 3);
}

#[test]
fn the_android_export_and_apply_take_the_page_schema() {
    let directory = tempfile::tempdir().unwrap();
    let schema = page_schema(&[("input.schema", "string")]);
    let exported = call(
        msime_client_account_settings_export,
        json!({ "preferences_directory": directory.path(), "schema": schema, "cloud": cloud() }),
    );
    assert_eq!(exported["ok"], true, "{exported}");
    // 带字段表时只留它收录的键。
    assert_eq!(
        exported["value"]["settings"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["input.schema"]
    );
    assert_eq!(exported["value"]["merged"]["revision"], 3);
    let applied = call(
        msime_client_account_settings_apply,
        json!({ "preferences_directory": directory.path(), "schema": schema, "cloud": cloud() }),
    );
    assert_eq!(applied["ok"], true, "{applied}");
}
