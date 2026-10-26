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
//! - `[[script.commands]]` 每条另有可选的 `arguments`：`"letters"`（默认）时触发词后面只接受小写字母，`"extended"` 时另外接受数字和 `abi::EXTENDED_ARGUMENT_SYMBOLS`。
//! - `[[script.settings]]` 1 到 16 项，用户在授权页插件详情里设置，见 [`ScriptSetting`]；值存在 `script-settings.json`，不进内容哈希，改设置不必重新授权。读值不需要能力。
//! - `[script].data` 1 到 4 个随包只读数据文件，平铺的 `.bin` 文件名，单个 1 字节到 4 MiB、合计不超过 8 MiB，按数据文件走同一份文件检查（存在、非空、算进 16 个文件），不能和模块同名；内容由作者自己编码，宿主只按偏移给原始字节。读它不需要能力。
//! - 载入时模块按 ABI v1 校验，只编译，不执行；导入时（`import`、`validate`）和输入法进程加载时用的是 `msime_script_runtime` 的同一份校验。
//! - 内容哈希是 `sha256("msime-script\0" ‖ len ‖ plugin.toml ‖ len ‖ 模块 ‖ len ‖ data_1 ‖ …)`，`len` 是后面那段的字节数，按 8 字节小端写入；数据文件按清单顺序追加，没有数据文件时什么都不追加，哈希和加数据文件之前逐字节相同（已有授权、官方包锁定的哈希都不变）。说明文件不计入。授权（`script_grants`）绑定在这个哈希上，包的清单、模块或数据一变，授权全部作废。

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
const SCRIPT_KEYS: [&str; 7] = [
    "abi",
    "module",
    "hooks",
    "memory_pages",
    "commands",
    "settings",
    "data",
];
/// `[[script.commands]]` 每条允许的键。
const COMMAND_KEYS: [&str; 3] = ["trigger", "title", "arguments"];
/// `[[script.settings]]` 每项都可以有的键；各类型另有自己的键（[`setting_type_keys`]）。
const SETTING_KEYS: [&str; 5] = ["key", "title", "description", "type", "default"];
/// 设置项 `key` 和选项 `value` 的字节数：小写字母、数字和 `_`。
pub const SETTING_KEY_BYTES: std::ops::RangeInclusive<usize> = 1..=32;
/// 设置项和选项标题的字符数。
pub const SETTING_TITLE_CHARS: std::ops::RangeInclusive<usize> = 1..=24;
/// 设置项说明的字符上限。
pub const SETTING_DESCRIPTION_CHARS: usize = 120;
/// `type = "choice"` 的选项个数。
pub const SETTING_OPTIONS: std::ops::RangeInclusive<usize> = 2..=16;
/// `type = "text"` 的 `max_length`（UTF-16 码元）。
pub const SETTING_TEXT_UTF16: std::ops::RangeInclusive<usize> = 1..=256;
/// 数据文件的扩展名。
pub const DATA_EXTENSION: &str = "bin";
/// 一个清单最多声明几条指令。
pub const MAX_COMMANDS: usize = 4;
/// 插件触发词的字节数（都是小写 ASCII 字母）：至少 2 个，免得占掉单个字母。同引擎的 `local::command::PLUGIN_TRIGGER_LENGTH`。
pub const COMMAND_TRIGGER_BYTES: std::ops::RangeInclusive<usize> = 2..=16;
/// 插件指令标题的字符数。同引擎的 `local::command::PLUGIN_TITLE_CHARS`。
pub const COMMAND_TITLE_CHARS: std::ops::RangeInclusive<usize> = 1..=8;
/// 内置 `/` 指令的触发词：日期、时间、星期、农历和翻译。client-services 不能依赖引擎（`docs/ARCHITECTURE.md`），这里复制了引擎 `local::command::builtin_triggers` 的全集，host-api 的一致性测试守着两边相同。
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
    /// 清单 `[[script.settings]]`，按清单顺序。没有时不序列化。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub settings: Vec<ScriptSetting>,
    /// 清单 `[script].data` 的文件名，按清单顺序。没有时不序列化。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<String>,
    /// 数据文件合计的字节数（授权页的「自带数据 N KB」），文件检查时按目录里的大小填。没有数据文件时为 0，不序列化。
    #[serde(skip_serializing_if = "is_zero")]
    pub data_bytes: u64,
    /// 内容哈希，64 位小写十六进制。只有读过模块的载入（导入、`scan`、[`load`]）才有，否则为空。
    pub content_hash: String,
    /// `DataFiles::Module` 载入时读出的模块字节，由 [`load`] 取走交给调用方，不进序列化。
    #[serde(skip)]
    pub(crate) module_bytes: Option<ModuleBytes>,
    /// `DataFiles::Module` 载入时读出的数据文件字节，按清单顺序，和模块字节一起由 [`load`] 取走。
    #[serde(skip)]
    pub(crate) data_contents: Option<Vec<ModuleBytes>>,
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

/// 插件在 `/` 指令里声明的一条指令。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScriptCommand {
    /// 用户在 `/` 之后打的小写字母。
    pub trigger: String,
    /// 插件行旁显示的标题。
    pub title: String,
    /// 触发词后面接受哪些字符。默认的 `letters` 不序列化，旧包的摘要不变。
    #[serde(skip_serializing_if = "CommandArguments::is_letters")]
    pub arguments: CommandArguments,
}

/// `[[script.commands]].arguments`：触发词后面接受哪些字符。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandArguments {
    /// 只有小写字母，和加这个键之前一样。
    #[default]
    Letters,
    /// 另外接受数字和 `abi::EXTENDED_ARGUMENT_SYMBOLS`。参数里一旦有数字，这一份输入里数字键不再选候选。
    Extended,
}

impl CommandArguments {
    pub fn is_letters(&self) -> bool {
        *self == Self::Letters
    }

    pub fn is_extended(self) -> bool {
        self == Self::Extended
    }

    fn parse(value: Option<&Value>, trigger: &str) -> Result<Self, String> {
        match value {
            None => Ok(Self::Letters),
            Some(Value::String(name)) if name == "letters" => Ok(Self::Letters),
            Some(Value::String(name)) if name == "extended" => Ok(Self::Extended),
            Some(_) => Err(format!(
                "指令 {trigger} 的 arguments 只能是 \"letters\" 或 \"extended\""
            )),
        }
    }
}

/// 清单 `[[script.settings]]` 的一项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScriptSetting {
    /// 1 到 32 个小写字母、数字和 `_`，清单内不重复。`script-settings.json` 按它存值。
    pub key: String,
    /// 1 到 24 个字符。
    pub title: String,
    /// 不超过 120 个字符。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(flatten)]
    pub kind: SettingKind,
}

/// 设置项的类型和默认值。序列化时 `type` 是类型名，其余字段平铺。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SettingKind {
    Bool {
        default: bool,
    },
    Choice {
        options: Vec<SettingOption>,
        /// 其中一个选项的 `value`。
        default: String,
    },
    /// 整数，`min <= default <= max`，都在 `i32` 里。
    Number {
        min: i32,
        max: i32,
        default: i32,
    },
    Text {
        /// 1 到 256 个 UTF-16 码元。
        max_length: u32,
        /// 可以为空。
        default: String,
    },
}

/// `type = "choice"` 的一个选项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettingOption {
    /// 1 到 32 个小写字母、数字和 `_`，同一项里不重复；插件经 `setting_value` 读到的就是它。
    pub value: String,
    /// 1 到 24 个字符。
    pub title: String,
}

impl ScriptSetting {
    /// `value`（`script-settings.json` 里存的 JSON 值）对这一项合不合规：布尔要 `true`/`false`，选项要是某个 `value`，数字要是 `min..=max` 里的整数，文本不超过 `max_length` 个 UTF-16 码元、不含插件文本禁用的字符。
    pub fn accepts(&self, value: &serde_json::Value) -> bool {
        match (&self.kind, value) {
            (SettingKind::Bool { .. }, serde_json::Value::Bool(_)) => true,
            (SettingKind::Choice { options, .. }, serde_json::Value::String(chosen)) => {
                options.iter().any(|option| option.value == *chosen)
            }
            (SettingKind::Number { min, max, .. }, serde_json::Value::Number(number)) => number
                .as_i64()
                .is_some_and(|number| (i64::from(*min)..=i64::from(*max)).contains(&number)),
            (SettingKind::Text { max_length, .. }, serde_json::Value::String(text)) => {
                valid_setting_text(text, *max_length as usize)
            }
            _ => false,
        }
    }

    /// 设置页显示的值，类型同清单：`value` 合规时是它，否则是默认值。
    pub fn value(&self, value: Option<&serde_json::Value>) -> serde_json::Value {
        if let Some(value) = value.filter(|value| self.accepts(value)) {
            return value.clone();
        }
        match &self.kind {
            SettingKind::Bool { default } => serde_json::Value::Bool(*default),
            SettingKind::Choice { default, .. } | SettingKind::Text { default, .. } => {
                serde_json::Value::String(default.clone())
            }
            SettingKind::Number { default, .. } => serde_json::Value::from(*default),
        }
    }

    /// 插件经 `setting_value` 读到的文本：`value` 合规时是它，否则（没存、清单改了类型、超出范围）是默认值。布尔是 `0`/`1`，数字是十进制，选项是 `value`，文本原样。
    pub fn abi_text(&self, value: Option<&serde_json::Value>) -> String {
        let value = value.filter(|value| self.accepts(value));
        match (&self.kind, value) {
            (SettingKind::Bool { .. }, Some(serde_json::Value::Bool(flag)))
            | (SettingKind::Bool { default: flag }, _) => u8::from(*flag).to_string(),
            (
                SettingKind::Choice { .. } | SettingKind::Text { .. },
                Some(serde_json::Value::String(text)),
            ) => text.clone(),
            (SettingKind::Choice { default, .. } | SettingKind::Text { default, .. }, _) => {
                default.clone()
            }
            (SettingKind::Number { .. }, Some(serde_json::Value::Number(number))) => {
                number.to_string()
            }
            (SettingKind::Number { default, .. }, _) => default.to_string(),
        }
    }
}

/// 清单里每一项设置交给插件的值，按清单顺序：`stored` 里有这个 `key`、而且对这一项合规的用它，否则用默认值。清单删掉的项（`stored` 里多出来的键）不交给插件。
pub fn setting_values(
    settings: &[ScriptSetting],
    stored: &serde_json::Map<String, serde_json::Value>,
) -> Vec<String> {
    settings
        .iter()
        .map(|setting| setting.abi_text(stored.get(&setting.key)))
        .collect()
}

/// 文本设置的值：不超过 `max_length` 个 UTF-16 码元，不含插件文本禁用的字符（和注释同一条规则），可以为空。
fn valid_setting_text(text: &str, max_length: usize) -> bool {
    text.encode_utf16().count() <= max_length && !abi::has_forbidden_plugin_char(text)
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
    let settings = parse_settings(script)?;
    let data = parse_data(script, module)?;
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
        settings,
        data,
        data_bytes: 0,
        content_hash: String::new(),
        module_bytes: None,
        data_contents: None,
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
        let arguments = CommandArguments::parse(row.get("arguments"), trigger)?;
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
            arguments,
        });
    }
    Ok(commands)
}

/// 设置项 `key` 和选项 `value`：1 到 32 个小写 ASCII 字母、数字和 `_`。
fn valid_setting_key(key: &str) -> bool {
    SETTING_KEY_BYTES.contains(&key.len())
        && key
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// 设置项和选项的标题：1 到 24 个字符，不含插件文本禁用的字符，首尾不是空白。
fn valid_setting_title(title: &str) -> bool {
    SETTING_TITLE_CHARS.contains(&title.chars().count())
        && !abi::has_forbidden_plugin_char(title)
        && title.trim() == title
}

/// 各类型在 [`SETTING_KEYS`] 之外的键。
fn setting_type_keys(kind: &str) -> Option<&'static [&'static str]> {
    match kind {
        "bool" => Some(&[]),
        "choice" => Some(&["options"]),
        "number" => Some(&["min", "max"]),
        "text" => Some(&["max_length"]),
        _ => None,
    }
}

/// `[[script.settings]]`：不写是空表。
fn parse_settings(script: &toml::map::Map<String, Value>) -> Result<Vec<ScriptSetting>, String> {
    let items = match script.get("settings") {
        None => return Ok(Vec::new()),
        Some(Value::Array(items)) => items,
        Some(_) => return Err("script.settings 必须是表的数组（[[script.settings]]）".into()),
    };
    if items.is_empty() || items.len() > limits::MAX_SETTINGS {
        return Err(format!(
            "script.settings 要有 1 到 {} 项",
            limits::MAX_SETTINGS
        ));
    }
    let mut settings: Vec<ScriptSetting> = Vec::with_capacity(items.len());
    for item in items {
        let row = item
            .as_table()
            .ok_or("script.settings 里的每一项都必须是一个表")?;
        let key = row
            .get("key")
            .and_then(Value::as_str)
            .ok_or("script.settings 的每一项都需要字符串 key")?;
        if !valid_setting_key(key) {
            return Err(format!(
                "设置项 {key} 的 key 必须是 1 到 32 个小写字母、数字或下划线"
            ));
        }
        if settings.iter().any(|earlier| earlier.key == key) {
            return Err(format!("设置项 {key} 重复了"));
        }
        let kind = row
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("设置项 {key} 需要字符串 type"))?;
        let type_keys = setting_type_keys(kind)
            .ok_or_else(|| format!("设置项 {key} 的 type 只能是 bool、choice、number 或 text"))?;
        let allowed: Vec<&str> = SETTING_KEYS.iter().chain(type_keys).copied().collect();
        only_keys(row, &allowed, &format!("设置项 {key}"))?;
        let title = row
            .get("title")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("设置项 {key} 需要字符串 title"))?;
        if !valid_setting_title(title) {
            return Err(format!(
                "设置项 {key} 的标题必须是 1 到 24 个字符，首尾不是空白，不含不可见的字符"
            ));
        }
        let description = match row.get("description") {
            None => None,
            Some(Value::String(text))
                if text.chars().count() <= SETTING_DESCRIPTION_CHARS
                    && !abi::has_forbidden_plugin_char(text) =>
            {
                Some(text.clone())
            }
            Some(_) => {
                return Err(format!(
                    "设置项 {key} 的 description 必须是不超过 120 个字符的字符串，不含不可见的字符"
                ))
            }
        };
        let default = row
            .get("default")
            .ok_or_else(|| format!("设置项 {key} 需要 default"))?;
        let kind = parse_setting_kind(key, kind, row, default)?;
        settings.push(ScriptSetting {
            key: key.to_owned(),
            title: title.to_owned(),
            description,
            kind,
        });
    }
    Ok(settings)
}

fn parse_setting_kind(
    key: &str,
    kind: &str,
    row: &toml::map::Map<String, Value>,
    default: &Value,
) -> Result<SettingKind, String> {
    match kind {
        "bool" => default
            .as_bool()
            .map(|default| SettingKind::Bool { default })
            .ok_or_else(|| format!("设置项 {key} 的 default 必须是 true 或 false")),
        "choice" => {
            let items = row
                .get("options")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("设置项 {key} 需要 options 数组"))?;
            if !SETTING_OPTIONS.contains(&items.len()) {
                return Err(format!("设置项 {key} 的 options 要有 2 到 16 个选项"));
            }
            let mut options: Vec<SettingOption> = Vec::with_capacity(items.len());
            for item in items {
                let option = item
                    .as_table()
                    .ok_or_else(|| format!("设置项 {key} 的每个选项都必须是一个表"))?;
                only_keys(option, &["value", "title"], &format!("设置项 {key} 的选项"))?;
                let value = option.get("value").and_then(Value::as_str);
                let title = option.get("title").and_then(Value::as_str);
                let (Some(value), Some(title)) = (value, title) else {
                    return Err(format!(
                        "设置项 {key} 的每个选项都需要字符串 value 和 title"
                    ));
                };
                if !valid_setting_key(value) {
                    return Err(format!(
                        "设置项 {key} 的选项 {value} 必须是 1 到 32 个小写字母、数字或下划线"
                    ));
                }
                if options.iter().any(|earlier| earlier.value == value) {
                    return Err(format!("设置项 {key} 的选项 {value} 重复了"));
                }
                if !valid_setting_title(title) {
                    return Err(format!(
                        "设置项 {key} 的选项 {value} 的标题必须是 1 到 24 个字符，首尾不是空白，不含不可见的字符"
                    ));
                }
                options.push(SettingOption {
                    value: value.to_owned(),
                    title: title.to_owned(),
                });
            }
            let default = default
                .as_str()
                .filter(|default| options.iter().any(|option| option.value == *default))
                .ok_or_else(|| format!("设置项 {key} 的 default 必须是其中一个选项的 value"))?;
            Ok(SettingKind::Choice {
                default: default.to_owned(),
                options,
            })
        }
        "number" => {
            let integer = |name: &str, value: Option<&Value>| {
                value
                    .and_then(Value::as_integer)
                    .and_then(|number| i32::try_from(number).ok())
                    .ok_or_else(|| format!("设置项 {key} 的 {name} 必须是 i32 范围内的整数"))
            };
            let min = integer("min", row.get("min"))?;
            let max = integer("max", row.get("max"))?;
            let default = integer("default", Some(default))?;
            if !(min <= default && default <= max) {
                return Err(format!("设置项 {key} 要满足 min <= default <= max"));
            }
            Ok(SettingKind::Number { min, max, default })
        }
        "text" => {
            let max_length = row
                .get("max_length")
                .and_then(Value::as_integer)
                .and_then(|length| usize::try_from(length).ok())
                .filter(|length| SETTING_TEXT_UTF16.contains(length))
                .ok_or_else(|| format!("设置项 {key} 的 max_length 必须是 1 到 256 之间的整数"))?;
            let default = default
                .as_str()
                .filter(|default| valid_setting_text(default, max_length))
                .ok_or_else(|| {
                    format!(
                        "设置项 {key} 的 default 必须是不超过 max_length 个 UTF-16 码元的字符串，不含不可见的字符"
                    )
                })?;
            Ok(SettingKind::Text {
                max_length: max_length as u32,
                default: default.to_owned(),
            })
        }
        _ => Err(format!(
            "设置项 {key} 的 type 只能是 bool、choice、number 或 text"
        )),
    }
}

/// `[script].data`：不写是空表。文件名是平铺的 `.bin`，不重复，不和模块同名。
fn parse_data(script: &toml::map::Map<String, Value>, module: &str) -> Result<Vec<String>, String> {
    let items = match script.get("data") {
        None => return Ok(Vec::new()),
        Some(Value::Array(items)) => items,
        Some(_) => return Err("script.data 必须是数组".into()),
    };
    if items.is_empty() || items.len() > limits::MAX_DATA_FILES {
        return Err(format!(
            "script.data 要有 1 到 {} 个文件",
            limits::MAX_DATA_FILES
        ));
    }
    let mut data: Vec<String> = Vec::with_capacity(items.len());
    for item in items {
        let name = item
            .as_str()
            .ok_or("script.data 里的每一项都必须是字符串")?;
        if !valid_file_name(name) {
            return Err(format!("script.data 的文件名 {name} 无效"));
        }
        if name == module || data.iter().any(|earlier| earlier == name) {
            return Err(format!("script.data 里的 {name} 重复了"));
        }
        data.push(name.to_owned());
    }
    Ok(data)
}

/// 模块在文件检查里的登记：必须存在、1 字节到 256 KiB、扩展名 `.wasm`。
pub(crate) fn module_file(pack: &ScriptPack) -> DataFile {
    DataFile {
        name: pack.module.clone(),
        max_bytes: MAX_MODULE_BYTES,
        extension: MODULE_EXTENSION,
    }
}

/// 数据文件在文件检查里的登记：必须存在、1 字节到 4 MiB、扩展名 `.bin`。
pub(crate) fn data_files(pack: &ScriptPack) -> impl Iterator<Item = DataFile> + '_ {
    pack.data.iter().map(|name| DataFile {
        name: name.clone(),
        max_bytes: limits::MAX_DATA_FILE_BYTES as u64,
        extension: DATA_EXTENSION,
    })
}

/// 文件检查之后核对数据文件合计不超过 8 MiB，并记下合计的字节数。`files` 是目录里的文件和大小，数据文件已经由文件检查确认在里面。
pub(crate) fn check_data_total(
    pack: &mut ScriptPack,
    files: &super::PackFiles,
) -> Result<(), String> {
    let total = pack
        .data
        .iter()
        .map(|name| files.get(name).copied().unwrap_or_default())
        .fold(0u64, u64::saturating_add);
    if total > limits::MAX_DATA_TOTAL_BYTES as u64 {
        return Err("script.data 的文件加起来太大".into());
    }
    pack.data_bytes = total;
    Ok(())
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
    let mut data = Vec::with_capacity(pack.data.len());
    let mut total = 0usize;
    for name in &pack.data {
        let bytes = read_file(directory, name, limits::MAX_DATA_FILE_BYTES as u64)
            .map_err(|_| format!("{name} 无法读取或太大"))?;
        if bytes.is_empty() {
            return Err(format!("{name} 为空或太大"));
        }
        total = total.saturating_add(bytes.len());
        data.push(bytes);
    }
    // 文件检查之后文件可能被换过，按实际读到的再查一次合计。
    if total > limits::MAX_DATA_TOTAL_BYTES {
        return Err("script.data 的文件加起来太大".into());
    }
    pack.data_bytes = total as u64;
    pack.content_hash = content_hash_with_data(manifest, &wasm, &data);
    if data_files == DataFiles::Module {
        pack.module_bytes = Some(ModuleBytes(wasm));
        pack.data_contents = Some(data.into_iter().map(ModuleBytes).collect());
    } else {
        msime_script_runtime::validate(&wasm, pack.declaration())
            .map_err(|error| format!("{} 不是有效的脚本模块：{error}", pack.module))?;
    }
    Ok(pack)
}

/// 没有数据文件的脚本包的内容哈希：`sha256("msime-script\0" ‖ len ‖ plugin.toml ‖ len ‖ 模块)`，64 位小写十六进制。
pub fn content_hash(manifest: &[u8], wasm: &[u8]) -> String {
    content_hash_with_data(manifest, wasm, &[])
}

/// 脚本包的内容哈希：[`content_hash`] 的那几段之后，按清单顺序给每个数据文件追加 `len ‖ data_i`。`data` 为空时什么都不追加，和 [`content_hash`] 逐字节相同。
pub fn content_hash_with_data(manifest: &[u8], wasm: &[u8], data: &[Vec<u8>]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(HASH_DOMAIN);
    hasher.update((manifest.len() as u64).to_le_bytes());
    hasher.update(manifest);
    hasher.update((wasm.len() as u64).to_le_bytes());
    hasher.update(wasm);
    for bytes in data {
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    hex::encode(hasher.finalize())
}

/// 输入法进程要编译的一个已安装脚本包。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedScript {
    pub id: String,
    pub pack: ScriptPack,
    /// 模块字节，和 `pack.content_hash` 出自同一次读取。还没有校验过：交给 `msime_script_runtime::compile`，它就是导入时那份校验。
    pub wasm: Vec<u8>,
    /// 数据文件的字节，按清单 `[script].data` 的顺序，和 `pack.content_hash` 出自同一次读取。没有数据文件时为空。
    pub data: Vec<Vec<u8>>,
}

impl LoadedScript {
    /// 在进程内存总账里和模块无关、原样常驻的字节（`limits::resident_bytes`）：数据文件按 1 倍记，申请了 `persist_data` 的再加一块存储缓冲。
    pub fn resident_bytes(&self) -> usize {
        limits::resident_bytes(
            self.data.iter().map(Vec::len).sum(),
            self.pack.permissions.contains(Capability::PersistData),
        )
    }
}

/// 读出脚本包 `id`：官方 id 从内置目录 `builtin_packs` 读，其他 id 从 `root` 下已安装的包读。清单和文件照常检查，模块读出来但不在这里编译。宿主的后台编译线程用它，编译一次就够，不必先校验一遍再编译一遍。
/// 只读清单、只查文件存在和大小的已安装脚本包（不读模块和数据文件，不算内容哈希）：设置页和输入法要设置项、权限时用。官方脚本插件从 `builtin_packs` 读。
pub fn declared(root: &Path, builtin_packs: Option<&Path>, id: &str) -> Result<ScriptPack, String> {
    let summary = super::load_package_unread(
        root,
        BuiltinDirs::packs(builtin_packs),
        PluginKind::Script,
        id,
    )?;
    match summary.content {
        PluginContent::Script(pack) => Ok(pack),
        _ => Err("plugin.toml 里的 kind 与所在目录不一致".into()),
    }
}

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
    let data = pack
        .data_contents
        .take()
        .unwrap_or_default()
        .into_iter()
        .map(|bytes| bytes.0)
        .collect();
    Ok(LoadedScript {
        id: summary.id,
        pack,
        wasm,
        data,
    })
}
