use std::fs;
use std::io;
use std::path::Path;

/// Reject an existing symbolic link before a storage operation follows it. The system links every app's storage passes through are the ones `msime-path-trust` names.
pub(crate) fn reject_symlink(path: &Path) -> io::Result<()> {
    msime_path_trust::reject_symlinked_components(path)
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
    fn rejects_existing_and_missing_paths_below_a_symlinked_ancestor() {
        use std::os::unix::fs::symlink;

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
