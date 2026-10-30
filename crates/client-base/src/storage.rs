use std::cell::Cell;
use std::ffi::{OsStr, OsString};
#[cfg(any(not(unix), test))]
use std::fs::OpenOptions;
use std::fs::{self, File};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::Path;
use std::path::PathBuf;
#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
static PRIVATE_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 暂存目录的创建和清理都绑定到同一个父目录句柄。
pub struct StagingDirectory {
    pub path: PathBuf,
    created: Cell<bool>,
    #[cfg(unix)]
    parent: File,
    #[cfg(unix)]
    name: OsString,
}

impl StagingDirectory {
    pub fn new(path: PathBuf) -> io::Result<Self> {
        #[cfg(unix)]
        {
            let parent_path = path.parent().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "staging has no parent")
            })?;
            let name = path
                .file_name()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "staging has no name"))?
                .to_owned();
            let parent = open_private_directory(parent_path)?;
            Ok(Self {
                path,
                created: Cell::new(false),
                parent,
                name,
            })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {
                path,
                created: Cell::new(false),
            })
        }
    }

    #[cfg(unix)]
    pub fn new_at(path: PathBuf, parent: &File) -> io::Result<Self> {
        let name = path
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "staging has no name"))?
            .to_owned();
        Ok(Self {
            path,
            created: Cell::new(false),
            parent: parent.try_clone()?,
            name,
        })
    }

    pub fn create(&self) -> io::Result<()> {
        #[cfg(unix)]
        let result = rustix::fs::mkdirat(
            &self.parent,
            &self.name,
            rustix::fs::Mode::from_raw_mode(0o700),
        )
        .map_err(io::Error::from);
        #[cfg(not(unix))]
        let result = fs::create_dir(&self.path);
        if result.is_ok() {
            self.created.set(true);
        }
        result
    }

    #[cfg(unix)]
    pub fn open_directory(&self) -> io::Result<File> {
        open_private_directory_at(&self.parent, &self.name)
    }

    pub fn create_child(&self, name: &OsStr) -> io::Result<()> {
        #[cfg(unix)]
        {
            let directory = self.open_directory()?;
            rustix::fs::mkdirat(&directory, name, rustix::fs::Mode::from_raw_mode(0o700))
                .map_err(io::Error::from)
        }
        #[cfg(not(unix))]
        {
            fs::create_dir(self.path.join(name))
        }
    }

    #[cfg(unix)]
    pub fn open_child(&self, name: &OsStr) -> io::Result<File> {
        open_private_directory_at(&self.open_directory()?, name)
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if !self.created.get() {
            return;
        }
        #[cfg(unix)]
        let _ = remove_private_tree_at(&self.parent, &self.name);
        #[cfg(not(unix))]
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Apple 平台不逐级打开：iOS 沙盒不允许应用以读方式打开容器的祖先目录（`/private/var/mobile/Containers/Shared/AppGroup` 这些），逐级 `RDONLY` 在走到 App Group 容器之前就 EPERM，统计、账号、剪贴板等私有文件读写全部失败，而 Darwin 没有 `O_PATH`。`O_NOFOLLOW_ANY` 让内核在一次解析里拒绝路径上任何一级符号链接，与逐级 `NOFOLLOW` 一样不留检查与打开之间的窗口，解析只需要祖先的搜索权限。路径经过 `msime-path-trust` 列出的系统别名（`/var`、`/tmp`）时先换成它唯一受信任的目标再打开。
#[cfg(target_vendor = "apple")]
pub fn open_private_directory(parent: &Path) -> io::Result<File> {
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
        result => return Ok(result?.into()),
    }
    let mut resolved = PathBuf::new();
    for component in parent.components() {
        resolved.push(component);
        if msime_path_trust::is_trusted_system_alias(&resolved) {
            let target = fs::read_link(&resolved)?;
            resolved = resolved
                .parent()
                .unwrap_or_else(|| Path::new("/"))
                .join(target);
        }
    }
    if resolved == parent {
        return Err(rustix::io::Errno::LOOP.into());
    }
    Ok(rustix::fs::open(&resolved, flags, rustix::fs::Mode::empty())?.into())
}

#[cfg(all(unix, not(target_vendor = "apple")))]
pub fn open_private_directory(parent: &Path) -> io::Result<File> {
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
pub fn open_private_file_at(directory: &File, name: &OsStr) -> io::Result<File> {
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
    // 只读打开：多链接文件在目录只有 root 或当前用户能写时也放行，见 `msime_path_trust::multi_link_is_trusted`（#6386）。
    if !metadata.is_file()
        || !(crate::file_lock::has_single_link(&file)?
            || msime_path_trust::multi_link_in_directory_is_trusted(directory)?)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private input is not a regular file with a trusted link count",
        ));
    }
    Ok(file)
}

/// Open a real child directory without following a symlink. The returned
/// descriptor remains bound to that directory if its parent entry is later
/// replaced.
#[cfg(unix)]
pub fn open_private_directory_at(directory: &File, name: &OsStr) -> io::Result<File> {
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

#[cfg(unix)]
pub fn clone_private_directory(directory: &File) -> io::Result<File> {
    directory.try_clone()
}

#[cfg(unix)]
pub fn private_directory_metadata(directory: &File) -> io::Result<fs::Metadata> {
    directory.metadata()
}

#[cfg(unix)]
pub fn open_private_lock_file_at(directory: &File, name: &OsStr) -> io::Result<File> {
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
    Ok(descriptor.into())
}

#[cfg(unix)]
pub fn read_private_directory(directory: &File) -> io::Result<Vec<OsString>> {
    let entries = rustix::fs::Dir::read_from(directory)?;
    entries
        .map(|entry| {
            let entry = entry?;
            let name = entry.file_name();
            if name.to_bytes() == b"." || name.to_bytes() == b".." {
                return Ok(None);
            }
            Ok(Some(OsString::from_vec(name.to_bytes().to_vec())))
        })
        .filter_map(|entry| entry.transpose())
        .collect()
}

#[cfg(unix)]
pub fn create_private_directory_at(parent: &File, name: &OsStr) -> io::Result<()> {
    rustix::fs::mkdirat(parent, name, rustix::fs::Mode::from_raw_mode(0o700)).map_err(Into::into)
}

#[cfg(unix)]
pub fn remove_private_directory_at(parent: &File, name: &OsStr) -> io::Result<()> {
    rustix::fs::unlinkat(parent, name, rustix::fs::AtFlags::REMOVEDIR).map_err(Into::into)
}

#[cfg(unix)]
pub fn rename_private_entry(
    from: &File,
    from_name: &OsStr,
    to: &File,
    to_name: &OsStr,
) -> io::Result<()> {
    rustix::fs::renameat(from, from_name, to, to_name).map_err(Into::into)
}

/// 非 Unix 平台上的「目录句柄」：没有 openat 这组调用，只记下已确认是真实目录（不是符号链接）的路径，`open_private_file_at` 再在它下面按名字打开。和 Unix 版的接口一致，调用方不必分平台。
#[cfg(not(unix))]
pub struct PrivateDirectory(PathBuf);

#[cfg(not(unix))]
pub fn open_private_directory(parent: &Path) -> io::Result<PrivateDirectory> {
    if !fs::symlink_metadata(parent)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private directory is not a real directory",
        ));
    }
    Ok(PrivateDirectory(parent.to_path_buf()))
}

#[cfg(not(unix))]
pub fn clone_private_directory(directory: &PrivateDirectory) -> io::Result<PrivateDirectory> {
    Ok(PrivateDirectory(directory.0.clone()))
}

#[cfg(not(unix))]
pub fn open_private_directory_at(
    directory: &PrivateDirectory,
    name: &OsStr,
) -> io::Result<PrivateDirectory> {
    let path = directory.0.join(name);
    if !fs::symlink_metadata(&path)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private directory is not a real directory",
        ));
    }
    Ok(PrivateDirectory(path))
}

#[cfg(not(unix))]
pub fn open_private_lock_file_at(directory: &PrivateDirectory, name: &OsStr) -> io::Result<File> {
    crate::file_lock::open_private_lock_file(directory.0.join(name))
}

#[cfg(not(unix))]
pub fn private_directory_metadata(directory: &PrivateDirectory) -> io::Result<fs::Metadata> {
    fs::metadata(&directory.0)
}

#[cfg(not(unix))]
pub fn read_private_directory(directory: &PrivateDirectory) -> io::Result<Vec<OsString>> {
    fs::read_dir(&directory.0)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect()
}

#[cfg(not(unix))]
pub fn create_private_directory_at(parent: &PrivateDirectory, name: &OsStr) -> io::Result<()> {
    fs::create_dir(parent.0.join(name))
}

#[cfg(not(unix))]
pub fn remove_private_directory_at(parent: &PrivateDirectory, name: &OsStr) -> io::Result<()> {
    fs::remove_dir(parent.0.join(name))
}

#[cfg(not(unix))]
pub fn rename_private_entry(
    from: &PrivateDirectory,
    from_name: &OsStr,
    to: &PrivateDirectory,
    to_name: &OsStr,
) -> io::Result<()> {
    fs::rename(from.0.join(from_name), to.0.join(to_name))
}

#[cfg(not(unix))]
pub fn open_private_file_at(directory: &PrivateDirectory, name: &OsStr) -> io::Result<File> {
    let path = directory.0.join(name);
    if !fs::symlink_metadata(&path)?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private input is not a regular file",
        ));
    }
    open_private_file(&path)
}

#[cfg(not(unix))]
pub fn replace_private_file_at(
    directory: &PrivateDirectory,
    name: &OsStr,
    contents: &[u8],
    permissions: Option<&fs::Permissions>,
) -> io::Result<()> {
    replace_private_file_with_permissions(&directory.0.join(name), contents, permissions)
}

#[cfg(not(unix))]
pub fn write_private_file_at(
    directory: &PrivateDirectory,
    name: &OsStr,
    contents: &[u8],
) -> io::Result<()> {
    replace_private_file_at(directory, name, contents, None)
}

#[cfg(unix)]
pub fn write_private_file_at(directory: &File, name: &OsStr, contents: &[u8]) -> io::Result<()> {
    write_private_file_at_with_permissions(directory, name, contents, None)
}

#[cfg(unix)]
pub fn write_private_file_at_with_permissions(
    directory: &File,
    name: &OsStr,
    contents: &[u8],
    permissions: Option<&fs::Permissions>,
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
    let result = file.write_all(contents).and_then(|()| {
        if let Some(permissions) = permissions {
            file.set_permissions(permissions.clone())?;
        }
        file.sync_all()
    });
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
pub fn replace_private_file_at(
    directory: &File,
    name: &OsStr,
    contents: &[u8],
    permissions: Option<&fs::Permissions>,
) -> io::Result<()> {
    write_private_file_at_with_permissions(directory, name, contents, permissions)
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
pub fn write_private_file_at_noclobber(
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
pub fn write_private_file_at_with<T, E, F>(
    directory: &File,
    name: &OsStr,
    writer: F,
) -> Result<(T, u64), E>
where
    E: From<io::Error>,
    F: FnOnce(File) -> Result<(File, T), E>,
{
    write_private_file_at_with_mode(directory, name, writer, false)
}

/// Stream a private file into a directory-bound temporary file and publish it
/// with an atomic no-replace operation. The callback returns the finished file
/// so callers can use writers that consume their output handle.
#[cfg(unix)]
pub fn write_private_file_at_noclobber_with<T, E, F>(
    directory: &File,
    name: &OsStr,
    writer: F,
) -> Result<(T, u64), E>
where
    E: From<io::Error>,
    F: FnOnce(File) -> Result<(File, T), E>,
{
    write_private_file_at_with_mode(directory, name, writer, true)
}

#[cfg(unix)]
fn write_private_file_at_with_mode<T, E, F>(
    directory: &File,
    name: &OsStr,
    writer: F,
    no_replace: bool,
) -> Result<(T, u64), E>
where
    E: From<io::Error>,
    F: FnOnce(File) -> Result<(File, T), E>,
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
    )
    .map_err(|error| E::from(io::Error::from(error)))?;
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
            return Err(E::from(error));
        }
    };
    drop(file);
    if !no_replace {
        if let Err(error) = rustix::fs::renameat(directory, &temporary_name, directory, name) {
            let _ = rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty());
            return Err(E::from(io::Error::from(error)));
        }
        return Ok((value, bytes));
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    match rustix::fs::renameat_with(
        directory,
        &temporary_name,
        directory,
        name,
        rustix::fs::RenameFlags::NOREPLACE,
    ) {
        Ok(()) => return Ok((value, bytes)),
        Err(rustix::io::Errno::EXIST) => {
            let _ = rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty());
            return Err(E::from(io::Error::from(io::ErrorKind::AlreadyExists)));
        }
        Err(rustix::io::Errno::INVAL | rustix::io::Errno::NOSYS) => {}
        Err(error) => {
            let _ = rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty());
            return Err(E::from(io::Error::from(error)));
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
            rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty())
                .map_err(|error| E::from(io::Error::from(error)))?;
            Ok((value, bytes))
        }
        Err(error) if error == rustix::io::Errno::EXIST => {
            let _ = rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty());
            Err(E::from(io::Error::from(io::ErrorKind::AlreadyExists)))
        }
        Err(error) => {
            let _ = rustix::fs::unlinkat(directory, &temporary_name, rustix::fs::AtFlags::empty());
            Err(E::from(io::Error::from(error)))
        }
    }
}

/// A private temporary file whose creation, publication and cleanup all use
/// the same opened directory. This lets large files be streamed before a
/// caller takes its state lock.
#[cfg(unix)]
pub struct PrivateStagedFile {
    directory: File,
    name: OsString,
    file: Option<File>,
}

#[cfg(unix)]
impl PrivateStagedFile {
    pub fn new(directory: &File) -> io::Result<Self> {
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

    pub fn file_mut(&mut self) -> &mut File {
        self.file.as_mut().expect("staged file was published")
    }

    pub fn directory(&self) -> &File {
        &self.directory
    }

    pub fn persist(mut self, name: &OsStr) -> io::Result<()> {
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
pub fn reject_symlink(path: &Path) -> io::Result<()> {
    msime_path_trust::reject_symlinked_components(path)
}

/// 打开私有文档时复用文件锁模块的无跟随实现。
pub fn open_private_file(path: &Path) -> io::Result<File> {
    crate::file_lock::open_private_file(path)
}

/// 打开私有目录中的普通文件；Unix 绑定父目录句柄，其他平台沿用私有文件打开策略。
pub fn open_private_file_in(path: &Path) -> io::Result<File> {
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

/// 把写好的临时文件发布为 `path`，替换已有的文件。和 `NamedTempFile::persist` 一样，但在 Windows 上替换失败于「目标正被别的进程打开」这类会自己消失的错误时，短暂等待后再试几次。
///
/// Windows 的 `persist` 是 `MoveFileExW(MOVEFILE_REPLACE_EXISTING)`：只要有进程打开着目标文件（杀毒软件、备份与同步工具、搜索索引常常为了读一下就打开它几毫秒），替换就返回访问拒绝 5 或共享冲突 32，共享标志里有没有 `FILE_SHARE_DELETE` 都一样。这次保存本来什么问题都没有，却直接报错，设置页随即重读旧值，用户刚做的修改就丢了。只对访问拒绝、共享冲突和锁冲突（33）重试，总等待约 1 秒（见 `REPLACE_RETRY_DELAYS_MS`）；别的错误照旧立刻返回。
///
/// 重试期间临时文件一直留在原处，目标要么还是旧内容、要么已经整份换成新内容，不会出现写了一半的文件；最后仍失败时临时文件随 `PersistError` 交回调用方，丢弃它就删掉了。调用方在存储锁里调用时，等待也发生在锁里，同一把锁的读者最多多等这一秒，不会读到别的写入方插进来的内容。Unix 上的 `rename(2)` 不受打开的句柄影响，直接调用 `persist`，行为不变。
pub fn persist_replacing<F>(
    temporary: tempfile::NamedTempFile<F>,
    path: impl AsRef<Path>,
) -> Result<F, tempfile::PersistError<F>> {
    #[cfg(windows)]
    {
        let path = path.as_ref();
        retry_transient_replace(
            temporary,
            |temporary| {
                temporary
                    .persist(path)
                    .map_err(|error| (error.error, error.file))
            },
            std::thread::sleep,
        )
        .map_err(|(error, file)| tempfile::PersistError { error, file })
    }
    #[cfg(not(windows))]
    {
        temporary.persist(path)
    }
}

/// Windows 替换目标失败后每次重试前等待的毫秒数，合计 950ms。一般的占用只有几毫秒，前几次就能过；第一次失败后最多再试七次。
#[cfg(any(windows, test))]
const REPLACE_RETRY_DELAYS_MS: [u64; 7] = [10, 20, 40, 80, 160, 320, 320];

/// 替换失败是不是目标暂时被别人占着：Windows 的 `ERROR_ACCESS_DENIED`（5）、`ERROR_SHARING_VIOLATION`（32）和 `ERROR_LOCK_VIOLATION`（33）。访问拒绝也可能是真的没有权限，那样重试只是晚约 1 秒报出同一个错误。这些是 Windows 的错误码，只在 Windows 上调用；测试在各平台上用同样的数字构造错误来驱动重试循环。
#[cfg(any(windows, test))]
fn is_transient_replace_error(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(5 | 32 | 33))
}

/// 按 `REPLACE_RETRY_DELAYS_MS` 重试替换。`replace` 失败时把错误和要重试的状态（临时文件）一起交回来，下一次接着用；遇到不是暂时占用的错误或次数用完，原样返回最后一次的错误和状态。等待经 `sleep` 进行，测试据此不必真的睡。
#[cfg(any(windows, test))]
fn retry_transient_replace<S, T>(
    mut state: S,
    mut replace: impl FnMut(S) -> Result<T, (io::Error, S)>,
    mut sleep: impl FnMut(std::time::Duration),
) -> Result<T, (io::Error, S)> {
    for delay in REPLACE_RETRY_DELAYS_MS {
        match replace(state) {
            Err((error, returned)) if is_transient_replace_error(&error) => {
                sleep(std::time::Duration::from_millis(delay));
                state = returned;
            }
            result => return result,
        }
    }
    replace(state)
}

/// 替换私有文件，并让 Unix 的读取和发布都绑定到已打开的父目录句柄。
pub fn replace_private_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    replace_private_file_with_permissions(path, contents, None)
}

pub fn replace_private_file_with_permissions(
    path: &Path,
    contents: &[u8],
    permissions: Option<&fs::Permissions>,
) -> io::Result<()> {
    #[cfg(unix)]
    {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let name = path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "private file has no name")
        })?;
        let directory = open_private_directory(parent)?;
        write_private_file_at_with_permissions(&directory, name, contents, permissions)
    }
    #[cfg(not(unix))]
    {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        reject_symlink(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(contents)?;
        if let Some(permissions) = permissions {
            temporary.as_file().set_permissions(permissions.clone())?;
        }
        temporary.as_file().sync_all()?;
        persist_replacing(temporary, path)
            .map(|_| ())
            .map_err(|error| error.error)
    }
}

/// Remove a private file relative to an opened parent directory, so a
/// concurrent replacement of the directory cannot redirect cleanup through a
/// symlink.
pub fn remove_private_file(path: &Path) -> io::Result<()> {
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

#[cfg(unix)]
pub fn remove_private_file_at(directory: &File, name: &OsStr) -> io::Result<()> {
    rustix::fs::unlinkat(directory, name, rustix::fs::AtFlags::empty()).map_err(Into::into)
}

#[cfg(not(unix))]
pub fn remove_private_file_at(directory: &PrivateDirectory, name: &OsStr) -> io::Result<()> {
    fs::remove_file(directory.0.join(name))
}

/// Remove a file or directory tree relative to an opened parent directory.
/// Directory traversal never reconstructs a path, and leaf symlinks are
/// unlinked rather than followed.
#[cfg(unix)]
pub fn remove_private_tree_at(directory: &File, name: &OsStr) -> io::Result<()> {
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
#[cfg(any(not(unix), test))]
pub fn open_private_read_write_file(path: &Path) -> io::Result<File> {
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
pub fn create_directory_and_check(path: &Path) -> io::Result<bool> {
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
    #[cfg(unix)]
    use super::*;

    // 重试循环在各平台上都能测：用 Windows 的错误码构造失败，等待记下来而不真的睡。
    #[test]
    fn transient_replace_failures_are_retried_until_the_replace_succeeds() {
        let mut attempts = 0;
        let mut waits = Vec::new();
        let result = super::retry_transient_replace(
            (),
            |()| {
                attempts += 1;
                match attempts {
                    1 => Err((std::io::Error::from_raw_os_error(32), ())),
                    2 => Err((std::io::Error::from_raw_os_error(5), ())),
                    3 => Err((std::io::Error::from_raw_os_error(33), ())),
                    _ => Ok("published"),
                }
            },
            |delay| waits.push(delay.as_millis()),
        );
        assert_eq!(result.unwrap(), "published");
        assert_eq!(attempts, 4);
        assert_eq!(waits, [10, 20, 40]);
    }

    #[test]
    fn replace_failures_that_do_not_go_away_fail_at_once() {
        for error in [
            // ERROR_FILE_NOT_FOUND、ERROR_DISK_FULL，以及不带系统错误码的错误。
            std::io::Error::from_raw_os_error(2),
            std::io::Error::from_raw_os_error(112),
            std::io::Error::other("synthetic failure"),
        ] {
            let expected = error.raw_os_error();
            let mut error = Some(error);
            let mut attempts = 0;
            let mut waits = 0;
            let result: Result<(), _> = super::retry_transient_replace(
                (),
                |()| {
                    attempts += 1;
                    Err((error.take().unwrap(), ()))
                },
                |_| waits += 1,
            );
            assert_eq!(result.unwrap_err().0.raw_os_error(), expected);
            assert_eq!(attempts, 1);
            assert_eq!(waits, 0);
        }
    }

    #[test]
    fn replace_gives_up_after_its_budget_and_hands_back_the_staged_file() {
        let root = tempfile::tempdir().unwrap();
        let staged = tempfile::NamedTempFile::new_in(root.path()).unwrap();
        let staged_path = staged.path().to_path_buf();
        let mut attempts = 0;
        let mut waited = 0;
        let result: Result<(), _> = super::retry_transient_replace(
            staged,
            |staged| {
                attempts += 1;
                // 每次尝试时临时文件都还在，下一次替换才有东西可发布。
                assert!(staged_path.exists());
                Err((std::io::Error::from_raw_os_error(32), staged))
            },
            |delay| waited += delay.as_millis(),
        );
        let (error, staged) = result.unwrap_err();
        assert_eq!(error.raw_os_error(), Some(32));
        assert_eq!(attempts, 8);
        assert_eq!(waited, 950);
        assert!(staged_path.exists());
        drop(staged);
        assert!(!staged_path.exists());
    }

    #[test]
    fn persist_replacing_publishes_the_staged_contents() {
        use std::io::Write as _;
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("synthetic.json");
        std::fs::write(&target, b"{\"revision\":1}").unwrap();
        let mut staged = tempfile::NamedTempFile::new_in(root.path()).unwrap();
        staged.write_all(b"{\"revision\":2}").unwrap();
        super::persist_replacing(staged, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"{\"revision\":2}");
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }

    /// 照杀毒软件、备份工具的方式打开目标：允许读写共享，不允许删除共享。
    #[cfg(windows)]
    fn hold_open_without_delete_sharing(path: &std::path::Path) -> std::fs::File {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x1;
        const FILE_SHARE_WRITE: u32 = 0x2;
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(path)
            .unwrap()
    }

    #[cfg(windows)]
    fn staged_with(directory: &std::path::Path, contents: &[u8]) -> tempfile::NamedTempFile {
        use std::io::Write as _;
        let mut staged = tempfile::NamedTempFile::new_in(directory).unwrap();
        staged.write_all(contents).unwrap();
        staged.as_file().sync_all().unwrap();
        staged
    }

    #[cfg(windows)]
    #[test]
    fn persist_replacing_waits_for_a_reader_that_briefly_holds_the_target_open() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("preferences.json");
        std::fs::write(&target, b"old").unwrap();

        // 对照：目标被打开着时，不重试的 `persist` 确实失败，原内容不变。下面的成功因此只能归功于重试。
        let reader = hold_open_without_delete_sharing(&target);
        let refused = staged_with(root.path(), b"new")
            .persist(&target)
            .unwrap_err();
        assert!(
            matches!(refused.error.raw_os_error(), Some(5 | 32)),
            "{:?}",
            refused.error
        );
        drop(refused);
        assert_eq!(std::fs::read(&target).unwrap(), b"old");

        let releasing = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            drop(reader);
        });
        super::persist_replacing(staged_with(root.path(), b"new"), &target).unwrap();
        releasing.join().unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        // 没有遗留的临时文件。
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn persist_replacing_gives_up_on_a_target_held_open_past_its_budget() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("preferences.json");
        std::fs::write(&target, b"old").unwrap();
        let reader = hold_open_without_delete_sharing(&target);
        let started = std::time::Instant::now();
        let refused =
            super::persist_replacing(staged_with(root.path(), b"new"), &target).unwrap_err();
        assert!(started.elapsed() >= std::time::Duration::from_millis(900));
        assert!(matches!(refused.error.raw_os_error(), Some(5 | 32)));
        drop(refused);
        drop(reader);
        assert_eq!(std::fs::read(&target).unwrap(), b"old");
        // 放弃后丢弃交回的临时文件，目录里只剩原来的目标。
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_staging_create_preserves_existing_directory() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(".staging-synthetic");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("sentinel"), b"synthetic-data").unwrap();

        let staging = super::StagingDirectory::new(path.clone()).unwrap();
        assert_eq!(
            staging.create().unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        drop(staging);

        assert_eq!(
            std::fs::read(path.join("sentinel")).unwrap(),
            b"synthetic-data"
        );
    }

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
        use msime_path_trust::untrusted_symlink as symlink;

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
        use msime_path_trust::untrusted_symlink as symlink;

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

    // 模拟 Android 的 `/data` 和 iOS 沙盒里 App Group 容器的祖先：祖先目录只有搜索权限、没有读权限，它下面的应用私有目录照样要能打开和写入。以 root 运行时权限检查不生效，这条测试只在普通用户下有区分度。
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
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
    #[cfg(unix)]
    #[test]
    fn private_file_publish_stays_in_an_open_directory_after_replacement() {
        use std::os::unix::fs::symlink;

        let parent = tempfile::tempdir().unwrap();
        let original = parent.path().join("state");
        let outside = parent.path().join("outside");
        fs::create_dir(&original).unwrap();
        fs::create_dir(&outside).unwrap();
        let directory = open_private_directory(&original).unwrap();
        let moved = parent.path().join("moved");
        fs::rename(&original, &moved).unwrap();
        symlink(&outside, &original).unwrap();

        write_private_file_at_with_permissions(
            &directory,
            OsStr::new("runtime-options.json"),
            b"synthetic-options",
            None,
        )
        .unwrap();

        assert_eq!(
            fs::read(moved.join("runtime-options.json")).unwrap(),
            b"synthetic-options"
        );
        assert!(!outside.join("runtime-options.json").exists());
    }
}
