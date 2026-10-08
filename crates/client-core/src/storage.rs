use std::ffi::OsStr;
#[cfg(unix)]
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
static PRIVATE_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(unix)]
pub(crate) fn open_private_directory(parent: &Path) -> io::Result<File> {
    let flags = rustix::fs::OFlags::RDONLY
        | rustix::fs::OFlags::DIRECTORY
        | rustix::fs::OFlags::NOFOLLOW
        | rustix::fs::OFlags::CLOEXEC
        | rustix::fs::OFlags::NONBLOCK;
    // 沿途的祖先目录只需要搜索权限（`x`）。Android 只给应用 `/data` 和 `/data/user` 的 `x`、不给 `r`，按 `RDONLY` 逐级打开会在走到应用自己的目录之前就 EACCES，所有私有文件读写因此全部失败。Linux 和 Android 用 `O_PATH` 只凭搜索权限打开祖先，`NOFOLLOW` 照样拒绝符号链接；走完后再用 `flags` 重新打开目录本身，调用方拿到的描述符和以前一样。
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

#[cfg(unix)]
pub(crate) fn open_private_file_at(directory: &File, name: &OsStr) -> io::Result<File> {
    let descriptor = rustix::fs::openat(
        directory,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )?;
    let file: File = descriptor.into();
    let metadata = file.metadata()?;
    if !metadata.is_file() || !has_single_link(&metadata) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private input is not a single-link regular file",
        ));
    }
    Ok(file)
}

#[cfg(unix)]
fn has_single_link(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.nlink() == 1
}

#[cfg(windows)]
fn has_single_link(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.number_of_links() == 1
}

#[cfg(not(any(unix, windows)))]
fn has_single_link(_: &fs::Metadata) -> bool {
    true
}

/// Open a real child directory without following a symlink. The returned
/// descriptor remains bound to that directory if its parent entry is later
/// replaced.
#[cfg(unix)]
pub(crate) fn open_private_directory_at(directory: &File, name: &OsStr) -> io::Result<File> {
    let descriptor = rustix::fs::openat(
        directory,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )?;
    Ok(descriptor.into())
}

/// 非 Unix 平台上的「目录句柄」：没有 openat 这组调用，只记下已确认是真实目录（不是符号链接）的路径，`open_private_file_at` 再在它下面按名字打开。和 Unix 版的接口一致，调用方不必分平台。
#[cfg(not(unix))]
pub(crate) struct PrivateDirectory(PathBuf);

#[cfg(not(unix))]
pub(crate) fn open_private_directory(parent: &Path) -> io::Result<PrivateDirectory> {
    if !fs::symlink_metadata(parent)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private directory is not a real directory",
        ));
    }
    Ok(PrivateDirectory(parent.to_path_buf()))
}

#[cfg(not(unix))]
pub(crate) fn open_private_file_at(directory: &PrivateDirectory, name: &OsStr) -> io::Result<File> {
    let path = directory.0.join(name);
    if !fs::symlink_metadata(&path)?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private input is not a regular file",
        ));
    }
    open_private_file(&path)
}

#[cfg(unix)]
pub(crate) fn write_private_file_at(
    directory: &File,
    name: &OsStr,
    contents: &[u8],
) -> io::Result<()> {
    let temporary_name = private_temporary_name();
    let descriptor = rustix::fs::openat(
        directory,
        &temporary_name,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )?;
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

#[cfg(unix)]
fn private_temporary_name() -> OsString {
    let mut temporary_name = OsString::from(".msime-private-");
    temporary_name.push(std::process::id().to_string());
    temporary_name.push("-");
    temporary_name.push(
        PRIVATE_FILE_COUNTER
            .fetch_add(1, Ordering::Relaxed)
            .to_string(),
    );
    temporary_name
}

/// Publish a private file without replacing an existing destination.
///
/// Unix has no portable `renameat` no-replace operation across all supported
/// targets. A hard link from the unique temporary file is atomic and fails
/// with `AlreadyExists` when another process published first; removing the
/// temporary name then leaves the linked file at the destination.
#[cfg(unix)]
pub(crate) fn write_private_file_at_noclobber(
    directory: &File,
    name: &OsStr,
    contents: &[u8],
) -> io::Result<bool> {
    let temporary_name = private_temporary_name();
    let descriptor = rustix::fs::openat(
        directory,
        &temporary_name,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )?;
    let mut file: File = descriptor.into();
    let result = file.write_all(contents).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = result {
        let _ = rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty());
        return Err(error);
    }
    // Android 的 SELinux 不允许应用（以及 adb shell）建硬链接，下面的 `linkat` 在那里一律 EACCES，匿名账号等不覆盖写入全部失败。Linux 和 Android 先用 `renameat2(RENAME_NOREPLACE)`：同样原子、目标已存在时返回 EEXIST，不需要硬链接；文件系统不支持这个标志（EINVAL）或内核没有这个调用（ENOSYS）时才退回硬链接。
    #[cfg(any(target_os = "linux", target_os = "android"))]
    match rustix::fs::renameat_with(
        directory,
        &temporary_name,
        directory,
        name,
        rustix::fs::RenameFlags::NOREPLACE,
    ) {
        Ok(()) => return Ok(true),
        Err(rustix::io::Errno::EXIST) => {
            let _ = rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty());
            return Ok(false);
        }
        Err(rustix::io::Errno::INVAL | rustix::io::Errno::NOSYS) => {}
        Err(error) => {
            let _ = rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty());
            return Err(error.into());
        }
    }
    match rustix::fs::linkat(
        directory,
        &temporary_name,
        directory,
        name,
        rustix::fs::AtFlags::empty(),
    ) {
        Ok(()) => {
            rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty())?;
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let _ = rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty());
            Ok(false)
        }
        Err(error) => {
            let _ = rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty());
            Err(error.into())
        }
    }
}

/// Stream a private file into a directory-bound temporary file and publish it
/// with an atomic rename. The callback returns the finished file so callers
/// can use writers, such as a zip encoder, that consume their output handle.
#[cfg(unix)]
pub(crate) fn write_private_file_at_with<T, F>(
    directory: &File,
    name: &OsStr,
    writer: F,
) -> io::Result<(T, u64)>
where
    F: FnOnce(File) -> io::Result<(File, T)>,
{
    let temporary_name = private_temporary_name();
    let descriptor = rustix::fs::openat(
        directory,
        &temporary_name,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )?;
    let file: File = descriptor.into();
    let (file, value) = match writer(file) {
        Ok(result) => result,
        Err(error) => {
            let _ = rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty());
            return Err(error);
        }
    };
    let bytes = match file
        .metadata()
        .and_then(|metadata| file.sync_all().map(|()| metadata.len()))
    {
        Ok(bytes) => bytes,
        Err(error) => {
            drop(file);
            let _ = rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty());
            return Err(error);
        }
    };
    drop(file);
    if let Err(error) = rustix::fs::renameat(directory, &temporary_name, directory, name) {
        let _ = rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty());
        return Err(error.into());
    }
    Ok((value, bytes))
}

/// A private temporary file whose creation, publication and cleanup all use
/// the same opened directory. This lets large files be streamed before a
/// caller takes its state lock.
#[cfg(unix)]
pub(crate) struct PrivateStagedFile {
    directory: File,
    name: OsString,
    file: Option<File>,
}

#[cfg(unix)]
impl PrivateStagedFile {
    pub(crate) fn new(directory: &File) -> io::Result<Self> {
        let directory = directory.try_clone()?;
        let name = private_temporary_name();
        let descriptor = rustix::fs::openat(
            &directory,
            &name,
            rustix::fs::OFlags::WRONLY
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::from_raw_mode(0o600),
        )?;
        Ok(Self {
            directory,
            name,
            file: Some(descriptor.into()),
        })
    }

    pub(crate) fn file_mut(&mut self) -> &mut File {
        self.file.as_mut().expect("staged file was published")
    }

    pub(crate) fn directory(&self) -> &File {
        &self.directory
    }

    pub(crate) fn persist(mut self, name: &OsStr) -> io::Result<()> {
        self.file
            .take()
            .expect("staged file was published")
            .sync_all()?;
        rustix::fs::renameat(&self.directory, &self.name, &self.directory, name)?;
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for PrivateStagedFile {
    fn drop(&mut self) {
        let _ = rustix::fs::unlinkat(&self.directory, &self.name, rustix::fs::AtFlags::empty());
    }
}

/// 在存储操作跟随已有的符号链接之前先拒绝它。每个应用的存储都会经过的系统链接，以 `msime-path-trust` 列出的为准。
pub(crate) fn reject_symlink(path: &Path) -> io::Result<()> {
    msime_path_trust::reject_symlinked_components(path)
}

/// 打开私有文档时复用文件锁模块的无跟随实现。
pub(crate) fn open_private_file(path: &Path) -> io::Result<File> {
    crate::file_lock::open_private_file(path)
}

/// 打开私有目录中的普通文件；Unix 绑定父目录句柄，其他平台沿用私有文件打开策略。
pub(crate) fn open_private_file_in(path: &Path) -> io::Result<File> {
    #[cfg(unix)]
    {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let name = path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "private file has no name")
        })?;
        let directory = open_private_directory(parent)?;
        open_private_file_at(&directory, name)
    }
    #[cfg(not(unix))]
    {
        open_private_file(path)
    }
}

/// Remove a private file relative to an opened parent directory, so a
/// concurrent replacement of the directory cannot redirect cleanup through a
/// symlink.
pub(crate) fn remove_private_file(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let name = path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "private file has no name")
        })?;
        let directory = open_private_directory(parent)?;
        rustix::fs::unlinkat(&directory, name, rustix::fs::AtFlags::empty()).map_err(Into::into)
    }
    #[cfg(not(unix))]
    {
        std::fs::remove_file(path)
    }
}

/// Remove a file or directory tree relative to an opened parent directory.
/// Directory traversal never reconstructs a path, and leaf symlinks are
/// unlinked rather than followed.
#[cfg(unix)]
pub(crate) fn remove_private_tree_at(directory: &File, name: &OsStr) -> io::Result<()> {
    let child = match rustix::fs::openat(
        directory,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    ) {
        Ok(child) => child,
        Err(error) if error == rustix::io::Errno::NOTDIR => {
            return rustix::fs::unlinkat(directory, name, rustix::fs::AtFlags::empty())
                .map_err(Into::into)
        }
        Err(error) if error == rustix::io::Errno::LOOP => {
            return rustix::fs::unlinkat(directory, name, rustix::fs::AtFlags::empty())
                .map_err(Into::into)
        }
        Err(error) => return Err(error.into()),
    };
    let child: File = child.into();
    let entries = rustix::fs::Dir::read_from(&child)?;
    for entry in entries {
        let entry = entry?;
        let entry_name = entry.file_name();
        if entry_name.to_bytes() == b"." || entry_name.to_bytes() == b".." {
            continue;
        }
        let entry_name = std::ffi::OsStr::from_bytes(entry_name.to_bytes());
        remove_private_tree_at(&child, entry_name)?;
    }
    rustix::fs::unlinkat(directory, name, rustix::fs::AtFlags::REMOVEDIR).map_err(Into::into)
}

/// Open a private resumable file for reading and writing without following a
/// leaf symlink. The caller is responsible for bounding the path and contents.
pub(crate) fn open_private_read_write_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
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
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private input is not a regular file",
        ));
    }
    Ok(file)
}

/// Create a directory and report whether the path itself is a real directory.
///
/// `create_dir_all` follows an existing symlink, while storage roots must stay
/// inside the path supplied by the host. `symlink_metadata` lets callers reject
/// that case without changing the I/O errors from either operation.
pub(crate) fn create_directory_and_check(path: &Path) -> io::Result<bool> {
    reject_symlink(path)?;
    // If the directory is new, inspect the nearest existing ancestor before
    // `create_dir_all` fills in missing components. Otherwise a symlink in that
    // gap would be followed and the new storage would be created elsewhere.
    let mut current = path;
    loop {
        match fs::symlink_metadata(current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                if msime_path_trust::is_trusted_system_alias(current) {
                    break;
                }
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "storage path has a symbolic-link ancestor",
                ));
            }
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let Some(parent) = current.parent() else {
                    break;
                };
                if parent == current {
                    break;
                }
                current = parent;
            }
            Err(error) => return Err(error),
        }
    }
    fs::create_dir_all(path)?;
    Ok(fs::symlink_metadata(path)?.file_type().is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn private_file_open_rejects_a_symlinked_leaf() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.json");
        std::fs::write(&target, b"synthetic-private-data").unwrap();
        let linked = root.path().join("private.json");
        symlink(&target, &linked).unwrap();

        assert!(open_private_file(&linked).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn private_file_remove_rejects_a_symlinked_parent() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("anonymous-session.json");
        std::fs::write(&target, b"synthetic-outside").unwrap();
        let linked = root.path().join("linked");
        symlink(outside.path(), &linked).unwrap();

        assert!(remove_private_file(&linked.join("anonymous-session.json")).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic-outside");
    }

    #[cfg(unix)]
    #[test]
    fn private_file_remove_rejects_a_symlinked_ancestor() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let nested = outside.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        let target = nested.join("anonymous-session.json");
        std::fs::write(&target, b"synthetic-outside").unwrap();
        let linked = root.path().join("linked");
        symlink(outside.path(), &linked).unwrap();

        assert!(remove_private_file(&linked.join("nested/anonymous-session.json")).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic-outside");
    }

    // 模拟 Android 的 `/data`：祖先目录只有搜索权限、没有读权限，它下面的应用私有目录照样要能打开和写入。以 root 运行时权限检查不生效，这条测试只在普通用户下有区分度。
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn private_directory_opens_below_a_search_only_ancestor() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let ancestor = root.path().join("search-only");
        let private = ancestor.join("private");
        std::fs::create_dir_all(&private).unwrap();
        std::fs::set_permissions(&ancestor, std::fs::Permissions::from_mode(0o111)).unwrap();

        let written = open_private_directory(&private).and_then(|directory| {
            write_private_file_at(&directory, OsStr::new("session.json"), b"synthetic-session")
        });
        let read = open_private_file_in(&private.join("session.json"));
        std::fs::set_permissions(&ancestor, std::fs::Permissions::from_mode(0o700)).unwrap();

        written.unwrap();
        read.unwrap();
        assert_eq!(
            std::fs::read(private.join("session.json")).unwrap(),
            b"synthetic-session"
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_file_write_stays_in_an_open_directory_after_replacement() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("original");
        let outside = root.path().join("outside");
        std::fs::create_dir(&original).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let directory = open_private_directory(&original).unwrap();
        let moved = root.path().join("moved");
        std::fs::rename(&original, &moved).unwrap();
        symlink(&outside, &original).unwrap();

        write_private_file_at(&directory, OsStr::new("session.json"), b"synthetic-session")
            .unwrap();

        assert_eq!(
            std::fs::read(moved.join("session.json")).unwrap(),
            b"synthetic-session"
        );
        assert!(!outside.join("session.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn private_file_no_clobber_preserves_first_publication() {
        let root = tempfile::tempdir().unwrap();
        let directory = open_private_directory(root.path()).unwrap();
        let name = OsStr::new("anonymous-account.json");

        assert!(write_private_file_at_noclobber(&directory, name, b"first").unwrap());
        assert!(!write_private_file_at_noclobber(&directory, name, b"second").unwrap());
        assert_eq!(fs::read(root.path().join(name)).unwrap(), b"first");
    }

    #[cfg(unix)]
    #[test]
    fn private_file_no_clobber_stays_in_open_directory_after_replacement() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("original");
        let outside = root.path().join("outside");
        fs::create_dir(&original).unwrap();
        fs::create_dir(&outside).unwrap();
        let directory = open_private_directory(&original).unwrap();
        let moved = root.path().join("moved");
        fs::rename(&original, &moved).unwrap();
        symlink(&outside, &original).unwrap();

        assert!(write_private_file_at_noclobber(
            &directory,
            OsStr::new("anonymous-account.json"),
            b"synthetic-identity"
        )
        .unwrap());
        assert_eq!(
            fs::read(moved.join("anonymous-account.json")).unwrap(),
            b"synthetic-identity"
        );
        assert!(!outside.join("anonymous-account.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn staged_private_file_publishes_in_its_original_directory() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("original");
        let outside = root.path().join("outside");
        fs::create_dir(&original).unwrap();
        fs::create_dir(&outside).unwrap();
        let directory = open_private_directory(&original).unwrap();
        let mut staged = PrivateStagedFile::new(&directory).unwrap();
        staged.file_mut().write_all(b"synthetic-snapshot").unwrap();
        staged.file_mut().sync_all().unwrap();

        let moved = root.path().join("moved");
        fs::rename(&original, &moved).unwrap();
        symlink(&outside, &original).unwrap();
        staged.persist(OsStr::new("snapshot.ndjson")).unwrap();

        assert_eq!(
            fs::read(moved.join("snapshot.ndjson")).unwrap(),
            b"synthetic-snapshot"
        );
        assert!(!outside.join("snapshot.ndjson").exists());
    }

    #[cfg(unix)]
    #[test]
    fn private_tree_remove_stays_in_an_open_directory() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("original");
        let outside = root.path().join("outside");
        fs::create_dir(&original).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::create_dir(original.join("nested")).unwrap();
        fs::write(original.join("nested/file"), b"synthetic").unwrap();
        let directory = open_private_directory(&original).unwrap();
        let moved = root.path().join("moved");
        fs::rename(&original, &moved).unwrap();
        symlink(&outside, &original).unwrap();

        remove_private_tree_at(&directory, OsStr::new("nested")).unwrap();
        assert!(!moved.join("nested").exists());
        assert!(outside.exists());
    }

    #[cfg(unix)]
    #[test]
    fn private_read_write_open_rejects_a_symlinked_leaf() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.json");
        std::fs::write(&target, b"synthetic-private-data").unwrap();
        let linked = root.path().join("private.json");
        symlink(&target, &linked).unwrap();

        assert!(open_private_read_write_file(&linked).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic-private-data");
    }

    #[cfg(unix)]
    #[test]
    #[cfg_attr(
        target_os = "android",
        ignore = "Android 的 adb shell 域不允许建 FIFO（SELinux 拒绝 fifo_file create）"
    )]
    fn private_read_write_open_rejects_a_fifo() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("partial-download");
        assert!(std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success());

        assert!(open_private_read_write_file(&path).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_existing_and_missing_paths_below_a_symlinked_ancestor() {
        use msime_path_trust::untrusted_symlink as symlink;

        let outside = tempfile::tempdir().unwrap();
        let parent = tempfile::tempdir().unwrap();
        let real_root = parent.path().join("real");
        std::fs::create_dir(&real_root).unwrap();
        let linked = real_root.join("linked");
        symlink(outside.path(), &linked).unwrap();

        let existing = linked.join("existing.json");
        std::fs::write(outside.path().join("existing.json"), b"synthetic").unwrap();
        assert!(reject_symlink(&existing).is_err());

        let missing = linked.join("new-directory");
        assert!(create_directory_and_check(&missing).is_err());
        assert!(!outside.path().join("new-directory").exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn allows_missing_paths_below_macos_system_aliases() {
        let path = std::path::Path::new("/tmp")
            .join(format!("msime-storage-alias-{}", uuid::Uuid::new_v4()));
        let _ = std::fs::remove_dir_all(&path);
        assert!(create_directory_and_check(&path).unwrap());
        assert!(std::fs::symlink_metadata(&path).unwrap().is_dir());
        std::fs::remove_dir_all(path).unwrap();
    }
}
