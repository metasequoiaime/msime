#[cfg(unix)]
use std::ffi::{CString, OsStr};
use std::fs;
use std::io;
#[cfg(unix)]
use std::os::fd::AsFd;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub(crate) fn remove_entry(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

pub(crate) fn validate_directory<E: Copy>(path: &Path, error: E) -> Result<PathBuf, E> {
    if !path.is_absolute() {
        return Err(error);
    }
    crate::shared::atomic_file::check_directory_ancestors(path).map_err(|_| error)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| error)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(error);
    }
    fs::canonicalize(path).map_err(|_| error)
}

/// Copy one data-tree entry while preserving its permissions. The caller may skip entries that
/// belong to a host-side lease or another platform-specific coordination file.
pub(crate) fn copy_entry<F>(source: &Path, destination: &Path, skip: &F) -> io::Result<()>
where
    F: Fn(&std::ffi::OsStr) -> bool,
{
    #[cfg(unix)]
    {
        copy_entry_unix(source, destination, skip)
    }
    #[cfg(not(unix))]
    {
        if let Some(parent) = source.parent() {
            crate::shared::atomic_file::check_directory_ancestors(parent)?;
        }
        if let Some(parent) = destination.parent() {
            crate::shared::atomic_file::check_directory_ancestors(parent)?;
        }
        let metadata = fs::symlink_metadata(source)?;
        if metadata.file_type().is_symlink() {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        if metadata.is_dir() {
            fs::create_dir(destination)?;
            for entry in fs::read_dir(source)? {
                let entry = entry?;
                if skip(&entry.file_name()) {
                    continue;
                }
                copy_entry(&entry.path(), &destination.join(entry.file_name()), skip)?;
            }
            fs::set_permissions(destination, metadata.permissions())?;
            return Ok(());
        }
        if !metadata.is_file() {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        let mut input = crate::shared::atomic_file::open_private(source)?;
        if !input.metadata()?.is_file() {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        io::copy(&mut input, &mut output)?;
        output.set_permissions(metadata.permissions())?;
        output.sync_all()
    }
}

#[cfg(unix)]
fn copy_entry_unix<F>(source: &Path, destination: &Path, skip: &F) -> io::Result<()>
where
    F: Fn(&OsStr) -> bool,
{
    let source_parent = source.parent().unwrap_or_else(|| Path::new("."));
    let source_name = source
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "source has no name"))?;
    let destination_parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let destination_name = destination
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "destination has no name"))?;
    let source_directory = crate::shared::atomic_file::open_private_directory(source_parent)?;
    let destination_directory =
        crate::shared::atomic_file::open_private_directory(destination_parent)?;
    copy_entry_from_directories(
        &source_directory,
        source_name,
        &destination_directory,
        destination_name,
        skip,
    )
}

#[cfg(unix)]
fn copy_entry_from_directories<F>(
    source_parent: &impl AsFd,
    source_name: &OsStr,
    destination_parent: &impl AsFd,
    destination_name: &OsStr,
    skip: &F,
) -> io::Result<()>
where
    F: Fn(&OsStr) -> bool,
{
    let source_name = CString::new(source_name.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "source name contains NUL"))?;
    let destination_name = CString::new(destination_name.as_bytes()).map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, "destination name contains NUL")
    })?;
    copy_entry_from_c_names(
        source_parent,
        &source_name,
        destination_parent,
        &destination_name,
        skip,
    )
}

#[cfg(unix)]
fn copy_entry_from_c_names<F>(
    source_parent: &impl AsFd,
    source_name: &std::ffi::CStr,
    destination_parent: &impl AsFd,
    destination_name: &std::ffi::CStr,
    skip: &F,
) -> io::Result<()>
where
    F: Fn(&OsStr) -> bool,
{
    let directory_flags = rustix::fs::OFlags::RDONLY
        | rustix::fs::OFlags::DIRECTORY
        | rustix::fs::OFlags::NOFOLLOW
        | rustix::fs::OFlags::CLOEXEC
        | rustix::fs::OFlags::NONBLOCK;
    let source_directory = match rustix::fs::openat(
        source_parent,
        source_name,
        directory_flags,
        rustix::fs::Mode::empty(),
    ) {
        Ok(directory) => Some(directory),
        Err(error) if error == rustix::io::Errno::NOTDIR => None,
        Err(error) if error == rustix::io::Errno::LOOP => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "source entry is a symbolic link",
            ));
        }
        Err(error) => return Err(error.into()),
    };
    if let Some(source_directory) = source_directory {
        let stat = rustix::fs::fstat(&source_directory)?;
        rustix::fs::mkdirat(
            destination_parent,
            destination_name,
            rustix::fs::Mode::from_raw_mode(0o700),
        )?;
        let destination_directory = rustix::fs::openat(
            destination_parent,
            destination_name,
            directory_flags,
            rustix::fs::Mode::empty(),
        )?;
        let mut entries = rustix::fs::Dir::new(source_directory)?;
        while let Some(entry) = entries.read() {
            let entry = entry?;
            let name = entry.file_name();
            if matches!(name.to_bytes(), b"." | b"..") {
                continue;
            }
            let name_os = OsStr::from_bytes(name.to_bytes());
            if skip(name_os) {
                continue;
            }
            copy_entry_from_c_names(&entries.fd()?, name, &destination_directory, name, skip)?;
        }
        rustix::fs::fchmod(
            &destination_directory,
            rustix::fs::Mode::from_raw_mode(stat.st_mode & 0o7777),
        )?;
        return Ok(());
    }

    let source_file = rustix::fs::openat(
        source_parent,
        source_name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| {
        if error == rustix::io::Errno::LOOP {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "source entry is a symbolic link",
            )
        } else {
            error.into()
        }
    })?;
    let source_file: fs::File = source_file.into();
    let stat = rustix::fs::fstat(&source_file)?;
    if !rustix::fs::FileType::from_raw_mode(stat.st_mode).is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "source entry is not a regular file",
        ));
    }
    let destination = rustix::fs::openat(
        destination_parent,
        destination_name,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )?;
    let mut destination: fs::File = destination.into();
    let mut source_file = source_file;
    io::copy(&mut source_file, &mut destination)?;
    rustix::fs::fchmod(
        &destination,
        rustix::fs::Mode::from_raw_mode(stat.st_mode & 0o7777),
    )?;
    destination.sync_all()
}

pub(crate) fn has_ownership_marker(directory: &Path, marker: &str) -> bool {
    fs::symlink_metadata(directory.join(marker))
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
}

pub(crate) fn write_data_marker(path: &Path) -> io::Result<()> {
    crate::shared::atomic_file::write(path, b"Metasequoia IME user data directory.\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn validate_directory_rejects_a_symlinked_ancestor() {
        use msime_path_trust::untrusted_symlink as symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let real = outside.path().join("Downloads");
        fs::create_dir(&real).unwrap();
        let linked = root.path().join("redirect");
        symlink(outside.path(), &linked).unwrap();

        assert!(validate_directory::<()>(&linked.join("Downloads"), ()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn data_marker_write_replaces_a_symlink_without_following_it() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside-marker");
        fs::write(&target, b"synthetic-outside").unwrap();
        let marker = root.path().join(".metasequoia-ime-data");
        symlink(&target, &marker).unwrap();

        write_data_marker(&marker).unwrap();

        assert_eq!(fs::read(&target).unwrap(), b"synthetic-outside");
        assert_eq!(
            fs::read(&marker).unwrap(),
            b"Metasequoia IME user data directory.\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn copy_entry_rejects_a_symlinked_destination() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        fs::write(&source, b"synthetic-source").unwrap();
        let target = outside.path().join("outside.txt");
        fs::write(&target, b"synthetic-outside").unwrap();
        let destination = root.path().join("destination.txt");
        symlink(&target, &destination).unwrap();

        assert!(copy_entry(&source, &destination, &|_| false).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"synthetic-outside");
    }

    #[cfg(unix)]
    #[test]
    fn copy_entry_copies_nested_directories_and_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let nested = source.join("nested");
        let destination = root.path().join("destination");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("state"), b"synthetic-state").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o750)).unwrap();
        fs::set_permissions(&nested, fs::Permissions::from_mode(0o711)).unwrap();
        fs::set_permissions(nested.join("state"), fs::Permissions::from_mode(0o640)).unwrap();
        fs::create_dir(&destination).unwrap();

        copy_entry(&source, &destination.join("copied"), &|_| false).unwrap();

        assert_eq!(
            fs::read(destination.join("copied/nested/state")).unwrap(),
            b"synthetic-state"
        );
        assert_eq!(
            fs::metadata(destination.join("copied"))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o750
        );
        assert_eq!(
            fs::metadata(destination.join("copied/nested"))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o711
        );
        assert_eq!(
            fs::metadata(destination.join("copied/nested/state"))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o640
        );
    }

    #[cfg(unix)]
    #[test]
    fn copy_entry_rejects_symlinked_and_special_sources() {
        use std::os::unix::fs::symlink;
        use std::process::Command;

        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        let symlink_source = root.path().join("symlink-source");
        let fifo_source = root.path().join("fifo-source");
        fs::write(&target, b"synthetic-target").unwrap();
        symlink(&target, &symlink_source).unwrap();
        assert!(copy_entry(
            &symlink_source,
            &root.path().join("symlink-destination"),
            &|_| false
        )
        .is_err());

        let status = Command::new("mkfifo").arg(&fifo_source).status().unwrap();
        if !status.success() {
            return;
        }
        assert!(
            copy_entry(&fifo_source, &root.path().join("fifo-destination"), &|_| {
                false
            })
            .is_err()
        );
        assert_eq!(fs::read(&target).unwrap(), b"synthetic-target");
    }

    #[cfg(unix)]
    #[test]
    fn copy_entry_stays_bound_to_an_open_destination_parent_when_replaced() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let source_parent = root.path().join("source");
        let destination_parent = root.path().join("destination");
        let moved = root.path().join("destination-moved");
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir(&source_parent).unwrap();
        fs::create_dir(&destination_parent).unwrap();
        let source = source_parent.join("state");
        fs::write(&source, b"synthetic-source").unwrap();
        let source_directory =
            crate::shared::atomic_file::open_private_directory(&source_parent).unwrap();
        let destination_directory =
            crate::shared::atomic_file::open_private_directory(&destination_parent).unwrap();

        fs::rename(&destination_parent, &moved).unwrap();
        symlink(outside.path(), &destination_parent).unwrap();

        copy_entry_from_directories(
            &source_directory,
            std::ffi::OsStr::new("state"),
            &destination_directory,
            std::ffi::OsStr::new("state"),
            &|_| false,
        )
        .unwrap();

        assert_eq!(fs::read(moved.join("state")).unwrap(), b"synthetic-source");
        assert!(!outside.path().join("state").exists());
    }
}
