//! Install the packaged macOS InputMethodKit bundle without making the
//! settings application own input-method state.
//!
//! The settings app is the public UI for this action, but the input source
//! remains a separate bundle with its existing IMK process boundary.  Copying
//! is deliberately staged and atomic so a failed update never removes a
//! working input source.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// The identifier the input method bundle carries, which is MetasequoiaIME's rather than a new one of this client's: the client supersedes that input source in place instead of standing beside it. `validate_bundle` looks for it in the packaged Info.plist, so a value that has drifted from platforms/macos/Info.plist.in rejects the correct bundle rather than accepting a wrong one.
pub(crate) const INPUT_SOURCE_BUNDLE_ID: &str = "app.msime.inputmethod.MetasequoiaIME";
pub(crate) const INPUT_SOURCE_BUNDLE_NAME: &str = "水杉输入法.app";
const INPUT_SOURCE_EXECUTABLE: &str = "水杉输入法";
const INITIAL_COMMAND_OUTPUT_CAPACITY: usize = 8 * 1024;
const MAX_INFO_PLIST_BYTES: u64 = 1024 * 1024;
const MAX_LAUNCH_SERVICES_DUMP_BYTES: usize = 8 * 1024 * 1024;
const MAX_INPUT_SOURCE_PREFERENCES_BYTES: usize = 1024 * 1024;
/// Bundles under a name this no longer installs, still sitting in `~/Library/Input Methods`.
///
/// The directory name is not something a user ever reads - the input menu and System Settings show
/// the localized display name - and moving it costs more than it looks: the system's list of
/// installed input sources is built per login session from the paths it knows, so a bundle that
/// arrives at a new path is invisible until the next login, however correctly it registers. The
/// name therefore stays where it has always been, and a copy briefly installed under the shorter
/// one is removed. Same bundle identifier on both, so leaving two behind would give the input menu
/// two entries for one source.
const LEGACY_BUNDLE_NAMES: [&str; 1] = ["水杉输入法（预览）.app"];

#[derive(Debug)]
pub(crate) enum InstallError {
    SourceUnavailable,
    InvalidBundle,
    HomeUnavailable,
    Io,
    Registration,
    /// A first install whose registration the system did not accept: the bundle is left in place, because there is nothing to roll back to and the login session's input source list only picks up an identifier that is new to it at the next login.
    RegistrationPending,
}

// Staging and backup directories are named by process id, so two installs in one process - the start-time refresh and the settings page's button, or an uninstall - would work on the same paths; every install and removal holds this for its whole run.
static INSTALL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn bounded_command_output(command: &mut Command, maximum: usize) -> Option<Vec<u8>> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let mut bytes = Vec::with_capacity(maximum.min(INITIAL_COMMAND_OUTPUT_CAPACITY));
    let read = stdout
        .take((maximum.saturating_add(1)) as u64)
        .read_to_end(&mut bytes);
    let oversized = bytes.len() > maximum;
    if read.is_err() || oversized {
        let _ = child.kill();
    }
    let status = child.wait().ok()?;
    if read.is_err() || oversized || !status.success() {
        return None;
    }
    Some(bytes)
}

/// Hold off every other install, refresh or removal of the input method in this process until the guard is dropped.
pub(crate) fn install_lock() -> std::sync::MutexGuard<'static, ()> {
    INSTALL_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn is_symlink(path: &Path) -> io::Result<bool> {
    Ok(fs::symlink_metadata(path)?.file_type().is_symlink())
}

fn validate_bundle(source: &Path) -> Result<(), InstallError> {
    let metadata = fs::symlink_metadata(source).map_err(|_| InstallError::SourceUnavailable)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(InstallError::InvalidBundle);
    }
    for directory in [source.join("Contents"), source.join("Contents/MacOS")] {
        let metadata = fs::symlink_metadata(&directory).map_err(|_| InstallError::InvalidBundle)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(InstallError::InvalidBundle);
        }
    }
    let info = source.join("Contents/Info.plist");
    let executable = source.join("Contents/MacOS").join(INPUT_SOURCE_EXECUTABLE);
    if is_symlink(&info).map_err(|_| InstallError::InvalidBundle)?
        || is_symlink(&executable).map_err(|_| InstallError::InvalidBundle)?
        || !info.is_file()
        || !executable.is_file()
    {
        return Err(InstallError::InvalidBundle);
    }
    let plist = crate::shared::bounded_body::read_bounded(
        fs::File::open(info).map_err(|_| InstallError::InvalidBundle)?,
        MAX_INFO_PLIST_BYTES as usize,
    )
    .map_err(|_| InstallError::InvalidBundle)?;
    if !String::from_utf8_lossy(&plist).contains(INPUT_SOURCE_BUNDLE_ID) {
        return Err(InstallError::InvalidBundle);
    }
    Ok(())
}

/// Whether a symlink at `link`, somewhere below `root`, resolves lexically to a path that is still below `root`.
///
/// A framework is mostly symlinks (`Sparkle.framework/Sparkle -> Versions/Current/Sparkle`, `Versions/Current -> B`), and the bundle's code signature seals them as links: copying their targets instead leaves a framework codesign calls ambiguous, so the input method cannot be launched. They are therefore recreated as links, but only relative ones that stay inside the bundle, so an installed bundle can never point at anything outside itself.
fn link_stays_inside(root: &Path, link: &Path, target: &Path) -> bool {
    use std::path::Component;
    let Some(parent) = link.parent() else {
        return false;
    };
    let Ok(parent) = parent.strip_prefix(root) else {
        return false;
    };
    let mut depth = parent.components().count();
    for component in target.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir if depth > 0 => depth -= 1,
            _ => return false,
        }
    }
    true
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), InstallError> {
    copy_tree_within(source, source, destination)
}

fn copy_tree_within(root: &Path, source: &Path, destination: &Path) -> Result<(), InstallError> {
    let metadata = fs::symlink_metadata(source).map_err(|_| InstallError::Io)?;
    if metadata.file_type().is_symlink() {
        let target = fs::read_link(source).map_err(|_| InstallError::Io)?;
        if !link_stays_inside(root, source, &target) {
            return Err(InstallError::InvalidBundle);
        }
        return std::os::unix::fs::symlink(&target, destination).map_err(|_| InstallError::Io);
    }
    if metadata.is_dir() {
        fs::create_dir(destination).map_err(|_| InstallError::Io)?;
        for entry in fs::read_dir(source).map_err(|_| InstallError::Io)? {
            let entry = entry.map_err(|_| InstallError::Io)?;
            copy_tree_within(root, &entry.path(), &destination.join(entry.file_name()))?;
        }
        fs::set_permissions(destination, metadata.permissions()).map_err(|_| InstallError::Io)?;
        return Ok(());
    }
    if !metadata.is_file() {
        return Err(InstallError::InvalidBundle);
    }
    fs::copy(source, destination).map_err(|_| InstallError::Io)?;
    fs::set_permissions(destination, metadata.permissions()).map_err(|_| InstallError::Io)
}

fn remove_staging(path: &Path) -> Result<(), InstallError> {
    if !path.exists() {
        return Ok(());
    }
    if is_symlink(path).map_err(|_| InstallError::Io)? {
        return Err(InstallError::InvalidBundle);
    }
    fs::remove_dir_all(path).map_err(|_| InstallError::Io)
}

/// Install a validated bundle below `input_methods`, replacing an existing directory only after the complete copy has succeeded and registration has accepted the staged replacement. A failed registration restores the old bundle before returning the error; with no old bundle it keeps the new one and returns `RegistrationPending`, as `scripts/install.sh` does.
fn install_bundle_at_with_registration<F>(
    source: &Path,
    input_methods: &Path,
    register: F,
) -> Result<PathBuf, InstallError>
where
    F: FnOnce(&Path) -> Result<(), InstallError>,
{
    validate_bundle(source)?;
    fs::create_dir_all(input_methods).map_err(|_| InstallError::Io)?;
    let target = input_methods.join(INPUT_SOURCE_BUNDLE_NAME);
    if target.exists() && is_symlink(&target).map_err(|_| InstallError::Io)? {
        return Err(InstallError::InvalidBundle);
    }
    let pid = std::process::id();
    let staging = input_methods.join(format!(".{INPUT_SOURCE_BUNDLE_NAME}.installing-{pid}"));
    let backup = input_methods.join(format!(".{INPUT_SOURCE_BUNDLE_NAME}.previous-{pid}"));
    remove_staging(&staging)?;
    if backup.exists() {
        return Err(InstallError::Io);
    }
    copy_tree(source, &staging)?;

    let had_previous = target.exists();
    if had_previous {
        fs::rename(&target, &backup).map_err(|_| {
            let _ = remove_staging(&staging);
            InstallError::Io
        })?;
    }
    if fs::rename(&staging, &target).is_err() {
        let _ = remove_staging(&staging);
        if had_previous {
            let _ = fs::rename(&backup, &target);
        }
        return Err(InstallError::Io);
    }
    if let Err(error) = register(&target) {
        // An identifier that was not in the input source list when the login session began cannot join it before the next login, however the bundle is signed (see platforms/macos/README.md). Deleting the only copy would leave nothing for that login to find.
        if !had_previous {
            return Err(InstallError::RegistrationPending);
        }
        if fs::remove_dir_all(&target).is_err() {
            return Err(InstallError::Io);
        }
        if had_previous && fs::rename(&backup, &target).is_err() {
            return Err(InstallError::Io);
        }
        return Err(error);
    }
    if had_previous {
        fs::remove_dir_all(&backup).map_err(|_| InstallError::Io)?;
    }
    remove_legacy_bundles(input_methods);
    Ok(target)
}

/// Delete bundles installed under an older name, after the new one is in place and registered.
///
/// Best-effort on purpose: the install has already succeeded by this point, and a stale copy the
/// user can delete themselves is a smaller problem than undoing an installation that worked. A
/// symlink is left alone - it is not something this ever created, and following it would delete
/// whatever it points at.
fn remove_legacy_bundles(input_methods: &Path) {
    for name in LEGACY_BUNDLE_NAMES {
        let legacy = input_methods.join(name);
        if !legacy.exists() || is_symlink(&legacy).unwrap_or(true) {
            continue;
        }
        let _ = fs::remove_dir_all(&legacy);
    }
}

/// Install a validated bundle below `input_methods`, replacing an existing
/// directory only after the complete copy has succeeded.
pub(crate) fn install_bundle_at(
    source: &Path,
    input_methods: &Path,
) -> Result<PathBuf, InstallError> {
    install_bundle_at_with_registration(source, input_methods, |_| Ok(()))
}

fn source_candidates(resource_directory: Option<&Path>, current_directory: &Path) -> Vec<PathBuf> {
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository_root = manifest_root.join("../../..");
    let mut candidates = Vec::with_capacity(3);
    if let Some(resource_directory) = resource_directory {
        candidates.push(resource_directory.join(INPUT_SOURCE_BUNDLE_NAME));
    }
    candidates.push(
        current_directory
            .join("target/macos")
            .join(INPUT_SOURCE_BUNDLE_NAME),
    );
    candidates.push(
        repository_root
            .join("target/macos")
            .join(INPUT_SOURCE_BUNDLE_NAME),
    );
    candidates
}

fn find_source(
    resource_directory: Option<&Path>,
    current_directory: &Path,
) -> Result<PathBuf, InstallError> {
    source_candidates(resource_directory, current_directory)
        .into_iter()
        .find(|candidate| validate_bundle(candidate).is_ok())
        .ok_or(InstallError::SourceUnavailable)
}

fn home_input_methods() -> Result<PathBuf, InstallError> {
    let home = std::env::var_os("HOME").ok_or(InstallError::HomeUnavailable)?;
    let home = PathBuf::from(home);
    if !home.is_absolute() {
        return Err(InstallError::HomeUnavailable);
    }
    Ok(home.join("Library/Input Methods"))
}

pub(crate) fn installed_bundle_path() -> Result<PathBuf, InstallError> {
    Ok(home_input_methods()?.join(INPUT_SOURCE_BUNDLE_NAME))
}

/// The path LaunchServices records bundles through, when it is where macOS keeps it.
const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/\
LaunchServices.framework/Support/lsregister";

/// Refresh the LaunchServices record for a bundle that was just replaced in place.
///
/// Without it the record still describes the previous copy at that path: the system then refuses to
/// launch the input method when its source is selected, `lsappinfo` reports a null bundle identifier
/// for the process, and the bundle's own `--register-input-source` returns success without the
/// source appearing in the list. All three were observed on the machine this was written on, and one
/// `-f` on the installed path clears them. Best-effort: the install itself has already succeeded, and
/// a machine without the tool is not a reason to fail it.
fn refresh_launch_services(bundle: &Path) {
    if !Path::new(LSREGISTER).exists() {
        return;
    }
    let _ = std::process::Command::new(LSREGISTER)
        .arg("-f")
        .arg(bundle)
        .status();
}

fn launch_services_paths_for_identifier(dump: &str, identifier: &str) -> Vec<PathBuf> {
    let mut path = None;
    let mut matches = Vec::new();
    for line in dump.lines() {
        if let Some(raw) = line.strip_prefix("path:") {
            let raw = raw.trim();
            let raw = raw
                .strip_suffix(')')
                .and_then(|raw| raw.rsplit_once(" (0x").map(|(path, _)| path))
                .unwrap_or(raw);
            path = Some(PathBuf::from(raw));
        } else if line
            .strip_prefix("identifier:")
            .is_some_and(|value| value.trim() == identifier)
        {
            if let Some(path) = path.take() {
                matches.push(path);
            }
        }
    }
    matches
}

/// Remove stale LaunchServices records for this input method before registering its replacement.
///
/// Each build directory is a bundle as far as LaunchServices is concerned. When a developer
/// installs from the settings page, old worktree bundles remain registered under the same
/// identifier and macOS presents each record as another copy of the input source. Keep the bundle
/// being installed and unregister every competing record; missing paths are safe to unregister as
/// well because `lsregister -u` only removes the registry entry.
fn remove_competing_launch_services_records(bundle: &Path) {
    if !Path::new(LSREGISTER).exists() {
        return;
    }
    let mut command = Command::new(LSREGISTER);
    command.arg("-dump");
    let Some(output) = bounded_command_output(&mut command, MAX_LAUNCH_SERVICES_DUMP_BYTES) else {
        return;
    };
    let installed = bundle
        .canonicalize()
        .ok()
        .unwrap_or_else(|| bundle.to_path_buf());
    let dump = String::from_utf8_lossy(&output);
    for stale in launch_services_paths_for_identifier(&dump, INPUT_SOURCE_BUNDLE_ID) {
        let same_bundle = stale == bundle
            || stale
                .canonicalize()
                .map(|path| path == installed)
                .unwrap_or(false);
        if same_bundle {
            continue;
        }
        let _ = std::process::Command::new(LSREGISTER)
            .arg("-u")
            .arg(stale)
            .status();
    }
}

fn register_installed_bundle(bundle: &Path) -> Result<(), InstallError> {
    remove_competing_launch_services_records(bundle);
    refresh_launch_services(bundle);
    let executable = bundle.join("Contents/MacOS").join(INPUT_SOURCE_EXECUTABLE);
    let status = std::process::Command::new(executable)
        .arg("--register-input-source")
        .status()
        .map_err(|_| InstallError::Registration)?;
    if status.success() {
        Ok(())
    } else {
        Err(InstallError::Registration)
    }
}

pub(crate) fn install(resource_directory: Option<&Path>) -> Result<(), InstallError> {
    let _guard = install_lock();
    install_unlocked(resource_directory)
}

fn install_unlocked(resource_directory: Option<&Path>) -> Result<(), InstallError> {
    let source = find_source(
        resource_directory,
        &std::env::current_dir().map_err(|_| InstallError::SourceUnavailable)?,
    )?;
    let input_methods = home_input_methods()?;
    install_bundle_at_with_registration(&source, &input_methods, register_installed_bundle)
        .map(|_| ())
}

/// The version an input method bundle declares: `CFBundleShortVersionString` first, then `CFBundleVersion`.
///
/// The short version is the release and is compared numerically, component by component, with trailing zeros ignored (0.50 equals 0.50.0). The build number is the commit count CMake stamps into the bundle, which only orders builds of the same release.
#[derive(Debug, Clone)]
pub(crate) struct BundleVersion {
    short: Vec<u64>,
    build: u64,
    text: String,
}

impl BundleVersion {
    fn parse(short: &str, build: &str) -> Option<Self> {
        let short = short.trim();
        let build = build.trim();
        let mut components = short
            .split('.')
            .map(|component| component.parse::<u64>().ok())
            .collect::<Option<Vec<_>>>()?;
        while components.last() == Some(&0) {
            components.pop();
        }
        Some(Self {
            short: components,
            build: build.parse().ok()?,
            text: format!("{short} ({build})"),
        })
    }

    /// The version as the user reads it, e.g. `0.50.0 (7289)`.
    pub(crate) fn label(&self) -> &str {
        &self.text
    }
}

impl PartialEq for BundleVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for BundleVersion {}

impl PartialOrd for BundleVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BundleVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.short
            .cmp(&other.short)
            .then(self.build.cmp(&other.build))
    }
}

fn plist_string(info: &Path, key: &str) -> Option<String> {
    let output = std::process::Command::new("/usr/bin/plutil")
        .args(["-extract", key, "raw", "-o", "-"])
        .arg(info)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// Read a bundle's version through `plutil`, which accepts both XML and binary property lists. Missing or non-numeric values give `None`.
pub(crate) fn bundle_version(bundle: &Path) -> Option<BundleVersion> {
    let info = bundle.join("Contents/Info.plist");
    if !info.is_file() {
        return None;
    }
    let short = plist_string(&info, "CFBundleShortVersionString")?;
    let build = plist_string(&info, "CFBundleVersion")?;
    BundleVersion::parse(&short, &build)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refresh {
    Install,
    Update,
    UpToDate,
}

/// What the start-time check does with the bundle the settings app carries.
///
/// Never a downgrade: the installed copy can legitimately be newer than the one inside the settings app being opened - an older DMG's app opened after a newer one, a newer build installed with `scripts/install.sh`, or, for builds that carry `SUFeedURL`, a Sparkle update - and replacing it would roll the user back. An installed bundle whose version cannot be read is replaced by a readable one, since nothing says it is newer; a bundled copy whose version cannot be read never replaces anything that is already installed.
pub(crate) fn refresh_decision(
    bundled: Option<&BundleVersion>,
    installed: Option<&BundleVersion>,
    installed_exists: bool,
) -> Refresh {
    if !installed_exists {
        return Refresh::Install;
    }
    match (bundled, installed) {
        (None, _) => Refresh::UpToDate,
        (Some(_), None) => Refresh::Update,
        (Some(bundled), Some(installed)) if bundled > installed => Refresh::Update,
        _ => Refresh::UpToDate,
    }
}

#[derive(Debug)]
pub(crate) struct RefreshOutcome {
    pub(crate) refresh: Refresh,
    pub(crate) bundled: Option<BundleVersion>,
    pub(crate) installed: Option<BundleVersion>,
}

/// The start-time check against explicit paths, with the installation step injected so tests never register anything.
fn ensure_current_with<F>(
    source: &Path,
    target: &Path,
    install_source: F,
) -> Result<RefreshOutcome, InstallError>
where
    F: FnOnce() -> Result<(), InstallError>,
{
    validate_bundle(source)?;
    let bundled = bundle_version(source);
    let installed = bundle_version(target);
    let refresh = refresh_decision(bundled.as_ref(), installed.as_ref(), target.exists());
    if refresh == Refresh::UpToDate {
        return Ok(RefreshOutcome {
            refresh,
            bundled,
            installed,
        });
    }
    install_source()?;
    Ok(RefreshOutcome {
        refresh,
        installed: bundle_version(target),
        bundled,
    })
}

/// Install the packaged input method when it is missing and refresh it when the packaged copy is newer, the way the Windows installer registers its TSF DLLs on every install and upgrade.
///
/// Only the bundle inside a packaged app's own `Contents/Resources` is considered - never a `target/macos` build, nor the copy tauri-build places next to a `cargo run` binary - so running a development build does not replace the input method a developer has installed. `SourceUnavailable` means the resource directory is not a packaged app's, or that build carries no input method at all.
pub(crate) fn ensure_current(resource_directory: &Path) -> Result<RefreshOutcome, InstallError> {
    if !is_packaged_resource_directory(resource_directory) {
        return Err(InstallError::SourceUnavailable);
    }
    let _guard = install_lock();
    let source = resource_directory.join(INPUT_SOURCE_BUNDLE_NAME);
    let target = installed_bundle_path()?;
    ensure_current_with(&source, &target, || {
        install_unlocked(Some(resource_directory))
    })
}

/// Whether `resource_directory` is the `Contents/Resources` of an `.app` bundle. A development run's resource directory is the cargo output directory, where tauri-build has copied the development input method with its framework symlinks flattened.
pub(crate) fn is_packaged_resource_directory(resource_directory: &Path) -> bool {
    resource_directory.file_name() == Some("Resources".as_ref())
        && resource_directory
            .parent()
            .filter(|contents| contents.file_name() == Some("Contents".as_ref()))
            .and_then(Path::parent)
            .and_then(Path::extension)
            == Some("app".as_ref())
}

/// Whether the `AppleEnabledInputSources` list, as JSON, has any entry for this input method.
fn enabled_in_input_source_list(json: &[u8]) -> Option<bool> {
    let list: serde_json::Value = serde_json::from_slice(json).ok()?;
    Some(list.as_array()?.iter().any(|entry| {
        entry.get("Bundle ID").and_then(serde_json::Value::as_str) == Some(INPUT_SOURCE_BUNDLE_ID)
    }))
}

/// Whether the user has this input method in the System Settings input source list. `None` when the list cannot be read.
///
/// Read through `defaults export`, which asks cfprefsd, rather than the plist file itself: the file lags behind a registration that has only just enabled the source.
pub(crate) fn input_source_enabled() -> Option<bool> {
    use std::io::Write;
    let mut command = Command::new("/usr/bin/defaults");
    command.args(["export", "com.apple.HIToolbox", "-"]);
    let exported = bounded_command_output(&mut command, MAX_INPUT_SOURCE_PREFERENCES_BYTES)?;
    let mut plutil = Command::new("/usr/bin/plutil")
        .args([
            "-extract",
            "AppleEnabledInputSources",
            "json",
            "-o",
            "-",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    plutil.stdin.take()?.write_all(&exported).ok()?;
    let output = plutil.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    enabled_in_input_source_list(&output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn fixture(root: &Path, id: &str, executable_contents: &[u8]) -> PathBuf {
        let bundle = root.join("fixture.app");
        fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
        fs::write(
            bundle.join("Contents/Info.plist"),
            format!("CFBundleIdentifier={id}"),
        )
        .unwrap();
        let executable = bundle.join(format!("Contents/MacOS/{INPUT_SOURCE_EXECUTABLE}"));
        fs::write(&executable, executable_contents).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        bundle
    }

    #[test]
    fn bounded_command_output_rejects_oversized_stdout() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf 12345"]);
        assert!(bounded_command_output(&mut command, 4).is_none());
    }

    #[cfg(target_os = "macos")]
    fn versioned_fixture(root: &Path, short: &str, build: &str, contents: &[u8]) -> PathBuf {
        let bundle = fixture(root, INPUT_SOURCE_BUNDLE_ID, contents);
        fs::write(
            bundle.join("Contents/Info.plist"),
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>CFBundleIdentifier</key><string>{INPUT_SOURCE_BUNDLE_ID}</string><key>CFBundleVersion</key><string>{build}</string><key>CFBundleShortVersionString</key><string>{short}</string></dict></plist>\n"
            ),
        )
        .unwrap();
        bundle
    }

    fn version(short: &str, build: &str) -> BundleVersion {
        BundleVersion::parse(short, build).unwrap()
    }

    #[test]
    fn rejects_oversized_info_plist() {
        let root = tempdir().unwrap();
        let bundle = fixture(root.path(), INPUT_SOURCE_BUNDLE_ID, b"synthetic");
        let mut plist = vec![b'x'; 1024 * 1024 + 1];
        plist.extend_from_slice(INPUT_SOURCE_BUNDLE_ID.as_bytes());
        fs::write(bundle.join("Contents/Info.plist"), plist).unwrap();

        assert!(matches!(
            validate_bundle(&bundle),
            Err(InstallError::InvalidBundle)
        ));
    }

    #[test]
    fn refresh_installs_when_missing_and_never_downgrades() {
        let current = version("0.50.0", "100");
        assert_eq!(
            refresh_decision(Some(&current), None, false),
            Refresh::Install
        );
        assert_eq!(refresh_decision(None, None, false), Refresh::Install);
        assert_eq!(
            refresh_decision(Some(&version("0.50.0", "101")), Some(&current), true),
            Refresh::Update
        );
        assert_eq!(
            refresh_decision(Some(&version("0.51", "1")), Some(&current), true),
            Refresh::Update
        );
        assert_eq!(
            refresh_decision(Some(&version("0.50", "100")), Some(&current), true),
            Refresh::UpToDate
        );
        // A Sparkle update leaves the installed copy newer than the bundled one.
        assert_eq!(
            refresh_decision(
                Some(&version("0.50.0", "200")),
                Some(&version("0.50.1", "150")),
                true
            ),
            Refresh::UpToDate
        );
        assert_eq!(
            refresh_decision(Some(&current), Some(&version("0.50.0", "101")), true),
            Refresh::UpToDate
        );
        assert_eq!(
            refresh_decision(None, Some(&current), true),
            Refresh::UpToDate
        );
        assert_eq!(
            refresh_decision(Some(&current), None, true),
            Refresh::Update
        );
    }

    #[test]
    fn version_components_compare_numerically() {
        assert!(version("0.10.0", "1") > version("0.9.9", "999"));
        assert_eq!(version("1.0", "7").label(), "1.0 (7)");
        assert!(BundleVersion::parse("0.50.x", "1").is_none());
        assert!(BundleVersion::parse("0.50.0", "").is_none());
    }

    // `bundle_version` reads the plist through `/usr/bin/plutil`, which exists only on macOS. The
    // module is built and tested on every Unix for its file handling, but a test that needs a real
    // version read can only run where `plutil` does; elsewhere every read is `None` by design.
    #[cfg(target_os = "macos")]
    #[test]
    fn reads_bundle_version_from_info_plist() {
        let root = tempdir().unwrap();
        let bundle = versioned_fixture(root.path(), "0.50.0", "7289", b"x");
        assert_eq!(bundle_version(&bundle), Some(version("0.50.0", "7289")));

        let unversioned = tempdir().unwrap();
        let bundle = fixture(unversioned.path(), INPUT_SOURCE_BUNDLE_ID, b"x");
        fs::write(
            bundle.join("Contents/Info.plist"),
            "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>CFBundleShortVersionString</key><string>0.50.0</string></dict></plist>",
        )
        .unwrap();
        assert_eq!(bundle_version(&bundle), None);
        assert_eq!(bundle_version(&root.path().join("missing.app")), None);
    }

    // `bundle_version` reads the plist through `/usr/bin/plutil`, which exists only on macOS. The
    // module is built and tested on every Unix for its file handling, but a test that needs a real
    // version read can only run where `plutil` does; elsewhere every read is `None` by design.
    #[cfg(target_os = "macos")]
    #[test]
    fn ensure_current_replaces_only_an_older_install() {
        let root = tempdir().unwrap();
        let destination = root.path().join("Library/Input Methods");
        let target = destination.join(INPUT_SOURCE_BUNDLE_NAME);
        let executable = target.join(format!("Contents/MacOS/{INPUT_SOURCE_EXECUTABLE}"));

        let first_root = tempdir().unwrap();
        let first = versioned_fixture(first_root.path(), "0.50.0", "10", b"first");
        let outcome = ensure_current_with(&first, &target, || {
            install_bundle_at(&first, &destination).map(|_| ())
        })
        .unwrap();
        assert_eq!(outcome.refresh, Refresh::Install);
        assert_eq!(outcome.installed, Some(version("0.50.0", "10")));

        let same_root = tempdir().unwrap();
        let same = versioned_fixture(same_root.path(), "0.50.0", "10", b"same");
        let outcome = ensure_current_with(&same, &target, || {
            panic!("an equal version must not install")
        })
        .unwrap();
        assert_eq!(outcome.refresh, Refresh::UpToDate);
        assert_eq!(fs::read(&executable).unwrap(), b"first");

        let older_root = tempdir().unwrap();
        let older = versioned_fixture(older_root.path(), "0.50.0", "9", b"older");
        let outcome = ensure_current_with(&older, &target, || {
            panic!("an older version must not install")
        })
        .unwrap();
        assert_eq!(outcome.refresh, Refresh::UpToDate);
        assert_eq!(fs::read(&executable).unwrap(), b"first");

        let newer_root = tempdir().unwrap();
        let newer = versioned_fixture(newer_root.path(), "0.50.0", "11", b"newer");
        let outcome = ensure_current_with(&newer, &target, || {
            install_bundle_at(&newer, &destination).map(|_| ())
        })
        .unwrap();
        assert_eq!(outcome.refresh, Refresh::Update);
        assert_eq!(outcome.bundled, Some(version("0.50.0", "11")));
        assert_eq!(outcome.installed, Some(version("0.50.0", "11")));
        assert_eq!(fs::read(&executable).unwrap(), b"newer");
    }

    #[test]
    fn ensure_current_reports_a_missing_bundled_copy() {
        let root = tempdir().unwrap();
        let result = ensure_current_with(
            &root.path().join(INPUT_SOURCE_BUNDLE_NAME),
            &root.path().join("installed.app"),
            || panic!("nothing to install"),
        );
        assert!(matches!(result, Err(InstallError::SourceUnavailable)));
    }

    #[test]
    fn enabled_list_matches_this_bundle_only() {
        let enabled = br#"[{"InputSourceKind":"Keyboard Layout","KeyboardLayout Name":"ABC"},{"Bundle ID":"app.msime.inputmethod.MetasequoiaIME","Input Mode":"app.msime.inputmethod.MetasequoiaIME.Hans","InputSourceKind":"Input Mode"}]"#;
        assert_eq!(enabled_in_input_source_list(enabled), Some(true));
        let absent = br#"[{"Bundle ID":"com.apple.inputmethod.Kotoeri.RomajiTyping"}]"#;
        assert_eq!(enabled_in_input_source_list(absent), Some(false));
        assert_eq!(enabled_in_input_source_list(b"not json"), None);
    }

    #[test]
    fn launch_services_parser_keeps_only_paths_for_the_requested_identifier() {
        let dump = "path: /old/水杉输入法.app (0x10)\nidentifier:                 app.msime.inputmethod.MetasequoiaIME\npath: /other.app (0x11)\nidentifier:                 com.example.other\npath: /new/水杉输入法.app (0x12)\nidentifier:                 app.msime.inputmethod.MetasequoiaIME\n";
        assert_eq!(
            launch_services_paths_for_identifier(dump, INPUT_SOURCE_BUNDLE_ID),
            vec![
                PathBuf::from("/old/水杉输入法.app"),
                PathBuf::from("/new/水杉输入法.app")
            ]
        );
    }

    // The bundle used to be called 水杉输入法（预览）.app and carries the same identifier, so a
    // machine that installed one would show two entries for a single input source until the old
    // copy went away. Installing removes it; a symlink of that name is left alone, because nothing
    // here ever made one and following it would delete whatever it points at.
    #[test]
    fn installing_removes_a_bundle_left_under_the_old_name() {
        let root = tempdir().unwrap();
        let destination = root.path().join("Library/Input Methods");
        fs::create_dir_all(&destination).unwrap();
        let legacy = destination.join(LEGACY_BUNDLE_NAMES[0]);
        fs::create_dir_all(legacy.join("Contents/MacOS")).unwrap();
        fs::write(legacy.join("Contents/Info.plist"), "old").unwrap();

        let source = fixture(root.path(), INPUT_SOURCE_BUNDLE_ID, b"new");
        let installed = install_bundle_at(&source, &destination).unwrap();
        assert!(installed.exists());
        assert!(!legacy.exists(), "the old bundle is gone");

        // A symlink under that name survives, and so does whatever it points at.
        let elsewhere = root.path().join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &legacy).unwrap();
        let replacement = fixture(root.path(), INPUT_SOURCE_BUNDLE_ID, b"replacement");
        install_bundle_at(&replacement, &destination).unwrap();
        assert!(fs::symlink_metadata(&legacy)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(elsewhere.exists());
    }

    #[test]
    fn installs_atomically_and_preserves_executable_mode() {
        let root = tempdir().unwrap();
        let source = fixture(root.path(), INPUT_SOURCE_BUNDLE_ID, b"new");
        let destination = root.path().join("Library/Input Methods");
        let installed = install_bundle_at(&source, &destination).unwrap();
        assert_eq!(
            fs::read(installed.join(format!("Contents/MacOS/{INPUT_SOURCE_EXECUTABLE}"))).unwrap(),
            b"new"
        );
        assert_eq!(
            fs::metadata(installed.join(format!("Contents/MacOS/{INPUT_SOURCE_EXECUTABLE}")))
                .unwrap()
                .permissions()
                .mode()
                & 0o111,
            0o111
        );

        let replacement = fixture(root.path(), INPUT_SOURCE_BUNDLE_ID, b"replacement");
        install_bundle_at(&replacement, &destination).unwrap();
        assert_eq!(
            fs::read(
                destination
                    .join(INPUT_SOURCE_BUNDLE_NAME)
                    .join(format!("Contents/MacOS/{INPUT_SOURCE_EXECUTABLE}"))
            )
            .unwrap(),
            b"replacement"
        );
        assert!(!destination
            .join(format!(
                ".{INPUT_SOURCE_BUNDLE_NAME}.installing-{}",
                std::process::id()
            ))
            .exists());
    }

    #[test]
    fn rejects_wrong_bundle_without_touching_existing_install() {
        let root = tempdir().unwrap();
        let destination = root.path().join("Library/Input Methods");
        let existing = fixture(root.path(), INPUT_SOURCE_BUNDLE_ID, b"existing");
        install_bundle_at(&existing, &destination).unwrap();
        let wrong = fixture(root.path(), "org.example.other", b"wrong");
        assert!(matches!(
            install_bundle_at(&wrong, &destination),
            Err(InstallError::InvalidBundle)
        ));
        assert_eq!(
            fs::read(
                destination
                    .join(INPUT_SOURCE_BUNDLE_NAME)
                    .join(format!("Contents/MacOS/{INPUT_SOURCE_EXECUTABLE}"))
            )
            .unwrap(),
            b"existing"
        );
    }

    #[test]
    fn rejects_symlinked_bundle_contents() {
        let root = tempdir().unwrap();
        let source = fixture(root.path(), INPUT_SOURCE_BUNDLE_ID, b"new");
        let link = source.join("Contents/Info.plist.link-target");
        fs::write(&link, b"outside").unwrap();
        fs::remove_file(source.join("Contents/Info.plist")).unwrap();
        std::os::unix::fs::symlink(&link, source.join("Contents/Info.plist")).unwrap();
        assert!(matches!(
            validate_bundle(&source),
            Err(InstallError::InvalidBundle)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_bundle_directory_ancestors() {
        use std::os::unix::fs::symlink;

        let root = tempdir().unwrap();
        let source = fixture(root.path(), INPUT_SOURCE_BUNDLE_ID, b"new");
        let contents = source.join("Contents");
        let real_contents = root.path().join("RealContents");
        fs::rename(&contents, &real_contents).unwrap();
        symlink(&real_contents, &contents).unwrap();

        assert!(matches!(
            validate_bundle(&source),
            Err(InstallError::InvalidBundle)
        ));
    }

    // The packaged input method carries Sparkle.framework, whose signature seals its symlinks as links; flattening them breaks the signature and the input method no longer launches.
    #[test]
    fn keeps_framework_symlinks_as_links() {
        let root = tempdir().unwrap();
        let source = fixture(root.path(), INPUT_SOURCE_BUNDLE_ID, b"new");
        let framework = source.join("Contents/Frameworks/Sparkle.framework");
        fs::create_dir_all(framework.join("Versions/B")).unwrap();
        fs::write(framework.join("Versions/B/Sparkle"), b"binary").unwrap();
        std::os::unix::fs::symlink("B", framework.join("Versions/Current")).unwrap();
        std::os::unix::fs::symlink("Versions/Current/Sparkle", framework.join("Sparkle")).unwrap();
        let destination = root.path().join("Library/Input Methods");
        let installed = install_bundle_at(&source, &destination).unwrap();
        let framework = installed.join("Contents/Frameworks/Sparkle.framework");
        assert_eq!(
            fs::read_link(framework.join("Versions/Current")).unwrap(),
            PathBuf::from("B")
        );
        assert_eq!(
            fs::read_link(framework.join("Sparkle")).unwrap(),
            PathBuf::from("Versions/Current/Sparkle")
        );
        assert_eq!(fs::read(framework.join("Sparkle")).unwrap(), b"binary");
    }

    #[test]
    fn rejects_symlinks_that_leave_the_bundle_without_touching_existing_install() {
        let root = tempdir().unwrap();
        let destination = root.path().join("Library/Input Methods");
        let existing = fixture(root.path(), INPUT_SOURCE_BUNDLE_ID, b"existing");
        install_bundle_at(&existing, &destination).unwrap();
        for target in ["../../../outside", "/etc/hosts"] {
            let source_root = tempdir().unwrap();
            let source = fixture(source_root.path(), INPUT_SOURCE_BUNDLE_ID, b"new");
            std::os::unix::fs::symlink(target, source.join("Contents/escape")).unwrap();
            assert!(
                matches!(
                    install_bundle_at(&source, &destination),
                    Err(InstallError::InvalidBundle)
                ),
                "{target}"
            );
        }
        assert_eq!(
            fs::read(
                destination
                    .join(INPUT_SOURCE_BUNDLE_NAME)
                    .join(format!("Contents/MacOS/{INPUT_SOURCE_EXECUTABLE}"))
            )
            .unwrap(),
            b"existing"
        );
    }

    #[test]
    fn only_a_packaged_app_resource_directory_counts() {
        assert!(is_packaged_resource_directory(Path::new(
            "/Applications/MSIME.app/Contents/Resources"
        )));
        assert!(!is_packaged_resource_directory(Path::new(
            "/Users/dev/msime/target/debug"
        )));
        assert!(!is_packaged_resource_directory(Path::new(
            "/Users/dev/msime/target/debug/Resources"
        )));
        assert!(!is_packaged_resource_directory(Path::new(
            "/Users/dev/Other.app/Resources"
        )));
        let result = ensure_current(Path::new("/Users/dev/msime/target/debug"));
        assert!(matches!(result, Err(InstallError::SourceUnavailable)));
    }

    #[test]
    fn first_install_keeps_the_bundle_when_registration_is_pending() {
        let new_root = tempdir().unwrap();
        let destination_root = tempdir().unwrap();
        let new_source = fixture(new_root.path(), INPUT_SOURCE_BUNDLE_ID, b"new");
        let destination = destination_root.path().join("Library/Input Methods");

        let result = install_bundle_at_with_registration(&new_source, &destination, |_| {
            Err(InstallError::Registration)
        });
        assert!(matches!(result, Err(InstallError::RegistrationPending)));
        assert_eq!(
            fs::read(
                destination
                    .join(INPUT_SOURCE_BUNDLE_NAME)
                    .join(format!("Contents/MacOS/{INPUT_SOURCE_EXECUTABLE}"))
            )
            .unwrap(),
            b"new"
        );
        assert!(!destination
            .join(format!(
                ".{INPUT_SOURCE_BUNDLE_NAME}.installing-{}",
                std::process::id()
            ))
            .exists());
    }

    #[test]
    fn registration_failure_restores_previous_install() {
        let old_root = tempdir().unwrap();
        let new_root = tempdir().unwrap();
        let destination_root = tempdir().unwrap();
        let old_source = fixture(old_root.path(), INPUT_SOURCE_BUNDLE_ID, b"old");
        let new_source = fixture(new_root.path(), INPUT_SOURCE_BUNDLE_ID, b"new");
        let destination = destination_root.path().join("Library/Input Methods");
        install_bundle_at(&old_source, &destination).unwrap();

        let result = install_bundle_at_with_registration(&new_source, &destination, |_| {
            Err(InstallError::Registration)
        });
        assert!(matches!(result, Err(InstallError::Registration)));
        assert_eq!(
            fs::read(
                destination
                    .join(INPUT_SOURCE_BUNDLE_NAME)
                    .join(format!("Contents/MacOS/{INPUT_SOURCE_EXECUTABLE}"))
            )
            .unwrap(),
            b"old"
        );
        assert!(!destination
            .join(format!(
                ".{INPUT_SOURCE_BUNDLE_NAME}.previous-{}",
                std::process::id()
            ))
            .exists());
    }
}
