//! 原生设置窗口的账号操作：与 Tauri 桌面端的十四个账号命令（`desktop_account.rs`）和 Windows 的四个设置同步命令（`windows_settings_sync.rs`）一一对应，用同一个 [`BackendAccountSession`] 和同一份本用户会话文件（[`FileAccountSessionStorage`]，`account-session.json`），返回的也是同样的脱敏形状（[`msime_client_core::account::page`]）。令牌只在本进程里，不出现在响应中。
//!
//! 每个会话目录在进程里只有一个会话对象，这样进行中的 Google 登录可以被另一次调用里的 `google_cancel` 结束；会话文件本身每次访问都重新读，共享应用、输入法和这里同时登录、刷新或退出时以文件为准，刷新时共用 `account-refresh.lock`。
//!
//! Part of the C ABI; see the parent module for what these shims guarantee.

use super::with_bounded_bytes;
use crate::*;
use msime_client_core::account::page::{
    providers_response, ChallengeResponse, PreferenceSchemaResponse, ProfileResponse,
    StatusResponse,
};
use msime_client_core::account::settings_sync::{
    prepare_desktop_settings_apply, upload_desktop_settings,
};
use msime_client_core::account::{
    adopt_legacy_account_session, AccountError, AccountPreferences, AccountSessionFileLayout,
    BackendAccountClient, BackendAccountSession, FileAccountSessionStorage,
};
use msime_client_core::preferences::PreferencesError;

/// 请求的上限：应用云端设置时请求里带着整份云端设置文档（其中可能有 Android 的自定义键盘皮肤库，最多 768 KiB），与 Android 的设置同步导出一样给 4 MiB；目录各最多 16 KiB。
const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_DIRECTORY_BYTES: usize = 16 * 1024;

pub(super) type Session = BackendAccountSession<BackendAccountClient, FileAccountSessionStorage>;

/// 每个会话目录一个会话对象。只在取对象时持锁，账号调用本身在锁外进行。
static ACCOUNT_SESSIONS: OnceLock<Mutex<HashMap<PathBuf, Arc<Session>>>> = OnceLock::new();

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountRequest {
    /// 共享应用登录时写 `account-session.json` 的目录。
    directory: String,
    /// 旧版本放会话的目录；新目录还没有会话而这里有时，先把它搬过去（[`adopt_legacy_account_session`]）。
    #[serde(default)]
    legacy_directory: Option<String>,
    action: AccountAction,
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
    },
    Login {
        challenge_id: String,
        code: String,
    },
    GoogleLogin,
    GoogleCancel,
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
}

/// 目录对应的会话对象，第一次用到时建立；建立前先接管旧目录里的会话，与共享应用启动时做的一样。社区操作（`msime_client_community`）经同一个对象认证。
pub(super) fn session_for(
    directory: &Path,
    legacy: Option<&Path>,
) -> Result<Arc<Session>, AccountError> {
    let sessions = ACCOUNT_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut sessions = sessions.lock().map_err(|_| AccountError::Unavailable)?;
    if let Some(session) = sessions.get(directory) {
        return Ok(Arc::clone(session));
    }
    if let Some(legacy) = legacy {
        adopt_legacy_account_session(legacy, directory);
    }
    let client = BackendAccountClient::new()?;
    let session = Arc::new(BackendAccountSession::new(
        client,
        FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Native),
    ));
    sessions.insert(directory.to_path_buf(), Arc::clone(&session));
    Ok(session)
}

fn value<T: Serialize>(value: T) -> Result<Value, AccountError> {
    serde_json::to_value(value).map_err(|_| AccountError::Unavailable)
}

fn status(user: Option<msime_client_core::account::AccountUser>) -> Result<Value, AccountError> {
    value(StatusResponse {
        user: user.map(Into::into),
    })
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

/// 设置同步的应用：client-core 算出应用后的偏好和读到时的修订号，再按这个修订号写回，与共享应用的 `account_preferences_apply` 相同；关掉剪贴板历史时与 `msime_client_save_preferences` 一样清掉已存的历史。返回保存后的偏好快照，宿主用它重画页面。
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

/// 一次操作。设置同步的上传和应用要读写宿主给的偏好目录，在这里处理；其余都只经会话，见 [`run_account`]。
fn run(session: &Session, action: AccountAction) -> Result<Value, String> {
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
        AccountAction::RequestCode { provider, target } => value(ChallengeResponse::from(
            session.request_code(&provider, &target)?,
        )),
        AccountAction::Login { challenge_id, code } => {
            status(Some(session.sign_in(&challenge_id, &code)?))
        }
        AccountAction::GoogleLogin => {
            status(Some(session.sign_in_google_with_browser(open_browser)?))
        }
        AccountAction::GoogleCancel => {
            session.cancel_google_sign_in();
            Ok(Value::Null)
        }
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
        // 要写本机偏好、错误码不全是账号错误码的两项由 run 处理，不会到这里。
        AccountAction::PreferencesUpload { .. } | AccountAction::PreferencesApply { .. } => {
            Err(AccountError::Invalid)
        }
    }
}

fn absolute_directory(value: &str) -> Result<&Path, String> {
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
    response(|| {
        let invalid = || AccountError::Invalid.code().to_owned();
        // SAFETY: forwarded from the documented caller contract.
        let request: AccountRequest = unsafe {
            with_bounded_bytes(
                request,
                length,
                MAX_REQUEST_BYTES,
                "account_invalid",
                |bytes| serde_json::from_slice(bytes).map_err(|_| invalid()),
            )?
        };
        let directory = absolute_directory(&request.directory)?;
        let legacy = request
            .legacy_directory
            .as_deref()
            .map(absolute_directory)
            .transpose()?;
        let session = session_for(directory, legacy).map_err(|error| error.code().to_owned())?;
        run(&session, request.action)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_core::account::{
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
}
