//! macOS、Windows 和 Linux 外壳上「插件」页的命令：装在状态目录下的插件包、应用包随带的内置音效包，以及 @ 模式的名单。
//!
//! 所有规则都在 `msime_client_services::plugins` 里；这些只是垫片，在宿主这边解析出各个根目录再交给它。页面从不指明路径：它要目录清单，导入时请宿主弹出自己的选择框，删除时按类别和 id 指明是哪个包。输入进程读的是同一个 `<state>/plugins` 目录（host-api 里的 `preferences_directory/plugins`），所以导入或保存名单不用另发通知就能到达它们：音效和音乐设置经偏好文档传递，命令表和名单在文件变动后、某个输入框获得焦点时重新读取。

use crate::DictionaryHostOptions;
use msime_client_core::preferences::PreferencesStore;
use msime_client_services::plugins::mentions::{MentionEntry, MentionStore};
use msime_client_services::plugins::script_grants::{self, ScriptGrantsView};
use msime_client_services::plugins::{
    self, script_commands, BuiltinDirs, PluginCatalog, PluginFailure, PluginSummary,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
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

/// 桌面安装包随带的官方插件包目录（`<kind>/<id>/`），在才用；开发运行没有它时只列已安装的包，官方包列不出来也授权不了。
///
/// macOS 和 Linux 放在内置音效包目录旁边的 `plugin-packs`（输入法 bundle 的 `Contents/Resources/plugin-packs`、`share/<client>/plugin-packs`），和 host-api 不给 `plugin_packs` 时的推导相同。Windows 放在本程序所在的 Server 目录（Program Files 下，只有管理员能写）的 `plugin-packs`，不放音效包所在的数据目录：那里普通用户可写，与 Server 写进会话选项的 `plugin_packs` 是同一个目录。
fn builtin_plugin_packs(
    #[cfg_attr(target_os = "windows", allow(unused_variables))] app: &tauri::AppHandle,
    #[cfg_attr(target_os = "windows", allow(unused_variables))] options: &DictionaryHostOptions,
) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let directory = std::env::current_exe()
        .ok()
        .and_then(|executable| Some(executable.parent()?.join("plugin-packs")));
    #[cfg(not(target_os = "windows"))]
    let directory = builtin_sound_packs(app, options)
        .and_then(|sounds| Some(sounds.parent()?.join("plugin-packs")));
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

/// 已安装的包和内置包，以及每个不是包的文件夹出了什么问题。脚本插件也列出（带声明的钩子、申请的能力和内容哈希），页面只在宿主声明了 `script_plugins` 时展示它们。
#[tauri::command]
pub async fn plugin_catalog(
    app: tauri::AppHandle,
    state: State<'_, PluginsState>,
    options: State<'_, DictionaryHostOptions>,
) -> Result<PluginCatalog, PluginFailure> {
    let root = state.0.clone();
    let options = options.inner().clone();
    blocking(move || {
        let sounds = builtin_sound_packs(&app, &options);
        let packs = builtin_plugin_packs(&app, &options);
        Ok(plugins::scan(
            &root,
            BuiltinDirs {
                sounds: sounds.as_deref(),
                packs: packs.as_deref(),
            },
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
    Ok(plugins::import_from_settings(source, root)?)
}

/// 删除一个已安装的包。内置音效包不能删除；官方包的 id 只删掉留在插件目录里的导入副本（client-services 的 `remove`）。
#[tauri::command]
pub async fn remove_plugin_pack(
    state: State<'_, PluginsState>,
    kind: String,
    id: String,
) -> Result<(), PluginFailure> {
    let root = state.0.clone();
    blocking(move || plugins::remove_named(&root, &kind, &id)).await
}

/// 宿主选项里的 `user_data`：输入法写健康文件的目录。
fn user_data(options: &DictionaryHostOptions) -> Option<PathBuf> {
    options
        .snapshot()
        .ok()?
        .get("user_data")?
        .as_str()
        .map(PathBuf::from)
}

/// 授权页说明插件指令的状态时用的用户指令表：偏好里启用的指令表包，和输入法交给引擎的是同一份。偏好读不出时按没有启用任何指令表说明。
fn user_command_triggers(store: &PreferencesStore, root: &Path) -> Vec<String> {
    let enabled = store
        .load()
        .map(|snapshot| snapshot.preferences.plugins.command_tables)
        .unwrap_or_default();
    script_commands::user_triggers(root, &enabled)
}

/// 交给授权页的整份授权，带上停用记录和插件 `/` 指令的状态。官方脚本插件从 `packs` 读。
fn grants_view(
    root: &Path,
    packs: Option<&Path>,
    grants: script_grants::ScriptGrants,
    options: &DictionaryHostOptions,
    store: &PreferencesStore,
) -> ScriptGrantsView {
    let user_data = user_data(options);
    script_grants::view(
        root,
        packs,
        grants,
        user_data.as_deref(),
        &user_command_triggers(store, root),
    )
}

/// 脚本插件的授权、执行顺序、输入法记下的停用和插件 `/` 指令的状态。
#[tauri::command]
pub async fn script_grants(
    app: tauri::AppHandle,
    state: State<'_, PluginsState>,
    options: State<'_, DictionaryHostOptions>,
    store: State<'_, Arc<PreferencesStore>>,
) -> Result<ScriptGrantsView, PluginFailure> {
    let root = state.0.clone();
    let options = options.inner().clone();
    let store = Arc::clone(store.inner());
    blocking(move || {
        let packs = builtin_plugin_packs(&app, &options);
        Ok(grants_view(
            &root,
            packs.as_deref(),
            script_grants::read(&root),
            &options,
            &store,
        ))
    })
    .await
}

/// 授予、调整或撤销一个脚本插件的授权。`hash` 是页面展示给用户的那个内容哈希，包在这之后被换过时返回 `plugin_script_changed`；启用时超出同时启用的个数或内存总账分别返回 `plugin_script_limit`、`plugin_script_memory`。
// 后四个参数是前端调用这条命令时传的字段，合并成结构体会改掉 IPC 契约；多出的一个是官方插件包目录要用的 AppHandle。
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn set_script_grant(
    app: tauri::AppHandle,
    state: State<'_, PluginsState>,
    options: State<'_, DictionaryHostOptions>,
    store: State<'_, Arc<PreferencesStore>>,
    id: String,
    hash: String,
    enabled: bool,
    granted: Vec<String>,
) -> Result<ScriptGrantsView, PluginFailure> {
    let root = state.0.clone();
    let options = options.inner().clone();
    let store = Arc::clone(store.inner());
    blocking(move || {
        let packs = builtin_plugin_packs(&app, &options);
        let granted = script_grants::capabilities(&granted)?;
        let grants = script_grants::set(&root, packs.as_deref(), &id, &hash, enabled, granted)?;
        Ok(grants_view(
            &root,
            packs.as_deref(),
            grants,
            &options,
            &store,
        ))
    })
    .await
}

/// 调整执行顺序：`ids` 按给出的顺序排在最前。
#[tauri::command]
pub async fn reorder_scripts(
    app: tauri::AppHandle,
    state: State<'_, PluginsState>,
    options: State<'_, DictionaryHostOptions>,
    store: State<'_, Arc<PreferencesStore>>,
    ids: Vec<String>,
) -> Result<ScriptGrantsView, PluginFailure> {
    let root = state.0.clone();
    let options = options.inner().clone();
    let store = Arc::clone(store.inner());
    blocking(move || {
        let packs = builtin_plugin_packs(&app, &options);
        let grants = script_grants::reorder(&root, &ids)?;
        Ok(grants_view(
            &root,
            packs.as_deref(),
            grants,
            &options,
            &store,
        ))
    })
    .await
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
    use msime_client_services::plugins::PluginKind;
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
        let catalog = plugins::scan(&root, BuiltinDirs::default());
        assert_eq!(catalog.packages.len(), 1);

        plugins::remove_named(&root, "command_table", "signature").unwrap();
        assert!(plugins::scan(&root, BuiltinDirs::default())
            .packages
            .is_empty());
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
