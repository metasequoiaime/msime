//! 插件 page commands for the macOS, Windows and Linux shells: the plugin packs installed under the state directory, the built-in sound packs the bundle ships, and the @ mode's name list.
//!
//! Every rule lives in `msime_client_core::plugins`; these are shims that resolve the roots host-side and hand them over. The page never names a path: it asks for the catalog, asks the host to show its own picker for an import, and names a pack to remove by kind and id. The input processes read the same `<state>/plugins` directory (`preferences_directory/plugins` in host-api), so an import or a name-list save reaches them without any notification of its own: sound and music settings travel through the preferences document, and the command tables and the name list are reread when a field gains focus after their files changed.

use crate::DictionaryHostOptions;
use msime_client_core::plugins::mentions::{MentionEntry, MentionStore};
use msime_client_core::plugins::{self, PluginCatalog, PluginFailure, PluginSummary};
use std::path::{Path, PathBuf};
use tauri::State;
use tauri_plugin_dialog::DialogExt;

/// Where installed packs and `mentions.json` live: `plugins` under the state directory the preferences document is in.
pub(crate) struct PluginsState(PathBuf);

impl PluginsState {
    pub(crate) fn new(state_directory: &Path) -> Self {
        Self(state_directory.join("plugins"))
    }

    /// The plugins root, for the community commands that publish from and install into it.
    pub(crate) fn root(&self) -> &Path {
        &self.0
    }
}

/// What the page asks the picker for. A native dialog chooses either files or folders, not both, so the page offers one button for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginImportSource {
    Folder,
    Archive,
}

/// The directory holding the built-in sound packs this installation ships, when it is there.
///
/// Each platform stages them beside its resources rather than inside, because the resource directory has to hold exactly what the dictionary lock pins: the macOS input method bundle's `Contents/Resources/sound-packs` (this app carries that bundle among its resources), the Windows data directory's `sound-packs`, and on Linux `share/msime-client/sound-packs` of the prefix this executable is installed in (`<prefix>/bin`), falling back to the one next to the `resources` the host options name. The installed prefix comes first because the host options name the user's own data directory once a newer dictionary has been downloaded, and no packs sit beside that one. A development run without them lists the installed packs only.
fn builtin_sound_packs(
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] app: &tauri::AppHandle,
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] options: &DictionaryHostOptions,
) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    let directory = {
        use tauri::Manager;
        app.path().resource_dir().ok().map(|resources| {
            resources
                .join(crate::macos_input_source::input_source_bundle_name())
                .join("Contents/Resources/sound-packs")
        })
    };
    #[cfg(target_os = "windows")]
    let directory =
        msime_host_windows::server_state_directory().map(|directory| directory.join("sound-packs"));
    #[cfg(target_os = "linux")]
    let directory = std::env::current_exe()
        .ok()
        .and_then(|executable| linux_installed_sound_packs(&executable))
        .filter(|directory| directory.is_dir())
        .or_else(|| {
            options.snapshot().ok().and_then(|document| {
                linux_builtin_sound_packs(
                    document
                        .get("resources")
                        .and_then(serde_json::Value::as_str)?,
                )
            })
        });
    directory.filter(|directory| directory.is_dir())
}

/// `<prefix>/share/msime-client/sound-packs` for an executable installed as `<prefix>/bin/<name>`, where the Linux package installs the built-in packs. 目录名随本安装包所属的版本（full 是 `msime-client`）。
#[cfg(any(target_os = "linux", test))]
fn linux_installed_sound_packs(executable: &Path) -> Option<PathBuf> {
    Some(
        executable
            .parent()?
            .parent()?
            .join("share")
            .join(
                &msime_client_core::edition::Edition::linux_package_identity_or_full()
                    .client_directory,
            )
            .join("sound-packs"),
    )
}

/// `<parent of resources>/sound-packs`, the rule host-api applies when a host passes no `sound_packs`.
#[cfg(any(target_os = "linux", test))]
fn linux_builtin_sound_packs(resources: &str) -> Option<PathBuf> {
    let resources = Path::new(resources);
    resources
        .is_absolute()
        .then(|| resources.parent().map(|parent| parent.join("sound-packs")))
        .flatten()
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, PluginFailure> + Send + 'static,
) -> Result<T, PluginFailure> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| PluginFailure::code("storage"))?
}

/// Every installed and built-in pack, with what is wrong with each folder that is not one.
#[tauri::command]
pub async fn plugin_catalog(
    app: tauri::AppHandle,
    state: State<'_, PluginsState>,
    options: State<'_, DictionaryHostOptions>,
) -> Result<PluginCatalog, PluginFailure> {
    let root = state.0.clone();
    let options = options.inner().clone();
    blocking(move || {
        Ok(plugins::scan(
            &root,
            builtin_sound_packs(&app, &options).as_deref(),
        ))
    })
    .await
}

/// Ask the user for a pack folder or `.zip` file with the platform's own dialog, and install it. `None` when the user closes the dialog.
#[tauri::command]
pub async fn import_plugin_pack(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, PluginsState>,
    source: PluginImportSource,
) -> Result<Option<PluginSummary>, PluginFailure> {
    let root = state.0.clone();
    blocking(move || {
        let dialog = app.dialog().file().set_parent(&window);
        // The dialog runs on the main thread; this worker only waits for the answer.
        let picked = match source {
            PluginImportSource::Folder => dialog.set_title("选择插件文件夹").blocking_pick_folder(),
            PluginImportSource::Archive => dialog
                .set_title("选择插件")
                .add_filter("插件", &["zip"])
                .blocking_pick_file(),
        };
        let Some(picked) = picked else {
            return Ok(None);
        };
        let path = picked
            .into_path()
            .map_err(|_| PluginFailure::code("plugin_unsupported_source"))?;
        import_pack_at(&path, &root).map(Some)
    })
    .await
}

fn import_pack_at(source: &Path, root: &Path) -> Result<PluginSummary, PluginFailure> {
    Ok(plugins::import(source, root)?)
}

/// Remove an installed pack. A built-in sound pack cannot be removed.
#[tauri::command]
pub async fn remove_plugin_pack(
    state: State<'_, PluginsState>,
    kind: String,
    id: String,
) -> Result<(), PluginFailure> {
    let root = state.0.clone();
    blocking(move || plugins::remove_named(&root, &kind, &id)).await
}

/// The @ mode's name list, empty before one was saved.
#[tauri::command]
pub async fn load_plugin_mentions(
    state: State<'_, PluginsState>,
) -> Result<Vec<MentionEntry>, PluginFailure> {
    let root = state.0.clone();
    blocking(move || Ok(MentionStore::new(root).load()?)).await
}

/// Replace the @ mode's name list. It stays on this machine: it is not part of the preferences document account sync reads.
#[tauri::command]
pub async fn save_plugin_mentions(
    state: State<'_, PluginsState>,
    entries: Vec<MentionEntry>,
) -> Result<(), PluginFailure> {
    let root = state.0.clone();
    blocking(move || Ok(MentionStore::new(root).save(&entries)?)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_core::plugins::PluginKind;
    use std::fs;

    fn write_command_table(directory: &Path, id: &str) {
        fs::create_dir_all(directory).unwrap();
        fs::write(
            directory.join("plugin.toml"),
            format!(
                "schema_version = 1\nkind = \"command_table\"\nid = \"{id}\"\nname = \"签名\"\nversion = \"1.0.0\"\nlicense = \"CC0-1.0\"\n\n[[commands]]\ntrigger = \"sig\"\ntitle = \"签名\"\ntemplate = \"{{date}} 测试\"\n"
            ),
        )
        .unwrap();
    }

    #[test]
    fn imports_lists_and_removes_a_pack_under_the_state_directory() {
        let state = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let pack = source.path().join("signature");
        write_command_table(&pack, "signature");
        let root = PluginsState::new(state.path()).0;

        let imported = import_pack_at(&pack, &root).unwrap();
        assert_eq!(imported.id, "signature");
        assert_eq!(imported.kind(), PluginKind::CommandTable);
        assert!(root.join("command_table/signature/plugin.toml").is_file());
        let catalog = plugins::scan(&root, None);
        assert_eq!(catalog.packages.len(), 1);

        plugins::remove_named(&root, "command_table", "signature").unwrap();
        assert!(plugins::scan(&root, None).packages.is_empty());
    }

    #[test]
    fn refusals_carry_the_rule_that_was_broken() {
        let state = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let pack = source.path().join("broken");
        fs::create_dir_all(&pack).unwrap();
        fs::write(pack.join("plugin.toml"), "schema_version = 2\n").unwrap();
        let root = PluginsState::new(state.path()).0;

        let error = import_pack_at(&pack, &root).unwrap_err();
        assert_eq!(error.code, "plugin_invalid");
        assert!(error
            .detail
            .as_deref()
            .is_some_and(|detail| !detail.is_empty()));
        assert_eq!(
            import_pack_at(&source.path().join("missing.txt"), &root).unwrap_err(),
            PluginFailure::code("plugin_unsupported_source")
        );
    }

    #[test]
    fn linux_built_in_packs_sit_beside_the_resource_directory() {
        assert_eq!(
            linux_builtin_sound_packs("/usr/share/msime-client/resources"),
            Some(PathBuf::from("/usr/share/msime-client/sound-packs"))
        );
        assert_eq!(linux_builtin_sound_packs("resources"), None);
        assert_eq!(linux_builtin_sound_packs(""), None);
    }

    #[test]
    fn linux_built_in_packs_follow_the_installed_prefix() {
        assert_eq!(
            linux_installed_sound_packs(Path::new("/usr/bin/msime-linux-desktop")),
            Some(PathBuf::from("/usr/share/msime-client/sound-packs"))
        );
        assert_eq!(
            linux_installed_sound_packs(Path::new("/opt/msime/bin/msime-linux-desktop")),
            Some(PathBuf::from("/opt/msime/share/msime-client/sound-packs"))
        );
        assert_eq!(linux_installed_sound_packs(Path::new("/")), None);
    }

    #[test]
    fn import_sources_use_the_page_spelling() {
        assert_eq!(
            serde_json::from_str::<PluginImportSource>("\"folder\"").unwrap(),
            PluginImportSource::Folder
        );
        assert_eq!(
            serde_json::from_str::<PluginImportSource>("\"archive\"").unwrap(),
            PluginImportSource::Archive
        );
        assert!(serde_json::from_str::<PluginImportSource>("\"path\"").is_err());
    }
}
