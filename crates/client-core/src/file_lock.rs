//! Android std::fs::File::lock is unsupported; use rustix's safe flock on that target.
//! The owning File keeps the lock alive and releases it when closed.
//!
//! This module is public so that no caller anywhere in the workspace has to re-derive which
//! locking API works on which target. `host-api` had its own `File::lock` call, and on Android it
//! failed on the first line of every shared clipboard operation - which made the keyboard's own
//! `onCreateInputView` throw and the input method die before it could draw a single key.
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

/// A parent directory opened without following an untrusted symlink. On Unix,
/// all operations through this value stay bound to that directory if its path
/// is replaced later.
pub struct PrivateDirectory {
    #[cfg(unix)]
    directory: File,
    #[cfg(not(unix))]
    directory: crate::storage::PrivateDirectory,
}

pub fn open_private_directory(path: impl AsRef<Path>) -> io::Result<PrivateDirectory> {
    Ok(PrivateDirectory {
        directory: crate::storage::open_private_directory(path.as_ref())?,
    })
}

impl PrivateDirectory {
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            directory: crate::storage::clone_private_directory(&self.directory)?,
        })
    }

    pub fn metadata(&self) -> io::Result<std::fs::Metadata> {
        crate::storage::private_directory_metadata(&self.directory)
    }
}

pub fn open_private_directory_at(
    directory: &PrivateDirectory,
    name: &OsStr,
) -> io::Result<PrivateDirectory> {
    Ok(PrivateDirectory {
        directory: crate::storage::open_private_directory_at(&directory.directory, name)?,
    })
}

pub fn open_private_lock_file_at(directory: &PrivateDirectory, name: &OsStr) -> io::Result<File> {
    ensure_regular(crate::storage::open_private_lock_file_at(
        &directory.directory,
        name,
    )?)
}

#[cfg(unix)]
pub(crate) fn open_private_lock_file_at_raw(directory: &File, name: &OsStr) -> io::Result<File> {
    ensure_regular(crate::storage::open_private_lock_file_at(directory, name)?)
}

pub fn read_private_directory(directory: &PrivateDirectory) -> io::Result<Vec<std::ffi::OsString>> {
    crate::storage::read_private_directory(&directory.directory)
}

pub fn create_private_directory_at(parent: &PrivateDirectory, name: &OsStr) -> io::Result<()> {
    crate::storage::create_private_directory_at(&parent.directory, name)
}

pub fn remove_private_directory_at(parent: &PrivateDirectory, name: &OsStr) -> io::Result<()> {
    crate::storage::remove_private_directory_at(&parent.directory, name)
}

pub fn rename_private_entry(
    from: &PrivateDirectory,
    from_name: &OsStr,
    to: &PrivateDirectory,
    to_name: &OsStr,
) -> io::Result<()> {
    crate::storage::rename_private_entry(&from.directory, from_name, &to.directory, to_name)
}

pub fn open_private_file_at(directory: &PrivateDirectory, name: &OsStr) -> io::Result<File> {
    crate::storage::open_private_file_at(&directory.directory, name)
}

pub fn remove_private_file_at(directory: &PrivateDirectory, name: &OsStr) -> io::Result<()> {
    crate::storage::remove_private_file_at(&directory.directory, name)
}

/// Replace a private file relative to an already opened parent directory.
pub fn write_private_file_at(
    directory: &PrivateDirectory,
    name: &OsStr,
    contents: &[u8],
) -> io::Result<()> {
    crate::storage::write_private_file_at(&directory.directory, name, contents)
}

pub fn replace_private_file_at(
    directory: &PrivateDirectory,
    name: &OsStr,
    contents: &[u8],
    permissions: &std::fs::Permissions,
) -> io::Result<()> {
    crate::storage::replace_private_file_at(&directory.directory, name, contents, Some(permissions))
}

fn lock_file_options() -> OpenOptions {
    let mut options = File::options();
    options.read(true).write(true).create(true).truncate(false);
    options
}

fn secure_lock_file_options(path: &Path) -> io::Result<OpenOptions> {
    crate::storage::reject_symlink(path)?;
    let mut options = lock_file_options();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    Ok(options)
}

pub(crate) fn open_lock_file(path: impl AsRef<Path>) -> io::Result<File> {
    let path = path.as_ref();
    ensure_regular(secure_lock_file_options(path)?.open(path)?)
}

/// Open a lock file with owner-only permissions on Unix hosts.
pub fn open_private_lock_file(path: impl AsRef<Path>) -> io::Result<File> {
    let path = path.as_ref();
    let options = secure_lock_file_options(path)?;
    #[cfg(unix)]
    let options = {
        use std::os::unix::fs::OpenOptionsExt;
        let mut options = options;
        options.mode(0o600);
        options
    };
    ensure_regular(options.open(path)?)
}

fn ensure_regular(file: File) -> io::Result<File> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || !has_single_link(&file)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "lock file is not a single-link regular file",
        ));
    }
    Ok(file)
}

/// 已打开的文件是否只有一个硬链接。私有文件被硬链接到别处时，写它就会改到链接另一端的文件，打开后用这个拒绝。
///
/// 放在这个公开模块里，是为了工作区各处共用同一份按平台的实现：Windows 上标准库的 `number_of_links` 还是不稳定特性，此前四个 crate 各抄一份，在 Windows 上一起编译失败。
#[cfg(unix)]
pub fn has_single_link(file: &File) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    Ok(file.metadata()?.nlink() == 1)
}

/// 已打开的文件是否只有一个硬链接；见 Unix 版的说明。
#[cfg(windows)]
pub fn has_single_link(file: &File) -> io::Result<bool> {
    Ok(winapi_util::file::information(file)?.number_of_links() == 1)
}

/// 没有硬链接计数可读的平台一律按单链接处理。
#[cfg(not(any(unix, windows)))]
pub fn has_single_link(_: &File) -> io::Result<bool> {
    Ok(true)
}

/// 以只读方式打开文件，并拒绝跟随最后一级符号链接。
///
/// 多链接文件只在所在目录只有 root 或当前用户能写时放行（`msime_path_trust::multi_link_is_trusted`）：Nix 的 store 去重、ostree 部署会把安装目录里的文件合并成硬链接（#6386），而在别人能写的目录里，链接可能是他们放进来的。读不会改到链接另一端；写入路径（锁文件等）继续只接受单链接。
pub fn open_private_file(path: impl AsRef<Path>) -> io::Result<File> {
    let path = path.as_ref();
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // A user-controlled FIFO must not block the host thread while it is opened.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || !(has_single_link(&file)? || msime_path_trust::multi_link_is_trusted(&file, path)?)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private input is not a regular file with a trusted link count",
        ));
    }
    Ok(file)
}

/// 相对于已检查的父目录打开私有文件；Unix 调用方会绑定到该目录句柄。
pub fn open_private_file_in(path: impl AsRef<Path>) -> io::Result<File> {
    crate::storage::open_private_file_in(path.as_ref())
}

/// 替换私有文件时不再通过路径解析可能已被替换的 Unix 父目录。
pub fn replace_private_file(path: impl AsRef<Path>, contents: &[u8]) -> io::Result<()> {
    crate::storage::replace_private_file(path.as_ref(), contents)
}

/// Replace a private file atomically while preserving the supplied permissions.
pub fn replace_private_file_with_permissions(
    path: impl AsRef<Path>,
    contents: &[u8],
    permissions: &std::fs::Permissions,
) -> io::Result<()> {
    crate::storage::replace_private_file_with_permissions(
        path.as_ref(),
        contents,
        Some(permissions),
    )
}

pub(crate) fn try_shared(file: &File) -> io::Result<bool> {
    #[cfg(not(target_os = "android"))]
    {
        match file.try_lock_shared() {
            Ok(()) => Ok(true),
            Err(std::fs::TryLockError::WouldBlock) => Ok(false),
            Err(std::fs::TryLockError::Error(error)) => Err(error),
        }
    }
    #[cfg(target_os = "android")]
    {
        match rustix::fs::flock(file, rustix::fs::FlockOperation::NonBlockingLockShared) {
            Ok(()) => Ok(true),
            Err(rustix::io::Errno::WOULDBLOCK | rustix::io::Errno::INTR) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}

/// False is contention, not an I/O failure. Never waits for another lock owner.
pub(crate) fn try_exclusive(file: &File) -> io::Result<bool> {
    #[cfg(not(target_os = "android"))]
    {
        match file.try_lock() {
            Ok(()) => Ok(true),
            Err(std::fs::TryLockError::WouldBlock) => Ok(false),
            Err(std::fs::TryLockError::Error(error)) => Err(error),
        }
    }
    #[cfg(target_os = "android")]
    {
        match rustix::fs::flock(file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Ok(true),
            Err(rustix::io::Errno::WOULDBLOCK | rustix::io::Errno::INTR) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}

/// `try_exclusive`，但对「刚被释放却还没真正放开」的那一瞬给一点宽限。
///
/// flock 的锁属于打开文件描述，而不是进程。任何一次 fork 都会让子进程在 exec 之前持有
/// 父进程全部描述符的副本，父进程此刻关掉文件也不会立即解锁——锁要等最后一个副本消失。
/// 这个窗口只有毫秒级，但它足以让另一条路径拿到一个转瞬即逝的「被占用」，而调用方会把
/// 它当成真的有人在用。本进程自己就会拉起子进程（provider 入口、桌面面板），所以这不是
/// 只在测试里才发生的事：仓库的测试套件正是因为剪贴板那组测试会启动四个子进程，才让快照
/// 队列的测试随机变红。
///
/// 因此在报告占用之前短暂重试。总时长有上限，仍然不会无限等待——真正被别的进程长期持有
/// 的锁还是会如实返回 false。
pub(crate) fn try_exclusive_with_grace(file: &File) -> io::Result<bool> {
    const ATTEMPTS: u32 = 10;
    const INTERVAL: std::time::Duration = std::time::Duration::from_millis(5);
    for attempt in 0..ATTEMPTS {
        if try_exclusive(file)? {
            return Ok(true);
        }
        if attempt + 1 < ATTEMPTS {
            std::thread::sleep(INTERVAL);
        }
    }
    Ok(false)
}

pub fn exclusive(file: &File) -> io::Result<()> {
    #[cfg(not(target_os = "android"))]
    {
        file.lock()
    }
    #[cfg(target_os = "android")]
    {
        loop {
            match rustix::fs::flock(file, rustix::fs::FlockOperation::LockExclusive) {
                Ok(()) => return Ok(()),
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
}

#[cfg(unix)]
pub(crate) fn unlock(file: &File) -> io::Result<()> {
    #[cfg(not(target_os = "android"))]
    {
        file.unlock()
    }
    #[cfg(target_os = "android")]
    {
        rustix::fs::flock(file, rustix::fs::FlockOperation::Unlock).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn directory_bound_rename_stays_on_original_inode_after_path_replacement() {
        use std::fs;

        let root = tempfile::tempdir().unwrap();
        let live = root.path().join("live");
        let moved = root.path().join("moved");
        let backup = root.path().join("backup");
        fs::create_dir(&live).unwrap();
        fs::create_dir(&backup).unwrap();
        fs::write(live.join("marker"), b"old").unwrap();
        let live_handle = open_private_directory(&live).unwrap();
        let backup_handle = open_private_directory(&backup).unwrap();

        fs::rename(&live, &moved).unwrap();
        fs::create_dir(&live).unwrap();

        rename_private_entry(
            &live_handle,
            OsStr::new("marker"),
            &backup_handle,
            OsStr::new("marker"),
        )
        .unwrap();

        assert!(!moved.join("marker").exists());
        assert!(!live.join("marker").exists());
        assert_eq!(fs::read(backup.join("marker")).unwrap(), b"old");
    }
    use std::time::{Duration, Instant};

    // 宽限期内释放的锁应当被拿到，长期被持有的锁仍要如实报告占用——后者是产品语义，
    // 不能因为加了重试就变成无限等待。
    #[cfg(unix)]
    #[test]
    fn refuses_a_symlinked_lock_leaf() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.lock");
        std::fs::write(&target, b"synthetic-lock-target").unwrap();
        let linked = root.path().join("state.lock");
        symlink(&target, &linked).unwrap();

        assert!(open_lock_file(&linked).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic-lock-target");
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlinked_private_lock_leaf() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.lock");
        std::fs::write(&target, b"synthetic-private-lock-target").unwrap();
        let linked = root.path().join("state.lock");
        symlink(&target, &linked).unwrap();

        assert!(open_private_lock_file(&linked).is_err());
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"synthetic-private-lock-target"
        );
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_hard_linked_private_file() {
        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.json");
        std::fs::write(&target, b"synthetic-private-target").unwrap();
        let linked = root.path().join("state.json");
        std::fs::hard_link(&target, &linked).unwrap();
        msime_path_trust::open_to_other_users(root.path()).unwrap();

        assert!(open_private_file(&linked).is_err());
    }

    /// 只有属主能写的目录里，多链接文件是 root 或用户自己建的（Nix 的 store 去重、`cp -al` 备份），只读打开照常进行（#6386）。
    #[cfg(unix)]
    #[test]
    fn reads_a_hard_linked_file_in_a_closed_directory() {
        let root = tempfile::tempdir().unwrap();
        msime_path_trust::close_to_other_users(root.path()).unwrap();
        let original = root.path().join("edition.json");
        std::fs::write(&original, b"synthetic-edition").unwrap();
        std::fs::hard_link(&original, root.path().join("deduplicated.json")).unwrap();

        let mut contents = Vec::new();
        std::io::Read::read_to_end(&mut open_private_file(&original).unwrap(), &mut contents)
            .unwrap();
        assert_eq!(contents, b"synthetic-edition");
        // 锁文件会被写，仍只接受单链接。
        assert!(open_private_lock_file(root.path().join("deduplicated.json")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_hard_linked_private_lock_leaf() {
        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.lock");
        std::fs::write(&target, b"synthetic-private-lock-target").unwrap();
        let linked = root.path().join("state.lock");
        std::fs::hard_link(&target, &linked).unwrap();

        assert!(open_private_lock_file(&linked).is_err());
    }

    #[cfg(unix)]
    #[test]
    #[cfg_attr(
        target_os = "android",
        ignore = "Android 的 adb shell 域不允许建 FIFO（SELinux 拒绝 fifo_file create）"
    )]
    fn refuses_a_fifo_lock_leaf() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.lock");
        assert!(std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success());

        assert!(open_lock_file(&path).is_err());
        assert!(open_private_lock_file(&path).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_directory_as_a_private_file() {
        let root = tempfile::tempdir().unwrap();

        assert!(open_private_file(root.path()).is_err());
    }

    #[test]
    fn grace_waits_for_a_lock_that_is_about_to_be_released_and_still_reports_real_contention() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("lease");
        let holder = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .unwrap();
        assert!(try_exclusive(&holder).unwrap());

        // 另一条路径此刻看到的是占用；宽限期结束后仍然是 false，而不是永远等下去。
        let waiter = File::options().read(true).write(true).open(&path).unwrap();
        let started = Instant::now();
        assert!(!try_exclusive_with_grace(&waiter).unwrap());
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "宽限期必须有上限"
        );

        // 持有者在宽限期内放手，等待方应当拿到锁，而不是报告一个转瞬即逝的占用。
        let releasing = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(15));
            drop(holder);
        });
        assert!(try_exclusive_with_grace(&waiter).unwrap());
        releasing.join().unwrap();
    }
}
