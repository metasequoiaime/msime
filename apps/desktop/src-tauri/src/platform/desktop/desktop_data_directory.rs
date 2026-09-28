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
