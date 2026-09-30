//! Copying a picked skin folder into a host's skin root.

use std::path::Path;

/// Copy a skin folder the user picked into `root`, under the folder's own name, and return that name.
///
/// This is how a skin arrives on a host whose skin folder sits inside the application sandbox: the iOS App Group, reached from both the Tauri shell and the native settings app through the C ABI. The name and the manifest are checked first, by the rule the catalog lists skins by, so nothing is copied that the page would then report as unusable. An existing skin of that name is replaced whole rather than merged, since a half-overwritten skin draws a stylesheet from one version with assets from another. Symbolic links are left behind: the catalog refuses anything that resolves outside the skin folder anyway.
pub fn import(source: &Path, root: &Path) -> Result<String, &'static str> {
    let source_metadata = std::fs::symlink_metadata(source).map_err(|_| "skin_manifest")?;
    if !source_metadata.is_dir() || source_metadata.file_type().is_symlink() {
        return Err("skin_manifest");
    }
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| super::catalog::is_external_id(name))
        .ok_or("skin_name")?
        .to_owned();
    let manifest = source.join("skin.toml");
    let manifest_metadata = std::fs::symlink_metadata(&manifest).map_err(|_| "skin_manifest")?;
    if !manifest_metadata.is_file() || manifest_metadata.file_type().is_symlink() {
        return Err("skin_manifest");
    }
    // A linked root would publish the import into an unrelated directory. Check before creating
    // it because `create_dir_all` follows a final-component symlink.
    if !crate::storage::create_directory_and_check(root).map_err(|_| "storage")? {
        return Err("storage");
    }
    // The leading dot keeps both helpers out of the catalog, which lists only names starting with a letter or digit.
    let staging = root.join(format!(".import-{name}"));
    let replaced = root.join(format!(".replaced-{name}"));
    for leftover in [&staging, &replaced] {
        if leftover.exists() {
            std::fs::remove_dir_all(leftover).map_err(|_| "storage")?;
        }
    }
    let mut budget = ImportBudget::default();
    if copy_tree(source, &staging, 0, &mut budget).is_err() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("storage");
    }
    replace_directory(&staging, &root.join(&name), &replaced)?;
    Ok(name)
}

/// Swap a fully written `staging` directory in as `target`, replacing any existing `target` whole. The previous directory is first moved aside to `backup` and restored if the swap fails, so a failure never leaves a half-replaced skin; `staging` is removed on failure and `backup` after success. Every error is `storage`.
pub(crate) fn replace_directory(
    staging: &Path,
    target: &Path,
    backup: &Path,
) -> Result<(), &'static str> {
    let had_previous = target.exists();
    if had_previous && std::fs::rename(target, backup).is_err() {
        let _ = std::fs::remove_dir_all(staging);
        return Err("storage");
    }
    if std::fs::rename(staging, target).is_err() {
        if had_previous {
            let _ = std::fs::rename(backup, target);
        }
        let _ = std::fs::remove_dir_all(staging);
        return Err("storage");
    }
    if had_previous {
        let _ = std::fs::remove_dir_all(backup);
    }
    Ok(())
}

/// Bounds on one import, so a mistakenly picked folder (a photo library, a whole drive) fails instead of filling the device.
struct ImportBudget {
    entries: usize,
    bytes: u64,
}

impl Default for ImportBudget {
    fn default() -> Self {
        Self {
            entries: 4096,
            bytes: 256 * 1024 * 1024,
        }
    }
}

const MAX_IMPORT_DEPTH: usize = 16;

fn copy_tree(
    source: &Path,
    destination: &Path,
    depth: usize,
    budget: &mut ImportBudget,
) -> std::io::Result<()> {
    if depth > MAX_IMPORT_DEPTH {
        return Err(std::io::Error::other("skin folder too deep"));
    }
    std::fs::create_dir(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        budget.entries = budget
            .entries
            .checked_sub(1)
            .ok_or_else(|| std::io::Error::other("skin folder too large"))?;
        let kind = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target, depth + 1, budget)?;
        } else if kind.is_file() {
            budget.bytes = budget
                .bytes
                .checked_sub(entry.metadata()?.len())
                .ok_or_else(|| std::io::Error::other("skin folder too large"))?;
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn picked(parent: &Path, name: &str) -> PathBuf {
        let skin = parent.join(name);
        std::fs::create_dir_all(skin.join("images")).unwrap();
        std::fs::write(skin.join("skin.toml"), b"id = 'synthetic'").unwrap();
        std::fs::write(skin.join("images").join("bg.png"), b"new").unwrap();
        skin
    }

    #[test]
    fn import_copies_the_picked_folder_under_its_own_name() {
        let files = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let root = state.path().join("skins");
        let source = picked(files.path(), "sakura");
        assert_eq!(import(&source, &root).unwrap(), "sakura");
        assert_eq!(
            std::fs::read(root.join("sakura").join("images").join("bg.png")).unwrap(),
            b"new"
        );
        assert!(source.join("skin.toml").is_file());
        let names: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["sakura"]);
    }

    #[test]
    fn import_replaces_an_existing_skin_whole() {
        let files = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let root = state.path().join("skins");
        std::fs::create_dir_all(root.join("sakura")).unwrap();
        std::fs::write(root.join("sakura").join("stale.css"), b"old").unwrap();
        import(&picked(files.path(), "sakura"), &root).unwrap();
        assert!(!root.join("sakura").join("stale.css").exists());
        assert!(root.join("sakura").join("skin.toml").is_file());
        assert!(!root.join(".replaced-sakura").exists());
    }

    #[test]
    fn import_refuses_what_the_catalog_would_not_list() {
        let files = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let root = state.path().join("skins");
        for name in ["Sakura", "night", "custom", "樱花"] {
            assert_eq!(import(&picked(files.path(), name), &root), Err("skin_name"));
        }
        let bare = files.path().join("bare");
        std::fs::create_dir_all(&bare).unwrap();
        assert_eq!(import(&bare, &root), Err("skin_manifest"));
        assert!(!root.exists());
    }

    #[cfg(unix)]
    #[test]
    fn import_rejects_a_symlinked_manifest_without_replacing_existing_skin() {
        let files = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let root = state.path().join("skins");
        std::fs::create_dir_all(root.join("sakura")).unwrap();
        std::fs::write(root.join("sakura").join("skin.toml"), b"old").unwrap();
        let source = picked(files.path(), "sakura");
        let outside = files.path().join("outside.toml");
        std::fs::write(&outside, b"id = 'synthetic'").unwrap();
        std::fs::remove_file(source.join("skin.toml")).unwrap();
        std::os::unix::fs::symlink(&outside, source.join("skin.toml")).unwrap();

        assert_eq!(import(&source, &root), Err("skin_manifest"));
        assert_eq!(
            std::fs::read(root.join("sakura").join("skin.toml")).unwrap(),
            b"old"
        );
    }

    #[cfg(unix)]
    #[test]
    fn import_rejects_a_symlinked_source_directory() {
        let files = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let root = state.path().join("skins");
        let actual = picked(files.path(), "sakura");
        let source = files.path().join("linked-sakura");
        std::os::unix::fs::symlink(&actual, &source).unwrap();

        assert_eq!(import(&source, &root), Err("skin_manifest"));
        assert!(!root.exists());
    }

    #[cfg(unix)]
    #[test]
    fn import_rejects_a_symlinked_destination_root() {
        let files = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = state.path().join("skins");
        std::os::unix::fs::symlink(outside.path(), &root).unwrap();
        let source = picked(files.path(), "sakura");

        assert_eq!(import(&source, &root), Err("storage"));
        assert!(!outside.path().join("sakura").exists());
        assert!(!outside.path().join(".import-sakura").exists());
    }

    #[test]
    fn import_that_runs_over_budget_leaves_the_previous_skin() {
        let files = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let root = state.path().join("skins");
        std::fs::create_dir_all(root.join("sakura")).unwrap();
        std::fs::write(root.join("sakura").join("skin.toml"), b"old").unwrap();
        let source = picked(files.path(), "sakura");
        let mut deep = source.clone();
        for _ in 0..=MAX_IMPORT_DEPTH {
            deep = deep.join("d");
        }
        std::fs::create_dir_all(&deep).unwrap();
        assert_eq!(import(&source, &root), Err("storage"));
        assert_eq!(
            std::fs::read(root.join("sakura").join("skin.toml")).unwrap(),
            b"old"
        );
        assert!(!root.join(".import-sakura").exists());
    }

    #[cfg(unix)]
    #[test]
    fn import_leaves_symbolic_links_behind() {
        let files = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let root = state.path().join("skins");
        let source = picked(files.path(), "sakura");
        std::os::unix::fs::symlink("/etc", source.join("escape")).unwrap();
        import(&source, &root).unwrap();
        assert!(std::fs::symlink_metadata(root.join("sakura").join("escape")).is_err());
    }
}
