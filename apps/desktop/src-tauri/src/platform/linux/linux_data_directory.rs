//! Relocation of the Linux user-data root.
//!
//! On Linux the default state root (`$XDG_CONFIG_HOME/msime-client`) is also where every consumer finds its locator: the IBus launcher, the Fcitx5 addon, the clipboard monitor unit and the settings launcher all read `runtime-options.json` from that fixed path, and the provider services read their credential files from the same directory. A move therefore never relocates that directory itself. It moves the state entries (dictionaries, learning data, cache, preferences, skins, clipboard history) into the chosen directory and rewrites the path-bearing values of the locators in place, keeping every other key (provider sockets, models) the setup wrote.
//!
//! The IBus and Fcitx5 hosts write the learning data under `user/` while a session is open, so the whole move runs with them held off it (`linux_dictionary_quiesce::hold_hosts_off`): the quiesce lease asks them to close their sessions and the exclusive dictionary lock proves they did and keeps new ones out until the locators point at the copy and the old entries are gone. The input method framework is restarted before the hold is released.

use super::linux_dictionary_quiesce::{self, HoldError};
use crate::linux_process;
use crate::{HostActionError, RuntimeOptionsState};
use serde_json::Value;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

pub(crate) const DATA_DIRECTORY_MARKER: &str = ".metasequoiaime-data";
const OPTIONS_FILE: &str = "runtime-options.json";
const MAX_OPTIONS_BYTES: u64 = 1024 * 1024;
const INITIAL_OPTIONS_READ_CAPACITY: usize = 8 * 1024;
/// Files that belong to the fixed configuration directory rather than to the movable state: the locator and the provider credentials the systemd services read from `$XDG_CONFIG_HOME/msime-client`.
const PINNED_FILES: [&str; 4] = [
    OPTIONS_FILE,
    "ai-provider.json",
    "tencent-provider.json",
    "voice-provider.json",
];
const STAGING_PREFIX: &str = ".msime-data-migration-";
/// The picker is interactive, so this only bounds a dialog that was abandoned on another workspace.
const PICKER_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveError {
    InvalidSource,
    InvalidTarget,
    TargetNotEmpty,
    /// An input session kept the user directory open.
    Busy,
    Copy,
    Publish,
}

impl MoveError {
    fn code(self) -> &'static str {
        match self {
            MoveError::InvalidTarget => "data_directory_invalid",
            MoveError::TargetNotEmpty => "data_directory_not_empty",
            MoveError::InvalidSource => "data_directory_unavailable",
            MoveError::Busy => "data_directory_busy",
            MoveError::Copy | MoveError::Publish => "data_directory_move_failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MoveOutcome {
    pub retained_old_data: bool,
}

struct LocatorBackup {
    path: PathBuf,
    contents: Vec<u8>,
}

fn is_pinned(name: &std::ffi::OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    // A provider credential save in flight writes `<file>.new` before its rename.
    PINNED_FILES
        .iter()
        .any(|pinned| name == *pinned || name.strip_suffix(".new") == Some(pinned))
}

/// `$XDG_CONFIG_HOME/msime-client`, or `~/.config/msime-client`. A relative `XDG_CONFIG_HOME` is invalid per the base directory specification and is ignored the same way the setup script ignores it.
pub(crate) fn default_root() -> Option<PathBuf> {
    let base = super::config_home(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )?;
    // 目录名随本安装包所属的版本（full 是 msime-client）。
    Some(base.join(
        &msime_client_core::edition::Edition::linux_package_identity_or_full().client_directory,
    ))
}

fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    crate::shared::atomic_file::check_directory_ancestors(parent)?;
    if fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "locator is a symbolic link",
        ));
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(contents)?;
    temporary.as_file().sync_all()?;
    let permissions = fs::metadata(path)?.permissions();
    temporary.as_file().set_permissions(permissions)?;
    temporary
        .persist(path)
        .map(|_| ())
        .map_err(|error| error.error)
}

/// A staging directory of an earlier move, or the trash of a cleanup that could not finish. It is never state to carry along, nor user content that makes a directory non-empty.
fn is_staging(name: &std::ffi::OsStr) -> bool {
    name.to_str()
        .is_some_and(|name| name.starts_with(STAGING_PREFIX))
}

/// The state entries of `source`: everything except the pinned configuration files, the ownership marker and leftover staging directories.
fn state_entries(source: &Path) -> Result<Vec<OsString>, MoveError> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(source).map_err(|_| MoveError::InvalidSource)? {
        let name = entry.map_err(|_| MoveError::InvalidSource)?.file_name();
        if is_pinned(&name) || name == DATA_DIRECTORY_MARKER || is_staging(&name) {
            continue;
        }
        entries.push(name);
    }
    entries.sort();
    Ok(entries)
}

/// A target may already hold the marker and leftover staging directories. The default root, as a target when moving back, may also hold its pinned files, and nothing else.
fn target_is_empty(target: &Path, default_root: &Path) -> Result<bool, MoveError> {
    for entry in fs::read_dir(target).map_err(|_| MoveError::InvalidTarget)? {
        let entry = entry.map_err(|_| MoveError::InvalidTarget)?;
        // The marker and staging names are normally tolerated so an interrupted move can be
        // resumed. They must still be real entries: fs::write(marker) below follows a symlink,
        // which could otherwise overwrite a file outside the selected data directory.
        if entry
            .file_type()
            .map_err(|_| MoveError::InvalidTarget)?
            .is_symlink()
        {
            return Err(MoveError::InvalidTarget);
        }
        let name = entry.file_name();
        if name == DATA_DIRECTORY_MARKER
            || is_staging(&name)
            || (target == default_root && is_pinned(&name))
        {
            continue;
        }
        return Ok(false);
    }
    Ok(true)
}

/// Replace the `source` prefix of every absolute path string in `document` with `target`. Strings that are not paths under `source` (resources, models, sockets, preference values) are left alone.
fn rebase_paths(document: &mut Value, source: &Path, target: &Path) {
    match document {
        Value::String(text) => {
            let path = Path::new(text.as_str());
            if !path.is_absolute() {
                return;
            }
            if let Ok(rest) = path.strip_prefix(source) {
                let rebased = if rest.as_os_str().is_empty() {
                    target.to_path_buf()
                } else {
                    target.join(rest)
                };
                if let Some(rebased) = rebased.to_str() {
                    *text = rebased.to_owned();
                }
            }
        }
        Value::Array(values) => values
            .iter_mut()
            .for_each(|value| rebase_paths(value, source, target)),
        Value::Object(values) => values
            .values_mut()
            .for_each(|value| rebase_paths(value, source, target)),
        _ => {}
    }
}

fn rebased_locator(
    path: &Path,
    source: &Path,
    written_source: &Path,
    target: &Path,
) -> Result<(LocatorBackup, Vec<u8>), MoveError> {
    let parent = path.parent().ok_or(MoveError::Publish)?;
    crate::shared::atomic_file::check_directory_ancestors(parent)
        .map_err(|_| MoveError::Publish)?;
    if fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(MoveError::Publish);
    }
    let file = fs::File::open(path).map_err(|_| MoveError::Publish)?;
    let mut contents =
        Vec::with_capacity((MAX_OPTIONS_BYTES as usize).min(INITIAL_OPTIONS_READ_CAPACITY));
    file.take(MAX_OPTIONS_BYTES + 1)
        .read_to_end(&mut contents)
        .map_err(|_| MoveError::Publish)?;
    if contents.len() as u64 > MAX_OPTIONS_BYTES {
        return Err(MoveError::Publish);
    }
    let mut document: Value = serde_json::from_slice(&contents).map_err(|_| MoveError::Publish)?;
    if !document.is_object() {
        return Err(MoveError::Publish);
    }
    rebase_paths(&mut document, written_source, target);
    if written_source != source {
        rebase_paths(&mut document, source, target);
    }
    let rewritten = serde_json::to_vec_pretty(&document).map_err(|_| MoveError::Publish)?;
    Ok((
        LocatorBackup {
            path: path.to_path_buf(),
            contents,
        },
        rewritten,
    ))
}

fn rollback(target: &Path, placed: &[OsString], wrote_marker: bool, backups: &[LocatorBackup]) {
    for backup in backups {
        let _ = atomic_write(&backup.path, &backup.contents);
    }
    for name in placed {
        let _ = crate::platform::desktop::desktop_data_directory::remove_entry(&target.join(name));
    }
    if wrote_marker {
        let _ = fs::remove_file(target.join(DATA_DIRECTORY_MARKER));
    }
}

fn cleanup_source(source: &Path, default_root: &Path, moved: &[OsString]) -> bool {
    if source != default_root
        && !crate::platform::desktop::desktop_data_directory::has_ownership_marker(
            source,
            DATA_DIRECTORY_MARKER,
        )
    {
        return false;
    }
    // Take every entry out of place with one rename before deleting it, so a host still configured for the old paths finds no directory at all rather than one being emptied under it, which it could open and start a new library in.
    let Ok(trash) = tempfile::Builder::new()
        .prefix(STAGING_PREFIX)
        .tempdir_in(source)
    else {
        return false;
    };
    let mut complete = true;
    for name in moved {
        complete &= fs::rename(source.join(name), trash.path().join(name)).is_ok();
    }
    let trash = trash.keep();
    if fs::remove_dir_all(&trash).is_err() {
        restore_survivors(source, &trash, moved);
        complete = false;
    }
    if complete && source != default_root {
        // Only an owned directory that is now empty apart from the marker is removed; anything the user put there stays.
        let _ = fs::remove_file(source.join(DATA_DIRECTORY_MARKER));
        let _ = fs::remove_dir(source);
    }
    complete
}

/// Put back under its own name whatever a failed cleanup could not delete, so the old data the settings page reports as retained is where the user will look for it rather than inside a hidden staging directory.
fn restore_survivors(source: &Path, trash: &Path, moved: &[OsString]) {
    for name in moved {
        let _ = fs::rename(trash.join(name), source.join(name));
    }
    let _ = fs::remove_dir(trash);
}

/// A move that has passed every check that does not need the hosts to let go: both directories are valid and distinct, the target is empty and every locator parses. Nothing has been written yet.
pub(crate) struct MovePlan {
    source: PathBuf,
    target: PathBuf,
    default_root: PathBuf,
    rewrites: Vec<(LocatorBackup, Vec<u8>)>,
}

/// Check a move of the state entries of `source` into `target` before any host is asked to let go, so a bad pick never interrupts typing. `None` when they are the same directory and there is nothing to do. `written_source` is the state root as the locators spell it, which may differ from its canonical form.
pub(crate) fn plan_move(
    source: &Path,
    written_source: &Path,
    target: &Path,
    default_root: &Path,
    locators: &[PathBuf],
) -> Result<Option<MovePlan>, MoveError> {
    let source = crate::platform::desktop::desktop_data_directory::validate_directory(
        source,
        MoveError::InvalidSource,
    )?;
    let target = crate::platform::desktop::desktop_data_directory::validate_directory(
        target,
        MoveError::InvalidTarget,
    )?;
    let default_root = fs::canonicalize(default_root).map_err(|_| MoveError::InvalidSource)?;
    if source == target {
        return Ok(None);
    }
    if target.parent().is_none()
        || target.starts_with(&source)
        || source.starts_with(&target)
        || (target != default_root && target.starts_with(&default_root))
    {
        return Err(MoveError::InvalidTarget);
    }
    if !target_is_empty(&target, &default_root)? {
        return Err(MoveError::TargetNotEmpty);
    }

    let mut unique = HashSet::with_capacity(locators.len());
    let mut rewrites = Vec::with_capacity(locators.len());
    for locator in locators {
        if unique.insert(fs::canonicalize(locator).unwrap_or_else(|_| locator.clone())) {
            rewrites.push(rebased_locator(locator, &source, written_source, &target)?);
        }
    }
    Ok(Some(MovePlan {
        source,
        target,
        default_root,
        rewrites,
    }))
}

impl MovePlan {
    /// Copy the state entries into the target, rewrite the locators to point at it, and only then remove the old owned entries. Run it with the hosts held off the source: while the lease is up they open no session there and save no preference there, so nothing they write is left behind in the old directory.
    pub(crate) fn execute(self) -> Result<MoveOutcome, MoveError> {
        let MovePlan {
            source,
            target,
            default_root,
            rewrites,
        } = self;
        let entries = state_entries(&source)?;

        // Stage inside the target so the final renames stay on one filesystem, and a failed copy leaves nothing but the staging directory behind.
        let staging = tempfile::Builder::new()
            .prefix(STAGING_PREFIX)
            .tempdir_in(&target)
            .map_err(|_| MoveError::Copy)?;
        for name in &entries {
            crate::platform::desktop::desktop_data_directory::copy_entry(
                &source.join(name),
                &staging.path().join(name),
                &linux_dictionary_quiesce::is_lease_file,
            )
            .map_err(|_| MoveError::Copy)?;
        }
        let wrote_marker = target != default_root
            && !crate::platform::desktop::desktop_data_directory::has_ownership_marker(
                &target,
                DATA_DIRECTORY_MARKER,
            );
        if wrote_marker
            && fs::write(
                target.join(DATA_DIRECTORY_MARKER),
                b"Metasequoia IME user data directory.\n",
            )
            .is_err()
        {
            return Err(MoveError::Copy);
        }
        let mut placed = Vec::with_capacity(entries.len());
        for name in &entries {
            if fs::rename(staging.path().join(name), target.join(name)).is_err() {
                rollback(&target, &placed, wrote_marker, &[]);
                return Err(MoveError::Copy);
            }
            placed.push(name.clone());
        }
        drop(staging);

        let mut backups = Vec::with_capacity(rewrites.len());
        for (backup, rewritten) in rewrites {
            let path = backup.path.clone();
            backups.push(backup);
            if atomic_write(&path, &rewritten).is_err() {
                rollback(&target, &placed, wrote_marker, &backups);
                return Err(MoveError::Publish);
            }
        }

        Ok(MoveOutcome {
            retained_old_data: !cleanup_source(&source, &default_root, &entries),
        })
    }
}

/// The whole move: check it, hold the hosts off the source, copy, publish and clean up, then restart the input method while they are still held, so an engine that has not yet read the new locator is replaced instead of reopening the old paths. Returns whether the restart went through. A failed restart does not undo a completed move: both hosts also read the locator again before they open a session.
fn move_holding_hosts<Held>(
    plan: Option<MovePlan>,
    hold: impl FnOnce() -> Result<Held, MoveError>,
    restart: impl FnOnce() -> bool,
) -> Result<(MoveOutcome, bool), MoveError> {
    let Some(plan) = plan else {
        return Ok((
            MoveOutcome {
                retained_old_data: false,
            },
            true,
        ));
    };
    let held = hold()?;
    let outcome = plan.execute()?;
    let restarted = restart();
    drop(held);
    Ok((outcome, restarted))
}

/// Hold both hosts off `user_data` for a move, in the terms the settings page reports: a session that would not let go is `Busy`, a user directory that cannot be leased or locked is `InvalidSource`.
fn hold_hosts_for_move(
    user_data: &Path,
    dictionaries: &Path,
) -> Result<Option<linux_dictionary_quiesce::HostsHeldOff>, MoveError> {
    linux_dictionary_quiesce::hold_hosts_off(user_data, dictionaries).map_err(|error| match error {
        HoldError::Busy => MoveError::Busy,
        HoldError::Unavailable => MoveError::InvalidSource,
    })
}

/// Ask the desktop's own dialog tool for a directory: KDE ships `kdialog`, GNOME and most others ship `zenity`. Returns `Ok(None)` when the user cancels.
fn pick_directory() -> Result<Option<PathBuf>, &'static str> {
    let kde = std::env::var("XDG_CURRENT_DESKTOP")
        .is_ok_and(|desktop| desktop.split(':').any(|name| name == "KDE"));
    let mut tools: Vec<(&str, &[&str])> = vec![
        (
            "zenity",
            &["--file-selection", "--directory", "--title=选择数据目录"],
        ),
        (
            "kdialog",
            &["--getexistingdirectory", "--title", "选择数据目录"],
        ),
    ];
    if kde {
        tools.reverse();
    }
    let (program, arguments) = tools
        .into_iter()
        .find(|(program, _)| linux_process::program_available(program))
        .ok_or("data_directory_picker_unavailable")?;
    // Both tools exit non-zero on cancel, which the bounded reader reports as no output.
    let Some(output) = linux_process::read_text(program, arguments, 4096, PICKER_TIMEOUT) else {
        return Ok(None);
    };
    let chosen = PathBuf::from(output.trim_end_matches('\n'));
    Ok(chosen.is_absolute().then_some(chosen))
}

fn absolute_option(document: &Value, key: &str) -> Option<PathBuf> {
    document
        .get(key)
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

fn written_state_root(runtime: &RuntimeOptionsState) -> Result<PathBuf, HostActionError> {
    let document = runtime.snapshot().map_err(|_| HostActionError {
        code: "data_directory_unavailable",
    })?;
    absolute_option(&document, "preferences_directory").ok_or(HostActionError {
        code: "data_directory_unavailable",
    })
}

#[derive(Default)]
pub(crate) struct DataDirectorySelectionState(Mutex<Option<PathBuf>>);

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DataDirectoryStatus {
    path: String,
    is_default: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DataDirectoryMoveResult {
    path: String,
    is_default: bool,
    retained_old_data: bool,
    /// False when the input method framework could not be restarted after the move; the settings page tells the user to restart it.
    input_method_restarted: bool,
}

fn same_directory(first: &Path, second: &Path) -> bool {
    fs::canonicalize(first).ok() == fs::canonicalize(second).ok()
}

#[tauri::command]
pub(crate) fn data_directory_status(
    runtime: tauri::State<'_, RuntimeOptionsState>,
) -> Result<DataDirectoryStatus, HostActionError> {
    let path = written_state_root(&runtime)?;
    let default = default_root().ok_or(HostActionError {
        code: "data_directory_unavailable",
    })?;
    Ok(DataDirectoryStatus {
        is_default: same_directory(&path, &default),
        path: path.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub(crate) async fn pick_data_directory(
    selection: tauri::State<'_, DataDirectorySelectionState>,
) -> Result<Option<String>, HostActionError> {
    let chosen = tauri::async_runtime::spawn_blocking(pick_directory)
        .await
        .map_err(|_| HostActionError {
            code: "data_directory_unavailable",
        })?
        .map_err(|code| HostActionError { code })?;
    *selection.0.lock().map_err(|_| HostActionError {
        code: "data_directory_unavailable",
    })? = chosen.clone();
    Ok(chosen.map(|path| path.to_string_lossy().into_owned()))
}

#[tauri::command]
pub(crate) async fn move_data_directory(
    app: tauri::AppHandle,
    runtime: tauri::State<'_, RuntimeOptionsState>,
    selection: tauri::State<'_, DataDirectorySelectionState>,
) -> Result<DataDirectoryMoveResult, HostActionError> {
    let document = runtime.snapshot().map_err(|_| HostActionError {
        code: "data_directory_unavailable",
    })?;
    let written_source =
        absolute_option(&document, "preferences_directory").ok_or(HostActionError {
            code: "data_directory_unavailable",
        })?;
    // Where the hosts look for the lease and take their dictionary locks. Sessions lock the dictionary generation as well as the user directory; one that is missing cannot be opened, so the user directory alone covers it.
    let user_data = absolute_option(&document, "user_data").ok_or(HostActionError {
        code: "data_directory_unavailable",
    })?;
    let dictionaries = absolute_option(&document, "dictionaries")
        .filter(|path| path.is_dir())
        .unwrap_or_else(|| user_data.clone());
    let target = selection
        .0
        .lock()
        .map_err(|_| HostActionError {
            code: "data_directory_unavailable",
        })?
        .take()
        .ok_or(HostActionError {
            code: "data_directory_invalid",
        })?;
    let default = default_root().ok_or(HostActionError {
        code: "data_directory_unavailable",
    })?;
    // The file this window reads and the fixed per-user locator every input method reads; normally the same file.
    let mut locators: Vec<PathBuf> = runtime.path.iter().cloned().collect();
    let default_locator = default.join(OPTIONS_FILE);
    if default_locator.is_file() {
        locators.push(default_locator);
    }
    if locators.is_empty() {
        return Err(HostActionError {
            code: "data_directory_unavailable",
        });
    }

    let is_default = same_directory(&target, &default);
    let moved_target = target.clone();
    let (outcome, restarted) = tauri::async_runtime::spawn_blocking(move || {
        let plan = plan_move(
            &written_source,
            &written_source,
            &moved_target,
            &default,
            &locators,
        )?;
        move_holding_hosts(
            plan,
            || hold_hosts_for_move(&user_data, &dictionaries),
            || match crate::restart_input_method_blocking() {
                Ok(()) => true,
                Err(error) => {
                    eprintln!(
                        "msime-linux-settings: data directory moved, but restarting the input method failed ({})",
                        error.code
                    );
                    false
                }
            },
        )
    })
    .await
    .map_err(|_| HostActionError {
        code: "data_directory_move_failed",
    })?
    .map_err(|error| HostActionError { code: error.code() })?;

    // The window still holds the old paths, so it closes; a failed restart leaves it up long enough to read what to do.
    let exit_delay = if restarted {
        Duration::from_millis(500)
    } else {
        Duration::from_secs(5)
    };
    let exit_app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(exit_delay);
        exit_app.exit(0);
    });
    Ok(DataDirectoryMoveResult {
        path: fs::canonicalize(&target)
            .unwrap_or(target)
            .to_string_lossy()
            .into_owned(),
        is_default,
        retained_old_data: outcome.retained_old_data,
        input_method_restarted: restarted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_core::dictionary::access::DictionaryAccess;
    use serde_json::json;
    use tempfile::tempdir;

    struct Layout {
        _root: tempfile::TempDir,
        default: PathBuf,
        target: PathBuf,
        locator: PathBuf,
    }

    fn setup() -> Layout {
        let root = tempdir().unwrap();
        let base = fs::canonicalize(root.path()).unwrap();
        let default = base.join("config/msime-client");
        let target = base.join("chosen-empty");
        fs::create_dir_all(default.join("user")).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(default.join("preferences.json"), b"synthetic-preferences").unwrap();
        fs::write(default.join("user/msime_user.db"), b"synthetic-dictionary").unwrap();
        fs::write(default.join("ai-provider.json"), b"synthetic-credential").unwrap();
        let locator = default.join(OPTIONS_FILE);
        let document = json!({
            "resources": base.join("resources"),
            "user_data": default.join("user"),
            "cache": default.join("cache"),
            "preferences_directory": default,
            "dictionaries": [{"path": default.join("user/msime_user.db")}],
            "online_provider_socket": "/run/user/1000/msime-client/online.sock",
        });
        fs::write(&locator, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
        Layout {
            _root: root,
            default,
            target,
            locator,
        }
    }

    fn locator_document(layout: &Layout) -> Value {
        serde_json::from_slice(&fs::read(&layout.locator).unwrap()).unwrap()
    }

    /// Check and run a move with no hosts to hold off, as the tests of the copy itself need.
    fn relocate_state(
        source: &Path,
        written_source: &Path,
        target: &Path,
        default_root: &Path,
        locators: &[PathBuf],
    ) -> Result<MoveOutcome, MoveError> {
        match plan_move(source, written_source, target, default_root, locators)? {
            Some(plan) => plan.execute(),
            None => Ok(MoveOutcome {
                retained_old_data: false,
            }),
        }
    }

    fn has_lease(directory: &Path) -> bool {
        fs::read_dir(directory)
            .unwrap()
            .any(|entry| linux_dictionary_quiesce::is_lease_file(&entry.unwrap().file_name()))
    }

    /// An input host with a session open on `user`, closing it once the lease appears, as both hosts do on their timers.
    fn host_session(user: &Path) -> std::thread::JoinHandle<()> {
        let session = DictionaryAccess::try_session(user, user).unwrap().unwrap();
        let user = user.to_path_buf();
        std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while !has_lease(&user) {
                assert!(
                    std::time::Instant::now() < deadline,
                    "the lease never appeared"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            drop(session);
        })
    }

    fn hold(
        user: &Path,
    ) -> impl FnOnce() -> Result<Option<linux_dictionary_quiesce::HostsHeldOff>, MoveError> + '_
    {
        move || hold_hosts_for_move(user, user)
    }

    #[test]
    fn busy_hosts_fail_the_move_before_anything_is_copied_or_restarted() {
        let layout = setup();
        let before = fs::read(&layout.locator).unwrap();
        let plan = plan_move(
            &layout.default,
            &layout.default,
            &layout.target,
            &layout.default,
            std::slice::from_ref(&layout.locator),
        )
        .unwrap();
        let result = move_holding_hosts(
            plan,
            || Err::<(), _>(MoveError::Busy),
            || unreachable!("nothing moved, so nothing to restart"),
        );
        assert_eq!(result, Err(MoveError::Busy));
        assert_eq!(MoveError::Busy.code(), "data_directory_busy");
        assert!(fs::read_dir(&layout.target).unwrap().next().is_none());
        assert_eq!(fs::read(&layout.locator).unwrap(), before);
        assert_eq!(
            fs::read(layout.default.join("user/msime_user.db")).unwrap(),
            b"synthetic-dictionary"
        );
        assert!(layout.default.join("preferences.json").is_file());
    }

    #[test]
    fn a_session_that_stays_open_makes_the_move_busy_and_leaves_the_source_as_it_was() {
        let layout = setup();
        let user = layout.default.join("user");
        let before = fs::read(&layout.locator).unwrap();
        // A host that never answers the lease.
        let _session = DictionaryAccess::try_session(&user, &user)
            .unwrap()
            .unwrap();
        let plan = plan_move(
            &layout.default,
            &layout.default,
            &layout.target,
            &layout.default,
            std::slice::from_ref(&layout.locator),
        )
        .unwrap();
        let result = move_holding_hosts(plan, hold(&user), || unreachable!("nothing moved"));
        assert_eq!(result, Err(MoveError::Busy));
        assert!(!has_lease(&user));
        assert!(fs::read_dir(&layout.target).unwrap().next().is_none());
        assert_eq!(fs::read(&layout.locator).unwrap(), before);
        assert!(user.join("msime_user.db").is_file());
    }

    #[test]
    fn the_hosts_stay_off_the_old_directory_until_the_restart_and_the_copy_carries_no_lease() {
        let layout = setup();
        // An unowned source keeps its entries, so the old user directory can be watched through the whole move.
        let source = layout.default.parent().unwrap().join("external-source");
        let user = source.join("user");
        fs::create_dir_all(&user).unwrap();
        fs::write(user.join("msime_user.db"), b"synthetic-dictionary").unwrap();
        let document = json!({"user_data": user, "preferences_directory": source});
        fs::write(
            &layout.locator,
            serde_json::to_vec_pretty(&document).unwrap(),
        )
        .unwrap();
        let host = host_session(&user);
        let plan = plan_move(
            &source,
            &source,
            &layout.target,
            &layout.default,
            std::slice::from_ref(&layout.locator),
        )
        .unwrap();
        let mut restarts = 0;
        let (outcome, restarted) = move_holding_hosts(plan, hold(&user), || {
            restarts += 1;
            assert!(has_lease(&user));
            assert!(DictionaryAccess::try_session(&user, &user)
                .unwrap()
                .is_none());
            assert_eq!(
                locator_document(&layout)["user_data"],
                json!(layout.target.join("user"))
            );
            true
        })
        .unwrap();
        host.join().unwrap();
        assert_eq!(restarts, 1);
        assert!(restarted);
        assert!(outcome.retained_old_data);
        assert!(!has_lease(&user));
        let moved = layout.target.join("user");
        assert!(!has_lease(&moved));
        assert_eq!(
            fs::read(moved.join("msime_user.db")).unwrap(),
            b"synthetic-dictionary"
        );
        assert!(DictionaryAccess::try_session(&moved, &moved)
            .unwrap()
            .is_some());
    }

    #[test]
    fn an_owned_source_is_gone_before_the_hosts_are_let_back_and_a_failed_restart_is_reported() {
        let layout = setup();
        let user = layout.default.join("user");
        let host = host_session(&user);
        let plan = plan_move(
            &layout.default,
            &layout.default,
            &layout.target,
            &layout.default,
            std::slice::from_ref(&layout.locator),
        )
        .unwrap();
        let (outcome, restarted) = move_holding_hosts(plan, hold(&user), || {
            // A host still on the old paths finds no user directory to open, rather than one it could start an empty library in.
            assert!(!user.exists());
            false
        })
        .unwrap();
        host.join().unwrap();
        assert!(!restarted);
        assert!(!outcome.retained_old_data);
        assert!(!user.exists());
        assert!(!has_lease(&layout.target.join("user")));
        let leftovers: Vec<_> = fs::read_dir(&layout.default)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name.to_string_lossy().starts_with(STAGING_PREFIX))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        assert_eq!(
            fs::read(layout.target.join("user/msime_user.db")).unwrap(),
            b"synthetic-dictionary"
        );
    }

    #[test]
    fn a_user_directory_that_cannot_be_held_is_reported_unavailable_before_anything_is_copied() {
        let layout = setup();
        let before = fs::read(&layout.locator).unwrap();
        // Not absolute, or not a directory the lease can be written into.
        let not_a_directory = layout.default.join("preferences.json");
        for user in [Path::new("relative/user"), not_a_directory.as_path()] {
            let plan = plan_move(
                &layout.default,
                &layout.default,
                &layout.target,
                &layout.default,
                std::slice::from_ref(&layout.locator),
            )
            .unwrap();
            let result = move_holding_hosts(
                plan,
                || hold_hosts_for_move(user, user),
                || unreachable!("nothing moved"),
            );
            assert_eq!(result, Err(MoveError::InvalidSource));
        }
        assert_eq!(
            MoveError::InvalidSource.code(),
            "data_directory_unavailable"
        );
        assert!(fs::read_dir(&layout.target).unwrap().next().is_none());
        assert_eq!(fs::read(&layout.locator).unwrap(), before);
        assert!(layout.default.join("user/msime_user.db").is_file());
    }

    #[test]
    fn what_a_failed_cleanup_could_not_delete_goes_back_under_its_own_name() {
        let layout = setup();
        let source = &layout.default;
        let trash = source.join(format!("{STAGING_PREFIX}trash"));
        // The state of a cleanup whose delete stopped part way: the dictionary survived, the preferences did not.
        fs::create_dir_all(&trash).unwrap();
        fs::rename(source.join("user"), trash.join("user")).unwrap();
        fs::remove_file(source.join("preferences.json")).unwrap();
        restore_survivors(
            source,
            &trash,
            &[OsString::from("preferences.json"), OsString::from("user")],
        );
        assert_eq!(
            fs::read(source.join("user/msime_user.db")).unwrap(),
            b"synthetic-dictionary"
        );
        assert!(!source.join("preferences.json").exists());
        assert!(!trash.exists());
    }

    #[test]
    fn a_leftover_staging_directory_is_neither_moved_nor_in_the_way_of_moving_back() {
        let layout = setup();
        let locators = [layout.locator.clone()];
        let leftover = layout.default.join(format!("{STAGING_PREFIX}stale"));
        fs::create_dir(&leftover).unwrap();
        fs::write(leftover.join("preferences.json"), b"stale").unwrap();
        relocate_state(
            &layout.default,
            &layout.default,
            &layout.target,
            &layout.default,
            &locators,
        )
        .unwrap();
        assert!(fs::read_dir(&layout.target)
            .unwrap()
            .all(|entry| !is_staging(&entry.unwrap().file_name())));
        assert!(leftover.join("preferences.json").is_file());
        let outcome = relocate_state(
            &layout.target,
            &layout.target,
            &layout.default,
            &layout.default,
            &locators,
        )
        .unwrap();
        assert!(!outcome.retained_old_data);
        assert_eq!(
            fs::read(layout.default.join("user/msime_user.db")).unwrap(),
            b"synthetic-dictionary"
        );
    }

    #[test]
    fn moving_onto_the_same_directory_holds_nothing() {
        let layout = setup();
        let plan = plan_move(
            &layout.default,
            &layout.default,
            &layout.default,
            &layout.default,
            std::slice::from_ref(&layout.locator),
        )
        .unwrap();
        assert!(plan.is_none());
        let (outcome, restarted) = move_holding_hosts(
            plan,
            || -> Result<(), MoveError> { unreachable!("nothing to hold") },
            || unreachable!("nothing to restart"),
        )
        .unwrap();
        assert!(!outcome.retained_old_data);
        assert!(restarted);
    }

    #[test]
    fn moves_state_and_rebases_only_the_state_paths_of_the_fixed_locator() {
        let layout = setup();
        let outcome = relocate_state(
            &layout.default,
            &layout.default,
            &layout.target,
            &layout.default,
            std::slice::from_ref(&layout.locator),
        )
        .unwrap();
        assert!(!outcome.retained_old_data);
        assert_eq!(
            fs::read(layout.target.join("user/msime_user.db")).unwrap(),
            b"synthetic-dictionary"
        );
        assert!(layout.target.join(DATA_DIRECTORY_MARKER).is_file());
        assert!(!layout.default.join("preferences.json").exists());
        assert!(!layout.default.join("user").exists());
        // The locator and the provider credentials stay where the services look for them.
        assert!(layout.locator.is_file());
        assert!(layout.default.join("ai-provider.json").is_file());
        assert!(!layout.target.join("ai-provider.json").exists());
        assert!(!layout.target.join(OPTIONS_FILE).exists());
        let document = locator_document(&layout);
        assert_eq!(document["preferences_directory"], json!(layout.target));
        assert_eq!(document["user_data"], json!(layout.target.join("user")));
        assert_eq!(
            document["dictionaries"][0]["path"],
            json!(layout.target.join("user/msime_user.db"))
        );
        assert_eq!(
            document["online_provider_socket"],
            "/run/user/1000/msime-client/online.sock"
        );
        assert_ne!(
            document["resources"],
            json!(layout.target.join("resources"))
        );
    }

    #[test]
    fn moving_back_to_the_default_root_removes_the_owned_custom_directory() {
        let layout = setup();
        let locators = [layout.locator.clone()];
        relocate_state(
            &layout.default,
            &layout.default,
            &layout.target,
            &layout.default,
            &locators,
        )
        .unwrap();
        let outcome = relocate_state(
            &layout.target,
            &layout.target,
            &layout.default,
            &layout.default,
            &locators,
        )
        .unwrap();
        assert!(!outcome.retained_old_data);
        assert!(!layout.target.exists());
        assert_eq!(
            fs::read(layout.default.join("preferences.json")).unwrap(),
            b"synthetic-preferences"
        );
        assert!(!layout.default.join(DATA_DIRECTORY_MARKER).exists());
        assert_eq!(
            locator_document(&layout)["preferences_directory"],
            json!(layout.default)
        );
    }

    #[test]
    fn refuses_nonempty_nested_and_symlinked_targets_without_touching_anything() {
        let layout = setup();
        let locators = [layout.locator.clone()];
        let before = fs::read(&layout.locator).unwrap();
        fs::write(layout.target.join("unrelated.txt"), b"keep").unwrap();
        assert_eq!(
            relocate_state(
                &layout.default,
                &layout.default,
                &layout.target,
                &layout.default,
                &locators
            ),
            Err(MoveError::TargetNotEmpty)
        );
        let nested = layout.default.join("user/nested");
        fs::create_dir(&nested).unwrap();
        assert_eq!(
            relocate_state(
                &layout.default,
                &layout.default,
                &nested,
                &layout.default,
                &locators
            ),
            Err(MoveError::InvalidTarget)
        );
        let link = layout.default.parent().unwrap().join("linked-target");
        std::os::unix::fs::symlink(&layout.target, &link).unwrap();
        assert_eq!(
            relocate_state(
                &layout.default,
                &layout.default,
                &link,
                &layout.default,
                &locators
            ),
            Err(MoveError::InvalidTarget)
        );
        assert_eq!(fs::read(&layout.locator).unwrap(), before);
        assert!(layout.default.join("preferences.json").is_file());
        assert_eq!(
            fs::read(layout.target.join("unrelated.txt")).unwrap(),
            b"keep"
        );
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlinked_marker_without_following_it() {
        use std::os::unix::fs::symlink;

        let layout = setup();
        let outside = layout._root.path().join("outside-marker");
        fs::write(&outside, b"keep").unwrap();
        symlink(&outside, layout.target.join(DATA_DIRECTORY_MARKER)).unwrap();

        assert_eq!(
            relocate_state(
                &layout.default,
                &layout.default,
                &layout.target,
                &layout.default,
                std::slice::from_ref(&layout.locator),
            ),
            Err(MoveError::InvalidTarget)
        );
        assert_eq!(fs::read(&outside).unwrap(), b"keep");
        assert!(layout.default.join("preferences.json").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlinked_locator_without_replacing_it() {
        use std::os::unix::fs::symlink;

        let layout = setup();
        let outside = layout._root.path().join("outside-locator");
        fs::write(&outside, fs::read(&layout.locator).unwrap()).unwrap();
        fs::remove_file(&layout.locator).unwrap();
        symlink(&outside, &layout.locator).unwrap();

        assert_eq!(
            relocate_state(
                &layout.default,
                &layout.default,
                &layout.target,
                &layout.default,
                std::slice::from_ref(&layout.locator),
            ),
            Err(MoveError::Publish)
        );
        assert!(fs::symlink_metadata(&layout.locator)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read(&outside).unwrap(),
            fs::read(&layout.locator).unwrap()
        );
        assert!(layout.default.join("preferences.json").is_file());
        assert!(layout.target.read_dir().unwrap().next().is_none());
    }

    #[test]
    fn unreadable_locator_aborts_before_any_state_is_copied() {
        let layout = setup();
        fs::write(&layout.locator, b"not json").unwrap();
        assert_eq!(
            relocate_state(
                &layout.default,
                &layout.default,
                &layout.target,
                &layout.default,
                std::slice::from_ref(&layout.locator),
            ),
            Err(MoveError::Publish)
        );
        assert!(fs::read_dir(&layout.target).unwrap().next().is_none());
        assert!(layout.default.join("preferences.json").is_file());
    }

    #[test]
    fn oversized_locator_aborts_before_any_state_is_copied() {
        let layout = setup();
        fs::write(
            &layout.locator,
            vec![b'x'; (MAX_OPTIONS_BYTES + 1) as usize],
        )
        .unwrap();
        assert_eq!(
            relocate_state(
                &layout.default,
                &layout.default,
                &layout.target,
                &layout.default,
                std::slice::from_ref(&layout.locator),
            ),
            Err(MoveError::Publish)
        );
        assert!(fs::read_dir(&layout.target).unwrap().next().is_none());
    }

    #[test]
    fn unowned_custom_source_is_copied_but_never_deleted() {
        let layout = setup();
        let source = layout.default.parent().unwrap().join("external-source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("preferences.json"), b"synthetic").unwrap();
        let outcome = relocate_state(
            &source,
            &source,
            &layout.target,
            &layout.default,
            std::slice::from_ref(&layout.locator),
        )
        .unwrap();
        assert!(outcome.retained_old_data);
        assert!(source.join("preferences.json").is_file());
        assert!(layout.target.join("preferences.json").is_file());
    }

    #[test]
    fn pinned_files_include_in_flight_credential_writes() {
        assert!(is_pinned(std::ffi::OsStr::new("voice-provider.json.new")));
        assert!(is_pinned(std::ffi::OsStr::new(OPTIONS_FILE)));
        assert!(!is_pinned(std::ffi::OsStr::new("preferences.json")));
        assert!(!is_pinned(std::ffi::OsStr::new("skins")));
    }
}
