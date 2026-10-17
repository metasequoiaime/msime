use super::*;
use std::path::Path;
use std::sync::mpsc;

/// 采用会话不联网；任何请求都说明规则走错了路。
struct NoNetwork;

impl AccountApi for NoNetwork {
    fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError> {
        panic!("session_adopt must not reach the network")
    }
    fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
        panic!("session_adopt must not reach the network")
    }
    fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
        panic!("session_adopt must not reach the network")
    }
    fn refresh(&self, _: &str) -> Result<AccountTokens, AccountError> {
        panic!("session_adopt must not reach the network")
    }
    fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
        panic!("session_adopt must not reach the network")
    }
    fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
        panic!("session_adopt must not reach the network")
    }
    fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
        panic!("session_adopt must not reach the network")
    }
    fn delete_account(&self, _: &str) -> Result<(), AccountError> {
        panic!("session_adopt must not reach the network")
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// 一份合成会话：`user` 的第 `sign_in` 次登录（`None` 是还没补上登录 ID 的旧文件），`minutes` 分钟后过期，刷新令牌由 `refresh` 这个十六进制字符重复而成。
fn saved(user: &str, sign_in: Option<u128>, minutes: u64, refresh: char) -> SavedAccountSession {
    SavedAccountSession {
        tokens: AccountTokens {
            access_token: "a".repeat(64),
            refresh_token: refresh.to_string().repeat(64),
            token_type: "Bearer".into(),
            expires_in: minutes * 60,
            user: AccountUser {
                id: user.into(),
                display_name: String::new(),
                created_at: "2026-01-01T00:00:00Z".into(),
                email: None,
                avatar_url: None,
            },
        },
        expires_at_unix_ms: now_ms() + minutes * 60_000,
        session_id: sign_in.map(uuid::Uuid::from_u128),
    }
}

fn storage(directory: &Path) -> FileAccountSessionStorage {
    FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Native)
}

fn session(directory: &Path) -> BackendAccountSession<NoNetwork, FileAccountSessionStorage> {
    BackendAccountSession::new(NoNetwork, storage(directory))
}

/// 本地会话的（用户、刷新令牌），没有会话时是 `None`。
fn local(directory: &Path) -> Option<(String, String)> {
    storage(directory)
        .load()
        .unwrap()
        .map(|saved| (saved.tokens.user.id, saved.tokens.refresh_token))
}

fn has(user: &str, refresh: char) -> Option<(String, String)> {
    Some((user.to_owned(), refresh.to_string().repeat(64)))
}

#[test]
fn a_sign_out_in_settings_reaches_the_keyboard_and_the_keyboard_cannot_undo_it() {
    let root = tempfile::tempdir().unwrap();
    let (settings, keyboard) = (root.path().join("settings"), root.path().join("keyboard"));
    storage(&keyboard)
        .save(&saved("person-a", Some(1), 60, 'b'))
        .unwrap();
    storage(&keyboard)
        .save_clipboard_keystore("v1.synthetic")
        .unwrap();

    // 设置应用退出了登录、发布方没有会话：键盘清掉本地会话和密钥串，与 forget 相同。
    let keyboard_session = session(&keyboard);
    assert_eq!(
        keyboard_session.adopt_session(None, SessionAdoptSource::Settings),
        Ok(SessionAdoptOutcome::Cleared)
    );
    assert_eq!(local(&keyboard), None);
    assert_eq!(storage(&keyboard).load_clipboard_keystore(), Ok(None));
    assert_eq!(keyboard_session.status().unwrap(), None);
    // 再来一次什么也没有可清的。
    assert_eq!(
        keyboard_session.adopt_session(None, SessionAdoptSource::Settings),
        Ok(SessionAdoptOutcome::Unchanged)
    );

    // 键盘还没导入、又发布了它那份更新的副本：设置应用不会因此重新登录。
    assert_eq!(
        session(&settings).adopt_session(
            Some(saved("person-a", Some(1), 120, 'c')),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(local(&settings), None);
    // 键盘那边也没有会话时同样不动。
    assert_eq!(
        session(&settings).adopt_session(None, SessionAdoptSource::Keyboard),
        Ok(SessionAdoptOutcome::Unchanged)
    );
}

#[test]
fn a_switched_account_is_not_rolled_back_by_the_keyboard() {
    let root = tempfile::tempdir().unwrap();
    let (settings, keyboard) = (root.path().join("settings"), root.path().join("keyboard"));
    storage(&settings)
        .save(&saved("person-b", Some(2), 30, 'd'))
        .unwrap();
    storage(&keyboard)
        .save(&saved("person-a", Some(1), 120, 'b'))
        .unwrap();

    // 键盘里旧账号的副本即使过期得更晚，也不会把设置应用换回去。
    assert_eq!(
        session(&settings).adopt_session(
            Some(storage(&keyboard).load().unwrap().unwrap()),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(local(&settings), has("person-b", 'd'));

    // 反过来，设置应用换了号，键盘直接换成它，不比过期时间。
    let keyboard_session = session(&keyboard);
    assert_eq!(keyboard_session.status().unwrap().unwrap().id, "person-a");
    assert_eq!(
        keyboard_session.adopt_session(
            Some(storage(&settings).load().unwrap().unwrap()),
            SessionAdoptSource::Settings
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(local(&keyboard), has("person-b", 'd'));
    assert_eq!(keyboard_session.status().unwrap().unwrap().id, "person-b");

    // 同一个账号重新登录（新的登录 ID）也一样以设置应用为准。
    assert_eq!(
        keyboard_session.adopt_session(
            Some(saved("person-b", Some(3), 10, 'e')),
            SessionAdoptSource::Settings
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(local(&keyboard), has("person-b", 'e'));
}

#[test]
fn within_one_sign_in_the_later_expiry_wins_in_both_directions() {
    let root = tempfile::tempdir().unwrap();
    let (settings, keyboard) = (root.path().join("settings"), root.path().join("keyboard"));
    storage(&settings)
        .save(&saved("person-a", Some(1), 60, 'b'))
        .unwrap();
    storage(&keyboard)
        .save(&saved("person-a", Some(1), 60, 'b'))
        .unwrap();

    // 键盘刷新过（过期时间推后、刷新令牌换了）：设置应用采用它，否则会拿用过的刷新令牌去刷新、整个会话被服务端吊销。
    assert_eq!(
        session(&settings).adopt_session(
            Some(saved("person-a", Some(1), 120, 'c')),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(local(&settings), has("person-a", 'c'));
    // 键盘之后发布的旧副本不会把它换回去。
    assert_eq!(
        session(&settings).adopt_session(
            Some(saved("person-a", Some(1), 90, 'b')),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(local(&settings), has("person-a", 'c'));

    // 设置应用发布的同一次登录也按新者胜：键盘自己刷新出来的更新副本留着。
    storage(&keyboard)
        .save(&saved("person-a", Some(1), 180, 'f'))
        .unwrap();
    assert_eq!(
        session(&keyboard).adopt_session(
            Some(storage(&settings).load().unwrap().unwrap()),
            SessionAdoptSource::Settings
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(local(&keyboard), has("person-a", 'f'));
    assert_eq!(
        session(&keyboard).adopt_session(
            Some(saved("person-a", Some(1), 240, '9')),
            SessionAdoptSource::Settings
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(local(&keyboard), has("person-a", '9'));
}

#[test]
fn sides_without_a_login_id_differ_unless_they_share_a_refresh_token() {
    let root = tempfile::tempdir().unwrap();
    let (settings, keyboard) = (root.path().join("settings"), root.path().join("keyboard"));
    storage(&settings)
        .save(&saved("person-a", None, 60, 'b'))
        .unwrap();
    storage(&keyboard)
        .save(&saved("person-a", None, 180, 'c'))
        .unwrap();

    // 两边都没有登录 ID、刷新令牌也不同（补出的登录 ID 就不同）：键盘的副本再新也不采用；有登录 ID 的一方也不会与按别的刷新令牌补出的 ID 相同。
    assert_eq!(
        session(&settings).adopt_session(
            Some(storage(&keyboard).load().unwrap().unwrap()),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(
        session(&settings).adopt_session(
            Some(saved("person-a", Some(1), 180, 'c')),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(local(&settings), has("person-a", 'b'));
    // 设置应用的副本即使更旧也替换键盘的。
    assert_eq!(
        session(&keyboard).adopt_session(
            Some(storage(&settings).load().unwrap().unwrap()),
            SessionAdoptSource::Settings
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(local(&keyboard), has("person-a", 'b'));
}

#[test]
fn forgetting_for_an_absent_source_waits_for_a_refresh_in_progress() {
    let root = tempfile::tempdir().unwrap();
    let keyboard = root.path().join("keyboard");
    storage(&keyboard)
        .save(&saved("person-a", Some(1), 1, 'b'))
        .unwrap();

    // 另一个线程正在刷新：持着 account-refresh.lock，稍后写回轮换过的会话。
    let (started, wait) = mpsc::channel();
    let refreshing = {
        let keyboard = keyboard.clone();
        std::thread::spawn(move || {
            storage(&keyboard).with_refresh_lock(|| {
                started.send(()).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(300));
                storage(&keyboard).save(&saved("person-a", Some(1), 60, 'c'))
            })
        })
    };
    wait.recv().unwrap();

    // 设置应用已经退出：清本地会话要等刷新写完，否则刷新会把刚清掉的会话写回来。
    assert_eq!(
        session(&keyboard).adopt_session(None, SessionAdoptSource::Settings),
        Ok(SessionAdoptOutcome::Cleared)
    );
    refreshing.join().unwrap().unwrap();
    assert_eq!(local(&keyboard), None);
}

#[test]
fn an_invalid_published_session_leaves_the_local_one_alone() {
    let root = tempfile::tempdir().unwrap();
    let keyboard = root.path().join("keyboard");
    storage(&keyboard)
        .save(&saved("person-a", Some(1), 60, 'b'))
        .unwrap();
    let mut published = saved("person-b", Some(2), 60, 'c');
    published.tokens.refresh_token = "not-a-token".into();
    assert_eq!(
        session(&keyboard).adopt_session(Some(published), SessionAdoptSource::Settings),
        Err(AccountError::Storage)
    );
    assert_eq!(local(&keyboard), has("person-a", 'b'));
}

/// 每次刷新按 `c`、`d`、`e`… 轮换刷新令牌，记下交出去的刷新令牌，像服务端那样拒收用过的。
#[derive(Clone, Default)]
struct RotatingApi {
    presented: Arc<Mutex<Vec<String>>>,
}

impl AccountApi for RotatingApi {
    fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn refresh(&self, refresh_token: &str) -> Result<AccountTokens, AccountError> {
        let mut presented = self.presented.lock().unwrap();
        if presented.iter().any(|spent| spent == refresh_token) {
            return Err(AccountError::Unauthorized);
        }
        presented.push(refresh_token.to_owned());
        let next = char::from(b'b' + presented.len() as u8);
        let mut tokens = saved("person-a", None, 15, next).tokens;
        tokens.access_token = next.to_string().repeat(64);
        Ok(tokens)
    }
    fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
    fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
    fn delete_account(&self, _: &str) -> Result<(), AccountError> {
        Err(AccountError::Unavailable)
    }
}

fn private_file(path: &Path, contents: &[u8]) {
    std::fs::write(path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

/// 升级后两个沙箱里是同一份没有 `session_id` 的旧会话，访问令牌已经过期（升级时多数用户的状态）。键盘先刷新，之后两边互相导入：谁都不能回到用过的刷新令牌 `b`，否则下一次刷新被服务端当成重放、整个会话被吊销。`file` 是会话文件名，`open` 按目录建出这份会话的存储。
fn assert_one_refresh_survives_the_upgrade<S: AccountSessionStorage>(
    file: &str,
    contents: &[u8],
    open: impl Fn(&Path) -> S,
) {
    let root = tempfile::tempdir().unwrap();
    let (settings, keyboard) = (root.path().join("settings"), root.path().join("keyboard"));
    for directory in [&settings, &keyboard] {
        storage(directory).create_directory().unwrap();
        private_file(&directory.join(file), contents);
    }
    let refreshed = |directory: &Path| {
        let saved = open(directory).load().unwrap().unwrap();
        (saved.tokens.refresh_token, saved.session_id)
    };
    let api = RotatingApi::default();
    let keyboard_session = BackendAccountSession::new(api.clone(), open(&keyboard));
    keyboard_session.credentials(None, None).unwrap();
    assert_eq!(*api.presented.lock().unwrap(), ["b".repeat(64)]);
    let (token, login_id) = refreshed(&keyboard);
    assert_eq!(token, "c".repeat(64));
    assert!(login_id.is_some());

    // 设置应用还没被 Rust 写过，发布的仍是旧会话：键盘留着自己刷新过的那份。
    assert_eq!(
        keyboard_session.adopt_session(
            read_published_session(&settings.join(file)).unwrap(),
            SessionAdoptSource::Settings
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(refreshed(&keyboard), ("c".repeat(64), login_id));

    // 设置应用本地也还是旧会话：采用键盘刷新过的那份，下一次刷新交出的是 `c`。
    let settings_session = BackendAccountSession::new(api.clone(), open(&settings));
    assert_eq!(
        settings_session.adopt_session(
            read_published_session(&keyboard.join(file)).unwrap(),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(refreshed(&settings), ("c".repeat(64), login_id));
    let access = "c".repeat(64);
    settings_session.credentials(Some(&access), None).unwrap();
    assert_eq!(
        *api.presented.lock().unwrap(),
        ["b".repeat(64), "c".repeat(64)]
    );
    assert_eq!(refreshed(&settings), ("d".repeat(64), login_id));

    // 键盘再导入设置应用刷新后的那份，仍是同一次登录、新者胜。
    assert_eq!(
        keyboard_session.adopt_session(
            read_published_session(&settings.join(file)).unwrap(),
            SessionAdoptSource::Settings
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(refreshed(&keyboard), ("d".repeat(64), login_id));
}

#[test]
fn an_upgraded_flat_session_refreshed_in_one_sandbox_is_not_rolled_back_by_the_other() {
    // ArkTS 桥接存的扁平登录会话，一小时前过期。
    let flat = format!(
        r#"{{"access_token":"{}","refresh_token":"{}","token_type":"Bearer","expires_at":{},"user":{{"id":"person-a","display_name":"","created_at":"2026-01-01T00:00:00Z"}}}}"#,
        "a".repeat(64),
        "b".repeat(64),
        now_ms() - 3_600_000
    );
    assert_one_refresh_survives_the_upgrade(ACCOUNT_SESSION_FILE, flat.as_bytes(), storage);
}

#[test]
fn an_upgraded_anonymous_session_refreshed_in_one_sandbox_is_not_rolled_back_by_the_other() {
    // ArkTS 存的匿名会话是 Native 布局，但同样没有 `session_id`。
    let mut expired = saved("person-a", None, 60, 'b');
    expired.expires_at_unix_ms = now_ms() - 3_600_000;
    let native = serde_json::to_vec(&expired).unwrap();
    assert!(!String::from_utf8_lossy(&native).contains("session_id"));
    assert_one_refresh_survives_the_upgrade(
        ANONYMOUS_SESSION_FILE,
        &native,
        LockedAnonymousSessionStorage::new,
    );
}
