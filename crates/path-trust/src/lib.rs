//! Storage paths are checked for symbolic links so that a planted link cannot redirect what the client writes. Some links on the way to every app's storage belong to the operating system, and a check that refuses those refuses all storage on that platform. This crate is the only place that names them; every crate that walks a storage path asks it.

use std::path::{Component, Path, PathBuf};

/// The system links a storage path may pass through, each with the only target it is trusted to have.
///
/// - macOS exposes `/tmp` and `/var` as links into `/private`; temporary and per-user directories live below them.
/// - Android 11 and later isolate app data: inside an app's mount namespace `/data/user/0` is a link to `/data/data`, so `Context.getFilesDir()` of the primary user passes through it. The link is not visible from `adb shell`.
const SYSTEM_ALIASES: &[(&str, &str)] = if cfg!(target_os = "macos") {
    &[("/tmp", "/private/tmp"), ("/var", "/private/var")]
} else if cfg!(target_os = "android") {
    &[("/data/user/0", "/data/data")]
} else {
    &[]
};

/// Whether `path` is one of this platform's system links and `target` (as read from it, possibly relative) resolves to the one place that link is trusted to point. The names alone are not an ownership guarantee, so the target is checked too.
pub fn trusted_system_alias_target(path: &Path, target: &Path) -> bool {
    let Some((_, expected)) = SYSTEM_ALIASES
        .iter()
        .find(|(alias, _)| path == Path::new(alias))
    else {
        return false;
    };
    let parent = path.parent().unwrap_or_else(|| Path::new("/"));
    normalize_lexical(&parent.join(target)) == Path::new(expected)
}

/// Whether `path` is a symbolic link this platform's system put there; see [`trusted_system_alias_target`].
pub fn is_trusted_system_alias(path: &Path) -> bool {
    !SYSTEM_ALIASES.is_empty()
        && std::fs::read_link(path).is_ok_and(|target| trusted_system_alias_target(path, &target))
}

/// Rejects a symbolic link at any level of `path`, the last level included, except at most one trusted system link above the last level.
///
/// Missing levels are fine: the caller is about to create them. Any other I/O error is returned as is.
pub fn reject_symlinked_components(path: &Path) -> std::io::Result<()> {
    let mut current = PathBuf::new();
    let mut saw_system_alias = false;
    let components: Vec<_> = path.components().collect();
    for (index, component) in components.iter().enumerate() {
        match component {
            Component::CurDir => continue,
            Component::Normal(_) => current.push(component),
            _ => {
                current.push(component);
                continue;
            }
        }
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let last = index + 1 == components.len();
                if last
                    || saw_system_alias
                    || !path.is_absolute()
                    || !is_trusted_system_alias(&current)
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "path contains a symbolic link",
                    ));
                }
                saw_system_alias = true;
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_require_the_exact_system_target() {
        let macos = cfg!(target_os = "macos");
        assert_eq!(
            trusted_system_alias_target(Path::new("/tmp"), Path::new("private/tmp")),
            macos
        );
        assert_eq!(
            trusted_system_alias_target(Path::new("/var"), Path::new("/private/var")),
            macos
        );
        assert!(!trusted_system_alias_target(
            Path::new("/tmp"),
            Path::new("/Users/synthetic/outside")
        ));
        assert!(!trusted_system_alias_target(
            Path::new("/tmp/work"),
            Path::new("/private/tmp/work")
        ));
        let android = cfg!(target_os = "android");
        assert_eq!(
            trusted_system_alias_target(Path::new("/data/user/0"), Path::new("/data/data")),
            android
        );
        assert!(!trusted_system_alias_target(
            Path::new("/data/user/0"),
            Path::new("/data/local/tmp")
        ));
        assert!(!trusted_system_alias_target(
            Path::new("/data/user/10"),
            Path::new("/data/data")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_links_the_system_did_not_make() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(root.path()).unwrap();
        let linked = root.join("linked");
        symlink(outside.path(), &linked).unwrap();
        assert!(reject_symlinked_components(&linked.join("missing/below")).is_err());
        assert!(reject_symlinked_components(&linked).is_err());
        assert!(reject_symlinked_components(&root.join("missing/below")).is_ok());
        std::fs::remove_file(linked).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn passes_through_a_macos_system_alias_once() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        assert!(Path::new("/tmp").is_symlink());
        assert!(reject_symlinked_components(&directory.path().join("missing")).is_ok());
        assert!(reject_symlinked_components(Path::new("/tmp")).is_err());
    }
}
