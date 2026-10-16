//! 鸿蒙的账号设置同步：账号设置文档与共享偏好、按键反馈之间的映射（client-services 的 `account::harmony_settings`），原来是鸿蒙 `HarmonyAccountCloudBridge.ets` 的 `preferences_upload`、`preferences_apply` 里调 `AccountPreferencePlan.ts` 的那部分。两个函数都不经过会话，联网、读会话和代次检查由宿主做，与 iOS 那一对（`msime_client_account_ios_settings_export`、`msime_client_account_ios_settings_apply`）相同。
//!
//! 属于 C ABI 的一部分，这些薄封装保证什么见父模块。每个函数都读写文件，宿主在工作线程调用。

use super::with_bounded_bytes;
use crate::*;
use msime_client_core::edition::{
    filter_downloaded_account_settings, filter_uploaded_account_settings,
};
use msime_client_core::preferences::PreferencesError;
use msime_client_services::account::harmony_settings::{
    apply_harmony_settings, export_harmony_settings, needs_harmony_feedback,
};
use msime_client_services::account::settings_sync::HostKeyboardFeedback;
use msime_client_services::account::{
    merge_account_preferences, AccountError, AccountPreferenceSchema, AccountPreferences,
};

/// 请求的字节上限：云端文档、字段表各一份，文档里可能带着最多 1 MiB 的自定义键盘设计。
const HARMONY_SETTINGS_REQUEST_LIMIT: usize = 4 * 1024 * 1024;
const MAX_DIRECTORY_BYTES: usize = 16 * 1024;

fn invalid() -> String {
    AccountError::Invalid.code().to_owned()
}

fn code(error: AccountError) -> String {
    error.code().to_owned()
}

fn preferences_store(directory: &str) -> Result<PreferencesStore, String> {
    let path = Path::new(directory);
    if directory.len() > MAX_DIRECTORY_BYTES || !path.is_absolute() {
        return Err(invalid());
    }
    Ok(PreferencesStore::new(path))
}

/// 偏好存储的保存错误，与 `msime_client_account_ios_settings_apply` 相同：修订号不符是 `conflict`，其余是 `storage`。
fn preferences_error(error: PreferencesError) -> String {
    match error {
        PreferencesError::Conflict => "conflict",
        _ => "storage",
    }
    .to_owned()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HarmonySettingsExportRequest {
    preferences_directory: String,
    /// 宿主反馈文件里的按键音、振动开关和振动强度；不带时不导出这三个键。
    #[serde(default)]
    feedback: Option<HostKeyboardFeedback>,
    schema: AccountPreferenceSchema,
    #[serde(default)]
    cloud: Option<AccountPreferences>,
}

fn export(request: HarmonySettingsExportRequest) -> Result<Value, String> {
    let store = preferences_store(&request.preferences_directory)?;
    let local = store
        .load()
        .map_err(|_| AccountError::Storage.code().to_owned())?;
    let mut settings =
        export_harmony_settings(&local.preferences, request.feedback.as_ref()).map_err(code)?;
    // 单方案版本不上传方案和随它的布局，多方案版本只上传本版本提供的方案。
    filter_uploaded_account_settings(Some(store.edition()), &mut settings);
    // 服务端只要有一个键不在字段表里就拒绝整份文档，所以按字段表过滤；服务端收录 `platform.harmony.*` 之前，只有 `input.*` 留下来。
    settings.retain(|key, _| request.schema.fields.contains_key(key));
    // 与鸿蒙原来的上传相同：没有可上传的键时不给宿主一份与云端相同的文档去覆盖。
    if settings.is_empty() {
        return Err(AccountError::Unavailable.code().to_owned());
    }
    let mut value = json!({ "settings": settings });
    if let Some(cloud) = &request.cloud {
        let merged = merge_account_preferences(cloud, &settings, &request.schema).map_err(code)?;
        value["merged"] = serde_json::to_value(merged).map_err(|_| invalid())?;
    }
    Ok(value)
}

/// 把鸿蒙的本机设置导出成账号设置文档的键值。请求、返回值和错误码见头文件。
/// # Safety
/// `request` 指向 `length` 个可读字节；NULL 被拒绝。返回的响应要用 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_account_harmony_settings_export(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: 由上面的调用方契约保证。
        unsafe {
            with_bounded_bytes(
                request,
                length,
                HARMONY_SETTINGS_REQUEST_LIMIT,
                "account_invalid",
                |bytes| {
                    let request: HarmonySettingsExportRequest =
                        serde_json::from_slice(bytes).map_err(|_| invalid())?;
                    export(request)
                },
            )
        }
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HarmonySettingsApplyRequest {
    preferences_directory: String,
    cloud: AccountPreferences,
    schema: AccountPreferenceSchema,
    /// 宿主反馈文件里当前的按键反馈；文档和字段表里都有按键反馈的键时必须带。
    #[serde(default)]
    feedback: Option<HostKeyboardFeedback>,
}

fn apply(mut request: HarmonySettingsApplyRequest) -> Result<Value, String> {
    let store = preferences_store(&request.preferences_directory)?;
    // 单方案版本不应用账号里的方案，多方案版本把本版本没有的方案当作缺失。
    filter_downloaded_account_settings(Some(store.edition()), &mut request.cloud.settings);
    if request.feedback.is_none() && needs_harmony_feedback(&request.cloud, &request.schema) {
        return Err(invalid());
    }
    let local = store
        .load()
        .map_err(|_| AccountError::Storage.code().to_owned())?;
    let applied = apply_harmony_settings(
        &local.preferences,
        &request.cloud,
        &request.schema,
        request.feedback,
    )
    .map_err(code)?;
    let snapshot = if applied.preferences == local.preferences {
        local
    } else {
        store
            .save(local.revision, applied.preferences)
            .map_err(preferences_error)?
    };
    Ok(json!({
        "preferences": serde_json::to_value(snapshot).map_err(|_| "storage".to_owned())?,
        "feedback": applied.feedback,
        "skipped": applied.skipped,
    }))
}

/// 把云端设置文档应用到鸿蒙的共享偏好，按键反馈交还宿主写。请求、返回值和错误码见头文件。
/// # Safety
/// `request` 指向 `length` 个可读字节；NULL 被拒绝。返回的响应要用 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_account_harmony_settings_apply(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: 由上面的调用方契约保证。
        unsafe {
            with_bounded_bytes(
                request,
                length,
                HARMONY_SETTINGS_REQUEST_LIMIT,
                "account_invalid",
                |bytes| {
                    let request: HarmonySettingsApplyRequest =
                        serde_json::from_slice(bytes).map_err(|_| invalid())?;
                    apply(request)
                },
            )
        }
    })
}

#[cfg(test)]
mod tests {
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
        let sound =
            json!({ "revision": 1, "settings": { "platform.harmony.sound_enabled": false } });
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
}
