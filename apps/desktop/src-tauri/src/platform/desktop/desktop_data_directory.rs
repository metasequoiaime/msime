use std::fs;
use std::io;
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
}
