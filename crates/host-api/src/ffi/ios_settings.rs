//! iOS 的账号设置同步：账号设置文档与 App Group 里的键盘设置、共享偏好之间的映射（client-services 的 `account::ios_settings`），与 Tauri iOS 的 `account_preferences_upload`、`account_preferences_apply` 相同。两个函数都不经过会话，联网、读会话和代次检查由宿主做，与 Android 那一对（`msime_client_account_settings_export`、`msime_client_account_settings_apply`）相同。
//!
//! 属于 C ABI 的一部分，这些薄封装保证什么见父模块。每个函数都读写文件，宿主在工作线程调用。

use super::with_bounded_bytes;
use crate::*;
use msime_client_core::edition::{
    filter_downloaded_account_settings, filter_uploaded_account_settings,
};
use msime_client_core::ios_keyboard::IosKeyboardPreferences;
use msime_client_core::preferences::PreferencesError;
use msime_client_services::account::ios_settings::{local_account_preferences, IosPreferencePlan};
use msime_client_services::account::{
    merge_account_preferences, AccountError, AccountPreferenceSchema, AccountPreferences,
};

/// 请求的字节上限：云端文档、字段表和最多 800 000 字节的自定义键盘皮肤各一份。
const IOS_SETTINGS_REQUEST_LIMIT: usize = 4 * 1024 * 1024;
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

/// 偏好存储的保存错误，与 `msime_client_account` 的 `preferences_apply` 相同：修订号不符是 `conflict`，其余是 `storage`。
fn preferences_error(error: PreferencesError) -> String {
    match error {
        PreferencesError::Conflict => "conflict",
        _ => "storage",
    }
    .to_owned()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IosSettingsExportRequest {
    preferences_directory: String,
    ios_native: IosKeyboardPreferences,
    schema: AccountPreferenceSchema,
    #[serde(default)]
    cloud: Option<AccountPreferences>,
}

fn export(request: IosSettingsExportRequest) -> Result<Value, String> {
    let store = preferences_store(&request.preferences_directory)?;
    let local = store
        .load()
        .map_err(|_| AccountError::Storage.code().to_owned())?;
    let mut settings = local_account_preferences(
        &request.ios_native,
        &local.preferences,
        local.preferences.custom_theme.keyboard.as_ref(),
    )
    .map_err(code)?;
    // 单方案版本不上传方案，多方案版本只上传本版本提供的方案。
    filter_uploaded_account_settings(Some(store.edition()), &mut settings);
    // 服务端只要有一个键不在字段表里就拒绝整份文档，所以按字段表过滤；`platform.ios.global_theme` 和 `platform.ios.custom_theme_base` 现在不在表里，会在这里被去掉。
    settings.retain(|key, _| request.schema.fields.contains_key(key));
    // 与 Tauri iOS 相同：没有可上传的键时不给宿主一份与云端相同的文档去覆盖。
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

/// 把本机的 iOS 设置导出成账号设置文档的键值。请求、返回值和错误码见头文件。
/// # Safety
/// `request` 指向 `length` 个可读字节；NULL 被拒绝。返回的响应要用 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_account_ios_settings_export(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: 由上面的调用方契约保证。
        unsafe {
            with_bounded_bytes(
                request,
                length,
                IOS_SETTINGS_REQUEST_LIMIT,
                "account_invalid",
                |bytes| {
                    let request: IosSettingsExportRequest =
                        serde_json::from_slice(bytes).map_err(|_| invalid())?;
                    export(request)
                },
            )
        }
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ApplyStep {
    Plan,
    Commit,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IosSettingsApplyRequest {
    preferences_directory: String,
    cloud: AccountPreferences,
    step: ApplyStep,
    /// `plan` 要带：宿主 App Group 里当前的键盘设置。
    #[serde(default)]
    ios_native: Option<IosKeyboardPreferences>,
    /// `commit` 要带：宿主按 `plan` 的结果写进 App Group 以后，实际保存下来的键盘设置。
    #[serde(default)]
    saved_native: Option<IosKeyboardPreferences>,
}

fn apply(mut request: IosSettingsApplyRequest) -> Result<Value, String> {
    let store = preferences_store(&request.preferences_directory)?;
    // 单方案版本不应用账号里的方案，多方案版本把本版本没有的方案当作缺失。
    filter_downloaded_account_settings(Some(store.edition()), &mut request.cloud.settings);
    let plan = IosPreferencePlan::from_cloud(&request.cloud).map_err(code)?;
    match request.step {
        ApplyStep::Plan => {
            let current = request.ios_native.ok_or_else(invalid)?;
            let requested = plan.requested_native(&current).map_err(code)?;
            Ok(json!({ "ios_native": requested }))
        }
        ApplyStep::Commit => {
            let saved = request.saved_native.ok_or_else(invalid)?;
            let local = store
                .load()
                .map_err(|_| AccountError::Storage.code().to_owned())?;
            let mut next = local.preferences.clone();
            plan.apply_shared(&saved, &mut next).map_err(code)?;
            let snapshot = store
                .save(local.revision, next)
                .map_err(preferences_error)?;
            Ok(json!({
                "preferences": serde_json::to_value(snapshot).map_err(|_| "storage".to_owned())?,
            }))
        }
    }
}

/// 把云端设置文档应用到 iOS 的键盘设置和共享偏好，分 `plan` 和 `commit` 两步。请求、返回值和错误码见头文件。
/// # Safety
/// `request` 指向 `length` 个可读字节；NULL 被拒绝。返回的响应要用 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_account_ios_settings_apply(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: 由上面的调用方契约保证。
        unsafe {
            with_bounded_bytes(
                request,
                length,
                IOS_SETTINGS_REQUEST_LIMIT,
                "account_invalid",
                |bytes| {
                    let request: IosSettingsApplyRequest =
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
}
