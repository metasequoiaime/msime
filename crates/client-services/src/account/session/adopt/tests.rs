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
}

#[test]
fn a_missing_publisher_clears_only_the_account_session_and_only_from_settings() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("sandbox");
    storage(&directory)
        .save(&saved("person-a", Some(1), 60, 'b'))
        .unwrap();
    let anonymous_storage = || LockedAnonymousSessionStorage::new(&directory);
    let anonymous = || BackendAccountSession::new(NoNetwork, anonymous_storage());
    anonymous_storage()
        .save(&saved("msime-anonymous", Some(2), 60, 'c'))
        .unwrap();
    let anonymous_local = || {
        anonymous_storage()
            .load()
            .unwrap()
            .map(|saved| saved.tokens.refresh_token)
    };

    // 键盘那边没有登录会话：设置应用不动自己的。
    assert_eq!(
        session(&directory).adopt_session(None, SessionAdoptSource::Keyboard),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(local(&directory), has("person-a", 'b'));
    // 匿名会话不论哪一边没有都不清：匿名账号没有退出登录，清掉只会逼着重新注册。
    for from in [SessionAdoptSource::Settings, SessionAdoptSource::Keyboard] {
        assert_eq!(
            anonymous().adopt_anonymous_session(None, from),
            Ok(SessionAdoptOutcome::Unchanged)
        );
        assert_eq!(anonymous_local(), Some("c".repeat(64)));
    }
    // 只有设置应用那边没有登录会话时才清，匿名会话仍在。
    assert_eq!(
        session(&directory).adopt_session(None, SessionAdoptSource::Settings),
        Ok(SessionAdoptOutcome::Cleared)
    );
    assert_eq!(local(&directory), None);
    assert_eq!(anonymous_local(), Some("c".repeat(64)));
}

/// 用户没打开过设置应用、先启用了键盘：键盘注册了匿名账号 K 并发布，设置应用第一次启动时本地还没有匿名会话（或者读不了），采用 K，不再注册第二个匿名账号。登录会话没有这条：键盘发布的登录会话在本地没有时仍不用，设置应用里退出登录以后不会被键盘的旧副本登回去。
#[test]
fn the_settings_app_takes_the_anonymous_account_the_keyboard_registered_first() {
    let root = tempfile::tempdir().unwrap();
    let settings = root.path().join("settings");
    let anonymous_storage = || LockedAnonymousSessionStorage::new(&settings);
    let anonymous = || BackendAccountSession::new(NoNetwork, anonymous_storage());
    let anonymous_local = || {
        anonymous_storage()
            .load()
            .unwrap()
            .map(|saved| (saved.tokens.user.id, saved.tokens.refresh_token))
    };
    let registered_by_keyboard = saved("msime-keyboard00000000", Some(1), 60, 'b');

    // 本地没有匿名会话。
    assert_eq!(
        anonymous().adopt_anonymous_session(
            Some(registered_by_keyboard.clone()),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(anonymous_local(), has("msime-keyboard00000000", 'b'));
    assert_eq!(
        anonymous().status().unwrap().unwrap().id,
        "msime-keyboard00000000"
    );

    // 本地的匿名会话读不了（内容坏了）：同样采用。
    private_file(&settings.join(ANONYMOUS_SESSION_FILE), b"not a session");
    assert!(matches!(
        anonymous_storage().load(),
        Err(AccountError::Storage)
    ));
    assert_eq!(
        anonymous().adopt_anonymous_session(
            Some(registered_by_keyboard.clone()),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(anonymous_local(), has("msime-keyboard00000000", 'b'));

    // 设置应用已经有自己的匿名账号时，键盘发布的另一个匿名账号不用，规则与换号相同。
    anonymous_storage()
        .save(&saved("msime-settings00000000", Some(2), 30, 'c'))
        .unwrap();
    assert_eq!(
        anonymous().adopt_anonymous_session(
            Some(registered_by_keyboard.clone()),
            SessionAdoptSource::Keyboard
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(anonymous_local(), has("msime-settings00000000", 'c'));

    // 登录会话不变：本地没有、读不了时键盘发布的都不用。
    let signed_in = saved("person-a", Some(3), 60, 'd');
    assert_eq!(
        session(&settings).adopt_session(Some(signed_in.clone()), SessionAdoptSource::Keyboard),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(local(&settings), None);
    private_file(&settings.join(ACCOUNT_SESSION_FILE), b"not a session");
    // 读不了时结论单独报出来：它取决于本地此刻的状态，宿主不能把这份发布物记成「不用再试」。
    assert_eq!(
        session(&settings).adopt_session(Some(signed_in.clone()), SessionAdoptSource::Keyboard),
        Ok(SessionAdoptOutcome::LocalUnreadable)
    );
    assert_eq!(
        std::fs::read(settings.join(ACCOUNT_SESSION_FILE)).unwrap(),
        b"not a session"
    );
    // 本地恢复可读、是同一个账号刷新前的那份：同一份发布物这次就该采用。
    storage(&settings)
        .save(&saved("person-a", Some(3), 30, 'e'))
        .unwrap();
    assert_eq!(
        session(&settings).adopt_session(Some(signed_in), SessionAdoptSource::Keyboard),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(local(&settings), has("person-a", 'd'));
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
}

#[test]
fn the_later_expiry_wins_in_both_directions_and_a_tie_keeps_the_local_copy() {
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

    // 设置应用发布的同一个账号也按新者胜：键盘自己刷新出来的更新副本留着。
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

    // 过期时间相同时两个方向都不动本地。
    let mut tie = storage(&keyboard).load().unwrap().unwrap();
    tie.tokens.refresh_token = "e".repeat(64);
    for from in [SessionAdoptSource::Settings, SessionAdoptSource::Keyboard] {
        assert_eq!(
            session(&keyboard).adopt_session(Some(tie.clone()), from),
            Ok(SessionAdoptOutcome::Unchanged)
        );
        assert_eq!(local(&keyboard), has("person-a", '9'));
    }
}

/// 换上指定的登录 ID（`None` 是还没有 ID 的旧文件）。
fn with_id(mut session: SavedAccountSession, id: Option<uuid::Uuid>) -> SavedAccountSession {
    session.session_id = id;
    session
}

/// 上一版按刷新令牌派生、写进过文件的那种 UUID v8。
fn v8(byte: u8) -> Option<uuid::Uuid> {
    Some(uuid::Builder::from_custom_bytes([byte; 16]).into_uuid())
}

fn v4() -> Option<uuid::Uuid> {
    Some(uuid::Uuid::new_v4())
}

/// 在 `directory` 里按 `item` 采用 `published`，不联网。
fn adopt_offline(
    item: Item,
    directory: &Path,
    published: SavedAccountSession,
    from: SessionAdoptSource,
) -> Result<SessionAdoptOutcome, AccountError> {
    match item {
        Item::Account => session(directory).adopt_session(Some(published), from),
        Item::Anonymous => {
            BackendAccountSession::new(NoNetwork, LockedAnonymousSessionStorage::new(directory))
                .adopt_anonymous_session(Some(published), from)
        }
    }
}

/// `item` 那份会话（登录会话是 `account-session.json`，匿名会话是 `anonymous-session.json`）在 `directory` 里存成 `value`。
fn save_item(item: Item, directory: &Path, value: &SavedAccountSession) {
    match item {
        Item::Account => storage(directory).save(value),
        Item::Anonymous => LockedAnonymousSessionStorage::new(directory).save(value),
    }
    .unwrap()
}

/// `item` 那份会话在 `directory` 里的（刷新令牌、登录 ID）。
fn load_item(item: Item, directory: &Path) -> (String, Option<uuid::Uuid>) {
    let saved = match item {
        Item::Account => storage(directory).load(),
        Item::Anonymous => LockedAnonymousSessionStorage::new(directory).load(),
    }
    .unwrap()
    .unwrap();
    (saved.tokens.refresh_token, saved.session_id)
}

/// 同一个账号的两份会话不论登录 ID 是什么（都没有、一边没有、一样、不一样，v4 还是 v8），两个方向、登录会话和匿名会话都只按 `expires_at` 新者胜。升级窗口里各种来历的 ID 都会碰到：ArkTS 写的扁平登录会话没有 ID，Rust 注册的匿名会话是 v4、旧桥接写回时丢了，各沙箱第一次认证前各自补上的 ID 互不相同。
#[test]
fn the_login_id_does_not_decide_which_copy_wins() {
    let shared = v4();
    let cases = [
        (None, None),
        (None, v4()),
        (v4(), None),
        (shared, shared),
        (v4(), v4()),
        (v8(1), v8(2)),
        (v8(1), None),
        (v4(), v8(2)),
    ];
    for (local_id, published_id) in cases {
        for from in [SessionAdoptSource::Settings, SessionAdoptSource::Keyboard] {
            for item in [Item::Account, Item::Anonymous] {
                let root = tempfile::tempdir().unwrap();
                let directory = root.path().join("sandbox");
                let context = format!("{local_id:?} {published_id:?} {from:?} {item:?}");
                save_item(
                    item,
                    &directory,
                    &with_id(saved("person-a", None, 30, 'b'), local_id),
                );
                // 发布的是用过的旧副本：留着本地更新的那份。
                let older = with_id(saved("person-a", None, 10, 'a'), published_id);
                assert_eq!(
                    adopt_offline(item, &directory, older, from),
                    Ok(SessionAdoptOutcome::Unchanged),
                    "{context}"
                );
                assert_eq!(
                    load_item(item, &directory),
                    ("b".repeat(64), local_id),
                    "{context}"
                );
                // 发布的是刷新过的：采用它，原样写进文件。
                let newer = with_id(saved("person-a", None, 90, 'c'), published_id);
                assert_eq!(
                    adopt_offline(item, &directory, newer, from),
                    Ok(SessionAdoptOutcome::Adopted),
                    "{context}"
                );
                assert_eq!(
                    load_item(item, &directory),
                    ("c".repeat(64), published_id),
                    "{context}"
                );
            }
        }
    }
}

#[test]
fn signing_in_again_with_the_same_account_wins_over_the_previous_sign_in() {
    let root = tempfile::tempdir().unwrap();
    let (settings, keyboard) = (root.path().join("settings"), root.path().join("keyboard"));
    // 上一次登录在键盘里五分钟前刷新过，十分钟后过期；设置应用里刚退出又登录了同一个账号，新的一次十五分钟后过期。
    let previous = with_id(saved("person-a", None, 10, 'b'), v4());
    let again = with_id(saved("person-a", None, 15, 'c'), v4());
    storage(&keyboard).save(&previous).unwrap();
    storage(&settings).save(&again).unwrap();

    let keyboard_session = session(&keyboard);
    let generation = keyboard_session
        .with_current_identity(|identity| Ok(identity.unwrap().1))
        .unwrap();
    assert_eq!(
        keyboard_session.adopt_session(Some(again.clone()), SessionAdoptSource::Settings),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(local(&keyboard), has("person-a", 'c'));
    // 换成了另一次登录：上一次登录下开始的本地操作作废。
    assert_eq!(
        keyboard_session.with_generation(generation, Some("person-a"), || Ok(())),
        Err(AccountError::Cancelled)
    );
    // 上一次登录的旧副本再发布过来，哪个方向都换不回去。
    assert_eq!(
        keyboard_session.adopt_session(Some(previous.clone()), SessionAdoptSource::Settings),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(
        session(&settings).adopt_session(Some(previous.clone()), SessionAdoptSource::Keyboard),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(local(&settings), has("person-a", 'c'));

    // 反过来设置应用还留着上一次登录、键盘里是新的一次（两边都只比过期时间）：设置应用采用键盘的。
    storage(&settings).save(&previous).unwrap();
    assert_eq!(
        session(&settings).adopt_session(Some(again), SessionAdoptSource::Keyboard),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(local(&settings), has("person-a", 'c'));
}

#[test]
fn adopting_a_refresh_of_the_same_sign_in_keeps_work_in_progress() {
    let root = tempfile::tempdir().unwrap();
    let keyboard = root.path().join("keyboard");
    let id = v4();
    storage(&keyboard)
        .save(&with_id(saved("person-a", None, 10, 'b'), id))
        .unwrap();
    let keyboard_session = session(&keyboard);
    let generation = keyboard_session
        .with_current_identity(|identity| Ok(identity.unwrap().1))
        .unwrap();
    assert_eq!(
        keyboard_session.adopt_session(
            Some(with_id(saved("person-a", None, 15, 'c'), id)),
            SessionAdoptSource::Settings
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(
        keyboard_session.with_generation(generation, Some("person-a"), || Ok(())),
        Ok(())
    );
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
#[derive(Clone, Copy, Debug)]
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

/// ArkTS 桥接存的扁平登录会话，`minutes_ago` 分钟前过期。
fn flat_session_expired(refresh: char, minutes_ago: u64) -> Vec<u8> {
    format!(
        r#"{{"access_token":"{}","refresh_token":"{}","token_type":"Bearer","expires_at":{},"user":{{"id":"person-a","display_name":"","created_at":"2026-01-01T00:00:00Z"}}}}"#,
        "a".repeat(64),
        refresh.to_string().repeat(64),
        now_ms() - minutes_ago * 60_000
    )
    .into_bytes()
}

/// ArkTS 桥接存的扁平登录会话，一小时前过期。
fn flat_session(refresh: char) -> Vec<u8> {
    flat_session_expired(refresh, 60)
}

/// `minutes_ago` 分钟前过期的 Native 会话，登录 ID 是 `session_id`。
fn native_session_expired(
    refresh: char,
    minutes_ago: u64,
    session_id: Option<uuid::Uuid>,
) -> Vec<u8> {
    let mut expired = saved("person-a", None, 60, refresh);
    expired.expires_at_unix_ms = now_ms() - minutes_ago * 60_000;
    expired.session_id = session_id;
    serde_json::to_vec(&expired).unwrap()
}

/// ArkTS 存的匿名会话是 Native 布局，但同样没有 `session_id`，一小时前过期。
fn anonymous_session(refresh: char) -> Vec<u8> {
    let native = native_session_expired(refresh, 60, None);
    assert!(!String::from_utf8_lossy(&native).contains("session_id"));
    native
}

/// Rust 注册（`sign_in_anonymous`）时写的匿名会话：随机的 v4 登录 ID，一小时前过期。
fn registered_anonymous_session(refresh: char) -> Vec<u8> {
    native_session_expired(refresh, 60, Some(uuid::Uuid::new_v4()))
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

/// 升级前旧版 ArkTS 已经在一边把刷新令牌从 `b` 轮换成 `c`，另一边还停在用过的 `b`（`rotated_in_settings` 选是哪一边）。不论哪个方向导入：持有 `c` 的一边不会被用过的 `b` 盖掉，停在 `b` 的一边采用 `c`，之后拿 `c` 刷新被服务端接受，另一边再导入刷新后的那份。`stale` 和 `rotated` 是两边旧文件的内容，`rotated` 后签发、过期得更晚。
fn assert_the_rotated_copy_wins_after_the_upgrade<S: AccountSessionStorage>(
    file: &str,
    stale: Vec<u8>,
    rotated: Vec<u8>,
    open: impl Fn(&Path) -> S,
    item: Item,
    rotated_in_settings: bool,
) {
    let root = tempfile::tempdir().unwrap();
    let (settings, keyboard) = (root.path().join("settings"), root.path().join("keyboard"));
    let api = RotatingApi::default();
    let token = |refresh: char| refresh.to_string().repeat(64);
    api.presented.lock().unwrap().push(token('b'));
    let (newer, older) = if rotated_in_settings {
        (&settings, &keyboard)
    } else {
        (&keyboard, &settings)
    };
    for (directory, contents) in [(newer, &rotated), (older, &stale)] {
        storage(directory).create_directory().unwrap();
        private_file(&directory.join(file), contents);
    }
    let published_by = |directory: &Path| {
        let source = if directory == settings {
            SessionAdoptSource::Settings
        } else {
            SessionAdoptSource::Keyboard
        };
        (
            read_published_session(&directory.join(file)).unwrap(),
            source,
        )
    };
    let refresh_token = |directory: &Path| {
        open(directory)
            .load()
            .unwrap()
            .unwrap()
            .tokens
            .refresh_token
    };
    let newer_session = BackendAccountSession::new(api.clone(), open(newer));
    let older_session = BackendAccountSession::new(api.clone(), open(older));
    let context = format!("{file} rotated_in_settings={rotated_in_settings}");

    // 持有 `c` 的一边导入用过的 `b`：不动。
    let (published, from) = published_by(older);
    assert_eq!(
        adopt(item, &newer_session, published, from),
        Ok(SessionAdoptOutcome::Unchanged),
        "{context}"
    );
    assert_eq!(refresh_token(newer), token('c'), "{context}");
    // 停在 `b` 的一边采用 `c`，下一次刷新交出的是 `c`。
    let (published, from) = published_by(newer);
    assert_eq!(
        adopt(item, &older_session, published, from),
        Ok(SessionAdoptOutcome::Adopted),
        "{context}"
    );
    assert_eq!(refresh_token(older), token('c'), "{context}");
    older_session.credentials(None, None).unwrap();
    assert_eq!(
        *api.presented.lock().unwrap(),
        [token('b'), token('c')],
        "{context}"
    );
    assert_eq!(refresh_token(older), token('d'), "{context}");
    // 原来持有 `c` 的一边再导入刷新后的那份。
    let (published, from) = published_by(older);
    assert_eq!(
        adopt(item, &newer_session, published, from),
        Ok(SessionAdoptOutcome::Adopted),
        "{context}"
    );
    assert_eq!(refresh_token(newer), token('d'), "{context}");
}

/// 旧版 ArkTS 在键盘或设置应用里已经轮换过一次：登录会话两边都是扁平布局，或者轮换过的那份已经被 Rust 补上了随机的登录 ID（v4）；匿名会话一边是注册时的 v4，另一边是旧桥接写回时丢了 ID 的、或者又被 Rust 补上了另一个 v4 的。
#[test]
fn an_old_token_never_overwrites_the_copy_the_old_arkts_already_rotated() {
    for rotated_in_settings in [false, true] {
        for rotated in [
            flat_session_expired('c', 50),
            native_session_expired('c', 50, Some(uuid::Uuid::new_v4())),
        ] {
            assert_the_rotated_copy_wins_after_the_upgrade(
                ACCOUNT_SESSION_FILE,
                flat_session('b'),
                rotated,
                storage,
                Item::Account,
                rotated_in_settings,
            );
        }
        for rotated in [
            native_session_expired('c', 50, None),
            native_session_expired('c', 50, Some(uuid::Uuid::new_v4())),
        ] {
            assert_the_rotated_copy_wins_after_the_upgrade(
                ANONYMOUS_SESSION_FILE,
                registered_anonymous_session('b'),
                rotated,
                LockedAnonymousSessionStorage::new,
                Item::Anonymous,
                rotated_in_settings,
            );
        }
    }
}

/// 一份合成的匿名身份文件内容：`subject` 是 `msime-` 后 16 个字符，`secret` 是 48 个字符，都由 `fill` 重复而成。
fn identity_bytes(fill: char) -> Vec<u8> {
    format!(
        r#"{{"subject":"msime-{}","secret":"{}"}}"#,
        fill.to_string().repeat(16),
        fill.to_string().repeat(48)
    )
    .into_bytes()
}

/// 本地身份的 subject，没有身份时是 `None`。
fn identity_subject(directory: &Path) -> Option<String> {
    match std::fs::read(directory.join(ANONYMOUS_ACCOUNT_FILE)) {
        Ok(bytes) => Some(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["subject"]
                .as_str()
                .unwrap()
                .to_owned(),
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("{error}"),
    }
}

fn published_identity(directory: &Path) -> PublishedAnonymousIdentity {
    read_published_anonymous_identity(&directory.join(ANONYMOUS_ACCOUNT_FILE)).unwrap()
}

/// 身份跟着匿名会话走（负责人决定，迁移差异 M40）：设置应用第一次注册断网失败、留着身份 S 没有会话，随后采用键盘注册的匿名账号 K 时连同 K 的身份一起采用，不会拿着 K 的会话、留着 S 的身份。
#[test]
fn the_anonymous_identity_follows_the_adopted_anonymous_session() {
    let root = tempfile::tempdir().unwrap();
    let (settings, keyboard) = (root.path().join("settings"), root.path().join("keyboard"));
    for directory in [&settings, &keyboard] {
        std::fs::create_dir_all(directory).unwrap();
    }
    let anonymous = |directory: &Path| {
        BackendAccountSession::new(NoNetwork, LockedAnonymousSessionStorage::new(directory))
    };
    let adopt = |published: SavedAccountSession, identity: &PublishedAnonymousIdentity| {
        anonymous(&settings).adopt_anonymous_session_with_identity(
            Some(published),
            SessionAdoptSource::Keyboard,
            &settings,
            identity,
        )
    };
    private_file(&settings.join(ANONYMOUS_ACCOUNT_FILE), &identity_bytes('s'));
    private_file(&keyboard.join(ANONYMOUS_ACCOUNT_FILE), &identity_bytes('k'));
    let keyboard_account = saved("msime-keyboard00000000", Some(1), 60, 'b');

    // 本地没有匿名会话：采用 K 的会话，身份换成 K 的。
    assert_eq!(
        adopt(keyboard_account.clone(), &published_identity(&keyboard)),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(
        identity_subject(&settings),
        Some(format!("msime-{}", "k".repeat(16)))
    );

    // 没有采用（本地的同一个账号更新）时身份不动，哪怕发布方的身份不同。
    private_file(&keyboard.join(ANONYMOUS_ACCOUNT_FILE), &identity_bytes('x'));
    assert_eq!(
        adopt(
            saved("msime-keyboard00000000", Some(1), 30, 'c'),
            &published_identity(&keyboard)
        ),
        Ok(SessionAdoptOutcome::Unchanged)
    );
    assert_eq!(
        identity_subject(&settings),
        Some(format!("msime-{}", "k".repeat(16)))
    );

    // 同一个账号刷新过、发布方没有身份：采用会话，本地身份留着，它就是这个账号的。
    std::fs::remove_file(keyboard.join(ANONYMOUS_ACCOUNT_FILE)).unwrap();
    assert_eq!(
        adopt(
            saved("msime-keyboard00000000", Some(1), 90, 'd'),
            &published_identity(&keyboard)
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(
        identity_subject(&settings),
        Some(format!("msime-{}", "k".repeat(16)))
    );

    // 换了账号、发布方没有身份（这里是设置应用发布给键盘）：本地身份属于另一个账号，删掉。
    let keyboard_session = anonymous(&keyboard);
    keyboard_session
        .storage
        .save(&saved("msime-keyboard00000000", Some(1), 60, 'e'))
        .unwrap();
    private_file(&keyboard.join(ANONYMOUS_ACCOUNT_FILE), &identity_bytes('k'));
    let empty = root.path().join("published-without-identity");
    assert_eq!(
        keyboard_session.adopt_anonymous_session_with_identity(
            Some(saved("msime-settings00000000", Some(2), 60, 'f')),
            SessionAdoptSource::Settings,
            &keyboard,
            &published_identity(&empty),
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(identity_subject(&keyboard), None);
    assert_eq!(
        keyboard_session.status().unwrap().unwrap().id,
        "msime-settings00000000"
    );
}

/// 会话写不进去时身份还原：不留下与本地会话不是同一个账号的身份。
#[test]
fn a_failed_session_write_puts_the_previous_identity_back() {
    let root = tempfile::tempdir().unwrap();
    let (settings, keyboard) = (root.path().join("settings"), root.path().join("keyboard"));
    for directory in [&settings, &keyboard] {
        std::fs::create_dir_all(directory).unwrap();
    }
    private_file(&settings.join(ANONYMOUS_ACCOUNT_FILE), &identity_bytes('s'));
    private_file(&keyboard.join(ANONYMOUS_ACCOUNT_FILE), &identity_bytes('k'));
    // 会话文件的位置被一个非空目录占着：读不了（于是要采用），也写不进去。
    std::fs::create_dir_all(settings.join(ANONYMOUS_SESSION_FILE).join("occupied")).unwrap();

    assert_eq!(
        BackendAccountSession::new(NoNetwork, LockedAnonymousSessionStorage::new(&settings))
            .adopt_anonymous_session_with_identity(
                Some(saved("msime-keyboard00000000", Some(1), 60, 'b')),
                SessionAdoptSource::Keyboard,
                &settings,
                &published_identity(&keyboard),
            ),
        Err(AccountError::Storage)
    );
    assert_eq!(
        identity_subject(&settings),
        Some(format!("msime-{}", "s".repeat(16)))
    );
}

/// 发布方的身份不合法时读它就失败，`session_adopt` 什么也不写。
#[test]
fn an_invalid_published_identity_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(ANONYMOUS_ACCOUNT_FILE);
    private_file(&path, br#"{"subject":"msime-SHORT","secret":"x"}"#);
    assert!(matches!(
        read_published_anonymous_identity(&path),
        Err(AccountError::Storage)
    ));
    private_file(&path, b"not json");
    assert!(matches!(
        read_published_anonymous_identity(&path),
        Err(AccountError::Storage)
    ));
}

/// 记下每次写会话那一刻本地身份的 subject。
struct IdentityAtSave {
    inner: LockedAnonymousSessionStorage,
    directory: std::path::PathBuf,
    seen: Mutex<Vec<Option<String>>>,
}

impl AccountSessionStorage for IdentityAtSave {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        self.inner.load()
    }
    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        self.seen
            .lock()
            .unwrap()
            .push(identity_subject(&self.directory));
        self.inner.save(session)
    }
    fn clear(&self) -> Result<(), AccountError> {
        self.inner.clear()
    }
    fn shared_across_processes(&self) -> bool {
        true
    }
    fn with_refresh_lock<T>(
        &self,
        body: impl FnOnce() -> Result<T, AccountError>,
    ) -> Result<T, AccountError> {
        self.inner.with_refresh_lock(body)
    }
}

/// 身份先于会话写：两次写之间崩溃时留下的是发布方的身份、原来的会话，下一次采用的输入不变、结论仍是 adopted，会写上会话。反过来先写会话，崩溃后本地已是发布的会话，再采用是 unchanged，身份就再也换不过来。
#[test]
fn the_identity_is_in_place_before_the_session_is_written() {
    let root = tempfile::tempdir().unwrap();
    let (settings, keyboard) = (root.path().join("settings"), root.path().join("keyboard"));
    for directory in [&settings, &keyboard] {
        std::fs::create_dir_all(directory).unwrap();
    }
    private_file(&settings.join(ANONYMOUS_ACCOUNT_FILE), &identity_bytes('s'));
    private_file(&keyboard.join(ANONYMOUS_ACCOUNT_FILE), &identity_bytes('k'));
    let session = BackendAccountSession::new(
        NoNetwork,
        IdentityAtSave {
            inner: LockedAnonymousSessionStorage::new(&settings),
            directory: settings.clone(),
            seen: Mutex::new(Vec::new()),
        },
    );
    assert_eq!(
        session.adopt_anonymous_session_with_identity(
            Some(saved("msime-keyboard00000000", Some(1), 60, 'b')),
            SessionAdoptSource::Keyboard,
            &settings,
            &published_identity(&keyboard),
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    assert_eq!(
        *session.storage.seen.lock().unwrap(),
        [Some(format!("msime-{}", "k".repeat(16)))]
    );
}

/// 登录请求停在服务端、等测试放行的接口：`started` 在请求到达时收到一次，`release` 收到以后返回 `tokens`。
struct LoginInFlight {
    started: Mutex<mpsc::Sender<()>>,
    release: Mutex<mpsc::Receiver<()>>,
    tokens: AccountTokens,
}

impl AccountApi for LoginInFlight {
    fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError> {
        panic!("only login is expected")
    }
    fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
        panic!("only login is expected")
    }
    fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
        self.started.lock().unwrap().send(()).unwrap();
        self.release.lock().unwrap().recv().unwrap();
        Ok(self.tokens.clone())
    }
    fn refresh(&self, _: &str) -> Result<AccountTokens, AccountError> {
        panic!("only login is expected")
    }
    fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
        panic!("only login is expected")
    }
    fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
        panic!("only login is expected")
    }
    fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
        panic!("only login is expected")
    }
    fn delete_account(&self, _: &str) -> Result<(), AccountError> {
        panic!("only login is expected")
    }
}

/// 同一个对象上的登录还在路上时采用了设置应用的换号：登录回来以后不能把它盖掉。这个对象还没读过存储（`loaded` 为假）时，采用以前不推进会话代次，登录在锁里看到的代次没变，就把自己的会话写了上去。
#[test]
fn a_sign_in_in_flight_does_not_overwrite_an_adopted_account_switch() {
    let root = tempfile::tempdir().unwrap();
    let keyboard = root.path().join("keyboard");
    storage(&keyboard)
        .save(&saved("person-a", Some(1), 60, 'b'))
        .unwrap();
    let (started, wait) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let keyboard_session = std::sync::Arc::new(BackendAccountSession::new(
        LoginInFlight {
            started: Mutex::new(started),
            release: Mutex::new(released),
            tokens: saved("person-b", None, 60, 'c').tokens,
        },
        storage(&keyboard),
    ));
    let signing_in = {
        let keyboard_session = std::sync::Arc::clone(&keyboard_session);
        std::thread::spawn(move || keyboard_session.sign_in("synthetic-challenge", "123456"))
    };
    wait.recv().unwrap();
    assert_eq!(
        keyboard_session.adopt_session(
            Some(saved("person-c", Some(3), 60, 'd')),
            SessionAdoptSource::Settings
        ),
        Ok(SessionAdoptOutcome::Adopted)
    );
    release.send(()).unwrap();
    assert_eq!(signing_in.join().unwrap(), Err(AccountError::Cancelled));
    assert_eq!(local(&keyboard), has("person-c", 'd'));
    assert_eq!(
        keyboard_session.status().unwrap().map(|user| user.id),
        Some("person-c".to_owned())
    );
}
