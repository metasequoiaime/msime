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
mod tests;
