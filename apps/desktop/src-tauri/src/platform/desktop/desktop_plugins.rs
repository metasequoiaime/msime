//! 插件 page commands for the macOS, Windows and Linux shells: the plugin packs installed under the state directory, the built-in sound packs the bundle ships, and the @ mode's name list.
//!
//! Every rule lives in `msime_client_core::plugins`; these are shims that resolve the roots host-side and hand them over. The page never names a path: it asks for the catalog, asks the host to show its own picker for an import, and names a pack to remove by kind and id. The input processes read the same `<state>/plugins` directory (`preferences_directory/plugins` in host-api), so an import or a name-list save reaches them without any notification of its own: sound and music settings travel through the preferences document, and the command tables and the name list are reread when a field gains focus after their files changed.

use crate::DictionaryHostOptions;
use msime_client_core::plugins::mentions::{MentionEntry, MentionStore};
use msime_client_core::plugins::script_grants::{self, ScriptGrants};
use msime_client_core::plugins::{self, PluginCatalog, PluginFailure, PluginSummary};
use serde::Serialize;
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
        // 导入脚本包时授权文件里其余条目要按新的 revision 重新签名，所以有授权文件时先把密钥交进来。
        if script_grants::document_path(&root).exists() {
            install_grant_key(&app);
        }
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
    app: tauri::AppHandle,
    state: State<'_, PluginsState>,
    kind: String,
    id: String,
) -> Result<(), PluginFailure> {
    let root = state.0.clone();
    blocking(move || {
        // 删脚本包时授权文件里其余条目要按新的 revision 重新签名。
        if kind == "script" && script_grants::document_path(&root).exists() {
            install_grant_key(&app);
        }
        plugins::remove_named(&root, &kind, &id)
    })
    .await
}

/// 授权页上的一个条目：`script-grants.json` 里的一条，加上它在本进程里能不能通过校验。
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ScriptGrantView {
    pub id: String,
    /// 授权时包的内容哈希；和目录里这个包现在的 `content_hash` 不同时，页面显示"已更新，需要重新授权"。
    pub hash: String,
    pub enabled: bool,
    pub granted: Vec<&'static str>,
    /// macOS 上 MAC 校验通过；其他平台恒为 true。为 false 的条目不运行，页面要求重新授权。
    pub verified: bool,
}

/// 输入法进程停用了的一个插件：`<user_data>/script-health.json` 的一条。
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ScriptHealthView {
    pub id: String,
    pub hash: String,
    /// `invalid`、`init`、`failed` 或 `slow`。
    pub disabled: String,
}

/// 授权页读到的全部：授权（按执行顺序）、输入法记下的停用，以及这个平台的条目要不要带 MAC、本进程有没有密钥。
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ScriptGrantsView {
    pub revision: u64,
    pub plugins: Vec<ScriptGrantView>,
    pub health: Vec<ScriptHealthView>,
    /// 条目要带 MAC 才生效（macOS）。
    pub mac_required: bool,
    /// 本进程拿到了钥匙串里的密钥。`mac_required` 而没有密钥时，页面说明授权写不进去。
    pub signed: bool,
}

/// 设置应用是授权文件的写入方：macOS 上在第一次读写之前从钥匙串取出（没有就生成）MAC 密钥交给 client-core，访问控制表一起信任输入法 bundle（已安装的那份和本应用资源里的那份，签名相同）。取不到时不报错：读照常，写入由 `script_grants` 以 `plugin_script_keychain` 拒绝。其他平台不带 MAC，什么都不做。
#[cfg(target_os = "macos")]
fn install_grant_key(app: &tauri::AppHandle) {
    use tauri::Manager;
    if script_grants::has_mac_key() {
        return;
    }
    let bundle = crate::macos_input_source::input_source_bundle_name();
    let peers: Vec<PathBuf> = [
        crate::macos_input_source::installed_bundle_path().ok(),
        app.path()
            .resource_dir()
            .ok()
            .map(|resources| resources.join(bundle)),
    ]
    .into_iter()
    .flatten()
    .collect();
    let peers: Vec<&Path> = peers.iter().map(PathBuf::as_path).collect();
    if let Ok(key) = msime_host_macos::script_grant_key(&peers) {
        script_grants::set_mac_key(&key);
    }
}

#[cfg(not(target_os = "macos"))]
fn install_grant_key(_app: &tauri::AppHandle) {}

/// 健康文件名，与 host-api 的 `scripts::process::HEALTH_FILE` 一致。
const SCRIPT_HEALTH_FILE: &str = "script-health.json";
/// 健康文件的字节上限：最多 256 个条目，每条一个 id、一个哈希和一个类别。
const MAX_SCRIPT_HEALTH_BYTES: u64 = 64 * 1024;

/// 输入法写下的停用记录。文件不在、读不出、格式不认识时为空：那只是说没有停用记录。
fn script_health(user_data: Option<&Path>) -> Vec<ScriptHealthView> {
    #[derive(serde::Deserialize)]
    struct Document {
        format_version: u64,
        #[serde(default)]
        plugins: Vec<serde_json::Value>,
    }
    #[derive(serde::Deserialize)]
    struct Entry {
        id: String,
        hash: String,
        disabled: String,
    }
    let Some(path) = user_data
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(SCRIPT_HEALTH_FILE))
    else {
        return Vec::new();
    };
    let Ok(metadata) = std::fs::symlink_metadata(&path) else {
        return Vec::new();
    };
    if !metadata.is_file() || metadata.len() > MAX_SCRIPT_HEALTH_BYTES {
        return Vec::new();
    }
    let Some(document) = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Document>(&bytes).ok())
        .filter(|document| document.format_version == 1)
    else {
        return Vec::new();
    };
    document
        .plugins
        .into_iter()
        .filter_map(|value| serde_json::from_value::<Entry>(value).ok())
        .filter(|entry| {
            matches!(
                entry.disabled.as_str(),
                "invalid" | "init" | "failed" | "slow"
            )
        })
        .map(|entry| ScriptHealthView {
            id: entry.id,
            hash: entry.hash,
            disabled: entry.disabled,
        })
        .collect()
}

fn grants_view(grants: ScriptGrants, user_data: Option<&Path>) -> ScriptGrantsView {
    ScriptGrantsView {
        revision: grants.revision,
        plugins: grants
            .plugins
            .into_iter()
            .map(|grant| ScriptGrantView {
                granted: grant
                    .granted
                    .iter()
                    .map(|capability| capability.as_str())
                    .collect(),
                id: grant.id,
                hash: grant.hash,
                enabled: grant.enabled,
                verified: grant.verified,
            })
            .collect(),
        health: script_health(user_data),
        mac_required: script_grants::MAC_REQUIRED,
        signed: script_grants::has_mac_key(),
    }
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

/// 脚本插件的授权、执行顺序和输入法记下的停用。
#[tauri::command]
pub async fn script_grants(
    app: tauri::AppHandle,
    state: State<'_, PluginsState>,
    options: State<'_, DictionaryHostOptions>,
) -> Result<ScriptGrantsView, PluginFailure> {
    let root = state.0.clone();
    let options = options.inner().clone();
    blocking(move || {
        let user_data = user_data(&options);
        // 没装脚本包、也没有授权文件时不碰钥匙串：打开插件页不该为一个用不上的功能去生成密钥或弹出钥匙串授权框。
        let scripts_installed =
            std::fs::read_dir(plugins::kind_directory(&root, plugins::PluginKind::Script))
                .is_ok_and(|mut entries| entries.next().is_some());
        if scripts_installed || script_grants::document_path(&root).exists() {
            install_grant_key(&app);
        }
        Ok(grants_view(
            script_grants::read(&root),
            user_data.as_deref(),
        ))
    })
    .await
}

/// 授予、调整或撤销一个脚本插件的授权。`hash` 是页面展示给用户的那个内容哈希，包在这之后被换过时返回 `plugin_script_changed`；启用时超出同时启用的个数或内存总账分别返回 `plugin_script_limit`、`plugin_script_memory`。
#[tauri::command]
pub async fn set_script_grant(
    app: tauri::AppHandle,
    state: State<'_, PluginsState>,
    options: State<'_, DictionaryHostOptions>,
    id: String,
    hash: String,
    enabled: bool,
    granted: Vec<String>,
) -> Result<ScriptGrantsView, PluginFailure> {
    let root = state.0.clone();
    let options = options.inner().clone();
    blocking(move || {
        let user_data = user_data(&options);
        install_grant_key(&app);
        let granted = script_grants::capabilities(&granted)?;
        let grants = script_grants::set(&root, &id, &hash, enabled, granted)?;
        Ok(grants_view(grants, user_data.as_deref()))
    })
    .await
}

/// 调整执行顺序：`ids` 按给出的顺序排在最前。
#[tauri::command]
pub async fn reorder_scripts(
    app: tauri::AppHandle,
    state: State<'_, PluginsState>,
    options: State<'_, DictionaryHostOptions>,
    ids: Vec<String>,
) -> Result<ScriptGrantsView, PluginFailure> {
    let root = state.0.clone();
    let options = options.inner().clone();
    blocking(move || {
        let user_data = user_data(&options);
        install_grant_key(&app);
        Ok(grants_view(
            script_grants::reorder(&root, &ids)?,
            user_data.as_deref(),
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
    fn script_health_lists_only_the_four_disabled_categories() {
        let user_data = tempfile::tempdir().unwrap();
        assert!(script_health(Some(user_data.path())).is_empty());
        assert!(script_health(Some(Path::new("relative"))).is_empty());
        assert!(script_health(None).is_empty());
        let file = user_data.path().join(SCRIPT_HEALTH_FILE);
        fs::write(
            &file,
            r#"{"format_version":1,"plugins":[{"id":"a","hash":"h1","disabled":"failed"},{"id":"b","hash":"h2","disabled":"unreachable"},{"id":"c"},{"id":"d","hash":"h4","disabled":"slow"}]}"#,
        )
        .unwrap();
        assert_eq!(
            script_health(Some(user_data.path())),
            [
                ScriptHealthView {
                    id: "a".into(),
                    hash: "h1".into(),
                    disabled: "failed".into(),
                },
                ScriptHealthView {
                    id: "d".into(),
                    hash: "h4".into(),
                    disabled: "slow".into(),
                },
            ]
        );
        // 不认识的格式或读不懂的文件当作没有停用记录。
        fs::write(
            &file,
            r#"{"format_version":2,"plugins":[{"id":"a","hash":"h1","disabled":"failed"}]}"#,
        )
        .unwrap();
        assert!(script_health(Some(user_data.path())).is_empty());
        fs::write(&file, "not json").unwrap();
        assert!(script_health(Some(user_data.path())).is_empty());
    }

    #[test]
    fn the_grants_view_carries_the_order_the_signature_and_the_platform_rule() {
        let view = grants_view(
            ScriptGrants {
                revision: 4,
                plugins: Vec::new(),
            },
            None,
        );
        assert_eq!(view.revision, 4);
        assert!(view.plugins.is_empty());
        assert_eq!(view.mac_required, cfg!(target_os = "macos"));
        assert_eq!(view.signed, script_grants::has_mac_key());
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
