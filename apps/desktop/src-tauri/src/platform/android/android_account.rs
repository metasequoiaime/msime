use crate::platform::mobile::mobile_account_helpers::{
    account_chat as shared_account_chat, account_chat_models as shared_account_chat_models,
    account_command_error, account_login as shared_account_login,
    account_preferences_load as shared_account_preferences_load,
    account_preferences_schema as shared_account_preferences_schema,
    account_profile as shared_account_profile, account_rename as shared_account_rename,
    account_request_code as shared_account_request_code, account_status as shared_account_status,
    call_session, cleanup_stale_snapshot_previews, clear_snapshot_previews,
    clear_snapshot_previews_after, cloud_dictionary_account_request, prepare_snapshot_directory,
    publish_snapshot_preview, read_snapshot_file, remove_snapshot_file, snapshot_command_error,
    snapshot_response_without_account, snapshot_text_within_limit, take_pending_snapshot,
    valid_mobile_haptic_strength, validate_pending_snapshot, write_snapshot_file, PendingSnapshot,
    SnapshotMetadata,
};
use crate::platform::mobile::mobile_community::MobileCommunityState;
use crate::shared::account_dto::{
    providers_response_without_apple, ChallengeResponse, ChatModelsResponse, ChatResponse,
    PreferenceSchemaResponse, ProfileResponse, ProvidersResponse, StatusResponse,
};
use msime_client_core::account::settings_sync::{
    apply_android_settings, export_android_settings, needs_host_feedback, HostKeyboardFeedback,
};
use msime_client_core::account::{
    merge_account_preferences, AccountChatMessage, AccountError, AccountPreferenceSchema,
    AccountPreferenceValue, AccountPreferences, AccountSessionStorage, BackendAccountClient,
    BackendAccountSession, SavedAccountSession,
};
use msime_client_core::cloud::dictionary::DictionaryKind;
use msime_client_core::cloud::snapshot_validation::{
    has_keys as snapshot_has_keys, parse_strict_object, required_integer as snapshot_integer,
    required_text as snapshot_text, valid_timestamp as snapshot_timestamp,
};
use msime_client_core::edition::{
    filter_downloaded_account_settings, filter_uploaded_account_settings,
};
use msime_client_core::preferences::{Preferences, PreferencesSnapshot, PreferencesStore};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager, Runtime, State, Wry};
use uuid::Uuid;

const MAX_SECURE_SESSION_BYTES: usize = 16 * 1024;

fn valid_secure_session(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_SECURE_SESSION_BYTES
}

#[derive(Deserialize)]
struct LoadResponse {
    value: Option<String>,
}

#[derive(Serialize)]
struct SaveRequest<'a> {
    value: &'a str,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FeedbackSettings {
    pub(crate) sound_enabled: bool,
    pub(crate) haptics_enabled: bool,
    pub(crate) haptic_strength: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MobileKeyboardFeedbackRequest {
    pub(crate) settings: FeedbackSettings,
}

#[derive(Deserialize)]
pub(crate) struct MobileKeyboardFeedbackPreviewRequest {
    pub(crate) strength: String,
}

#[derive(Serialize)]
struct AiModelsRequest<'a> {
    endpoint: &'a str,
    token: &'a str,
}

#[derive(Serialize)]
struct AiTestRequest<'a> {
    endpoint: &'a str,
    model: &'a str,
    prompt: &'a str,
    token: &'a str,
    text: &'a str,
}

#[derive(Clone)]
pub(crate) struct AndroidAccountStorage<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> AccountSessionStorage for AndroidAccountStorage<R> {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        let response = self
            .0
            .run_mobile_plugin::<LoadResponse>("loadSession", ())
            .map_err(|_| AccountError::Storage)?;
        response
            .value
            .map(|value| {
                if !valid_secure_session(&value) {
                    return Err(AccountError::Storage);
                }
                serde_json::from_str(&value).map_err(|_| AccountError::Storage)
            })
            .transpose()
    }

    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        let value = serde_json::to_string(session).map_err(|_| AccountError::Storage)?;
        if !valid_secure_session(&value) {
            return Err(AccountError::Storage);
        }
        self.0
            .run_mobile_plugin::<()>("saveSession", SaveRequest { value: &value })
            .map_err(|_| AccountError::Storage)
    }

    fn clear(&self) -> Result<(), AccountError> {
        self.0
            .run_mobile_plugin::<()>("clearSession", ())
            .map_err(|_| AccountError::Storage)
    }
}

type Session = BackendAccountSession<BackendAccountClient, AndroidAccountStorage<Wry>>;

pub struct AccountState {
    session: Arc<Session>,
    pub(crate) platform: PluginHandle<Wry>,
    snapshot_directory: PathBuf,
    snapshot_previews: Arc<Mutex<HashMap<String, PendingSnapshot>>>,
    feedback: PluginHandle<Wry>,
}

impl AccountState {
    /// The account session, for the account-backed commands shared with iOS.
    pub(crate) fn session(&self) -> &Arc<Session> {
        &self.session
    }
}

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("account-storage")
        .setup(|app, api| {
            let handle = api.register_android_plugin("app.msime.android", "AccountPlugin")?;
            let platform = handle.clone();
            let feedback = handle.clone();
            let client = BackendAccountClient::new()?;
            let session = Arc::new(BackendAccountSession::new(
                client.clone(),
                AndroidAccountStorage(handle),
            ));
            let community = MobileCommunityState::new(client, &session)?;
            let snapshot_directory = app
                .path()
                .app_data_dir()?
                .join("files/bootstrap/state/dictionary-snapshots");
            prepare_snapshot_directory(&snapshot_directory)?;
            cleanup_stale_snapshot_previews(&snapshot_directory)?;
            app.manage(AccountState {
                session,
                platform,
                snapshot_directory,
                snapshot_previews: Arc::new(Mutex::new(HashMap::new())),
                feedback,
            });
            app.manage(community);
            Ok(())
        })
        .build()
}

fn inspect_snapshot_record(
    map: &serde_json::Map<String, Value>,
    revision: i64,
    entry_keys: &mut HashMap<(String, String, String), i64>,
    entry_ids: &mut HashSet<String>,
    overlays: &mut HashMap<(String, String, String), (bool, bool, i64)>,
    positions: &mut HashSet<(String, String, String)>,
    position_slots: &mut HashSet<(String, i64)>,
    selections: &mut HashSet<(String, String, String)>,
) -> Result<u8, AccountError> {
    let kind = map
        .get("type")
        .and_then(Value::as_str)
        .ok_or(AccountError::Invalid)?;
    let data = map
        .get("data")
        .and_then(Value::as_object)
        .ok_or(AccountError::Invalid)?;
    let code = snapshot_text(data, "code", 512, AccountError::Invalid)?.to_owned();
    let word = snapshot_text(data, "word", 2048, AccountError::Invalid)?.to_owned();
    match kind {
        "entry" | "overlay" => {
            let outer_keys_valid = if kind == "overlay" {
                snapshot_has_keys(map, &["type", "data", "deleted"])
            } else {
                snapshot_has_keys(map, &["type", "data"])
            };
            if !outer_keys_valid {
                return Err(AccountError::Invalid);
            }
            let deleted = if kind == "overlay" {
                map.get("deleted")
                    .and_then(Value::as_bool)
                    .ok_or(AccountError::Invalid)?
            } else {
                false
            };
            let expected = [
                "id",
                "kind",
                "code",
                "word",
                "weight",
                "revision",
                "updated_at",
            ];
            let mut data_keys = data.keys().map(String::as_str).collect::<HashSet<_>>();
            if data_keys.remove("user_inserted") {
                let user_inserted = data
                    .get("user_inserted")
                    .and_then(Value::as_bool)
                    .ok_or(AccountError::Invalid)?;
                if kind == "entry" && !user_inserted {
                    return Err(AccountError::Invalid);
                }
            }
            if data_keys.len() != expected.len()
                || expected.iter().any(|key| !data_keys.contains(key))
            {
                return Err(AccountError::Invalid);
            }
            let dictionary_kind = data
                .get("kind")
                .and_then(Value::as_str)
                .filter(|value| {
                    matches!(*value, "pinyin" | "wubi" | "wubi98" | "english" | "quick")
                })
                .ok_or(AccountError::Invalid)?
                .to_owned();
            let id = data
                .get("id")
                .and_then(Value::as_str)
                .ok_or(AccountError::Invalid)?
                .to_owned();
            if kind == "entry"
                && (id.is_empty()
                    || id.len() > 128
                    || id
                        .bytes()
                        .any(|byte| matches!(byte, 0 | b'\t' | b'\n' | b'\r')))
            {
                return Err(AccountError::Invalid);
            }
            let weight = snapshot_integer(data, "weight", AccountError::Invalid)?;
            let record_revision = snapshot_integer(data, "revision", AccountError::Invalid)?;
            if !(0..=100_000_000).contains(&weight)
                || (weight == 0 && !deleted)
                || !(1..=revision).contains(&record_revision)
                || !snapshot_timestamp(
                    data.get("updated_at")
                        .and_then(Value::as_str)
                        .ok_or(AccountError::Invalid)?,
                )
            {
                return Err(AccountError::Invalid);
            }
            let identity = (dictionary_kind, code, word);
            if kind == "entry" {
                if entry_keys.insert(identity, weight).is_some() || !entry_ids.insert(id) {
                    return Err(AccountError::Invalid);
                }
            } else {
                let user_inserted = data
                    .get("user_inserted")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                if overlays
                    .insert(identity, (deleted, user_inserted, weight))
                    .is_some()
                {
                    return Err(AccountError::Invalid);
                }
            }
            Ok(if kind == "entry" { 1 } else { 2 })
        }
        "position" | "selection" => {
            if !snapshot_has_keys(map, &["type", "data"]) {
                return Err(AccountError::Invalid);
            }
            let value_key = if kind == "position" {
                "position"
            } else {
                "count"
            };
            if !snapshot_has_keys(data, &["context", "code", "word", value_key]) {
                return Err(AccountError::Invalid);
            }
            let context = snapshot_text(data, "context", 512, AccountError::Invalid)?.to_owned();
            if context.len() + code.len() + word.len() > 2048 {
                return Err(AccountError::Invalid);
            }
            let identity = (context.clone(), code, word);
            if kind == "position" {
                let position = snapshot_integer(data, "position", AccountError::Invalid)?;
                if !(1..=5).contains(&position)
                    || !positions.insert(identity)
                    || !position_slots.insert((context, position))
                {
                    return Err(AccountError::Invalid);
                }
                Ok(3)
            } else {
                let count = snapshot_integer(data, "count", AccountError::Invalid)?;
                if !(0..=10).contains(&count) || !selections.insert(identity) {
                    return Err(AccountError::Invalid);
                }
                Ok(4)
            }
        }
        _ => Err(AccountError::Invalid),
    }
}

fn inspect_snapshot(path: &std::path::Path) -> Result<SnapshotMetadata, AccountError> {
    const MAX_BYTES: u64 = 512 * 1024 * 1024;
    let file =
        crate::shared::atomic_file::open_private(path).map_err(|_| AccountError::Unavailable)?;
    let mut reader = BufReader::with_capacity(65_536, file);
    let mut line = Vec::with_capacity(65_536);
    let mut total_bytes = 0u64;
    let mut records = 0usize;
    let mut counts = [0usize; 4];
    let mut category = 0u8;
    let mut revision = None;
    let mut digest = Sha256::new();
    let mut file_digest = Sha256::new();
    let mut checksum = None;
    let mut entry_keys = HashMap::new();
    let mut entry_ids = HashSet::new();
    let mut overlays = HashMap::new();
    let mut positions = HashSet::new();
    let mut position_slots = HashSet::new();
    let mut selections = HashSet::new();
    loop {
        line.clear();
        let complete = loop {
            let chunk = reader.fill_buf().map_err(|_| AccountError::Unavailable)?;
            if chunk.is_empty() {
                break false;
            }
            if let Some(index) = chunk.iter().position(|byte| *byte == b'\n') {
                if line.len() + index + 1 > 65_536 {
                    return Err(AccountError::Invalid);
                }
                line.extend_from_slice(&chunk[..=index]);
                reader.consume(index + 1);
                break true;
            }
            if line.len() + chunk.len() >= 65_536 {
                return Err(AccountError::Invalid);
            }
            line.extend_from_slice(chunk);
            let length = chunk.len();
            reader.consume(length);
        };
        let has_newline = complete;
        file_digest.update(&line);
        if has_newline {
            line.pop();
            if line.ends_with(b"\r") {
                return Err(AccountError::Invalid);
            }
        } else if line.is_empty() {
            break;
        }
        total_bytes = total_bytes
            .checked_add(line.len() as u64 + u64::from(has_newline))
            .ok_or(AccountError::Unavailable)?;
        if total_bytes > MAX_BYTES || line.is_empty() {
            return Err(AccountError::Invalid);
        }
        let map = parse_strict_object(&line).map_err(|_| AccountError::Invalid)?;
        let kind = map
            .get("type")
            .and_then(Value::as_str)
            .ok_or(AccountError::Invalid)?;
        match kind {
            "header" => {
                if records != 0
                    || revision.is_some()
                    || !snapshot_has_keys(&map, &["type", "format", "version", "revision"])
                    || map.get("format").and_then(Value::as_str)
                        != Some("msime-dictionary-snapshot")
                    || map.get("version").and_then(Value::as_i64) != Some(1)
                {
                    return Err(AccountError::Invalid);
                }
                revision = Some(
                    map.get("revision")
                        .and_then(Value::as_i64)
                        .filter(|value| *value >= 0)
                        .ok_or(AccountError::Invalid)?,
                );
                // The header is a record like any other: the reference
                // implementation counts it and hashes it, so a snapshot whose
                // footer was written by the Server only verifies if we do too.
                records = records.checked_add(1).ok_or(AccountError::Unavailable)?;
                digest.update(&line);
                digest.update([b'\n']);
            }
            "entry" | "overlay" | "position" | "selection" => {
                let snapshot_revision = revision.ok_or(AccountError::Invalid)?;
                if checksum.is_some() {
                    return Err(AccountError::Invalid);
                }
                let next = inspect_snapshot_record(
                    &map,
                    snapshot_revision,
                    &mut entry_keys,
                    &mut entry_ids,
                    &mut overlays,
                    &mut positions,
                    &mut position_slots,
                    &mut selections,
                )?;
                if next < category {
                    return Err(AccountError::Invalid);
                }
                category = next;
                counts[(next - 1) as usize] = counts[(next - 1) as usize]
                    .checked_add(1)
                    .ok_or(AccountError::Unavailable)?;
                records = records.checked_add(1).ok_or(AccountError::Unavailable)?;
                if records > 500_000 || counts[0] > 100_000 {
                    return Err(AccountError::Invalid);
                }
                digest.update(&line);
                digest.update([b'\n']);
            }
            "footer" => {
                if revision.is_none()
                    || checksum.is_some()
                    || !snapshot_has_keys(&map, &["type", "records", "sha256"])
                {
                    return Err(AccountError::Invalid);
                }
                let expected_records = map
                    .get("records")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or(AccountError::Invalid)?;
                let expected_sha = map
                    .get("sha256")
                    .and_then(Value::as_str)
                    .filter(|value| msime_client_core::is_ascii_hex(value, 64))
                    .ok_or(AccountError::Invalid)?;
                // Cloned, not consumed: the loop keeps reading after the footer
                // so that trailing data is rejected, and those iterations still
                // reach the digest.
                let actual = hex::encode(digest.clone().finalize());
                if expected_records != records || expected_sha != actual {
                    return Err(AccountError::Invalid);
                }
                checksum = Some(expected_sha.to_owned());
            }
            _ => return Err(AccountError::Invalid),
        }
        if !has_newline {
            break;
        }
    }
    let revision = revision.ok_or(AccountError::Invalid)?;
    let sha256 = checksum.ok_or(AccountError::Invalid)?;
    for (identity, entry_weight) in &entry_keys {
        let Some((deleted, user_inserted, weight)) = overlays.get(identity) else {
            return Err(AccountError::Invalid);
        };
        if *deleted || !*user_inserted || *weight != *entry_weight {
            return Err(AccountError::Invalid);
        }
    }
    for (identity, (deleted, user_inserted, weight)) in &overlays {
        if !*deleted && *user_inserted {
            let Some(entry_weight) = entry_keys.get(identity) else {
                return Err(AccountError::Invalid);
            };
            if *entry_weight != *weight {
                return Err(AccountError::Invalid);
            }
        }
    }
    Ok(SnapshotMetadata {
        cloud_revision: revision,
        sha256,
        file_sha256: hex::encode(file_digest.finalize()),
        bytes: total_bytes,
        records,
        entries: counts[0],
        overlays: counts[1],
        positions: counts[2],
        selections: counts[3],
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EnqueueSnapshotRequest {
    source: String,
    account_id: String,
    cloud_revision: i64,
    expected_local_version: String,
    file_sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CancelSnapshotRequest {
    account_id: String,
}

fn cancel_queued_snapshot(platform: &PluginHandle<Wry>, account_id: Option<&str>) {
    if let Some(account_id) = account_id {
        let _ = platform.run_mobile_plugin::<Value>(
            "cancelSnapshot",
            CancelSnapshotRequest {
                account_id: account_id.to_owned(),
            },
        );
    }
}

async fn dictionary_snapshot_preview(
    state: State<'_, AccountState>,
) -> Result<Value, crate::CommandError> {
    let session = Arc::clone(&state.session);
    let directory = state.snapshot_directory.clone();
    let previews = Arc::clone(&state.snapshot_previews);
    let token = Uuid::new_v4().to_string();
    // The token names the staged file and is also returned to the caller, so the worker takes a
    // copy rather than the value the pending entry below is keyed on.
    let file_token = token.clone();
    let (account_id, generation, path, metadata) =
        tauri::async_runtime::spawn_blocking(move || {
            prepare_snapshot_directory(&directory).map_err(|_| AccountError::Unavailable)?;
            let profile = session.profile()?;
            let (_, _, generation) =
                session.credentials_with_generation(None, Some(&profile.user.id))?;
            let path = directory.join(format!("download-{file_token}.ndjson"));
            let result = session
                .dictionary_snapshot_to_file(&path)
                .and_then(|_| inspect_snapshot(&path));
            match result {
                Ok(metadata) => Ok((profile.user.id, generation, path, metadata)),
                Err(error) => {
                    let _ = remove_snapshot_file(&path);
                    Err(error)
                }
            }
        })
        .await
        .map_err(|_| snapshot_command_error())?
        .map_err(account_command_error)?;
    let old = publish_snapshot_preview(
        &state.session,
        generation,
        &account_id,
        &previews,
        token.clone(),
        PendingSnapshot {
            account_id: account_id.clone(),
            generation,
            path: path.clone(),
            metadata: metadata.clone(),
        },
    );
    let old = match old {
        Ok(old) => old,
        Err(error) => {
            let _ = remove_snapshot_file(&path);
            return Err(account_command_error(error));
        }
    };
    for preview in old {
        let _ = remove_snapshot_file(&preview.path);
    }
    Ok(serde_json::json!({
        "previewToken": token,
        "snapshot": metadata,
    }))
}

async fn dictionary_snapshot_enqueue(
    state: State<'_, AccountState>,
    token: String,
) -> Result<Value, crate::CommandError> {
    let pending = take_pending_snapshot(&state.snapshot_previews, &token)?;
    let session = Arc::clone(&state.session);
    let platform = state.platform.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let path = pending.path.clone();
        let result = (|| {
            validate_pending_snapshot(&session, &pending)?;
            let state = platform
                .run_mobile_plugin::<Value>("snapshotState", ())
                .map_err(|_| AccountError::Unavailable)?;
            let expected = state
                .get("localVersion")
                .and_then(Value::as_str)
                .ok_or(AccountError::Conflict)?
                .to_owned();
            let request = EnqueueSnapshotRequest {
                source: path.to_string_lossy().into_owned(),
                account_id: pending.account_id.clone(),
                cloud_revision: pending.metadata.cloud_revision,
                expected_local_version: expected,
                file_sha256: pending.metadata.file_sha256.clone(),
            };
            session.with_generation(pending.generation, Some(&pending.account_id), || {
                platform
                    .run_mobile_plugin::<Value>("enqueueSnapshot", request)
                    .map_err(|_| AccountError::Unavailable)
            })
        })();
        let _ = remove_snapshot_file(&path);
        result
    })
    .await
    .map_err(|_| snapshot_command_error())?
    .map_err(account_command_error)?;
    snapshot_response_without_account(result)
}

async fn dictionary_snapshot_export(
    state: State<'_, AccountState>,
) -> Result<Value, crate::CommandError> {
    let session = Arc::clone(&state.session);
    let directory = state.snapshot_directory.clone();
    let token = Uuid::new_v4().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        prepare_snapshot_directory(&directory).map_err(|_| AccountError::Unavailable)?;
        let path = directory.join(format!("export-{token}.ndjson"));
        let result = session
            .dictionary_snapshot_to_file(&path)
            .and_then(|_| inspect_snapshot(&path))
            .and_then(|metadata| {
                let text = read_snapshot_file(&path).map_err(|_| AccountError::Unavailable)?;
                Ok(serde_json::json!({
                    "text": text,
                    "filename": "msime-dictionary-snapshot.ndjson",
                    "snapshot": metadata,
                }))
            });
        let _ = remove_snapshot_file(&path);
        result
    })
    .await
    .map_err(|_| snapshot_command_error())?
    .map_err(account_command_error)
}

async fn dictionary_snapshot_restore_preview(
    state: State<'_, AccountState>,
    text: String,
) -> Result<Value, crate::CommandError> {
    if !snapshot_text_within_limit(text.len()) {
        return Err(crate::CommandError {
            code: "snapshot_invalid",
        });
    }
    let session = Arc::clone(&state.session);
    let directory = state.snapshot_directory.clone();
    let token = Uuid::new_v4().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        prepare_snapshot_directory(&directory).map_err(|_| AccountError::Unavailable)?;
        let path = directory.join(format!("restore-{token}.ndjson"));
        let result = write_snapshot_file(&path, text.as_bytes())
            .map_err(|_| AccountError::Unavailable)
            .and_then(|_| inspect_snapshot(&path))
            .and_then(|metadata| {
                session
                    .dictionary_catalog(DictionaryKind::Quick, "", 0, "pinyin", "xiaohe")
                    .map(|page| {
                        serde_json::json!({
                            "snapshot": metadata,
                            "expectedRevision": page.revision,
                        })
                    })
            });
        let _ = remove_snapshot_file(&path);
        result
    })
    .await
    .map_err(|_| snapshot_command_error())?
    .map_err(account_command_error)
}

async fn dictionary_snapshot_restore(
    state: State<'_, AccountState>,
    text: String,
    expected_sha256: String,
    revision: i64,
) -> Result<Value, crate::CommandError> {
    if !snapshot_text_within_limit(text.len()) {
        return Err(crate::CommandError {
            code: "snapshot_invalid",
        });
    }
    let session = Arc::clone(&state.session);
    let directory = state.snapshot_directory.clone();
    let token = Uuid::new_v4().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        prepare_snapshot_directory(&directory).map_err(|_| AccountError::Unavailable)?;
        let path = directory.join(format!("restore-{token}.ndjson"));
        let result = write_snapshot_file(&path, text.as_bytes())
            .map_err(|_| AccountError::Unavailable)
            .and_then(|_| inspect_snapshot(&path))
            .and_then(|metadata| {
                if metadata.sha256 != expected_sha256 {
                    return Err(AccountError::Invalid);
                }
                session
                    .restore_dictionary_snapshot(text.as_bytes(), revision)
                    .and_then(|result| {
                        Ok(serde_json::json!({
                            "revision": result.revision,
                            "reset": result.reset,
                        }))
                    })
            });
        let _ = remove_snapshot_file(&path);
        result
    })
    .await
    .map_err(|_| snapshot_command_error())?
    .map_err(account_command_error)
}

async fn dictionary_snapshot_status(
    state: State<'_, AccountState>,
) -> Result<Value, crate::CommandError> {
    let platform = state.platform.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        platform
            .run_mobile_plugin::<Value>("snapshotState", ())
            .map_err(|_| snapshot_command_error())
    })
    .await
    .map_err(|_| snapshot_command_error())??;
    snapshot_response_without_account(result)
}

async fn dictionary_snapshot_cancel(
    state: State<'_, AccountState>,
) -> Result<Value, crate::CommandError> {
    let session = Arc::clone(&state.session);
    let platform = state.platform.clone();
    let previews = Arc::clone(&state.snapshot_previews);
    let account_id = tauri::async_runtime::spawn_blocking(move || {
        session.profile().map(|profile| profile.user.id)
    })
    .await
    .map_err(|_| snapshot_command_error())?
    .map_err(account_command_error)?;
    clear_snapshot_previews(&previews);
    let result = tauri::async_runtime::spawn_blocking(move || {
        platform
            .run_mobile_plugin::<Value>("cancelSnapshot", CancelSnapshotRequest { account_id })
            .map_err(|_| snapshot_command_error())
    })
    .await
    .map_err(|_| snapshot_command_error())??;
    snapshot_response_without_account(result)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppIconResponse {
    pub supported: bool,
    pub selected: String,
}

#[derive(Serialize)]
struct AppIconRequest<'a> {
    style: &'a str,
}

#[tauri::command]
pub async fn account_status(
    state: State<'_, AccountState>,
) -> Result<StatusResponse, crate::CommandError> {
    shared_account_status(state).await
}

#[tauri::command]
pub async fn android_open_input_method_settings(
    state: State<'_, AccountState>,
) -> Result<(), crate::CommandError> {
    let plugin = state.platform.clone();
    tauri::async_runtime::spawn_blocking(move || {
        plugin
            .run_mobile_plugin::<()>("openInputMethodSettings", ())
            .map_err(|_| crate::CommandError {
                code: "system_settings",
            })
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "system_settings",
    })?
}

#[tauri::command]
pub async fn android_open_keyboard_tryout(
    state: State<'_, AccountState>,
) -> Result<(), crate::CommandError> {
    let plugin = state.platform.clone();
    tauri::async_runtime::spawn_blocking(move || {
        plugin
            .run_mobile_plugin::<()>("openKeyboardTryout", ())
            .map_err(|_| crate::CommandError {
                code: "keyboard_tryout",
            })
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "keyboard_tryout",
    })?
}

#[tauri::command]
pub async fn android_show_input_method_picker(
    state: State<'_, AccountState>,
) -> Result<(), crate::CommandError> {
    let plugin = state.platform.clone();
    tauri::async_runtime::spawn_blocking(move || {
        plugin
            .run_mobile_plugin::<()>("showInputMethodPicker", ())
            .map_err(|_| crate::CommandError {
                code: "input_method_picker",
            })
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "input_method_picker",
    })?
}

#[tauri::command]
pub async fn android_bootstrap_status(
    state: State<'_, AccountState>,
) -> Result<bool, crate::CommandError> {
    let plugin = state.platform.clone();
    tauri::async_runtime::spawn_blocking(move || {
        plugin
            .run_mobile_plugin::<Value>("bootstrapStatus", ())
            .map_err(|_| crate::CommandError { code: "bootstrap" })?
            .get("ready")
            .and_then(Value::as_bool)
            .ok_or(crate::CommandError { code: "bootstrap" })
    })
    .await
    .map_err(|_| crate::CommandError { code: "bootstrap" })?
}

#[tauri::command]
pub async fn android_prepare_bootstrap(
    state: State<'_, AccountState>,
) -> Result<(), crate::CommandError> {
    let plugin = state.platform.clone();
    tauri::async_runtime::spawn_blocking(move || {
        plugin
            .run_mobile_plugin::<Value>("prepareBootstrap", ())
            .map_err(|_| crate::CommandError { code: "bootstrap" })?
            .get("ready")
            .and_then(Value::as_bool)
            .filter(|ready| *ready)
            .map(|_| ())
            .ok_or(crate::CommandError { code: "bootstrap" })
    })
    .await
    .map_err(|_| crate::CommandError { code: "bootstrap" })?
}

#[tauri::command]
pub async fn ai_models(
    state: State<'_, AccountState>,
    endpoint: String,
    token: String,
) -> Result<Vec<String>, crate::CommandError> {
    let plugin = state.platform.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let response = plugin
            .run_mobile_plugin::<Value>(
                "aiModels",
                AiModelsRequest {
                    endpoint: &endpoint,
                    token: &token,
                },
            )
            .map_err(|_| crate::CommandError {
                code: "ai_models_unavailable",
            })?;
        response
            .get("models")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
            .ok_or(crate::CommandError {
                code: "ai_models_invalid",
            })
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "ai_models_unavailable",
    })?
}

#[tauri::command]
pub async fn ai_test(
    state: State<'_, AccountState>,
    endpoint: String,
    model: String,
    prompt: String,
    token: String,
    text: String,
) -> Result<String, crate::CommandError> {
    let plugin = state.platform.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let response = plugin
            .run_mobile_plugin::<Value>(
                "aiTest",
                AiTestRequest {
                    endpoint: &endpoint,
                    model: &model,
                    prompt: &prompt,
                    token: &token,
                    text: &text,
                },
            )
            .map_err(|_| crate::CommandError {
                code: "ai_test_unavailable",
            })?;
        response
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or(crate::CommandError {
                code: "ai_test_invalid",
            })
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "ai_test_unavailable",
    })?
}

#[tauri::command]
pub async fn account_providers(
    state: State<'_, AccountState>,
) -> Result<ProvidersResponse, crate::CommandError> {
    call_session(state.session(), |session| {
        session.providers().map(providers_response_without_apple)
    })
    .await
}

#[tauri::command]
pub async fn account_request_code(
    state: State<'_, AccountState>,
    provider: String,
    target: String,
) -> Result<ChallengeResponse, crate::CommandError> {
    shared_account_request_code(state, provider, target).await
}

#[tauri::command]
pub async fn account_login(
    state: State<'_, AccountState>,
    challenge_id: String,
    code: String,
) -> Result<StatusResponse, crate::CommandError> {
    shared_account_login(state, challenge_id, code).await
}

#[tauri::command]
pub async fn account_profile(
    state: State<'_, AccountState>,
) -> Result<ProfileResponse, crate::CommandError> {
    shared_account_profile(state).await
}

#[tauri::command]
pub async fn account_chat_models(
    state: State<'_, AccountState>,
) -> Result<ChatModelsResponse, crate::CommandError> {
    shared_account_chat_models(state).await
}

#[tauri::command]
pub async fn account_chat(
    state: State<'_, AccountState>,
    messages: Vec<AccountChatMessage>,
    model: String,
) -> Result<ChatResponse, crate::CommandError> {
    shared_account_chat(state, messages, model).await
}

#[tauri::command]
pub async fn account_rename(
    state: State<'_, AccountState>,
    display_name: String,
) -> Result<ProfileResponse, crate::CommandError> {
    shared_account_rename(state, display_name).await
}

#[tauri::command]
pub async fn account_logout(
    state: State<'_, AccountState>,
    all: bool,
) -> Result<(), crate::CommandError> {
    let session = Arc::clone(&state.session);
    let previews = Arc::clone(&state.snapshot_previews);
    let platform = state.platform.clone();
    let result = call_session(&session, move |session| {
        session.logout_with_cleanup(all, |account_id| {
            cancel_queued_snapshot(&platform, account_id)
        })
    })
    .await;
    clear_snapshot_previews_after(&session, &previews, result)
}

#[tauri::command]
pub async fn account_delete(state: State<'_, AccountState>) -> Result<(), crate::CommandError> {
    let session = Arc::clone(&state.session);
    let previews = Arc::clone(&state.snapshot_previews);
    let platform = state.platform.clone();
    let result = call_session(&session, move |session| {
        session
            .delete_account_with_cleanup(|account_id| cancel_queued_snapshot(&platform, account_id))
    })
    .await;
    clear_snapshot_previews_after(&session, &previews, result)
}

#[tauri::command]
pub async fn account_forget(state: State<'_, AccountState>) -> Result<(), crate::CommandError> {
    let session = Arc::clone(&state.session);
    let previews = Arc::clone(&state.snapshot_previews);
    let platform = state.platform.clone();
    let result = call_session(&session, move |session| {
        session.forget_with_cleanup(|account_id| cancel_queued_snapshot(&platform, account_id))
    })
    .await;
    clear_snapshot_previews_after(&session, &previews, result)
}

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
        // Restoring from a host-held file is a macOS path; this host stages its own preview.
        CloudDictionaryRequest::SnapshotRestoreNative { .. } => Err(crate::CommandError {
            code: "snapshot_unavailable",
        }),
        CloudDictionaryRequest::SnapshotRestoreCancel => Err(crate::CommandError {
            code: "snapshot_unavailable",
        }),
        CloudDictionaryRequest::SnapshotChooseRestore => Err(crate::CommandError {
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

#[tauri::command]
pub async fn app_icon_info(
    state: State<'_, AccountState>,
) -> Result<AppIconResponse, crate::CommandError> {
    let plugin = state.platform.clone();
    tauri::async_runtime::spawn_blocking(move || {
        plugin
            .run_mobile_plugin::<AppIconResponse>("appIconInfo", ())
            .map_err(|_| crate::CommandError { code: "app_icon" })
    })
    .await
    .map_err(|_| crate::CommandError { code: "app_icon" })?
}

#[tauri::command]
pub async fn app_icon_set(
    state: State<'_, AccountState>,
    style: String,
) -> Result<AppIconResponse, crate::CommandError> {
    if !msime_tauri_mobile_platform::is_supported_app_icon_style(&style) {
        return Err(crate::CommandError {
            code: "invalid_app_icon",
        });
    }
    let plugin = state.platform.clone();
    tauri::async_runtime::spawn_blocking(move || {
        plugin
            .run_mobile_plugin::<AppIconResponse>("setAppIcon", AppIconRequest { style: &style })
            .map_err(|_| crate::CommandError { code: "app_icon" })
    })
    .await
    .map_err(|_| crate::CommandError { code: "app_icon" })?
}

fn load_host_feedback(feedback: &PluginHandle<Wry>) -> Result<HostKeyboardFeedback, AccountError> {
    let loaded = feedback
        .run_mobile_plugin::<FeedbackSettings>("loadFeedback", ())
        .map_err(|_| AccountError::Storage)?;
    Ok(HostKeyboardFeedback {
        sound_enabled: loaded.sound_enabled,
        haptics_enabled: loaded.haptics_enabled,
        haptic_strength: loaded.haptic_strength,
    })
}

/// 本机设置导出成账号文档的键值，与原生 Android 宿主用同一份映射（`settings_sync::export_android_settings`）；按键反馈从插件读。
fn local_account_preferences(
    snapshot: &PreferencesSnapshot,
    feedback: &PluginHandle<Wry>,
) -> Result<BTreeMap<String, AccountPreferenceValue>, AccountError> {
    let host = load_host_feedback(feedback)?;
    export_android_settings(&snapshot.preferences, Some(&host))
}

/// 把云端文档应用到本机偏好，与原生 Android 宿主用同一份映射（`settings_sync::apply_android_settings`）：本机不认识或超出范围的键跳过，其余照常应用。文档带按键反馈时先从插件读出当前值，应用后写回插件。
fn apply_local_account_preferences(
    snapshot: &PreferencesSnapshot,
    cloud: &AccountPreferences,
    schema: &AccountPreferenceSchema,
    feedback: &PluginHandle<Wry>,
) -> Result<Preferences, AccountError> {
    let host = if needs_host_feedback(cloud, schema) {
        Some(load_host_feedback(feedback)?)
    } else {
        None
    };
    let applied = apply_android_settings(&snapshot.preferences, cloud, schema, host)?;
    if let Some(next) = applied.feedback {
        let request = serde_json::json!({
            "soundEnabled": next.sound_enabled,
            "hapticsEnabled": next.haptics_enabled,
            "hapticStrength": next.haptic_strength,
        });
        feedback
            .run_mobile_plugin::<()>("saveFeedback", request)
            .map_err(|_| AccountError::Storage)?;
    }
    Ok(applied.preferences)
}

#[tauri::command]
pub async fn account_preferences_schema(
    state: State<'_, AccountState>,
) -> Result<PreferenceSchemaResponse, crate::CommandError> {
    shared_account_preferences_schema(state).await
}

#[tauri::command]
pub async fn account_preferences_load(
    state: State<'_, AccountState>,
) -> Result<AccountPreferences, crate::CommandError> {
    shared_account_preferences_load(state).await
}

#[tauri::command]
pub async fn account_preferences_upload(
    app: tauri::AppHandle,
    state: State<'_, AccountState>,
    store: State<'_, Arc<PreferencesStore>>,
) -> Result<AccountPreferences, crate::CommandError> {
    let session = Arc::clone(&state.session);
    let feedback = state.feedback.clone();
    let store = store.inner().clone();
    let edition = crate::host_edition(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let (user_id, _, generation) = session.credentials_with_generation(None, None)?;
        let schema = session.preference_schema()?;
        let cloud = session.preferences()?;
        let local = store.load().map_err(|_| AccountError::Storage)?;
        let mut values = local_account_preferences(&local, &feedback)?;
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
    .map_err(|_| crate::CommandError {
        code: "account_unavailable",
    })?
    .map_err(account_command_error)
}

#[tauri::command]
pub async fn account_preferences_apply(
    app: tauri::AppHandle,
    state: State<'_, AccountState>,
    store: State<'_, Arc<PreferencesStore>>,
    user_id: String,
    mut preferences: AccountPreferences,
) -> Result<(), crate::CommandError> {
    let session = Arc::clone(&state.session);
    let feedback = state.feedback.clone();
    let store = store.inner().clone();
    // 单方案版本不应用账号里的方案，多方案版本把本版本没有的方案当作缺失。
    filter_downloaded_account_settings(crate::host_edition(&app), &mut preferences.settings);
    tauri::async_runtime::spawn_blocking(move || {
        let (_, _, generation) = session.credentials_with_generation(None, Some(&user_id))?;
        let schema = session.preference_schema()?;
        let local = store.load().map_err(|_| AccountError::Storage)?;
        session.with_generation(generation, Some(&user_id), || {
            let next = apply_local_account_preferences(&local, &preferences, &schema, &feedback)?;
            store
                .save(local.revision, next)
                .map_err(|_| AccountError::Storage)
        })?;
        Ok::<(), AccountError>(())
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "account_unavailable",
    })?
    .map_err(account_command_error)
}

#[tauri::command]
pub async fn mobile_keyboard_feedback_load(
    state: State<'_, AccountState>,
) -> Result<FeedbackSettings, crate::CommandError> {
    let feedback = state.feedback.clone();
    tauri::async_runtime::spawn_blocking(move || {
        feedback
            .run_mobile_plugin::<FeedbackSettings>("loadFeedback", ())
            .map_err(|_| crate::CommandError {
                code: "feedback_storage",
            })
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "feedback_storage",
    })?
}

#[tauri::command]
pub async fn mobile_keyboard_feedback_save(
    state: State<'_, AccountState>,
    request: MobileKeyboardFeedbackRequest,
) -> Result<FeedbackSettings, crate::CommandError> {
    if !valid_mobile_haptic_strength(&request.settings.haptic_strength) {
        return Err(crate::CommandError {
            code: "invalid_feedback",
        });
    }
    let feedback = state.feedback.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let payload = serde_json::json!({
            "soundEnabled": request.settings.sound_enabled,
            "hapticsEnabled": request.settings.haptics_enabled,
            "hapticStrength": request.settings.haptic_strength,
        });
        feedback
            .run_mobile_plugin::<()>("saveFeedback", payload)
            .map_err(|_| crate::CommandError {
                code: "feedback_storage",
            })?;
        Ok(request.settings)
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "feedback_storage",
    })?
}

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
    let feedback = state.feedback.clone();
    tauri::async_runtime::spawn_blocking(move || {
        feedback
            .run_mobile_plugin::<()>(
                "previewFeedback",
                serde_json::json!({
                    "hapticStrength": request.strength,
                }),
            )
            .map_err(|_| crate::CommandError {
                code: "feedback_preview",
            })
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "feedback_preview",
    })?
}
