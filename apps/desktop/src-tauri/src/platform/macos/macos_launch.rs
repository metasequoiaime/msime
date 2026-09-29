//! Resolve the same prepared configuration used by the macOS input method.
use serde_json::Value;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAX_OPTIONS_BYTES: u64 = 1024 * 1024;
// The macOS bundle identifier in tauri.macos.conf.json, and therefore the default state directory `app_data_dir` resolves to.
const APPLICATION_ID: &str = "app.msime.macos";
// Earlier identifiers whose default state is copied to APPLICATION_ID on first launch; app.msime.client is the identifier macOS shared with the other desktop platforms until it got its own.
const LEGACY_APPLICATION_IDS: [&str; 3] = [
    "app.msime.client",
    "app.msime.client.preview",
    "app.msime.inputmethod.MetasequoiaIME.settings",
];

pub(crate) struct LaunchState {
    pub options_path: PathBuf,
    pub preferences_directory: PathBuf,
    pub document: Value,
    pub publish_native_locator: bool,
}

#[cfg(test)]
pub(crate) fn resolve(
    application_directory: &Path,
    options_override: Option<OsString>,
    state_override: Option<OsString>,
) -> Result<LaunchState, &'static str> {
    resolve_with_resources(
        application_directory,
        None,
        options_override,
        state_override,
    )
}

pub(crate) fn resolve_with_resources(
    application_directory: &Path,
    resources_directory: Option<&Path>,
    options_override: Option<OsString>,
    state_override: Option<OsString>,
) -> Result<LaunchState, &'static str> {
    if !application_directory.is_absolute() {
        return Err("Application data directory must be absolute");
    }
    let state_directory = state_override.as_deref().map(PathBuf::from);
    if let Some(state_directory) = state_directory.as_ref() {
        if !state_directory.is_absolute() {
            return Err("Runtime preferences directory must be an absolute path");
        }
    }
    let using_default_options = options_override.is_none();
    let options_path = options_override
        .map(PathBuf::from)
        .unwrap_or_else(|| application_directory.join("runtime-options.json"));
    if !options_path.is_absolute() {
        return Err("HostOptions path must be absolute");
    }
    if let Some(parent) = options_path.parent() {
        crate::shared::atomic_file::check_directory_ancestors(parent)
            .map_err(|_| "Cannot read prepared HostOptions JSON")?;
    }
    let options_exists = match fs::symlink_metadata(&options_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err("Cannot read prepared HostOptions JSON");
        }
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => return Err("Cannot read prepared HostOptions JSON"),
    };
    if using_default_options && !options_exists {
        let resources = resources_directory.ok_or("Cannot read prepared HostOptions JSON")?;
        let state_root = state_directory.as_deref().unwrap_or(application_directory);
        prepare_default_options(resources, state_root, &options_path)?;
    }
    if options_exists || options_path.symlink_metadata().is_ok() {
        if options_path
            .symlink_metadata()
            .map_err(|_| "Cannot read prepared HostOptions JSON")?
            .file_type()
            .is_symlink()
        {
            return Err("Cannot read prepared HostOptions JSON");
        }
        refresh_options(&options_path);
    }
    let file =
        std::fs::File::open(&options_path).map_err(|_| "Cannot read prepared HostOptions JSON")?;
    let bytes = read_options_bytes(file)?;
    let document: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Cannot parse prepared HostOptions JSON")?;
    if !document.is_object() {
        return Err("Prepared HostOptions JSON must be an object");
    }
    let preferences_directory = match state_directory {
        Some(value) => value,
        None => match document.get("preferences_directory") {
            None | Some(Value::Null) => application_directory.to_path_buf(),
            Some(Value::String(value)) if value.is_empty() => application_directory.to_path_buf(),
            Some(Value::String(value)) => PathBuf::from(value),
            _ => return Err("Runtime preferences directory must be an absolute path"),
        },
    };
    if !preferences_directory.is_absolute() {
        return Err("Runtime preferences directory must be an absolute path");
    }
    Ok(LaunchState {
        options_path,
        preferences_directory,
        document,
        publish_native_locator: using_default_options,
    })
}

pub(crate) fn native_locator_root() -> Result<PathBuf, &'static str> {
    let home = std::env::var_os("HOME").ok_or("Cannot resolve native HostOptions locator")?;
    let home = PathBuf::from(home);
    if !home.is_absolute() {
        return Err("Cannot resolve native HostOptions locator");
    }
    Ok(home
        .join("Library/Application Support")
        .join(APPLICATION_ID))
}

pub(crate) fn legacy_native_locator_roots() -> Result<Vec<PathBuf>, &'static str> {
    let home = std::env::var_os("HOME").ok_or("Cannot resolve native HostOptions locator")?;
    let home = PathBuf::from(home);
    if !home.is_absolute() {
        return Err("Cannot resolve native HostOptions locator");
    }
    let support = home.join("Library/Application Support");
    Ok(LEGACY_APPLICATION_IDS
        .into_iter()
        .map(|identifier| support.join(identifier))
        .collect())
}

fn read_options(path: &Path) -> Option<Value> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_OPTIONS_BYTES
    {
        return None;
    }
    let file = fs::File::open(path).ok()?;
    let bytes = read_options_bytes(file).ok()?;
    serde_json::from_slice::<Value>(&bytes)
        .ok()
        .filter(Value::is_object)
}

fn read_options_bytes(file: impl Read) -> Result<Vec<u8>, &'static str> {
    match crate::shared::bounded_body::read_bounded(file, MAX_OPTIONS_BYTES as usize) {
        Ok(bytes) => Ok(bytes),
        Err(crate::shared::bounded_body::BoundedReadError::TooLarge) => {
            Err("Prepared HostOptions JSON exceeds size limit")
        }
        Err(crate::shared::bounded_body::BoundedReadError::Read(_)) => {
            Err("Cannot read prepared HostOptions JSON")
        }
    }
}

fn copy_legacy_entry(source: &Path, destination: &Path) -> Result<(), &'static str> {
    let metadata =
        fs::symlink_metadata(source).map_err(|_| "Cannot migrate legacy application data")?;
    if metadata.file_type().is_symlink() {
        return Err("Cannot migrate legacy application data");
    }
    if metadata.is_dir() {
        fs::create_dir(destination).map_err(|_| "Cannot migrate legacy application data")?;
        for entry in fs::read_dir(source).map_err(|_| "Cannot migrate legacy application data")? {
            let entry = entry.map_err(|_| "Cannot migrate legacy application data")?;
            copy_legacy_entry(&entry.path(), &destination.join(entry.file_name()))?;
        }
        fs::set_permissions(destination, metadata.permissions())
            .map_err(|_| "Cannot migrate legacy application data")?;
        return Ok(());
    }
    if !metadata.is_file() {
        return Err("Cannot migrate legacy application data");
    }
    fs::copy(source, destination).map_err(|_| "Cannot migrate legacy application data")?;
    fs::set_permissions(destination, metadata.permissions())
        .map_err(|_| "Cannot migrate legacy application data")?;
    Ok(())
}

fn copy_legacy_state(source: &Path, destination: &Path) -> Result<(), &'static str> {
    let metadata =
        fs::symlink_metadata(source).map_err(|_| "Cannot migrate legacy application data")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Cannot migrate legacy application data");
    }
    let parent = destination
        .parent()
        .ok_or("Cannot migrate legacy application data")?;
    crate::shared::atomic_file::create_directory_and_check(parent)
        .map_err(|_| "Cannot migrate legacy application data")?;
    let staging = tempfile::Builder::new()
        .prefix(".msime-client-migration-")
        .tempdir_in(parent)
        .map_err(|_| "Cannot migrate legacy application data")?;
    for entry in fs::read_dir(source).map_err(|_| "Cannot migrate legacy application data")? {
        let entry = entry.map_err(|_| "Cannot migrate legacy application data")?;
        if entry.file_name() == "runtime-options.json" {
            continue;
        }
        copy_legacy_entry(&entry.path(), &staging.path().join(entry.file_name()))?;
    }
    if destination.exists() {
        let mut entries =
            fs::read_dir(destination).map_err(|_| "Cannot migrate legacy application data")?;
        if entries.next().is_some() {
            return Ok(());
        }
        fs::remove_dir(destination).map_err(|_| "Cannot migrate legacy application data")?;
    }
    let staging = staging.keep();
    fs::rename(staging, destination).map_err(|_| "Cannot migrate legacy application data")
}

/// Move the active default state off historical application identifiers before first use of the
/// canonical directory. An explicitly moved data directory remains where the user chose it: only
/// its small locator is copied. Legacy default data is copied (not deleted) so a failed downgrade
/// still has a complete source, then HostOptions is rebuilt with canonical absolute paths.
pub(crate) fn migrate_legacy_application_data(
    application_directory: &Path,
    resources_directory: &Path,
    legacy_roots: &[PathBuf],
) -> Result<bool, &'static str> {
    if application_directory.exists()
        && fs::read_dir(application_directory)
            .map_err(|_| "Cannot inspect application data directory")?
            .next()
            .is_some()
    {
        return Ok(false);
    }
    for legacy_root in legacy_roots {
        let options = legacy_root.join("runtime-options.json");
        let Some(document) = read_options(&options) else {
            continue;
        };
        let Some(preferences) = document
            .get("preferences_directory")
            .and_then(Value::as_str)
        else {
            continue;
        };
        let preferences = PathBuf::from(preferences);
        if !preferences.is_absolute() {
            continue;
        }
        if !legacy_roots.iter().any(|root| root == &preferences) {
            return Ok(recover_default_options(application_directory, &options));
        }
        copy_legacy_state(&preferences, application_directory)?;
        let document =
            msime_host_api::prepare_host_configuration(resources_directory, application_directory)
                .map_err(|_| "Cannot migrate legacy application data")?;
        let document: Value = serde_json::from_str(&document)
            .map_err(|_| "Cannot migrate legacy application data")?;
        replace_options(
            &application_directory.join("runtime-options.json"),
            &document,
        )?;
        return Ok(true);
    }
    Ok(false)
}

pub(crate) fn publish_native_options(document: &Value) -> Result<PathBuf, &'static str> {
    let path = native_locator_root()?.join("runtime-options.json");
    replace_options(&path, document)?;
    Ok(path)
}

/// Restore the settings bundle's locator from the IMK bundle's copy after a settings-app reinstall.
/// A malformed, oversized or symlinked native locator is ignored and normal first-run preparation
/// takes over; it is never allowed to choose a relative state path.
pub(crate) fn recover_default_options(application_directory: &Path, native_options: &Path) -> bool {
    let local = application_directory.join("runtime-options.json");
    if local.exists() {
        return false;
    }
    let Ok(metadata) = std::fs::symlink_metadata(native_options) else {
        return false;
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    let Ok(file) = std::fs::File::open(native_options) else {
        return false;
    };
    let Ok(bytes) = read_options_bytes(file) else {
        return false;
    };
    let Ok(document) = serde_json::from_slice::<Value>(&bytes) else {
        return false;
    };
    let Some(preferences) = document
        .get("preferences_directory")
        .and_then(Value::as_str)
    else {
        return false;
    };
    if !document.is_object()
        || !Path::new(preferences).is_absolute()
        || ["resources", "user_data", "cache", "dictionaries"]
            .into_iter()
            .any(|key| {
                !document
                    .get(key)
                    .and_then(Value::as_str)
                    .is_some_and(|path| Path::new(path).is_absolute())
            })
    {
        return false;
    }
    replace_options(&local, &document).is_ok()
}

/// Bring options written before an app upgrade up to the dictionary generation this build's lock describes, the counterpart of the user-dictionary replay the Windows installer runs on every upgrade: the Host API prepares the new generation, replays the user journal into it and atomically rewrites only `resources` and `dictionaries`. This runs before any session exists. Symlinks and documents outside the prepared layout are left alone by the Host API. A failure keeps the previous generation in use and the next launch tries again; the error is not printed because it can name private paths.
fn refresh_options(options_path: &Path) {
    if msime_host_api::refresh_host_options(options_path).is_err() {
        eprintln!(
            "Cannot update the dictionary to the installed generation; keeping the current one"
        );
    }
}

fn prepare_default_options(
    resources_directory: &Path,
    state_root: &Path,
    options_path: &Path,
) -> Result<(), &'static str> {
    crate::shared::atomic_file::create_directory_and_check(state_root)
        .map_err(|_| "Cannot prepare default HostOptions JSON")?;
    let document = msime_host_api::prepare_host_configuration(resources_directory, state_root)
        .map_err(|_| "Cannot prepare default HostOptions JSON")?;
    let document: Value =
        serde_json::from_str(&document).map_err(|_| "Cannot prepare default HostOptions JSON")?;
    if !document.is_object() {
        return Err("Cannot prepare default HostOptions JSON");
    }
    publish_options(options_path, &document)
}

fn publish_options(options_path: &Path, document: &Value) -> Result<(), &'static str> {
    let parent = options_path
        .parent()
        .ok_or("Cannot prepare default HostOptions JSON")?;
    crate::shared::atomic_file::create_directory_and_check(parent)
        .map_err(|_| "Cannot prepare default HostOptions JSON")?;
    let serialized = serde_json::to_vec_pretty(document)
        .map_err(|_| "Cannot prepare default HostOptions JSON")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|_| "Cannot prepare default HostOptions JSON")?;
    temporary
        .write_all(&serialized)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|_| "Cannot prepare default HostOptions JSON")?;
    match temporary.persist_noclobber(options_path) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(_) => Err("Cannot prepare default HostOptions JSON"),
    }
}

pub(crate) fn replace_options(options_path: &Path, document: &Value) -> Result<(), &'static str> {
    let parent = options_path
        .parent()
        .ok_or("Cannot publish prepared HostOptions JSON")?;
    crate::shared::atomic_file::create_directory_and_check(parent)
        .map_err(|_| "Cannot publish prepared HostOptions JSON")?;
    let serialized = serde_json::to_vec_pretty(document)
        .map_err(|_| "Cannot publish prepared HostOptions JSON")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|_| "Cannot publish prepared HostOptions JSON")?;
    temporary
        .write_all(&serialized)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|_| "Cannot publish prepared HostOptions JSON")?;
    temporary
        .persist(options_path)
        .map(|_| ())
        .map_err(|_| "Cannot publish prepared HostOptions JSON")
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_core::preferences::PreferencesStore;
    use serde_json::json;

    #[test]
    fn finder_launch_uses_native_default_configuration_without_environment() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("runtime-options.json");
        let document = json!({"api_version":1,"resources":"synthetic-resources"});
        std::fs::write(&path, document.to_string()).unwrap();
        let launch = resolve(root.path(), None, None).unwrap();
        assert_eq!(launch.options_path, path);
        assert_eq!(launch.preferences_directory, root.path());
        assert_eq!(launch.document, document);
        assert!(launch.publish_native_locator);
    }

    #[test]
    fn missing_configuration_is_published_atomically_without_overwriting_existing_state() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("runtime-options.json");
        let first = json!({"api_version":1,"resources":"first"});
        let second = json!({"api_version":1,"resources":"second"});
        publish_options(&path, &first).unwrap();
        publish_options(&path, &second).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(path).unwrap()).unwrap(),
            first
        );
    }

    #[test]
    fn explicit_options_override_never_triggers_default_preparation() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path().join("app");
        let explicit = root.path().join("explicit-runtime-options.json");
        let missing_resources = root.path().join("missing-resources");
        assert!(resolve_with_resources(
            &app,
            Some(&missing_resources),
            Some(explicit.into_os_string()),
            None,
        )
        .is_err());
        assert!(!app.exists());
    }

    #[test]
    fn native_launch_saves_to_the_hosts_preferences_directory() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path().join("app");
        let state = root.path().join("共享 配置");
        let path = root.path().join("bundled-runtime-options.json");
        std::fs::write(&path, json!({"preferences_directory":state}).to_string()).unwrap();
        let launch = resolve(&app, Some(path.clone().into_os_string()), None).unwrap();
        let store = PreferencesStore::new(&launch.preferences_directory);
        let initial = store.load().unwrap();
        let mut preferences = initial.preferences;
        preferences.candidate_page_size = 7;
        store.save(initial.revision, preferences).unwrap();
        let native = PreferencesStore::new(&state).load().unwrap();
        assert_eq!(native.preferences.candidate_page_size, 7);
        assert!(!app.exists(), "must not create a second preferences store");
        assert_eq!(launch.options_path, path);
    }

    #[test]
    fn explicit_state_override_does_not_change_the_selected_options_file() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("runtime-options.json");
        std::fs::write(
            &path,
            json!({"preferences_directory":root.path().join("native")}).to_string(),
        )
        .unwrap();
        let override_path = root.path().join("explicit-state");
        let launch = resolve(
            root.path(),
            None,
            Some(override_path.clone().into_os_string()),
        )
        .unwrap();
        assert_eq!(launch.preferences_directory, override_path);
        assert_eq!(launch.options_path, path);
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlinked_options_file() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let application = root.path().join("app");
        std::fs::create_dir(&application).unwrap();
        let outside = root.path().join("outside-options.json");
        std::fs::write(
            &outside,
            json!({"preferences_directory": root.path().join("outside-state")}).to_string(),
        )
        .unwrap();
        symlink(&outside, application.join("runtime-options.json")).unwrap();

        assert!(resolve(&application, None, None).is_err());
        assert!(!root.path().join("outside-state").exists());
    }

    #[test]
    fn bad_explicit_paths_never_fall_back_to_another_store() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("runtime-options.json");
        std::fs::write(&path, "{}").unwrap();
        for value in [OsString::new(), OsString::from("relative.json")] {
            assert!(resolve(root.path(), Some(value.clone()), None).is_err());
            assert!(resolve(root.path(), None, Some(value)).is_err());
        }
        assert!(resolve(
            root.path(),
            Some(root.path().join("missing").into_os_string()),
            None
        )
        .is_err());
        for value in [json!("relative"), json!(42), json!([])] {
            std::fs::write(&path, json!({"preferences_directory":value}).to_string()).unwrap();
            assert!(resolve(root.path(), None, None).is_err());
        }
    }

    #[test]
    fn invalid_documents_are_bounded_and_errors_do_not_expose_contents_or_paths() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("runtime-options.json");
        for bytes in [
            "synthetic-private-config".to_owned(),
            "[]".into(),
            "x".repeat(MAX_OPTIONS_BYTES as usize + 1),
        ] {
            std::fs::write(&path, bytes).unwrap();
            let error = resolve(root.path(), None, None).err().unwrap();
            assert!(!error.contains("synthetic-private-config"));
            assert!(!error.contains(&root.path().to_string_lossy().to_string()));
        }
    }

    #[test]
    fn replace_options_atomically_updates_a_locator() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("native/runtime-options.json");
        replace_options(&path, &json!({"preferences_directory":"/synthetic/old"})).unwrap();
        replace_options(&path, &json!({"preferences_directory":"/synthetic/new"})).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(path).unwrap()).unwrap()
                ["preferences_directory"],
            "/synthetic/new"
        );
    }

    #[cfg(unix)]
    #[test]
    fn replace_options_refuses_a_symlinked_parent() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let linked = root.path().join("redirect");
        symlink(outside.path(), &linked).unwrap();
        let path = linked.join("runtime-options.json");

        assert_eq!(
            replace_options(&path, &json!({"synthetic": true})),
            Err("Cannot publish prepared HostOptions JSON")
        );
        assert!(!outside.path().join("runtime-options.json").exists());
    }

    #[test]
    fn missing_settings_locator_recovers_only_a_valid_native_absolute_state() {
        let root = tempfile::tempdir().unwrap();
        let application = root.path().join("settings");
        let native = root.path().join("native/runtime-options.json");
        replace_options(
            &native,
            &json!({
                "preferences_directory":"/synthetic/preserved-state",
                "resources":"/synthetic/resources",
                "user_data":"/synthetic/preserved-state/user",
                "cache":"/synthetic/preserved-state/cache",
                "dictionaries":"/synthetic/preserved-state/dictionaries"
            }),
        )
        .unwrap();
        assert!(recover_default_options(&application, &native));
        assert_eq!(
            serde_json::from_slice::<Value>(
                &std::fs::read(application.join("runtime-options.json")).unwrap()
            )
            .unwrap()["preferences_directory"],
            "/synthetic/preserved-state"
        );
        std::fs::remove_file(application.join("runtime-options.json")).unwrap();
        replace_options(&native, &json!({"preferences_directory":"relative"})).unwrap();
        assert!(!recover_default_options(&application, &native));
        assert!(!application.join("runtime-options.json").exists());
    }

    #[test]
    fn legacy_external_state_keeps_its_location_but_moves_the_locator() {
        let root = tempfile::tempdir().unwrap();
        let application = root.path().join("app.msime.macos");
        let legacy = root.path().join("app.msime.client");
        let external = root.path().join("external-state");
        replace_options(
            &legacy.join("runtime-options.json"),
            &json!({
                "preferences_directory":external,
                "resources":root.path().join("resources"),
                "user_data":external.join("user"),
                "cache":external.join("cache"),
                "dictionaries":external.join("dictionaries")
            }),
        )
        .unwrap();
        assert!(migrate_legacy_application_data(
            &application,
            &root.path().join("unused-resources"),
            std::slice::from_ref(&legacy),
        )
        .unwrap());
        assert_eq!(
            serde_json::from_slice::<Value>(
                &fs::read(application.join("runtime-options.json")).unwrap()
            )
            .unwrap()["preferences_directory"],
            external.to_string_lossy().as_ref()
        );
    }

    #[test]
    fn legacy_default_copy_preserves_state_but_drops_stale_locator() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("app.msime.client");
        let application = root.path().join("app.msime.macos");
        fs::create_dir_all(legacy.join("skins/sample")).unwrap();
        fs::write(legacy.join("preferences.json"), b"synthetic-preferences").unwrap();
        fs::write(legacy.join("skins/sample/skin.toml"), b"schema = 1").unwrap();
        fs::write(legacy.join("runtime-options.json"), b"stale absolute paths").unwrap();

        copy_legacy_state(&legacy, &application).unwrap();

        assert_eq!(
            fs::read(application.join("preferences.json")).unwrap(),
            b"synthetic-preferences"
        );
        assert_eq!(
            fs::read(application.join("skins/sample/skin.toml")).unwrap(),
            b"schema = 1"
        );
        assert!(!application.join("runtime-options.json").exists());
        assert!(legacy.join("runtime-options.json").exists());
    }

    fn prepared_layout(root: &Path, resources: &Path, generation: &str) -> Value {
        let state = root.join("state");
        json!({
            "api_version":1,
            "resources":resources,
            "user_data":state.join("user"),
            "cache":state.join("cache"),
            "dictionaries":state.join("user/dictionaries").join(generation),
            "preferences_directory":state,
            "online_provider_socket":"/synthetic/provider.sock"
        })
    }

    #[test]
    fn a_stale_generation_that_cannot_be_refreshed_keeps_the_file_and_still_launches() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("runtime-options.json");
        let document = prepared_layout(
            root.path(),
            &root.path().join("missing-resources"),
            "previous-generation",
        );
        let bytes = serde_json::to_vec_pretty(&document).unwrap();
        fs::write(&path, &bytes).unwrap();
        let launch = resolve(root.path(), None, None).unwrap();
        assert_eq!(launch.document, document);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(launch.preferences_directory, root.path().join("state"));
    }

    #[test]
    fn options_already_on_the_installed_generation_are_not_rewritten() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("runtime-options.json");
        let specification: msime_client_core::resources::ResourceSet = serde_json::from_str(
            include_str!("../../../../../../resources/desktop-dictionary.lock.json"),
        )
        .unwrap();
        let document = prepared_layout(
            root.path(),
            &root.path().join("missing-resources"),
            &specification.generation().unwrap(),
        );
        let bytes = serde_json::to_vec_pretty(&document).unwrap();
        fs::write(&path, &bytes).unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let launch = resolve(root.path(), None, None).unwrap();
        assert_eq!(launch.document, document);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    }
}
