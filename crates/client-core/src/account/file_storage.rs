//! The desktop hosts' account session: one owner-only JSON file, shared by every process of the user that signs in.
//!
//! On macOS the settings app and the input method both hold the account, and the backend rotates the refresh token on every refresh and revokes the whole session when a spent one comes back. The two therefore read the same file on every access and take the same lock file around a refresh, the way the iOS app and its keyboard extension share theirs through the App Group. The file layout on macOS is the Swift backend's (`BackendSavedSession`, an `expiresAt` in seconds since 2001), so `shared/backend` reads it unchanged; the other hosts keep [`SavedAccountSession`]. Either layout is accepted on load.

use super::*;
use std::fs::File;
use std::path::PathBuf;

/// A session document is a pair of JWTs, a user and an expiry. Anything appreciably larger is not one, and is rejected before it is parsed.
const MAX_SESSION_BYTES: u64 = 64 * 1024;
/// Seconds between the Unix epoch and the Foundation reference date, 2001-01-01.
const APPLE_REFERENCE_DATE_UNIX_SECONDS: f64 = 978_307_200.0;
/// How long a refresh waits for another process's. The Swift side gives up after the same time, and its refresh request times out well before it.
const LOCK_TIMEOUT: Duration = Duration::from_secs(20);
const LOCK_POLL: Duration = Duration::from_millis(50);

pub const ACCOUNT_SESSION_FILE: &str = "account-session.json";
pub const ACCOUNT_REFRESH_LOCK_FILE: &str = "account-refresh.lock";

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

    fn path(&self) -> PathBuf {
        self.directory.join(ACCOUNT_SESSION_FILE)
    }

    fn create_directory(&self) -> Result<(), AccountError> {
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
        match self.layout {
            AccountSessionFileLayout::Native => serde_json::to_vec(session),
            AccountSessionFileLayout::Apple => serde_json::to_vec(&AppleSavedSession {
                tokens: session.tokens.clone(),
                expires_at: session.expires_at_unix_ms as f64 / 1000.0
                    - APPLE_REFERENCE_DATE_UNIX_SECONDS,
            }),
        }
        .map_err(|_| AccountError::Storage)
    }

    fn lock(&self) -> Result<File, AccountError> {
        self.create_directory()?;
        let file = crate::file_lock::open_private_lock_file(
            self.directory.join(ACCOUNT_REFRESH_LOCK_FILE),
        )
        .map_err(|_| AccountError::Storage)?;
        let deadline = std::time::Instant::now() + LOCK_TIMEOUT;
        loop {
            if crate::file_lock::try_exclusive(&file).map_err(|_| AccountError::Storage)? {
                return Ok(file);
            }
            if std::time::Instant::now() >= deadline {
                return Err(AccountError::Unavailable);
            }
            std::thread::sleep(LOCK_POLL);
        }
    }
}

fn decode(bytes: &[u8]) -> Result<SavedAccountSession, AccountError> {
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
    })
}

fn read_session_file(file: File) -> Result<Vec<u8>, AccountError> {
    crate::bounded_io::read_bounded_file_with(
        file,
        MAX_SESSION_BYTES,
        || AccountError::Storage,
        |_| AccountError::Storage,
    )
}

impl AccountSessionStorage for FileAccountSessionStorage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        let path = self.path();
        crate::storage::reject_symlink(&self.directory).map_err(|_| AccountError::Storage)?;
        // symlink_metadata, not metadata: a symlink here is not a store this host wrote, and following it would read through a path chosen by whoever planted it. A file another user can read or write is likewise not one any host would have produced.
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(AccountError::Storage),
        };
        if !metadata.is_file() || metadata.len() > MAX_SESSION_BYTES {
            return Err(AccountError::Storage);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let directory =
                std::fs::symlink_metadata(&self.directory).map_err(|_| AccountError::Storage)?;
            if metadata.mode() & 0o077 != 0 || metadata.uid() != directory.uid() {
                return Err(AccountError::Storage);
            }
        }
        // The file can be replaced after symlink_metadata returns. Read through a bounded handle so the size check cannot turn into an unbounded allocation.
        let bytes = read_session_file(File::open(&path).map_err(|_| AccountError::Storage)?)?;
        decode(&bytes).map(Some)
    }

    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        let bytes = self.encode(session)?;
        if bytes.len() as u64 > MAX_SESSION_BYTES {
            return Err(AccountError::Storage);
        }
        self.create_directory()?;
        // Published by rename so no reader ever sees half a document. NamedTempFile creates the file 0600 on Unix, so the tokens are never readable by anyone else, not even between create and rename.
        let mut temporary =
            tempfile::NamedTempFile::new_in(&self.directory).map_err(|_| AccountError::Storage)?;
        temporary
            .write_all(&bytes)
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|_| AccountError::Storage)?;
        temporary
            .persist(self.path())
            .map(|_| ())
            .map_err(|_| AccountError::Storage)
    }

    fn clear(&self) -> Result<(), AccountError> {
        crate::storage::reject_symlink(&self.directory).map_err(|_| AccountError::Storage)?;
        match std::fs::remove_file(self.path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(AccountError::Storage),
        }
    }

    fn shared_across_processes(&self) -> bool {
        true
    }

    fn with_refresh_lock<T>(
        &self,
        body: impl FnOnce() -> Result<T, AccountError>,
    ) -> Result<T, AccountError> {
        // Released when the handle closes at the end of this call.
        let _lock = self.lock()?;
        body()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_file_read_reserves_file_size() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.json");
        let contents = br#"{"tokens":{},"expiresAt":0}"#;
        std::fs::write(&path, contents).unwrap();

        let bytes = read_session_file(File::open(path).unwrap()).unwrap();
        assert_eq!(bytes, contents);
        assert_eq!(bytes.capacity(), contents.len());
    }
}
