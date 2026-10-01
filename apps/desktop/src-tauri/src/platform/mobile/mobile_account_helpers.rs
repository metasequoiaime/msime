pub(crate) use crate::platform::account_helpers::{
    account_command_error, account_value, call_session, cleanup_stale_snapshot_previews,
    snapshot_text_within_limit,
};
pub(crate) use crate::platform::mobile::mobile_account_preferences::valid_mobile_haptic_strength;
use crate::shared::account_dto::{
    ChallengeResponse, ChatModelsResponse, ChatResponse, PreferenceSchemaResponse, ProfileResponse,
    StatusResponse,
};
use msime_client_core::account::AccountError;
use msime_client_core::account::AccountPreferences;
use msime_client_core::account::{AccountCandidateQuery, AccountChatMessage};
use msime_client_core::cloud::dictionary::DictionaryKind;
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

pub(crate) fn replace_pending_snapshot(
    previews: &std::sync::Arc<std::sync::Mutex<HashMap<String, PendingSnapshot>>>,
    token: String,
    snapshot: PendingSnapshot,
) -> Result<Vec<PathBuf>, crate::CommandError> {
    let mut pending = previews.lock().map_err(|_| snapshot_command_error())?;
    let old = pending
        .drain()
        .map(|(_, item)| item.path)
        .collect::<Vec<_>>();
    pending.insert(token, snapshot);
    Ok(old)
}

pub(crate) fn validate_pending_snapshot(
    session: &crate::platform::mobile::MobileSession,
    pending: &PendingSnapshot,
) -> Result<(), AccountError> {
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
    previews: &std::sync::Arc<std::sync::Mutex<HashMap<String, PendingSnapshot>>>,
    result: Result<T, crate::CommandError>,
) -> Result<T, crate::CommandError> {
    if result.is_ok() {
        clear_snapshot_previews(previews);
    }
    result
}

pub(crate) fn parse_cloud_dictionary_request(
    action: &Value,
) -> Result<msime_host_api::cloud_dictionary::CloudDictionaryRequest, crate::CommandError> {
    let request = serde_json::from_value(action.clone()).map_err(|_| crate::CommandError {
        code: "invalid_cloud_dictionary",
    })?;
    msime_host_api::cloud_dictionary::validate_cloud_request(&request).map_err(|_| {
        crate::CommandError {
            code: "invalid_cloud_dictionary",
        }
    })?;
    Ok(request)
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

/// Handle the account-backed cloud dictionary operations shared by Android and iOS.
///
/// Snapshot operations are platform-owned because Android stages files through its plugin while
/// iOS sends the corresponding request through its native bridge. Returning `None` leaves those
/// branches to the caller.
pub(crate) async fn cloud_dictionary_account_request(
    state: &tauri::State<'_, crate::platform::mobile::MobileAccountState>,
    request: &msime_host_api::cloud_dictionary::CloudDictionaryRequest,
) -> Option<Result<Value, crate::CommandError>> {
    use msime_host_api::cloud_dictionary::CloudDictionaryRequest;

    match request {
        CloudDictionaryRequest::List {
            kind,
            offset,
            search,
        } => {
            let kind = match dictionary_kind(kind) {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error)),
            };
            Some(
                call_session(state.session(), {
                    let search = search.clone();
                    let offset = *offset;
                    move |session| {
                        session
                            .dictionary(kind, &search, offset)
                            .and_then(|page| account_value(page))
                    }
                })
                .await,
            )
        }
        CloudDictionaryRequest::Catalog {
            kind,
            code,
            offset,
            scheme,
            profile,
        } => {
            let kind = match dictionary_kind(kind) {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error)),
            };
            let code = code.clone();
            let offset = *offset;
            let scheme = scheme.clone();
            let profile = profile.clone();
            Some(
                call_session(state.session(), move |session| {
                    session
                        .dictionary_catalog(kind, &code, offset, &scheme, &profile)
                        .and_then(|page| {
                            account_value(serde_json::json!({
                                "catalog_entries": page.entries,
                                "has_more": page.has_more,
                                "offset": page.offset,
                                "revision": page.revision,
                                "normalized": page.normalized,
                            }))
                        })
                })
                .await,
            )
        }
        CloudDictionaryRequest::Changes { after, limit } => {
            let after = *after;
            let limit = *limit;
            Some(
                call_session(state.session(), move |session| {
                    session
                        .dictionary_changes(after, limit)
                        .and_then(|page| account_value(page))
                })
                .await,
            )
        }
        CloudDictionaryRequest::Add {
            kind,
            code,
            word,
            weight,
        } => {
            let kind = match dictionary_kind(kind) {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error)),
            };
            let code = code.clone();
            let word = word.clone();
            let weight = *weight;
            Some(
                call_session(state.session(), move |session| {
                    session
                        .add_dictionary(kind, &code, &word, weight)
                        .and_then(|change| account_value(change))
                })
                .await,
            )
        }
        CloudDictionaryRequest::Update {
            kind,
            id,
            code,
            word,
            weight,
            revision,
        } => {
            let kind = match dictionary_kind(kind) {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error)),
            };
            let id = id.clone();
            let code = code.clone();
            let word = word.clone();
            let weight = *weight;
            let revision = *revision;
            Some(
                call_session(state.session(), move |session| {
                    session
                        .update_dictionary(kind, &id, &code, &word, weight, revision)
                        .and_then(|change| account_value(change))
                })
                .await,
            )
        }
        CloudDictionaryRequest::EditCatalog {
            kind,
            code,
            word,
            revision,
            replacement,
        } => {
            let kind = match dictionary_kind(kind) {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error)),
            };
            let code = code.clone();
            let word = word.clone();
            let revision = *revision;
            let replacement = replacement
                .as_ref()
                .map(|value| (value.code.clone(), value.word.clone(), value.weight));
            Some(
                call_session(state.session(), move |session| {
                    let replacement = replacement
                        .as_ref()
                        .map(|(code, word, weight)| (code.as_str(), word.as_str(), *weight));
                    session
                        .edit_dictionary_catalog(kind, &code, &word, revision, replacement)
                        .and_then(|change| account_value(change))
                })
                .await,
            )
        }
        CloudDictionaryRequest::Candidates {
            text,
            kind,
            scheme,
            profile,
            limit,
        } => {
            let query = AccountCandidateQuery {
                text: text.clone(),
                kind: kind.clone(),
                scheme: scheme.clone(),
                profile: profile.clone(),
                limit: *limit,
            };
            Some(
                call_session(state.session(), move |session| {
                    session
                        .personal_candidates(&query)
                        .and_then(|result| account_value(result))
                })
                .await,
            )
        }
        CloudDictionaryRequest::Rank {
            text,
            kind,
            scheme,
            profile,
            limit,
            code,
            word,
            revision,
            mode,
            linear_step,
            trigger_count,
            force_top,
        } => {
            let query = AccountCandidateQuery {
                text: text.clone(),
                kind: kind.clone(),
                scheme: scheme.clone(),
                profile: profile.clone(),
                limit: *limit,
            };
            let code = code.clone();
            let word = word.clone();
            let mode = mode.clone();
            let revision = *revision;
            let linear_step = *linear_step;
            let trigger_count = *trigger_count;
            let force_top = *force_top;
            Some(
                call_session(state.session(), move |session| {
                    session
                        .rank_candidate(
                            &query,
                            &code,
                            &word,
                            revision,
                            &mode,
                            linear_step,
                            trigger_count,
                            force_top,
                        )
                        .map(|result| {
                            serde_json::json!({
                                "revision": result.revision,
                                "changed": result.changed,
                                "selection_count": result.selection.count,
                            })
                        })
                })
                .await,
            )
        }
        CloudDictionaryRequest::RemoveCandidate {
            text,
            kind,
            scheme,
            profile,
            limit,
            code,
            word,
            revision,
        } => {
            let query = AccountCandidateQuery {
                text: text.clone(),
                kind: kind.clone(),
                scheme: scheme.clone(),
                profile: profile.clone(),
                limit: *limit,
            };
            let code = code.clone();
            let word = word.clone();
            let revision = *revision;
            Some(
                call_session(state.session(), move |session| {
                    session
                        .remove_candidate(&query, &code, &word, revision)
                        .and_then(|result| account_value(result))
                })
                .await,
            )
        }
        CloudDictionaryRequest::FixedPositions { context, offset } => {
            let context = context.clone();
            let offset = *offset;
            Some(
                call_session(state.session(), move |session| {
                    session
                        .fixed_positions(&context, offset)
                        .and_then(|result| account_value(result))
                })
                .await,
            )
        }
        CloudDictionaryRequest::SetFixedPosition {
            context,
            code,
            word,
            position,
            revision,
        } => {
            let context = context.clone();
            let code = code.clone();
            let word = word.clone();
            let position = *position;
            let revision = *revision;
            Some(
                call_session(state.session(), move |session| {
                    session
                        .set_fixed_position(&context, &code, &word, position, revision)
                        .and_then(|result| account_value(result))
                })
                .await,
            )
        }
        CloudDictionaryRequest::Delete { kind, id, revision } => {
            let kind = match dictionary_kind(kind) {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error)),
            };
            let id = id.clone();
            let revision = *revision;
            Some(
                call_session(state.session(), move |session| {
                    session
                        .delete_dictionary(kind, &id, revision)
                        .and_then(|change| account_value(change))
                })
                .await,
            )
        }
        CloudDictionaryRequest::Import { kind, format, text } => {
            let kind = match dictionary_kind(kind) {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error)),
            };
            let format = format.clone();
            let text = text.clone();
            Some(
                call_session(state.session(), move |session| {
                    session
                        .import_dictionary(kind, &format, &text)
                        .and_then(|result| account_value(result))
                })
                .await,
            )
        }
        CloudDictionaryRequest::Export { kind, format } => {
            let kind = match dictionary_kind(kind) {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error)),
            };
            let format = format.clone();
            Some(
                call_session(state.session(), move |session| {
                    session.export_dictionary(kind, &format).map(|result| {
                        serde_json::json!({
                            "text": result.text,
                            "filename": result.filename,
                        })
                    })
                })
                .await,
            )
        }
        CloudDictionaryRequest::SnapshotPreview
        | CloudDictionaryRequest::SnapshotExport
        | CloudDictionaryRequest::SnapshotRestorePreview { .. }
        | CloudDictionaryRequest::SnapshotRestore { .. }
        | CloudDictionaryRequest::SnapshotRestoreNative { .. }
        | CloudDictionaryRequest::SnapshotRestoreCancel
        | CloudDictionaryRequest::SnapshotEnqueue { .. }
        | CloudDictionaryRequest::SnapshotStatus
        | CloudDictionaryRequest::SnapshotCancel => None,
    }
}

pub(crate) fn snapshot_command_error() -> crate::CommandError {
    crate::CommandError {
        code: "snapshot_unavailable",
    }
}

pub(crate) fn parse_snapshot_token(value: &str) -> Result<uuid::Uuid, crate::CommandError> {
    uuid::Uuid::parse_str(value).map_err(|_| crate::CommandError {
        code: "snapshot_invalid",
    })
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

pub(crate) async fn account_login(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
    challenge_id: String,
    code: String,
) -> Result<StatusResponse, crate::CommandError> {
    call_session(state.session(), move |session| {
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

pub(crate) async fn account_logout(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
    all: bool,
) -> Result<(), crate::CommandError> {
    call_session(state.session(), move |session| session.logout(all)).await
}

pub(crate) async fn account_delete(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
) -> Result<(), crate::CommandError> {
    call_session(state.session(), |session| session.delete_account()).await
}

pub(crate) async fn account_forget(
    state: tauri::State<'_, crate::platform::mobile::MobileAccountState>,
) -> Result<(), crate::CommandError> {
    call_session(state.session(), |session| session.forget()).await
}
