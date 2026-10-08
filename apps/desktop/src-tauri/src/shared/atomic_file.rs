#[cfg(unix)]
use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::fd::OwnedFd;
#[cfg(all(unix, any(target_os = "ios", target_os = "android", test)))]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Check that `path` and all existing ancestors are real directories.
///
/// Refusing symlink ancestors keeps callers from writing through a redirected
/// settings or export directory.
pub(crate) fn check_directory_ancestors(path: &Path) -> io::Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                if !msime_path_trust::is_trusted_system_alias(&current) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "directory has a symbolic-link ancestor",
                    ));
                }
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    "directory parent is not a directory",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Create `path` only when it and all existing ancestors are real directories.
///
/// The final metadata check also closes the common race where a missing path is
/// replaced while `create_dir_all` runs.
pub(crate) fn create_directory_and_check(path: &Path) -> io::Result<()> {
    check_directory_ancestors(path)?;
    std::fs::create_dir_all(path)?;
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "directory is not a real directory",
        ));
    }
    Ok(())
}

/// Open a host-owned file without following a replaced leaf symlink.
pub(crate) fn open_private(path: &Path) -> io::Result<File> {
    #[cfg(unix)]
    {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let name = path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "private file has no name")
        })?;
        let directory = open_private_directory(parent)?;
        return open_private_fd(&directory, name);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        let file = options.open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "private input is not a regular file",
            ));
        }
        if !msime_client_core::file_lock::has_single_link(&file)? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "private input must have a single link",
            ));
        }
        return Ok(file);
    }
    #[allow(unreachable_code)]
    let file = OpenOptions::new().read(true).open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private input is not a regular file",
        ));
    }
    Ok(file)
}

#[cfg(unix)]
pub(crate) fn open_private_fd(directory: &OwnedFd, name: &OsStr) -> io::Result<File> {
    let descriptor = rustix::fs::openat(
        directory,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| {
        if error == rustix::io::Errno::LOOP {
            io::Error::new(io::ErrorKind::InvalidInput, "private input is a symlink")
        } else {
            error.into()
        }
    })?;
    let stat = rustix::fs::fstat(&descriptor)?;
    if !rustix::fs::FileType::from_raw_mode(stat.st_mode).is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private input is not a regular file",
        ));
    }
    if stat.st_nlink != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private input must have a single link",
        ));
    }
    Ok(descriptor.into())
}

/// Remove a private file relative to its opened parent directory. Opening the
/// parent with `O_NOFOLLOW` keeps a concurrent replacement of the final
/// directory component from redirecting cleanup through a symlink.
#[cfg(any(target_os = "ios", target_os = "android", target_os = "linux", test))]
pub(crate) fn remove_private(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let name = path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "private file has no name")
        })?;
        let directory = open_private_directory(parent)?;
        remove_private_at(&directory, name)
    }
    #[cfg(not(unix))]
    {
        std::fs::remove_file(path)
    }
}

#[cfg(all(
    unix,
    any(target_os = "ios", target_os = "android", target_os = "linux", test)
))]
pub(crate) fn remove_private_at(directory: &OwnedFd, name: &OsStr) -> io::Result<()> {
    rustix::fs::unlinkat(directory, name, rustix::fs::AtFlags::empty()).map_err(Into::into)
}

/// Replace a file after fully writing and syncing a temporary sibling.
pub(crate) fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent: PathBuf = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    create_directory_and_check(&parent)?;
    #[cfg(unix)]
    {
        let name = path
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "file path has no name"))?;
        let directory = open_private_directory(&parent)?;
        write_in_directory(&directory, name, contents)
    }
    #[cfg(not(unix))]
    {
        let mut temporary = tempfile::NamedTempFile::new_in(&parent)?;
        temporary.write_all(contents)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(path)
            .map(|_| ())
            .map_err(|error| error.error)
    }
}

/// Open a parent directory and verify that the descriptor still names the
/// directory selected by the path. Callers can then use the descriptor for
/// all writes, so a replacement of the path cannot redirect the operation.
///
/// Apple 平台不逐级打开：iOS 沙盒不允许应用以读方式打开容器的祖先目录，逐级 `RDONLY` 在走到 App Group 容器之前就 EPERM，而 Darwin 没有 `O_PATH`。`O_NOFOLLOW_ANY` 让内核在一次解析里拒绝路径上任何一级符号链接，解析只需要祖先的搜索权限。路径经过 `msime-path-trust` 列出的系统别名（`/var`、`/tmp`）时先换成它唯一受信任的目标再打开。与 `msime-client-core` 的 `storage::open_private_directory` 保持一致。
#[cfg(target_vendor = "apple")]
pub(crate) fn open_private_directory(parent: &Path) -> io::Result<OwnedFd> {
    let flags = rustix::fs::OFlags::RDONLY
        | rustix::fs::OFlags::DIRECTORY
        | rustix::fs::OFlags::NOFOLLOW_ANY
        | rustix::fs::OFlags::CLOEXEC
        | rustix::fs::OFlags::NONBLOCK;
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    match rustix::fs::open(parent, flags, rustix::fs::Mode::empty()) {
        Err(rustix::io::Errno::LOOP) => {}
        result => return Ok(result?),
    }
    let mut resolved = PathBuf::new();
    for component in parent.components() {
        resolved.push(component);
        if msime_path_trust::is_trusted_system_alias(&resolved) {
            let target = std::fs::read_link(&resolved)?;
            resolved = resolved
                .parent()
                .unwrap_or_else(|| Path::new("/"))
                .join(target);
        }
    }
    if resolved == parent {
        return Err(rustix::io::Errno::LOOP.into());
    }
    Ok(rustix::fs::open(
        &resolved,
        flags,
        rustix::fs::Mode::empty(),
    )?)
}

/// Open a parent directory and verify that the descriptor still names the
/// directory selected by the path. Callers can then use the descriptor for
/// all writes, so a replacement of the path cannot redirect the operation.
#[cfg(all(unix, not(target_vendor = "apple")))]
pub(crate) fn open_private_directory(parent: &Path) -> io::Result<OwnedFd> {
    let flags = rustix::fs::OFlags::RDONLY
        | rustix::fs::OFlags::DIRECTORY
        | rustix::fs::OFlags::NOFOLLOW
        | rustix::fs::OFlags::CLOEXEC
        | rustix::fs::OFlags::NONBLOCK;
    // 沿途的祖先目录只需要搜索权限（`x`）。Android 只给应用 `/data` 和 `/data/user` 的 `x`、不给 `r`，按 `RDONLY` 逐级打开会在走到应用自己的目录之前就 EACCES。Linux 和 Android 用 `O_PATH` 只凭搜索权限打开祖先，`NOFOLLOW` 照样拒绝符号链接；走完后再用 `flags` 重新打开目录本身，调用方拿到的描述符和以前一样。
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let search = rustix::fs::OFlags::PATH
        | rustix::fs::OFlags::DIRECTORY
        | rustix::fs::OFlags::NOFOLLOW
        | rustix::fs::OFlags::CLOEXEC;
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let search = flags;
    let absolute = parent.is_absolute();
    let mut directory = rustix::fs::open(
        if absolute {
            Path::new("/")
        } else {
            Path::new(".")
        },
        search,
        rustix::fs::Mode::empty(),
    )?;
    let mut logical = if absolute {
        PathBuf::from("/")
    } else {
        PathBuf::from(".")
    };
    for component in parent.components() {
        match component {
            std::path::Component::Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "directory path has an unsupported prefix",
                ));
            }
            std::path::Component::RootDir | std::path::Component::CurDir => continue,
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
    Ok(rustix::fs::openat(
        &directory,
        ".",
        flags,
        rustix::fs::Mode::empty(),
    )?)
}

/// Check that a path still names the directory held by `directory`.
#[cfg(all(unix, any(target_os = "ios", target_os = "android", test)))]
#[allow(clippy::useless_conversion)]
pub(crate) fn directory_matches(path: &Path, directory: &OwnedFd) -> io::Result<bool> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Ok(false);
    }
    let stat = rustix::fs::fstat(directory)?;
    Ok(u64::try_from(stat.st_dev).ok() == Some(metadata.dev())
        && u64::try_from(stat.st_ino).ok() == Some(metadata.ino()))
}

/// Atomically replace `name` using only an already opened parent directory.
#[cfg(unix)]
fn write_in_directory(directory: &OwnedFd, name: &OsStr, contents: &[u8]) -> io::Result<()> {
    let mut temporary_name = OsString::from(".msime-atomic-");
    temporary_name.push(std::process::id().to_string());
    temporary_name.push("-");
    temporary_name.push(
        TEMPORARY_COUNTER
            .fetch_add(1, Ordering::Relaxed)
            .to_string(),
    );
    let descriptor = loop {
        match rustix::fs::openat(
            directory,
            &temporary_name,
            rustix::fs::OFlags::WRONLY
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::from_raw_mode(0o600),
        ) {
            Ok(descriptor) => break descriptor,
            Err(error) if error == rustix::io::Errno::EXIST => {
                temporary_name.push("-");
                temporary_name.push(
                    TEMPORARY_COUNTER
                        .fetch_add(1, Ordering::Relaxed)
                        .to_string(),
                );
            }
            Err(error) => return Err(error.into()),
        }
    };
    let mut file: File = descriptor.into();
    let result = file.write_all(contents).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = result {
        let _ = rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty());
        return Err(error);
    }
    if let Err(error) = rustix::fs::renameat(directory, &temporary_name, directory, name) {
        let _ = rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty());
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_ancestor_before_creating_the_parent() {
        use msime_path_trust::untrusted_symlink as symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let linked = root.path().join("linked");
        symlink(outside.path(), &linked).unwrap();
        let path = linked.join("missing").join("state.json");

        assert!(write(&path, b"synthetic").is_err());
        assert!(!outside.path().join("missing").exists());
    }

    #[cfg(unix)]
    #[test]
    fn write_stays_bound_to_the_open_parent_when_its_path_is_replaced() {
        use std::ffi::OsStr;
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let parent = root.path().join("state");
        let moved = root.path().join("state-moved");
        std::fs::create_dir(&parent).unwrap();
        let directory = open_private_directory(&parent).unwrap();

        std::fs::rename(&parent, &moved).unwrap();
        symlink(outside.path(), &parent).unwrap();

        write_in_directory(&directory, OsStr::new("marker"), b"synthetic").unwrap();

        assert_eq!(std::fs::read(moved.join("marker")).unwrap(), b"synthetic");
        assert!(!outside.path().join("marker").exists());
    }
}

#[cfg(test)]
mod private_open_tests {
    #[cfg(unix)]
    #[test]
    fn private_remove_refuses_a_symlinked_parent() {
        use super::remove_private;
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("private-input");
        std::fs::write(&target, b"synthetic-outside").unwrap();
        let linked = root.path().join("linked");
        symlink(outside.path(), &linked).unwrap();

        assert!(remove_private(&linked.join("private-input")).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic-outside");
    }

    // 模拟 Android 的 `/data` 和 iOS 沙盒里 App Group 容器的祖先：祖先目录只有搜索权限、没有读权限，它下面的应用私有目录照样要能打开。以 root 运行时权限检查不生效，这条测试只在普通用户下有区分度。
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    #[test]
    fn private_directory_opens_below_a_search_only_ancestor() {
        use super::{open_private_directory, open_private_fd};
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let ancestor = root.path().join("search-only");
        let private = ancestor.join("private");
        std::fs::create_dir_all(&private).unwrap();
        std::fs::write(private.join("private-input"), b"synthetic").unwrap();
        std::fs::set_permissions(&ancestor, std::fs::Permissions::from_mode(0o111)).unwrap();

        let opened = open_private_directory(&private).and_then(|directory| {
            open_private_fd(&directory, std::ffi::OsStr::new("private-input"))
        });
        std::fs::set_permissions(&ancestor, std::fs::Permissions::from_mode(0o700)).unwrap();

        opened.unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn private_remove_stays_bound_to_the_open_parent_when_its_path_is_replaced() {
        use super::{open_private_directory, remove_private_at};

        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("state");
        let moved = root.path().join("state-moved");
        let replacement = root.path().join("replacement");
        std::fs::create_dir(&parent).unwrap();
        std::fs::create_dir(&replacement).unwrap();
        std::fs::write(parent.join("private-input"), b"original").unwrap();
        std::fs::write(replacement.join("private-input"), b"replacement").unwrap();
        let directory = open_private_directory(&parent).unwrap();

        std::fs::rename(&parent, &moved).unwrap();
        std::fs::rename(&replacement, &parent).unwrap();

        remove_private_at(&directory, std::ffi::OsStr::new("private-input")).unwrap();

        assert!(!moved.join("private-input").exists());
        assert_eq!(
            std::fs::read(parent.join("private-input")).unwrap(),
            b"replacement"
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_open_stays_bound_to_the_open_parent_when_its_path_is_replaced() {
        use super::{open_private_directory, open_private_fd};
        use std::io::Read;

        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("state");
        let moved = root.path().join("state-moved");
        let replacement = root.path().join("replacement");
        std::fs::create_dir(&parent).unwrap();
        std::fs::create_dir(&replacement).unwrap();
        std::fs::write(parent.join("private-input"), b"original").unwrap();
        std::fs::write(replacement.join("private-input"), b"replacement").unwrap();
        let directory = open_private_directory(&parent).unwrap();

        std::fs::rename(&parent, &moved).unwrap();
        std::fs::rename(&replacement, &parent).unwrap();

        let mut file = open_private_fd(&directory, std::ffi::OsStr::new("private-input")).unwrap();
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();
        assert_eq!(contents, b"original");
    }

    #[cfg(unix)]
    #[test]
    fn private_open_rejects_an_untrusted_ancestor_link() {
        use super::open_private;
        use msime_path_trust::untrusted_symlink as symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let linked = root.path().join("linked");
        let outside_state = outside.path().join("state");
        std::fs::create_dir(&outside_state).unwrap();
        std::fs::write(outside_state.join("private-input"), b"synthetic").unwrap();
        symlink(outside.path(), &linked).unwrap();

        assert!(open_private(&linked.join("state/private-input")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn private_open_rejects_a_symlinked_leaf() {
        use super::open_private;
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.json");
        std::fs::write(&target, b"synthetic-private-data").unwrap();
        let linked = root.path().join("private.json");
        symlink(&target, &linked).unwrap();

        assert!(open_private(&linked).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic-private-data");
    }

    #[cfg(unix)]
    #[test]
    fn private_open_rejects_a_hard_linked_leaf() {
        use super::open_private;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.json");
        std::fs::write(&target, b"synthetic-private-data").unwrap();
        let linked = root.path().join("private.json");
        std::fs::hard_link(&target, &linked).unwrap();

        assert!(open_private(&linked).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic-private-data");
    }

    #[cfg(windows)]
    #[test]
    fn private_open_rejects_a_hard_linked_leaf() {
        use super::open_private;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.json");
        std::fs::write(&target, b"synthetic-private-data").unwrap();
        let linked = root.path().join("private.json");
        std::fs::hard_link(&target, &linked).unwrap();

        assert!(open_private(&linked).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic-private-data");
    }

    #[cfg(unix)]
    #[test]
    fn private_open_rejects_a_fifo_without_blocking() {
        use super::open_private;
        use rustix::fs::{open, Mode, OFlags};
        use std::sync::mpsc;
        use std::time::Duration;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("private-input");
        assert!(std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success());

        let (done, ready) = mpsc::channel();
        let worker_path = path.clone();
        let worker = std::thread::spawn(move || {
            let file = open_private(&worker_path);
            done.send(file.is_err()).unwrap();
        });
        let rejected = match ready.recv_timeout(Duration::from_millis(100)) {
            Ok(rejected) => rejected,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _writer =
                    open(&path, OFlags::WRONLY | OFlags::NONBLOCK, Mode::empty()).unwrap();
                ready.recv_timeout(Duration::from_secs(1)).unwrap()
            }
            Err(error) => panic!("private reader failed to report: {error}"),
        };
        worker.join().unwrap();
        assert!(
            rejected,
            "FIFO private input must be rejected without blocking"
        );
    }
}
