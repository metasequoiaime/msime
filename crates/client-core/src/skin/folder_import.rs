//! Copying a picked skin folder into a host's skin root.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

/// Serializes every write to a skin root in this process: folder import and community install share the `.replaced-<id>` backup name, and community install shares one staging folder, so two overlapping writes would otherwise delete each other's helpers or both pass the "already installed" check.
static SKIN_ROOT_WRITES: Mutex<()> = Mutex::new(());

/// Hold the skin-root write lock. A write that panicked leaves nothing the next one relies on, since each write clears its own helpers first, so a poisoned lock is taken over rather than failing every later write.
pub(crate) fn lock_skin_root() -> MutexGuard<'static, ()> {
    SKIN_ROOT_WRITES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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
    let source_parent = source.parent().ok_or("skin_manifest")?;
    super::catalog::load_package(source_parent, &name).map_err(|_| "skin_manifest")?;
    // A linked root would publish the import into an unrelated directory. Check before creating
    // it because `create_dir_all` follows a final-component symlink.
    if !crate::storage::create_directory_and_check(root).map_err(|_| "storage")? {
        return Err("storage");
    }
    let _writes = lock_skin_root();
    // The leading dot keeps both helpers out of the catalog, which lists only names starting with a letter or digit.
    let staging = root.join(format!(".import-{name}"));
    let replaced = root.join(format!(".replaced-{name}"));
    for leftover in [&staging, &replaced] {
        remove_leftover(leftover)?;
    }
    let mut budget = ImportBudget::default();
    if copy_tree(source, &staging, 0, &mut budget).is_err() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("storage");
    }
    replace_directory(&staging, &root.join(&name), &replaced)?;
    Ok(name)
}

#[allow(clippy::needless_return)]
fn remove_leftover(path: &Path) -> Result<(), &'static str> {
    #[cfg(unix)]
    {
        let parent = path.parent().ok_or("storage")?;
        let name = path.file_name().ok_or("storage")?;
        let directory = crate::storage::open_private_directory(parent).map_err(|_| "storage")?;
        return match crate::storage::remove_private_tree_at(&directory, name) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("storage"),
        };
    }
    #[cfg(not(unix))]
    {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err("storage"),
        };
        if metadata.is_dir() {
            std::fs::remove_dir_all(path).map_err(|_| "storage")?;
        } else {
            std::fs::remove_file(path).map_err(|_| "storage")?;
        }
        Ok(())
    }
}

/// Swap a fully written `staging` directory in as `target`, replacing any existing `target` whole. The previous directory is first moved aside to `backup` and restored if the swap fails, so a failure never leaves a half-replaced skin; `staging` is removed on failure and `backup` after success. Every error is `storage`.
#[allow(clippy::needless_return)]
pub(crate) fn replace_directory(
    staging: &Path,
    target: &Path,
    backup: &Path,
) -> Result<(), &'static str> {
    #[cfg(unix)]
    return replace_directory_unix(staging, target, backup);
    #[cfg(not(unix))]
    {
        replace_directory_by_path(staging, target, backup)
    }
}

#[cfg(unix)]
fn replace_directory_unix(
    staging: &Path,
    target: &Path,
    backup: &Path,
) -> Result<(), &'static str> {
    let parent = staging.parent().ok_or("storage")?;
    if target.parent() != Some(parent) || backup.parent() != Some(parent) {
        return Err("storage");
    }
    let directory = crate::storage::open_private_directory(parent).map_err(|_| "storage")?;
    let staging_name = staging.file_name().ok_or("storage")?;
    let target_name = target.file_name().ok_or("storage")?;
    let backup_name = backup.file_name().ok_or("storage")?;
    let had_previous = rustix::fs::statat(
        &directory,
        target_name,
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    )
    .is_ok();
    if had_previous
        && rustix::fs::renameat(&directory, target_name, &directory, backup_name).is_err()
    {
        let _ = crate::storage::remove_private_tree_at(&directory, staging_name);
        return Err("storage");
    }
    if let Err(error) = rustix::fs::renameat(&directory, staging_name, &directory, target_name) {
        if had_previous {
            let _ = rustix::fs::renameat(&directory, backup_name, &directory, target_name);
        }
        let _ = crate::storage::remove_private_tree_at(&directory, staging_name);
        let _ = error;
        return Err("storage");
    }
    if had_previous {
        let _ = crate::storage::remove_private_tree_at(&directory, backup_name);
    }
    Ok(())
}

#[cfg(not(unix))]
fn replace_directory_by_path(
    staging: &Path,
    target: &Path,
    backup: &Path,
) -> Result<(), &'static str> {
    // `exists()` follows links and reports false for a dangling link, even though the
    // destination name still blocks the publish rename. Inspect the directory entry itself so
    // files and links are moved aside just like an existing directory.
    let had_previous = std::fs::symlink_metadata(target).is_ok();
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
        let _ = remove_leftover(backup);
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
            // Keep the type and bytes tied to the same no-follow handle. A picked
            // folder can be changed while it is being imported; `fs::copy` would
            // otherwise follow a file that was replaced by a symlink after the
            // `file_type` check.
            let mut input = crate::storage::open_private_file_in(&entry.path())?;
            let length = input.metadata()?.len();
            budget.bytes = budget
                .bytes
                .checked_sub(length)
                .ok_or_else(|| std::io::Error::other("skin folder too large"))?;
            let mut output = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target)?;
            std::io::copy(&mut input, &mut output)?;
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
        std::fs::write(
            skin.join("skin.toml"),
            format!(
                "schema_version = 1\nid = '{name}'\nname = 'Sample'\nversion = '1.0'\nbase = 'night'\n[supports]\nlayouts = ['vertical']\nthemes = ['light']\n[candidate_window]\nmin_width_dip = 10\n"
            ),
        )
        .unwrap();
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
    fn import_cleans_the_backup_when_an_existing_skin_slot_is_a_file() {
        let files = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let root = state.path().join("skins");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("sakura"), b"stray slot").unwrap();

        import(&picked(files.path(), "sakura"), &root).unwrap();

        assert!(root.join("sakura").join("skin.toml").is_file());
        assert!(!root.join(".replaced-sakura").exists());
    }

    #[cfg(unix)]
    #[test]
    fn import_replaces_a_dangling_skin_slot_link_without_following_it() {
        let files = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = state.path().join("skins");
        std::fs::create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink(outside.path().join("missing"), root.join("sakura")).unwrap();

        import(&picked(files.path(), "sakura"), &root).unwrap();

        assert!(root.join("sakura").join("skin.toml").is_file());
        assert!(!root.join(".replaced-sakura").exists());
        assert!(!outside.path().join("missing").exists());
    }

    #[test]
    fn import_clears_a_stray_file_left_by_an_interrupted_import() {
        let files = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let root = state.path().join("skins");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(".replaced-sakura"), b"stray leftover").unwrap();

        import(&picked(files.path(), "sakura"), &root).unwrap();

        assert!(root.join("sakura").join("skin.toml").is_file());
        assert!(!root.join(".replaced-sakura").exists());
    }

    #[test]
    fn import_rejects_an_invalid_manifest_before_replacing_existing_skin() {
        let files = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let root = state.path().join("skins");
        std::fs::create_dir_all(root.join("sakura")).unwrap();
        std::fs::write(root.join("sakura").join("skin.toml"), b"old").unwrap();
        let source = picked(files.path(), "sakura");
        std::fs::write(source.join("skin.toml"), b"not valid skin metadata").unwrap();

        assert_eq!(import(&source, &root), Err("skin_manifest"));
        assert_eq!(
            std::fs::read(root.join("sakura").join("skin.toml")).unwrap(),
            b"old"
        );
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

    #[cfg(unix)]
    #[test]
    fn replacement_refuses_a_root_replaced_by_a_symlink() {
        use std::os::unix::fs::symlink;

        let state = tempfile::tempdir().unwrap();
        let root = state.path().join("skins");
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir(&root).unwrap();
        let staging = root.join(".import-sakura");
        let target = root.join("sakura");
        let backup = root.join(".replaced-sakura");
        std::fs::create_dir(&staging).unwrap();
        std::fs::write(staging.join("skin.toml"), b"synthetic").unwrap();
        let moved = state.path().join("skins-moved");
        std::fs::rename(&root, &moved).unwrap();
        symlink(outside.path(), &root).unwrap();

        assert_eq!(
            replace_directory(&staging, &target, &backup),
            Err("storage")
        );
        assert!(moved.join(".import-sakura/skin.toml").exists());
        assert!(!outside.path().join("sakura").exists());
    }

    #[cfg(unix)]
    #[test]
    fn leftover_cleanup_rejects_a_symlinked_parent() {
        use std::os::unix::fs::symlink;

        let state = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let linked = state.path().join("linked");
        let victim = outside.path().join("staging");
        std::fs::create_dir(&victim).unwrap();
        std::fs::write(victim.join("keep.bin"), b"synthetic outside data").unwrap();
        symlink(outside.path(), &linked).unwrap();

        assert_eq!(remove_leftover(&linked.join("staging")), Err("storage"));
        assert!(victim.join("keep.bin").exists());
    }
}
