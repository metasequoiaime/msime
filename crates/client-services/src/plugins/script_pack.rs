//! `kind = "script"`：桌面脚本插件。只在 macOS、Windows 和非 ohos 的 Linux 上编译，移动端没有这个类型。
//!
//! ```toml
//! schema_version = 2
//! kind = "script"
//! id = "pangu-spacing"
//! name = "中英文之间加空格"
//! version = "1.0.0"
//! license = "MIT"
//! permissions = ["read_commit", "rewrite_commit"]
//! [script]
//! abi = 1
//! module = "pangu.wasm"
//! hooks = ["before_commit"]
//! memory_pages = 16
//! ```
//!
//! 在 `/` 指令里给出候选的插件另外声明自己的触发词：
//!
//! ```toml
//! permissions = ["provide_commands"]
//! [script]
//! abi = 1
//! module = "uuid.wasm"
//! hooks = ["command"]
//! [[script.commands]]
//! trigger = "uuid"
//! title = "UUID"
//! ```
//!
//! - 清单必须写 `schema_version = 2`；其余 8 种类型仍只认 1，所以不认识 script 的旧客户端在版本检查处就会拒绝它。
//! - `permissions` 只认 `msime_script_runtime::Capability` 的白名单，不能重复，依赖必须一起申请（改写上屏要 `read_commit`，三种候选变换要 `read_candidates`），`rewrite_commit` 和 `read_composition` 不能同时申请。申请了就必须在模块里用到，反过来也一样，这一条由模块校验检查。
//! - `[script]` 只有 5 个键：`abi` 必须是 1；`module` 点名包里的一个平铺 `.wasm` 文件，1 字节到 256 KiB，和辅助码表一样按数据文件走同一份文件检查；`hooks` 是非空、不重复的钩子名数组，只有声明过的钩子才会投递；`memory_pages` 可选，1 到 64，默认 16；`commands` 可选，见下一条。
//! - `[[script.commands]]` 1 到 4 条，每条只有 `trigger` 和 `title`。触发词 2 到 16 个小写 ASCII 字母，不等于、也不和任何内置触发词（[`BUILTIN_COMMAND_TRIGGERS`]）互为前缀，同一清单里的触发词不重复、不互为前缀；标题 1 到 8 个字符，不含插件文本禁用的字符（`abi::has_forbidden_plugin_char`），首尾不是空白，不等于内置指令的标题（[`BUILTIN_COMMAND_TITLES`]）。声明了 `commands` 就必须申请 `provide_commands` 并声明 `command` 钩子，反过来也一样。和别的插件、用户指令表之间的冲突在这里查不了，由输入法按「用户指令表赢、插件先到先得」决定哪条生效。
//! - 载入时模块按 ABI v1 校验，只编译，不执行；导入时（`import`、`validate`）和输入法进程加载时用的是 `msime_script_runtime` 的同一份校验。
//! - 内容哈希是 `sha256("msime-script\0" ‖ len ‖ plugin.toml ‖ len ‖ 模块)`，`len` 是后面那段的字节数，按 8 字节小端写入；说明文件不计入。授权（`script_grants`）绑定在这个哈希上，包的清单或模块一变，授权全部作废。

use msime_script_runtime::{
    abi, limits, Capability, CapabilityRuleError, CapabilitySet, Declaration, Hook, HookSet,
};
use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::path::Path;
use toml::Value;

use super::{
    load_installed, only_keys, pack_parent, read_file, valid_file_name, BuiltinDirs, DataFile,
    DataFiles, PluginContent, PluginKind,
};

/// 脚本插件清单的 `schema_version`。
pub const SCHEMA_VERSION: i64 = 2;
pub(crate) const MANIFEST_KEYS: [&str; 1] = ["script"];
/// `[script]` 表里允许的键。
const SCRIPT_KEYS: [&str; 5] = ["abi", "module", "hooks", "memory_pages", "commands"];
/// `[[script.commands]]` 每条允许的键。
const COMMAND_KEYS: [&str; 2] = ["trigger", "title"];
/// 一个清单最多声明几条指令。
pub const MAX_COMMANDS: usize = 4;
/// 插件触发词的字节数（都是小写 ASCII 字母）：至少 2 个，免得占掉单个字母。同引擎的 `local::command::PLUGIN_TRIGGER_LENGTH`。
pub const COMMAND_TRIGGER_BYTES: std::ops::RangeInclusive<usize> = 2..=16;
/// 插件指令标题的字符数。同引擎的 `local::command::PLUGIN_TITLE_CHARS`。
pub const COMMAND_TITLE_CHARS: std::ops::RangeInclusive<usize> = 1..=8;
/// 内置 `/` 指令的触发词：日期、时间、星期、农历和翻译。client-core 不能依赖引擎（`docs/ARCHITECTURE.md`），这里复制了引擎 `local::command::builtin_triggers` 的全集，host-api 的一致性测试守着两边相同。
pub const BUILTIN_COMMAND_TRIGGERS: [&str; 15] = [
    "rq",
    "riqi",
    "date",
    "sj",
    "shijian",
    "time",
    "xq",
    "xingqi",
    "week",
    "nl",
    "nongli",
    "yinli",
    "translate",
    "fanyi",
    "fy",
];
/// 内置 `/` 指令的标题，同引擎的 `local::command::builtin_titles`，同样由一致性测试守着。
pub const BUILTIN_COMMAND_TITLES: [&str; 5] = ["日期", "时间", "星期", "农历", "翻译"];
/// 模块文件的字节上限，即运行时的 `limits::MAX_MODULE_BYTES`。
pub const MAX_MODULE_BYTES: u64 = limits::MAX_MODULE_BYTES as u64;
/// 模块文件的扩展名。
pub const MODULE_EXTENSION: &str = "wasm";
/// 内容哈希的域分隔前缀，让它不可能和别处对同样字节算的 SHA-256 相同。
const HASH_DOMAIN: &[u8] = b"msime-script\0";

/// 设置页和宿主看到的脚本包。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScriptPack {
    /// 清单 `[script].abi`，目前只能是 1。
    pub abi: i32,
    /// 模块文件名。
    pub module: String,
    /// 声明的钩子，序列化成钩子名。
    #[serde(serialize_with = "hook_names")]
    pub hooks: HookSet,
    /// 清单申请的能力，序列化成能力名。实际生效的是它和用户授权的交集。
    #[serde(serialize_with = "capability_names")]
    pub permissions: CapabilitySet,
    /// 每个实例一次预留的线性内存页数。
    pub memory_pages: u32,
    /// 清单 `[[script.commands]]`：插件在 `/` 指令里声明的触发词和标题，按清单顺序。没有时不序列化，旧包的摘要不变。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<ScriptCommand>,
    /// 内容哈希，64 位小写十六进制。只有读过模块的载入（导入、`scan`、[`load`]）才有，否则为空。
    pub content_hash: String,
    /// `DataFiles::Module` 载入时读出的模块字节，由 [`load`] 取走交给调用方，不进序列化。
    #[serde(skip)]
    pub(crate) module_bytes: Option<ModuleBytes>,
}

/// 插件在 `/` 指令里声明的一条指令。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScriptCommand {
    /// 用户在 `/` 之后打的小写字母。
    pub trigger: String,
    /// 插件行旁显示的标题。
    pub title: String,
}

impl ScriptPack {
    /// 交给 `msime_script_runtime::compile`/`validate` 的声明。
    pub fn declaration(&self) -> Declaration {
        Declaration {
            capabilities: self.permissions,
            hooks: self.hooks,
            memory_pages: self.memory_pages,
        }
    }
}

/// 模块字节。`Debug` 只写长度，免得一次断言失败把整个模块打进日志。
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ModuleBytes(pub(crate) Vec<u8>);

impl std::fmt::Debug for ModuleBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ModuleBytes({} bytes)", self.0.len())
    }
}

fn hook_names<S: Serializer>(hooks: &HookSet, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(hooks.iter().map(Hook::as_str))
}

fn capability_names<S: Serializer>(
    capabilities: &CapabilitySet,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(capabilities.iter().map(Capability::as_str))
}

/// 按类型要求 `schema_version`：script 必须写 2，其他类型只认 1。调用方已经排除了 1 和 2 以外的值。
pub(crate) fn check_schema_version(kind: PluginKind, version: Option<i64>) -> Result<(), String> {
    match (kind.is_script(), version) {
        (true, Some(SCHEMA_VERSION)) | (false, Some(1)) => Ok(()),
        (true, _) => Err(format!(
            "不支持这个 schema_version，script 插件必须写 schema_version = {SCHEMA_VERSION}"
        )),
        (false, _) => Err("不支持这个 schema_version".into()),
    }
}

/// 脚本插件的 `permissions`：白名单里的能力名，不重复，满足依赖和互斥规则。不写等于不申请任何能力。
pub(crate) fn parse_permissions(
    table: &toml::map::Map<String, Value>,
) -> Result<CapabilitySet, String> {
    let items = match table.get("permissions") {
        None => return Ok(CapabilitySet::EMPTY),
        Some(Value::Array(items)) => items,
        Some(_) => return Err("permissions 必须是数组".into()),
    };
    let mut permissions = CapabilitySet::EMPTY;
    for item in items {
        let name = item
            .as_str()
            .ok_or("permissions 里的每一项都必须是字符串")?;
        let capability =
            Capability::parse(name).ok_or_else(|| format!("permissions 里有未知的权限 {name}"))?;
        if permissions.contains(capability) {
            return Err(format!("permissions 里的 {name} 重复了"));
        }
        permissions.insert(capability);
    }
    permissions.check_rules().map_err(rule_message)?;
    Ok(permissions)
}

/// 能力组合规则的中文说明，清单校验、授权写入和 `msime-pack run --grant` 共用。
pub fn rule_message(error: CapabilityRuleError) -> String {
    match error {
        CapabilityRuleError::MissingDependency {
            capability,
            requires,
        } => format!(
            "申请 {} 必须同时申请 {}",
            capability.as_str(),
            requires.as_str()
        ),
        CapabilityRuleError::Exclusive { first, second } => {
            format!("{} 不能和 {} 同时申请", first.as_str(), second.as_str())
        }
    }
}

/// `[script]` 表。模块内容由 [`read`] 在文件检查之后读取。
pub(crate) fn parse(
    table: &toml::map::Map<String, Value>,
    permissions: CapabilitySet,
) -> Result<ScriptPack, String> {
    let script = table
        .get("script")
        .and_then(Value::as_table)
        .ok_or("脚本插件缺少 script 表")?;
    only_keys(script, &SCRIPT_KEYS, "script")?;
    if script.get("abi").and_then(Value::as_integer) != Some(i64::from(abi::ABI_VERSION)) {
        return Err(format!("script.abi 只能是 {}", abi::ABI_VERSION));
    }
    let module = script
        .get("module")
        .and_then(Value::as_str)
        .ok_or("script.module 必须是字符串")?;
    if !valid_file_name(module) {
        return Err(format!("script.module 的文件名 {module} 无效"));
    }
    let names = script
        .get("hooks")
        .and_then(Value::as_array)
        .ok_or("script.hooks 必须是数组")?;
    if names.is_empty() {
        return Err("script.hooks 至少要有一个钩子".into());
    }
    let mut hooks = HookSet::EMPTY;
    for name in names {
        let name = name
            .as_str()
            .ok_or("script.hooks 里的每一项都必须是字符串")?;
        let hook =
            Hook::parse(name).ok_or_else(|| format!("script.hooks 里有未知的钩子 {name}"))?;
        if hooks.contains(hook) {
            return Err(format!("script.hooks 里的 {name} 重复了"));
        }
        hooks.insert(hook);
    }
    let commands = parse_commands(script)?;
    match (
        !commands.is_empty(),
        permissions.contains(Capability::ProvideCommands),
        hooks.contains(Hook::Command),
    ) {
        (true, true, true) | (false, false, false) => {}
        (true, false, _) => return Err("声明了 script.commands 就必须申请 provide_commands".into()),
        (true, true, false) => return Err("声明了 script.commands 就必须声明 command 钩子".into()),
        (false, true, _) => {
            return Err("申请了 provide_commands 就必须在 script.commands 里声明指令".into())
        }
        (false, false, true) => {
            return Err("声明了 command 钩子就必须在 script.commands 里声明指令".into())
        }
    }
    let memory_pages = match script.get("memory_pages") {
        None => limits::DEFAULT_MEMORY_PAGES,
        Some(value) => value
            .as_integer()
            .and_then(|pages| u32::try_from(pages).ok())
            .filter(|pages| (1..=limits::MAX_MEMORY_PAGES).contains(pages))
            .ok_or_else(|| {
                format!(
                    "script.memory_pages 必须是 1 到 {} 之间的整数",
                    limits::MAX_MEMORY_PAGES
                )
            })?,
    };
    Ok(ScriptPack {
        abi: abi::ABI_VERSION,
        module: module.to_owned(),
        hooks,
        permissions,
        memory_pages,
        commands,
        content_hash: String::new(),
        module_bytes: None,
    })
}

/// 两个触发词相等或一个是另一个的前缀：用户打着其中一个时，分不清是在打哪一个。同引擎的规则。
fn triggers_overlap(left: &str, right: &str) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

/// `[[script.commands]]`：不写是空表。
fn parse_commands(script: &toml::map::Map<String, Value>) -> Result<Vec<ScriptCommand>, String> {
    let items = match script.get("commands") {
        None => return Ok(Vec::new()),
        Some(Value::Array(items)) => items,
        Some(_) => return Err("script.commands 必须是表的数组（[[script.commands]]）".into()),
    };
    if items.is_empty() || items.len() > MAX_COMMANDS {
        return Err(format!("script.commands 要有 1 到 {MAX_COMMANDS} 条指令"));
    }
    let mut commands: Vec<ScriptCommand> = Vec::with_capacity(items.len());
    for item in items {
        let row = item
            .as_table()
            .ok_or("script.commands 里的每一条都必须是一个表")?;
        only_keys(row, &COMMAND_KEYS, "script.commands")?;
        let field = |key: &str| {
            row.get(key)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("script.commands 的每一条都需要字符串 {key}"))
        };
        let trigger = field("trigger")?;
        let title = field("title")?;
        if !COMMAND_TRIGGER_BYTES.contains(&trigger.len())
            || !trigger.bytes().all(|byte| byte.is_ascii_lowercase())
        {
            return Err(format!("指令触发词 {trigger} 必须是 2 到 16 个小写字母"));
        }
        if let Some(builtin) = BUILTIN_COMMAND_TRIGGERS
            .iter()
            .find(|builtin| triggers_overlap(trigger, builtin))
        {
            return Err(format!(
                "指令触发词 {trigger} 和内置指令 /{builtin} 相同或互为前缀"
            ));
        }
        if let Some(earlier) = commands
            .iter()
            .find(|earlier| triggers_overlap(trigger, &earlier.trigger))
        {
            return Err(if earlier.trigger == trigger {
                format!("指令触发词 {trigger} 重复了")
            } else {
                format!("指令触发词 {trigger} 和 {} 互为前缀", earlier.trigger)
            });
        }
        if !COMMAND_TITLE_CHARS.contains(&title.chars().count()) {
            return Err(format!("指令 {trigger} 的标题必须是 1 到 8 个字符"));
        }
        if abi::has_forbidden_plugin_char(title) {
            return Err(format!("指令 {trigger} 的标题含有不允许的字符"));
        }
        if title.trim() != title {
            return Err(format!("指令 {trigger} 的标题首尾不能是空白"));
        }
        if BUILTIN_COMMAND_TITLES.contains(&title) {
            return Err(format!(
                "指令 {trigger} 的标题不能和内置指令的标题「{title}」相同"
            ));
        }
        commands.push(ScriptCommand {
            trigger: trigger.to_owned(),
            title: title.to_owned(),
        });
    }
    Ok(commands)
}

/// 模块在文件检查里的登记：必须存在、1 字节到 256 KiB、扩展名 `.wasm`。
pub(crate) fn module_file(pack: &ScriptPack) -> DataFile {
    DataFile {
        name: pack.module.clone(),
        max_bytes: MAX_MODULE_BYTES,
        extension: MODULE_EXTENSION,
    }
}

/// 文件检查之后读出模块、算出内容哈希。`Summarize` 按 ABI v1 校验模块（只编译，不执行），不留字节；`Module` 不校验，把字节留给 [`load`] 的调用方去编译。`manifest` 是刚才解析的那份 `plugin.toml` 字节，哈希和解析出自同一次读取。
pub(super) fn read(
    directory: &Path,
    mut pack: ScriptPack,
    manifest: &[u8],
    data_files: DataFiles,
) -> Result<ScriptPack, String> {
    let wasm = read_file(directory, &pack.module, MAX_MODULE_BYTES)
        .map_err(|_| format!("{} 无法读取或太大", pack.module))?;
    pack.content_hash = content_hash(manifest, &wasm);
    if data_files == DataFiles::Module {
        pack.module_bytes = Some(ModuleBytes(wasm));
    } else {
        msime_script_runtime::validate(&wasm, pack.declaration())
            .map_err(|error| format!("{} 不是有效的脚本模块：{error}", pack.module))?;
    }
    Ok(pack)
}

/// 脚本包的内容哈希：`sha256("msime-script\0" ‖ len ‖ plugin.toml ‖ len ‖ 模块)`，64 位小写十六进制。
pub fn content_hash(manifest: &[u8], wasm: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(HASH_DOMAIN);
    hasher.update((manifest.len() as u64).to_le_bytes());
    hasher.update(manifest);
    hasher.update((wasm.len() as u64).to_le_bytes());
    hasher.update(wasm);
    hex::encode(hasher.finalize())
}

/// 输入法进程要编译的一个已安装脚本包。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedScript {
    pub id: String,
    pub pack: ScriptPack,
    /// 模块字节，和 `pack.content_hash` 出自同一次读取。还没有校验过：交给 `msime_script_runtime::compile`，它就是导入时那份校验。
    pub wasm: Vec<u8>,
}

/// 读出脚本包 `id`：官方 id 从内置目录 `builtin_packs` 读，其他 id 从 `root` 下已安装的包读。清单和文件照常检查，模块读出来但不在这里编译。宿主的后台编译线程用它，编译一次就够，不必先校验一遍再编译一遍。
pub fn load(root: &Path, builtin_packs: Option<&Path>, id: &str) -> Result<LoadedScript, String> {
    let (parent, builtin) = pack_parent(
        root,
        BuiltinDirs::packs(builtin_packs),
        PluginKind::Script,
        id,
    )?;
    let summary = load_installed(&parent, id, PluginKind::Script, builtin, DataFiles::Module)?;
    let PluginContent::Script(mut pack) = summary.content else {
        return Err("plugin.toml 里的 kind 与所在目录不一致".into());
    };
    let wasm = pack
        .module_bytes
        .take()
        .ok_or_else(|| format!("{} 无法读取或太大", pack.module))?
        .0;
    Ok(LoadedScript {
        id: summary.id,
        pack,
        wasm,
    })
}
