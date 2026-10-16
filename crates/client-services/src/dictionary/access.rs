//! Cooperative cross-process access to prepared dictionaries and their user journal.
//! Lock files are stable coordination objects and must not be removed.

use std::fs::File;
use std::io;
use std::path::Path;

/// A guard held for the lifetime of every Engine/session using the paths.
pub struct DictionaryAccess {
    _files: Vec<File>,
    roots: Vec<(std::path::PathBuf, crate::file_lock::PrivateDirectory)>,
}

#[cfg(unix)]
impl Drop for DictionaryAccess {
    fn drop(&mut self) {
        // 继承或复制的描述符不能把租约延长到守卫结束之后。
        for file in &self._files {
            let _ = crate::file_lock::unlock(file);
        }
    }
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

    /// Return a handle to one of the roots captured when this lease was acquired.
    /// The handle remains bound to the original directory if its path is later replaced.
    pub fn directory(&self, path: &Path) -> io::Result<crate::file_lock::PrivateDirectory> {
        let canonical = path.canonicalize()?;
        self.roots
            .iter()
            .find(|(root, _)| root == &canonical)
            .map(|(_, directory)| directory.try_clone())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "directory is not leased"))?
    }

    fn acquire(user: &Path, dictionaries: &Path, exclusive: bool) -> io::Result<Option<Self>> {
        if !user.is_absolute() || !dictionaries.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "absolute dictionary paths required",
            ));
        }
        crate::storage::reject_symlink(user)?;
        crate::storage::reject_symlink(dictionaries)?;
        let mut roots = vec![user.canonicalize()?, dictionaries.canonicalize()?];
        roots.sort();
        roots.dedup();
        let mut access = Self {
            _files: Vec::with_capacity(roots.len()),
            roots: Vec::with_capacity(roots.len()),
        };
        for root in roots {
            let directory = crate::file_lock::open_private_directory(&root)?;
            let file = crate::file_lock::open_private_lock_file_at(
                &directory,
                std::ffi::OsStr::new(".msime-dictionary-access.lock"),
            )?;
            let acquired = if exclusive {
                crate::file_lock::try_exclusive(&file)?
            } else {
                crate::file_lock::try_shared(&file)?
            };
            if !acquired {
                return Ok(None);
            }
            access._files.push(file);
            access.roots.push((root, directory));
        }
        Ok(Some(access))
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
        let writer = DictionaryAccess::try_maintenance(user.path(), dictionaries.path())
            .unwrap()
            .unwrap();
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
    fn assert_access_releases_with_duplicate_descriptors(exclusive: bool) {
        let user = tempfile::tempdir().unwrap();
        let dictionaries = tempfile::tempdir().unwrap();
        let access = DictionaryAccess::acquire(user.path(), dictionaries.path(), exclusive)
            .unwrap()
            .unwrap();
        let duplicates: Vec<_> = access
            ._files
            .iter()
            .map(|file| file.try_clone().unwrap())
            .collect();
        assert!(
            DictionaryAccess::try_maintenance(user.path(), dictionaries.path())
                .unwrap()
                .is_none()
        );
        drop(access);

        let next = DictionaryAccess::try_maintenance(user.path(), dictionaries.path())
            .unwrap()
            .expect("结束的访问租约不能被描述符副本继续持有");
        drop(duplicates);
        assert!(
            DictionaryAccess::try_session(user.path(), dictionaries.path())
                .unwrap()
                .is_none()
        );
        assert!(
            DictionaryAccess::try_maintenance(user.path(), dictionaries.path())
                .unwrap()
                .is_none()
        );
        assert!(
            DictionaryAccess::try_session(user.path(), dictionaries.path())
                .unwrap()
                .is_none()
        );
        drop(next);
        assert!(
            DictionaryAccess::try_session(user.path(), dictionaries.path())
                .unwrap()
                .is_some()
        );
    }

    #[cfg(unix)]
    #[test]
    fn session_access_releases_with_duplicate_descriptors() {
        assert_access_releases_with_duplicate_descriptors(false);
    }

    #[cfg(unix)]
    #[test]
    fn maintenance_access_releases_with_duplicate_descriptors() {
        assert_access_releases_with_duplicate_descriptors(true);
    }

    #[test]
    fn a_busy_second_root_releases_the_first_without_releasing_the_holder() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("a");
        let second = root.path().join("z");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        let holder = DictionaryAccess::try_maintenance(&second, &second)
            .unwrap()
            .unwrap();

        assert!(DictionaryAccess::try_maintenance(&first, &second)
            .unwrap()
            .is_none());
        let first_only = DictionaryAccess::try_maintenance(&first, &first)
            .unwrap()
            .expect("部分获取失败应归还第一把锁");
        assert!(DictionaryAccess::try_session(&second, &second)
            .unwrap()
            .is_none());
        drop(first_only);
        drop(holder);
        assert!(DictionaryAccess::try_maintenance(&first, &second)
            .unwrap()
            .is_some());
    }

    #[test]
    fn an_invalid_second_lock_releases_the_first_root() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("a");
        let second = root.path().join("z");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        std::fs::create_dir(second.join(".msime-dictionary-access.lock")).unwrap();

        assert!(DictionaryAccess::try_maintenance(&first, &second).is_err());
        assert!(DictionaryAccess::try_maintenance(&first, &first)
            .unwrap()
            .is_some());
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
    fn maintenance_directory_handle_survives_root_replacement() {
        use std::fs;

        let root = tempfile::tempdir().unwrap();
        let user = root.path().join("user");
        let dictionaries = root.path().join("dictionaries");
        fs::create_dir(&user).unwrap();
        fs::create_dir(&dictionaries).unwrap();
        let access = DictionaryAccess::try_maintenance(&user, &dictionaries)
            .unwrap()
            .unwrap();
        let bound = access.directory(&user).unwrap();

        let moved = root.path().join("moved-user");
        fs::rename(&user, &moved).unwrap();
        fs::create_dir(&user).unwrap();
        crate::file_lock::write_private_file_at(&bound, std::ffi::OsStr::new("marker"), b"bound")
            .unwrap();

        assert_eq!(fs::read(moved.join("marker")).unwrap(), b"bound");
        assert!(!user.join("marker").exists());
    }
}
