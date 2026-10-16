//! The desktop hosts' account session: one owner-only JSON file, shared by every process of the user that signs in.
//!
//! On macOS the settings app and the input method both hold the account, and the backend rotates the refresh token on every refresh and revokes the whole session when a spent one comes back. The two therefore read the same file on every access and take the same lock file around a refresh, the way the iOS app and its keyboard extension share theirs through the App Group. The file layout on macOS is the Swift backend's (`BackendSavedSession`, an `expiresAt` in seconds since 2001), so `shared/backend` reads it unchanged; the other hosts keep [`SavedAccountSession`]. Either layout is accepted on load.

use super::*;
use std::fs::File;
use std::path::PathBuf;

/// A session document is a pair of JWTs, a user and an expiry. Anything appreciably larger is not one, and is rejected before it is parsed.
pub(crate) const MAX_SESSION_BYTES: u64 = 64 * 1024;
/// Seconds between the Unix epoch and the Foundation reference date, 2001-01-01.
const APPLE_REFERENCE_DATE_UNIX_SECONDS: f64 = 978_307_200.0;
/// How long a refresh waits for another process's. The Swift side gives up after the same time, and its refresh request times out well before it.
pub(crate) const LOCK_TIMEOUT: Duration = Duration::from_secs(20);
const LOCK_POLL: Duration = Duration::from_millis(50);

pub const ACCOUNT_SESSION_FILE: &str = "account-session.json";
pub const ACCOUNT_REFRESH_LOCK_FILE: &str = "account-refresh.lock";
/// 云剪贴板的密钥串，与会话文件同目录、同样仅本人可读写。macOS 上输入法和设置应用（Swift）读写同一个文件，内容是密钥串本身（UTF-8，不带换行）。
pub const CLOUD_CLIPBOARD_KEYSTORE_FILE: &str = "cloud-clipboard-keystore";
/// 密钥串约 130 个字符；明显更大的文件不是这里写的。
pub(crate) const MAX_KEYSTORE_BYTES: u64 = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountSessionFileLayout {
    /// [`SavedAccountSession`] as serde writes it.
    Native,
    /// The Swift backend's `BackendSavedSession`: `tokens` and `expiresAt`, seconds since 2001-01-01.
    Apple,
}

#[derive(Serialize, Deserialize)]
struct AppleSavedSession {
    tokens: AccountTokens,
    #[serde(rename = "expiresAt")]
    expires_at: f64,
    #[serde(rename = "sessionID", default, skip_serializing_if = "Option::is_none")]
    session_id: Option<uuid::Uuid>,
}

#[derive(Clone)]
pub struct FileAccountSessionStorage {
    directory: PathBuf,
    layout: AccountSessionFileLayout,
}

impl FileAccountSessionStorage {
    /// `account-session.json` and its lock file in `directory`, which is created owner-only on first save.
    pub fn new(directory: impl Into<PathBuf>, layout: AccountSessionFileLayout) -> Self {
        Self {
            directory: directory.into(),
            layout,
        }
    }

    /// 会话文件所在的目录。桌面端把自建中继的密钥串（`crate::clipboard_relay`）放在同一个目录里。
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub(crate) fn create_directory(&self) -> Result<(), AccountError> {
        crate::storage::reject_symlink(&self.directory).map_err(|_| AccountError::Storage)?;
        std::fs::create_dir_all(&self.directory).map_err(|_| AccountError::Storage)?;
        let metadata =
            std::fs::symlink_metadata(&self.directory).map_err(|_| AccountError::Storage)?;
        if !metadata.is_dir() {
            return Err(AccountError::Storage);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                std::fs::set_permissions(&self.directory, std::fs::Permissions::from_mode(0o700))
                    .map_err(|_| AccountError::Storage)?;
            }
        }
        Ok(())
    }

    fn encode(&self, session: &SavedAccountSession) -> Result<Vec<u8>, AccountError> {
        encode_session(self.layout, session)
    }

    fn lock(&self) -> Result<crate::file_lock::ScopedFileLock, AccountError> {
        self.create_directory()?;
        let file = crate::file_lock::open_private_lock_file(
            self.directory.join(ACCOUNT_REFRESH_LOCK_FILE),
        )
        .map_err(|_| AccountError::Storage)?;
        let deadline = std::time::Instant::now() + LOCK_TIMEOUT;
        loop {
            if crate::file_lock::try_exclusive(&file).map_err(|_| AccountError::Storage)? {
                return Ok(crate::file_lock::ScopedFileLock::from_locked(file));
            }
            if std::time::Instant::now() >= deadline {
                return Err(AccountError::Unavailable);
            }
            std::thread::sleep(LOCK_POLL);
        }
    }
}

/// 会话换了目录以后接管旧目录里的会话：`directory` 还没有 `account-session.json` 而 `legacy` 里有时，把旧文件搬过去，用户不用重新登录。Windows 的共享应用以前把会话放在自己的 `%LOCALAPPDATA%\<tauri identifier>`，它和原生设置窗口打开账号时都先调这里。两个目录在同一个卷上，搬动是一次改名。搬不动时什么也不做，用户看到的是未登录，重新登录即可。
pub fn adopt_legacy_account_session(legacy: &Path, directory: &Path) {
    let source = legacy.join(ACCOUNT_SESSION_FILE);
    let destination = directory.join(ACCOUNT_SESSION_FILE);
    let regular_file = |path: &Path| {
        std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
    };
    if legacy == directory
        || std::fs::symlink_metadata(&destination).is_ok()
        || !regular_file(&source)
        || FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Native)
            .create_directory()
            .is_err()
    {
        return;
    }
    let _ = move_session_if_absent(&source, &destination);
}

fn move_session_if_absent(source: &Path, destination: &Path) -> std::io::Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    {
        rustix::fs::renameat_with(
            rustix::fs::CWD,
            source,
            rustix::fs::CWD,
            destination,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(Into::into)
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
    {
        std::fs::hard_link(source, destination)?;
        std::fs::remove_file(source)
    }
}

/// 按 `layout` 编码一份会话文档。文件存储和宿主注入的存储（`host_storage`）写出的字节相同。
pub(crate) fn encode_session(
    layout: AccountSessionFileLayout,
    session: &SavedAccountSession,
) -> Result<Vec<u8>, AccountError> {
    match layout {
        AccountSessionFileLayout::Native => serde_json::to_vec(session),
        AccountSessionFileLayout::Apple => serde_json::to_vec(&AppleSavedSession {
            tokens: session.tokens.clone(),
            expires_at: session.expires_at_unix_ms as f64 / 1000.0
                - APPLE_REFERENCE_DATE_UNIX_SECONDS,
            session_id: session.session_id,
        }),
    }
    .map_err(|_| AccountError::Storage)
}

/// 两种布局都接受；合法性由读取方（`BackendAccountSession` 的 `validate_saved_session`）再查一遍。
pub(crate) fn decode(bytes: &[u8]) -> Result<SavedAccountSession, AccountError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| AccountError::Storage)?;
    if value.get("expiresAt").is_none() {
        return serde_json::from_value(value).map_err(|_| AccountError::Storage);
    }
    let saved: AppleSavedSession =
        serde_json::from_value(value).map_err(|_| AccountError::Storage)?;
    let expires_at_unix_ms = (saved.expires_at + APPLE_REFERENCE_DATE_UNIX_SECONDS) * 1000.0;
    if !expires_at_unix_ms.is_finite() || expires_at_unix_ms < 0.0 {
        return Err(AccountError::Storage);
    }
    Ok(SavedAccountSession {
        tokens: saved.tokens,
        expires_at_unix_ms: expires_at_unix_ms.round() as u64,
        session_id: saved.session_id,
    })
}

fn read_bounded_file(file: File, maximum: u64) -> Result<Vec<u8>, AccountError> {
    crate::bounded_io::read_bounded_file_with(
        file,
        maximum,
        || AccountError::Storage,
        |_| AccountError::Storage,
    )
}

#[cfg(unix)]
fn save_bytes(
    storage: &FileAccountSessionStorage,
    name: &str,
    bytes: &[u8],
) -> Result<(), AccountError> {
    // Published by rename so no reader ever sees half a document. On Unix the parent handle
    // also keeps a concurrent replacement of the account directory from redirecting tokens.
    let directory = crate::storage::open_private_directory(&storage.directory)
        .map_err(|_| AccountError::Storage)?;
    crate::storage::write_private_file_at(&directory, std::ffi::OsStr::new(name), bytes)
        .map_err(|_| AccountError::Storage)
}

#[cfg(not(unix))]
fn save_bytes(
    storage: &FileAccountSessionStorage,
    name: &str,
    bytes: &[u8],
) -> Result<(), AccountError> {
    // Published by rename so no reader ever sees half a document. NamedTempFile creates the
    // file 0600 on Unix, so the tokens are never readable by anyone else during publication.
    let mut temporary =
        tempfile::NamedTempFile::new_in(&storage.directory).map_err(|_| AccountError::Storage)?;
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|_| AccountError::Storage)?;
    temporary
        .persist(storage.directory.join(name))
        .map(|_| ())
        .map_err(|_| AccountError::Storage)
}

impl FileAccountSessionStorage {
    /// 读目录里的一个私有文件；不存在时为 `None`。自建中继的密钥串（`crate::clipboard_relay`）也经它读。
    pub(crate) fn read_private(
        &self,
        name: &str,
        maximum: u64,
    ) -> Result<Option<Vec<u8>>, AccountError> {
        crate::storage::reject_symlink(&self.directory).map_err(|_| AccountError::Storage)?;
        #[cfg(unix)]
        let directory = match crate::storage::open_private_directory(&self.directory) {
            Ok(directory) => directory,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(AccountError::Storage),
        };
        // symlink_metadata, not metadata: a symlink here is not a store this host wrote, and following it would read through a path chosen by whoever planted it. A file another user can read or write is likewise not one any host would have produced.
        #[cfg(unix)]
        let file =
            match crate::storage::open_private_file_at(&directory, std::ffi::OsStr::new(name)) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(_) => return Err(AccountError::Storage),
            };
        #[cfg(not(unix))]
        let path = self.directory.join(name);
        #[cfg(not(unix))]
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(AccountError::Storage),
        };
        #[cfg(not(unix))]
        let file = crate::storage::open_private_file(&path).map_err(|_| AccountError::Storage)?;
        #[cfg(unix)]
        let metadata = file.metadata().map_err(|_| AccountError::Storage)?;
        if !metadata.is_file() || metadata.len() > maximum {
            return Err(AccountError::Storage);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let directory_metadata = directory.metadata().map_err(|_| AccountError::Storage)?;
            if metadata.mode() & 0o077 != 0 || metadata.uid() != directory_metadata.uid() {
                return Err(AccountError::Storage);
            }
        }
        // The file can be replaced after symlink_metadata returns. Read through a bounded handle so the size check cannot turn into an unbounded allocation.
        read_bounded_file(file, maximum).map(Some)
    }

    /// 原子替换目录里的一个私有文件（仅本人可读写），目录不存在时先建好。
    pub(crate) fn write_private(&self, name: &str, bytes: &[u8]) -> Result<(), AccountError> {
        self.create_directory()?;
        save_bytes(self, name, bytes)
    }

    fn remove_private(&self, name: &str) -> Result<(), AccountError> {
        match crate::storage::remove_private_file(&self.directory.join(name)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(AccountError::Storage),
        }
    }
}

impl AccountSessionStorage for FileAccountSessionStorage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        match self.read_private(ACCOUNT_SESSION_FILE, MAX_SESSION_BYTES)? {
            Some(bytes) => decode(&bytes).map(Some),
            None => Ok(None),
        }
    }

    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        let bytes = self.encode(session)?;
        if bytes.len() as u64 > MAX_SESSION_BYTES {
            return Err(AccountError::Storage);
        }
        self.create_directory()?;
        save_bytes(self, ACCOUNT_SESSION_FILE, &bytes)
    }

    /// 删除会话，也删除云剪贴板密钥串：退出登录后本机不再保留能解开云端条目的密钥。
    fn clear(&self) -> Result<(), AccountError> {
        crate::storage::reject_symlink(&self.directory).map_err(|_| AccountError::Storage)?;
        let session = self.remove_private(ACCOUNT_SESSION_FILE);
        let keystore = self.remove_private(CLOUD_CLIPBOARD_KEYSTORE_FILE);
        session.and(keystore)
    }

    fn load_clipboard_keystore(&self) -> Result<Option<String>, AccountError> {
        self.read_private(CLOUD_CLIPBOARD_KEYSTORE_FILE, MAX_KEYSTORE_BYTES)?
            .map(|bytes| {
                let value = String::from_utf8(bytes).map_err(|_| AccountError::Storage)?;
                // 别的写入方可能在末尾带一个换行，它不属于密钥串；带着它解析会失败，驱动随后会换一个新的设备身份。
                Ok(value.trim_end_matches(['\n', '\r']).to_owned())
            })
            .transpose()
    }

    fn save_clipboard_keystore(&self, value: &str) -> Result<(), AccountError> {
        if value.is_empty() || value.len() as u64 > MAX_KEYSTORE_BYTES {
            return Err(AccountError::Storage);
        }
        self.create_directory()?;
        save_bytes(self, CLOUD_CLIPBOARD_KEYSTORE_FILE, value.as_bytes())
    }

    fn shared_across_processes(&self) -> bool {
        true
    }

    fn with_refresh_lock<T>(
        &self,
        body: impl FnOnce() -> Result<T, AccountError>,
    ) -> Result<T, AccountError> {
        // Unix 由守卫在本次调用结束时显式解锁；Windows 继续通过关闭文件释放锁。
        let _lock = self.lock()?;
        body()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_before_first_save_returns_no_session_without_creating_a_directory() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("account-not-created");
        let storage = FileAccountSessionStorage::new(&directory, AccountSessionFileLayout::Native);

        assert!(storage.load().unwrap().is_none());
        assert!(!directory.exists());
    }

    #[test]
    fn clipboard_keystore_is_private_replaced_whole_and_cleared_with_the_session() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("account");
        let storage = FileAccountSessionStorage::new(&directory, AccountSessionFileLayout::Apple);
        assert_eq!(storage.load_clipboard_keystore().unwrap(), None);

        storage
            .save_clipboard_keystore("v1.synthetic-first")
            .unwrap();
        storage
            .save_clipboard_keystore("v1.synthetic-second")
            .unwrap();
        assert_eq!(
            storage.load_clipboard_keystore().unwrap().as_deref(),
            Some("v1.synthetic-second")
        );
        let path = directory.join(CLOUD_CLIPBOARD_KEYSTORE_FILE);
        assert_eq!(std::fs::read(&path).unwrap(), b"v1.synthetic-second");
        // 原子替换：目录里只剩会话目录该有的文件，没有残留的临时文件。
        let names = std::fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, [CLOUD_CLIPBOARD_KEYSTORE_FILE]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
            assert_eq!(
                std::fs::metadata(&directory).unwrap().permissions().mode() & 0o077,
                0
            );
        }

        std::fs::write(&path, b"v1.synthetic-swift\n").unwrap();
        assert_eq!(
            storage.load_clipboard_keystore().unwrap().as_deref(),
            Some("v1.synthetic-swift")
        );

        storage.clear().unwrap();
        assert!(!path.exists());
        assert_eq!(storage.load_clipboard_keystore().unwrap(), None);
        // 再清一次也没问题：会话和密钥串都已不在。
        storage.clear().unwrap();
    }

    #[test]
    fn clipboard_keystore_rejects_files_this_host_did_not_write() {
        let root = tempfile::tempdir().unwrap();
        let storage = FileAccountSessionStorage::new(root.path(), AccountSessionFileLayout::Native);
        assert_eq!(
            storage.save_clipboard_keystore(""),
            Err(AccountError::Storage)
        );
        assert_eq!(
            storage.save_clipboard_keystore(&"x".repeat(4097)),
            Err(AccountError::Storage)
        );
        storage.save_clipboard_keystore("v1.synthetic").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = root.path().join(CLOUD_CLIPBOARD_KEYSTORE_FILE);
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert_eq!(
                storage.load_clipboard_keystore(),
                Err(AccountError::Storage)
            );
            std::fs::remove_file(&path).unwrap();
            std::os::unix::fs::symlink(root.path().join("elsewhere"), &path).unwrap();
            assert_eq!(
                storage.load_clipboard_keystore(),
                Err(AccountError::Storage)
            );
        }
    }

    #[test]
    fn an_apple_layout_document_keeps_the_email_and_avatar_of_its_user() {
        // Swift 的 `BackendSavedSession` 写出的形状：expiresAt 是 2001 年起的秒数，sessionID 是 UUID。
        let document = br#"{"tokens":{"access_token":"synthetic-access","refresh_token":"synthetic-refresh","token_type":"Bearer","expires_in":900,"user":{"id":"synthetic-user","display_name":"","created_at":"2026-01-01T00:00:00Z","email":"person@example.test","avatar_url":"https://media.msime.app/avatars/synthetic.jpg"}},"expiresAt":800000000,"sessionID":"6f9619ff-8b86-d011-b42d-00c04fc964ff"}"#;
        let saved = decode(document).unwrap();
        assert_eq!(
            saved.tokens.user.email.as_deref(),
            Some("person@example.test")
        );
        assert_eq!(
            saved.tokens.user.avatar_url.as_deref(),
            Some("https://media.msime.app/avatars/synthetic.jpg")
        );
        assert_eq!(saved.expires_at_unix_ms, 1_778_307_200_000);
        assert!(saved.session_id.is_some());

        // 再按 Apple 布局写回，两个字段原样保留。
        let encoded: serde_json::Value = serde_json::from_slice(
            &encode_session(AccountSessionFileLayout::Apple, &saved).unwrap(),
        )
        .unwrap();
        assert_eq!(encoded["tokens"]["user"]["email"], "person@example.test");
        assert_eq!(
            encoded["tokens"]["user"]["avatar_url"],
            "https://media.msime.app/avatars/synthetic.jpg"
        );
        assert_eq!(encoded["expiresAt"], 800_000_000.0);
    }

    #[test]
    fn session_file_read_reserves_file_size() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.json");
        let contents = br#"{"tokens":{},"expiresAt":0}"#;
        std::fs::write(&path, contents).unwrap();

        let bytes = read_bounded_file(File::open(path).unwrap(), MAX_SESSION_BYTES).unwrap();
        assert_eq!(bytes, contents);
        assert_eq!(bytes.capacity(), contents.len());
    }

    #[test]
    fn a_session_left_in_the_old_directory_moves_to_the_shared_one() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("app.msime.windows");
        let shared = root.path().join("MSIME").join("account");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join(ACCOUNT_SESSION_FILE), b"synthetic-session").unwrap();

        adopt_legacy_account_session(&legacy, &shared);

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

        adopt_legacy_account_session(&legacy, &shared);

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
    fn a_session_published_after_the_absence_check_wins_the_move() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("legacy-session.json");
        let shared = root.path().join("shared-session.json");
        std::fs::write(&legacy, b"synthetic-old").unwrap();
        assert!(!shared.exists());
        // Another process publishes a newer login after the migration checked for absence.
        std::fs::write(&shared, b"synthetic-new").unwrap();

        assert!(move_session_if_absent(&legacy, &shared).is_err());
        assert_eq!(std::fs::read(&shared).unwrap(), b"synthetic-new");
        assert_eq!(std::fs::read(&legacy).unwrap(), b"synthetic-old");
    }

    #[test]
    fn without_an_old_session_nothing_is_created() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("app.msime.windows");
        let shared = root.path().join("MSIME").join("account");
        std::fs::create_dir_all(&legacy).unwrap();

        adopt_legacy_account_session(&legacy, &shared);

        assert!(!shared.exists());
    }

    #[cfg(unix)]
    #[test]
    fn finished_refresh_scope_releases_an_inherited_lock() {
        let root = tempfile::tempdir().unwrap();
        let storage = FileAccountSessionStorage::new(root.path(), AccountSessionFileLayout::Native);
        let held = storage.lock().unwrap();
        let inherited = crate::file_lock::testing::duplicate(&held);
        crate::file_lock::testing::assert_finished_scope_releases_lock(
            held,
            inherited,
            &root.path().join(ACCOUNT_REFRESH_LOCK_FILE),
        );
    }
}
