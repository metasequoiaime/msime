#[cfg(unix)]
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
#[cfg(unix)]
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
    let absolute = parent.is_absolute();
    let mut directory = rustix::fs::open(
        if absolute {
            Path::new("/")
        } else {
            Path::new(".")
        },
        flags,
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
                directory = rustix::fs::openat(&directory, "..", flags, rustix::fs::Mode::empty())?;
                logical.pop();
            }
            std::path::Component::Normal(name) => {
                logical.push(name);
                directory =
                    match rustix::fs::openat(&directory, name, flags, rustix::fs::Mode::empty()) {
                        Ok(directory) => directory,
                        Err(error)
                            if (error == rustix::io::Errno::LOOP
                                || error == rustix::io::Errno::NOTDIR)
                                && msime_path_trust::is_trusted_system_alias(&logical) =>
                        {
                            rustix::fs::openat(
                                &directory,
                                name,
                                flags & !rustix::fs::OFlags::NOFOLLOW,
                                rustix::fs::Mode::empty(),
                            )?
                        }
                        Err(error) => return Err(error.into()),
                    };
            }
        }
    }
    Ok(directory.into())
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
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private input is not a regular file",
        ));
    }
    Ok(file)
}

#[cfg(unix)]
pub(crate) fn write_private_file_at(
    directory: &File,
    name: &OsStr,
    contents: &[u8],
) -> io::Result<()> {
    let mut temporary_name = OsString::from(".msime-private-");
    temporary_name.push(std::process::id().to_string());
    temporary_name.push("-");
    temporary_name.push(
        PRIVATE_FILE_COUNTER
            .fetch_add(1, Ordering::Relaxed)
            .to_string(),
    );
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

/// 在存储操作跟随已有的符号链接之前先拒绝它。每个应用的存储都会经过的系统链接，以 `msime-path-trust` 列出的为准。
pub(crate) fn reject_symlink(path: &Path) -> io::Result<()> {
    msime_path_trust::reject_symlinked_components(path)
}

/// 打开私有文档时复用文件锁模块的无跟随实现。
pub(crate) fn open_private_file(path: &Path) -> io::Result<File> {
    crate::file_lock::open_private_file(path)
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
