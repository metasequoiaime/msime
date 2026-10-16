//! Windows 的「设置同步」：把本机的共享偏好上传到账号，或把账号里的设置应用到本机。
//!
//! 页面是 iOS、Android 也在用的设置同步卡片，四个命令的名字和返回值与移动端相同。上传、应用的只有各平台共有的 `input.*` 键（`settings_sync::export_desktop_settings`、`apply_desktop_settings`）：Windows 的设置就是共享偏好文档，桌面专属的设置在账号文档里只有 macOS 原生偏好的 `platform.macos.*`，Windows 不去改它。应用走与设置页保存相同的 [`crate::save_preferences_impl`]，按读到时的修订号写入，期间本机设置变过就拒绝，同时把新偏好同步进运行时选项。

use crate::platform::account_helpers::call_session;
use crate::platform::desktop::desktop_account::AccountState;
use crate::shared::account_dto::PreferenceSchemaResponse;
use msime_client_core::preferences::PreferencesStore;
use msime_client_services::account::settings_sync::{
    prepare_desktop_settings_apply, upload_desktop_settings,
};
use msime_client_services::account::AccountPreferences;
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

/// 本机的 `input.*` 设置合并进账号文档上传，其他平台的设置原样保留。字段表里没有的键不上传；一个都没有时报告服务不可用。逻辑在 client-services（[`upload_desktop_settings`]），原生设置窗口也用它。
#[tauri::command]
pub async fn account_preferences_upload(
    app: tauri::AppHandle,
    state: State<'_, AccountState>,
    store: State<'_, Arc<PreferencesStore>>,
) -> Result<AccountPreferences, crate::CommandError> {
    let store = store.inner().clone();
    let edition = crate::host_edition(&app);
    call_session(&state.session, move |session| {
        upload_desktop_settings(session, &store, edition)
    })
    .await
}

/// 把页面刚下载的账号设置应用到本机。`user_id` 是页面下载时的账号，期间退出或换号就取消。应用的逻辑在 client-services（[`prepare_desktop_settings_apply`]），原生设置窗口也用它。
#[tauri::command]
pub async fn account_preferences_apply(
    app: tauri::AppHandle,
    state: State<'_, AccountState>,
    store: State<'_, Arc<PreferencesStore>>,
    runtime: State<'_, crate::RuntimeOptionsState>,
    user_id: String,
    preferences: AccountPreferences,
) -> Result<(), crate::CommandError> {
    let edition = crate::host_edition(&app);
    let local_store = store.inner().clone();
    let (revision, next) = call_session(&state.session, move |session| {
        prepare_desktop_settings_apply(session, &local_store, edition, &user_id, preferences)
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
