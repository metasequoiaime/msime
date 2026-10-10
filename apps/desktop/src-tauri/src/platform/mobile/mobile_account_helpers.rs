pub(crate) use crate::platform::account_helpers::{
    account_command_error, call_session, cleanup_stale_snapshot_previews,
    prepare_snapshot_directory, publish_snapshot_preview, read_snapshot_file, remove_snapshot_file,
    snapshot_text_within_limit, take_invalid_snapshot_previews, write_snapshot_file,
};
pub(crate) use crate::platform::cloud_dictionary::{
    parse_cloud_dictionary_request, snapshot_command_error, snapshot_response_without_account,
};
pub(crate) use crate::platform::mobile::mobile_account_preferences::valid_mobile_haptic_strength;
use crate::shared::account_dto::{
    ChallengeResponse, ChatModelsResponse, ChatResponse, PreferenceSchemaResponse, ProfileResponse,
    StatusResponse,
};
use msime_client_core::account::AccountChatMessage;
use msime_client_core::account::AccountError;
use msime_client_core::account::AccountPreferences;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SnapshotMetadata {
    pub(crate) cloud_revision: i64,
    pub(crate) sha256: String,
    #[serde(skip_serializing)]
    pub(crate) file_sha256: String,
    pub(crate) bytes: u64,
    pub(crate) records: usize,
    pub(crate) entries: usize,
    pub(crate) overlays: usize,
    pub(crate) positions: usize,
    pub(crate) selections: usize,
}

#[derive(Clone)]
pub(crate) struct PendingSnapshot {
    pub(crate) account_id: String,
    pub(crate) generation: u64,
    pub(crate) path: PathBuf,
    pub(crate) metadata: SnapshotMetadata,
}

pub(crate) fn clear_snapshot_previews(
    previews: &std::sync::Arc<std::sync::Mutex<HashMap<String, PendingSnapshot>>>,
) {
    let Ok(mut pending) = previews.lock() else {
        return;
    };
    for item in pending.drain().map(|(_, item)| item) {
        let _ = remove_snapshot_file(&item.path);
    }
}

pub(crate) fn validate_pending_snapshot(
    session: &crate::platform::mobile::MobileSession,
    pending: &PendingSnapshot,
) -> Result<(), AccountError> {
    session.with_generation(pending.generation, Some(&pending.account_id), || Ok(()))?;
    let profile = session.profile()?;
    if profile.user.id != pending.account_id {
        return Err(AccountError::Conflict);
    }
    let changes = session.dictionary_changes(pending.metadata.cloud_revision, 1)?;
    if !changes.changes.is_empty() {
        return Err(AccountError::Conflict);
    }
    Ok(())
}

pub(crate) fn take_pending_snapshot(
    previews: &std::sync::Arc<std::sync::Mutex<HashMap<String, PendingSnapshot>>>,
    token: &str,
) -> Result<PendingSnapshot, crate::CommandError> {
    let token = parse_snapshot_token(token)?;
    let mut pending = previews.lock().map_err(|_| snapshot_command_error())?;
    pending
        .remove(&token.to_string())
        .ok_or(crate::CommandError {
            code: "snapshot_invalid",
        })
}

pub(crate) fn clear_snapshot_previews_after<T>(
    session: &std::sync::Arc<crate::platform::mobile::MobileSession>,
    previews: &std::sync::Arc<std::sync::Mutex<HashMap<String, PendingSnapshot>>>,
    result: Result<T, crate::CommandError>,
) -> Result<T, crate::CommandError> {
    let stale = take_invalid_snapshot_previews(session, previews, |preview| {
        (&preview.account_id, preview.generation)
    });
    match stale {
        Ok(stale) => {
            for preview in stale {
                let _ = remove_snapshot_file(&preview.path);
            }
        }
        Err(error) if result.is_ok() => return Err(account_command_error(error)),
        Err(_) => {}
    }
    result
}

/// 用本平台的账号会话处理云词库请求，分发在 [`crate::platform::cloud_dictionary::account_request`]，与 Windows 共用。快照操作由各平台自己处理：Android 经插件落盘，iOS 经原生桥接；返回 `None` 时由调用方接手。
pub(crate) async fn cloud_dictionary_account_request(
    state: &tauri::State<'_, crate::platform::mobile::MobileAccountState>,
    request: &msime_host_api::cloud_dictionary::CloudDictionaryRequest,
) -> Option<Result<Value, crate::CommandError>> {
    crate::platform::cloud_dictionary::account_request(state.session(), request).await
}

pub(crate) fn parse_snapshot_token(value: &str) -> Result<uuid::Uuid, crate::CommandError> {
    uuid::Uuid::parse_str(value).map_err(|_| crate::CommandError {
        code: "snapshot_invalid",
    })
}

pub(crate) async fn account_status(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
) -> Result<StatusResponse, crate::CommandError> {
    call_session(state.session(), |session| {
        session.status().map(|user| StatusResponse {
            user: user.map(Into::into),
        })
    })
    .await
}

pub(crate) async fn account_request_code(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
    provider: String,
    target: String,
) -> Result<ChallengeResponse, crate::CommandError> {
    call_session(state.session(), move |session| {
        session
            .request_code(&provider, &target)
            .map(ChallengeResponse::from)
    })
    .await
}

pub(crate) async fn account_login<F>(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
    challenge_id: String,
    code: String,
    cleanup: F,
) -> Result<StatusResponse, crate::CommandError>
where
    F: FnOnce(&str) + Send + 'static,
{
    call_session(state.session(), move |session| {
        session
            .sign_in_with_cleanup(&challenge_id, &code, cleanup)
            .map(|user| StatusResponse {
                user: Some(user.into()),
            })
    })
    .await
}

pub(crate) async fn account_profile(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
) -> Result<ProfileResponse, crate::CommandError> {
    call_session(state.session(), |session| {
        session.profile().map(ProfileResponse::from)
    })
    .await
}

pub(crate) async fn account_chat_models(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
) -> Result<ChatModelsResponse, crate::CommandError> {
    call_session(state.session(), |session| {
        session.chat_models().map(Into::into)
    })
    .await
}

pub(crate) async fn account_chat(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
    messages: Vec<AccountChatMessage>,
    model: String,
) -> Result<ChatResponse, crate::CommandError> {
    call_session(state.session(), move |session| {
        session
            .chat(&messages, &model)
            .map(|content| ChatResponse { content })
    })
    .await
}

pub(crate) async fn account_rename(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
    display_name: String,
) -> Result<ProfileResponse, crate::CommandError> {
    call_session(state.session(), move |session| {
        session.rename(&display_name).map(ProfileResponse::from)
    })
    .await
}

pub(crate) async fn account_preferences_schema(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
) -> Result<PreferenceSchemaResponse, crate::CommandError> {
    call_session(state.session(), |session| {
        session.preference_schema().map(Into::into)
    })
    .await
}

pub(crate) async fn account_preferences_load(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
) -> Result<AccountPreferences, crate::CommandError> {
    call_session(state.session(), |session| session.preferences()).await
}
