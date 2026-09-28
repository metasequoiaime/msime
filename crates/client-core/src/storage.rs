use std::fs;
use std::io;
use std::path::Path;

/// Create a directory and report whether the path itself is a real directory.
///
/// `create_dir_all` follows an existing symlink, while storage roots must stay
/// inside the path supplied by the host. `symlink_metadata` lets callers reject
/// that case without changing the I/O errors from either operation.
pub(crate) fn create_directory_and_check(path: &Path) -> io::Result<bool> {
    fs::create_dir_all(path)?;
    Ok(fs::symlink_metadata(path)?.file_type().is_dir())
}
