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

/// 换上指定的登录 ID：`legacy_session_id` 派生的（升级前那次登录的后代）或随机的 v4（Rust 里的新登录）。
fn with_id(mut session: SavedAccountSession, id: uuid::Uuid) -> SavedAccountSession {
    session.session_id = Some(id);
    session
}

#[test]
fn a_session_without_a_login_id_matches_the_same_user_from_before_the_upgrade() {
    let root = tempfile::tempdir().unwrap();
    let (settings, keyboard) = (root.path().join("settings"), root.path().join("keyboard"));
    // 升级前键盘已经把 `b` 轮换成 `c`，设置应用还没导入：两边都没有登录 ID，刷新令牌不同。
    storage(&settings)
        .save(&saved("person-a", None, 60, 'b'))
        .unwrap();
    storage(&keyboard)
        .save(&saved("person-a", None, 180, 'c'))
        .unwrap();

    // 设置应用发布的是用过的 `b`：键盘留着更新的 `c`，不被换回去。
    assert_eq!(
        session(&keyboard).adopt_session(
            Some(storage(&settings).load().unwrap().unwrap()),
            SessionAdoptSource::Settings
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(local(&keyboard), has("person-a", 'c'));
    // 设置应用采用键盘的 `c`，写进文件的是由 `c` 派生的登录 ID。
    assert_eq!(
        session(&settings).adopt_session(
            Some(storage(&keyboard).load().unwrap().unwrap()),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(local(&settings), has("person-a", 'c'));
    let derived = legacy_session_id(&"c".repeat(64));
    assert_eq!(
        storage(&settings).load().unwrap().unwrap().session_id,
        Some(derived)
    );
}

#[test]
fn a_migrated_keyboard_session_still_matches_a_settings_copy_without_a_login_id() {
    let root = tempfile::tempdir().unwrap();
    let (settings, keyboard) = (root.path().join("settings"), root.path().join("keyboard"));
    storage(&settings)
        .save(&saved("person-a", None, 60, 'b'))
        .unwrap();
    // 键盘已经被 Rust 用过：补上了由 `c` 派生的登录 ID，又刷新成 `d`。
    let keyboard_id = legacy_session_id(&"c".repeat(64));
    storage(&keyboard)
        .save(&with_id(saved("person-a", None, 180, 'd'), keyboard_id))
        .unwrap();

    assert_eq!(
        session(&keyboard).adopt_session(
            Some(storage(&settings).load().unwrap().unwrap()),
            SessionAdoptSource::Settings
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(local(&keyboard), has("person-a", 'd'));
    assert_eq!(
        session(&settings).adopt_session(
            Some(storage(&keyboard).load().unwrap().unwrap()),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(local(&settings), has("person-a", 'd'));
    assert_eq!(
        storage(&settings).load().unwrap().unwrap().session_id,
        Some(keyboard_id)
    );
}

#[test]
fn a_new_sign_in_is_not_mistaken_for_the_session_from_before_the_upgrade() {
    let root = tempfile::tempdir().unwrap();
    let (settings, keyboard) = (root.path().join("settings"), root.path().join("keyboard"));
    storage(&settings)
        .save(&saved("person-a", None, 60, 'b'))
        .unwrap();
    storage(&keyboard)
        .save(&saved("person-a", None, 180, 'c'))
        .unwrap();

    // 键盘发布的是 Rust 里新的一次登录（随机 ID），设置应用不采用，再新也一样。
    assert_eq!(
        session(&settings).adopt_session(
            Some(with_id(
                saved("person-a", None, 240, 'e'),
                uuid::Uuid::new_v4()
            )),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(local(&settings), has("person-a", 'b'));
    // 设置应用里重新登录了同一个账号：键盘直接换成它，即使本地的旧会话过期得更晚。
    assert_eq!(
        session(&keyboard).adopt_session(
            Some(with_id(
                saved("person-a", None, 30, 'f'),
                uuid::Uuid::new_v4()
            )),
            SessionAdoptSource::Settings
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(local(&keyboard), has("person-a", 'f'));
    // 两边都已经有登录 ID（各自由不同的令牌派生）时只认相同的 ID。
    storage(&settings)
        .save(&with_id(
            saved("person-a", None, 60, 'b'),
            legacy_session_id(&"b".repeat(64)),
        ))
        .unwrap();
    assert_eq!(
        session(&settings).adopt_session(
            Some(with_id(
                saved("person-a", None, 240, 'c'),
                legacy_session_id(&"c".repeat(64))
            )),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    // 另一个账号不论有没有登录 ID 都不算。
    assert_eq!(
        session(&settings).adopt_session(
            Some(saved("person-b", None, 240, 'c')),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(local(&settings), has("person-a", 'b'));
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

/// 采用的是哪一份会话，与 host-api 的 `item` 相同：登录会话走 `adopt_session`，匿名会话走 `adopt_anonymous_session`。
#[derive(Clone, Copy)]
enum Item {
    Account,
    Anonymous,
}

fn adopt<S: AccountSessionStorage>(
    item: Item,
    session: &BackendAccountSession<RotatingApi, S>,
    published: Option<SavedAccountSession>,
    from: SessionAdoptSource,
) -> Result<SessionAdoptOutcome, AccountError> {
    match item {
        Item::Account => session.adopt_session(published, from),
        Item::Anonymous => session.adopt_anonymous_session(published, from),
    }
}

/// 升级后两个沙箱里各有一份升级前的旧会话，访问令牌已经过期（升级时多数用户的状态）。`keyboard_rotated` 时旧版键盘在升级前已经把 `b` 轮换成 `c`、设置应用还没导入，`b` 已经用过；否则两边都是 `b`。键盘先刷新，之后两边互相导入：谁都不能回到用过的刷新令牌，否则下一次刷新被服务端当成重放、整个会话被吊销。`file` 是会话文件名，`settings_file` 和 `keyboard_file` 按刷新令牌的字符写出两边旧文件的内容，`open` 按目录建出这份会话的存储，`item` 选采用的规则。
fn assert_one_refresh_survives_the_upgrade<S: AccountSessionStorage>(
    file: &str,
    settings_file: impl Fn(char) -> Vec<u8>,
    keyboard_file: impl Fn(char) -> Vec<u8>,
    open: impl Fn(&Path) -> S,
    item: Item,
    keyboard_rotated: bool,
) {
    let root = tempfile::tempdir().unwrap();
    let (settings, keyboard) = (root.path().join("settings"), root.path().join("keyboard"));
    let api = RotatingApi::default();
    let token = |refresh: char| refresh.to_string().repeat(64);
    let next = |refresh: char| char::from(refresh as u8 + 1);
    let keyboard_token = if keyboard_rotated {
        api.presented.lock().unwrap().push(token('b'));
        'c'
    } else {
        'b'
    };
    for (directory, contents) in [
        (&settings, settings_file('b')),
        (&keyboard, keyboard_file(keyboard_token)),
    ] {
        storage(directory).create_directory().unwrap();
        private_file(&directory.join(file), &contents);
    }
    let mut spent = api.presented.lock().unwrap().clone();
    let refreshed = |directory: &Path| {
        let saved = open(directory).load().unwrap().unwrap();
        (saved.tokens.refresh_token, saved.session_id)
    };
    let keyboard_session = BackendAccountSession::new(api.clone(), open(&keyboard));
    keyboard_session.credentials(None, None).unwrap();
    spent.push(token(keyboard_token));
    assert_eq!(*api.presented.lock().unwrap(), spent);
    let first = next(keyboard_token);
    let (current, login_id) = refreshed(&keyboard);
    assert_eq!(current, token(first));
    assert!(login_id.is_some());

    // 设置应用还没被 Rust 写过，发布的仍是旧会话：键盘留着自己刷新过的那份。
    assert_eq!(
        adopt(
            item,
            &keyboard_session,
            read_published_session(&settings.join(file)).unwrap(),
            SessionAdoptSource::Settings
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(refreshed(&keyboard), (token(first), login_id));

    // 设置应用本地也还是旧会话：采用键盘刷新过的那份，下一次刷新交出的是它。
    let settings_session = BackendAccountSession::new(api.clone(), open(&settings));
    assert_eq!(
        adopt(
            item,
            &settings_session,
            read_published_session(&keyboard.join(file)).unwrap(),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(refreshed(&settings), (token(first), login_id));
    settings_session
        .credentials(Some(&token(first)), None)
        .unwrap();
    spent.push(token(first));
    assert_eq!(*api.presented.lock().unwrap(), spent);
    let second = next(first);
    assert_eq!(refreshed(&settings), (token(second), login_id));

    // 键盘再导入设置应用刷新后的那份，仍是同一次登录、新者胜。
    assert_eq!(
        adopt(
            item,
            &keyboard_session,
            read_published_session(&settings.join(file)).unwrap(),
            SessionAdoptSource::Settings
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(refreshed(&keyboard), (token(second), login_id));
}

/// ArkTS 桥接存的扁平登录会话，一小时前过期。
fn flat_session(refresh: char) -> Vec<u8> {
    format!(
        r#"{{"access_token":"{}","refresh_token":"{}","token_type":"Bearer","expires_at":{},"user":{{"id":"person-a","display_name":"","created_at":"2026-01-01T00:00:00Z"}}}}"#,
        "a".repeat(64),
        refresh.to_string().repeat(64),
        now_ms() - 3_600_000
    )
    .into_bytes()
}

/// 一小时前过期的 Native 匿名会话，登录 ID 是 `session_id`。
fn expired_anonymous_session(refresh: char, session_id: Option<uuid::Uuid>) -> Vec<u8> {
    let mut expired = saved("person-a", None, 60, refresh);
    expired.expires_at_unix_ms = now_ms() - 3_600_000;
    expired.session_id = session_id;
    serde_json::to_vec(&expired).unwrap()
}

/// ArkTS 存的匿名会话是 Native 布局，但同样没有 `session_id`，一小时前过期。
fn anonymous_session(refresh: char) -> Vec<u8> {
    let native = expired_anonymous_session(refresh, None);
    assert!(!String::from_utf8_lossy(&native).contains("session_id"));
    native
}

/// Rust 注册（`sign_in_anonymous`）时写的匿名会话：随机的 v4 登录 ID，一小时前过期。
fn registered_anonymous_session(refresh: char) -> Vec<u8> {
    expired_anonymous_session(refresh, Some(uuid::Uuid::new_v4()))
}

#[test]
fn an_upgraded_flat_session_refreshed_in_one_sandbox_is_not_rolled_back_by_the_other() {
    assert_one_refresh_survives_the_upgrade(
        ACCOUNT_SESSION_FILE,
        flat_session,
        flat_session,
        storage,
        Item::Account,
        false,
    );
}

#[test]
fn an_upgraded_anonymous_session_refreshed_in_one_sandbox_is_not_rolled_back_by_the_other() {
    assert_one_refresh_survives_the_upgrade(
        ANONYMOUS_SESSION_FILE,
        anonymous_session,
        anonymous_session,
        LockedAnonymousSessionStorage::new,
        Item::Anonymous,
        false,
    );
}

#[test]
fn an_upgraded_flat_session_the_keyboard_rotated_before_the_upgrade_survives() {
    assert_one_refresh_survives_the_upgrade(
        ACCOUNT_SESSION_FILE,
        flat_session,
        flat_session,
        storage,
        Item::Account,
        true,
    );
}

#[test]
fn an_upgraded_anonymous_session_the_keyboard_rotated_before_the_upgrade_survives() {
    assert_one_refresh_survives_the_upgrade(
        ANONYMOUS_SESSION_FILE,
        anonymous_session,
        anonymous_session,
        LockedAnonymousSessionStorage::new,
        Item::Anonymous,
        true,
    );
}

/// 设置应用里还是 Rust 注册时写的 `{b, v4}`，旧版键盘的 ArkTS 桥接已经把它轮换成 `c`、按 `anonymousSessionForStorage` 写回时丢了 `session_id`，`b` 已经用过（升级时常见的状态）。
#[test]
fn an_anonymous_session_registered_in_rust_and_rotated_by_the_old_keyboard_survives_the_upgrade() {
    assert_one_refresh_survives_the_upgrade(
        ANONYMOUS_SESSION_FILE,
        registered_anonymous_session,
        anonymous_session,
        LockedAnonymousSessionStorage::new,
        Item::Anonymous,
        true,
    );
}

/// 键盘还没被 Rust 用过、两边都没刷新时，匿名会话注册时的 v4 副本与 ArkTS 写回的没有 ID 的副本也按新者胜；两边都是 v4 时仍只认相同的 ID，登录会话的规则不变。
#[test]
fn an_anonymous_registration_matches_the_copy_the_old_bridge_wrote_back_without_an_id() {
    let root = tempfile::tempdir().unwrap();
    let (settings, keyboard) = (root.path().join("settings"), root.path().join("keyboard"));
    let registration = uuid::Uuid::new_v4();
    let anonymous = |directory: &Path| {
        BackendAccountSession::new(NoNetwork, LockedAnonymousSessionStorage::new(directory))
    };
    let anonymous_storage = |directory: &Path| LockedAnonymousSessionStorage::new(directory);
    anonymous_storage(&settings)
        .save(&with_id(saved("person-a", None, 30, 'b'), registration))
        .unwrap();
    anonymous_storage(&keyboard)
        .save(&saved("person-a", None, 90, 'c'))
        .unwrap();
    let anonymous_local = |directory: &Path| {
        let saved = anonymous_storage(directory).load().unwrap().unwrap();
        (saved.tokens.refresh_token, saved.session_id)
    };
    let token = |refresh: char| refresh.to_string().repeat(64);

    // 设置应用发布的是用过的 `b`：键盘留着更新的 `c`。
    assert_eq!(
        anonymous(&keyboard).adopt_anonymous_session(
            anonymous_storage(&settings).load().unwrap(),
            SessionAdoptSource::Settings
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(anonymous_local(&keyboard), (token('c'), None));
    // 设置应用采用键盘的 `c`，写进文件的是由 `c` 派生的登录 ID。
    assert_eq!(
        anonymous(&settings).adopt_anonymous_session(
            anonymous_storage(&keyboard).load().unwrap(),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(
        anonymous_local(&settings),
        (token('c'), Some(legacy_session_id(&token('c'))))
    );

    // 派生 ID 的一边与注册时的 v4 仍算同一次登录；换上那个 v4 以后，两边都是 v4、但不相同时不是同一次登录，键盘发布的再新也不采用。
    assert_eq!(
        anonymous(&settings).adopt_anonymous_session(
            Some(with_id(saved("person-a", None, 240, 'd'), registration)),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(
        anonymous(&settings).adopt_anonymous_session(
            Some(with_id(
                saved("person-a", None, 480, 'e'),
                uuid::Uuid::new_v4()
            )),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(anonymous_local(&settings), (token('d'), Some(registration)));
    // 另一个匿名账号不论有没有登录 ID 都不算。
    assert_eq!(
        anonymous(&settings).adopt_anonymous_session(
            Some(saved("person-b", None, 480, 'e')),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );

    // 同样两份登录会话仍按登录会话的规则：v4 与没有 ID 的副本不算同一次登录。
    storage(&settings)
        .save(&with_id(saved("person-a", None, 30, 'b'), registration))
        .unwrap();
    assert_eq!(
        session(&settings).adopt_session(
            Some(saved("person-a", None, 90, 'c')),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(local(&settings), has("person-a", 'b'));
}
