//! The device's anonymous MSIME account, registered when the client is first installed or run so services that need an account work before anyone signs in.
//!
//! The identity is a client-generated subject and secret, the same shape the macOS/iOS (`shared/backend/account/BackendAnonymousAccount.swift`), Android (`BackendAnonymousAccount.java`) and Linux (`msime-linux-online-provider`) clients create: `msime-` plus 16 characters, and a 48-character secret, both drawn from `[a-z0-9]`. Whoever presents the subject first creates the account. The files are those clients' names and layout too: `anonymous-account.json` holds `{subject, secret}` and `anonymous-session.json` holds a `SavedAccountSession`.

use super::*;
use std::path::PathBuf;

pub const ANONYMOUS_ACCOUNT_FILE: &str = "anonymous-account.json";
pub const ANONYMOUS_SESSION_FILE: &str = "anonymous-session.json";
const SUBJECT_PREFIX: &str = "msime-";
const SUBJECT_LENGTH: usize = 16;
const SECRET_LENGTH: usize = 48;
const ALPHABET: &[u8; 36] = b"abcdefghijklmnopqrstuvwxyz0123456789";
const MAX_FILE_BYTES: u64 = 64 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub struct AnonymousIdentity {
    pub subject: String,
    pub secret: String,
}

impl AnonymousIdentity {
    fn generate() -> Self {
        Self {
            subject: format!("{SUBJECT_PREFIX}{}", random_text(SUBJECT_LENGTH)),
            secret: random_text(SECRET_LENGTH),
        }
    }

    fn is_valid(&self) -> bool {
        self.subject
            .strip_prefix(SUBJECT_PREFIX)
            .is_some_and(|suffix| valid_anonymous_text(suffix, SUBJECT_LENGTH))
            && valid_anonymous_text(&self.secret, SECRET_LENGTH)
    }
}

pub(super) fn valid_anonymous_subject(value: &str) -> bool {
    value
        .strip_prefix(SUBJECT_PREFIX)
        .is_some_and(|suffix| valid_anonymous_text(suffix, SUBJECT_LENGTH))
}

fn valid_anonymous_text(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| ALPHABET.contains(&byte))
}

/// Uniform characters from the OS random source. A v4 UUID is 122 bits from `getrandom`, which is the CSPRNG every supported platform provides; bytes past the largest multiple of 36 are rejected so no character is favoured.
fn random_text(length: usize) -> String {
    let mut text = String::with_capacity(length);
    while text.len() < length {
        for byte in uuid::Uuid::new_v4().into_bytes() {
            if byte < 252 && text.len() < length {
                text.push(char::from(ALPHABET[usize::from(byte % 36)]));
            }
        }
    }
    text
}

/// `anonymous-session.json` in the host's directory, written the way the other clients write it: replaced whole, readable only by the owner.
pub struct AnonymousSessionStorage {
    directory: PathBuf,
}

impl AnonymousSessionStorage {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }
}

impl AccountSessionStorage for AnonymousSessionStorage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        read_private_json(&self.directory.join(ANONYMOUS_SESSION_FILE))
    }

    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        let bytes = serde_json::to_vec(session).map_err(|_| AccountError::Storage)?;
        prepare_directory(&self.directory)?;
        write_private(&self.directory, ANONYMOUS_SESSION_FILE, &bytes, false).map(|_| ())
    }

    fn clear(&self) -> Result<(), AccountError> {
        crate::storage::reject_symlink(&self.directory).map_err(|_| AccountError::Storage)?;
        match crate::storage::remove_private_file(&self.directory.join(ANONYMOUS_SESSION_FILE)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(AccountError::Storage),
        }
    }
}

/// Registers the device's anonymous account under `directory` unless it already has a session there. Blocks on the network: call it off the thread that handles input. Safe to call on every start; once registered it only reads the session file.
pub fn ensure_anonymous_account(directory: &Path) -> Result<(), AccountError> {
    // Checked before building the HTTP client, so a start after registration costs one file read.
    if has_session(directory) {
        return Ok(());
    }
    ensure_anonymous_account_with(BackendAccountClient::new()?, directory)
}

pub(super) fn ensure_anonymous_account_with<A: AccountApi>(
    api: A,
    directory: &Path,
) -> Result<(), AccountError> {
    prepare_directory(directory)?;
    if has_session(directory) {
        return Ok(());
    }
    let identity = anonymous_identity(directory)?;
    BackendAccountSession::new(api, AnonymousSessionStorage::new(directory))
        .sign_in_anonymous(&identity.subject, &identity.secret)
        .map(|_| ())
}

/// 输入法进程调用账号服务时用的令牌，以及它属于哪个账号。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SharedAccessToken {
    pub access_token: String,
    pub user_id: String,
    /// 设置应用里没有登录、用的是本机匿名账号时为真。
    pub anonymous: bool,
}

/// 输入法进程取账号令牌。`directory` 是设置应用和输入法共用的账号目录：设置应用登录后把 `account-session.json`（[`FileAccountSessionStorage`] 的 Native 格式）写在这里，输入法安装后注册的匿名会话 `anonymous-session.json` 也在这里。已登录的账号优先，刷新时与设置应用拿同一把 `account-refresh.lock`，两个进程不会用对方已经用掉的刷新令牌；没有登录时用匿名会话。每次调用都重新读文件，所以设置应用里的登录、退出和换号下一次调用就生效，不需要进程间通知。`rejected_token` 是服务端刚以 401 拒绝的令牌，传入后强制刷新。刷新会阻塞网络，调用方放在输入线程以外。
pub fn shared_access_token(
    directory: &Path,
    rejected_token: Option<&str>,
) -> Result<SharedAccessToken, AccountError> {
    shared_access_token_with(BackendAccountClient::new()?, directory, rejected_token)
}

pub(super) fn shared_access_token_with<A: AccountApi + Clone>(
    api: A,
    directory: &Path,
    rejected_token: Option<&str>,
) -> Result<SharedAccessToken, AccountError> {
    let signed_in = BackendAccountSession::new(
        api.clone(),
        FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Native),
    );
    match signed_in.credentials(rejected_token, None) {
        Ok((user_id, access_token)) => {
            return Ok(SharedAccessToken {
                access_token,
                user_id,
                anonymous: false,
            })
        }
        // 没有登录，或者登录的会话已被服务端吊销并清掉：退回匿名账号。
        Err(AccountError::Unauthorized) => {}
        Err(error) => return Err(error),
    }
    BackendAccountSession::new(api, LockedAnonymousSessionStorage::new(directory))
        .credentials(rejected_token, None)
        .map(|(user_id, access_token)| SharedAccessToken {
            access_token,
            user_id,
            anonymous: true,
        })
}

/// [`shared_access_token`] 读匿名会话用的存储：文件仍是 `anonymous-session.json`，但每次都重新读，刷新时持与登录会话同一把 `account-refresh.lock`。输入法进程里几个线程可能同时取令牌，每次调用又各建一个会话，不加锁时它们会拿同一个刷新令牌各刷新一次，而服务端见到用过的刷新令牌会吊销整个会话。只用在这里，[`ensure_anonymous_account`] 和其他宿主读写匿名会话的方式不变。
struct LockedAnonymousSessionStorage {
    session: AnonymousSessionStorage,
    lock: FileAccountSessionStorage,
}

impl LockedAnonymousSessionStorage {
    fn new(directory: &Path) -> Self {
        Self {
            session: AnonymousSessionStorage::new(directory),
            lock: FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Native),
        }
    }
}

impl AccountSessionStorage for LockedAnonymousSessionStorage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        self.session.load()
    }

    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        self.session.save(session)
    }

    fn clear(&self) -> Result<(), AccountError> {
        self.session.clear()
    }

    fn shared_across_processes(&self) -> bool {
        true
    }

    fn with_refresh_lock<T>(
        &self,
        body: impl FnOnce() -> Result<T, AccountError>,
    ) -> Result<T, AccountError> {
        self.lock.with_refresh_lock(body)
    }
}

/// 只有可读取且有效的会话才跳过重新登录；无效会话用已保存的身份重新领取 token。
fn has_session(directory: &Path) -> bool {
    matches!(
        AnonymousSessionStorage::new(directory).load(),
        Ok(Some(saved)) if super::session::validate_saved_session(&saved).is_ok()
    )
}

/// The saved identity, or a new one. A new identity is published with no-clobber, so two processes starting together (HarmonyOS runs the app and the keyboard separately) end up with the one that landed first rather than registering two accounts.
fn anonymous_identity(directory: &Path) -> Result<AnonymousIdentity, AccountError> {
    let path = directory.join(ANONYMOUS_ACCOUNT_FILE);
    if let Some(saved) = read_private_json::<AnonymousIdentity>(&path)? {
        return saved
            .is_valid()
            .then_some(saved)
            .ok_or(AccountError::Storage);
    }
    let identity = AnonymousIdentity::generate();
    let bytes = serde_json::to_vec(&identity).map_err(|_| AccountError::Storage)?;
    if write_private(directory, ANONYMOUS_ACCOUNT_FILE, &bytes, true)? {
        return Ok(identity);
    }
    read_private_json::<AnonymousIdentity>(&path)?
        .filter(AnonymousIdentity::is_valid)
        .ok_or(AccountError::Storage)
}

fn prepare_directory(directory: &Path) -> Result<(), AccountError> {
    crate::storage::create_directory_and_check(directory).map_err(|_| AccountError::Storage)?;
    let metadata = std::fs::symlink_metadata(directory).map_err(|_| AccountError::Storage)?;
    if !metadata.file_type().is_dir() {
        return Err(AccountError::Storage);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| AccountError::Storage)?;
        }
    }
    Ok(())
}

fn read_private_file(file: std::fs::File) -> Result<Vec<u8>, AccountError> {
    crate::bounded_io::read_bounded_file_with(
        file,
        MAX_FILE_BYTES,
        || AccountError::Storage,
        |_| AccountError::Storage,
    )
}

/// 会话文件不允许带的权限位。鸿蒙的 `@ohos.file.fs` 没有 chmod，宿主 ArkTS 代码轮换匿名会话后重写的文件按应用 umask 带属组读写位；应用沙箱里属组只有本应用，目录也已由 `prepare_directory` 收紧为 0700，所以鸿蒙上只拒绝其他用户位，其他平台仍要求文件只有属主可读写。
#[cfg(any(unix, test))]
fn forbidden_session_mode_bits(ohos: bool) -> u32 {
    if ohos {
        0o007
    } else {
        0o077
    }
}

#[cfg(unix)]
fn validate_opened_private_json_file(
    file: &std::fs::File,
    directory: &std::fs::File,
) -> Result<(), AccountError> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file.metadata().map_err(|_| AccountError::Storage)?;
    let directory_metadata = directory.metadata().map_err(|_| AccountError::Storage)?;
    if !metadata.is_file()
        || metadata.mode() & forbidden_session_mode_bits(cfg!(target_env = "ohos")) != 0
        || metadata.uid() != directory_metadata.uid()
    {
        return Err(AccountError::Storage);
    }
    Ok(())
}

fn read_private_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, AccountError> {
    let parent = path.parent().ok_or(AccountError::Storage)?;
    #[cfg(unix)]
    let name = path.file_name().ok_or(AccountError::Storage)?;
    crate::storage::reject_symlink(parent).map_err(|_| AccountError::Storage)?;
    #[cfg(unix)]
    let bytes = {
        let directory =
            crate::storage::open_private_directory(parent).map_err(|_| AccountError::Storage)?;
        let file = match crate::storage::open_private_file_at(&directory, name) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(AccountError::Storage),
        };
        validate_opened_private_json_file(&file, &directory)?;
        read_private_file(file)?
    };
    #[cfg(not(unix))]
    let bytes = {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(AccountError::Storage),
        };
        if !metadata.file_type().is_file() {
            return Err(AccountError::Storage);
        }
        read_private_file(
            crate::storage::open_private_file_in(path).map_err(|_| AccountError::Storage)?,
        )?
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| AccountError::Storage)
}

/// Stages `bytes` beside the target and publishes it into place. Unix binds
/// both the temporary file and publication to an opened directory handle; on
/// Windows and HarmonyOS the directory is already private to the user or the
/// app. With `no_clobber`, returns `false` instead of replacing a file another
/// process published first.
fn write_private(
    directory: &Path,
    name: &str,
    bytes: &[u8],
    no_clobber: bool,
) -> Result<bool, AccountError> {
    prepare_directory(directory)?;
    #[cfg(unix)]
    {
        let directory_handle =
            crate::storage::open_private_directory(directory).map_err(|_| AccountError::Storage)?;
        let name = std::ffi::OsStr::new(name);
        if no_clobber {
            crate::storage::write_private_file_at_noclobber(&directory_handle, name, bytes)
                .map_err(|_| AccountError::Storage)
        } else {
            crate::storage::write_private_file_at(&directory_handle, name, bytes)
                .map(|_| true)
                .map_err(|_| AccountError::Storage)
        }
    }
    #[cfg(not(unix))]
    {
        let mut temporary =
            tempfile::NamedTempFile::new_in(directory).map_err(|_| AccountError::Storage)?;
        temporary
            .write_all(bytes)
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|_| AccountError::Storage)?;
        let path = directory.join(name);
        if no_clobber {
            return match temporary.persist_noclobber(&path) {
                Ok(_) => Ok(true),
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
                Err(_) => Err(AccountError::Storage),
            };
        }
        temporary
            .persist(&path)
            .map(|_| true)
            .map_err(|_| AccountError::Storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_files_reject_other_users_everywhere_and_group_bits_off_harmony() {
        assert_eq!(0o660 & forbidden_session_mode_bits(true), 0);
        assert_ne!(0o660 & forbidden_session_mode_bits(false), 0);
        assert_ne!(0o604 & forbidden_session_mode_bits(true), 0);
        assert_ne!(0o604 & forbidden_session_mode_bits(false), 0);
        assert_eq!(0o600 & forbidden_session_mode_bits(false), 0);
    }
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone, Default)]
    struct RegisteringApi {
        challenges: Arc<Mutex<Vec<(String, String)>>>,
        logins: Arc<Mutex<Vec<(String, String)>>>,
        failures: Arc<AtomicUsize>,
    }

    impl AccountApi for RegisteringApi {
        fn challenge(
            &self,
            provider: &str,
            target: &str,
        ) -> Result<AccountChallenge, AccountError> {
            validate_provider_target(provider, target)?;
            if self.failures.load(Ordering::SeqCst) > 0 {
                self.failures.fetch_sub(1, Ordering::SeqCst);
                return Err(AccountError::Unavailable);
            }
            self.challenges
                .lock()
                .unwrap()
                .push((provider.into(), target.into()));
            Ok(AccountChallenge {
                challenge_id: "fixture-challenge".into(),
                expires_in: 300,
                nonce: None,
                authorization_url: None,
            })
        }

        fn login(&self, challenge: &str, credential: &str) -> Result<AccountTokens, AccountError> {
            validate_login_request(challenge, credential)?;
            self.logins
                .lock()
                .unwrap()
                .push((challenge.into(), credential.into()));
            Ok(AccountTokens {
                access_token: "a".repeat(64),
                refresh_token: "b".repeat(64),
                token_type: "Bearer".into(),
                expires_in: 3600,
                user: AccountUser {
                    id: "fixture-anonymous".into(),
                    display_name: String::new(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    email: None,
                    avatar_url: None,
                },
            })
        }

        fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError> {
            Ok(Default::default())
        }

        fn refresh(&self, _refresh_token: &str) -> Result<AccountTokens, AccountError> {
            Err(AccountError::Unavailable)
        }

        fn profile(&self, _access_token: &str) -> Result<AccountProfile, AccountError> {
            Err(AccountError::Unavailable)
        }

        fn rename(&self, _display_name: &str, _access_token: &str) -> Result<(), AccountError> {
            Err(AccountError::Unavailable)
        }

        fn logout(&self, _access_token: &str, _all: bool) -> Result<(), AccountError> {
            Err(AccountError::Unavailable)
        }

        fn delete_account(&self, _access_token: &str) -> Result<(), AccountError> {
            Err(AccountError::Unavailable)
        }
    }

    #[test]
    fn registers_once_with_the_shared_identity_shape() {
        let dir = tempfile::tempdir().unwrap();
        let api = RegisteringApi::default();
        ensure_anonymous_account_with(api.clone(), dir.path()).unwrap();
        ensure_anonymous_account_with(api.clone(), dir.path()).unwrap();

        let identity: AnonymousIdentity = serde_json::from_slice(
            &std::fs::read(dir.path().join(ANONYMOUS_ACCOUNT_FILE)).unwrap(),
        )
        .unwrap();
        assert!(identity.is_valid());
        assert_eq!(
            *api.challenges.lock().unwrap(),
            [("anonymous".to_string(), identity.subject.clone())]
        );
        assert_eq!(
            *api.logins.lock().unwrap(),
            [("fixture-challenge".to_string(), identity.secret.clone())]
        );
        let saved = AnonymousSessionStorage::new(dir.path())
            .load()
            .unwrap()
            .unwrap();
        assert_eq!(saved.tokens.access_token, "a".repeat(64));
        #[cfg(unix)]
        for name in [ANONYMOUS_ACCOUNT_FILE, ANONYMOUS_SESSION_FILE] {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join(name))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0, "{name} is readable by others");
        }
    }

    #[test]
    fn a_failed_registration_keeps_the_identity_for_the_retry() {
        let dir = tempfile::tempdir().unwrap();
        let api = RegisteringApi::default();
        api.failures.store(1, Ordering::SeqCst);
        assert_eq!(
            ensure_anonymous_account_with(api.clone(), dir.path()).err(),
            Some(AccountError::Unavailable)
        );
        let first = std::fs::read(dir.path().join(ANONYMOUS_ACCOUNT_FILE)).unwrap();
        ensure_anonymous_account_with(api.clone(), dir.path()).unwrap();
        assert_eq!(
            std::fs::read(dir.path().join(ANONYMOUS_ACCOUNT_FILE)).unwrap(),
            first
        );
        assert_eq!(api.challenges.lock().unwrap().len(), 1);
    }

    #[test]
    fn an_identity_published_first_by_another_process_wins() {
        let dir = tempfile::tempdir().unwrap();
        let theirs = AnonymousIdentity::generate();
        let bytes = serde_json::to_vec(&theirs).unwrap();
        assert!(write_private(dir.path(), ANONYMOUS_ACCOUNT_FILE, &bytes, true).unwrap());
        assert!(!write_private(dir.path(), ANONYMOUS_ACCOUNT_FILE, b"{}", true).unwrap());
        assert_eq!(
            anonymous_identity(dir.path()).unwrap().subject,
            theirs.subject
        );
    }

    #[test]
    fn an_unreadable_session_is_replaced_by_signing_in_again() {
        let dir = tempfile::tempdir().unwrap();
        let api = RegisteringApi::default();
        ensure_anonymous_account_with(api.clone(), dir.path()).unwrap();
        std::fs::write(dir.path().join(ANONYMOUS_SESSION_FILE), b"not json").unwrap();
        ensure_anonymous_account_with(api.clone(), dir.path()).unwrap();
        assert_eq!(api.logins.lock().unwrap().len(), 2);
        assert!(AnonymousSessionStorage::new(dir.path())
            .load()
            .unwrap()
            .is_some());
    }

    #[test]
    fn a_parseable_but_invalid_session_is_replaced_by_signing_in_again() {
        let dir = tempfile::tempdir().unwrap();
        let api = RegisteringApi::default();
        ensure_anonymous_account_with(api.clone(), dir.path()).unwrap();
        let storage = AnonymousSessionStorage::new(dir.path());
        let mut saved = storage.load().unwrap().unwrap();
        saved.expires_at_unix_ms = u64::MAX;
        storage.save(&saved).unwrap();
        assert_eq!(
            BackendAccountSession::new(api.clone(), AnonymousSessionStorage::new(dir.path()))
                .status(),
            Err(AccountError::Storage)
        );

        ensure_anonymous_account_with(api.clone(), dir.path()).unwrap();
        assert_eq!(api.logins.lock().unwrap().len(), 2);
        assert!(
            BackendAccountSession::new(api, AnonymousSessionStorage::new(dir.path()))
                .status()
                .unwrap()
                .is_some()
        );
    }

    fn signed_in_session(access: char) -> SavedAccountSession {
        SavedAccountSession {
            tokens: AccountTokens {
                access_token: access.to_string().repeat(64),
                refresh_token: "d".repeat(64),
                token_type: "Bearer".into(),
                expires_in: 3600,
                user: AccountUser {
                    id: "synthetic-user".into(),
                    display_name: "合成用户".into(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    email: None,
                    avatar_url: None,
                },
            },
            // 一小时以后才过期，不会触发提前刷新。
            expires_at_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                + 3_600_000,
            session_id: None,
        }
    }

    /// 每次刷新都轮换令牌；同一个刷新令牌第二次出现时像服务端那样拒绝。刷新故意放慢，让并发的调用重叠。
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
            std::thread::sleep(std::time::Duration::from_millis(100));
            let mut presented = self.presented.lock().unwrap();
            if presented.iter().any(|token| token == refresh_token) {
                return Err(AccountError::Unauthorized);
            }
            presented.push(refresh_token.to_owned());
            let mut tokens = anonymous_session('a', 'b').tokens;
            tokens.access_token = "c".repeat(64);
            tokens.refresh_token = "d".repeat(64);
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

    fn anonymous_session(access: char, refresh: char) -> SavedAccountSession {
        SavedAccountSession {
            tokens: AccountTokens {
                access_token: access.to_string().repeat(64),
                refresh_token: refresh.to_string().repeat(64),
                token_type: "Bearer".into(),
                expires_in: 3600,
                user: AccountUser {
                    id: "fixture-anonymous".into(),
                    display_name: String::new(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    email: None,
                    avatar_url: None,
                },
            },
            expires_at_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                + 3_600_000,
            session_id: Some(uuid::Uuid::new_v4()),
        }
    }

    #[test]
    fn concurrent_input_method_calls_refresh_the_anonymous_session_only_once() {
        let dir = tempfile::tempdir().unwrap();
        AnonymousSessionStorage::new(dir.path())
            .save(&anonymous_session('a', 'b'))
            .unwrap();
        let api = RotatingApi::default();
        let rejected = "a".repeat(64);
        let start = Arc::new(std::sync::Barrier::new(2));
        let calls: Vec<_> = (0..2)
            .map(|_| {
                let (api, directory, rejected, start) = (
                    api.clone(),
                    dir.path().to_path_buf(),
                    rejected.clone(),
                    Arc::clone(&start),
                );
                std::thread::spawn(move || {
                    start.wait();
                    shared_access_token_with(api, &directory, Some(&rejected))
                })
            })
            .collect();
        for call in calls {
            let token = call.join().unwrap().unwrap();
            assert_eq!(token.access_token, "c".repeat(64));
            assert!(token.anonymous);
        }
        // 后拿到锁的那一次读到已经轮换过的会话，直接用新令牌，不再拿用过的刷新令牌去刷新。
        assert_eq!(*api.presented.lock().unwrap(), ["b".repeat(64)]);
        assert_eq!(
            AnonymousSessionStorage::new(dir.path())
                .load()
                .unwrap()
                .unwrap()
                .tokens
                .refresh_token,
            "d".repeat(64)
        );
    }

    #[test]
    fn the_input_method_token_prefers_the_signed_in_account_over_the_anonymous_one() {
        let dir = tempfile::tempdir().unwrap();
        let api = RegisteringApi::default();
        assert_eq!(
            shared_access_token_with(api.clone(), dir.path(), None).err(),
            Some(AccountError::Unauthorized)
        );

        ensure_anonymous_account_with(api.clone(), dir.path()).unwrap();
        assert_eq!(
            shared_access_token_with(api.clone(), dir.path(), None).unwrap(),
            SharedAccessToken {
                access_token: "a".repeat(64),
                user_id: "fixture-anonymous".into(),
                anonymous: true,
            }
        );

        // 设置应用登录后写下的会话文件，输入法下一次调用就读到。
        FileAccountSessionStorage::new(dir.path(), AccountSessionFileLayout::Native)
            .save(&signed_in_session('c'))
            .unwrap();
        assert_eq!(
            shared_access_token_with(api.clone(), dir.path(), None).unwrap(),
            SharedAccessToken {
                access_token: "c".repeat(64),
                user_id: "synthetic-user".into(),
                anonymous: false,
            }
        );

        // 退出登录删掉会话文件后，又回到匿名账号。
        FileAccountSessionStorage::new(dir.path(), AccountSessionFileLayout::Native)
            .clear()
            .unwrap();
        assert!(
            shared_access_token_with(api, dir.path(), None)
                .unwrap()
                .anonymous
        );
    }

    #[test]
    fn a_corrupt_identity_is_not_replaced() {
        // Replacing it would register a second account and orphan whatever the first one owns.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(ANONYMOUS_ACCOUNT_FILE),
            br#"{"subject":"msime-short","secret":"x"}"#,
        )
        .unwrap();
        assert_eq!(
            ensure_anonymous_account_with(RegisteringApi::default(), dir.path()).err(),
            Some(AccountError::Storage)
        );
    }

    #[test]
    fn generated_text_uses_only_the_shared_alphabet() {
        for _ in 0..64 {
            let identity = AnonymousIdentity::generate();
            assert!(identity.is_valid());
            assert!(valid_anonymous_subject(&identity.subject));
        }
        assert!(!valid_anonymous_subject("msime-ABCDEFGHIJKLMNOP"));
        assert!(!valid_anonymous_subject("someone@example.com"));
    }

    #[test]
    fn anonymous_json_file_read_reserves_file_size() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("identity.json");
        let contents = br#"{"subject":"msime-synthetic","secret":"synthetic"}"#;
        std::fs::write(&path, contents).unwrap();

        let bytes = read_private_file(std::fs::File::open(path).unwrap()).unwrap();
        assert_eq!(bytes, contents);
        assert_eq!(bytes.capacity(), contents.len());
    }

    #[cfg(unix)]
    #[test]
    fn private_json_validation_uses_the_opened_file_descriptor() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(ANONYMOUS_ACCOUNT_FILE);
        std::fs::write(&path, b"{}").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let parent = crate::storage::open_private_directory(directory.path()).unwrap();
        let file = crate::storage::open_private_file_at(
            &parent,
            std::ffi::OsStr::new(ANONYMOUS_ACCOUNT_FILE),
        )
        .unwrap();
        assert!(validate_opened_private_json_file(&file, &parent).is_ok());

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(validate_opened_private_json_file(&file, &parent).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_anonymous_directory() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let linked = root.path().join("anonymous");
        symlink(outside.path(), &linked).unwrap();

        assert_eq!(
            ensure_anonymous_account_with(RegisteringApi::default(), &linked).err(),
            Some(AccountError::Storage)
        );
        assert!(!outside.path().join(ANONYMOUS_ACCOUNT_FILE).exists());
        assert!(!outside.path().join(ANONYMOUS_SESSION_FILE).exists());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_anonymous_session_file() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("session.json");
        std::fs::write(&target, br#"{}"#).unwrap();
        let linked = root.path().join(ANONYMOUS_SESSION_FILE);
        symlink(&target, &linked).unwrap();

        assert_eq!(
            AnonymousSessionStorage::new(root.path()).load().err(),
            Some(AccountError::Storage)
        );
        assert_eq!(std::fs::read(&target).unwrap(), br#"{}"#);
    }
}
