//! Cooperative cross-process access to prepared dictionaries and their user journal.
//! Lock files are stable coordination objects and must not be removed.

use std::fs::File;
use std::io;
use std::path::Path;

/// A guard held for the lifetime of every Engine/session using the paths.
pub struct DictionaryAccess {
    _files: Vec<File>,
}

impl DictionaryAccess {
    /// Acquire shared access without waiting. `None` means maintenance is active.
    pub fn try_session(user: &Path, dictionaries: &Path) -> io::Result<Option<Self>> {
        Self::acquire(user, dictionaries, false)
    }

    /// Acquire exclusive maintenance access without waiting. `None` means sessions/writers are active.
    pub fn try_maintenance(user: &Path, dictionaries: &Path) -> io::Result<Option<Self>> {
        Self::acquire(user, dictionaries, true)
    }

    fn acquire(user: &Path, dictionaries: &Path, exclusive: bool) -> io::Result<Option<Self>> {
        if !user.is_absolute() || !dictionaries.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "absolute dictionary paths required",
            ));
        }
        let mut roots = vec![user.canonicalize()?, dictionaries.canonicalize()?];
        roots.sort();
        roots.dedup();
        let mut files = Vec::with_capacity(roots.len());
        for root in roots {
            let file = crate::file_lock::open_private_lock_file(
                root.join(".msime-dictionary-access.lock"),
            )?;
            let acquired = if exclusive {
                crate::file_lock::try_exclusive(&file)?
            } else {
                crate::file_lock::try_shared(&file)?
            };
            if !acquired {
                return Ok(None);
            }
            files.push(file);
        }
        Ok(Some(Self { _files: files }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_access_excludes_maintenance_and_releases_cleanly() {
        let user = tempfile::tempdir().unwrap();
        let dictionaries = tempfile::tempdir().unwrap();
        let first = DictionaryAccess::try_session(user.path(), dictionaries.path())
            .unwrap()
            .unwrap();
        let second = DictionaryAccess::try_session(user.path(), dictionaries.path())
            .unwrap()
            .unwrap();
        assert!(
            DictionaryAccess::try_maintenance(user.path(), dictionaries.path())
                .unwrap()
                .is_none()
        );
        drop(first);
        assert!(
            DictionaryAccess::try_maintenance(user.path(), dictionaries.path())
                .unwrap()
                .is_none()
        );
        drop(second);
        // Other tests spawn processes concurrently. On Unix, fork can briefly
        // inherit our locked file descriptions before close-on-exec runs.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let writer = loop {
            if let Some(writer) =
                DictionaryAccess::try_maintenance(user.path(), dictionaries.path()).unwrap()
            {
                break writer;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "released dictionary lock remained busy"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        };
        assert!(
            DictionaryAccess::try_session(user.path(), dictionaries.path())
                .unwrap()
                .is_none()
        );
        drop(writer);
        assert!(
            DictionaryAccess::try_session(user.path(), dictionaries.path())
                .unwrap()
                .is_some()
        );
    }
}
