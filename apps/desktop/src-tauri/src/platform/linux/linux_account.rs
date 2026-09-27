//! Linux desktop account session storage.
//!
//! The commands themselves are the same nine the Windows and macOS shells register, shared in [`crate::platform::desktop::desktop_account`]; only the session store differs, because Linux has no single secret service every target desktop is guaranteed to run. This host keeps the session in an owner-only file inside the shared state directory - the same rule the Linux provider services already state for their credential files, and the same 0700 directory `msime-linux-prepare` publishes. That is weaker than the Windows Credential Manager or the macOS Keychain, which encrypt at rest; it is stronger than not offering the account surface at all, which is where this host was.
//!
//! The React surface receives the same redacted DTOs as every other host: the tokens never leave this process.

use crate::platform::desktop::desktop_account;
use msime_client_core::account::{AccountError, AccountSessionStorage, SavedAccountSession};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// A session document is a pair of JWTs and an expiry. Anything appreciably
/// larger is not one, and is rejected before it is parsed.
const MAX_ACCOUNT_SESSION_BYTES: u64 = 16 * 1024;

#[derive(Clone)]
pub(crate) struct LinuxAccountStorage {
    path: PathBuf,
}

impl LinuxAccountStorage {
    pub(crate) fn new(directory: &Path) -> Self {
        Self {
            path: directory.join("account-session.json"),
        }
    }
}

impl AccountSessionStorage for LinuxAccountStorage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        // symlink_metadata, not metadata: a symlink here is not a store this
        // host wrote, and following it would read through a path chosen by
        // whoever planted it. A file any other user can read is likewise not
        // one this host would have produced, and a session read out of it is a
        // session nobody vouched for.
        let metadata = match std::fs::symlink_metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(AccountError::Storage),
        };
        if !metadata.is_file()
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o077 != 0
            || metadata.len() > MAX_ACCOUNT_SESSION_BYTES
        {
            return Err(AccountError::Storage);
        }
        // The file can grow after symlink_metadata returns. Read through a bounded handle so a
        // concurrent replacement cannot turn the size check into an unbounded allocation.
        let mut bytes = Vec::new();
        std::fs::File::open(&self.path)
            .map_err(|_| AccountError::Storage)?
            .take(MAX_ACCOUNT_SESSION_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| AccountError::Storage)?;
        if bytes.len() as u64 > MAX_ACCOUNT_SESSION_BYTES {
            return Err(AccountError::Storage);
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| AccountError::Storage)?;
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|_| AccountError::Storage)
    }

    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        let value = serde_json::to_vec(session).map_err(|_| AccountError::Storage)?;
        if value.len() as u64 > MAX_ACCOUNT_SESSION_BYTES {
            return Err(AccountError::Storage);
        }
        let directory = self.path.parent().ok_or(AccountError::Storage)?;
        std::fs::create_dir_all(directory).map_err(|_| AccountError::Storage)?;
        // Publish by rename so a reader never sees a half-written document, and
        // create the temporary file 0600 from the start rather than widening it
        // afterwards - between create and chmod the tokens would be readable.
        // mode() only applies to a new file, so a leftover temporary (or a
        // symlink planted there) is removed and creation is exclusive.
        let temporary = self.path.with_extension("json.new");
        let _ = std::fs::remove_file(&temporary);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| AccountError::Storage)?;
        let written = file
            .write_all(&value)
            .and_then(|()| file.sync_all())
            .map_err(|_| AccountError::Storage);
        drop(file);
        if let Err(error) = written {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
        if std::fs::rename(&temporary, &self.path).is_err() {
            let _ = std::fs::remove_file(&temporary);
            return Err(AccountError::Storage);
        }
        Ok(())
    }

    fn clear(&self) -> Result<(), AccountError> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(AccountError::Storage),
        }
    }
}

/// Registers the shared desktop account state around the session file in `directory`, the resolved shared state directory.
pub fn setup(app: &tauri::AppHandle, directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    desktop_account::manage(app, LinuxAccountStorage::new(directory))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_session_is_rejected_after_a_bounded_read() {
        let directory = tempfile::tempdir().unwrap();
        let storage = LinuxAccountStorage::new(directory.path());
        std::fs::write(
            directory.path().join("account-session.json"),
            vec![b' '; MAX_ACCOUNT_SESSION_BYTES as usize + 1],
        )
        .unwrap();
        assert!(matches!(storage.load(), Err(AccountError::Storage)));
    }
}
