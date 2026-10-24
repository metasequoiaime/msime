//! 原生设置窗口的账号操作：与 Tauri 桌面端的十四个账号命令（`desktop_account.rs`）和 Windows 与 Linux 共用的四个设置同步命令（`desktop_settings_sync.rs`）一一对应，用同一个 [`BackendAccountSession`] 和同一份本用户会话文件（[`FileAccountSessionStorage`]，`account-session.json`），返回的也是同样的脱敏形状（[`msime_client_services::account::page`]）。令牌只在本进程里，不出现在响应中。
//!
//! 每个会话目录（和宿主自报的 `client_agent`）在进程里只有一个会话对象，这样进行中的 Google 登录可以被另一次调用里的 `google_cancel` 结束；会话文件本身每次访问都重新读，共享应用、输入法和这里同时登录、刷新或退出时以文件为准，刷新时共用 `account-refresh.lock`。
//!
//! 请求带 `session_store` 而不带 `directory` 时，会话在宿主注入的存储里（[`super::session_store`]），会话对象是注册时建好的那一个。
//!
//! #8201 新增的操作（对话、释义、常用语、我的设备、云端数据、反馈、诊断上传、发送下载链接、语音贡献、删除语音样本和鸿蒙两个沙箱之间的会话镜像 `session_adopt`）在 [`crate::account_ops`]；已有的 `delete` 注销成功以后也经它删匿名账号的语音样本；它们失败时还可能带 `retry_after_seconds`。用到匿名账号的几项，按目录时用同一目录里的 `anonymous-session.json`（每个目录、每个 `client_agent` 一个会话对象，刷新时与登录会话共用 `account-refresh.lock`）和 `anonymous-account.json` 里的身份，注入时用该 store 的匿名会话和 `ANONYMOUS_IDENTITY` 项。
//!
//! Part of the C ABI; see the parent module for what these shims guarantee.

use super::session_store::{session_source, SessionStorage, Source};
use super::with_bounded_bytes;
use crate::*;
use msime_client_core::preferences::PreferencesError;
use msime_client_services::account::page::{
    providers_response, ChallengeResponse, PreferenceSchemaResponse, ProfileResponse,
    StatusResponse,
};
use msime_client_services::account::settings_sync::{
    prepare_desktop_settings_apply, upload_desktop_settings,
};
use msime_client_services::account::{
    adopt_legacy_account_session, client_agent, ensure_anonymous_account,
    ensure_anonymous_account_in, ensure_anonymous_account_with, valid_client_agent, AccountError,
    AccountPreferences, AccountSessionFileLayout, AnonymousIdentitySource, BackendAccountClient,
    BackendAccountSession, FileAccountSessionStorage, LockedAnonymousSessionStorage,
};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// 请求的上限：应用云端设置时请求里带着整份云端设置文档（其中可能有 Android 的自定义键盘皮肤库，最多 768 KiB），与 Android 的设置同步导出一样给 4 MiB；目录各最多 16 KiB。
const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_DIRECTORY_BYTES: usize = 16 * 1024;
/// `msime_client_ensure_anonymous_account_with_agent` 的请求上限：16 KiB 的目录加上 `client_agent` 和 JSON 本身。
const MAX_ENSURE_ANONYMOUS_REQUEST_BYTES: usize = 32 * 1024;
/// `msime_client_client_agent` 的请求上限。
const MAX_CLIENT_AGENT_REQUEST_BYTES: usize = 16 * 1024;

pub(super) type Session = BackendAccountSession<BackendAccountClient, SessionStorage>;

/// 会话对象的键：会话目录和宿主自报的 User-Agent（没给时为 `None`，发 `MSIME/Android`）。
type SessionKey = (PathBuf, Option<String>);

/// 每个会话目录、每个 `client_agent` 一个会话对象。同一目录的几个对象读写同一份会话文件，刷新时共用 `account-refresh.lock`，与几个进程共用一个目录相同。只在取对象时持锁，账号调用本身在锁外进行。
static ACCOUNT_SESSIONS: OnceLock<Mutex<HashMap<SessionKey, Arc<Session>>>> = OnceLock::new();

/// 每个会话目录、每个 `client_agent` 一个匿名会话对象（同一目录的 `anonymous-session.json`），给登录账号优先、退回匿名账号的新操作用。
static ANONYMOUS_SESSIONS: OnceLock<Mutex<HashMap<SessionKey, Arc<Session>>>> = OnceLock::new();

/// 目录模式里宿主自报的每个 User-Agent 一个 HTTP 客户端，整个进程共用：客户端里只有连接池和超时，没有账号状态（见 [`BackendAccountClient::new`] 共用的那一个），各建一份时每个都多占一个后台线程和几个文件描述符。
static AGENT_CLIENTS: OnceLock<Mutex<HashMap<String, BackendAccountClient>>> = OnceLock::new();

#[cfg(test)]
thread_local! {
    /// 测试里本线程上 [`directory_client`] 改连的本机假服务端（`http://127.0.0.1:<端口>`），每次都新建、不进 [`AGENT_CLIENTS`]。按线程设，并行跑的其他测试仍连不到它；`msime_client_account` 在调用线程上完成，所以经 C 入口的请求也走它。
    static TEST_ORIGIN: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountRequest {
    /// 共享应用登录时写 `account-session.json` 的目录；与 `session_store` 恰好给一个。
    #[serde(default)]
    directory: Option<String>,
    /// 旧版本放会话的目录；新目录还没有会话而这里有时，先把它搬过去（[`adopt_legacy_account_session`]）。只能与 `directory` 一起给。
    #[serde(default)]
    legacy_directory: Option<String>,
    /// `msime_client_session_store_register` 给的 id；与 `directory` 恰好给一个。
    #[serde(default)]
    session_store: Option<u64>,
    /// 宿主自报的 User-Agent，形状与注入存储的 `client_agent` 相同（[`valid_client_agent`]）；只能与 `directory` 一起给，注入存储用注册时给的那个。不给时发 `MSIME/Android`，与以前相同（#8201 的决定 1）。
    #[serde(default)]
    client_agent: Option<String>,
    action: Action,
}

/// 已有的操作（[`AccountAction`]）或 #8201 新增的（[`crate::account_ops::Operation`]）。两者都按 `operation` 区分、都拒绝未知字段，名字不重复，所以先试前者、不是它再试后者。
#[derive(Deserialize)]
#[serde(untagged)]
enum Action {
    Account(AccountAction),
    Extended(crate::account_ops::Operation),
}

/// Tauri 的 `account_*` 命令，名字去掉前缀；`choose_avatar` 的文件对话框由宿主自己弹，这里只收选好的路径。`preferences_*` 是 Windows 的设置同步，偏好目录由宿主传入（共享应用用的是它自己的偏好存储）。
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum AccountAction {
    Status,
    Providers,
    RequestCode {
        provider: String,
        target: String,
        #[serde(default)]
        purpose: Purpose,
    },
    Login {
        challenge_id: String,
        code: String,
        #[serde(default)]
        purpose: Purpose,
    },
    /// Windows 的浏览器登录和它的取消。写成空的结构体变体，理由与 `AppleChallenge {}` 相同：多带 `purpose` 之类字段的请求是 `account_invalid`，不会照样打开浏览器做一次普通登录。
    GoogleLogin {},
    GoogleCancel {},
    /// 宿主自己弹 Apple 登录界面（iOS、macOS 的 `ASAuthorizationAppleIDProvider`）：先要 challenge 和 nonce，再交回 identity token。写成空的结构体变体：内部标签的单元变体不检查多余字段，多带字段的请求会照样联网申请 challenge。
    AppleChallenge {},
    AppleLogin {
        challenge_id: String,
        identity_token: String,
    },
    /// 不带 `target` 是 ID token 流程（iOS 的 GoogleSignIn、安卓的 Credential Manager），返回 nonce；带 `target`（`msime_client_google_loopback` 的 `target` 给的回跳地址）是回环流程（鸿蒙），返回校验过的授权链接。
    GoogleChallenge {
        #[serde(default)]
        target: Option<String>,
        #[serde(default)]
        purpose: Purpose,
    },
    GoogleCodeLogin {
        challenge_id: String,
        code: String,
    },
    GoogleTokenLogin {
        challenge_id: String,
        id_token: String,
        #[serde(default)]
        purpose: Purpose,
    },
    /// 没有 Apple 原生登录的宿主（安卓）走 Apple 网页登录：`app` 是发起登录的 applicationId。
    AppleWebStart {
        app: String,
        #[serde(default)]
        purpose: Purpose,
    },
    AppleWebLogin {
        grant: String,
        code_verifier: String,
        #[serde(default)]
        purpose: Purpose,
    },
    Profile,
    Rename {
        display_name: String,
    },
    Avatar,
    UploadAvatar {
        path: String,
    },
    RemoveAvatar,
    Logout {
        all: bool,
    },
    Delete,
    Forget,
    PreferencesSchema,
    PreferencesLoad,
    PreferencesUpload {
        preferences_directory: String,
    },
    PreferencesApply {
        preferences_directory: String,
        user_id: String,
        preferences: AccountPreferences,
    },
    /// 本机匿名账号还没有会话时注册它，与 `msime_client_ensure_anonymous_account` 相同，但失败时返回账号错误码。
    EnsureAnonymous,
}

/// 邮箱、Google ID token 和 Apple 网页登录这几个操作的用途：缺省是登录；`link` 是给已登录的账号添加这种登录方式（#8201 的决定 9，Android 的「添加登录方式」），申请和完成两步都要带。
#[derive(Clone, Copy, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Purpose {
    #[default]
    Login,
    Link,
}

/// 宿主给的 `client_agent`：不合法时是 [`AccountError::Invalid`]，在用到目录之前就拒绝。
pub(super) fn checked_agent(agent: Option<&str>) -> Result<Option<&str>, AccountError> {
    match agent {
        Some(agent) if !valid_client_agent(agent) => Err(AccountError::Invalid),
        agent => Ok(agent),
    }
}

/// 目录模式的 HTTP 客户端：没有 `agent` 时是进程共用的默认客户端（`MSIME/Android`），有时是这个 User-Agent 的那一个，第一次用到时建立。
fn directory_client(agent: Option<&str>) -> Result<BackendAccountClient, AccountError> {
    #[cfg(test)]
    if let Some(origin) = TEST_ORIGIN.with_borrow(Clone::clone) {
        return BackendAccountClient::loopback_with_agent(
            &origin,
            agent.unwrap_or("MSIME/Android"),
        );
    }
    let Some(agent) = agent else {
        return BackendAccountClient::new();
    };
    let clients = AGENT_CLIENTS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut clients = clients.lock().map_err(|_| AccountError::Unavailable)?;
    if let Some(client) = clients.get(agent) {
        return Ok(client.clone());
    }
    let client = BackendAccountClient::with_user_agent(agent)?;
    clients.insert(agent.to_owned(), client.clone());
    Ok(client)
}

/// 目录对应的会话对象（默认 User-Agent），第一次用到时建立；建立前先接管旧目录里的会话，与共享应用启动时做的一样。社区操作（`msime_client_community`）经同一个对象认证。
pub(super) fn session_for(
    directory: &Path,
    legacy: Option<&Path>,
) -> Result<Arc<Session>, AccountError> {
    session_with_agent(directory, legacy, None)
}

/// 与 [`session_for`] 相同，只是请求以宿主自报的 `agent` 作 User-Agent（`agent` 已经过 [`checked_agent`]）；同一目录、同一个 `agent` 的请求用同一个对象。
pub(super) fn session_with_agent(
    directory: &Path,
    legacy: Option<&Path>,
    agent: Option<&str>,
) -> Result<Arc<Session>, AccountError> {
    let sessions = ACCOUNT_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut sessions = sessions.lock().map_err(|_| AccountError::Unavailable)?;
    let key = (directory.to_path_buf(), agent.map(str::to_owned));
    if let Some(session) = sessions.get(&key) {
        return Ok(Arc::clone(session));
    }
    if let Some(legacy) = legacy {
        adopt_legacy_account_session(legacy, directory);
    }
    let session = Arc::new(BackendAccountSession::new(
        directory_client(agent)?,
        SessionStorage::File(FileAccountSessionStorage::new(
            directory,
            AccountSessionFileLayout::Native,
        )),
    ));
    sessions.insert(key, Arc::clone(&session));
    Ok(session)
}

/// 目录里匿名会话的对象（默认 User-Agent），第一次用到时建立。
pub(super) fn anonymous_session_for(directory: &Path) -> Result<Arc<Session>, AccountError> {
    anonymous_session_with_agent(directory, None)
}

/// 与 [`anonymous_session_for`] 相同，只是请求（包括删除语音样本时匿名账号的重新登录）以 `agent` 作 User-Agent。
pub(super) fn anonymous_session_with_agent(
    directory: &Path,
    agent: Option<&str>,
) -> Result<Arc<Session>, AccountError> {
    let sessions = ANONYMOUS_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut sessions = sessions.lock().map_err(|_| AccountError::Unavailable)?;
    let key = (directory.to_path_buf(), agent.map(str::to_owned));
    if let Some(session) = sessions.get(&key) {
        return Ok(Arc::clone(session));
    }
    let session = Arc::new(BackendAccountSession::new(
        directory_client(agent)?,
        SessionStorage::AnonymousFile(LockedAnonymousSessionStorage::new(directory)),
    ));
    sessions.insert(key, Arc::clone(&session));
    Ok(session)
}

/// `msime_client_account` 的一次失败：错误码，以及新操作里服务端给了 `Retry-After` 时的秒数。已有操作的失败没有后者，响应与以前相同。
pub(super) struct Failure {
    pub(super) code: String,
    pub(super) retry_after_seconds: Option<u64>,
}

impl From<String> for Failure {
    fn from(code: String) -> Self {
        Self {
            code,
            retry_after_seconds: None,
        }
    }
}

impl From<crate::account_ops::OperationFailure> for Failure {
    fn from(failure: crate::account_ops::OperationFailure) -> Self {
        Self {
            code: failure.code.to_owned(),
            retry_after_seconds: failure.retry_after_seconds,
        }
    }
}

impl From<msime_client_services::account::AccountFailure> for Failure {
    fn from(failure: msime_client_services::account::AccountFailure) -> Self {
        crate::account_ops::OperationFailure::from(failure).into()
    }
}

/// 与 crate 的 `response` 相同，失败时多一个可选的 `retry_after_seconds`：`{"ok":false,"error":code,"retry_after_seconds":N}`，只在已知时出现。
pub(super) fn account_failure_response(
    operation: impl FnOnce() -> Result<Value, Failure>,
) -> *mut c_char {
    let value = match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(value)) => json!({ "ok": true, "value": value }),
        Ok(Err(Failure {
            code,
            retry_after_seconds: Some(seconds),
        })) => json!({ "ok": false, "error": code, "retry_after_seconds": seconds }),
        Ok(Err(Failure { code, .. })) => json!({ "ok": false, "error": code }),
        Err(_) => json!({ "ok": false, "error": "internal runtime failure" }),
    };
    // JSON 会转义 NUL，这里不会有内部的 NUL。
    CString::new(value.to_string())
        .expect("JSON contains no NUL")
        .into_raw()
}

fn value<T: Serialize>(value: T) -> Result<Value, AccountError> {
    serde_json::to_value(value).map_err(|_| AccountError::Unavailable)
}

fn status(
    user: Option<msime_client_services::account::AccountUser>,
) -> Result<Value, AccountError> {
    value(StatusResponse {
        user: user.map(Into::into),
    })
}

/// Apple 登录和 Google ID token 流程的 challenge：nonce 只交给宿主自己弹的登录界面，账号页的 `request_code` 不返回它。
fn nonce_challenge(
    challenge: msime_client_services::account::AccountChallenge,
) -> Result<Value, AccountError> {
    Ok(json!({
        "challengeId": challenge.challenge_id,
        "expiresIn": challenge.expires_in,
        "nonce": challenge.nonce,
    }))
}

/// 打开系统浏览器去 Google 授权页。链接已经由会话校验过，是指向本次回环监听的 Google 授权链接。
#[cfg(windows)]
fn open_browser(url: &str) -> Result<(), AccountError> {
    msime_host_windows::open_url(url)
        .then_some(())
        .ok_or(AccountError::Unavailable)
}

/// 只有 Windows 的原生设置窗口经这里登录 Google；其他宿主各有自己的浏览器入口（鸿蒙用 `msime_client_google_loopback`）。
#[cfg(not(windows))]
fn open_browser(_url: &str) -> Result<(), AccountError> {
    Err(AccountError::Unavailable)
}

/// 偏好存储的保存错误，与共享应用的 `CommandError::from(PreferencesError)` 对同一错误给的码相同：修订号不符是 `conflict`，其余是 `storage`（应用后的偏好已经校验过，校验类错误不会出现）。
fn preferences_error(error: PreferencesError) -> String {
    match error {
        PreferencesError::Conflict => "conflict",
        _ => "storage",
    }
    .to_owned()
}

/// 设置同步的应用：client-services 算出应用后的偏好和读到时的修订号，再按这个修订号写回，与共享应用的 `account_preferences_apply` 相同；关掉剪贴板历史时与 `msime_client_save_preferences` 一样清掉已存的历史。返回保存后的偏好快照，宿主用它重画页面。
fn apply_preferences(
    session: &Session,
    directory: &str,
    user_id: &str,
    preferences: AccountPreferences,
) -> Result<Value, String> {
    let store = PreferencesStore::new(absolute_directory(directory)?);
    let (revision, next) = prepare_desktop_settings_apply(
        session,
        &store,
        Some(store.edition()),
        user_id,
        preferences,
    )
    .map_err(|error| error.code().to_owned())?;
    let saved = store.save(revision, next).map_err(preferences_error)?;
    if !saved.preferences.clipboard_history {
        store
            .clear_disabled_clipboard_history()
            .map_err(preferences_error)?;
    }
    serde_json::to_value(saved).map_err(|_| "storage".to_owned())
}

fn apple_web_started(started: msime_client_services::account::AppleWebSignIn) -> Value {
    json!({
        "authorizationUrl": started.authorization_url,
        "expiresIn": started.expires_in,
        "codeVerifier": started.code_verifier,
    })
}

/// 给已登录账号添加登录方式的一步：失败时按 #8201 新操作的规则带细节（需要最近登录是 `account_recent_login_required`，断网是 `account_network`，限流带 `retry_after_seconds`）。
fn link(
    operation: impl FnOnce() -> Result<Value, msime_client_services::account::AccountFailure>,
) -> Result<Value, Failure> {
    operation().map_err(Failure::from)
}

/// 一次操作。`purpose:"link"` 的几步（给已登录账号添加登录方式）只用登录会话，在这里处理；其余见 [`run_other`]。
fn run(session: &Session, action: AccountAction) -> Result<Value, Failure> {
    match action {
        AccountAction::RequestCode {
            provider,
            target,
            purpose: Purpose::Link,
        } => link(|| {
            Ok(value(ChallengeResponse::from(
                session.link_request_code(&provider, &target)?,
            ))?)
        }),
        AccountAction::Login {
            challenge_id,
            code,
            purpose: Purpose::Link,
        } => link(|| Ok(status(Some(session.link_email(&challenge_id, &code)?))?)),
        // 绑定只走 ID token 流程（Android 的 Credential Manager），回环流程不能绑定。
        AccountAction::GoogleChallenge {
            target: Some(_),
            purpose: Purpose::Link,
        } => Err(AccountError::Invalid.code().to_owned().into()),
        AccountAction::GoogleChallenge {
            target: None,
            purpose: Purpose::Link,
        } => link(|| Ok(nonce_challenge(session.link_google_id_token_challenge()?)?)),
        AccountAction::GoogleTokenLogin {
            challenge_id,
            id_token,
            purpose: Purpose::Link,
        } => link(|| {
            Ok(status(Some(
                session.link_google_id_token(&challenge_id, &id_token)?,
            ))?)
        }),
        AccountAction::AppleWebStart {
            app,
            purpose: Purpose::Link,
        } => link(|| Ok(apple_web_started(session.link_apple_web_start(&app)?))),
        AccountAction::AppleWebLogin {
            grant,
            code_verifier,
            purpose: Purpose::Link,
        } => link(|| {
            Ok(status(Some(
                session.link_apple_web(&grant, &code_verifier)?,
            ))?)
        }),
        action => run_other(session, action).map_err(Failure::from),
    }
}

/// 不是绑定的操作。设置同步的上传和应用要读写宿主给的偏好目录，在这里处理；其余都只经会话，见 [`run_account`]。
fn run_other(session: &Session, action: AccountAction) -> Result<Value, String> {
    let code = |error: AccountError| error.code().to_owned();
    match action {
        AccountAction::PreferencesApply {
            preferences_directory,
            user_id,
            preferences,
        } => apply_preferences(session, &preferences_directory, &user_id, preferences),
        AccountAction::PreferencesUpload {
            preferences_directory,
        } => {
            let store = PreferencesStore::new(absolute_directory(&preferences_directory)?);
            value(upload_desktop_settings(session, &store, Some(store.edition())).map_err(code)?)
                .map_err(code)
        }
        action => run_account(session, action).map_err(code),
    }
}

/// 与 Tauri 命令一一对应、只经会话的操作，错误都是账号错误码。
fn run_account(session: &Session, action: AccountAction) -> Result<Value, AccountError> {
    match action {
        AccountAction::Status => status(session.status()?),
        AccountAction::Providers => value(providers_response(session.providers()?)),
        AccountAction::RequestCode {
            provider,
            target,
            purpose: Purpose::Login,
        } => value(ChallengeResponse::from(
            session.request_code(&provider, &target)?,
        )),
        AccountAction::Login {
            challenge_id,
            code,
            purpose: Purpose::Login,
        } => status(Some(session.sign_in(&challenge_id, &code)?)),
        AccountAction::GoogleLogin {} => {
            status(Some(session.sign_in_google_with_browser(open_browser)?))
        }
        AccountAction::GoogleCancel {} => {
            session.cancel_google_sign_in();
            Ok(Value::Null)
        }
        AccountAction::AppleChallenge {} => nonce_challenge(session.apple_challenge()?),
        AccountAction::AppleLogin {
            challenge_id,
            identity_token,
        } => status(Some(session.sign_in_apple(&challenge_id, &identity_token)?)),
        AccountAction::GoogleChallenge {
            target: None,
            purpose: Purpose::Login,
        } => nonce_challenge(session.google_id_token_challenge()?),
        AccountAction::GoogleChallenge {
            target: Some(target),
            purpose: Purpose::Login,
        } => {
            let challenge = session.google_loopback_challenge(&target)?;
            Ok(json!({
                "challengeId": challenge.challenge_id,
                "expiresIn": challenge.expires_in,
                "authorizationUrl": challenge.authorization_url,
            }))
        }
        AccountAction::GoogleCodeLogin { challenge_id, code } => {
            status(Some(session.sign_in_google(&challenge_id, &code)?))
        }
        AccountAction::GoogleTokenLogin {
            challenge_id,
            id_token,
            purpose: Purpose::Login,
        } => status(Some(
            session.sign_in_google_id_token(&challenge_id, &id_token)?,
        )),
        AccountAction::AppleWebStart {
            app,
            purpose: Purpose::Login,
        } => Ok(apple_web_started(session.apple_web_start(&app)?)),
        AccountAction::AppleWebLogin {
            grant,
            code_verifier,
            purpose: Purpose::Login,
        } => status(Some(session.sign_in_apple_web(&grant, &code_verifier)?)),
        AccountAction::Profile => value(ProfileResponse::from(session.profile()?)),
        AccountAction::Rename { display_name } => {
            value(ProfileResponse::from(session.rename(&display_name)?))
        }
        AccountAction::Avatar => value(session.avatar()?.map(|image| image.data_url())),
        // 相对路径、不是普通文件、超过 1 MiB 或不是 PNG/JPEG 时，读文件这一步就以 account_invalid 失败，不会上传。
        AccountAction::UploadAvatar { path } => value(ProfileResponse::from(
            session.upload_avatar(Path::new(&path))?,
        )),
        AccountAction::RemoveAvatar => value(ProfileResponse::from(session.remove_avatar()?)),
        AccountAction::Logout { all } => session.logout(all).map(|()| Value::Null),
        AccountAction::Delete => session.delete_account().map(|()| Value::Null),
        AccountAction::Forget => session.forget().map(|()| Value::Null),
        AccountAction::PreferencesSchema => {
            value(PreferenceSchemaResponse::from(session.preference_schema()?))
        }
        AccountAction::PreferencesLoad => value(session.preferences()?),
        // 要写本机偏好、错误码不全是账号错误码的两项由 run_other 处理，绑定登录方式的几步由 run 处理，要看会话来源的 ensure_anonymous 由导出函数处理，都不会到这里。
        AccountAction::PreferencesUpload { .. }
        | AccountAction::PreferencesApply { .. }
        | AccountAction::EnsureAnonymous
        | AccountAction::RequestCode {
            purpose: Purpose::Link,
            ..
        }
        | AccountAction::Login {
            purpose: Purpose::Link,
            ..
        }
        | AccountAction::GoogleChallenge {
            purpose: Purpose::Link,
            ..
        }
        | AccountAction::GoogleTokenLogin {
            purpose: Purpose::Link,
            ..
        }
        | AccountAction::AppleWebStart {
            purpose: Purpose::Link,
            ..
        }
        | AccountAction::AppleWebLogin {
            purpose: Purpose::Link,
            ..
        } => Err(AccountError::Invalid),
    }
}

/// 目录模式的匿名账号注册：没有 `agent` 时与 `msime_client_ensure_anonymous_account` 完全相同（已有会话时只读一次会话文件、不建 HTTP 客户端），有时经这个 User-Agent 的客户端注册。`agent` 已经过 [`checked_agent`]。
fn ensure_anonymous_in_directory(
    directory: &Path,
    agent: Option<&str>,
) -> Result<(), AccountError> {
    match agent {
        None => ensure_anonymous_account(directory),
        Some(agent) => directory_client(Some(agent))
            .and_then(|client| ensure_anonymous_account_with(client, directory)),
    }
}

/// `msime_client_ensure_anonymous_account_with_agent` 的请求。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnsureAnonymousRequest {
    directory: String,
    #[serde(default)]
    client_agent: Option<String>,
}

/// 与 `msime_client_ensure_anonymous_account` 相同，只是请求是 JSON `{"directory": 绝对路径, "client_agent": 可选}`，带 `client_agent` 时注册请求以它作 User-Agent（#8201 的决定 1 扩展到这里），服务端「我的设备」据此显示平台。校验与 `msime_client_account` 的 `client_agent` 相同，不合法是 `account_invalid`；不带时与 `msime_client_ensure_anonymous_account` 发的请求相同。失败时 error 是账号错误码，与 `msime_client_account` 的 `ensure_anonymous` 相同，不是旧入口那样的说明文字。
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_ensure_anonymous_account_with_agent(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        let invalid = || AccountError::Invalid.code().to_owned();
        // SAFETY: forwarded from the documented caller contract.
        let request: EnsureAnonymousRequest = unsafe {
            with_bounded_bytes(
                request,
                length,
                MAX_ENSURE_ANONYMOUS_REQUEST_BYTES,
                "account_invalid",
                |bytes| serde_json::from_slice(bytes).map_err(|_| invalid()),
            )?
        };
        let directory = absolute_directory(&request.directory)?;
        let agent = checked_agent(request.client_agent.as_deref()).map_err(|_| invalid())?;
        ensure_anonymous_in_directory(directory, agent)
            .map(|()| Value::Bool(true))
            .map_err(|error| error.code().to_owned())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountNameRequest {
    name: String,
    id: String,
}

/// 计算昵称草稿的规范化、合法性与默认显示名；不创建会话、不读文件、不联网。
///
/// # Safety
/// `request` 必须在本次调用期间指向 `length` 个可读字节；NULL 被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_account_name(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        let request: AccountNameRequest = unsafe {
            with_bounded_bytes(request, length, 16 * 1024, "account_invalid", |bytes| {
                serde_json::from_slice(bytes).map_err(|_| "account_invalid".to_owned())
            })?
        };
        Ok(json!(msime_client_services::account::name::present_name(
            &request.name,
            &request.id,
        )))
    })
}

/// `msime_client_client_agent` 的请求。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClientAgentRequest {
    platform: String,
    version: String,
    #[serde(default)]
    device: Option<String>,
    #[serde(default)]
    system: Option<String>,
    edition: String,
}

/// 拼出宿主自报的 User-Agent（#8201 迁移计划 P11），规则在 client-services 的 `account::client_agent`，用例在 `shared/contracts/client-agent/cases.json`。不联网、不读文件。
///
/// # Safety
/// `request` 指向 `length` 个可读的 UTF-8 JSON 字节，NULL 被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_client_agent(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        let invalid = || AccountError::Invalid.code().to_owned();
        // SAFETY: 由调用方契约保证。
        let request: ClientAgentRequest = unsafe {
            with_bounded_bytes(
                request,
                length,
                MAX_CLIENT_AGENT_REQUEST_BYTES,
                "account_invalid",
                |bytes| serde_json::from_slice(bytes).map_err(|_| invalid()),
            )?
        };
        client_agent(
            &request.platform,
            &request.version,
            request.device.as_deref(),
            request.system.as_deref(),
            &request.edition,
        )
        .map(Value::String)
        .map_err(|error| error.code().to_owned())
    })
}

pub(super) fn absolute_directory(value: &str) -> Result<&Path, String> {
    let path = Path::new(value);
    if value.len() > MAX_DIRECTORY_BYTES || !path.is_absolute() {
        return Err(AccountError::Invalid.code().to_owned());
    }
    Ok(path)
}

/// 原生设置窗口的一次账号操作。请求、各操作的返回值和错误码见头文件。
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_account(request: *const u8, length: usize) -> *mut c_char {
    account_failure_response(|| {
        let invalid = || Failure::from(AccountError::Invalid.code().to_owned());
        // SAFETY: forwarded from the documented caller contract.
        let request: AccountRequest = unsafe {
            with_bounded_bytes(
                request,
                length,
                MAX_REQUEST_BYTES,
                "account_invalid",
                |bytes| serde_json::from_slice(bytes).map_err(|_| "account_invalid".to_owned()),
            )?
        };
        let code = |error: AccountError| Failure::from(error.code().to_owned());
        let source = session_source(
            request.directory.as_deref(),
            request.legacy_directory.is_some(),
            request.session_store,
        )
        .ok_or_else(invalid)?;
        match source {
            Source::Directory(directory) => {
                let directory = absolute_directory(directory)?;
                let legacy = request
                    .legacy_directory
                    .as_deref()
                    .map(absolute_directory)
                    .transpose()?;
                let agent = checked_agent(request.client_agent.as_deref()).map_err(code)?;
                if matches!(
                    request.action,
                    Action::Account(AccountAction::EnsureAnonymous)
                ) {
                    return ensure_anonymous_in_directory(directory, agent)
                        .map(|()| Value::Bool(true))
                        .map_err(code);
                }
                let session = session_with_agent(directory, legacy, agent).map_err(code)?;
                match request.action {
                    // 注销成功以后尽力删掉本机匿名账号的语音样本（#8201 的决定 12 后续 2），结果在 voice_samples 里，不影响注销本身。
                    Action::Account(AccountAction::Delete) => {
                        run(&session, AccountAction::Delete)?;
                        Ok(crate::account_ops::account_deleted(
                            || anonymous_session_with_agent(directory, agent),
                            AnonymousIdentitySource::Directory(directory),
                        ))
                    }
                    Action::Account(action) => run(&session, action),
                    Action::Extended(operation) => crate::account_ops::run(
                        &session,
                        || anonymous_session_with_agent(directory, agent),
                        AnonymousIdentitySource::Directory(directory),
                        operation,
                    )
                    .map_err(Failure::from),
                }
            }
            // 注入存储的 User-Agent 是注册时给的那个。
            Source::Store(_) if request.client_agent.is_some() => Err(invalid()),
            Source::Store(entry) => match request.action {
                Action::Account(AccountAction::EnsureAnonymous) => {
                    ensure_anonymous_account_in(&entry.anonymous, entry.backend.as_ref())
                        .map(|()| Value::Bool(true))
                        .map_err(code)
                }
                Action::Account(AccountAction::Delete) => {
                    run(&entry.account, AccountAction::Delete)?;
                    Ok(crate::account_ops::account_deleted(
                        || Ok(Arc::clone(&entry.anonymous)),
                        AnonymousIdentitySource::Store(entry.backend.as_ref()),
                    ))
                }
                Action::Account(action) => run(&entry.account, action),
                Action::Extended(operation) => crate::account_ops::run(
                    &entry.account,
                    || Ok(Arc::clone(&entry.anonymous)),
                    AnonymousIdentitySource::Store(entry.backend.as_ref()),
                    operation,
                )
                .map_err(Failure::from),
            },
        }
    })
}

#[cfg(test)]
mod client_agent_assembly_tests;
#[cfg(test)]
mod client_agent_tests;
#[cfg(test)]
mod link_tests;
#[cfg(test)]
mod session_adopt_tests;
#[cfg(test)]
mod voice_samples_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_services::account::{
        AccountSessionStorage, AccountTokens, AccountUser, SavedAccountSession,
        ACCOUNT_SESSION_FILE,
    };
    use std::ffi::CStr;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn call(request: Value) -> Value {
        let bytes = request.to_string();
        // SAFETY: the buffer is live for the call, and the returned string is freed below.
        unsafe {
            let raw = msime_client_account(bytes.as_ptr(), bytes.len());
            let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
            msime_client_string_free(raw);
            serde_json::from_str(&text).unwrap()
        }
    }

    fn action(directory: &Path, action: Value) -> Value {
        call(json!({ "directory": directory, "action": action }))
    }

    /// 写一份未过期的合成会话，访问令牌在有效期内，读状态和清除都不需要联网。
    fn save_session(directory: &Path) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Native)
            .save(&SavedAccountSession {
                tokens: AccountTokens {
                    access_token: "a".repeat(64),
                    refresh_token: "b".repeat(64),
                    token_type: "Bearer".into(),
                    expires_in: 3600,
                    user: AccountUser {
                        id: "synthetic-user".into(),
                        display_name: "合成用户".into(),
                        created_at: "2026-01-01T00:00:00Z".into(),
                        email: Some("person@example.test".into()),
                        avatar_url: Some("https://media.msime.app/avatars/abc.jpg".into()),
                    },
                },
                expires_at_unix_ms: now + 3_600_000,
                session_id: None,
            })
            .unwrap();
    }

    mod injected {
        use super::super::super::session_store::tests::{
            leaked_store, registered, session_bytes, unregister, TestStore,
        };
        use super::*;
        use std::sync::atomic::Ordering;

        const SHARED: u32 = 1;

        fn store_action(id: u64, action: Value) -> Value {
            call(json!({ "session_store": id, "action": action }))
        }

        fn access_token(request: Value) -> Value {
            let bytes = request.to_string();
            // SAFETY: 缓冲区在调用期间有效，返回的字符串在下面释放。
            unsafe {
                let raw = msime_client_account_access_token(bytes.as_ptr(), bytes.len());
                let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
                msime_client_string_free(raw);
                serde_json::from_str(&text).unwrap()
            }
        }

        fn cloud_clipboard(request: Value) -> Value {
            let bytes = request.to_string();
            // SAFETY: 同上。
            unsafe {
                let raw = msime_client_account_cloud_clipboard(bytes.as_ptr(), bytes.len());
                let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
                msime_client_string_free(raw);
                serde_json::from_str(&text).unwrap()
            }
        }

        fn community(request: Value) -> Value {
            let bytes = request.to_string();
            // SAFETY: 同上。
            unsafe {
                let raw =
                    crate::ffi::community::msime_client_community(bytes.as_ptr(), bytes.len());
                let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
                msime_client_string_free(raw);
                serde_json::from_str(&text).unwrap()
            }
        }

        fn put(store: &TestStore, item: u32, bytes: Vec<u8>) {
            store.items.lock().unwrap().insert(item, bytes);
        }

        #[test]
        fn status_forget_and_tokens_go_through_the_host_store() {
            let store = leaked_store();
            let id = registered(store, SHARED, 0);
            assert_eq!(
                store_action(id, json!({ "operation": "status" })),
                json!({ "ok": true, "value": { "user": null } })
            );
            put(store, 1, session_bytes("synthetic-user", 'a', true));
            put(store, 2, b"v1.synthetic-keystore".to_vec());
            assert_eq!(
                store_action(id, json!({ "operation": "status" }))["value"]["user"]["id"],
                "synthetic-user"
            );
            assert_eq!(
                access_token(json!({ "session_store": id })),
                json!({ "ok": true, "value": {
                    "access_token": "a".repeat(64),
                    "user_id": "synthetic-user",
                    "anonymous": false,
                } })
            );
            // 会话已有登录 ID、令牌也没过期：取令牌不加锁、不写入。
            assert!(store.locks.lock().unwrap().is_empty());
            assert!(store.saves.lock().unwrap().is_empty());

            // 清掉本地会话在 ACCOUNT 锁里进行，密钥串一起删掉。
            assert_eq!(
                store_action(id, json!({ "operation": "forget" })),
                json!({ "ok": true, "value": null })
            );
            assert!(store.items.lock().unwrap().is_empty());
            let locks = store.locks.lock().unwrap().clone();
            assert_eq!(locks.len(), 1);
            assert_eq!(locks[0].0, 1);
            assert_eq!(*store.unlocks.lock().unwrap(), locks);

            // 没有登录时用匿名会话；两个都没有时是 account_unauthorized。
            assert_eq!(
                access_token(json!({ "session_store": id }))["error"],
                "account_unauthorized"
            );
            put(store, 3, session_bytes("synthetic-anonymous", 'c', true));
            let token = access_token(json!({ "session_store": id }));
            assert_eq!(token["value"]["user_id"], "synthetic-anonymous");
            assert_eq!(token["value"]["anonymous"], true);

            // 其他只经会话的操作在发请求前就结束，与按目录时相同。
            assert_eq!(
                store_action(id, json!({ "operation": "profile" }))["error"],
                "account_unauthorized"
            );
            assert_eq!(
                cloud_clipboard(json!({
                    "session_store": id,
                    "action": { "operation": "list", "search": "" },
                }))["error"],
                "account_unauthorized"
            );
            let skins = tempfile::tempdir().unwrap();
            assert_eq!(
                community(json!({
                    "session_store": id,
                    "skins_directory": skins.path(),
                    "action": { "operation": "candidate_skin_sync" },
                }))["error"],
                "community_unauthorized"
            );
            assert_eq!(
                store_action(id, json!({ "operation": "google_cancel" })),
                json!({ "ok": true, "value": null })
            );

            assert_eq!(unregister(id)["value"], true);
            assert_eq!(store.releases.load(Ordering::SeqCst), 1);
            assert_eq!(
                store_action(id, json!({ "operation": "status" })),
                json!({ "ok": false, "error": "account_invalid" })
            );
        }

        #[test]
        fn every_shared_layout_case_reads_through_a_host_store() {
            // 共享用例 `shared/contracts/account-session-layouts/cases.json`：Swift、Rust、鸿蒙旧版 ArkTS 和 Android 原来的 Java 写的真实字节，经 C 接口注册的宿主存储交来，status 认出用例里的账号；Apple 布局的用例按 layout 1 注册，其余按 layout 0。读不出的是 account_storage。
            let contract: Value = serde_json::from_str(include_str!(
                "../../../../shared/contracts/account-session-layouts/cases.json"
            ))
            .unwrap();
            for case in contract["cases"].as_array().unwrap() {
                let name = case["name"].as_str().unwrap();
                let store = leaked_store();
                let id = registered(store, SHARED, u32::from(case["layout"] == "apple"));
                put(
                    store,
                    1,
                    case["document"].as_str().unwrap().as_bytes().to_vec(),
                );
                let status = store_action(id, json!({ "operation": "status" }));
                if let Some(code) = case["error"].as_str() {
                    assert_eq!(status, json!({ "ok": false, "error": code }), "{name}");
                } else {
                    let user = &case["session"]["tokens"]["user"];
                    assert_eq!(status["value"]["user"]["id"], user["id"], "{name}");
                    assert_eq!(
                        status["value"]["user"].get("email"),
                        user.get("email"),
                        "{name}"
                    );
                }
                unregister(id);
            }
        }

        #[test]
        fn an_unshared_store_is_read_once_and_a_shared_one_on_every_call() {
            for (flags, expected) in [(0, 1), (SHARED, 3)] {
                let store = leaked_store();
                let id = registered(store, flags, 0);
                put(store, 1, session_bytes("synthetic-user", 'a', true));
                for _ in 0..3 {
                    assert_eq!(
                        store_action(id, json!({ "operation": "status" }))["value"]["user"]["id"],
                        "synthetic-user"
                    );
                }
                assert_eq!(
                    store.loads.load(Ordering::SeqCst),
                    expected,
                    "flags {flags}"
                );
                unregister(id);
            }
        }

        #[test]
        fn a_session_without_a_login_id_is_migrated_under_the_host_lock() {
            let store = leaked_store();
            let id = registered(store, SHARED, 0);
            put(store, 1, session_bytes("synthetic-user", 'a', false));
            assert_eq!(
                access_token(json!({ "session_store": id }))["value"]["access_token"],
                "a".repeat(64)
            );
            assert_eq!(*store.saves.lock().unwrap(), [1]);
            let saved: serde_json::Value =
                serde_json::from_slice(&store.items.lock().unwrap()[&1]).unwrap();
            assert!(saved["session_id"].is_string());
            let locks = store.locks.lock().unwrap().clone();
            assert_eq!(locks.len(), 1);
            assert_eq!(locks[0].0, 1);
            assert_eq!(*store.unlocks.lock().unwrap(), locks);
            unregister(id);
        }

        #[test]
        fn the_host_lock_and_save_codes_reach_the_caller_as_account_codes() {
            // 补登录 ID 要持 ACCOUNT 锁再写回，不联网；锁和写入的返回码原样映射到 C 的错误码。
            for (lock, save, expected) in [
                (Some(1), None, "account_unavailable"),
                (Some(2), None, "account_storage"),
                (Some(-1), None, "account_storage"),
                (None, Some(1), "account_storage"),
                (None, Some(-1), "account_storage"),
            ] {
                let store = leaked_store();
                let id = registered(store, SHARED, 0);
                put(store, 1, session_bytes("synthetic-user", 'a', false));
                *store.lock_result.lock().unwrap() = lock;
                *store.save_result.lock().unwrap() = save;
                assert_eq!(
                    access_token(json!({ "session_store": id })),
                    json!({ "ok": false, "error": expected }),
                    "lock {lock:?} save {save:?}"
                );
                // 拿到锁的那次才解锁，而且恰好一次。
                let locks = store.locks.lock().unwrap().clone();
                assert_eq!(locks.len(), usize::from(lock.is_none()));
                assert_eq!(*store.unlocks.lock().unwrap(), locks);
                unregister(id);
            }
        }

        #[test]
        fn a_callback_that_reenters_the_same_store_is_refused_with_account_unavailable() {
            let store = leaked_store();
            let id = registered(store, SHARED, 0);
            put(store, 1, session_bytes("synthetic-user", 'a', false));
            let nested = Arc::new(Mutex::new(None));
            let seen = Arc::clone(&nested);
            *store.on_lock.lock().unwrap() = Some(Box::new(move || {
                *seen.lock().unwrap() = Some(access_token(json!({ "session_store": id })));
            }));
            let outer = access_token(json!({ "session_store": id }));
            assert_eq!(outer["value"]["access_token"], "a".repeat(64), "{outer}");
            // 回调里的那次在同一线程上又要这个 store 的锁：不调宿主的 lock、不死锁，直接报错。
            assert_eq!(
                nested.lock().unwrap().take(),
                Some(json!({ "ok": false, "error": "account_unavailable" }))
            );
            assert_eq!(store.locks.lock().unwrap().len(), 1);
            assert_eq!(store.unlocks.lock().unwrap().len(), 1);
            unregister(id);
        }

        #[test]
        fn every_entry_needs_exactly_one_session_source() {
            let store = leaked_store();
            let id = registered(store, 0, 0);
            let root = tempfile::tempdir().unwrap();
            let directory = root.path().join("account");
            let status = json!({ "operation": "status" });
            for request in [
                json!({ "directory": directory, "session_store": id, "action": status }),
                json!({ "action": status }),
                json!({ "session_store": u64::MAX, "action": status }),
                json!({ "session_store": id, "legacy_directory": directory, "action": status }),
                json!({ "session_store": "1", "action": status }),
            ] {
                assert_eq!(
                    call(request.clone()),
                    json!({ "ok": false, "error": "account_invalid" }),
                    "{request}"
                );
            }
            let list = json!({ "operation": "list", "search": "" });
            for request in [
                json!({ "directory": directory, "session_store": id, "action": list }),
                json!({ "action": list }),
                json!({ "session_store": u64::MAX, "action": list }),
            ] {
                assert_eq!(cloud_clipboard(request)["error"], "invalid_cloud_clipboard");
            }
            for request in [
                json!({ "directory": directory, "session_store": id }),
                json!({}),
                json!({ "session_store": u64::MAX }),
            ] {
                assert_eq!(access_token(request)["error"], "account_invalid");
            }
            let report = json!({ "operation": "candidate_skin_sync" });
            for request in [
                json!({ "directory": directory, "session_store": id, "action": report }),
                json!({ "action": report }),
                json!({ "session_store": u64::MAX, "action": report }),
            ] {
                assert_eq!(community(request)["error"], "community_invalid");
            }
            // 什么都没写进宿主的存储，也没有建目录。
            assert!(store.items.lock().unwrap().is_empty());
            assert!(!directory.exists());
            unregister(id);
        }

        #[test]
        fn new_operations_pick_the_stores_sessions() {
            let store = leaked_store();
            let id = registered(store, SHARED, 0);
            // 两个会话都没有：只用登录账号的、登录优先退回匿名的都在取令牌时结束。
            for operation in [
                json!({ "operation": "phrases_load" }),
                json!({ "operation": "translate", "texts": ["词"], "source_lang": "zh", "target_lang": "en" }),
            ] {
                assert_eq!(
                    store_action(id, operation.clone()),
                    json!({ "ok": false, "error": "account_unauthorized" }),
                    "{operation}"
                );
            }
            // 上传诊断时的账号已经不是 store 里的任何一个：报错，不退回另一个账号，也不联网。
            put(store, 1, session_bytes("synthetic-user", 'a', true));
            put(store, 3, session_bytes("synthetic-anonymous", 'c', true));
            for operation in [
                "diagnostics_state",
                "diagnostics_regenerate_token",
                "diagnostics_delete",
            ] {
                assert_eq!(
                    store_action(
                        id,
                        json!({ "operation": operation, "user_id": "someone-else" })
                    ),
                    json!({ "ok": false, "error": "account_identity_changed" }),
                    "{operation}"
                );
            }
            unregister(id);
        }

        #[test]
        fn signed_out_community_and_ai_skin_requests_read_the_stores_anonymous_session() {
            let store = leaked_store();
            let id = registered(store, SHARED, 0);
            // store 里只有一份解不开的匿名会话：要是只看登录会话，下面几项会以未登录结束；它们读了匿名会话，在发请求之前就以存储错误结束。
            put(store, 3, b"not a session".to_vec());
            let item = "10000000-0000-4000-8000-000000000001";
            for action in [
                json!({ "operation": "keyboard_skin_download", "id": item }),
                json!({ "operation": "report", "kind": "skins", "id": item, "reason": "其他" }),
            ] {
                assert_eq!(
                    community(json!({ "session_store": id, "action": action })),
                    json!({ "ok": false, "error": "community_storage" }),
                    "{action}"
                );
            }
            // 只认登录账号的社区操作不看匿名会话。
            assert_eq!(
                community(
                    json!({ "session_store": id, "action": { "operation": "keyboard_skin_rate", "id": item, "stars": 5 } })
                ),
                json!({ "ok": false, "error": "community_unauthorized" })
            );
            let request = json!({ "session_store": id, "action": { "operation": "create", "prompt": "合成插画" } }).to_string();
            // SAFETY: 缓冲区在调用期间有效，返回的字符串在下面释放。
            let job = unsafe {
                let raw =
                    crate::ffi::ai_skin::msime_client_ai_skin_job(request.as_ptr(), request.len());
                let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
                msime_client_string_free(raw);
                serde_json::from_str::<Value>(&text).unwrap()
            };
            assert_eq!(job, json!({ "ok": false, "error": "account_storage" }));
            unregister(id);
        }

        #[test]
        fn host_driven_sign_ins_go_through_the_stores_session() {
            let store = leaked_store();
            let id = registered(store, SHARED, 0);
            for request in host_sign_in_requests() {
                assert_eq!(
                    store_action(id, request.clone()),
                    json!({ "ok": false, "error": "account_invalid" }),
                    "{request}"
                );
            }
            // 被拒的登录不碰 store 里原有的会话。
            put(store, 1, session_bytes("synthetic-user", 'a', true));
            for request in host_sign_in_requests() {
                store_action(id, request);
            }
            assert_eq!(
                store_action(id, json!({ "operation": "status" }))["value"]["user"]["id"],
                "synthetic-user"
            );
            unregister(id);
        }
    }

    /// 宿主自己弹登录界面的几种登录里，在联网之前就该以 account_invalid 拒绝的请求。
    fn host_sign_in_requests() -> Vec<Value> {
        let grant = "g".repeat(42) + "A";
        let verifier = "v".repeat(43);
        let id_token = "eyJhbGciOiJSUzI1NiJ9.eyJub25jZSI6ImZpeHR1cmUifQ.c2lnbmF0dXJl";
        vec![
            json!({ "operation": "apple_challenge", "extra": 1 }),
            json!({ "operation": "apple_login", "challenge_id": "synthetic", "identity_token": "" }),
            json!({ "operation": "apple_login", "challenge_id": "", "identity_token": id_token }),
            json!({ "operation": "apple_login", "challenge_id": "synthetic", "code": "123456" }),
            json!({ "operation": "google_challenge", "target": "" }),
            json!({ "operation": "google_challenge", "target": "http://127.0.0.1:80/callback" }),
            json!({ "operation": "google_challenge", "target": "https://127.0.0.1:53682/callback" }),
            json!({ "operation": "google_code_login", "challenge_id": "synthetic", "code": "4/0A code" }),
            json!({ "operation": "google_code_login", "challenge_id": "synthetic", "code": "" }),
            json!({ "operation": "google_token_login", "challenge_id": "synthetic", "id_token": "4/0Afixture-code" }),
            json!({ "operation": "google_token_login", "challenge_id": "", "id_token": id_token }),
            json!({ "operation": "apple_web_start", "app": "App.Msime.Android" }),
            json!({ "operation": "apple_web_start" }),
            json!({ "operation": "apple_web_login", "grant": "g".repeat(42) + "h", "code_verifier": verifier }),
            json!({ "operation": "apple_web_login", "grant": grant, "code_verifier": "short" }),
        ]
    }

    /// 合格的请求解析成对应的操作（不执行，执行要联网）；头文件写的字段名与这里一致。
    #[test]
    fn host_driven_sign_in_requests_parse_into_their_operations() {
        let parse = |value: Value| serde_json::from_value::<AccountAction>(value).unwrap();
        assert!(matches!(
            parse(json!({ "operation": "apple_challenge" })),
            AccountAction::AppleChallenge {}
        ));
        assert!(matches!(
            parse(json!({ "operation": "apple_login", "challenge_id": "c", "identity_token": "t" })),
            AccountAction::AppleLogin { challenge_id, identity_token } if challenge_id == "c" && identity_token == "t"
        ));
        assert!(matches!(
            parse(json!({ "operation": "google_challenge" })),
            AccountAction::GoogleChallenge {
                target: None,
                purpose: Purpose::Login
            }
        ));
        assert!(matches!(
            parse(json!({ "operation": "google_challenge", "target": "http://127.0.0.1:53682/callback" })),
            AccountAction::GoogleChallenge { target: Some(target), purpose: Purpose::Login } if target == "http://127.0.0.1:53682/callback"
        ));
        assert!(matches!(
            parse(json!({ "operation": "google_code_login", "challenge_id": "c", "code": "k" })),
            AccountAction::GoogleCodeLogin { challenge_id, code } if challenge_id == "c" && code == "k"
        ));
        assert!(matches!(
            parse(json!({ "operation": "google_token_login", "challenge_id": "c", "id_token": "t" })),
            AccountAction::GoogleTokenLogin { challenge_id, id_token, purpose: Purpose::Login } if challenge_id == "c" && id_token == "t"
        ));
        assert!(matches!(
            parse(json!({ "operation": "apple_web_start", "app": "app.msime.android" })),
            AccountAction::AppleWebStart { app, purpose: Purpose::Login } if app == "app.msime.android"
        ));
        assert!(matches!(
            parse(json!({ "operation": "apple_web_login", "grant": "g", "code_verifier": "v" })),
            AccountAction::AppleWebLogin { grant, code_verifier, purpose: Purpose::Login } if grant == "g" && code_verifier == "v"
        ));
        // 已有的 google_login 仍是不带参数的 Windows 浏览器流程。
        assert!(matches!(
            parse(json!({ "operation": "google_login" })),
            AccountAction::GoogleLogin {}
        ));
        assert!(matches!(
            parse(json!({ "operation": "google_cancel" })),
            AccountAction::GoogleCancel {}
        ));
    }

    #[test]
    fn host_driven_sign_ins_check_their_credentials_before_the_network() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("account");
        for request in host_sign_in_requests() {
            assert_eq!(
                action(&directory, request.clone()),
                json!({ "ok": false, "error": "account_invalid" }),
                "{request}"
            );
        }
        // 被拒的登录不写会话文件。
        assert!(!directory.join(ACCOUNT_SESSION_FILE).exists());
    }

    #[test]
    fn status_reads_the_shared_session_file_without_its_tokens() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("account");
        assert_eq!(
            action(&directory, json!({ "operation": "status" })),
            json!({ "ok": true, "value": { "user": null } })
        );

        // 共享应用在这次调用之后登录：同一个会话对象下一次读到的就是新文件。
        save_session(&directory);
        let signed_in = action(&directory, json!({ "operation": "status" }));
        assert_eq!(
            signed_in,
            json!({ "ok": true, "value": { "user": {
                "id": "synthetic-user",
                "displayName": "合成用户",
                "preferredName": "合成用户",
                "createdAt": "2026-01-01T00:00:00Z",
                "email": "person@example.test",
                "avatarUrl": "https://media.msime.app/avatars/abc.jpg",
                "avatarUploaded": true,
            } } })
        );
        let text = signed_in.to_string();
        assert!(!text.contains(&"a".repeat(64)) && !text.contains(&"b".repeat(64)));

        // 清除失效会话只删本地文件，不联网。
        assert_eq!(
            action(&directory, json!({ "operation": "forget" })),
            json!({ "ok": true, "value": null })
        );
        assert!(!directory.join(ACCOUNT_SESSION_FILE).exists());
        assert_eq!(
            action(&directory, json!({ "operation": "status" }))["value"],
            json!({ "user": null })
        );
    }

    #[test]
    fn a_session_in_the_legacy_directory_is_adopted_first() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("app.msime.windows");
        let directory = root.path().join("MSIME").join("account");
        save_session(&legacy);

        let signed_in = call(json!({
            "directory": directory,
            "legacy_directory": legacy,
            "action": { "operation": "status" },
        }));
        assert_eq!(signed_in["value"]["user"]["id"], "synthetic-user");
        assert!(directory.join(ACCOUNT_SESSION_FILE).exists());
        assert!(!legacy.join(ACCOUNT_SESSION_FILE).exists());
    }

    #[test]
    fn bad_requests_fail_before_the_network() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("account");
        let invalid = json!({ "ok": false, "error": "account_invalid" });
        assert_eq!(
            call(json!({ "directory": "relative", "action": { "operation": "status" } })),
            invalid
        );
        assert_eq!(
            call(json!({
                "directory": directory,
                "legacy_directory": "relative",
                "action": { "operation": "status" },
            })),
            invalid
        );
        assert_eq!(
            action(&directory, json!({ "operation": "unknown" })),
            invalid
        );
        assert_eq!(
            action(
                &directory,
                json!({ "operation": "logout", "all": true, "extra": 1 })
            ),
            invalid
        );
        assert_eq!(
            action(
                &directory,
                json!({ "operation": "request_code", "provider": "carrier-pigeon", "target": "x" })
            ),
            invalid
        );
        assert_eq!(
            action(
                &directory,
                json!({ "operation": "login", "challenge_id": "synthetic", "code": "12ab56" })
            ),
            invalid
        );
        assert_eq!(
            action(
                &directory,
                json!({ "operation": "rename", "display_name": " 空格 " })
            ),
            invalid
        );
        assert_eq!(
            action(
                &directory,
                json!({ "operation": "upload_avatar", "path": "avatar.png" })
            ),
            invalid
        );
        let not_an_image = root.path().join("avatar.png");
        std::fs::write(&not_an_image, b"synthetic text, not an image").unwrap();
        assert_eq!(
            action(
                &directory,
                json!({ "operation": "upload_avatar", "path": not_an_image })
            ),
            invalid
        );
    }

    #[test]
    fn signed_out_calls_report_unauthorized_and_cancel_is_harmless() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("account");
        assert_eq!(
            action(&directory, json!({ "operation": "google_cancel" })),
            json!({ "ok": true, "value": null })
        );
        assert_eq!(
            action(&directory, json!({ "operation": "avatar" })),
            json!({ "ok": true, "value": null })
        );
        // 没有会话时取不到令牌，失败在发请求之前。
        for operation in ["profile", "remove_avatar", "delete"] {
            assert_eq!(
                action(&directory, json!({ "operation": operation })),
                json!({ "ok": false, "error": "account_unauthorized" }),
                "{operation}"
            );
        }
        assert_eq!(
            action(&directory, json!({ "operation": "logout", "all": true })),
            json!({ "ok": false, "error": "account_unauthorized" })
        );
    }

    #[test]
    fn settings_sync_checks_its_arguments_and_the_account_before_the_network() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("account");
        let preferences = root.path().join("preferences");
        let cloud = json!({ "revision": 3, "settings": { "input.scheme": "pinyin" } });
        let apply = |user_id: &str| {
            action(
                &directory,
                json!({
                    "operation": "preferences_apply",
                    "preferences_directory": preferences,
                    "user_id": user_id,
                    "preferences": cloud,
                }),
            )
        };
        let unauthorized = json!({ "ok": false, "error": "account_unauthorized" });
        for operation in ["preferences_schema", "preferences_load"] {
            assert_eq!(
                action(&directory, json!({ "operation": operation })),
                unauthorized,
                "{operation}"
            );
        }
        assert_eq!(
            action(
                &directory,
                json!({ "operation": "preferences_upload", "preferences_directory": preferences })
            ),
            unauthorized
        );
        // 未登录时没有页面下载时的那个账号，应用被取消。
        assert_eq!(
            apply("synthetic-user"),
            json!({ "ok": false, "error": "account_cancelled" })
        );

        let invalid = json!({ "ok": false, "error": "account_invalid" });
        assert_eq!(
            action(
                &directory,
                json!({ "operation": "preferences_upload", "preferences_directory": "relative" })
            ),
            invalid
        );
        assert_eq!(
            action(&directory, json!({ "operation": "preferences_upload" })),
            invalid
        );

        // 页面下载后换了账号：在取令牌、联网之前就以取消结束，本机偏好不动。
        save_session(&directory);
        assert_eq!(
            apply("another-user"),
            json!({ "ok": false, "error": "account_cancelled" })
        );
        assert!(!preferences.join("preferences.json").exists());
    }

    #[test]
    fn new_operations_check_requests_and_the_account_before_the_network() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("account");
        // 匿名会话与登录会话在同一个目录里；目录本身要存在才读得了匿名会话，与 `msime_client_account_access_token` 相同。
        std::fs::create_dir(&directory).unwrap();
        let unauthorized = json!({ "ok": false, "error": "account_unauthorized" });
        for operation in [
            json!({ "operation": "chat_models" }),
            json!({ "operation": "chat", "messages": [{ "role": "user", "content": "你好" }], "model": "synthetic-model" }),
            json!({ "operation": "account_delete" }),
            json!({ "operation": "phrases_load" }),
            json!({ "operation": "sessions_list" }),
            json!({ "operation": "session_revoke", "id": "0".repeat(64) }),
            json!({ "operation": "data_summary" }),
            json!({ "operation": "data_delete", "sections": ["voice"] }),
            // 没有登录也没有匿名会话。
            json!({ "operation": "translate", "texts": ["词"], "source_lang": "zh", "target_lang": "en" }),
            json!({ "operation": "feedback", "type": "bug", "text": "合成反馈", "platform": "windows", "app_version": "0.9.0", "edition": "full" }),
        ] {
            assert_eq!(
                action(&directory, operation.clone()),
                unauthorized,
                "{operation}"
            );
        }
        let invalid = json!({ "ok": false, "error": "account_invalid" });
        for operation in [
            json!({ "operation": "phrases_save", "revision": 0, "phrases": [{ "id": "a", "text": "", "group": "", "position": 0 }] }),
            json!({ "operation": "data_delete", "sections": [] }),
            json!({ "operation": "data_export", "destination": "relative.zip" }),
            json!({ "operation": "session_revoke", "id": "short" }),
            json!({ "operation": "translate", "texts": [], "source_lang": "zh", "target_lang": "en" }),
            json!({ "operation": "translate", "texts": ["词"], "source_lang": "zh", "target_lang": "en", "extra": 1 }),
            json!({ "operation": "diagnostics_state", "user_id": "" }),
        ] {
            assert_eq!(
                action(&directory, operation.clone()),
                invalid,
                "{operation}"
            );
        }
        let full = "a".repeat(8192);
        assert_eq!(
            action(
                &directory,
                json!({ "operation": "translate", "texts": vec![full; 32], "source_lang": "zh", "target_lang": "en" })
            ),
            json!({ "ok": false, "error": "account_too_large" })
        );
        // 登录着，但诊断快照是另一个账号上传的。
        save_session(&directory);
        assert_eq!(
            action(
                &directory,
                json!({ "operation": "diagnostics_delete", "user_id": "someone-else" })
            ),
            json!({ "ok": false, "error": "account_identity_changed" })
        );
        assert!(!directory
            .join(msime_client_services::account::ANONYMOUS_SESSION_FILE)
            .exists());
    }

    fn gloss(request: Value) -> Value {
        let bytes = request.to_string();
        // SAFETY: 缓冲区在调用期间有效，返回的字符串在下面释放。
        unsafe {
            let raw = crate::msime_client_account_gloss(bytes.as_ptr(), bytes.len());
            let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
            msime_client_string_free(raw);
            serde_json::from_str(&text).unwrap()
        }
    }

    #[test]
    fn the_gloss_request_and_parse_halves_follow_translate() {
        let request = gloss(
            json!({ "operation": "request", "texts": ["水杉", "输入法"], "source_lang": "zh", "target_lang": "ja" }),
        );
        assert_eq!(request["value"]["method"], "POST");
        assert_eq!(request["value"]["path"], "/v1/translate");
        let body: Value = serde_json::from_str(request["value"]["body"].as_str().unwrap()).unwrap();
        assert_eq!(
            body,
            json!({ "texts": ["水杉", "输入法"], "source_lang": "ZH", "target_lang": "JA" })
        );
        assert_eq!(
            gloss(json!({
                "operation": "parse",
                "texts": ["水杉", "输入法"],
                "status": 200,
                "body": json!({ "code": 200, "data": ["メタセコイア\n", "输入法"] }).to_string(),
            })),
            json!({ "ok": true, "value": { "translations": ["メタセコイア", null] } })
        );
        assert_eq!(
            gloss(json!({
                "operation": "parse",
                "texts": ["水杉"],
                "status": 429,
                "body": r#"{"error":{"code":"rate_limit_exceeded","message":"rate_limit_exceeded"}}"#,
                "retry_after": "60",
            })),
            json!({ "ok": false, "error": "account_rate_limited", "retry_after_seconds": 60 })
        );
        assert_eq!(
            gloss(
                json!({ "operation": "parse", "texts": ["水杉"], "status": 200, "body": "{\"code\":\"200\",\"data\":[\"x\"]}" })
            ),
            json!({ "ok": false, "error": "account_unavailable" })
        );
        assert_eq!(
            gloss(
                json!({ "operation": "request", "texts": ["词"], "source_lang": "zh_CN", "target_lang": "en" })
            ),
            json!({ "ok": false, "error": "account_invalid" })
        );
        let full = "a".repeat(8192);
        assert_eq!(
            gloss(
                json!({ "operation": "request", "texts": vec![full; 32], "source_lang": "zh", "target_lang": "en" })
            ),
            json!({ "ok": false, "error": "account_too_large" })
        );
        assert_eq!(
            gloss(json!({ "operation": "send" })),
            json!({ "ok": false, "error": "account_invalid" })
        );
    }
}
