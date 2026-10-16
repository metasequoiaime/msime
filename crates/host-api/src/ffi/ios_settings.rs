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
mod tests;
