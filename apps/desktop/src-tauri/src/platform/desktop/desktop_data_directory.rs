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
    fs::copy(source, destination)?;
    fs::set_permissions(destination, metadata.permissions())?;
    fs::File::open(destination).and_then(|file| file.sync_all())
}

pub(crate) fn has_ownership_marker(directory: &Path, marker: &str) -> bool {
    fs::symlink_metadata(directory.join(marker))
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
}
