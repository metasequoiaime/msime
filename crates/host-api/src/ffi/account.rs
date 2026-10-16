//! 原生设置窗口的账号操作：与 Tauri 桌面端的十四个账号命令（`desktop_account.rs`）和 Windows 的四个设置同步命令（`windows_settings_sync.rs`）一一对应，用同一个 [`BackendAccountSession`] 和同一份本用户会话文件（[`FileAccountSessionStorage`]，`account-session.json`），返回的也是同样的脱敏形状（[`msime_client_services::account::page`]）。令牌只在本进程里，不出现在响应中。
//!
//! 每个会话目录在进程里只有一个会话对象，这样进行中的 Google 登录可以被另一次调用里的 `google_cancel` 结束；会话文件本身每次访问都重新读，共享应用、输入法和这里同时登录、刷新或退出时以文件为准，刷新时共用 `account-refresh.lock`。
//!
//! 请求带 `session_store` 而不带 `directory` 时，会话在宿主注入的存储里（[`super::session_store`]），会话对象是注册时建好的那一个。
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
    adopt_legacy_account_session, ensure_anonymous_account, ensure_anonymous_account_in,
    AccountError, AccountPreferences, AccountSessionFileLayout, BackendAccountClient,
    BackendAccountSession, FileAccountSessionStorage,
};

/// 请求的上限：应用云端设置时请求里带着整份云端设置文档（其中可能有 Android 的自定义键盘皮肤库，最多 768 KiB），与 Android 的设置同步导出一样给 4 MiB；目录各最多 16 KiB。
const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_DIRECTORY_BYTES: usize = 16 * 1024;

pub(super) type Session = BackendAccountSession<BackendAccountClient, SessionStorage>;

/// 每个会话目录一个会话对象。只在取对象时持锁，账号调用本身在锁外进行。
static ACCOUNT_SESSIONS: OnceLock<Mutex<HashMap<PathBuf, Arc<Session>>>> = OnceLock::new();

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
    /// 本机匿名账号还没有会话时注册它，与 `msime_client_ensure_anonymous_account` 相同，但失败时返回账号错误码。
    EnsureAnonymous,
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
        SessionStorage::File(FileAccountSessionStorage::new(
            directory,
            AccountSessionFileLayout::Native,
        )),
    ));
    sessions.insert(directory.to_path_buf(), Arc::clone(&session));
    Ok(session)
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
        // 要写本机偏好、错误码不全是账号错误码的两项由 run 处理，要看会话来源的 ensure_anonymous 由导出函数处理，都不会到这里。
        AccountAction::PreferencesUpload { .. }
        | AccountAction::PreferencesApply { .. }
        | AccountAction::EnsureAnonymous => Err(AccountError::Invalid),
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
        let code = |error: AccountError| error.code().to_owned();
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
                if matches!(request.action, AccountAction::EnsureAnonymous) {
                    return ensure_anonymous_account(directory)
                        .map(|()| Value::Bool(true))
                        .map_err(code);
                }
                let session = session_for(directory, legacy).map_err(code)?;
                run(&session, request.action)
            }
            Source::Store(entry) => {
                if matches!(request.action, AccountAction::EnsureAnonymous) {
                    return ensure_anonymous_account_in(&entry.anonymous, entry.backend.as_ref())
                        .map(|()| Value::Bool(true))
                        .map_err(code);
                }
                run(&entry.account, request.action)
            }
        }
    })
}

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
            // SAFETY: the buffer is live for the call, and the returned string is freed below.
            unsafe {
                let raw = msime_client_account_access_token(bytes.as_ptr(), bytes.len());
                let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
                msime_client_string_free(raw);
                serde_json::from_str(&text).unwrap()
            }
        }

        fn cloud_clipboard(request: Value) -> Value {
            let bytes = request.to_string();
            // SAFETY: as above.
            unsafe {
                let raw = msime_client_account_cloud_clipboard(bytes.as_ptr(), bytes.len());
                let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
                msime_client_string_free(raw);
                serde_json::from_str(&text).unwrap()
            }
        }

        fn community(request: Value) -> Value {
            let bytes = request.to_string();
            // SAFETY: as above.
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
        fn an_apple_layout_store_reads_what_swift_wrote() {
            let store = leaked_store();
            let id = registered(store, SHARED, 1);
            // Swift 的 BackendSavedSession：expiresAt 是 2001 年起的秒数。
            let document = json!({
                "tokens": {
                    "access_token": "a".repeat(64),
                    "refresh_token": "b".repeat(64),
                    "token_type": "Bearer",
                    "expires_in": 900,
                    "user": {
                        "id": "synthetic-user",
                        "display_name": "",
                        "created_at": "2026-01-01T00:00:00Z",
                        "email": "person@example.test",
                    },
                },
                "expiresAt": 800_000_000.0,
                "sessionID": "6f9619ff-8b86-d011-b42d-00c04fc964ff",
            });
            put(store, 1, document.to_string().into_bytes());
            let user = &store_action(id, json!({ "operation": "status" }))["value"]["user"];
            assert_eq!(user["id"], "synthetic-user");
            assert_eq!(user["email"], "person@example.test");
            unregister(id);
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
