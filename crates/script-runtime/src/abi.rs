//! 脚本 ABI v1 的常量：能力、钩子、事件字段编号、import 白名单和返回码。
//!
//! 这些编号是插件二进制和宿主之间的契约，发布之后只能追加，不能改值或复用。导入时的清单校验（client-services）、作者工具（pack-tool）和输入法进程里的执行器都从这里取同一份定义。

/// 模块必须从 `msime_abi()` 返回的版本号。
pub const ABI_VERSION: i32 = 1;

/// import 的模块名。其他命名空间（包括 WASI）一律拒绝。
pub const IMPORT_MODULE: &str = "msime";

/// 必须导出的线性内存名。
pub const EXPORT_MEMORY: &str = "memory";
/// 必须导出：`() -> i32`，返回 [`ABI_VERSION`]。
pub const EXPORT_ABI: &str = "msime_abi";
/// 必须导出：`(kind: i32) -> i32`，`kind` 是 [`Hook::code`]，返回负数算这一次失败。
pub const EXPORT_ON_EVENT: &str = "msime_on_event";
/// 可选导出：`() -> i32`，实例化后调用一次，返回负数算初始化失败。
pub const EXPORT_INIT: &str = "msime_init";

/// import 的返回码。成功时返回非负数（复制的字节数、字段值）。
pub mod status {
    /// 没有授予这个 import 所属的能力。
    pub const DENIED: i32 = -1;
    /// 本次事件没有这个字段，或者它被隐私规则屏蔽。两者故意不可区分。
    pub const ABSENT: i32 = -2;
    /// 字段编号不认识，或者参数不合法。
    pub const INVALID: i32 = -3;
    /// 目标缓冲区放不下，什么都没有复制；先用对应的 `*_scalar` 读长度。
    pub const LIMIT: i32 = -4;
}

/// `event_scalar(field) -> i64` 的字段。只有枚举值，不带文本，也不需要能力。
pub mod event_field {
    /// 事件类型，即 [`super::Hook::code`]。
    pub const KIND: i32 = 0;
    /// 上屏来源，`commit` 与 `before_commit`。
    pub const ORIGIN: i32 = 1;
    /// 怎么选中的，`candidate_selected`。
    pub const HOW: i32 = 2;
    /// 选中之后的结果，`candidate_selected`。
    pub const OUTCOME: i32 = 3;
    /// 组字结束或候选隐藏的原因。
    pub const REASON: i32 = 4;
    /// 本地模式类别。
    pub const LOCAL_MODE: i32 = 5;
    /// 打字前那次派发的动作类别，`input`。
    pub const INPUT_CLASS: i32 = 6;
    /// 撤回暂存选择的方式，`selection_retracted`。
    pub const RETRACTION: i32 = 7;
    /// 0 或 1：这次上屏能否改写，`before_commit`。
    pub const REWRITABLE: i32 = 8;
    /// 0 或 1：同一次 transition 里接着有新的组字，`commit`。
    pub const CONTINUES: i32 = 9;
    /// 0 或 1：引擎生成的文本（计算器、命令），`commit`。
    pub const GENERATED: i32 = 10;
}

/// `composition_text(field, dst, cap)` 与 `composition_scalar(field)` 的字段，需要 `read_composition`。
pub mod composition_field {
    /// 文本：预编辑串；标量：它的 UTF-8 字节数。
    pub const PREEDIT: i32 = 0;
    /// 文本：读音；标量：它的 UTF-8 字节数。
    pub const READING: i32 = 1;
    /// 文本：`input` 事件里这次打的字符；标量：它的 UTF-8 字节数。
    pub const INPUT_CHAR: i32 = 2;
    /// 只有标量：光标前已经打了几个编码字符。按的是编辑串（宿主看到的 `editing_text`，打下的字母本身）里的位置，不是预编辑串里的：两者在笔画、韩文、九键下不是同一个串。
    pub const CARET: i32 = 3;
}

/// `commit_text(field, dst, cap)` 与 `commit_scalar(field)` 的字段，需要 `read_commit`。
pub mod commit_field {
    /// 文本：选中的候选；标量：它的 UTF-8 字节数。
    pub const SELECTED: i32 = 0;
    /// 文本：完整的上屏文本；标量：它的 UTF-8 字节数。
    pub const COMMIT: i32 = 1;
    /// 文本：上屏文本里可改写的那一段（不含标点、顶字的尾巴）；标量：它的 UTF-8 字节数。
    pub const COMMIT_BODY: i32 = 2;
    /// 只有标量：选中候选在呈现顺序里的座位。
    pub const SEAT: i32 = 3;
}

/// `candidate_meta(index, field) -> i64` 的字段，需要 `read_candidates`。
pub mod candidate_field {
    /// 候选来源，同 transition 里的 `source`。
    pub const SOURCE: i32 = 0;
    /// 候选文本的 UTF-8 字节数。
    pub const TEXT_LEN: i32 = 1;
    /// 编码的 UTF-8 字节数。
    pub const CODE_LEN: i32 = 2;
    /// 注释的 UTF-8 字节数。
    pub const ANNOTATION_LEN: i32 = 3;
    /// 当前页码，`index` 不看。
    pub const PAGE: i32 = 16;
    /// 高亮行在页内的下标，`index` 不看。
    pub const HIGHLIGHTED: i32 = 17;
}

/// `command_text(field, dst, cap)` 的字段，需要 `provide_commands`，只在 `command` 钩子里有值。
pub mod command_field {
    /// 用户打的触发词，即清单 `[[script.commands]]` 里的某个 `trigger`。
    pub const TRIGGER: i32 = 0;
    /// 触发词后面用户打的字母，可以为空（`/uuid`），只含小写 ASCII 字母，最多 [`crate::limits::MAX_COMMAND_ARGUMENT_BYTES`] 字节。
    pub const ARGUMENT: i32 = 1;
}

/// `log(level, src, len)` 的级别。产品里 `log` 是空操作，只有设了调试输出（`msime-pack run`）才打印。
pub mod log_level {
    pub const DEBUG: i32 = 0;
    pub const INFO: i32 = 1;
    pub const WARN: i32 = 2;
    pub const ERROR: i32 = 3;
}

/// `event_scalar` 里各枚举字段的取值：名字和数值。数值是 host-api、input-runtime 对应枚举的判别值（host-api 的测试逐项核对），名字是 `msime-pack run` 夹具和 SDK 里用的写法。和字段编号一样只能追加。
pub mod scalar_values {
    /// `ORIGIN`：host-api 的 `CommitOrigin`。
    pub const ORIGIN: &[(&str, u8)] = &[
        ("selection", 0),
        ("raw", 1),
        ("held_flush", 2),
        ("scheme_text", 3),
        ("focus_loss", 4),
    ];
    /// `HOW`：input-runtime 的 `SelectionHow`。
    pub const HOW: &[(&str, u8)] = &[
        ("finish", 0),
        ("punctuation_finish", 1),
        ("wubi_top", 2),
        ("space_over_list", 3),
        ("digit", 4),
        ("grid_raw", 5),
        ("select", 6),
        ("any_candidate", 7),
        ("edge", 8),
        ("highlighted", 9),
        ("wubi_auto", 10),
        ("grid_limit", 11),
    ];
    /// `OUTCOME`：input-runtime 的 `SelectionOutcome`。
    pub const OUTCOME: &[(&str, u8)] =
        &[("committed", 0), ("held", 1), ("pinned", 2), ("partial", 3)];
    /// `REASON`：host-api 的 `EndReason`，组字结束和候选隐藏共用。
    pub const REASON: &[(&str, u8)] = &[
        ("commit", 0),
        ("cancel", 1),
        ("deleted", 2),
        ("focus", 3),
        ("destroyed", 4),
        ("host_commit", 5),
        ("other", 6),
    ];
    /// `LOCAL_MODE`：host-api 的 `LocalModeClass`，名字同 `commit_context.local_mode`。0 到 5 在 U6 白名单里（[`super::local_mode_allows_text`]），其余模式（含拆字 13、符号 14）下文本一律按屏蔽处理。
    pub const LOCAL_MODE: &[(&str, u8)] = &[
        ("none", 0),
        ("temporary_english", 1),
        ("temporary_japanese", 2),
        ("super_jianpin", 3),
        ("emoji", 4),
        ("kaomoji", 5),
        ("mention", 6),
        ("url", 7),
        ("command", 8),
        ("expression", 9),
        ("unicode", 10),
        ("date_time", 11),
        ("quick_phrase", 12),
        ("chaizi", 13),
        ("symbol", 14),
        ("unknown", 255),
    ];
    /// `INPUT_CLASS`：host-api 的 `InputClass`。
    pub const INPUT_CLASS: &[(&str, u8)] = &[
        ("character", 0),
        ("punctuation", 1),
        ("selection", 2),
        ("navigation", 3),
        ("edit", 4),
        ("commit", 5),
        ("cancel", 6),
        ("host_commit", 7),
        ("other", 8),
        ("focus_loss", 9),
    ];
    /// `RETRACTION`：input-runtime 的 `SelectionRetraction`。
    pub const RETRACTION: &[(&str, u8)] = &[("restore", 0), ("drop", 1)];

    /// 按名字查数值。
    pub fn value(table: &[(&str, u8)], name: &str) -> Option<u8> {
        table
            .iter()
            .find(|(entry, _)| *entry == name)
            .map(|(_, value)| *value)
    }

    /// 按数值查名字。
    pub fn name(table: &[(&'static str, u8)], value: u8) -> Option<&'static str> {
        table
            .iter()
            .find(|(_, entry)| *entry == value)
            .map(|(name, _)| *name)
    }
}

/// 插件在清单 `permissions` 里申请、用户在授权页逐项授予的能力。判别值是位下标，只能追加。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Capability {
    /// 读预编辑串、读音和本次打的字符。
    ReadComposition = 0,
    /// 读选中的候选和上屏文本。
    ReadCommit = 1,
    /// 读候选窗口里的前 20 行（插件变换之前的顺序和注释）。
    ReadCandidates = 2,
    /// 改写上屏文本的可改写段。依赖 `read_commit`，不能和 `read_composition` 同时授予。
    RewriteCommit = 3,
    /// 重排候选。依赖 `read_candidates`。
    RerankCandidates = 4,
    /// 给候选加注释。依赖 `read_candidates`。
    AnnotateCandidates = 5,
    /// 把候选降到末尾并隐藏。依赖 `read_candidates`。
    FilterCandidates = 6,
    /// 在自己声明的 `/` 指令里给出候选：读用户在触发词后面打的字母，交回最多 [`crate::limits::MAX_COMMAND_ROWS`] 行。没有依赖。
    ProvideCommands = 7,
}

// `CapabilitySet` 是 u8，按判别值取位：第 9 项能力（判别值 8）的 `1 << 8` 在 debug 构建里 panic、在 release 构建里取模撞到第 1 项上。加第 9 项之前先把 `CapabilitySet` 扩宽，授权文件存的是能力名，扩宽不影响格式。
const _: () = assert!(Capability::ALL.len() <= u8::BITS as usize);

impl Capability {
    pub const ALL: [Self; 8] = [
        Self::ReadComposition,
        Self::ReadCommit,
        Self::ReadCandidates,
        Self::RewriteCommit,
        Self::RerankCandidates,
        Self::AnnotateCandidates,
        Self::FilterCandidates,
        Self::ProvideCommands,
    ];

    /// 清单和授权文件里的名字。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadComposition => "read_composition",
            Self::ReadCommit => "read_commit",
            Self::ReadCandidates => "read_candidates",
            Self::RewriteCommit => "rewrite_commit",
            Self::RerankCandidates => "rerank_candidates",
            Self::AnnotateCandidates => "annotate_candidates",
            Self::FilterCandidates => "filter_candidates",
            Self::ProvideCommands => "provide_commands",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|capability| capability.as_str() == name)
    }

    /// 授予这一项之前必须已经授予的能力。
    pub fn requires(self) -> Option<Self> {
        match self {
            Self::RewriteCommit => Some(Self::ReadCommit),
            Self::RerankCandidates | Self::AnnotateCandidates | Self::FilterCandidates => {
                Some(Self::ReadCandidates)
            }
            Self::ReadComposition
            | Self::ReadCommit
            | Self::ReadCandidates
            | Self::ProvideCommands => None,
        }
    }

    /// 能读到文本的能力。授予了其中任何一项，实例在每次 focus 和 destroy 时都整个丢弃。`provide_commands` 读的是用户在它的触发词后面打的字母，也算。
    pub fn reads_text(self) -> bool {
        matches!(
            self,
            Self::ReadComposition | Self::ReadCommit | Self::ReadCandidates | Self::ProvideCommands
        )
    }

    fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// 一组能力，按位存放。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CapabilitySet(u8);

impl CapabilitySet {
    pub const EMPTY: Self = Self(0);

    pub fn contains(self, capability: Capability) -> bool {
        self.0 & capability.bit() != 0
    }

    pub fn insert(&mut self, capability: Capability) {
        self.0 |= capability.bit();
    }

    pub fn with(mut self, capability: Capability) -> Self {
        self.insert(capability);
        self
    }

    /// 两边都有的能力：授权文件里多出来的、清单没申请过的项被丢掉。
    pub fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn iter(self) -> impl Iterator<Item = Capability> {
        Capability::ALL
            .into_iter()
            .filter(move |capability| self.contains(*capability))
    }

    /// 是否含有任何能读到文本的能力。
    pub fn reads_text(self) -> bool {
        self.iter().any(Capability::reads_text)
    }

    /// 依赖和互斥规则。清单校验和授权写入用同一条：缺依赖、`rewrite_commit` 与 `read_composition` 同时出现都拒绝。重复项由解析清单的一方拒绝，集合本身表达不了重复。
    pub fn check_rules(self) -> Result<(), CapabilityRuleError> {
        for capability in self.iter() {
            if let Some(required) = capability.requires() {
                if !self.contains(required) {
                    return Err(CapabilityRuleError::MissingDependency {
                        capability,
                        requires: required,
                    });
                }
            }
        }
        if self.contains(Capability::RewriteCommit) && self.contains(Capability::ReadComposition) {
            return Err(CapabilityRuleError::Exclusive {
                first: Capability::RewriteCommit,
                second: Capability::ReadComposition,
            });
        }
        Ok(())
    }
}

impl FromIterator<Capability> for CapabilitySet {
    fn from_iter<I: IntoIterator<Item = Capability>>(iter: I) -> Self {
        let mut set = Self::EMPTY;
        for capability in iter {
            set.insert(capability);
        }
        set
    }
}

/// 能力组合违反依赖或互斥规则。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityRuleError {
    MissingDependency {
        capability: Capability,
        requires: Capability,
    },
    Exclusive {
        first: Capability,
        second: Capability,
    },
}

impl std::fmt::Display for CapabilityRuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingDependency {
                capability,
                requires,
            } => write!(
                f,
                "permission `{}` requires `{}`",
                capability.as_str(),
                requires.as_str()
            ),
            Self::Exclusive { first, second } => write!(
                f,
                "permissions `{}` and `{}` cannot be granted together",
                first.as_str(),
                second.as_str()
            ),
        }
    }
}

impl std::error::Error for CapabilityRuleError {}

/// 投递给脚本的事件，也是清单 `hooks` 里的名字。
///
/// 编号沿用 host-api `LifecycleEvent` 的判别值：0 和 1 是 focus，6-15 是派发类事件；2-5（字段属性、激活、停用、按键）和 16（会话结束）只给核心，不投递给脚本，编号保留不用。`candidates` 不是生命周期事件，是 runtime 内部的候选变换，排在后面。`command` 也不是生命周期事件：用户在 `/` 指令里打了某个插件声明的触发词时，宿主只把它投递给这个触发词的主人，排在最后。只能追加。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Hook {
    FocusIn = 0,
    FocusOut = 1,
    Input = 6,
    SelectionRetracted = 7,
    CandidateSelected = 8,
    BeforeCommit = 9,
    Commit = 10,
    CompositionEnd = 11,
    CompositionStart = 12,
    CompositionUpdate = 13,
    CandidatesShown = 14,
    CandidatesHidden = 15,
    Candidates = 17,
    Command = 18,
}

impl Hook {
    pub const ALL: [Self; 14] = [
        Self::FocusIn,
        Self::FocusOut,
        Self::Input,
        Self::SelectionRetracted,
        Self::CandidateSelected,
        Self::BeforeCommit,
        Self::Commit,
        Self::CompositionEnd,
        Self::CompositionStart,
        Self::CompositionUpdate,
        Self::CandidatesShown,
        Self::CandidatesHidden,
        Self::Candidates,
        Self::Command,
    ];

    /// 清单 `hooks` 里的名字。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FocusIn => "focus_in",
            Self::FocusOut => "focus_out",
            Self::Input => "input",
            Self::SelectionRetracted => "selection_retracted",
            Self::CandidateSelected => "candidate_selected",
            Self::BeforeCommit => "before_commit",
            Self::Commit => "commit",
            Self::CompositionEnd => "composition_end",
            Self::CompositionStart => "composition_start",
            Self::CompositionUpdate => "composition_update",
            Self::CandidatesShown => "candidates_shown",
            Self::CandidatesHidden => "candidates_hidden",
            Self::Candidates => "candidates",
            Self::Command => "command",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|hook| hook.as_str() == name)
    }

    /// 传给 `msime_on_event` 的 `kind`，也是 `event_scalar(KIND)` 的值。
    pub fn code(self) -> i32 {
        self as i32
    }

    /// 可修改的钩子。修改类 import 只在这几个钩子里可用，别的钩子里返回 `DENIED`：`rewrite_commit` 只在 `before_commit`，三个候选类只在 `candidates`，`command_row` 只在 `command`。真正的门控在每个 import 自己的检查里（授权加上所在的钩子），这里只是把它们归成一类。
    pub fn modifies(self) -> bool {
        matches!(self, Self::BeforeCommit | Self::Candidates | Self::Command)
    }

    /// 跑在 `msime_init` 之后的快照上、看不到之前任何事件留下的状态的钩子：输出只能是这一次输入的函数。`before_commit` 改写的只能是这一次的上屏文本；`command` 给出的行只能取决于触发词、参数、随机数和模块本身，夹带不了同一个字段里早先的组字或上屏。
    pub fn runs_on_snapshot(self) -> bool {
        matches!(self, Self::BeforeCommit | Self::Command)
    }

    fn bit(self) -> u32 {
        1 << self as u8
    }
}

/// 清单声明的钩子集合。只有声明过的钩子才会投递。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HookSet(u32);

impl HookSet {
    pub const EMPTY: Self = Self(0);

    pub fn contains(self, hook: Hook) -> bool {
        self.0 & hook.bit() != 0
    }

    pub fn insert(&mut self, hook: Hook) {
        self.0 |= hook.bit();
    }

    pub fn with(mut self, hook: Hook) -> Self {
        self.insert(hook);
        self
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn iter(self) -> impl Iterator<Item = Hook> {
        Hook::ALL
            .into_iter()
            .filter(move |hook| self.contains(*hook))
    }
}

impl FromIterator<Hook> for HookSet {
    fn from_iter<I: IntoIterator<Item = Hook>>(iter: I) -> Self {
        let mut set = Self::EMPTY;
        for hook in iter {
            set.insert(hook);
        }
        set
    }
}

/// 文本类 import 和可修改的钩子只在这些本地模式里可用（U6）。编号同 host-api 的 `LocalModeClass`：none、临时英文、临时日文、超级简拼、emoji、颜文字；提及、网址、命令、表达式、Unicode、日期时间、快捷短语、拆字和不认识的模式一律屏蔽。
pub fn local_mode_allows_text(local_mode: u8) -> bool {
    local_mode <= 5
}

/// 修改类 import 的约定。它们只暂存动作，钩子正常返回（非负）之后宿主才采用；trap、fuel 耗尽或返回负数时，这个插件这一次暂存的动作全部丢弃（包括 `command_row` 暂存的行）。
///
/// - `rewrite_commit(src, len) -> i32`：只在 `before_commit` 里、这次上屏可改写（`event_scalar(REWRITABLE) == 1`）时可用，否则 `DENIED`。把 `[src, src + len)` 的 UTF-8 文本作为新的 body，tail（标点、顶字的尾巴）由宿主原样接回。区间越界直接 trap；超过 [`crate::limits::MAX_REWRITE_BYTES`] 字节或 [`crate::limits::rewrite_utf16_limit`] 个 UTF-16 码元返回 `LIMIT`；不是 UTF-8、为空、含 [`is_forbidden_plugin_char`] 拦下的字符（控制字符、格式字符、行和段分隔符、不显示的填充符和变体选择符）返回 `INVALID`。成功返回 0，再调一次覆盖上一次。多个插件依次改写，后面的插件读到的 body 是前面改写过的，长度上限仍按原文算。
/// - `candidates_order(src, count) -> i32`、`candidates_hide(src, count) -> i32`：只在 `candidates` 里可用，否则 `DENIED`。`[src, src + 4 × count)` 是 `count` 个小端 u32 行号，指 `read_candidates` 读到的那份窗口（插件变换之前的顺序）。行号越界、重复或 `count` 超过窗口行数返回 `INVALID`，区间越界 trap。`order` 把这些行按给出的顺序排到前面，其余行保持原来的相对顺序，再调一次覆盖上一次；`hide` 把这些行降到列表末尾并隐藏，多次调用取并集。锁定的座位（固定了位置的词、云和 AI 行、五笔四码唯一时的首位）和引擎下标 0 的行由宿主静默忽略，插件看不出来，返回值照样是 0。
/// - `candidates_annotate(index, src, len) -> i32`：只在 `candidates` 里可用。给窗口第 `index` 行加注释，不超过 [`crate::limits::MAX_ANNOTATION_UTF16`] 个 UTF-16 码元（超出返回 `LIMIT`），不含 [`is_forbidden_plugin_char`] 拦下的字符（否则 `INVALID`），空串表示不加；同一行再调一次覆盖。注释另放一格显示，不改引擎的注释，其他插件也读不到它。
///
/// - `command_row(text, tlen, note, nlen) -> i32`：只在 `command` 里、授予了 `provide_commands` 时可用，否则 `DENIED`。暂存一行候选：`[text, text + tlen)` 是行的文本，用户选中时原样上屏；`[note, note + nlen)` 是插件注释，画在插件注释格里，可以为空。两个区间越界直接 trap。已经暂存了 [`crate::limits::MAX_COMMAND_ROWS`] 行、文本超过 [`crate::limits::MAX_COMMAND_ROW_UTF16`] 个 UTF-16 码元或注释超过 [`crate::limits::MAX_ANNOTATION_UTF16`] 个返回 `LIMIT`；不是 UTF-8、文本为空、首尾是空白、含 [`is_forbidden_plugin_char`] 拦下的字符或者含 `://`、`www.`、`mailto:`（不让插件行成为链接），注释含拦下的字符，返回 `INVALID`。成功返回这一行的行号（从 0 起）。行按暂存的顺序排；同文的行由引擎去掉后一行。
/// - `random_fill(dst, len) -> i32`：不需要能力。每次 `command` 事件宿主给 [`crate::limits::RANDOM_BYTES_PER_EVENT`] 字节随机数，插件按调用顺序取：把接下来的 `len` 个字节写进 `[dst, dst + len)`，返回 `len`；剩下的不够 `len` 个返回 `LIMIT`，什么都不写。别的钩子里没有随机数，`len` 不为 0 时一律 `LIMIT`。区间越界直接 trap。产品里随机数取自操作系统的 CSPRNG，`msime-pack run` 取自夹具的 `random` 字段。
///
/// 多个插件的候选操作都针对同一份原始窗口、按候选身份（文本加来源）记下，再按授权顺序合成：重排后面的插件优先，隐藏取并集，注释后面的覆盖前面的。`command` 只投递给触发词的主人，它的行不和别的插件合成。
pub mod modification {}

/// `command_row` 的文本里不许出现的片段：插件行不能成为可点的链接或伪装成网址。按 ASCII 不分大小写比较。
pub const COMMAND_ROW_FORBIDDEN_FRAGMENTS: [&str; 3] = ["://", "www.", "mailto:"];

/// `command_row` 的文本是否合规（长度另查）：非空、首尾不是空白、不含 [`is_forbidden_plugin_char`] 拦下的字符和 [`COMMAND_ROW_FORBIDDEN_FRAGMENTS`]。
pub fn valid_command_row_text(text: &str) -> bool {
    !text.is_empty()
        && text.trim() == text
        && !has_forbidden_plugin_char(text)
        && !COMMAND_ROW_FORBIDDEN_FRAGMENTS
            .iter()
            .any(|fragment| contains_ignore_ascii_case(text, fragment))
}

fn contains_ignore_ascii_case(text: &str, fragment: &str) -> bool {
    text.as_bytes()
        .windows(fragment.len())
        .any(|window| window.eq_ignore_ascii_case(fragment.as_bytes()))
}

/// 插件交回的文本（改写后的上屏、候选注释）里不许出现的字符：控制字符（Cc）、格式字符（Cf），行分隔符 U+2028（Zl）和段分隔符 U+2029（Zp），以及一批不是 Cf、却同样看不见或只改字形的字符。Cf 包括双向控制符（U+202A..U+202E、U+2066..U+2069）、零宽字符（U+200B..U+200F、U+2060..U+2064、U+FEFF）和标签字符，它们能让显示的文本和实际写进文档的不一样。U+2028 和 U+2029 不是 Cc，却和换行一样断行：macOS 的宿主把候选释义交给会话时就拿 U+2028 当换行（`InputController.mm` 的 `MSIMEGlossLineSeparator`），不拦它们，拦 `\n` 就没有意义。Cf 的列表按 Unicode 15.1 写死，标准库没有通用类别的查询；以后的版本新增的 Cf 字符不在表里。
///
/// 不是 Cf 但同样拦下的：组合用字形连接符 U+034F（Mn，不显示）；韩文填充符 U+115F、U+1160、U+3164、U+FFA0（Lo，显示成空白）；高棉文固有元音 U+17B4、U+17B5（Mn，不显示）；蒙古文自由变体选择符 U+180B..U+180D、U+180F（Mn）；盲文空白 U+2800（So，显示成空白）；变体选择符 U+FE00..U+FE0D 和表意文字变体选择符 U+E0100..U+E01EF（Mn，只改前一个字的字形）。它们能让一段看起来相同的文本在字节上不同，或者让一行看起来是空的。U+FE0E、U+FE0F（VS15、VS16）放行：这里是逐字符判断，分不出它是不是跟在 emoji 后面，而 emoji 注释离不开 VS16；单独出现的 VS15、VS16 不改变字义。表意文字变体选择符拦下之后，改写类插件不能再输出带异体选择符的汉字。
///
/// 这条规则是插件专用的，不改 client-base 的 `text` 模块：那里的检查用在别的输入边界上。
pub fn is_forbidden_plugin_char(c: char) -> bool {
    c.is_control()
        || matches!(
            u32::from(c),
            0x00AD
                | 0x034F
                | 0x0600..=0x0605
                | 0x061C
                | 0x06DD
                | 0x070F
                | 0x0890..=0x0891
                | 0x08E2
                | 0x115F..=0x1160
                | 0x17B4..=0x17B5
                | 0x180B..=0x180F
                | 0x200B..=0x200F
                | 0x2028..=0x2029
                | 0x202A..=0x202E
                | 0x2060..=0x2064
                | 0x2066..=0x206F
                | 0x2800
                | 0x3164
                | 0xFE00..=0xFE0D
                | 0xFEFF
                | 0xFFA0
                | 0xFFF9..=0xFFFB
                | 0x110BD
                | 0x110CD
                | 0x13430..=0x1343F
                | 0x1BCA0..=0x1BCA3
                | 0x1D173..=0x1D17A
                | 0xE0001
                | 0xE0020..=0xE007F
                | 0xE0100..=0xE01EF
        )
}

/// `text` 里有没有 [`is_forbidden_plugin_char`] 的字符。
pub fn has_forbidden_plugin_char(text: &str) -> bool {
    text.chars().any(is_forbidden_plugin_char)
}

/// import 的值类型，只用到这两种。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ty {
    I32,
    I64,
}

/// 白名单里的一个 import：名字、所属能力和签名。一个 import 只属于一个能力，静态最小权限的双向检查依赖这一点。
#[derive(Clone, Copy, Debug)]
pub(crate) struct ImportSpec {
    pub name: &'static str,
    /// `None` 表示不需要能力（`event_scalar`、`log`、`random_fill`）。
    pub capability: Option<Capability>,
    pub params: &'static [Ty],
    pub results: &'static [Ty],
}

const I32_TO_I64: (&[Ty], &[Ty]) = (&[Ty::I32], &[Ty::I64]);
const TEXT_COPY: (&[Ty], &[Ty]) = (&[Ty::I32, Ty::I32, Ty::I32], &[Ty::I32]);

const fn spec(
    name: &'static str,
    capability: Option<Capability>,
    signature: (&'static [Ty], &'static [Ty]),
) -> ImportSpec {
    ImportSpec {
        name,
        capability,
        params: signature.0,
        results: signature.1,
    }
}

/// `msime` 命名空间的全部 import。表里有的才能导入。修改类 import 的约定见 [`modification`]。
pub(crate) const IMPORTS: [ImportSpec; 18] = [
    spec("event_scalar", None, I32_TO_I64),
    spec("log", None, (&[Ty::I32, Ty::I32, Ty::I32], &[])),
    spec(
        "composition_text",
        Some(Capability::ReadComposition),
        TEXT_COPY,
    ),
    spec(
        "composition_scalar",
        Some(Capability::ReadComposition),
        I32_TO_I64,
    ),
    spec("commit_text", Some(Capability::ReadCommit), TEXT_COPY),
    spec("commit_scalar", Some(Capability::ReadCommit), I32_TO_I64),
    spec(
        "candidate_count",
        Some(Capability::ReadCandidates),
        (&[], &[Ty::I32]),
    ),
    spec(
        "candidate_text",
        Some(Capability::ReadCandidates),
        TEXT_COPY,
    ),
    spec(
        "candidate_code",
        Some(Capability::ReadCandidates),
        TEXT_COPY,
    ),
    spec(
        "candidate_annotation",
        Some(Capability::ReadCandidates),
        TEXT_COPY,
    ),
    spec(
        "candidate_meta",
        Some(Capability::ReadCandidates),
        (&[Ty::I32, Ty::I32], &[Ty::I64]),
    ),
    spec(
        "rewrite_commit",
        Some(Capability::RewriteCommit),
        (&[Ty::I32, Ty::I32], &[Ty::I32]),
    ),
    spec(
        "candidates_order",
        Some(Capability::RerankCandidates),
        (&[Ty::I32, Ty::I32], &[Ty::I32]),
    ),
    spec(
        "candidates_hide",
        Some(Capability::FilterCandidates),
        (&[Ty::I32, Ty::I32], &[Ty::I32]),
    ),
    spec(
        "candidates_annotate",
        Some(Capability::AnnotateCandidates),
        (&[Ty::I32, Ty::I32, Ty::I32], &[Ty::I32]),
    ),
    spec("command_text", Some(Capability::ProvideCommands), TEXT_COPY),
    spec(
        "command_row",
        Some(Capability::ProvideCommands),
        (&[Ty::I32, Ty::I32, Ty::I32, Ty::I32], &[Ty::I32]),
    ),
    spec("random_fill", None, (&[Ty::I32, Ty::I32], &[Ty::I32])),
];

/// 白名单里全部 import 的名字和所属能力（`None` 表示不需要能力），按 ABI 的顺序。作者文档和 SDK 的一致性测试按它核对。
pub fn imports() -> impl Iterator<Item = (&'static str, Option<Capability>)> {
    IMPORTS.iter().map(|spec| (spec.name, spec.capability))
}

/// 按名字找白名单里的 import。
pub(crate) fn import_spec(name: &str) -> Option<&'static ImportSpec> {
    IMPORTS.iter().find(|spec| spec.name == name)
}
