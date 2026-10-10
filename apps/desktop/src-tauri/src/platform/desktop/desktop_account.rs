//! Desktop account commands shared by the macOS, Windows and Linux shells.
//!
//! The fourteen commands, their state and the blocking-call bridge are the same on all three desktop hosts, and so is the store: an owner-only `account-session.json` ([`FileAccountSessionStorage`]). Each platform module only chooses its directory and file layout in a thin `setup` that hands the store to [`manage`]: on macOS the input method's Application Support directory, shared with the input method, in the Swift backend's layout; on Windows the user's local application data; on Linux the shared state directory. The React surface only receives the same redacted DTOs as the mobile hosts; the tokens never leave this process.

use crate::platform::account_helpers::call_session;
use crate::platform::desktop::desktop_candidate_skin_community::CandidateSkinCommunityState;
use crate::platform::desktop::desktop_community_report::CommunityReportState;
use crate::platform::desktop::desktop_plugin_community::PluginCommunityState;
use crate::shared::account_dto::{
    providers_response, ChallengeResponse, ProfileResponse, ProvidersResponse, StatusResponse,
};
use msime_client_core::account::{
    AccountError, BackendAccountClient, BackendAccountSession, FileAccountSessionStorage,
};
use msime_client_core::community::report::BackendCommunityReportService;
use msime_client_core::plugins::community::BackendCommunityPluginService;
use msime_client_core::skin::candidate_community::BackendCandidateSkinCommunityService;
use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

pub(crate) type Storage = FileAccountSessionStorage;

pub(crate) type Session = BackendAccountSession<BackendAccountClient, Storage>;

pub struct AccountState {
    pub(crate) session: Arc<Session>,
}

/// Builds the backend client and registers the account state around the platform's session storage, plus the candidate-skin, plugin and report community services that authenticate through the same session.
pub(crate) fn manage(
    app: &tauri::AppHandle,
    storage: Storage,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = BackendAccountClient::new()?;
    let session = Arc::new(BackendAccountSession::new(client.clone(), storage));
    app.manage(CandidateSkinCommunityState {
        service: Arc::new(BackendCandidateSkinCommunityService::new(
            client.clone(),
            Arc::clone(&session),
        )),
    });
    app.manage(CommunityReportState {
        service: Arc::new(BackendCommunityReportService::new(
            client.clone(),
            Arc::clone(&session),
        )),
    });
    app.manage(PluginCommunityState {
        service: Arc::new(BackendCommunityPluginService::new(
            client,
            Arc::clone(&session),
        )),
    });
    // 社区词包与回复模板目前只在 Windows 的设置应用里提供；macOS 由输入法自己的账号窗口提供，Linux 还没有接入。
    #[cfg(target_os = "windows")]
    app.manage(crate::platform::community_resources::CommunityResourceState::new(&session)?);
    app.manage(AccountState { session });
    Ok(())
}

/// 会话换了目录以后接管旧目录里的会话：`directory` 还没有 `account-session.json` 而 `legacy` 里有时，把旧文件搬过去，用户不用重新登录。两个目录在同一个卷上，搬动是一次改名。搬不动时什么也不做，用户看到的是未登录，重新登录即可。
#[cfg(any(target_os = "windows", test))]
pub(crate) fn adopt_legacy_session(legacy: &std::path::Path, directory: &std::path::Path) {
    use msime_client_core::account::ACCOUNT_SESSION_FILE;
    let source = legacy.join(ACCOUNT_SESSION_FILE);
    let destination = directory.join(ACCOUNT_SESSION_FILE);
    let regular_file = |path: &std::path::Path| {
        std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
    };
    if legacy == directory
        || std::fs::symlink_metadata(&destination).is_ok()
        || !regular_file(&source)
        || crate::shared::atomic_file::create_directory_and_check(directory).is_err()
    {
        return;
    }
    let _ = std::fs::rename(&source, &destination);
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

/// Signs in with Google in the system browser. The session binds a loopback listener, the backend builds the authorization URL and later exchanges the code with its own PKCE verifier and client secret; this command only supplies the browser launch. It returns once the browser redirects back, the user denies access, [`account_google_cancel`] is called, or the wait runs out shortly before the backend challenge expires.
#[tauri::command]
pub async fn account_google_login(
    state: tauri::State<'_, AccountState>,
) -> Result<StatusResponse, crate::CommandError> {
    call_session(&state.session, |session| {
        session
            .sign_in_google_with_browser(|url| {
                crate::launch_external_url(url).map_err(|_| AccountError::Unavailable)
            })
            .map(|user| StatusResponse {
                user: Some(user.into()),
            })
    })
    .await
}

/// Ends a pending [`account_google_login`], which then fails with `account_cancelled`. The browser cannot tell the app that the user closed the Google tab, so the page calls this when the user gives up instead of waiting out the timeout.
#[tauri::command]
pub fn account_google_cancel(state: tauri::State<'_, AccountState>) {
    state.session.cancel_google_sign_in();
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

/// The signed-in user's avatar as a `data:` URL, because the page's content security policy loads no remote image. `None` when signed out or when the user has no avatar.
#[tauri::command]
pub async fn account_avatar(
    state: tauri::State<'_, AccountState>,
) -> Result<Option<String>, crate::CommandError> {
    call_session(&state.session, |session| {
        Ok(session.avatar()?.map(|image| image.data_url()))
    })
    .await
}

/// Ask for a PNG or JPEG with the platform's own open dialog and upload it as the user's avatar. The page never names a path. `None` when the user closes the dialog.
#[tauri::command]
pub async fn account_choose_avatar(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, AccountState>,
) -> Result<Option<ProfileResponse>, crate::CommandError> {
    call_session(&state.session, move |session| {
        // The dialog runs on the main thread; this worker only waits for the answer.
        let picked = app
            .dialog()
            .file()
            .set_parent(&window)
            .set_title("选择头像")
            .add_filter("图片", &["png", "jpg", "jpeg"])
            .blocking_pick_file();
        let Some(picked) = picked else {
            return Ok(None);
        };
        let path = picked.into_path().map_err(|_| AccountError::Invalid)?;
        session
            .upload_avatar(&path)
            .map(|profile| Some(profile.into()))
    })
    .await
}

/// Remove the uploaded avatar; the Google picture, if any, shows again.
#[tauri::command]
pub async fn account_remove_avatar(
    state: tauri::State<'_, AccountState>,
) -> Result<ProfileResponse, crate::CommandError> {
    call_session(&state.session, |session| {
        session.remove_avatar().map(Into::into)
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

#[cfg(test)]
mod tests {
    use super::adopt_legacy_session;
    use msime_client_core::account::ACCOUNT_SESSION_FILE;

    #[test]
    fn a_session_left_in_the_old_directory_moves_to_the_shared_one() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("app.msime.windows");
        let shared = root.path().join("MSIME").join("account");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join(ACCOUNT_SESSION_FILE), b"synthetic-session").unwrap();

        adopt_legacy_session(&legacy, &shared);

        assert_eq!(
            std::fs::read(shared.join(ACCOUNT_SESSION_FILE)).unwrap(),
            b"synthetic-session"
        );
        assert!(!legacy.join(ACCOUNT_SESSION_FILE).exists());
    }

    #[test]
    fn a_session_already_in_the_shared_directory_wins() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("app.msime.windows");
        let shared = root.path().join("MSIME").join("account");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(legacy.join(ACCOUNT_SESSION_FILE), b"synthetic-old").unwrap();
        std::fs::write(shared.join(ACCOUNT_SESSION_FILE), b"synthetic-current").unwrap();

        adopt_legacy_session(&legacy, &shared);

        assert_eq!(
            std::fs::read(shared.join(ACCOUNT_SESSION_FILE)).unwrap(),
            b"synthetic-current"
        );
        assert_eq!(
            std::fs::read(legacy.join(ACCOUNT_SESSION_FILE)).unwrap(),
            b"synthetic-old"
        );
    }

    #[test]
    fn without_an_old_session_nothing_is_created() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("app.msime.windows");
        let shared = root.path().join("MSIME").join("account");
        std::fs::create_dir_all(&legacy).unwrap();

        adopt_legacy_session(&legacy, &shared);

        assert!(!shared.exists());
    }
}
