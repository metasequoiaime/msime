//! Desktop account commands shared by the macOS, Windows and Linux shells.
//!
//! The nine commands, their state and the blocking-call bridge are the same on all three desktop hosts; only where the session tokens are kept differs. Each platform module owns its [`AccountSessionStorage`](msime_client_core::account::AccountSessionStorage) implementation and a thin `setup` that builds it and hands it to [`manage`]: the macOS Keychain item written by the Swift backend, the per-user Windows Credential Manager, or an owner-only file in the Linux shared state directory. The React surface only receives the same redacted DTOs as the mobile hosts; the tokens never leave this process.

use crate::platform::account_helpers::call_session;
use crate::platform::desktop::desktop_candidate_skin_community::CandidateSkinCommunityState;
use crate::shared::account_dto::{
    providers_response, ChallengeResponse, ProfileResponse, ProvidersResponse, StatusResponse,
};
use msime_client_core::account::{BackendAccountClient, BackendAccountSession};
use msime_client_core::skin::candidate_community::BackendCandidateSkinCommunityService;
use std::sync::Arc;
use tauri::Manager;

#[cfg(target_os = "macos")]
pub(crate) type Storage = crate::platform::macos::macos_account::MacosAccountStorage;
#[cfg(target_os = "windows")]
pub(crate) type Storage = crate::platform::windows::windows_account::WindowsAccountStorage;
#[cfg(target_os = "linux")]
pub(crate) type Storage = crate::platform::linux::linux_account::LinuxAccountStorage;

pub(crate) type Session = BackendAccountSession<BackendAccountClient, Storage>;

pub struct AccountState {
    pub(crate) session: Arc<Session>,
}

/// Builds the backend client and registers the account state around the platform's session storage, plus the candidate-skin community service that authenticates through the same session.
pub(crate) fn manage(
    app: &tauri::AppHandle,
    storage: Storage,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = BackendAccountClient::new()?;
    let session = Arc::new(BackendAccountSession::new(client.clone(), storage));
    app.manage(CandidateSkinCommunityState {
        service: Arc::new(BackendCandidateSkinCommunityService::new(
            client,
            Arc::clone(&session),
        )),
    });
    app.manage(AccountState { session });
    Ok(())
}

#[tauri::command]
pub async fn account_status(
    state: tauri::State<'_, AccountState>,
) -> Result<StatusResponse, crate::CommandError> {
    call_session(&state.session, |session| {
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
    call_session(&state.session, |session| {
        session.providers().map(providers_response)
    })
    .await
}

#[tauri::command]
pub async fn account_request_code(
    state: tauri::State<'_, AccountState>,
    provider: String,
    target: String,
) -> Result<ChallengeResponse, crate::CommandError> {
    call_session(&state.session, move |session| {
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
    call_session(&state.session, move |session| {
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
    call_session(&state.session, |session| session.profile().map(Into::into)).await
}

#[tauri::command]
pub async fn account_rename(
    state: tauri::State<'_, AccountState>,
    display_name: String,
) -> Result<ProfileResponse, crate::CommandError> {
    call_session(&state.session, move |session| {
        session.rename(&display_name).map(Into::into)
    })
    .await
}

#[tauri::command]
pub async fn account_logout(
    state: tauri::State<'_, AccountState>,
    all: bool,
) -> Result<(), crate::CommandError> {
    call_session(&state.session, move |session| session.logout(all)).await
}

#[tauri::command]
pub async fn account_delete(
    state: tauri::State<'_, AccountState>,
) -> Result<(), crate::CommandError> {
    call_session(&state.session, |session| session.delete_account()).await
}

#[tauri::command]
pub async fn account_forget(
    state: tauri::State<'_, AccountState>,
) -> Result<(), crate::CommandError> {
    call_session(&state.session, |session| session.forget()).await
}
