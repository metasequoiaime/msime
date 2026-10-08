//! Resolve the same prepared configuration used by the macOS input method.
use serde_json::Value;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAX_OPTIONS_BYTES: u64 = 1024 * 1024;

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
        refresh_options(&options_path, resources_directory);
    }
    let file = crate::shared::atomic_file::open_private(&options_path)
        .map_err(|_| "Cannot read prepared HostOptions JSON")?;
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
    // 设置应用的 bundle identifier（full 是 tauri.macos.conf.json 里的 app.msime.macos），也就是 `app_data_dir` 解析出的默认状态目录；随版本而变，同时安装的版本各用各的目录。
    Ok(home
        .join("Library/Application Support")
        .join(&crate::platform::macos::macos_identity().settings_bundle_id))
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

pub(crate) fn publish_native_options(document: &Value) -> Result<PathBuf, &'static str> {
    let path = native_locator_root()?.join("runtime-options.json");
    replace_options(&path, document)?;
    Ok(path)
}

/// 把应用升级前写下的配置带到本构建词库锁描述的代次，对应 Windows 安装程序每次升级时做的用户词库回放：Host API 准备新代次、把用户词库日志回放进去，并原子地只改写 `resources` 与 `dictionaries`。它在任何会话建立之前运行。符号链接和不符合准备布局的文件由 Host API 原样保留。失败时继续用旧代次，下次启动再试；错误不打印，因为其中可能有私人路径。
///
/// 打包的应用传入 bundle 内的 `EngineResources`：记录的资源目录是没有安装包会升级的副本（手工暂存到 Application Support，或在输入法「准备词库」里选的目录）且已与词库锁不符时，改从 bundle 准备代次，此后配置指向 bundle，bundle 里的 `language-dictionaries` 也就成了记录的资源目录的同级目录，由输入法自己的刷新记入 `language_dictionaries`。开发运行的资源目录是随时可能被清掉的 cargo 产物，不会这样记录。
fn refresh_options(options_path: &Path, bundled_resources: Option<&Path>) {
    let refreshed = match packaged_resources(bundled_resources) {
        Some(bundled) => msime_host_api::refresh_host_options_from(options_path, bundled),
        None => msime_host_api::refresh_host_options(options_path),
    };
    if refreshed.is_err() {
        eprintln!(
            "Cannot update the dictionary to the installed generation; keeping the current one"
        );
    }
}

/// `resources` 是打包应用 `Contents/Resources` 下的目录时原样返回，开发运行的 cargo 产物目录返回 `None`。
fn packaged_resources(resources: Option<&Path>) -> Option<&Path> {
    resources.filter(|resources| {
        resources
            .parent()
            .is_some_and(super::macos_input_source::is_packaged_resource_directory)
    })
}

fn prepare_default_options(
    resources_directory: &Path,
    state_root: &Path,
    options_path: &Path,
) -> Result<(), &'static str> {
    crate::shared::atomic_file::create_directory_and_check(state_root)
        .map_err(|_| "Cannot prepare default HostOptions JSON")?;
    // 还没有 HostOptions 文档时，版本取自状态目录里留下的记录（例如定位文件被删掉而状态还在）；没有记录时是本设置应用所属的版本（安装包的版本声明，full 的包没有声明）。
    let edition = msime_client_core::edition::Edition::recorded_in(state_root)
        .unwrap_or_else(crate::platform::macos::macos_edition);
    let document = msime_host_api::prepare_host_configuration_for_edition(
        resources_directory,
        state_root,
        edition,
    )
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

    /// 升级时只有打包应用自带的 `EngineResources` 能顶替过期的资源目录；开发运行的资源目录是 cargo 产物，不能写进用户的配置。
    #[test]
    fn only_a_packaged_bundle_replaces_outdated_resources() {
        let packaged = Path::new("/Applications/MSIME.app/Contents/Resources/EngineResources");
        assert_eq!(packaged_resources(Some(packaged)), Some(packaged));
        for development in [
            Path::new("/repo/target/debug/EngineResources"),
            Path::new("/repo/target/release/bundle/Resources/EngineResources"),
        ] {
            assert_eq!(packaged_resources(Some(development)), None);
        }
        assert_eq!(packaged_resources(None), None);
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
