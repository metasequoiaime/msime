//! 脚本插件的授权：`<plugins root>/script-grants.json`，和 `mentions.json` 同级。只在桌面编译。
//!
//! ```json
//! {"format_version":1,"revision":3,"plugins":[{"id":"pangu-spacing","hash":"<64 位十六进制>","enabled":true,"granted":["read_commit","rewrite_commit"]}]}
//! ```
//!
//! - 放在插件目录而不放进偏好文档：偏好文档会被宿主复制、被账号同步读取，授权不该跟着走；它也不同步。`PluginPreferences` 带 `deny_unknown_fields`，往里加键会让旧版输入法读不了偏好。
//! - 数组顺序就是执行顺序。只有 `enabled` 的条目会运行，按顺序最多 `limits::MAX_ENABLED_SCRIPTS` 个。
//! - **哈希绑定**：条目的 `hash` 必须等于已安装包当前的内容哈希（`script_pack::content_hash`）才生效，包的清单或模块一变，授权全部作废。实际生效的能力是 `granted` 和清单 `permissions` 的交集。
//! - **只有一个写入方**，就是这里。写入时持有 `import::lock_plugin_root`（和导入、删包同一把锁），整份文件原子替换，每次写入都推进 `(revision_epoch, revision)`；读取方因此不加锁，看到的要么是旧文件，要么是新文件。删包、重新导入时在同一把锁里更新条目。宿主只在这个代次对比以前看到过的都大时恢复被熔断的插件。
//! - **读取方容忍未知字段**（不用 `deny_unknown_fields`），给以后的版本加字段留余地；但 `format_version` 不认识、文件读不出或不是合法 JSON 时，一律按"没有任何脚本启用"处理。单个条目不合规（id 或哈希格式不对、授权违反依赖或互斥规则）只跳过这一条。早期开发构建在条目里写过的 `mac` 字段同样当作未知字段忽略，下一次写入时不再写出。
//! - **授权不签名，所有桌面平台一样**：同一用户下的任何进程都能手写一条带正确哈希的授权，这是设计接受的残余风险，由授权页在三个桌面平台上写明（macOS 和 Linux 是共享设置页，Windows 是原生设置应用）。曾经考虑过在 macOS 上用钥匙串里的密钥给条目签 MAC，用户决定授权不使用钥匙串，取舍见 `.agents/notes/implemented/architecture/` 下本阶段的授权笔记。Windows 的提权窗口里永远不运行脚本。
//! - 授权页读到的形状（[`ScriptGrantsView`]，带 `script-health.json` 里的停用记录）也在这里，桌面外壳的授权命令和 host-api 的 `msime_client_plugins` 共用。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use msime_script_runtime::{limits, Capability, CapabilitySet};
use serde::{Deserialize, Serialize};

use super::import::{lock_plugin_root, RootWrites};
use super::script_pack::{self, rule_message};
use crate::skin::catalog::safe_id;

/// 授权文件名。
pub const GRANTS_FILE: &str = "script-grants.json";
/// 本版读写的格式。
pub const FORMAT_VERSION: u64 = 1;
/// 授权文件的字节上限。
pub const MAX_DOCUMENT_BYTES: u64 = 256 * 1024;
/// 读取的条目上限，之后的忽略。
pub const MAX_ENTRIES: usize = 256;
/// `revision` 的上限，2^53。写入方写出的值远小于它（当前时间的微秒数要到 2255 年才到）；读到更大的值说明文件不是写入方写的，下一次写入不接着它往上加，免得一份 `revision` 为 `u64::MAX` 的伪造文件让以后每次写入都停在同一个值上、熔断再也不复位。它也是 JavaScript 能精确表示的最大整数，设置页拿到的 `revision` 不失真。
pub const MAX_REVISION: u64 = 1 << 53;

/// 推进授权代次。普通写入取上一代次加 1 与当前时间微秒数的较大者；
/// 到上界时回到当前时间，并递增纪元，确保旧纪元文件不能被回放来恢复熔断。
fn advance_revision(grants: &mut ScriptGrants) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_micros()).unwrap_or(MAX_REVISION)
        })
        .min(MAX_REVISION - 1);
    if grants.revision >= MAX_REVISION - 1 {
        grants.revision_epoch += 1;
        grants.revision = now;
    } else {
        grants.revision = (grants.revision + 1).max(now);
    }
}

/// 一个脚本插件的授权。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptGrant {
    pub id: String,
    /// 授权时包的内容哈希，64 位小写十六进制。
    pub hash: String,
    pub enabled: bool,
    /// 用户授予的能力。不认识的能力名在读取时丢掉。
    pub granted: CapabilitySet,
}

/// 整份授权。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptGrants {
    /// 仅在 revision 回绕时递增；旧文件没有这个字段时为 0。
    pub revision_epoch: u64,
    pub revision: u64,
    /// 按执行顺序。
    pub plugins: Vec<ScriptGrant>,
}

impl ScriptGrants {
    /// 会运行的条目：启用的、按执行顺序的前 `limits::MAX_ENABLED_SCRIPTS` 个。哈希是否和已安装的包一致由调用方核对。
    pub fn enabled(&self) -> impl Iterator<Item = &ScriptGrant> {
        self.plugins
            .iter()
            .filter(|grant| grant.enabled)
            .take(limits::MAX_ENABLED_SCRIPTS)
    }

    pub fn get(&self, id: &str) -> Option<&ScriptGrant> {
        self.plugins.iter().find(|grant| grant.id == id)
    }
}

/// 写入授权失败。
#[derive(Debug, thiserror::Error)]
pub enum ScriptGrantError {
    /// 没有装这个脚本包，或者它载入不了；文本说明原因。
    #[error("plugin_script_missing: {0}")]
    Missing(String),
    /// 包的内容哈希和调用方看到的不一致：授权页展示之后包被换过。
    #[error("plugin_script_changed")]
    Changed,
    /// 授权本身不合规；文本说明原因。
    #[error("plugin_script_grant_invalid: {0}")]
    Invalid(String),
    /// 已经启用了 `limits::MAX_ENABLED_SCRIPTS` 个脚本。
    #[error("plugin_script_limit")]
    Limit,
    /// 启用后超出进程内存总账（`limits::PROCESS_MEMORY_BYTES`）。
    #[error("plugin_script_memory")]
    Memory,
    /// 授权文件是更新的版本写的，这一版不认识，不覆盖它。
    #[error("plugin_script_grants_format")]
    Format,
    /// 插件目录是符号链接或不是目录。
    #[error("plugin_storage")]
    Storage,
    #[error("plugin_io: {0}")]
    Io(#[from] io::Error),
}

/// 设置页传来的能力名换成能力集合。不认识的名字整个拒绝（`plugin_script_grant_invalid`），不像读文件时那样丢掉：这是用户刚勾选的授权，丢掉一项就等于替用户改了它。
pub fn capabilities(names: &[String]) -> Result<CapabilitySet, ScriptGrantError> {
    names
        .iter()
        .map(|name| {
            Capability::parse(name)
                .ok_or_else(|| ScriptGrantError::Invalid(format!("未知的能力 {name}")))
        })
        .collect()
}

/// 授权文件的路径，给要按文件戳判断是否重读的宿主用。
pub fn document_path(root: &Path) -> PathBuf {
    root.join(GRANTS_FILE)
}

/// 当前的授权。不加锁：写入方整份原子替换。文件不存在、读不出、不是合法 JSON 或 `format_version` 不认识时，返回空授权（没有任何脚本启用）。
pub fn read(root: &Path) -> ScriptGrants {
    match load_document(root) {
        Stored::Parsed(grants) => grants,
        Stored::Missing | Stored::Unreadable | Stored::UnknownFormat => ScriptGrants::default(),
    }
}

/// 授予、调整或撤销脚本插件 `id` 的授权，返回写入后的整份授权。
///
/// `hash` 是授权页展示给用户的那个内容哈希：写入前在锁里重新读一遍包、算一遍哈希，不一致时返回 [`ScriptGrantError::Changed`]，免得用户授权的是一个版本、生效的是另一个。`granted` 必须是清单 `permissions` 的子集，并满足依赖和互斥规则。启用时检查同时启用的个数和进程内存总账。新条目排在最后执行。包目录已经不在的条目顺带去掉（和删包时一样）：它们永远不会运行，却占着输入法运行的前 `limits::MAX_ENABLED_SCRIPTS` 个位置，页面上又列不出来、撤销不了。
pub fn set(
    root: &Path,
    id: &str,
    hash: &str,
    enabled: bool,
    granted: CapabilitySet,
) -> Result<ScriptGrants, ScriptGrantError> {
    if !safe_id(id) {
        return Err(ScriptGrantError::Invalid("插件 id 无效".into()));
    }
    crate::storage::reject_symlink(root).map_err(|_| ScriptGrantError::Storage)?;
    if !fs::symlink_metadata(root).is_ok_and(|metadata| metadata.is_dir()) {
        return Err(ScriptGrantError::Missing("插件文件夹不存在".into()));
    }
    let writes = lock_plugin_root(root)?;
    let installed = script_pack::load(root, id).map_err(ScriptGrantError::Missing)?;
    if installed.pack.content_hash != hash {
        return Err(ScriptGrantError::Changed);
    }
    // 授权的必须是一个能运行的模块：和导入时同一份 ABI 校验，只编译，不执行。
    msime_script_runtime::validate(&installed.wasm, installed.pack.declaration()).map_err(
        |error| {
            ScriptGrantError::Invalid(format!(
                "{} 不是有效的脚本模块：{error}",
                installed.pack.module
            ))
        },
    )?;
    if let Some(extra) = granted
        .iter()
        .find(|capability| !installed.pack.permissions.contains(*capability))
    {
        return Err(ScriptGrantError::Invalid(format!(
            "清单没有申请 {}",
            extra.as_str()
        )));
    }
    granted
        .check_rules()
        .map_err(|error| ScriptGrantError::Invalid(rule_message(error)))?;
    let mut grants = match load_document(root) {
        Stored::Parsed(grants) => grants,
        // 读不出的文件本来就等于没有任何授权，整份重写。
        Stored::Missing | Stored::Unreadable => ScriptGrants::default(),
        Stored::UnknownFormat => return Err(ScriptGrantError::Format),
    };
    let packs = super::kind_directory(root, super::PluginKind::Script);
    grants.plugins.retain(|grant| {
        !matches!(fs::symlink_metadata(packs.join(&grant.id)), Err(error) if error.kind() == io::ErrorKind::NotFound)
    });
    if enabled {
        admit(root, &grants, &installed)?;
    }
    let entry = ScriptGrant {
        id: id.to_owned(),
        hash: hash.to_owned(),
        enabled,
        granted,
    };
    let count = grants.plugins.len();
    match grants.plugins.iter_mut().find(|grant| grant.id == id) {
        Some(existing) => *existing = entry,
        // 读取时只认前 `MAX_ENTRIES` 条，再多一条写进去读回来也会被丢掉。
        None if count >= MAX_ENTRIES => {
            return Err(ScriptGrantError::Invalid("授权的脚本插件太多".into()));
        }
        None => grants.plugins.push(entry),
    }
    advance_revision(&mut grants);
    write(root, &writes, &grants)?;
    Ok(grants)
}

/// 调整执行顺序：`ids` 里的条目按给出的顺序排在最前，没列出的条目按原来的相对顺序排在后面。`ids` 里有重复或没有授权条目的 id 时什么都不写。返回写入后的整份授权。
pub fn reorder(root: &Path, ids: &[String]) -> Result<ScriptGrants, ScriptGrantError> {
    crate::storage::reject_symlink(root).map_err(|_| ScriptGrantError::Storage)?;
    if !fs::symlink_metadata(root).is_ok_and(|metadata| metadata.is_dir()) {
        return Err(ScriptGrantError::Missing("插件文件夹不存在".into()));
    }
    let writes = lock_plugin_root(root)?;
    let mut grants = match load_document(root) {
        Stored::Parsed(grants) => grants,
        Stored::Missing | Stored::Unreadable => ScriptGrants::default(),
        Stored::UnknownFormat => return Err(ScriptGrantError::Format),
    };
    let mut ordered = Vec::with_capacity(grants.plugins.len());
    for id in ids {
        let Some(position) = grants.plugins.iter().position(|grant| &grant.id == id) else {
            return Err(ScriptGrantError::Invalid(
                if ordered.iter().any(|grant: &ScriptGrant| &grant.id == id) {
                    format!("{id} 重复了")
                } else {
                    format!("没有 {id} 的授权")
                },
            ));
        };
        ordered.push(grants.plugins.remove(position));
    }
    ordered.append(&mut grants.plugins);
    grants.plugins = ordered;
    advance_revision(&mut grants);
    write(root, &writes, &grants)?;
    Ok(grants)
}

/// 输入法记下停用的文件名，放在宿主选项的 `user_data` 下。host-api 的输入法进程写它，授权页读它。
pub const HEALTH_FILE: &str = "script-health.json";
/// 健康文件的字节上限：最多 256 个条目，每条一个 id、一个哈希和一个类别。
const MAX_HEALTH_BYTES: u64 = 64 * 1024;

/// 授权页上的一个条目：`script-grants.json` 里的一条。
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ScriptGrantView {
    pub id: String,
    /// 授权时包的内容哈希；和目录里这个包现在的 `content_hash` 不同时，页面显示"已更新，需要重新授权"。
    pub hash: String,
    pub enabled: bool,
    pub granted: Vec<&'static str>,
}

/// 输入法进程停用了的一个插件：`<user_data>/script-health.json` 的一条。
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ScriptHealthView {
    pub id: String,
    pub hash: String,
    /// `invalid`、`init`、`failed` 或 `slow`。
    pub disabled: String,
}

/// 授权页读到的全部：授权（按执行顺序）和输入法记下的停用。桌面外壳的授权命令和 host-api 的 `msime_client_plugins`（Windows 的原生设置应用）回答的都是它。
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ScriptGrantsView {
    pub revision: u64,
    pub plugins: Vec<ScriptGrantView>,
    pub health: Vec<ScriptHealthView>,
}

/// 输入法写下的停用记录。`user_data` 没有、不是绝对路径，文件不在、读不出、格式不认识时为空：那只是说没有停用记录。
pub fn health(user_data: Option<&Path>) -> Vec<ScriptHealthView> {
    #[derive(Deserialize)]
    struct Document {
        format_version: u64,
        #[serde(default)]
        plugins: Vec<serde_json::Value>,
    }
    #[derive(Deserialize)]
    struct Entry {
        id: String,
        hash: String,
        disabled: String,
    }
    let Some(path) = user_data
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(HEALTH_FILE))
    else {
        return Vec::new();
    };
    let Ok(metadata) = fs::symlink_metadata(&path) else {
        return Vec::new();
    };
    if !metadata.is_file() || metadata.len() > MAX_HEALTH_BYTES {
        return Vec::new();
    }
    let Some(document) = fs::read(&path)
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

/// 交给授权页的整份授权：条目原样按执行顺序、能力名原样，加上 `user_data` 下的停用记录。
pub fn view(grants: ScriptGrants, user_data: Option<&Path>) -> ScriptGrantsView {
    ScriptGrantsView {
        revision: grants.revision,
        plugins: grants
            .plugins
            .into_iter()
            .map(|grant| ScriptGrantView {
                granted: grant.granted.iter().map(Capability::as_str).collect(),
                id: grant.id,
                hash: grant.hash,
                enabled: grant.enabled,
            })
            .collect(),
        health: health(user_data),
    }
}

/// 启用 `installed` 之前的准入检查：同时启用的个数，和进程内存总账。个数按输入法占位置的方式算：其他启用的条目，不管包载不载得了、哈希对不对得上，它们都占着输入法运行的前 `limits::MAX_ENABLED_SCRIPTS` 个位置（输入法取前几个之后才核对哈希），少算一个就会让新启用的插件排到位置之外、永远不运行。内存只算其中真会占内存的：包还在、哈希对得上的。
fn admit(
    root: &Path,
    grants: &ScriptGrants,
    installed: &script_pack::LoadedScript,
) -> Result<(), ScriptGrantError> {
    let others: Vec<&ScriptGrant> = grants
        .enabled()
        .filter(|grant| grant.id != installed.id)
        .collect();
    if others.len() >= limits::MAX_ENABLED_SCRIPTS {
        return Err(ScriptGrantError::Limit);
    }
    let others: Vec<script_pack::LoadedScript> = others
        .into_iter()
        .filter_map(|grant| {
            script_pack::load(root, &grant.id)
                .ok()
                .filter(|other| other.pack.content_hash == grant.hash)
        })
        .collect();
    let footprint =
        |script: &script_pack::LoadedScript| (script.wasm.len(), script.pack.memory_pages);
    if !limits::fits_memory_budget(
        others
            .iter()
            .map(footprint)
            .chain(std::iter::once(footprint(installed))),
    ) {
        return Err(ScriptGrantError::Memory);
    }
    Ok(())
}

/// 导入一个脚本包时、替换包目录之前调用，持有插件目录的写锁。已有这个 id 的条目时：内容哈希没变、并且是原地覆盖一个已安装的包，授权保留；否则（包换了内容，或者包目录在这之前已经不在了）停用并清空授予项，保留旧哈希，让授权页能显示"已更新，需要重新授权"。两种情况 `revision` 都加 1。文件不存在、读不出或格式不认识时不动它：那样本来就没有任何授权生效。
pub(super) fn installing(
    root: &Path,
    writes: &RootWrites,
    id: &str,
    hash: &str,
    replaces_installed: bool,
) -> io::Result<()> {
    let Stored::Parsed(mut grants) = load_document(root) else {
        return Ok(());
    };
    let Some(entry) = grants.plugins.iter_mut().find(|grant| grant.id == id) else {
        return Ok(());
    };
    if !(replaces_installed && entry.hash == hash) {
        entry.enabled = false;
        entry.granted = CapabilitySet::EMPTY;
    }
    advance_revision(&mut grants);
    write(root, writes, &grants)
}

/// 删除一个脚本包时、移走包目录之前调用，持有插件目录的写锁：去掉它的条目，`revision` 加 1。没有条目、文件不存在、读不出或格式不认识时不动它。
pub(super) fn removing(root: &Path, writes: &RootWrites, id: &str) -> io::Result<()> {
    let Stored::Parsed(mut grants) = load_document(root) else {
        return Ok(());
    };
    let before = grants.plugins.len();
    grants.plugins.retain(|grant| grant.id != id);
    if grants.plugins.len() == before {
        return Ok(());
    }
    advance_revision(&mut grants);
    write(root, writes, &grants)
}

/// 磁盘上的授权文件是什么状态。
enum Stored {
    Missing,
    Unreadable,
    UnknownFormat,
    Parsed(ScriptGrants),
}

#[derive(Deserialize)]
struct RawDocument {
    #[serde(default)]
    revision_epoch: u64,
    #[serde(default)]
    revision: u64,
    #[serde(default)]
    plugins: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct RawEntry {
    id: String,
    hash: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    granted: Vec<String>,
}

#[derive(Serialize)]
struct WireDocument<'a> {
    format_version: u64,
    #[serde(skip_serializing_if = "is_zero")]
    revision_epoch: u64,
    revision: u64,
    plugins: Vec<WireEntry<'a>>,
}

#[derive(Serialize)]
struct WireEntry<'a> {
    id: &'a str,
    hash: &'a str,
    enabled: bool,
    granted: Vec<&'static str>,
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
            io::Error::other("script grants are too large")
        })
    }) else {
        return Stored::Unreadable;
    };
    // 先只看 `format_version`：更新的版本可以改掉别的字段的形状，版本不认识时整份不解释。
    #[derive(Deserialize)]
    struct Version {
        format_version: serde_json::Value,
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
    // 写入方不会产生超出 JavaScript 精确整数范围的 revision。若先把
    // 伪造的极大值交给宿主，后续正常写入反而比它小，熔断状态不会恢复。
    if document.revision >= MAX_REVISION || document.revision_epoch >= MAX_REVISION {
        return Stored::Unreadable;
    }
    let mut plugins: Vec<ScriptGrant> = Vec::new();
    for value in document.plugins.into_iter().take(MAX_ENTRIES) {
        let Some(grant) = entry(value) else {
            continue;
        };
        if plugins.iter().any(|existing| existing.id == grant.id) {
            continue;
        }
        plugins.push(grant);
    }
    Stored::Parsed(ScriptGrants {
        revision_epoch: document.revision_epoch,
        revision: document.revision,
        plugins,
    })
}

/// 一个条目，不合规时 `None`（跳过这一条，不影响别的）。
fn entry(value: serde_json::Value) -> Option<ScriptGrant> {
    let raw: RawEntry = serde_json::from_value(value).ok()?;
    if !safe_id(&raw.id) || !is_hash(&raw.hash) {
        return None;
    }
    let granted: CapabilitySet = raw
        .granted
        .iter()
        .filter_map(|name| Capability::parse(name))
        .collect();
    granted.check_rules().ok()?;
    Some(ScriptGrant {
        id: raw.id,
        hash: raw.hash,
        enabled: raw.enabled,
        granted,
    })
}

/// 64 位小写十六进制，`script_pack::content_hash` 的形状。
fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// 整份原子替换。`_writes` 证明调用方持有插件目录的写锁。`revision` 已经由调用方换过。
fn write(root: &Path, _writes: &RootWrites, grants: &ScriptGrants) -> io::Result<()> {
    let document = WireDocument {
        format_version: FORMAT_VERSION,
        revision_epoch: grants.revision_epoch,
        revision: grants.revision,
        plugins: grants
            .plugins
            .iter()
            .map(|grant| WireEntry {
                id: &grant.id,
                hash: &grant.hash,
                enabled: grant.enabled,
                granted: grant.granted.iter().map(Capability::as_str).collect(),
            })
            .collect(),
    };
    let bytes = serde_json::to_vec_pretty(&document).map_err(io::Error::other)?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(io::Error::other("script grants are too large"));
    }
    crate::storage::replace_private_file(&document_path(root), &bytes)
}
