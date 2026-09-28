//! Safe relocation of the macOS user-data root.
//!
//! The settings bundle and the InputMethodKit bundle are separate processes with separate bundle
//! identifiers. Each keeps a small `runtime-options.json` locator in its conventional Application
//! Support directory; both locators point at the one movable state root.

use serde_json::Value;
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub(crate) const DATA_DIRECTORY_MARKER: &str = ".metasequoiaime-data";
const OPTIONS_FILE: &str = "runtime-options.json";
const MAX_LOCATOR_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveError {
    InvalidSource,
    InvalidTarget,
    TargetNotEmpty,
    Copy,
    Prepare,
    Publish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MoveOutcome {
    pub retained_old_data: bool,
}

#[derive(Clone)]
struct LocatorBackup {
    path: PathBuf,
    contents: Option<Vec<u8>>,
}

fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(contents)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map(|_| ())
        .map_err(|error| error.error)
}

fn locator_backups(locators: &[PathBuf]) -> Result<Vec<LocatorBackup>, MoveError> {
    let mut unique = BTreeSet::new();
    let mut backups = Vec::new();
    for path in locators {
        if !unique.insert(path.clone()) {
            continue;
        }
        let contents = match File::open(path) {
            Ok(file) => {
                let mut contents = Vec::new();
                file.take(MAX_LOCATOR_BYTES + 1)
                    .read_to_end(&mut contents)
                    .map_err(|_| MoveError::Publish)?;
                if contents.len() as u64 > MAX_LOCATOR_BYTES {
                    return Err(MoveError::Publish);
                }
                Some(contents)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(_) => return Err(MoveError::Publish),
        };
        backups.push(LocatorBackup {
            path: path.clone(),
            contents,
        });
    }
    Ok(backups)
}

fn restore_locators(backups: &[LocatorBackup]) {
    for backup in backups {
        match &backup.contents {
            Some(contents) => {
                let _ = atomic_write(&backup.path, contents);
            }
            None => {
                let _ = fs::remove_file(&backup.path);
            }
        }
    }
}

fn copy_tree_contents(source: &Path, destination: &Path) -> Result<(), MoveError> {
    for entry in fs::read_dir(source).map_err(|_| MoveError::Copy)? {
        let entry = entry.map_err(|_| MoveError::Copy)?;
        copy_entry(&entry.path(), &destination.join(entry.file_name()))?;
    }
    Ok(())
}

fn copy_entry(source: &Path, destination: &Path) -> Result<(), MoveError> {
    let metadata = fs::symlink_metadata(source).map_err(|_| MoveError::Copy)?;
    if metadata.file_type().is_symlink() {
        return Err(MoveError::Copy);
    }
    if metadata.is_dir() {
        fs::create_dir(destination).map_err(|_| MoveError::Copy)?;
        copy_tree_contents(source, destination)?;
        fs::set_permissions(destination, metadata.permissions()).map_err(|_| MoveError::Copy)?;
        return Ok(());
    }
    if !metadata.is_file() {
        return Err(MoveError::Copy);
    }
    fs::copy(source, destination).map_err(|_| MoveError::Copy)?;
    fs::set_permissions(destination, metadata.permissions()).map_err(|_| MoveError::Copy)?;
    fs::File::open(destination)
        .and_then(|file| file.sync_all())
        .map_err(|_| MoveError::Copy)
}

fn target_entries_are_replaceable(target: &Path, default_root: &Path) -> Result<bool, MoveError> {
    let allowed: BTreeSet<&str> = if target == default_root {
        [DATA_DIRECTORY_MARKER, OPTIONS_FILE].into_iter().collect()
    } else {
        [DATA_DIRECTORY_MARKER].into_iter().collect()
    };
    for entry in fs::read_dir(target).map_err(|_| MoveError::InvalidTarget)? {
        let entry = entry.map_err(|_| MoveError::InvalidTarget)?;
        if entry
            .file_type()
            .map_err(|_| MoveError::InvalidTarget)?
            .is_symlink()
        {
            return Err(MoveError::InvalidTarget);
        }
        if !allowed.contains(entry.file_name().to_string_lossy().as_ref()) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn overlaps(first: &Path, second: &Path) -> bool {
    first.starts_with(second) || second.starts_with(first)
}

fn has_ownership_marker(directory: &Path) -> bool {
    fs::symlink_metadata(directory.join(DATA_DIRECTORY_MARKER))
        .map(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn restore_target(target: &Path, had_marker: bool, backups: &[LocatorBackup]) {
    if target.exists() {
        let _ = crate::platform::desktop::desktop_data_directory::remove_entry(target);
    }
    let _ = fs::create_dir_all(target);
    if had_marker {
        let _ = fs::write(
            target.join(DATA_DIRECTORY_MARKER),
            b"Metasequoia IME user data directory.\n",
        );
    }
    restore_locators(backups);
}

fn cleanup_source(source: &Path, default_root: &Path, locators: &[PathBuf]) -> bool {
    if source != default_root && !has_ownership_marker(source) {
        return false;
    }
    let preserved: BTreeSet<std::ffi::OsString> = locators
        .iter()
        .filter(|path| {
            path.parent()
                .and_then(|parent| fs::canonicalize(parent).ok())
                .as_deref()
                == Some(source)
        })
        .filter_map(|path| path.file_name().map(std::ffi::OsStr::to_os_string))
        .chain(std::iter::once(DATA_DIRECTORY_MARKER.into()))
        .collect();
    if source != default_root && preserved.len() == 1 {
        return fs::remove_dir_all(source).is_ok();
    }
    let Ok(entries) = fs::read_dir(source) else {
        return false;
    };
    let mut complete = true;
    for entry in entries.flatten() {
        if preserved.contains(&entry.file_name()) {
            continue;
        }
        complete &=
            crate::platform::desktop::desktop_data_directory::remove_entry(&entry.path()).is_ok();
    }
    complete
}

/// Copy state to `target`, rebuild path-bearing HostOptions there, atomically switch both process
/// locators, and only then remove the old owned data. `prepare` must not activate a session.
pub(crate) fn move_data_directory<F>(
    source: &Path,
    target: &Path,
    default_root: &Path,
    native_locator_root: &Path,
    locators: &[PathBuf],
    prepare: F,
) -> Result<MoveOutcome, MoveError>
where
    F: FnOnce(&Path) -> Result<Value, MoveError>,
{
    let source = crate::platform::desktop::desktop_data_directory::validate_directory(
        source,
        MoveError::InvalidSource,
    )?;
    let target = crate::platform::desktop::desktop_data_directory::validate_directory(
        target,
        MoveError::InvalidTarget,
    )?;
    let default_root = fs::canonicalize(default_root).map_err(|_| MoveError::InvalidSource)?;
    let native_locator_root = crate::platform::desktop::desktop_data_directory::validate_directory(
        native_locator_root,
        MoveError::InvalidTarget,
    )?;
    if source == target {
        return Ok(MoveOutcome {
            retained_old_data: false,
        });
    }
    if !target_entries_are_replaceable(&target, &default_root)? {
        return Err(MoveError::TargetNotEmpty);
    }
    if target == native_locator_root
        || overlaps(&source, &target)
        || (target != default_root && overlaps(&default_root, &target))
        || overlaps(&native_locator_root, &target)
        || target.parent().is_none()
        || target.parent() == Some(Path::new("/Volumes"))
    {
        return Err(MoveError::InvalidTarget);
    }

    let backups = locator_backups(locators)?;
    let had_marker = has_ownership_marker(&target);
    let parent = target.parent().ok_or(MoveError::InvalidTarget)?;
    let staging = tempfile::Builder::new()
        .prefix(".msime-data-migration-")
        .tempdir_in(parent)
        .map_err(|_| MoveError::Copy)?;
    copy_tree_contents(&source, staging.path())?;
    fs::write(
        staging.path().join(DATA_DIRECTORY_MARKER),
        b"Metasequoia IME user data directory.\n",
    )
    .map_err(|_| MoveError::Copy)?;

    crate::platform::desktop::desktop_data_directory::remove_entry(&target)
        .map_err(|_| MoveError::Copy)?;
    let staging = staging.keep();
    if fs::rename(&staging, &target).is_err() {
        let _ = fs::create_dir_all(&target);
        restore_target(&target, had_marker, &backups);
        return Err(MoveError::Copy);
    }

    let document = match prepare(&target) {
        Ok(document) if document.is_object() => document,
        _ => {
            restore_target(&target, had_marker, &backups);
            return Err(MoveError::Prepare);
        }
    };
    let serialized = serde_json::to_vec_pretty(&document).map_err(|_| MoveError::Prepare)?;
    if atomic_write(&target.join(OPTIONS_FILE), &serialized).is_err() {
        restore_target(&target, had_marker, &backups);
        return Err(MoveError::Prepare);
    }
    for locator in locators {
        if atomic_write(locator, &serialized).is_err() {
            restore_target(&target, had_marker, &backups);
            return Err(MoveError::Publish);
        }
    }

    let cleaned = cleanup_source(&source, &default_root, locators);
    Ok(MoveOutcome {
        retained_old_data: !cleaned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf, Vec<PathBuf>) {
        let root = tempdir().unwrap();
        let default = root.path().join("settings-state");
        let native = root.path().join("native-locator");
        let target = root.path().join("chosen-empty");
        fs::create_dir_all(&default).unwrap();
        fs::create_dir_all(&native).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(default.join("preferences.json"), b"synthetic-preferences").unwrap();
        fs::create_dir(default.join("user")).unwrap();
        fs::write(default.join("user/msime_user.db"), b"synthetic-dictionary").unwrap();
        let locators = vec![default.join(OPTIONS_FILE), native.join(OPTIONS_FILE)];
        for locator in &locators {
            fs::write(locator, b"old locator").unwrap();
        }
        (root, default, native, target, locators)
    }

    #[test]
    fn successful_move_switches_both_locators_before_cleaning_default_state() {
        let (_root, default, native, target, locators) = setup();
        let result = move_data_directory(&default, &target, &default, &native, &locators, |path| {
            Ok(json!({"preferences_directory": path, "user_data": path.join("user")}))
        })
        .unwrap();
        assert!(!result.retained_old_data);
        assert_eq!(
            fs::read(target.join("preferences.json")).unwrap(),
            b"synthetic-preferences"
        );
        assert_eq!(
            fs::read(target.join("user/msime_user.db")).unwrap(),
            b"synthetic-dictionary"
        );
        assert!(target.join(DATA_DIRECTORY_MARKER).is_file());
        assert!(!default.join("preferences.json").exists());
        assert!(default.join(OPTIONS_FILE).is_file());
        assert_eq!(
            fs::read(&locators[0]).unwrap(),
            fs::read(&locators[1]).unwrap()
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&locators[0]).unwrap()).unwrap()
                ["preferences_directory"],
            fs::canonicalize(&target)
                .unwrap()
                .to_string_lossy()
                .as_ref()
        );
    }

    #[test]
    fn failed_prepare_restores_locators_and_leaves_source_untouched() {
        let (_root, default, native, target, locators) = setup();
        let error = move_data_directory(&default, &target, &default, &native, &locators, |_| {
            Err(MoveError::Prepare)
        })
        .unwrap_err();
        assert_eq!(error, MoveError::Prepare);
        assert_eq!(fs::read(&locators[0]).unwrap(), b"old locator");
        assert_eq!(fs::read(&locators[1]).unwrap(), b"old locator");
        assert_eq!(
            fs::read(default.join("preferences.json")).unwrap(),
            b"synthetic-preferences"
        );
        assert!(fs::read_dir(target).unwrap().next().is_none());
    }

    #[test]
    fn oversized_locator_is_rejected_before_relocation() {
        let (_root, default, native, target, locators) = setup();
        fs::write(&locators[0], vec![b'x'; MAX_LOCATOR_BYTES as usize + 1]).unwrap();
        assert_eq!(
            move_data_directory(&default, &target, &default, &native, &locators, |_| {
                Ok(json!({}))
            }),
            Err(MoveError::Publish)
        );
        assert!(default.join("preferences.json").is_file());
    }

    #[test]
    fn refuses_nonempty_nested_and_symlinked_targets() {
        let (root, default, native, target, locators) = setup();
        fs::write(target.join("unrelated.txt"), b"keep").unwrap();
        assert_eq!(
            move_data_directory(&default, &target, &default, &native, &locators, |_| Ok(
                json!({})
            )),
            Err(MoveError::TargetNotEmpty)
        );
        let nested = default.join("nested");
        fs::create_dir(&nested).unwrap();
        assert_eq!(
            move_data_directory(&default, &nested, &default, &native, &locators, |_| Ok(
                json!({})
            )),
            Err(MoveError::InvalidTarget)
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let link = root.path().join("linked-target");
            symlink(&target, &link).unwrap();
            assert_eq!(
                move_data_directory(&default, &link, &default, &native, &locators, |_| Ok(
                    json!({})
                )),
                Err(MoveError::InvalidTarget)
            );
        }
        assert_eq!(fs::read(target.join("unrelated.txt")).unwrap(), b"keep");
    }

    #[test]
    fn unowned_custom_source_is_copied_but_never_deleted() {
        let (root, default, native, target, locators) = setup();
        let source = root.path().join("external-source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("preferences.json"), b"synthetic").unwrap();
        let outcome = move_data_directory(&source, &target, &default, &native, &locators, |path| {
            Ok(json!({"preferences_directory": path}))
        })
        .unwrap();
        assert!(outcome.retained_old_data);
        assert!(source.join("preferences.json").is_file());
        assert!(target.join("preferences.json").is_file());
    }
}
