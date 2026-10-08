//! Cooperative cross-process access to prepared dictionaries and their user journal.
//! Lock files are stable coordination objects and must not be removed.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

const ACCESS_LOCK_NAME: &str = ".msime-dictionary-access.lock";

struct LockedRoot {
    path: PathBuf,
    directory: crate::file_lock::PrivateDirectory,
    _lock: File,
}

/// A guard held for the lifetime of every Engine/session using the paths.
pub struct DictionaryAccess {
    roots: Vec<LockedRoot>,
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

    /// Return the directory handle used by this guard for one of the locked
    /// roots. The handle remains bound to the directory even if its path is
    /// replaced after the lock was acquired.
    pub fn directory_for(&self, path: &Path) -> io::Result<&crate::file_lock::PrivateDirectory> {
        let canonical = path.canonicalize()?;
        self.roots
            .iter()
            .find(|root| root.path == canonical)
            .map(|root| &root.directory)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "directory is not held by this guard",
                )
            })
    }

    fn acquire(user: &Path, dictionaries: &Path, exclusive: bool) -> io::Result<Option<Self>> {
        if !user.is_absolute() || !dictionaries.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "absolute dictionary paths required",
            ));
        }
        let mut roots = Vec::with_capacity(2);
        for path in [user, dictionaries] {
            crate::storage::reject_symlink(path)?;
            let directory = crate::file_lock::open_private_directory(path)?;
            roots.push((path.canonicalize()?, directory));
        }
        roots.sort_by(|left, right| left.0.cmp(&right.0));
        roots.dedup_by(|left, right| left.0 == right.0);
        let mut locked = Vec::with_capacity(roots.len());
        for (path, directory) in roots {
            let file = crate::file_lock::open_private_lock_file_at(
                &directory,
                std::ffi::OsStr::new(ACCESS_LOCK_NAME),
            )?;
            let acquired = if exclusive {
                crate::file_lock::try_exclusive(&file)?
            } else {
                crate::file_lock::try_shared(&file)?
            };
            if !acquired {
                return Ok(None);
            }
            locked.push(LockedRoot {
                path,
                directory,
                _lock: file,
            });
        }
        Ok(Some(Self { roots: locked }))
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

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_root_before_creating_an_external_lock() {
        use std::os::unix::fs::symlink;

        let parent = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let user = parent.path().join("user");
        symlink(outside.path(), &user).unwrap();
        let dictionaries = tempfile::tempdir().unwrap();

        assert!(DictionaryAccess::try_session(&user, dictionaries.path()).is_err());
        assert!(!outside
            .path()
            .join(".msime-dictionary-access.lock")
            .exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_root_replaced_after_lock_acquisition_is_not_returned_as_the_held_directory() {
        use std::os::unix::fs::symlink;

        let parent = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let user = parent.path().join("user");
        let dictionaries = parent.path().join("dictionaries");
        std::fs::create_dir(&user).unwrap();
        std::fs::create_dir(&dictionaries).unwrap();
        let access = DictionaryAccess::try_maintenance(&user, &dictionaries)
            .unwrap()
            .unwrap();

        let moved = parent.path().join("moved-user");
        std::fs::rename(&user, &moved).unwrap();
        symlink(outside.path(), &user).unwrap();

        assert!(access.directory_for(&user).is_err());
        assert!(!outside
            .path()
            .join(".msime-dictionary-access.lock")
            .exists());
    }
}
