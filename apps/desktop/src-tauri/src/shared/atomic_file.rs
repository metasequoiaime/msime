use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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
        let descriptor = rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )?;
        let stat = rustix::fs::fstat(&descriptor)?;
        if !rustix::fs::FileType::from_raw_mode(stat.st_mode).is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "private input is not a regular file",
            ));
        }
        return Ok(descriptor.into());
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
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "private input is not a regular file",
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

/// Remove a private file relative to its opened parent directory. Opening the
/// parent with `O_NOFOLLOW` keeps a concurrent replacement of the final
/// directory component from redirecting cleanup through a symlink.
pub(crate) fn remove_private(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let name = path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "private file has no name")
        })?;
        let directory = rustix::fs::open(
            parent,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )?;
        return rustix::fs::unlinkat(&directory, name, rustix::fs::AtFlags::empty())
            .map_err(Into::into);
    }
    #[cfg(not(unix))]
    {
        std::fs::remove_file(path)
    }
}

/// Replace a file after fully writing and syncing a temporary sibling.
pub(crate) fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent: PathBuf = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    create_directory_and_check(&parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(&parent)?;
    temporary.write_all(contents)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map(|_| ())
        .map_err(|error| error.error)
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
