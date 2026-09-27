//! Linux desktop account commands.
//!
//! The commands themselves are the same nine the Windows and macOS shells
//! register; only the session store differs, because Linux has no single secret
//! service every target desktop is guaranteed to run. This host keeps the
//! session in an owner-only file inside the shared state directory - the same
//! rule the Linux provider services already state for their credential files,
//! and the same 0700 directory `msime-linux-prepare` publishes. That is
//! weaker than the Windows Credential Manager or the macOS Keychain, which
//! encrypt at rest; it is stronger than not offering the account surface at
//! all, which is where this host was.
//!
//! The React surface receives the same redacted DTOs as every other host: the
//! tokens never leave this process.

use msime_client_core::account::{
    AccountChallenge, AccountError, AccountProfile, AccountSessionStorage, AccountUser,
    BackendAccountClient, BackendAccountSession, SavedAccountSession,
};
use serde::Serialize;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::Manager;

/// A session document is a pair of JWTs and an expiry. Anything appreciably
/// larger is not one, and is rejected before it is parsed.
const MAX_ACCOUNT_SESSION_BYTES: u64 = 16 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    user: Option<UserResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UserResponse {
    id: String,
    display_name: String,
    created_at: String,
}

impl From<AccountUser> for UserResponse {
    fn from(user: AccountUser) -> Self {
        Self {
            id: user.id,
            display_name: user.display_name,
            created_at: user.created_at,
        }
    }
}

#[derive(Serialize)]
pub struct ProvidersResponse {
    email: bool,
    phone: bool,
    apple: bool,
}

fn providers_response(providers: std::collections::HashMap<String, bool>) -> ProvidersResponse {
    ProvidersResponse {
        email: providers.get("email") == Some(&true),
        phone: providers.get("phone") == Some(&true) || providers.get("sms") == Some(&true),
        apple: providers.get("apple") == Some(&true),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeResponse {
    challenge_id: String,
    expires_in: u64,
}

impl From<AccountChallenge> for ChallengeResponse {
    fn from(value: AccountChallenge) -> Self {
        Self {
            challenge_id: value.challenge_id,
            expires_in: value.expires_in,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileResponse {
    user: UserResponse,
    providers: Vec<String>,
}

impl From<AccountProfile> for ProfileResponse {
    fn from(profile: AccountProfile) -> Self {
        let mut providers = Vec::new();
        for identity in profile.identities {
            if !providers.contains(&identity.provider) {
                providers.push(identity.provider);
            }
        }
        Self {
            user: profile.user.into(),
            providers,
        }
    }
}

#[derive(Clone)]
pub(crate) struct LinuxAccountStorage {
    path: PathBuf,
}

impl LinuxAccountStorage {
    pub(crate) fn new(directory: &Path) -> Self {
        Self {
            path: directory.join("account-session.json"),
        }
    }
}

impl AccountSessionStorage for LinuxAccountStorage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        // symlink_metadata, not metadata: a symlink here is not a store this
        // host wrote, and following it would read through a path chosen by
        // whoever planted it. A file any other user can read is likewise not
        // one this host would have produced, and a session read out of it is a
        // session nobody vouched for.
        let metadata = match std::fs::symlink_metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(AccountError::Storage),
        };
        if !metadata.is_file()
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o077 != 0
            || metadata.len() > MAX_ACCOUNT_SESSION_BYTES
        {
            return Err(AccountError::Storage);
        }
        // The file can grow after symlink_metadata returns. Read through a bounded handle so a
        // concurrent replacement cannot turn the size check into an unbounded allocation.
        let mut bytes = Vec::new();
        std::fs::File::open(&self.path)
            .map_err(|_| AccountError::Storage)?
            .take(MAX_ACCOUNT_SESSION_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| AccountError::Storage)?;
        if bytes.len() as u64 > MAX_ACCOUNT_SESSION_BYTES {
            return Err(AccountError::Storage);
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| AccountError::Storage)?;
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|_| AccountError::Storage)
    }

    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        let value = serde_json::to_vec(session).map_err(|_| AccountError::Storage)?;
        if value.len() as u64 > MAX_ACCOUNT_SESSION_BYTES {
            return Err(AccountError::Storage);
        }
        let directory = self.path.parent().ok_or(AccountError::Storage)?;
        std::fs::create_dir_all(directory).map_err(|_| AccountError::Storage)?;
        // Publish by rename so a reader never sees a half-written document, and
        // create the temporary file 0600 from the start rather than widening it
        // afterwards - between create and chmod the tokens would be readable.
        // mode() only applies to a new file, so a leftover temporary (or a
        // symlink planted there) is removed and creation is exclusive.
        let temporary = self.path.with_extension("json.new");
        let _ = std::fs::remove_file(&temporary);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| AccountError::Storage)?;
        let written = file
            .write_all(&value)
            .and_then(|()| file.sync_all())
            .map_err(|_| AccountError::Storage);
        drop(file);
        if let Err(error) = written {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
        if std::fs::rename(&temporary, &self.path).is_err() {
            let _ = std::fs::remove_file(&temporary);
            return Err(AccountError::Storage);
        }
        Ok(())
    }

    fn clear(&self) -> Result<(), AccountError> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(AccountError::Storage),
        }
    }
}

type Session = BackendAccountSession<BackendAccountClient, LinuxAccountStorage>;

pub struct AccountState {
    pub(crate) session: Arc<Session>,
}

pub fn setup(app: &tauri::AppHandle, directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let client = BackendAccountClient::new()?;
    let session = Arc::new(BackendAccountSession::new(
        client,
        LinuxAccountStorage::new(directory),
    ));
    app.manage(AccountState { session });
    Ok(())
}

async fn call<T, F>(
    state: tauri::State<'_, AccountState>,
    operation: F,
) -> Result<T, crate::CommandError>
where
    T: Send + 'static,
    F: FnOnce(&Session) -> Result<T, AccountError> + Send + 'static,
{
    let session = Arc::clone(&state.session);
    tauri::async_runtime::spawn_blocking(move || operation(&session))
        .await
        .map_err(|_| crate::CommandError {
            code: "account_unavailable",
        })?
        .map_err(|error| crate::CommandError { code: error.code() })
}

#[tauri::command]
pub async fn account_status(
    state: tauri::State<'_, AccountState>,
) -> Result<StatusResponse, crate::CommandError> {
    call(state, |session| {
        session.status().map(|user| StatusResponse {
            user: user.map(Into::into),
        })
    })
    .await
}

#[tauri::command]
pub async fn account_providers(
    state: tauri::State<'_, AccountState>,
) -> Result<ProvidersResponse, crate::CommandError> {
    call(state, |session| session.providers().map(providers_response)).await
}

#[tauri::command]
pub async fn account_request_code(
    state: tauri::State<'_, AccountState>,
    provider: String,
    target: String,
) -> Result<ChallengeResponse, crate::CommandError> {
    call(state, move |session| {
        session.request_code(&provider, &target).map(Into::into)
    })
    .await
}

#[tauri::command]
pub async fn account_login(
    state: tauri::State<'_, AccountState>,
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

#[tauri::command]
pub async fn account_profile(
    state: tauri::State<'_, AccountState>,
) -> Result<ProfileResponse, crate::CommandError> {
    call(state, |session| session.profile().map(Into::into)).await
}

#[tauri::command]
pub async fn account_rename(
    state: tauri::State<'_, AccountState>,
    display_name: String,
) -> Result<ProfileResponse, crate::CommandError> {
    call(state, move |session| {
        session.rename(&display_name).map(Into::into)
    })
    .await
}

#[tauri::command]
pub async fn account_logout(
    state: tauri::State<'_, AccountState>,
    all: bool,
) -> Result<(), crate::CommandError> {
    call(state, move |session| session.logout(all)).await
}

#[tauri::command]
pub async fn account_delete(
    state: tauri::State<'_, AccountState>,
) -> Result<(), crate::CommandError> {
    call(state, |session| session.delete_account()).await
}

#[tauri::command]
pub async fn account_forget(
    state: tauri::State<'_, AccountState>,
) -> Result<(), crate::CommandError> {
    call(state, |session| session.forget()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_session_is_rejected_after_a_bounded_read() {
        let directory = tempfile::tempdir().unwrap();
        let storage = LinuxAccountStorage::new(directory.path());
        std::fs::write(
            directory.path().join("account-session.json"),
            vec![b' '; MAX_ACCOUNT_SESSION_BYTES as usize + 1],
        )
        .unwrap();
        assert!(matches!(storage.load(), Err(AccountError::Storage)));
    }
}
