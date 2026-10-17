//! Desktop account commands shared by the macOS, Windows and Linux shells.
//!
//! The fourteen commands, their state and the blocking-call bridge are the same on all three desktop hosts, and so is the store: an owner-only `account-session.json` ([`FileAccountSessionStorage`]). Each platform module only chooses its directory and file layout in a thin `setup` that hands the store to [`manage`]: on macOS the input method's Application Support directory, shared with the input method, in the Swift backend's layout; on Windows the user's local application data; on Linux the shared state directory. The React surface only receives the same redacted DTOs as the mobile hosts; the tokens never leave this process.

use crate::platform::account_helpers::{call_session, call_session_detailed};
use crate::platform::desktop::desktop_candidate_skin_community::CandidateSkinCommunityState;
use crate::platform::desktop::desktop_community_report::CommunityReportState;
use crate::platform::desktop::desktop_plugin_community::PluginCommunityState;
use crate::shared::account_dto::{
    providers_response, ChallengeResponse, ProfileResponse, ProvidersResponse, StatusResponse,
};
use msime_client_services::account::{
    client_agent, AccountError, AccountFailure, BackendAccountClient, BackendAccountSession,
    FileAccountSessionStorage,
};
use msime_client_services::community::report::BackendCommunityReportService;
use msime_client_services::plugins::community::BackendCommunityPluginService;
use msime_client_services::skin::candidate_community::BackendCandidateSkinCommunityService;
use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

pub(crate) type Storage = FileAccountSessionStorage;

pub(crate) type Session = BackendAccountSession<BackendAccountClient, Storage>;

pub struct AccountState {
    pub(crate) session: Arc<Session>,
    /// 会话文件所在的目录。云剪贴板配置了自建中继时，本机的中继密钥串也放在这里，与输入法读写同一份（见 `msime_client_services::clipboard_relay`）。
    pub(crate) directory: std::path::PathBuf,
    /// 账号请求用的 HTTP 客户端，带本机自报的 User-Agent；注销后删匿名账号的语音样本时，匿名会话也用它。
    pub(crate) client: BackendAccountClient,
}

/// 本机自报的 User-Agent（#8201 的 P11）：拼装和清洗在 client-services 的 [`client_agent`]，这里只交出平台、版本、设备型号和系统版本，读的是「关于」页系统信息用的那几项（macOS 的 `ProductVersion`，Linux 的 os-release 和 DMI）。服务端「我的设备」按登录时的 User-Agent 显示平台和设备。拼不出时为 `None`，客户端照旧用默认的 User-Agent。
fn desktop_client_agent(app: &tauri::AppHandle) -> Option<String> {
    use msime_client_core::host_surface::{HostCapabilities, HostPlatform};
    let platform = crate::host_platform();
    let name = match platform {
        HostPlatform::Macos => "macos",
        HostPlatform::Windows => "windows",
        HostPlatform::Linux => "linux",
        _ => return None,
    };
    let mut capabilities = HostCapabilities::for_platform(platform);
    capabilities.os_version =
        crate::macos_product_version().map(|version| format!("macOS {version}"));
    crate::fill_linux_environment(&mut capabilities);
    client_agent(
        name,
        &app.package_info().version.to_string(),
        capabilities.device_model.as_deref(),
        capabilities.os_version.as_deref(),
        &crate::package_edition().id,
    )
    .ok()
}

/// Builds the backend client and registers the account state around the platform's session storage, plus the candidate-skin, plugin and report community services that authenticate through the same session.
pub(crate) fn manage(
    app: &tauri::AppHandle,
    storage: Storage,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = match desktop_client_agent(app) {
        Some(agent) => BackendAccountClient::with_user_agent(&agent)?,
        None => BackendAccountClient::new()?,
    };
    let directory = storage.directory().to_path_buf();
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
            client.clone(),
            Arc::clone(&session),
        )),
    });
    // 社区词包与回复模板目前只在 Windows 的设置应用里提供；macOS 由输入法自己的账号窗口提供，Linux 还没有接入。
    #[cfg(target_os = "windows")]
    app.manage(crate::platform::community_resources::CommunityResourceState::new(&session)?);
    app.manage(AccountState {
        session,
        directory,
        client,
    });
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

/// 注销以后删匿名账号名下语音样本的结果，形状与 C 接口 `account_delete` 的 `voice_samples` 相同：删掉了（或本机从来没有匿名身份）是 `{ok: true}`，否则 `{ok: false, error, retry_after_seconds?}`。
#[derive(Debug, serde::Serialize)]
pub struct VoiceSamplesResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_after_seconds: Option<u64>,
}

impl From<Result<(), AccountFailure>> for VoiceSamplesResponse {
    fn from(result: Result<(), AccountFailure>) -> Self {
        match result {
            Ok(()) => Self {
                ok: true,
                error: None,
                retry_after_seconds: None,
            },
            Err(failure) => Self {
                ok: false,
                error: Some(failure.code()),
                retry_after_seconds: failure.retry_after_seconds,
            },
        }
    }
}

/// `voice_samples` 在 macOS 上是 `null`：Tauri 不持有 App Group 里的匿名会话，macOS 也从不做语音贡献。
#[derive(Debug, serde::Serialize)]
pub struct AccountDeleteResponse {
    voice_samples: Option<VoiceSamplesResponse>,
}

/// 注销账号，与 C 接口的 `account_delete` 相同：会话不是 10 分钟内登录的是 `account_recent_login_required`，本机登录留着，页面提示先重新登录。注销成功以后在 Windows、Linux 上尽力删掉本机匿名账号名下的语音样本（语音贡献只用匿名账号，#8201 的决定 12），这一步失败不让注销失败，结果在 `voice_samples` 里。
#[tauri::command]
pub async fn account_delete(
    state: tauri::State<'_, AccountState>,
) -> Result<AccountDeleteResponse, crate::CommandError> {
    let directory = state.directory.clone();
    let client = state.client.clone();
    call_session_detailed(&state.session, move |session| {
        session.delete_account_detailed()?;
        // macOS 上 Tauri 不持有 App Group 里的匿名会话，macOS 也从不做语音贡献。
        let voice_samples =
            (!cfg!(target_os = "macos")).then(|| anonymous_voice_samples(&directory, client));
        Ok(AccountDeleteResponse { voice_samples })
    })
    .await
}

/// 删掉 `directory` 里本机匿名账号名下的语音样本，见 [`delete_anonymous_voice_samples`](msime_client_services::account::delete_anonymous_voice_samples)。匿名会话必须用 [`LockedAnonymousSessionStorage`](msime_client_services::account::LockedAnonymousSessionStorage)：Server 和 Linux 的 provider 拿同一枚匿名刷新令牌，不在 `account-refresh.lock` 里各刷新一次的话，服务端见到用过的刷新令牌会吊销整个匿名会话。macOS 不调它，但各平台都编译，测试在哪里都跑得到。
fn anonymous_voice_samples(
    directory: &std::path::Path,
    client: BackendAccountClient,
) -> VoiceSamplesResponse {
    use msime_client_services::account::{
        delete_anonymous_voice_samples, AnonymousIdentitySource, LockedAnonymousSessionStorage,
    };
    let anonymous =
        BackendAccountSession::new(client, LockedAnonymousSessionStorage::new(directory));
    delete_anonymous_voice_samples(&anonymous, AnonymousIdentitySource::Directory(directory)).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletion_without_an_anonymous_identity_removes_nothing_and_succeeds() {
        let directory = tempfile::tempdir().unwrap();
        let response =
            anonymous_voice_samples(directory.path(), BackendAccountClient::new().unwrap());
        assert_eq!(
            serde_json::to_value(AccountDeleteResponse {
                voice_samples: Some(response)
            })
            .unwrap(),
            serde_json::json!({ "voice_samples": { "ok": true } })
        );
        // 没有身份时不联网，也不在目录里留下任何东西。
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[test]
    fn a_failed_voice_deletion_keeps_the_code_and_retry_hint() {
        let mut failure = AccountFailure::from(AccountError::RateLimited);
        failure.retry_after_seconds = Some(3600);
        assert_eq!(
            serde_json::to_value(VoiceSamplesResponse::from(Err(failure))).unwrap(),
            serde_json::json!({ "ok": false, "error": "account_rate_limited", "retry_after_seconds": 3600 })
        );
        assert_eq!(
            serde_json::to_value(AccountDeleteResponse {
                voice_samples: None
            })
            .unwrap(),
            serde_json::json!({ "voice_samples": null })
        );
    }
}

#[tauri::command]
pub async fn account_forget(
    state: tauri::State<'_, AccountState>,
) -> Result<(), crate::CommandError> {
    call_session(&state.session, |session| session.forget()).await
}
