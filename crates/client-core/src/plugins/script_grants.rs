//! 脚本插件的授权：`<plugins root>/script-grants.json`，和 `mentions.json` 同级。只在桌面编译。
//!
//! ```json
//! {"format_version":1,"revision":3,"plugins":[{"id":"pangu-spacing","hash":"<64 位十六进制>","enabled":true,"granted":["read_commit","rewrite_commit"],"mac":"<64 位十六进制>"}]}
//! ```
//!
//! - 放在插件目录而不放进偏好文档：偏好文档会被宿主复制、被账号同步读取，授权不该跟着走；它也不同步。`PluginPreferences` 带 `deny_unknown_fields`，往里加键会让旧版输入法读不了偏好。
//! - 数组顺序就是执行顺序。只有 `enabled` 的条目会运行，按顺序最多 `limits::MAX_ENABLED_SCRIPTS` 个。
//! - **哈希绑定**：条目的 `hash` 必须等于已安装包当前的内容哈希（`script_pack::content_hash`）才生效，包的清单或模块一变，授权全部作废。实际生效的能力是 `granted` 和清单 `permissions` 的交集。
//! - **只有一个写入方**，就是这里。写入时持有 `import::lock_plugin_root`（和导入、删包同一把锁），整份文件原子替换，每次写入 `revision` 都变大（[`next_revision`]：上一份加 1 和当前时间的微秒数取大的那个）；读取方因此不加锁，看到的要么是旧文件，要么是新文件。删包、重新导入时在同一把锁里更新条目。`revision` 变大时宿主恢复被熔断的插件，macOS 上只认至少有一个条目通过校验的文件里的 `revision`（[`ScriptGrants::authenticated_revision`]）。
//! - **读取方容忍未知字段**（不用 `deny_unknown_fields`），给以后的版本加字段留余地；但 `format_version` 不认识、文件读不出或不是合法 JSON 时，一律按"没有任何脚本启用"处理。单个条目不合规（id 或哈希格式不对、授权违反依赖或互斥规则）只跳过这一条。
//! - **macOS 上每个条目要带 MAC 才生效**（[`MAC_REQUIRED`]）：`mac` 是用进程级密钥（[`set_mac_key`]）对 (id, hash, enabled, granted, revision, 条目在文件里的位置, 条目总数) 算的 HMAC-SHA256，所以挪动、删掉或插入条目都让签名失效，执行顺序也在签名里。密钥是存在钥匙串里的 32 字节随机数，钥匙串项只信任签了名的输入法和设置应用，由宿主在启动时（输入法）或第一次用到时（设置应用）交进来。没有密钥、没有 `mac` 或校验不过的条目照常读出来给设置页看，但 `verified` 为 false，[`ScriptGrants::enabled`] 不列出它。写入时只给原来就校验通过的条目和这次写的条目重新签名（`revision` 每次都变，所以每次写入都要重签），从不替一条没校验过的条目补签：那样等于设置应用替伪造者背书。macOS 上没有密钥时 [`set`] 和 [`reorder`] 一律拒绝（`plugin_script_keychain`）。
//! - Windows 和 Linux 不带 MAC：同一用户下的任何进程都能手写一条带正确哈希的授权，这是设计接受的残余风险，由授权页写明；Windows 的提权窗口里永远不运行脚本。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, KeyInit, Mac};
use msime_script_runtime::{limits, Capability, CapabilitySet};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

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
/// MAC 密钥的字节数。
pub const MAC_KEY_BYTES: usize = 32;
/// 条目必须带校验通过的 MAC 才生效的平台：macOS，那里的钥匙串能把密钥只交给签了名的输入法和设置应用。
pub const MAC_REQUIRED: bool = cfg!(target_os = "macos");
/// MAC 的域分隔前缀，让它不可能和别处用同一把密钥对同样字节算的 HMAC 相同。
const MAC_DOMAIN: &[u8] = b"msime-script-grant\0";
/// `revision` 的上限，2^53。写入方写出的值远小于它（当前时间的微秒数要到 2255 年才到）；读到更大的值说明文件不是写入方写的，下一次写入不接着它往上加，免得一份 `revision` 为 `u64::MAX` 的伪造文件让以后每次写入都停在同一个值上。它也是 JavaScript 能精确表示的最大整数，设置页拿到的 `revision` 不失真。
pub const MAX_REVISION: u64 = 1 << 53;

/// 进程级的 MAC 密钥，第一次交进来的那把生效。
static MAC_KEY: OnceLock<[u8; MAC_KEY_BYTES]> = OnceLock::new();

/// 交进本进程的 MAC 密钥，必须是 [`MAC_KEY_BYTES`] 字节。只认第一次交进来的那把：返回本进程现在用的密钥是不是这一把，长度不对时返回 false。macOS 输入法在启动时经 `msime_client_script_grants_key` 调用，设置应用在第一次读写授权之前调用。
pub fn set_mac_key(key: &[u8]) -> bool {
    let Ok(key) = <[u8; MAC_KEY_BYTES]>::try_from(key) else {
        return false;
    };
    *MAC_KEY.get_or_init(|| key) == key
}

/// 本进程有没有 MAC 密钥。
pub fn has_mac_key() -> bool {
    MAC_KEY.get().is_some()
}

/// 一次读写用的密钥和规则。产品里总是 [`Keying::process`]；测试经它模拟没有密钥的 macOS 进程和不要求 MAC 的平台，不受进程级密钥只认第一把的限制。
#[derive(Debug, Clone, Copy)]
pub(super) struct Keying<'a> {
    pub(super) key: Option<&'a [u8; MAC_KEY_BYTES]>,
    /// 条目必须带校验通过的 MAC 才生效：产品里是 [`MAC_REQUIRED`]。
    pub(super) required: bool,
}

impl Keying<'static> {
    /// 本进程的密钥和本平台的规则。
    pub(super) fn process() -> Self {
        Self {
            key: MAC_KEY.get(),
            required: MAC_REQUIRED,
        }
    }
}

/// 一个条目签名时所在的那份文件：`revision`、条目在 `plugins` 里的位置和条目总数。三者都签进 MAC：每次写入的 `revision` 都不同，所以两份文件里的条目拼不到一起；位置和总数让挪动、删掉或插入条目都让签名失效，执行顺序因此也受签名保护。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub revision: u64,
    pub position: usize,
    pub count: usize,
}

/// 一个条目的 MAC，64 位小写十六进制。宿主的测试用它手写带 MAC 的授权文件。
pub fn entry_mac(
    key: &[u8; MAC_KEY_BYTES],
    placement: Placement,
    id: &str,
    hash: &str,
    enabled: bool,
    granted: CapabilitySet,
) -> String {
    hex::encode(
        mac_state(key, placement, id, hash, enabled, granted)
            .finalize()
            .into_bytes(),
    )
}

/// HMAC-SHA256 覆盖的字节，依次是域前缀、`revision`、位置、条目总数（各 8 字节小端）、id、hash、`enabled`（一个字节）、授予的能力个数（8 字节小端），再按 `Capability::ALL` 的顺序逐个写能力名；每一段前面都带它的字节数（8 字节小端），字节在两段之间挪动不会得到同一个 MAC。写名字而不写位：能力的位编号以后可以变，名字是授权文件里的契约。
fn mac_state(
    key: &[u8; MAC_KEY_BYTES],
    placement: Placement,
    id: &str,
    hash: &str,
    enabled: bool,
    granted: CapabilitySet,
) -> Hmac<Sha256> {
    let mut mac = <Hmac<Sha256>>::new_from_slice(key).expect("HMAC 接受任意长度的密钥");
    let mut field = |bytes: &[u8]| {
        mac.update(&(bytes.len() as u64).to_le_bytes());
        mac.update(bytes);
    };
    field(MAC_DOMAIN);
    field(&placement.revision.to_le_bytes());
    field(&(placement.position as u64).to_le_bytes());
    field(&(placement.count as u64).to_le_bytes());
    field(id.as_bytes());
    field(hash.as_bytes());
    field(&[u8::from(enabled)]);
    field(&(granted.iter().count() as u64).to_le_bytes());
    for capability in granted.iter() {
        field(capability.as_str().as_bytes());
    }
    mac
}

/// 条目的 `mac` 是否是 `key` 在 `placement` 下对它算出来的。比较是常量时间的。
fn mac_verifies(
    key: Option<&[u8; MAC_KEY_BYTES]>,
    placement: Placement,
    grant: &ScriptGrant,
) -> bool {
    let (Some(key), Some(tag)) = (key, grant.mac.as_deref()) else {
        return false;
    };
    let Ok(tag) = hex::decode(tag) else {
        return false;
    };
    mac_state(
        key,
        placement,
        &grant.id,
        &grant.hash,
        grant.enabled,
        grant.granted,
    )
    .verify_slice(&tag)
    .is_ok()
}

/// 下一次写入的 `revision`：比 `current` 大，并且不小于当前时间的微秒数。只加 1 的话，同一用户下的程序把 `revision` 改小（或者放回一份旧文件）之后，写入方会再用一遍以前签过名的 `revision`，两份文件里位置相同的条目就能拼成一份用户从没确认过的授权；取当前时间之后，每次写入的 `revision` 都比以前签过的大（时钟不往回拨时）。写入方写出的值总小于 [`MAX_REVISION`]；`current` 到了 `MAX_REVISION - 1` 或更大时，它不是写入方写的，不接着它加。
fn next_revision(current: u64) -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_micros()).unwrap_or(MAX_REVISION)
        })
        .min(MAX_REVISION - 1);
    if current >= MAX_REVISION - 1 {
        return now;
    }
    (current + 1).max(now)
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
    /// 条目的 MAC，64 位小写十六进制；形状不对时读成 `None`。
    pub mac: Option<String>,
    /// 这条授权能不能生效：macOS 上要求 `mac` 用本进程的密钥校验通过，其他平台恒为 true。为 false 的条目照样列给设置页（显示为需要重新授权），但不运行。
    pub verified: bool,
}

/// 整份授权。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptGrants {
    pub revision: u64,
    /// 按执行顺序。
    pub plugins: Vec<ScriptGrant>,
}

impl ScriptGrants {
    /// 会运行的条目：启用的、校验通过的、按执行顺序的前 `limits::MAX_ENABLED_SCRIPTS` 个。哈希是否和已安装的包一致由调用方核对。
    pub fn enabled(&self) -> impl Iterator<Item = &ScriptGrant> {
        self.plugins
            .iter()
            .filter(|grant| grant.enabled && grant.verified)
            .take(limits::MAX_ENABLED_SCRIPTS)
    }

    pub fn get(&self, id: &str) -> Option<&ScriptGrant> {
        self.plugins.iter().find(|grant| grant.id == id)
    }

    /// 能让宿主恢复被熔断的插件的 `revision`：至少一个条目通过校验时才有。macOS 上 MAC 覆盖 `revision`，有一条校验通过就说明这个 `revision` 出自持有密钥的写入方；一份没有签名的文件（或者文件不在、读不出）给不出它，同一用户下的程序就没法靠换文件、删文件让熔断复位。其他平台的条目恒通过校验，这里只要求文件里有条目。
    pub fn authenticated_revision(&self) -> Option<u64> {
        self.plugins
            .iter()
            .any(|grant| grant.verified)
            .then_some(self.revision)
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
    /// macOS 上本进程没有 MAC 密钥（钥匙串读不出或被拒绝），写出去的授权不会生效，所以不写。
    #[error("plugin_script_keychain")]
    Keychain,
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
    read_with(root, Keying::process())
}

/// [`read`]，用给定的密钥和规则校验。
pub(super) fn read_with(root: &Path, keying: Keying<'_>) -> ScriptGrants {
    match load_document(root, keying) {
        Stored::Parsed(grants) => grants,
        Stored::Missing | Stored::Unreadable | Stored::UnknownFormat => ScriptGrants::default(),
    }
}

/// 授予、调整或撤销脚本插件 `id` 的授权，返回写入后的整份授权。
///
/// `hash` 是授权页展示给用户的那个内容哈希：写入前在锁里重新读一遍包、算一遍哈希，不一致时返回 [`ScriptGrantError::Changed`]，免得用户授权的是一个版本、生效的是另一个。`granted` 必须是清单 `permissions` 的子集，并满足依赖和互斥规则。启用时检查同时启用的个数和进程内存总账。新条目排在最后执行。这次写的条目由本进程的密钥签名（有密钥时），所以设置页上原来校验不过的条目经用户重新授权后恢复生效。包目录已经不在的条目顺带去掉（和删包时一样）：它们永远不会运行，却占着输入法运行的前 `limits::MAX_ENABLED_SCRIPTS` 个位置，页面上又列不出来、撤销不了。
pub fn set(
    root: &Path,
    id: &str,
    hash: &str,
    enabled: bool,
    granted: CapabilitySet,
) -> Result<ScriptGrants, ScriptGrantError> {
    set_with(root, Keying::process(), id, hash, enabled, granted)
}

/// [`set`]，用给定的密钥和规则签名、校验。
pub(super) fn set_with(
    root: &Path,
    keying: Keying<'_>,
    id: &str,
    hash: &str,
    enabled: bool,
    granted: CapabilitySet,
) -> Result<ScriptGrants, ScriptGrantError> {
    if !safe_id(id) {
        return Err(ScriptGrantError::Invalid("插件 id 无效".into()));
    }
    // macOS 上没有密钥时写出去的条目全都校验不过，连别的条目原有的签名也会因为 `revision` 变了而失效，等于撤销全部授权，所以不写。
    if keying.required && keying.key.is_none() {
        return Err(ScriptGrantError::Keychain);
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
    let mut grants = match load_document(root, keying) {
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
    // 用户刚在授权页上确认过这一条，由本进程签名。
    let entry = ScriptGrant {
        id: id.to_owned(),
        hash: hash.to_owned(),
        enabled,
        granted,
        mac: None,
        verified: true,
    };
    let count = grants.plugins.len();
    match grants.plugins.iter_mut().find(|grant| grant.id == id) {
        Some(existing) => *existing = entry,
        // 读取时只认前 `MAX_ENTRIES` 条，再多一条读回来就对不上签名里的条目总数。
        None if count >= MAX_ENTRIES => {
            return Err(ScriptGrantError::Invalid("授权的脚本插件太多".into()));
        }
        None => grants.plugins.push(entry),
    }
    grants.revision = next_revision(grants.revision);
    write(root, &writes, &mut grants, keying)?;
    Ok(grants)
}

/// 调整执行顺序：`ids` 里的条目按给出的顺序排在最前，没列出的条目按原来的相对顺序排在后面。`ids` 里有重复或没有授权条目的 id 时什么都不写。返回写入后的整份授权。
pub fn reorder(root: &Path, ids: &[String]) -> Result<ScriptGrants, ScriptGrantError> {
    reorder_with(root, Keying::process(), ids)
}

/// [`reorder`]，用给定的密钥和规则签名、校验。
pub(super) fn reorder_with(
    root: &Path,
    keying: Keying<'_>,
    ids: &[String],
) -> Result<ScriptGrants, ScriptGrantError> {
    if keying.required && keying.key.is_none() {
        return Err(ScriptGrantError::Keychain);
    }
    crate::storage::reject_symlink(root).map_err(|_| ScriptGrantError::Storage)?;
    if !fs::symlink_metadata(root).is_ok_and(|metadata| metadata.is_dir()) {
        return Err(ScriptGrantError::Missing("插件文件夹不存在".into()));
    }
    let writes = lock_plugin_root(root)?;
    let mut grants = match load_document(root, keying) {
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
    grants.revision = next_revision(grants.revision);
    write(root, &writes, &mut grants, keying)?;
    Ok(grants)
}

/// 启用 `installed` 之前的准入检查：同时启用的个数，和进程内存总账。个数按输入法占位置的方式算：其他启用并且校验通过的条目，不管包载不载得了、哈希对不对得上，它们都占着输入法运行的前 `limits::MAX_ENABLED_SCRIPTS` 个位置（输入法取前几个之后才核对哈希），少算一个就会让新启用的插件排到位置之外、永远不运行。内存只算其中真会占内存的：包还在、哈希对得上的。
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
    let keying = Keying::process();
    let Stored::Parsed(mut grants) = load_document(root, keying) else {
        return Ok(());
    };
    let Some(entry) = grants.plugins.iter_mut().find(|grant| grant.id == id) else {
        return Ok(());
    };
    if !(replaces_installed && entry.hash == hash) {
        entry.enabled = false;
        entry.granted = CapabilitySet::EMPTY;
        entry.mac = None;
    }
    grants.revision = next_revision(grants.revision);
    write(root, writes, &mut grants, keying)
}

/// 删除一个脚本包时、移走包目录之前调用，持有插件目录的写锁：去掉它的条目，`revision` 加 1。没有条目、文件不存在、读不出或格式不认识时不动它。
pub(super) fn removing(root: &Path, writes: &RootWrites, id: &str) -> io::Result<()> {
    let keying = Keying::process();
    let Stored::Parsed(mut grants) = load_document(root, keying) else {
        return Ok(());
    };
    let before = grants.plugins.len();
    grants.plugins.retain(|grant| grant.id != id);
    if grants.plugins.len() == before {
        return Ok(());
    }
    grants.revision = next_revision(grants.revision);
    write(root, writes, &mut grants, keying)
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
    #[serde(default)]
    mac: Option<String>,
}

#[derive(Serialize)]
struct WireDocument<'a> {
    format_version: u64,
    revision: u64,
    plugins: Vec<WireEntry<'a>>,
}

#[derive(Serialize)]
struct WireEntry<'a> {
    id: &'a str,
    hash: &'a str,
    enabled: bool,
    granted: Vec<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mac: Option<&'a str>,
}

fn load_document(root: &Path, keying: Keying<'_>) -> Stored {
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
    let count = plugins.len();
    for (position, grant) in plugins.iter_mut().enumerate() {
        let placement = Placement {
            revision: document.revision,
            position,
            count,
        };
        grant.verified = !keying.required || mac_verifies(keying.key, placement, grant);
    }
    Stored::Parsed(ScriptGrants {
        revision: document.revision,
        plugins,
    })
}

/// 一个条目，不合规时 `None`（跳过这一条，不影响别的）。`verified` 由调用方按整份文件的 `revision`、条目的位置和总数校验后填上。
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
        mac: raw.mac.filter(|mac| is_hash(mac)),
        verified: false,
    })
}

/// 64 位小写十六进制，`script_pack::content_hash` 和 MAC 的形状。
fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// 整份原子替换。`_writes` 证明调用方持有插件目录的写锁。`revision` 已经由调用方换过：有密钥时给 `verified` 的条目按新的 `revision` 和各自现在的位置、条目总数重新签名，其余条目不带 MAC；写完后 `verified` 按新的签名更新，和随后 [`read`] 读到的一致。
fn write(
    root: &Path,
    _writes: &RootWrites,
    grants: &mut ScriptGrants,
    keying: Keying<'_>,
) -> io::Result<()> {
    let revision = grants.revision;
    let count = grants.plugins.len();
    for (position, grant) in grants.plugins.iter_mut().enumerate() {
        let placement = Placement {
            revision,
            position,
            count,
        };
        grant.mac = match keying.key {
            Some(key) if grant.verified => Some(entry_mac(
                key,
                placement,
                &grant.id,
                &grant.hash,
                grant.enabled,
                grant.granted,
            )),
            _ => None,
        };
        grant.verified = !keying.required || grant.mac.is_some();
    }
    let document = WireDocument {
        format_version: FORMAT_VERSION,
        revision: grants.revision,
        plugins: grants
            .plugins
            .iter()
            .map(|grant| WireEntry {
                id: &grant.id,
                hash: &grant.hash,
                enabled: grant.enabled,
                granted: grant.granted.iter().map(Capability::as_str).collect(),
                mac: grant.mac.as_deref(),
            })
            .collect(),
    };
    let bytes = serde_json::to_vec_pretty(&document).map_err(io::Error::other)?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(io::Error::other("script grants are too large"));
    }
    crate::storage::replace_private_file(&document_path(root), &bytes)
}
