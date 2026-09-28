use crate::shared::account_dto::{
    ChallengeResponse, ChatModelsResponse, ChatResponse, PreferenceSchemaResponse, ProfileResponse,
    StatusResponse,
};
use msime_client_core::account::AccountChatMessage;
use msime_client_core::account::AccountError;
use msime_client_core::account::AccountPreferences;
use msime_client_core::cloud::dictionary::DictionaryKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

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
        let _ = std::fs::remove_file(item.path);
    }
}

pub(crate) fn dictionary_kind(value: &str) -> Result<DictionaryKind, crate::CommandError> {
    match value {
        "pinyin" => Ok(DictionaryKind::Pinyin),
        "wubi" => Ok(DictionaryKind::Wubi),
        "quick" => Ok(DictionaryKind::Quick),
        "english" => Ok(DictionaryKind::English),
        _ => Err(crate::CommandError {
            code: "invalid_cloud_dictionary",
        }),
    }
}

pub(crate) fn snapshot_command_error() -> crate::CommandError {
    crate::CommandError {
        code: "snapshot_unavailable",
    }
}

pub(crate) fn snapshot_response_without_account(
    mut value: Value,
) -> Result<Value, crate::CommandError> {
    let object = value.as_object_mut().ok_or_else(snapshot_command_error)?;
    if let Some(request) = object.get_mut("request").and_then(Value::as_object_mut) {
        request.remove("accountId");
    }
    Ok(value)
}

pub(crate) async fn call<T, F>(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
    operation: F,
) -> Result<T, crate::CommandError>
where
    T: Send + 'static,
    F: FnOnce(&crate::platform::mobile::MobileSession) -> Result<T, AccountError> + Send + 'static,
{
    let session = Arc::clone(state.session());
    tauri::async_runtime::spawn_blocking(move || operation(&session))
        .await
        .map_err(|_| crate::CommandError {
            code: "account_unavailable",
        })?
        .map_err(|error| crate::CommandError { code: error.code() })
}

pub(crate) async fn account_status(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
) -> Result<StatusResponse, crate::CommandError> {
    call(state, |session| {
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
    call(state, move |session| {
        session
            .request_code(&provider, &target)
            .map(ChallengeResponse::from)
    })
    .await
}

pub(crate) async fn account_login(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
    challenge_id: String,
    code: String,
) -> Result<StatusResponse, crate::CommandError> {
    call(state, move |session| {
        session
            .sign_in(&challenge_id, &code)
            .map(|user| StatusResponse {
                user: Some(user.into()),
            })
    })
    .await
}

pub(crate) async fn account_profile(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
) -> Result<ProfileResponse, crate::CommandError> {
    call(state, |session| {
        session.profile().map(ProfileResponse::from)
    })
    .await
}

pub(crate) async fn account_chat_models(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
) -> Result<ChatModelsResponse, crate::CommandError> {
    call(state, |session| session.chat_models().map(Into::into)).await
}

pub(crate) async fn account_chat(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
    messages: Vec<AccountChatMessage>,
    model: String,
) -> Result<ChatResponse, crate::CommandError> {
    call(state, move |session| {
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
    call(state, move |session| {
        session.rename(&display_name).map(ProfileResponse::from)
    })
    .await
}

pub(crate) async fn account_preferences_schema(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
) -> Result<PreferenceSchemaResponse, crate::CommandError> {
    call(state, |session| session.preference_schema().map(Into::into)).await
}

pub(crate) async fn account_preferences_load(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
) -> Result<AccountPreferences, crate::CommandError> {
    call(state, |session| session.preferences()).await
}

pub(crate) async fn account_logout(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
    all: bool,
) -> Result<(), crate::CommandError> {
    call(state, move |session| session.logout(all)).await
}

pub(crate) async fn account_delete(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
) -> Result<(), crate::CommandError> {
    call(state, |session| session.delete_account()).await
}

pub(crate) async fn account_forget(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
) -> Result<(), crate::CommandError> {
    call(state, |session| session.forget()).await
}
