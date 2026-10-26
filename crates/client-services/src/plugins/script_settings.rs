//! 脚本插件的设置值和存储代次：`<plugins root>/script-settings.json`，和 `script-grants.json` 同级。只在桌面编译。
//!
//! ```json
//! {"format_version":1,"revision":3,"plugins":{"rare-chars":{"values":{"hide":true,"level":"level_2"},"storage_epoch":1}}}
//! ```
//!
//! - **值**按插件 id、按清单 `[[script.settings]]` 的 `key` 存，类型同清单（布尔、选项的 `value`、整数、文本）。不进内容哈希，所以改设置不作废授权；清单里删掉或改了类型的项，旧值在交给插件时丢弃、用新的默认值（`script_pack::setting_values`），文件里的旧值等下一次写入这个插件时换掉。写入时按已安装包的清单逐项核对，不合规的整个拒绝（`plugin_script_setting_invalid`），不替用户改值。
//! - **存储代次** `storage_epoch`：用户清除数据、卸载插件或撤销 `persist_data` 时加 1（`script_storage::clear_locked`）。输入法进程按它判断自己手上的存储缓冲是不是已经被清掉，写盘前也在同一把锁里再核对一次，免得把清掉的数据又写回去。卸载时值一并清掉，代次保留并加 1：同 id 重新导入之后，旧进程手上的缓冲对不上新代次，写不回去。
//! - 写入方和授权文件一样：持有 `import::lock_plugin_root`，整份原子替换，每次写入 `revision` 加 1；读取方不加锁。`format_version` 不认识、读不出或不是合法 JSON 时按「没有任何设置、代次全为 0」处理，写入时拒绝覆盖不认识的版本（`plugin_script_settings_format`）。单个条目不合规（id 不对、`values` 不是对象、代次不是整数）只跳过这一条。

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::import::{lock_plugin_root, RootWrites};
use super::script_grants::ScriptGrantError;
use super::script_pack;
use crate::skin::catalog::safe_id;

/// 设置文件名。
pub const SETTINGS_FILE: &str = "script-settings.json";
/// 本版读写的格式。
pub const FORMAT_VERSION: u64 = 1;
/// 设置文件的字节上限。一个插件最多 16 项、文本最多 256 个 UTF-16 码元（转义后不到 2 KiB），常见的远小于它。
pub const MAX_DOCUMENT_BYTES: u64 = 1024 * 1024;
/// 读取的条目上限，之后的忽略。
pub const MAX_ENTRIES: usize = 256;

/// 一个插件的设置值和存储代次。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PluginSettings {
    /// 按清单 `key` 存的值，原样（交给插件之前再按清单核对）。
    pub values: Map<String, Value>,
    /// 清除数据、卸载、撤销 `persist_data` 时加 1。
    pub storage_epoch: u64,
}

/// 整份设置。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScriptSettings {
    pub revision: u64,
    pub plugins: BTreeMap<String, PluginSettings>,
}

impl ScriptSettings {
    /// `id` 存下的值；没有时为空。
    pub fn values(&self, id: &str) -> Option<&Map<String, Value>> {
        self.plugins.get(id).map(|plugin| &plugin.values)
    }

    /// `id` 的存储代次；没有条目时为 0。
    pub fn storage_epoch(&self, id: &str) -> u64 {
        self.plugins
            .get(id)
            .map_or(0, |plugin| plugin.storage_epoch)
    }
}

/// 设置文件的路径，给要按文件戳判断是否重读的宿主用。
pub fn document_path(root: &Path) -> PathBuf {
    root.join(SETTINGS_FILE)
}

/// 当前的设置。不加锁：写入方整份原子替换。文件不存在、读不出、不是合法 JSON 或 `format_version` 不认识时为空。
pub fn read(root: &Path) -> ScriptSettings {
    match load_document(root) {
        Stored::Parsed(settings) => settings,
        Stored::Missing | Stored::Unreadable | Stored::UnknownFormat => ScriptSettings::default(),
    }
}

/// 设置页读到的一个插件：清单里每一项现在交给插件的值（类型同清单，存下的值不合规时是默认值），以及 `persist_data` 的存储现在占多少字节。
#[derive(Debug, Serialize, PartialEq)]
pub struct ScriptPluginStateView {
    pub id: String,
    /// 按清单 `key`，覆盖清单里的每一项。
    pub values: Map<String, Value>,
    /// `script-data/<id>.bin` 的字节数；没有文件时为 0。
    pub storage_bytes: u64,
}

/// 设置页要的一个插件的状态。插件没装、清单载入不了时返回 `plugin_script_missing`。官方脚本插件从 `builtin_packs` 读清单。
pub fn state(
    root: &Path,
    builtin_packs: Option<&Path>,
    id: &str,
) -> Result<ScriptPluginStateView, ScriptGrantError> {
    if !safe_id(id) {
        return Err(ScriptGrantError::Invalid("插件 id 无效".into()));
    }
    let pack = script_pack::declared(root, builtin_packs, id).map_err(ScriptGrantError::Missing)?;
    Ok(view_of(root, id, &pack, &read(root)))
}

fn view_of(
    root: &Path,
    id: &str,
    pack: &script_pack::ScriptPack,
    settings: &ScriptSettings,
) -> ScriptPluginStateView {
    let stored = settings.values(id);
    ScriptPluginStateView {
        id: id.to_owned(),
        values: pack
            .settings
            .iter()
            .map(|setting| {
                (
                    setting.key.clone(),
                    setting.value(stored.and_then(|values| values.get(&setting.key))),
                )
            })
            .collect(),
        storage_bytes: super::script_storage::used_bytes(root, id),
    }
}

/// 整份替换插件 `id` 的设置值，返回写入后这个插件的状态。`values` 的每个键都必须是已安装包清单里的一项、值对那一项合规（`ScriptSetting::accepts`），否则整个拒绝；没写的项用默认值。存储代次不变。
pub fn set(
    root: &Path,
    builtin_packs: Option<&Path>,
    id: &str,
    values: Map<String, Value>,
) -> Result<ScriptPluginStateView, ScriptGrantError> {
    if !safe_id(id) {
        return Err(ScriptGrantError::Invalid("插件 id 无效".into()));
    }
    // 官方脚本插件不经导入，用户第一次设置它时插件目录可能还不在，和授权一样先建好。
    if !crate::storage::create_directory_and_check(root).map_err(|_| ScriptGrantError::Storage)? {
        return Err(ScriptGrantError::Storage);
    }
    let writes = lock_plugin_root(root)?;
    let pack = script_pack::declared(root, builtin_packs, id).map_err(ScriptGrantError::Missing)?;
    for (key, value) in &values {
        let Some(setting) = pack.settings.iter().find(|setting| setting.key == *key) else {
            return Err(ScriptGrantError::SettingInvalid(format!(
                "清单里没有设置项 {key}"
            )));
        };
        if !setting.accepts(value) {
            return Err(ScriptGrantError::SettingInvalid(format!(
                "设置项 {key} 的值不合规"
            )));
        }
    }
    let mut settings = editable(root)?;
    let count = settings.plugins.len();
    match settings.plugins.get_mut(id) {
        Some(plugin) => plugin.values = values,
        None if count >= MAX_ENTRIES => {
            return Err(ScriptGrantError::SettingInvalid(
                "设置过的脚本插件太多".into(),
            ));
        }
        None => {
            settings.plugins.insert(
                id.to_owned(),
                PluginSettings {
                    values,
                    storage_epoch: 0,
                },
            );
        }
    }
    settings.revision = next_revision(settings.revision);
    write(root, &writes, &settings)?;
    Ok(view_of(root, id, &pack, &settings))
}

/// 存储代次加 1，返回新的代次；`keep_values` 为假时（卸载）一并清掉设置值。持有插件目录的写锁。设置文件是更新的版本写的时候不改它：那样读取方本来就把代次当 0，这里改不动也不该改。
pub(super) fn bump_storage_epoch(
    root: &Path,
    writes: &RootWrites,
    id: &str,
    keep_values: bool,
) -> io::Result<u64> {
    let mut settings = match load_document(root) {
        Stored::Parsed(settings) => settings,
        Stored::Missing | Stored::Unreadable => ScriptSettings::default(),
        Stored::UnknownFormat => return Ok(0),
    };
    let plugin = settings.plugins.entry(id.to_owned()).or_default();
    plugin.storage_epoch = plugin.storage_epoch.saturating_add(1);
    if !keep_values {
        plugin.values = Map::new();
    }
    let epoch = plugin.storage_epoch;
    settings.revision = next_revision(settings.revision);
    write(root, writes, &settings)?;
    Ok(epoch)
}

/// 写入前读出的整份设置：读不出的文件按空的重写，不认识的版本拒绝覆盖。
fn editable(root: &Path) -> Result<ScriptSettings, ScriptGrantError> {
    match load_document(root) {
        Stored::Parsed(settings) => Ok(settings),
        Stored::Missing | Stored::Unreadable => Ok(ScriptSettings::default()),
        Stored::UnknownFormat => Err(ScriptGrantError::SettingsFormat),
    }
}

fn next_revision(revision: u64) -> u64 {
    revision
        .saturating_add(1)
        .min(super::script_grants::MAX_REVISION - 1)
}

enum Stored {
    Missing,
    Unreadable,
    UnknownFormat,
    Parsed(ScriptSettings),
}

#[derive(Deserialize)]
struct RawDocument {
    #[serde(default)]
    revision: u64,
    #[serde(default)]
    plugins: Map<String, Value>,
}

#[derive(Deserialize)]
struct RawEntry {
    #[serde(default)]
    values: Map<String, Value>,
    #[serde(default)]
    storage_epoch: u64,
}

#[derive(Serialize)]
struct WireDocument<'a> {
    format_version: u64,
    revision: u64,
    plugins: BTreeMap<&'a str, WireEntry<'a>>,
}

#[derive(Serialize)]
struct WireEntry<'a> {
    values: &'a Map<String, Value>,
    #[serde(skip_serializing_if = "is_zero")]
    storage_epoch: u64,
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

fn load_document(root: &Path) -> Stored {
    let path = document_path(root);
    if crate::storage::reject_symlink(&path).is_err() {
        return Stored::Unreadable;
    }
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Stored::Unreadable,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Stored::Missing,
        Err(_) => return Stored::Unreadable,
    }
    let Ok(bytes) = crate::storage::open_private_file_in(&path).and_then(|file| {
        crate::bounded_io::read_bounded_file(file, MAX_DOCUMENT_BYTES, || {
            io::Error::other("script settings are too large")
        })
    }) else {
        return Stored::Unreadable;
    };
    #[derive(Deserialize)]
    struct Version {
        format_version: Value,
    }
    let Ok(version) = serde_json::from_slice::<Version>(&bytes) else {
        return Stored::Unreadable;
    };
    if version.format_version.as_u64() != Some(FORMAT_VERSION) {
        return Stored::UnknownFormat;
    }
    let Ok(document) = serde_json::from_slice::<RawDocument>(&bytes) else {
        return Stored::Unreadable;
    };
    let plugins = document
        .plugins
        .into_iter()
        .filter(|(id, _)| safe_id(id))
        .filter_map(|(id, value)| {
            let entry: RawEntry = serde_json::from_value(value).ok()?;
            Some((
                id,
                PluginSettings {
                    values: entry.values,
                    storage_epoch: entry.storage_epoch,
                },
            ))
        })
        .take(MAX_ENTRIES)
        .collect();
    Stored::Parsed(ScriptSettings {
        revision: document
            .revision
            .min(super::script_grants::MAX_REVISION - 1),
        plugins,
    })
}

/// 整份原子替换。`_writes` 证明调用方持有插件目录的写锁。
fn write(root: &Path, _writes: &RootWrites, settings: &ScriptSettings) -> io::Result<()> {
    let document = WireDocument {
        format_version: FORMAT_VERSION,
        revision: settings.revision,
        plugins: settings
            .plugins
            .iter()
            .map(|(id, plugin)| {
                (
                    id.as_str(),
                    WireEntry {
                        values: &plugin.values,
                        storage_epoch: plugin.storage_epoch,
                    },
                )
            })
            .collect(),
    };
    let bytes = serde_json::to_vec_pretty(&document).map_err(io::Error::other)?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(io::Error::other("script settings are too large"));
    }
    crate::storage::replace_private_file(&document_path(root), &bytes)
}
