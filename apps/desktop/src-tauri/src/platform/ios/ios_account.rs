#[cfg(target_os = "ios")]
use crate::platform::mobile::mobile_account_helpers::{
    account_chat as shared_account_chat, account_chat_models as shared_account_chat_models,
    account_command_error, account_delete as shared_account_delete,
    account_forget as shared_account_forget, account_login as shared_account_login,
    account_logout as shared_account_logout,
    account_preferences_load as shared_account_preferences_load,
    account_preferences_schema as shared_account_preferences_schema,
    account_profile as shared_account_profile, account_rename as shared_account_rename,
    account_request_code as shared_account_request_code, account_status as shared_account_status,
    call, clear_snapshot_previews, clear_snapshot_previews_after, cloud_dictionary_account_request,
    parse_snapshot_token, replace_pending_snapshot, snapshot_command_error,
    snapshot_response_without_account, valid_mobile_haptic_strength, validate_pending_snapshot,
    PendingSnapshot, SnapshotMetadata,
};
#[cfg(target_os = "ios")]
use crate::shared::account_dto::{
    providers_response, ChallengeResponse, ProfileResponse, ProvidersResponse, StatusResponse,
};
use crate::shared::account_dto::{ChatModelsResponse, ChatResponse, PreferenceSchemaResponse};
use std::collections::BTreeMap;
#[cfg(target_os = "ios")]
use std::collections::HashMap;

#[cfg(target_os = "ios")]
use serde_json::Value;

#[cfg(target_os = "ios")]
mod account_preferences;

#[cfg(target_os = "ios")]
use crate::platform::mobile::mobile_community::MobileCommunityState;
#[cfg(target_os = "ios")]
use msime_client_core::account::{
    merge_account_preferences, AccountChatMessage, AccountError, AccountPreferences,
    AccountSessionStorage, BackendAccountClient, BackendAccountSession, SavedAccountSession,
};
#[cfg(target_os = "ios")]
use msime_client_core::cloud::dictionary::DictionaryKind;
#[cfg(target_os = "ios")]
use msime_client_core::preferences::PreferencesStore;
#[cfg(target_os = "ios")]
use msime_tauri_mobile_platform::{IosKeyboardPreferences, MobilePlatform};
#[cfg(target_os = "ios")]
use std::sync::Arc;
#[cfg(target_os = "ios")]
use std::{fs, path::PathBuf, sync::Mutex};
#[cfg(target_os = "ios")]
use tauri::{AppHandle, Manager, Runtime, State, Wry};

#[cfg(target_os = "ios")]
#[derive(Clone)]
pub(crate) struct IosAccountStorage<R: Runtime>(MobilePlatform<R>);

#[cfg(target_os = "ios")]
impl<R: Runtime> AccountSessionStorage for IosAccountStorage<R> {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        self.0
            .load_account_session()
            .map_err(|_| AccountError::Storage)?
            .map(|value| serde_json::from_str(&value).map_err(|_| AccountError::Storage))
            .transpose()
    }

    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        let value = serde_json::to_string(session).map_err(|_| AccountError::Storage)?;
        self.0
            .save_account_session(&value)
            .map_err(|_| AccountError::Storage)
    }

    fn clear(&self) -> Result<(), AccountError> {
        self.0
            .clear_account_session()
            .map_err(|_| AccountError::Storage)
    }
}

#[cfg(target_os = "ios")]
type Session = BackendAccountSession<BackendAccountClient, IosAccountStorage<Wry>>;

#[cfg(target_os = "ios")]
pub struct AccountState {
    session: Arc<Session>,
    platform: MobilePlatform<Wry>,
    snapshot_directory: PathBuf,
    snapshot_previews: Arc<Mutex<HashMap<String, PendingSnapshot>>>,
}

#[cfg(target_os = "ios")]
impl AccountState {
    /// The account session, for the account-backed commands shared with Android.
    pub(crate) fn session(&self) -> &Arc<Session> {
        &self.session
    }
}

#[cfg(target_os = "ios")]
pub fn setup(app: &AppHandle<Wry>) -> Result<(), AccountError> {
    let platform = app
        .try_state::<MobilePlatform<Wry>>()
        .ok_or(AccountError::Storage)?
        .inner()
        .clone();
    let client = BackendAccountClient::new()?;
    let session = Arc::new(BackendAccountSession::new(
        client.clone(),
        IosAccountStorage(platform.clone()),
    ));
    let community = MobileCommunityState::new(client, &session)?;
    let snapshot_directory =
        std::env::temp_dir().join(format!("msime-ios-tauri-snapshots-{}", std::process::id()));
    fs::create_dir_all(&snapshot_directory).map_err(|_| AccountError::Storage)?;
    app.manage(AccountState {
        session,
        platform,
        snapshot_directory,
        snapshot_previews: Arc::new(Mutex::new(HashMap::new())),
    });
    app.manage(community);
    Ok(())
}

#[cfg(target_os = "ios")]
fn ai_command_error(
    operation: &'static str,
    error: crate::shared::mobile_ai::Error,
) -> crate::CommandError {
    crate::CommandError {
        code: match (operation, error) {
            ("models", crate::shared::mobile_ai::Error::Invalid) => "ai_models_invalid",
            ("models", crate::shared::mobile_ai::Error::Unavailable) => "ai_models_unavailable",
            ("test", crate::shared::mobile_ai::Error::Invalid) => "ai_test_invalid",
            ("test", crate::shared::mobile_ai::Error::Unavailable) => "ai_test_unavailable",
            _ => "ai_unavailable",
        },
    }
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn ai_models(
    endpoint: String,
    token: String,
) -> Result<Vec<String>, crate::CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::shared::mobile_ai::fetch_models(&endpoint, &token)
            .map_err(|error| ai_command_error("models", error))
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "ai_models_unavailable",
    })?
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn ai_test(
    endpoint: String,
    model: String,
    prompt: String,
    token: String,
    text: String,
) -> Result<String, crate::CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::shared::mobile_ai::polish(&endpoint, &model, &prompt, &token, &text)
            .map_err(|error| ai_command_error("test", error))
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "ai_test_unavailable",
    })?
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn account_status(
    state: State<'_, AccountState>,
) -> Result<StatusResponse, crate::CommandError> {
    shared_account_status(state).await
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn account_providers(
    state: State<'_, AccountState>,
) -> Result<ProvidersResponse, crate::CommandError> {
    call(state, |session| session.providers().map(providers_response)).await
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn account_request_code(
    state: State<'_, AccountState>,
    provider: String,
    target: String,
) -> Result<ChallengeResponse, crate::CommandError> {
    shared_account_request_code(state, provider, target).await
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn account_login(
    state: State<'_, AccountState>,
    challenge_id: String,
    code: String,
) -> Result<StatusResponse, crate::CommandError> {
    shared_account_login(state, challenge_id, code).await
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn account_apple_login(
    state: State<'_, AccountState>,
) -> Result<StatusResponse, crate::CommandError> {
    let session = Arc::clone(&state.session);
    let challenge = tauri::async_runtime::spawn_blocking(move || session.request_code("apple", ""))
        .await
        .map_err(|_| crate::CommandError {
            code: "account_unavailable",
        })?
        .map_err(account_command_error)?;
    let nonce = challenge.nonce.ok_or(crate::CommandError {
        code: "apple_sign_in",
    })?;
    let credential = state
        .platform
        .sign_in_with_apple(&challenge.challenge_id, &nonce)
        .await
        .map_err(|_| crate::CommandError {
            code: "apple_sign_in",
        })?;
    call(state, move |session| {
        session
            .sign_in_apple(&challenge.challenge_id, &credential)
            .map(|user| StatusResponse {
                user: Some(user.into()),
            })
    })
    .await
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn account_profile(
    state: State<'_, AccountState>,
) -> Result<ProfileResponse, crate::CommandError> {
    shared_account_profile(state).await
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn account_chat_models(
    state: State<'_, AccountState>,
) -> Result<ChatModelsResponse, crate::CommandError> {
    shared_account_chat_models(state).await
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn account_chat(
    state: State<'_, AccountState>,
    messages: Vec<AccountChatMessage>,
    model: String,
) -> Result<ChatResponse, crate::CommandError> {
    shared_account_chat(state, messages, model).await
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn account_rename(
    state: State<'_, AccountState>,
    display_name: String,
) -> Result<ProfileResponse, crate::CommandError> {
    shared_account_rename(state, display_name).await
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn account_logout(
    state: State<'_, AccountState>,
    all: bool,
) -> Result<(), crate::CommandError> {
    let previews = Arc::clone(&state.snapshot_previews);
    clear_snapshot_previews_after(&previews, shared_account_logout(state, all).await)
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn account_delete(state: State<'_, AccountState>) -> Result<(), crate::CommandError> {
    let previews = Arc::clone(&state.snapshot_previews);
    clear_snapshot_previews_after(&previews, shared_account_delete(state).await)
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn account_forget(state: State<'_, AccountState>) -> Result<(), crate::CommandError> {
    let previews = Arc::clone(&state.snapshot_previews);
    clear_snapshot_previews_after(&previews, shared_account_forget(state).await)
}

#[cfg(target_os = "ios")]
fn snapshot_bridge(action: Value) -> Result<Value, crate::CommandError> {
    let bytes = serde_json::to_vec(&action).map_err(|_| crate::CommandError {
        code: "snapshot_invalid",
    })?;
    if bytes.len() > 8 * 1024 {
        return Err(crate::CommandError {
            code: "snapshot_invalid",
        });
    }
    let response =
        msime_ios_native_ffi::dictionary_snapshot_request(&bytes).ok_or(crate::CommandError {
            code: "snapshot_unavailable",
        })?;
    let envelope: Value = serde_json::from_slice(&response).map_err(|_| crate::CommandError {
        code: "snapshot_unavailable",
    })?;
    if envelope.get("ok") == Some(&Value::Bool(true)) {
        return envelope.get("value").cloned().ok_or(crate::CommandError {
            code: "snapshot_unavailable",
        });
    }
    let code = match envelope.get("error").and_then(Value::as_str) {
        Some("snapshot_busy") => "snapshot_busy",
        Some("snapshot_conflict") => "snapshot_conflict",
        Some("snapshot_invalid") => "snapshot_invalid",
        _ => "snapshot_unavailable",
    };
    Err(crate::CommandError { code })
}

#[cfg(target_os = "ios")]
fn snapshot_metadata(value: Value) -> Result<SnapshotMetadata, crate::CommandError> {
    serde_json::from_value(value).map_err(|_| crate::CommandError {
        code: "snapshot_invalid",
    })
}

#[cfg(target_os = "ios")]
async fn dictionary_snapshot_preview(
    state: State<'_, AccountState>,
) -> Result<Value, crate::CommandError> {
    let session = Arc::clone(&state.session);
    let directory = state.snapshot_directory.clone();
    let previews = Arc::clone(&state.snapshot_previews);
    let token = uuid::Uuid::new_v4().to_string();
    let file_token = token.clone();
    let (account_id, path, metadata) = tauri::async_runtime::spawn_blocking(move || {
        fs::create_dir_all(&directory).map_err(|_| snapshot_command_error())?;
        let profile = session.profile().map_err(account_command_error)?;
        let path = directory.join(format!("download-{file_token}.ndjson"));
        if let Err(error) = session.dictionary_snapshot_to_file(&path) {
            let _ = fs::remove_file(&path);
            return Err(crate::CommandError { code: error.code() });
        }
        let inspected = snapshot_bridge(serde_json::json!({
            "operation": "inspect",
            "path": path.to_string_lossy(),
        }));
        let metadata = match inspected.and_then(snapshot_metadata) {
            Ok(value) => value,
            Err(error) => {
                let _ = fs::remove_file(&path);
                return Err(error);
            }
        };
        Ok((profile.user.id, path, metadata))
    })
    .await
    .map_err(|_| snapshot_command_error())??;
    let old = replace_pending_snapshot(
        &previews,
        token.clone(),
        PendingSnapshot {
            account_id,
            path,
            metadata: metadata.clone(),
        },
    )?;
    for path in old {
        let _ = fs::remove_file(path);
    }
    Ok(serde_json::json!({
        "previewToken": token,
        "snapshot": metadata,
    }))
}

#[cfg(target_os = "ios")]
async fn dictionary_snapshot_enqueue(
    state: State<'_, AccountState>,
    token: String,
) -> Result<Value, crate::CommandError> {
    let parsed = parse_snapshot_token(&token)?;
    let pending = {
        let mut previews = state
            .snapshot_previews
            .lock()
            .map_err(|_| snapshot_command_error())?;
        previews
            .remove(&parsed.to_string())
            .ok_or(crate::CommandError {
                code: "snapshot_invalid",
            })?
    };
    let session = Arc::clone(&state.session);
    let path = pending.path.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            validate_pending_snapshot(&session, &pending).map_err(account_command_error)?;
            let state = snapshot_bridge(serde_json::json!({ "operation": "state" }))?;
            let expected = state
                .get("localVersion")
                .and_then(Value::as_str)
                .ok_or(crate::CommandError {
                    code: "snapshot_conflict",
                })?
                .to_owned();
            snapshot_bridge(serde_json::json!({
                "operation": "enqueue",
                "path": path.to_string_lossy(),
                "accountId": pending.account_id,
                "cloudRevision": pending.metadata.cloud_revision,
                "expectedLocalVersion": expected,
                "fileSha256": pending.metadata.file_sha256,
            }))
        })();
        let _ = fs::remove_file(path);
        result
    })
    .await
    .map_err(|_| snapshot_command_error())??;
    snapshot_response_without_account(result)
}

#[cfg(target_os = "ios")]
async fn dictionary_snapshot_export(
    state: State<'_, AccountState>,
) -> Result<Value, crate::CommandError> {
    let session = Arc::clone(&state.session);
    let directory = state.snapshot_directory.clone();
    let token = uuid::Uuid::new_v4().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        fs::create_dir_all(&directory).map_err(|_| snapshot_command_error())?;
        let path = directory.join(format!("export-{token}.ndjson"));
        let result = (|| {
            session
                .dictionary_snapshot_to_file(&path)
                .map_err(account_command_error)?;
            let metadata = snapshot_metadata(snapshot_bridge(serde_json::json!({
                "operation": "inspect",
                "path": path.to_string_lossy(),
            }))?)?;
            let text = fs::read_to_string(&path).map_err(|_| snapshot_command_error())?;
            Ok(serde_json::json!({
                "text": text,
                "filename": "msime-dictionary-snapshot.ndjson",
                "snapshot": metadata,
            }))
        })();
        let _ = fs::remove_file(path);
        result
    })
    .await
    .map_err(|_| snapshot_command_error())?
}

#[cfg(target_os = "ios")]
async fn dictionary_snapshot_restore_preview(
    state: State<'_, AccountState>,
    text: String,
) -> Result<Value, crate::CommandError> {
    let session = Arc::clone(&state.session);
    let directory = state.snapshot_directory.clone();
    let token = uuid::Uuid::new_v4().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        fs::create_dir_all(&directory).map_err(|_| snapshot_command_error())?;
        let path = directory.join(format!("restore-{token}.ndjson"));
        let result = (|| {
            fs::write(&path, text.as_bytes()).map_err(|_| snapshot_command_error())?;
            let metadata = snapshot_metadata(snapshot_bridge(serde_json::json!({
                "operation": "inspect",
                "path": path.to_string_lossy(),
            }))?)?;
            let page = session
                .dictionary_catalog(DictionaryKind::Quick, "", 0, "pinyin", "xiaohe")
                .map_err(account_command_error)?;
            Ok(serde_json::json!({
                "snapshot": metadata,
                "expectedRevision": page.revision,
            }))
        })();
        let _ = fs::remove_file(path);
        result
    })
    .await
    .map_err(|_| snapshot_command_error())?
}

#[cfg(target_os = "ios")]
async fn dictionary_snapshot_restore(
    state: State<'_, AccountState>,
    text: String,
    expected_sha256: String,
    revision: i64,
) -> Result<Value, crate::CommandError> {
    let session = Arc::clone(&state.session);
    let directory = state.snapshot_directory.clone();
    let token = uuid::Uuid::new_v4().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        fs::create_dir_all(&directory).map_err(|_| snapshot_command_error())?;
        let path = directory.join(format!("restore-{token}.ndjson"));
        let result = (|| {
            fs::write(&path, text.as_bytes()).map_err(|_| snapshot_command_error())?;
            let metadata = snapshot_metadata(snapshot_bridge(serde_json::json!({
                "operation": "inspect",
                "path": path.to_string_lossy(),
            }))?)?;
            if metadata.sha256 != expected_sha256 {
                return Err(crate::CommandError {
                    code: "snapshot_invalid",
                });
            }
            let result = session
                .restore_dictionary_snapshot(text.as_bytes(), revision)
                .map_err(account_command_error)?;
            Ok(serde_json::json!({
                "revision": result.revision,
                "reset": result.reset,
            }))
        })();
        let _ = fs::remove_file(path);
        result
    })
    .await
    .map_err(|_| snapshot_command_error())?
}

#[cfg(target_os = "ios")]
async fn dictionary_snapshot_status(
    _state: State<'_, AccountState>,
) -> Result<Value, crate::CommandError> {
    snapshot_response_without_account(snapshot_bridge(serde_json::json!({
        "operation": "state",
    }))?)
}

#[cfg(target_os = "ios")]
async fn dictionary_snapshot_cancel(
    state: State<'_, AccountState>,
) -> Result<Value, crate::CommandError> {
    let session = Arc::clone(&state.session);
    let previews = Arc::clone(&state.snapshot_previews);
    let account_id = tauri::async_runtime::spawn_blocking(move || {
        session
            .profile()
            .map(|profile| profile.user.id)
            .map_err(account_command_error)
    })
    .await
    .map_err(|_| snapshot_command_error())??;
    clear_snapshot_previews(&previews);
    snapshot_response_without_account(snapshot_bridge(serde_json::json!({
        "operation": "cancel",
        "accountId": account_id,
    }))?)
}

#[cfg(target_os = "ios")]
pub async fn cloud_dictionary_request(
    state: State<'_, AccountState>,
    request: msime_host_api::cloud_dictionary::CloudDictionaryRequest,
) -> Result<Value, crate::CommandError> {
    use msime_host_api::cloud_dictionary::CloudDictionaryRequest;
    if let Some(result) = cloud_dictionary_account_request(&state, &request).await {
        return result;
    }
    match request {
        CloudDictionaryRequest::SnapshotPreview => dictionary_snapshot_preview(state).await,
        CloudDictionaryRequest::SnapshotExport => dictionary_snapshot_export(state).await,
        CloudDictionaryRequest::SnapshotRestorePreview { text } => {
            dictionary_snapshot_restore_preview(state, text).await
        }
        CloudDictionaryRequest::SnapshotRestore {
            text,
            expected_sha256,
            revision,
        } => dictionary_snapshot_restore(state, text, expected_sha256, revision).await,
        CloudDictionaryRequest::SnapshotRestoreNative { .. } => Err(crate::CommandError {
            code: "snapshot_unavailable",
        }),
        CloudDictionaryRequest::SnapshotRestoreCancel => Err(crate::CommandError {
            code: "snapshot_unavailable",
        }),
        CloudDictionaryRequest::SnapshotEnqueue { token } => {
            dictionary_snapshot_enqueue(state, token).await
        }
        CloudDictionaryRequest::SnapshotStatus => dictionary_snapshot_status(state).await,
        CloudDictionaryRequest::SnapshotCancel => dictionary_snapshot_cancel(state).await,
        _ => unreachable!("account cloud dictionary request was handled above"),
    }
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn account_preferences_schema(
    state: State<'_, AccountState>,
) -> Result<PreferenceSchemaResponse, crate::CommandError> {
    shared_account_preferences_schema(state).await
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn account_preferences_load(
    state: State<'_, AccountState>,
) -> Result<AccountPreferences, crate::CommandError> {
    shared_account_preferences_load(state).await
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn account_preferences_upload(
    state: State<'_, AccountState>,
    store: State<'_, Arc<PreferencesStore>>,
) -> Result<AccountPreferences, crate::CommandError> {
    let session = Arc::clone(&state.session);
    let platform = state.platform.clone();
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let schema = session.preference_schema()?;
        let cloud = session.preferences()?;
        let native = platform
            .load_keyboard_preferences()
            .map_err(|_| AccountError::Storage)?;
        let local = store.load().map_err(|_| AccountError::Storage)?;
        let values = account_preferences::local_account_preferences(
            &native,
            &local.preferences,
            &local.preferences.custom_touch_keyboard_skin,
        )?
        .into_iter()
        .filter(|(key, _)| schema.fields.contains_key(key))
        .collect::<BTreeMap<_, _>>();
        if values.is_empty() {
            return Err(AccountError::Unavailable);
        }
        let merged = merge_account_preferences(&cloud, &values, &schema)?;
        session.put_preferences(&merged)
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "account_unavailable",
    })?
    .map_err(account_command_error)
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn account_preferences_apply(
    state: State<'_, AccountState>,
    store: State<'_, Arc<PreferencesStore>>,
    user_id: String,
    preferences: AccountPreferences,
) -> Result<(), crate::CommandError> {
    let session = Arc::clone(&state.session);
    let platform = state.platform.clone();
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        session.credentials(None, Some(&user_id))?;
        let plan = account_preferences::IosPreferencePlan::from_cloud(&preferences)?;
        let local = store.load().map_err(|_| AccountError::Storage)?;
        let previous_native = platform
            .load_keyboard_preferences()
            .map_err(|_| AccountError::Storage)?;
        let requested = plan.requested_native(&previous_native)?;
        let saved_native = platform
            .save_keyboard_preferences(&requested)
            .map_err(|_| AccountError::Storage)?;
        let mut next = local.preferences.clone();
        if let Err(error) = plan.apply_shared(&saved_native, &mut next) {
            let _ = platform.save_keyboard_preferences(&previous_native);
            return Err(error);
        }
        if store.save(local.revision, next).is_err() {
            let _ = platform.save_keyboard_preferences(&previous_native);
            return Err(AccountError::Storage);
        }
        Ok::<(), AccountError>(())
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "account_unavailable",
    })?
    .map_err(account_command_error)
}

#[cfg(target_os = "ios")]
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileKeyboardFeedback {
    pub sound_enabled: bool,
    pub haptics_enabled: bool,
    pub haptic_strength: String,
    #[serde(default = "default_english_suggestions")]
    pub english_suggestions: bool,
    #[serde(default)]
    pub candidate_palette_follows_desktop: bool,
    #[serde(default)]
    pub inline_preedit: bool,
    /// Reported by the device, never saved: the page hides the vibration controls where it is false.
    #[serde(default = "default_haptics_available")]
    pub haptics_available: bool,
    /// iPad only; absent on a phone, so the page leaves the switch out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tablet_full_keys: Option<bool>,
}

#[cfg(target_os = "ios")]
fn default_english_suggestions() -> bool {
    true
}

#[cfg(target_os = "ios")]
fn default_haptics_available() -> bool {
    true
}

#[cfg(target_os = "ios")]
#[derive(serde::Deserialize)]
pub struct MobileKeyboardFeedbackRequest {
    pub settings: MobileKeyboardFeedback,
}

#[cfg(target_os = "ios")]
#[derive(serde::Deserialize)]
pub struct MobileKeyboardFeedbackPreviewRequest {
    pub strength: String,
}

#[cfg(target_os = "ios")]
fn keyboard_feedback(native: &IosKeyboardPreferences) -> MobileKeyboardFeedback {
    MobileKeyboardFeedback {
        sound_enabled: native.sound_enabled,
        haptics_enabled: native.haptics_enabled,
        haptic_strength: native.haptic_strength.clone(),
        english_suggestions: native.english_suggestions,
        candidate_palette_follows_desktop: native.candidate_palette_follows_desktop,
        inline_preedit: native.inline_preedit,
        haptics_available: native.haptics_available,
        tablet_full_keys: native.tablet_full_keys,
    }
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn mobile_keyboard_feedback_load(
    state: State<'_, AccountState>,
) -> Result<MobileKeyboardFeedback, crate::CommandError> {
    let platform = state.platform.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let native = platform
            .load_keyboard_preferences()
            .map_err(|_| crate::CommandError {
                code: "feedback_storage",
            })?;
        Ok(keyboard_feedback(&native))
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "feedback_storage",
    })?
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn mobile_keyboard_feedback_save(
    state: State<'_, AccountState>,
    request: MobileKeyboardFeedbackRequest,
) -> Result<MobileKeyboardFeedback, crate::CommandError> {
    if !valid_mobile_haptic_strength(&request.settings.haptic_strength) {
        return Err(crate::CommandError {
            code: "invalid_feedback",
        });
    }
    let platform = state.platform.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut native = platform
            .load_keyboard_preferences()
            .map_err(|_| crate::CommandError {
                code: "feedback_storage",
            })?;
        native.sound_enabled = request.settings.sound_enabled;
        native.haptics_enabled = request.settings.haptics_enabled;
        native.haptic_strength = request.settings.haptic_strength.clone();
        native.english_suggestions = request.settings.english_suggestions;
        native.candidate_palette_follows_desktop =
            request.settings.candidate_palette_follows_desktop;
        native.inline_preedit = request.settings.inline_preedit;
        if request.settings.tablet_full_keys.is_some() {
            native.tablet_full_keys = request.settings.tablet_full_keys;
        }
        let saved =
            platform
                .save_keyboard_preferences(&native)
                .map_err(|_| crate::CommandError {
                    code: "feedback_storage",
                })?;
        Ok(keyboard_feedback(&saved))
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "feedback_storage",
    })?
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub async fn mobile_keyboard_feedback_preview(
    state: State<'_, AccountState>,
    request: MobileKeyboardFeedbackPreviewRequest,
) -> Result<(), crate::CommandError> {
    if !valid_mobile_haptic_strength(&request.strength) {
        return Err(crate::CommandError {
            code: "invalid_feedback",
        });
    }
    state
        .platform
        .preview_keyboard_haptics(&request.strength)
        .map_err(|_| crate::CommandError {
            code: "feedback_preview",
        })
}

#[cfg(test)]
mod tests {
    use super::{ChatModelsResponse, ChatResponse, PreferenceSchemaResponse};
    use msime_client_core::account::{
        AccountChatModel, AccountChatModels, AccountPreferenceField, AccountPreferenceSchema,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn chat_responses_match_the_shared_webview_contract() {
        let models = ChatModelsResponse::from(AccountChatModels {
            data: vec![AccountChatModel {
                id: "synthetic-model".into(),
            }],
            default_model: "synthetic-model".into(),
        });
        assert_eq!(
            serde_json::to_value(models).unwrap(),
            json!({"data":[{"id":"synthetic-model"}],"defaultModel":"synthetic-model"})
        );

        let response = ChatResponse {
            content: "synthetic-response".into(),
        };
        assert_eq!(
            serde_json::to_value(response).unwrap(),
            json!({"content":"synthetic-response"})
        );
    }

    #[test]
    fn preference_schema_response_matches_the_shared_webview_contract() {
        let response = PreferenceSchemaResponse::from(AccountPreferenceSchema {
            fields: BTreeMap::from([(
                "platform.ios.nine_key".into(),
                AccountPreferenceField {
                    value_type: "boolean".into(),
                },
            )]),
            maximum_bytes: 65_536,
            update_mode: "replace".into(),
            revision_required: true,
        });
        assert_eq!(
            serde_json::to_value(response).unwrap(),
            json!({
                "fields":{"platform.ios.nine_key":{"type":"boolean"}},
                "maximumBytes":65536,
                "updateMode":"replace",
                "revisionRequired":true
            })
        );
    }
}
