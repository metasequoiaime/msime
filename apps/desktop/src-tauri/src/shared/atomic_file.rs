use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn is_system_path_alias(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        return std::fs::read_link(path)
            .ok()
            .is_some_and(|target| trusted_system_alias_target(path, &target));
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        false
    }
}

#[cfg(target_os = "macos")]
fn trusted_system_alias_target(path: &Path, target: &Path) -> bool {
    let expected = match path {
        path if path == Path::new("/tmp") => Path::new("/private/tmp"),
        path if path == Path::new("/var") => Path::new("/private/var"),
        _ => return false,
    };
    let parent = path.parent().unwrap_or_else(|| Path::new("/"));
    std::fs::canonicalize(parent.join(target))
        .ok()
        .is_some_and(|resolved| resolved == expected)
}

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
                if !is_system_path_alias(&current) {
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

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_aliases_require_the_private_system_target() {
        assert!(trusted_system_alias_target(
            Path::new("/tmp"),
            Path::new("private/tmp")
        ));
        assert!(trusted_system_alias_target(
            Path::new("/var"),
            Path::new("/private/var")
        ));
        assert!(!trusted_system_alias_target(
            Path::new("/tmp"),
            Path::new("/Users/synthetic/outside")
        ));
        assert!(!trusted_system_alias_target(
            Path::new("/var"),
            Path::new("private/tmp")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_ancestor_before_creating_the_parent() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let linked = root.path().join("linked");
        symlink(outside.path(), &linked).unwrap();
        let path = linked.join("missing").join("state.json");

        assert!(write(&path, b"synthetic").is_err());
        assert!(!outside.path().join("missing").exists());
    }
}
