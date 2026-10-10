//! Windows 的「设置同步」：把本机的共享偏好上传到账号，或把账号里的设置应用到本机。
//!
//! 页面是 iOS、Android 也在用的设置同步卡片，四个命令的名字和返回值与移动端相同。上传、应用的只有各平台共有的 `input.*` 键（`settings_sync::export_desktop_settings`、`apply_desktop_settings`）：Windows 的设置就是共享偏好文档，桌面专属的设置在账号文档里只有 macOS 原生偏好的 `platform.macos.*`，Windows 不去改它。应用走与设置页保存相同的 [`crate::save_preferences_impl`]，按读到时的修订号写入，期间本机设置变过就拒绝，同时把新偏好同步进运行时选项。

use crate::platform::account_helpers::call_session;
use crate::platform::desktop::desktop_account::AccountState;
use crate::shared::account_dto::PreferenceSchemaResponse;
use msime_client_core::account::settings_sync::{apply_desktop_settings, export_desktop_settings};
use msime_client_core::account::{merge_account_preferences, AccountError, AccountPreferences};
use msime_client_core::edition::{
    filter_downloaded_account_settings, filter_uploaded_account_settings,
};
use msime_client_core::preferences::PreferencesStore;
use std::collections::BTreeMap;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn account_preferences_schema(
    state: State<'_, AccountState>,
) -> Result<PreferenceSchemaResponse, crate::CommandError> {
    call_session(&state.session, |session| {
        session.preference_schema().map(Into::into)
    })
    .await
}

#[tauri::command]
pub async fn account_preferences_load(
    state: State<'_, AccountState>,
) -> Result<AccountPreferences, crate::CommandError> {
    call_session(&state.session, |session| session.preferences()).await
}

/// 本机的 `input.*` 设置合并进账号文档上传，其他平台的设置原样保留。字段表里没有的键不上传；一个都没有时报告服务不可用。
#[tauri::command]
pub async fn account_preferences_upload(
    app: tauri::AppHandle,
    state: State<'_, AccountState>,
    store: State<'_, Arc<PreferencesStore>>,
) -> Result<AccountPreferences, crate::CommandError> {
    let store = store.inner().clone();
    let edition = crate::host_edition(&app);
    call_session(&state.session, move |session| {
        let (user_id, _, generation) = session.credentials_with_generation(None, None)?;
        let schema = session.preference_schema()?;
        let cloud = session.preferences()?;
        let local = store.load().map_err(|_| AccountError::Storage)?;
        let mut values = export_desktop_settings(&local.preferences);
        // 单方案版本不上传方案，多方案版本只上传本版本提供的方案。
        filter_uploaded_account_settings(edition, &mut values);
        let values = values
            .into_iter()
            .filter(|(key, _)| schema.fields.contains_key(key))
            .collect::<BTreeMap<_, _>>();
        if values.is_empty() {
            return Err(AccountError::Unavailable);
        }
        let merged = merge_account_preferences(&cloud, &values, &schema)?;
        session.put_preferences_with_generation(&merged, generation, &user_id)
    })
    .await
}

/// 把页面刚下载的账号设置应用到本机。`user_id` 是页面下载时的账号，期间退出或换号就取消。
#[tauri::command]
pub async fn account_preferences_apply(
    app: tauri::AppHandle,
    state: State<'_, AccountState>,
    store: State<'_, Arc<PreferencesStore>>,
    runtime: State<'_, crate::RuntimeOptionsState>,
    user_id: String,
    mut preferences: AccountPreferences,
) -> Result<(), crate::CommandError> {
    // 单方案版本不应用账号里的方案，多方案版本把本版本没有的方案当作缺失。
    filter_downloaded_account_settings(crate::host_edition(&app), &mut preferences.settings);
    let local_store = store.inner().clone();
    let (revision, next) = call_session(&state.session, move |session| {
        let (_, _, generation) = session.credentials_with_generation(None, Some(&user_id))?;
        let schema = session.preference_schema()?;
        let local = local_store.load().map_err(|_| AccountError::Storage)?;
        let applied = apply_desktop_settings(&local.preferences, &preferences, &schema)?;
        session.with_generation(generation, Some(&user_id), || {
            Ok((local.revision, applied.preferences))
        })
    })
    .await?;
    crate::save_preferences_impl(
        store.inner().clone(),
        runtime.inner().clone(),
        revision,
        next,
    )
    .await
    .map(|_| ())
}
