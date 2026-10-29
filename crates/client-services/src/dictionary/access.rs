//! Cooperative cross-process access to prepared dictionaries and their user journal.
//! Lock files are stable coordination objects and must not be removed.
//!
//! 锁本身（锁文件名 `.msime-dictionary-access.lock`、取锁顺序、共享与独占的规则）在引擎里，见 `msime_engine::DictionaryAccess`；引擎层的维护接口自己取同一把锁。这里只包一层，给宿主保留返回 [`PrivateDirectory`] 的 [`DictionaryAccess::directory`]。

use std::io;
use std::path::Path;

use crate::file_lock::PrivateDirectory;

/// A guard held for the lifetime of every Engine/session using the paths.
pub struct DictionaryAccess {
    lease: msime_engine::DictionaryAccess,
}

impl DictionaryAccess {
    /// Acquire shared access without waiting. `None` means maintenance is active.
    pub fn try_session(user: &Path, dictionaries: &Path) -> io::Result<Option<Self>> {
        Ok(
            msime_engine::DictionaryAccess::try_session(user, dictionaries)?
                .map(|lease| Self { lease }),
        )
    }

    /// Acquire exclusive maintenance access without waiting. `None` means sessions/writers are active.
    pub fn try_maintenance(user: &Path, dictionaries: &Path) -> io::Result<Option<Self>> {
        Ok(
            msime_engine::DictionaryAccess::try_maintenance(user, dictionaries)?
                .map(|lease| Self { lease }),
        )
    }

    /// Return a handle to one of the roots captured when this lease was acquired.
    /// The handle remains bound to the original directory if its path is later replaced.
    pub fn directory(&self, path: &Path) -> io::Result<PrivateDirectory> {
        #[cfg(unix)]
        {
            PrivateDirectory::from_opened(self.lease.directory(path)?)
        }
        // 非 Unix 的目录句柄本来就只是确认过的路径。
        #[cfg(not(unix))]
        {
            crate::file_lock::open_private_directory(self.lease.directory(path)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 锁的规则由引擎的测试覆盖（`msime_engine::access`）；这里只确认包装层取到的就是那把锁。
    #[test]
    fn shared_access_excludes_maintenance_through_the_engine_lease() {
        let user = tempfile::tempdir().unwrap();
        let dictionaries = tempfile::tempdir().unwrap();
        let session = DictionaryAccess::try_session(user.path(), dictionaries.path())
            .unwrap()
            .unwrap();
        assert!(
            msime_engine::DictionaryAccess::try_maintenance(user.path(), dictionaries.path())
                .unwrap()
                .is_none()
        );
        assert!(
            DictionaryAccess::try_maintenance(user.path(), dictionaries.path())
                .unwrap()
                .is_none()
        );
        drop(session);
        let writer = DictionaryAccess::try_maintenance(user.path(), dictionaries.path())
            .unwrap()
            .unwrap();
        assert!(
            msime_engine::DictionaryAccess::try_session(user.path(), dictionaries.path())
                .unwrap()
                .is_none()
        );
        drop(writer);
        assert!(
            DictionaryAccess::try_session(user.path(), dictionaries.path())
                .unwrap()
                .is_some()
        );
        assert!(user.path().join(msime_engine::access::LOCK_FILE).is_file());
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
