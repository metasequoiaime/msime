use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

/// 在存储操作跟随已有的符号链接之前先拒绝它。每个应用的存储都会经过的系统链接，以 `msime-path-trust` 列出的为准。
pub(crate) fn reject_symlink(path: &Path) -> io::Result<()> {
    msime_path_trust::reject_symlinked_components(path)
}

/// Open a private document without following a leaf symlink. Callers still
/// validate the opened handle's type, owner and mode; this flag closes the
/// check-then-open race between those metadata checks and the read itself.
pub(crate) fn open_private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    add_no_follow_flag(&mut options);
    options.open(path)
}

/// Open a private document for bounded read/write access without following a leaf symlink.
/// `create(true)` is intentional for resumable downloads; an existing link is still rejected.
pub(crate) fn open_private_file_rw(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    add_no_follow_flag(&mut options);
    options.open(path)
}

fn add_no_follow_flag(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
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
    fn private_file_rw_open_rejects_a_symlinked_leaf() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.partial");
        std::fs::write(&target, b"synthetic-target").unwrap();
        let linked = root.path().join("download.partial");
        symlink(&target, &linked).unwrap();

        assert!(open_private_file_rw(&linked).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic-target");
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
