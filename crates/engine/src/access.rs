//! 跨进程的词库访问租约：已准备的代次目录（`RuntimePaths::dictionaries`）和用户数据目录（`user_data`，日志在这里）各放一个锁文件 [`LOCK_FILE`]。用这些数据的会话持共享租约，改个人词库、重置学习这类维护持独占租约，两者互斥，只试不等。
//!
//! 锁文件是稳定的协调对象，各版本、各进程都按同一个名字和同一套规则来取，不能改名，也不能删掉。规则与搬进引擎之前 `msime-client-services` 的 `dictionary::access` 相同：两个根目录先规范化、排序、去重，按顺序逐个打开并取锁，任何一个取不到就放掉已经取到的，返回 `None`；Unix 上目录和锁文件都经目录句柄打开、不跟随符号链接，结束时显式解锁，复制出去的描述符不能把租约延长到守卫之后。
//!
//! 引擎层的维护接口（[`crate::edit_personal_dictionary`]、[`crate::edit_bundled_dictionary_entry`]、[`crate::reset_learned_data`]）自己取独占租约；`host` 门面的同名接口假定调用方（host-api）已经拿着租约，不再取。

use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::paths::RuntimePaths;

/// 租约锁文件的名字，放在每个根目录里。
pub const LOCK_FILE: &str = ".msime-dictionary-access.lock";

/// 一份词库访问租约，持有期间别的进程（和本进程里的其他租约）按 [`DictionaryAccess::try_session`]、[`DictionaryAccess::try_maintenance`] 的规则互斥。
pub struct DictionaryAccess {
    files: Vec<File>,
    roots: Vec<Root>,
}

/// 租约里的一个根目录：规范化后的路径，Unix 上还有取锁时打开的目录句柄。
struct Root {
    path: PathBuf,
    #[cfg(unix)]
    directory: File,
}

#[cfg(unix)]
impl Drop for DictionaryAccess {
    fn drop(&mut self) {
        // 继承或复制的描述符不能把租约延长到守卫结束之后。
        for file in &self.files {
            let _ = unlock(file);
        }
    }
}

impl DictionaryAccess {
    /// 不等待地取共享租约（会话用）。`None` 表示正在维护。
    pub fn try_session(user: &Path, dictionaries: &Path) -> io::Result<Option<Self>> {
        Self::acquire(user, dictionaries, false)
    }

    /// 不等待地取独占的维护租约。`None` 表示还有会话或别的维护持着租约。
    pub fn try_maintenance(user: &Path, dictionaries: &Path) -> io::Result<Option<Self>> {
        Self::acquire(user, dictionaries, true)
    }

    /// 取租约时打开的那个根目录的句柄（复制一份）。之后这个路径被换成别的目录，句柄仍指向取锁时的那个。`path` 不是租约的根目录时报 `NotFound`。
    #[cfg(unix)]
    pub fn directory(&self, path: &Path) -> io::Result<File> {
        self.root(path)?.directory.try_clone()
    }

    /// 非 Unix 平台没有目录句柄，返回租约里规范化后的根目录路径。`path` 不是租约的根目录时报 `NotFound`。
    #[cfg(not(unix))]
    pub fn directory(&self, path: &Path) -> io::Result<PathBuf> {
        Ok(self.root(path)?.path.clone())
    }

    fn root(&self, path: &Path) -> io::Result<&Root> {
        let canonical = path.canonicalize()?;
        self.roots
            .iter()
            .find(|root| root.path == canonical)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "directory is not leased"))
    }

    fn acquire(user: &Path, dictionaries: &Path, exclusive: bool) -> io::Result<Option<Self>> {
        if !user.is_absolute() || !dictionaries.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "absolute dictionary paths required",
            ));
        }
        msime_path_trust::reject_symlinked_components(user)?;
        msime_path_trust::reject_symlinked_components(dictionaries)?;
        let mut roots = vec![user.canonicalize()?, dictionaries.canonicalize()?];
        roots.sort();
        roots.dedup();
        let mut access = Self {
            files: Vec::with_capacity(roots.len()),
            roots: Vec::with_capacity(roots.len()),
        };
        for path in roots {
            let directory = open_root(&path)?;
            let file = open_lock_file(&directory, OsStr::new(LOCK_FILE))?;
            let acquired = if exclusive {
                try_exclusive(&file)?
            } else {
                try_shared(&file)?
            };
            if !acquired {
                // 已经取到的前几把锁随 `access` 一起放掉。
                return Ok(None);
            }
            access.files.push(file);
            access.roots.push(Root {
                path,
                #[cfg(unix)]
                directory,
            });
        }
        Ok(Some(access))
    }
}

/// 引擎层维护接口取独占租约：取不到是 `Busy`（`DICTIONARY_MAINTENANCE_BUSY`），打不开目录或锁文件是存储错误。
pub(crate) fn maintenance(paths: &RuntimePaths) -> Result<DictionaryAccess> {
    DictionaryAccess::try_maintenance(&paths.user_data, &paths.dictionaries)?
        .ok_or_else(|| EngineError::busy(diagnostics::DICTIONARY_MAINTENANCE_BUSY))
}

/// 不跟随符号链接地打开根目录，规则与 `msime-client-base` 的 `storage::open_private_directory` 相同。Apple 平台不逐级打开：iOS 沙盒不让应用以读方式打开容器的祖先目录，`O_NOFOLLOW_ANY` 一次解析就拒绝路径上任何一级符号链接；路径经过 `msime-path-trust` 列出的系统别名（`/var`、`/tmp`）时先换成它唯一受信任的目标。
#[cfg(target_vendor = "apple")]
fn open_root(path: &Path) -> io::Result<File> {
    let flags = rustix::fs::OFlags::RDONLY
        | rustix::fs::OFlags::DIRECTORY
        | rustix::fs::OFlags::NOFOLLOW_ANY
        | rustix::fs::OFlags::CLOEXEC
        | rustix::fs::OFlags::NONBLOCK;
    match rustix::fs::open(path, flags, rustix::fs::Mode::empty()) {
        Err(rustix::io::Errno::LOOP) => {}
        result => return Ok(result?.into()),
    }
    let mut resolved = PathBuf::new();
    for component in path.components() {
        resolved.push(component);
        if msime_path_trust::is_trusted_system_alias(&resolved) {
            let target = std::fs::read_link(&resolved)?;
            resolved = resolved
                .parent()
                .unwrap_or_else(|| Path::new("/"))
                .join(target);
        }
    }
    if resolved == path {
        return Err(rustix::io::Errno::LOOP.into());
    }
    Ok(rustix::fs::open(&resolved, flags, rustix::fs::Mode::empty())?.into())
}

/// 其余 Unix 逐级经目录句柄打开，每一级都不跟随符号链接（系统别名除外）。Linux 和 Android 用 `O_PATH` 只凭搜索权限走过祖先目录（Android 不给应用 `/data` 的读权限），走完再按读方式打开根目录本身。
#[cfg(all(unix, not(target_vendor = "apple")))]
fn open_root(path: &Path) -> io::Result<File> {
    let flags = rustix::fs::OFlags::RDONLY
        | rustix::fs::OFlags::DIRECTORY
        | rustix::fs::OFlags::NOFOLLOW
        | rustix::fs::OFlags::CLOEXEC
        | rustix::fs::OFlags::NONBLOCK;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let search = rustix::fs::OFlags::PATH
        | rustix::fs::OFlags::DIRECTORY
        | rustix::fs::OFlags::NOFOLLOW
        | rustix::fs::OFlags::CLOEXEC;
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let search = flags;
    // 根目录在 `acquire` 里已经确认是绝对路径。
    let mut directory = rustix::fs::open(Path::new("/"), search, rustix::fs::Mode::empty())?;
    let mut logical = PathBuf::from("/");
    for component in path.components() {
        match component {
            std::path::Component::Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "directory path has an unsupported prefix",
                ));
            }
            std::path::Component::RootDir | std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                directory =
                    rustix::fs::openat(&directory, "..", search, rustix::fs::Mode::empty())?;
                logical.pop();
            }
            std::path::Component::Normal(name) => {
                logical.push(name);
                directory =
                    match rustix::fs::openat(&directory, name, search, rustix::fs::Mode::empty()) {
                        Ok(directory) => directory,
                        Err(error)
                            if (error == rustix::io::Errno::LOOP
                                || error == rustix::io::Errno::NOTDIR)
                                && msime_path_trust::is_trusted_system_alias(&logical) =>
                        {
                            rustix::fs::openat(
                                &directory,
                                name,
                                search & !rustix::fs::OFlags::NOFOLLOW,
                                rustix::fs::Mode::empty(),
                            )?
                        }
                        Err(error) => return Err(error.into()),
                    };
            }
        }
    }
    Ok(rustix::fs::openat(&directory, ".", flags, rustix::fs::Mode::empty())?.into())
}

/// 非 Unix 平台没有 openat 这组调用：只确认根目录是真实目录（不是符号链接），锁文件再按路径打开。
#[cfg(not(unix))]
fn open_root(path: &Path) -> io::Result<PathBuf> {
    if !std::fs::symlink_metadata(path)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private directory is not a real directory",
        ));
    }
    Ok(path.to_path_buf())
}

/// 在根目录里打开（没有就建）锁文件：Unix 经目录句柄、不跟随符号链接、权限 0600；打开后要求是只有一个硬链接的普通文件。
#[cfg(unix)]
fn open_lock_file(directory: &File, name: &OsStr) -> io::Result<File> {
    let open = || {
        rustix::fs::openat(
            directory,
            name,
            rustix::fs::OFlags::RDWR
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::from_raw_mode(0o600),
        )
    };
    #[cfg(target_vendor = "apple")]
    let descriptor = match open() {
        // 并发首次创建锁文件时，Darwin 的 openat 偶尔返回 ENOENT；目录句柄仍有效，重试同一入口即可。
        Err(rustix::io::Errno::NOENT) => {
            std::thread::yield_now();
            open()?
        }
        result => result?,
    };
    #[cfg(not(target_vendor = "apple"))]
    let descriptor = open()?;
    ensure_regular(descriptor.into())
}

/// 非 Unix 平台按路径打开锁文件，先拒绝路径上的符号链接；Windows 上不跟随重解析点。
#[cfg(not(unix))]
fn open_lock_file(directory: &Path, name: &OsStr) -> io::Result<File> {
    let path = directory.join(name);
    msime_path_trust::reject_symlinked_components(&path)?;
    let mut options = File::options();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    ensure_regular(options.open(path)?)
}

fn ensure_regular(file: File) -> io::Result<File> {
    if !file.metadata()?.is_file() || !crate::paths::has_single_link(&file)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "lock file is not a single-link regular file",
        ));
    }
    Ok(file)
}

/// 假是被占用，不是出错；从不等待。Android 上标准库的文件锁不可用（直接失败而不是等待），那里用下面 rustix 的 `flock` 版本；按函数而不是按语句分开，`scripts/test-file-locking-goes-through-client-core.py` 才认得出这是 Android 不编译的标准库调用。
#[cfg(not(target_os = "android"))]
fn try_shared(file: &File) -> io::Result<bool> {
    match file.try_lock_shared() {
        Ok(()) => Ok(true),
        Err(std::fs::TryLockError::WouldBlock) => Ok(false),
        Err(std::fs::TryLockError::Error(error)) => Err(error),
    }
}

#[cfg(target_os = "android")]
fn try_shared(file: &File) -> io::Result<bool> {
    flock(file, rustix::fs::FlockOperation::NonBlockingLockShared)
}

/// 假是被占用，不是出错；从不等待。
#[cfg(not(target_os = "android"))]
fn try_exclusive(file: &File) -> io::Result<bool> {
    match file.try_lock() {
        Ok(()) => Ok(true),
        Err(std::fs::TryLockError::WouldBlock) => Ok(false),
        Err(std::fs::TryLockError::Error(error)) => Err(error),
    }
}

#[cfg(target_os = "android")]
fn try_exclusive(file: &File) -> io::Result<bool> {
    flock(file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
}

#[cfg(target_os = "android")]
fn flock(file: &File, operation: rustix::fs::FlockOperation) -> io::Result<bool> {
    match rustix::fs::flock(file, operation) {
        Ok(()) => Ok(true),
        Err(rustix::io::Errno::WOULDBLOCK | rustix::io::Errno::INTR) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

#[cfg(all(unix, not(target_os = "android")))]
fn unlock(file: &File) -> io::Result<()> {
    file.unlock()
}

#[cfg(target_os = "android")]
fn unlock(file: &File) -> io::Result<()> {
    rustix::fs::flock(file, rustix::fs::FlockOperation::Unlock).map_err(Into::into)
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

    /// 旧版本的宿主直接对两个根目录里的 `.msime-dictionary-access.lock` 取文件锁；搬进引擎后的租约必须和它互斥，文件名和锁的种类都不能变。旧锁在这里用标准库来取，Android 上标准库的文件锁不可用，这条只在其他平台跑；Android 上新旧两边本来就是同一个 `flock`。
    #[cfg(not(target_os = "android"))]
    #[test]
    fn interoperates_with_a_lock_taken_on_the_same_file_names() {
        let user = tempfile::tempdir().unwrap();
        let dictionaries = tempfile::tempdir().unwrap();
        let open = |root: &Path| {
            File::options()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(root.join(".msime-dictionary-access.lock"))
                .unwrap()
        };
        let older_session = open(dictionaries.path());
        older_session.try_lock_shared().unwrap();
        assert!(
            DictionaryAccess::try_maintenance(user.path(), dictionaries.path())
                .unwrap()
                .is_none()
        );
        assert!(
            DictionaryAccess::try_session(user.path(), dictionaries.path())
                .unwrap()
                .is_some()
        );
        drop(older_session);

        let maintenance = DictionaryAccess::try_maintenance(user.path(), dictionaries.path())
            .unwrap()
            .unwrap();
        let older_writer = open(user.path());
        assert!(matches!(
            older_writer.try_lock(),
            Err(std::fs::TryLockError::WouldBlock)
        ));
        assert!(matches!(
            older_writer.try_lock_shared(),
            Err(std::fs::TryLockError::WouldBlock)
        ));
        drop(maintenance);
        older_writer.try_lock().unwrap();
    }

    #[cfg(unix)]
    fn assert_access_releases_with_duplicate_descriptors(exclusive: bool) {
        let user = tempfile::tempdir().unwrap();
        let dictionaries = tempfile::tempdir().unwrap();
        let access = DictionaryAccess::acquire(user.path(), dictionaries.path(), exclusive)
            .unwrap()
            .unwrap();
        let duplicates: Vec<_> = access
            .files
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
        std::fs::create_dir(second.join(LOCK_FILE)).unwrap();

        assert!(DictionaryAccess::try_maintenance(&first, &second).is_err());
        assert!(DictionaryAccess::try_maintenance(&first, &first)
            .unwrap()
            .is_some());
    }

    #[test]
    fn relative_roots_are_refused() {
        let root = tempfile::tempdir().unwrap();
        let error = DictionaryAccess::try_maintenance(Path::new("user"), root.path())
            .err()
            .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
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
        assert!(!outside.path().join(LOCK_FILE).exists());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_hard_linked_lock_file() {
        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.lock");
        std::fs::write(&target, b"synthetic-lock-target").unwrap();
        std::fs::hard_link(&target, root.path().join(LOCK_FILE)).unwrap();

        assert!(DictionaryAccess::try_maintenance(root.path(), root.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn the_directory_handle_survives_root_replacement() {
        let root = tempfile::tempdir().unwrap();
        let user = root.path().join("user");
        let dictionaries = root.path().join("dictionaries");
        std::fs::create_dir(&user).unwrap();
        std::fs::create_dir(&dictionaries).unwrap();
        let access = DictionaryAccess::try_maintenance(&user, &dictionaries)
            .unwrap()
            .unwrap();
        let bound = access.directory(&user).unwrap();
        assert_eq!(
            access
                .directory(&root.path().join("elsewhere"))
                .err()
                .map(|error| error.kind()),
            Some(io::ErrorKind::NotFound)
        );

        let moved = root.path().join("moved-user");
        std::fs::rename(&user, &moved).unwrap();
        std::fs::create_dir(&user).unwrap();
        let marker = rustix::fs::openat(
            &bound,
            "marker",
            rustix::fs::OFlags::WRONLY | rustix::fs::OFlags::CREATE | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::from_raw_mode(0o600),
        )
        .unwrap();
        drop(marker);

        assert!(moved.join("marker").exists());
        assert!(!user.join("marker").exists());
    }
}
