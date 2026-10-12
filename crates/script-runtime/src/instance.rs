//! 每个会话、每个插件一个实例：实例化、按轮和按钩子的 fuel 配额、`before_commit` 的快照复位，以及观察类和修改类 import。
//!
//! import 只读写本次调用的上下文，从不回调宿主：宿主的会话正借给这次调用，回调只会撞上 `"reentrant host call"`。上下文在调用前按授权复制、调用后清空；没有授权的文本根本不复制进来，屏蔽和缺失对插件来说同样是 `ABSENT`。

use crate::abi::{
    self, candidate_field, commit_field, composition_field, event_field, status, Capability,
    CapabilitySet, Hook,
};
use crate::limits;
use crate::module::ScriptModule;
use wasmi::{
    Caller, Engine, Error, Instance, Linker, Memory, Store, StoreLimits, StoreLimitsBuilder,
    TrapCode, TypedFunc, Val,
};

/// 事件里的枚举字段，经 `event_scalar` 读出，不需要能力。值是 host-api 各枚举的判别值；`None` 表示这个事件没有该字段（`ABSENT`）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scalars {
    pub origin: Option<u8>,
    pub how: Option<u8>,
    pub outcome: Option<u8>,
    pub reason: Option<u8>,
    /// 本地模式类别（`LocalModeClass`）。文本类 import 只在白名单里的模式可用，带文本的事件必须填它，不填时文本一律按屏蔽处理。
    pub local_mode: Option<u8>,
    pub input_class: Option<u8>,
    pub retraction: Option<u8>,
    pub rewritable: Option<bool>,
    pub continues: Option<bool>,
    pub generated: Option<bool>,
}

/// `read_composition` 的字段。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Composition<'a> {
    pub preedit: Option<&'a str>,
    pub reading: Option<&'a str>,
    pub input_char: Option<&'a str>,
    pub caret: Option<u32>,
}

/// `read_commit` 的字段。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommitTexts<'a> {
    pub selected: Option<&'a str>,
    pub commit: Option<&'a str>,
    pub body: Option<&'a str>,
    pub seat: Option<u32>,
    /// `before_commit` 里原文 body 的 UTF-16 码元数，`rewrite_commit` 的长度上限按它算，插件读不到。多个插件依次改写时，后面的插件拿到的 `body` 是前面改写过的，宿主要在这里填原文的长度；`None` 时按 `body` 算。
    pub original_body_utf16: Option<usize>,
}

/// 候选窗口里的一行，插件变换之前的样子。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateRow<'a> {
    pub text: &'a str,
    pub code: &'a str,
    pub annotation: &'a str,
    pub source: u8,
}

/// `read_candidates` 的字段。超过 [`limits::MAX_CANDIDATE_ROWS`] 的行不交给插件。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Candidates<'a> {
    pub rows: &'a [CandidateRow<'a>],
    pub page: Option<u32>,
    pub highlighted: Option<u32>,
}

/// 投递给一个插件的一次事件。宿主按事件填它拥有的字段；每个插件按自己的授权各取一份。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event<'a> {
    pub hook: Hook,
    pub scalars: Scalars,
    pub composition: Composition<'a>,
    pub commit: CommitTexts<'a>,
    pub candidates: Option<Candidates<'a>>,
}

impl<'a> Event<'a> {
    /// 只有事件类型、其余字段都缺失的事件。
    pub fn new(hook: Hook) -> Self {
        Self {
            hook,
            scalars: Scalars::default(),
            composition: Composition::default(),
            commit: CommitTexts::default(),
            candidates: None,
        }
    }
}

/// 一次失败的原因。封闭枚举，`as_str` 可以直接写进诊断（`script plugin disabled`），不带任何文本。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Failure {
    /// 超出本钩子或本轮的 fuel 配额。
    OutOfFuel,
    /// 超出值栈高度或递归深度。
    StackOverflow,
    /// 线性内存或表要长到预留之外。
    MemoryLimit,
    /// 越界访问线性内存，包括 import 拿到的越界指针。
    MemoryOutOfBounds,
    /// 执行了 `unreachable`（Rust 的 panic 就落在这里）。
    Unreachable,
    /// 其他 trap：除零、错误的间接调用等。
    Trap,
    /// `msime_on_event` 返回了负数。
    NegativeReturn,
    /// `msime_abi()` 返回的不是 [`abi::ABI_VERSION`]。
    AbiMismatch,
    /// `msime_init()` 返回了负数。
    InitFailed,
    /// 实例化失败（链接或分配）。
    Instantiate,
    /// 实例在之前的 trap 之后已不可用，宿主应当丢弃它、需要时重新实例化。
    Unusable,
}

impl Failure {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OutOfFuel => "out_of_fuel",
            Self::StackOverflow => "stack_overflow",
            Self::MemoryLimit => "memory_limit",
            Self::MemoryOutOfBounds => "memory_out_of_bounds",
            Self::Unreachable => "unreachable",
            Self::Trap => "trap",
            Self::NegativeReturn => "negative_return",
            Self::AbiMismatch => "abi_mismatch",
            Self::InitFailed => "init_failed",
            Self::Instantiate => "instantiate",
            Self::Unusable => "unusable",
        }
    }

    fn of(error: &Error) -> Self {
        match error.as_trap_code() {
            Some(TrapCode::OutOfFuel) => Self::OutOfFuel,
            Some(TrapCode::StackOverflow) => Self::StackOverflow,
            Some(TrapCode::GrowthOperationLimited | TrapCode::OutOfSystemMemory) => {
                Self::MemoryLimit
            }
            Some(TrapCode::MemoryOutOfBounds) => Self::MemoryOutOfBounds,
            Some(TrapCode::UnreachableCodeReached) => Self::Unreachable,
            Some(_) | None => Self::Trap,
        }
    }
}

/// 一次投递的结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// 清单没声明这个钩子，没有运行，也不花 fuel。
    NotSubscribed,
    /// `msime_on_event` 正常返回了非负数 `status`。
    Delivered { status: i32, fuel: u64 },
    /// 计为插件自己的一次失败。它这次暂存的动作全部丢弃，不影响后面的插件。
    Failed { failure: Failure, fuel: u64 },
    /// 本轮的配额用完了，不算插件的失败：宿主并不在每次派发之前都开一轮（修饰键组合、快捷键和失焦命令不经过 `msime_client_key`），这样的派发和上一次按键共用一轮，把它花光的可能是之前那几次正常的投递。`fuel` 为 0 时这次没有运行；不为 0 时运行到一半被本轮剩下的配额截断，而且这一次用掉的不到本钩子上限（[`limits::hook_fuel`]）的一半，像 trap 一样暂存的动作全部丢弃，持久实例随之不可用（[`ScriptInstance::is_usable`]；`before_commit` 跑在用完即弃的快照实例上，持久实例不受影响）。被本轮配额截断时已经用掉一半或更多，和超出单个钩子自己的上限一样，是 fuel 耗尽的 [`Outcome::Failed`]。
    TurnExhausted { fuel: u64 },
}

/// 实例化或初始化失败。初始化失败（含 ABI 不符）时宿主应立即停用这个插件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InitFailure {
    pub failure: Failure,
    /// 失败前已经用掉的 fuel，计入 focus 的初始化合计。
    pub fuel: u64,
}

/// 调试输出：`log(level, src, len)` 的级别和文本（非法 UTF-8 按替换字符解码）。产品里不设，`log` 就是空操作。
pub type LogSink = Box<dyn FnMut(i32, &str) + Send>;

/// 修改类钩子这一次暂存的动作，已经按 ABI 的规则校验过（见 [`abi::modification`]）。只有钩子正常返回时才交给宿主，否则为空。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Actions {
    /// `rewrite_commit`：新的 body。
    pub rewrite: Option<String>,
    /// `candidates_order`：窗口行号，按给出的顺序。
    pub order: Vec<u32>,
    /// `candidates_hide`：窗口行号，不重复。
    pub hidden: Vec<u32>,
    /// `candidates_annotate`：窗口行号和注释，每行最多一条。
    pub annotations: Vec<(u32, String)>,
}

impl Actions {
    fn clear(&mut self) {
        self.rewrite = None;
        self.order.clear();
        self.hidden.clear();
        self.annotations.clear();
    }

    /// 什么动作都没有。
    pub fn is_empty(&self) -> bool {
        self.rewrite.is_none()
            && self.order.is_empty()
            && self.hidden.is_empty()
            && self.annotations.is_empty()
    }
}

/// 调试输出：修改类 import（`rewrite_commit`、`candidates_*`）的一次调用和宿主交还给插件的返回码。产品里不设；`msime-pack run` 用它把每次改写、重排、隐藏和注释的结果列进输出。只是旁观：返回码由 import 本身决定，设不设它都一样。
pub type ActionSink = Box<dyn FnMut(&'static str, i32) + Send>;

/// 实例化之前就接上的调试输出，`msime_init` 里的 `log` 也能看到。只有作者工具设它。
#[derive(Default)]
pub struct DebugSinks {
    pub log: Option<LogSink>,
    pub actions: Option<ActionSink>,
}

#[derive(Default)]
struct OwnedText {
    value: String,
    present: bool,
}

impl OwnedText {
    fn load(&mut self, source: Option<&str>) {
        self.value.clear();
        self.present = false;
        if let Some(source) = source {
            self.value.push_str(source);
            self.present = true;
        }
    }

    fn clear(&mut self) {
        self.value.clear();
        self.present = false;
    }

    fn get(&self) -> Option<&str> {
        self.present.then_some(self.value.as_str())
    }
}

#[derive(Default)]
struct OwnedRow {
    text: String,
    code: String,
    annotation: String,
    source: u8,
}

/// 本次调用的上下文。字符串在调用之间保留容量，按键路径上不重新分配。
#[derive(Default)]
struct CallContext {
    /// 不在事件里（`msime_abi`、`msime_init`）时为 `None`，所有字段都是 `ABSENT`。
    hook: Option<Hook>,
    scalars: Scalars,
    preedit: OwnedText,
    reading: OwnedText,
    input_char: OwnedText,
    caret: Option<u32>,
    selected: OwnedText,
    commit: OwnedText,
    body: OwnedText,
    seat: Option<u32>,
    /// 见 [`CommitTexts::original_body_utf16`]。
    original_body_utf16: Option<usize>,
    candidates_present: bool,
    rows: Vec<OwnedRow>,
    row_count: usize,
    page: Option<u32>,
    highlighted: Option<u32>,
    /// 修改类 import 暂存的动作，钩子正常返回后才交出去。
    actions: Actions,
}

impl CallContext {
    /// 按授权和本地模式复制这一次能看的字段；不能看的根本不复制。
    fn load(&mut self, grants: CapabilitySet, event: &Event<'_>) {
        self.hook = Some(event.hook);
        self.scalars = event.scalars;
        self.actions.clear();
        let text_allowed = event
            .scalars
            .local_mode
            .is_some_and(abi::local_mode_allows_text);
        let allowed = |capability| text_allowed && grants.contains(capability);

        let composition = if allowed(Capability::ReadComposition) {
            event.composition
        } else {
            Composition::default()
        };
        self.preedit.load(composition.preedit);
        self.reading.load(composition.reading);
        self.input_char.load(composition.input_char);
        self.caret = composition.caret;

        let commit = if allowed(Capability::ReadCommit) {
            event.commit
        } else {
            CommitTexts::default()
        };
        self.selected.load(commit.selected);
        self.commit.load(commit.commit);
        self.body.load(commit.body);
        self.seat = commit.seat;
        self.original_body_utf16 = commit.original_body_utf16;

        self.candidates_present = false;
        self.row_count = 0;
        self.page = None;
        self.highlighted = None;
        if let Some(candidates) = event
            .candidates
            .filter(|_| allowed(Capability::ReadCandidates))
        {
            let rows = &candidates.rows[..candidates.rows.len().min(limits::MAX_CANDIDATE_ROWS)];
            if self.rows.len() < rows.len() {
                self.rows.resize_with(rows.len(), OwnedRow::default);
            }
            for (owned, row) in self.rows.iter_mut().zip(rows) {
                owned.text.clear();
                owned.text.push_str(row.text);
                owned.code.clear();
                owned.code.push_str(row.code);
                owned.annotation.clear();
                owned.annotation.push_str(row.annotation);
                owned.source = row.source;
            }
            self.candidates_present = true;
            self.row_count = rows.len();
            self.page = candidates.page;
            self.highlighted = candidates.highlighted;
        }
    }

    /// 调用结束后清空，文本不在两次调用之间留在插件的上下文里。
    fn clear(&mut self) {
        self.hook = None;
        self.scalars = Scalars::default();
        self.preedit.clear();
        self.reading.clear();
        self.input_char.clear();
        self.caret = None;
        self.selected.clear();
        self.commit.clear();
        self.body.clear();
        self.seat = None;
        self.original_body_utf16 = None;
        for row in &mut self.rows[..self.row_count] {
            row.text.clear();
            row.code.clear();
            row.annotation.clear();
        }
        self.candidates_present = false;
        self.row_count = 0;
        self.page = None;
        self.highlighted = None;
        self.actions.clear();
    }

    fn event_scalar(&self, field: i32) -> i64 {
        let Some(hook) = self.hook else {
            return i64::from(status::ABSENT);
        };
        let scalars = &self.scalars;
        let value = match field {
            event_field::KIND => Some(i64::from(hook.code())),
            event_field::ORIGIN => scalars.origin.map(i64::from),
            event_field::HOW => scalars.how.map(i64::from),
            event_field::OUTCOME => scalars.outcome.map(i64::from),
            event_field::REASON => scalars.reason.map(i64::from),
            event_field::LOCAL_MODE => scalars.local_mode.map(i64::from),
            event_field::INPUT_CLASS => scalars.input_class.map(i64::from),
            event_field::RETRACTION => scalars.retraction.map(i64::from),
            event_field::REWRITABLE => scalars.rewritable.map(i64::from),
            event_field::CONTINUES => scalars.continues.map(i64::from),
            event_field::GENERATED => scalars.generated.map(i64::from),
            _ => return i64::from(status::INVALID),
        };
        value.unwrap_or(i64::from(status::ABSENT))
    }

    fn composition_text(&self, field: i32) -> Result<&str, i32> {
        let text = match field {
            composition_field::PREEDIT => &self.preedit,
            composition_field::READING => &self.reading,
            composition_field::INPUT_CHAR => &self.input_char,
            _ => return Err(status::INVALID),
        };
        text.get().ok_or(status::ABSENT)
    }

    fn composition_scalar(&self, field: i32) -> i64 {
        let value = match field {
            composition_field::CARET => self.caret.map(i64::from),
            _ => match self.composition_text(field) {
                Ok(text) => Some(text.len() as i64),
                Err(code) => return i64::from(code),
            },
        };
        value.unwrap_or(i64::from(status::ABSENT))
    }

    fn commit_text(&self, field: i32) -> Result<&str, i32> {
        let text = match field {
            commit_field::SELECTED => &self.selected,
            commit_field::COMMIT => &self.commit,
            commit_field::COMMIT_BODY => &self.body,
            _ => return Err(status::INVALID),
        };
        text.get().ok_or(status::ABSENT)
    }

    fn commit_scalar(&self, field: i32) -> i64 {
        let value = match field {
            commit_field::SEAT => self.seat.map(i64::from),
            _ => match self.commit_text(field) {
                Ok(text) => Some(text.len() as i64),
                Err(code) => return i64::from(code),
            },
        };
        value.unwrap_or(i64::from(status::ABSENT))
    }

    fn row(&self, index: i32) -> Result<&OwnedRow, i32> {
        if !self.candidates_present {
            return Err(status::ABSENT);
        }
        usize::try_from(index)
            .ok()
            .filter(|index| *index < self.row_count)
            .map(|index| &self.rows[index])
            .ok_or(status::ABSENT)
    }

    fn candidate_meta(&self, index: i32, field: i32) -> i64 {
        let page_level = match field {
            candidate_field::PAGE => Some(self.page),
            candidate_field::HIGHLIGHTED => Some(self.highlighted),
            _ => None,
        };
        if let Some(value) = page_level {
            return match value.filter(|_| self.candidates_present) {
                Some(value) => i64::from(value),
                None => i64::from(status::ABSENT),
            };
        }
        if !matches!(
            field,
            candidate_field::SOURCE
                | candidate_field::TEXT_LEN
                | candidate_field::CODE_LEN
                | candidate_field::ANNOTATION_LEN
        ) {
            return i64::from(status::INVALID);
        }
        match self.row(index) {
            Ok(row) => match field {
                candidate_field::SOURCE => i64::from(row.source),
                candidate_field::TEXT_LEN => row.text.len() as i64,
                candidate_field::CODE_LEN => row.code.len() as i64,
                _ => row.annotation.len() as i64,
            },
            Err(code) => i64::from(code),
        }
    }
}

/// 一个 Store 的宿主数据。
pub(crate) struct HostState {
    limits: StoreLimits,
    grants: CapabilitySet,
    /// 实例化之后才有。
    memory: Option<Memory>,
    context: CallContext,
    log_sink: Option<LogSink>,
    action_sink: Option<ActionSink>,
}

fn new_store(module: &ScriptModule, grants: CapabilitySet) -> Store<HostState> {
    let limits = StoreLimitsBuilder::new()
        .memory_size(module.reserved_memory_bytes())
        .memories(1)
        .instances(1)
        .tables(limits::MAX_TABLES)
        .table_elements(limits::MAX_TABLE_ELEMENTS)
        .trap_on_grow_failure(true)
        .build();
    let mut store = Store::new(
        module.module.engine(),
        HostState {
            limits,
            grants,
            memory: None,
            context: CallContext::default(),
            log_sink: None,
            action_sink: None,
        },
    );
    store.limiter(|state| &mut state.limits);
    store
}

/// 实例化、找到内存并一次预留到 `reserved_pages`。之后线性内存不会再长：`StoreLimits` 卡死在这个值，`memory.grow` 越过它就 trap。
fn instantiate(
    store: &mut Store<HostState>,
    module: &ScriptModule,
) -> Result<(Instance, Memory), Failure> {
    let instance = module
        .linker
        .instantiate_and_start(&mut *store, &module.module)
        .map_err(|error| match error.kind() {
            wasmi::errors::ErrorKind::Memory(_) => Failure::MemoryLimit,
            _ => Failure::Instantiate,
        })?;
    let memory = instance
        .get_memory(&*store, abi::EXPORT_MEMORY)
        .ok_or(Failure::Instantiate)?;
    let current = memory.size(&*store);
    let reserved = u64::from(module.reserved_pages);
    if current < reserved {
        memory
            .grow(&mut *store, reserved - current)
            .map_err(|_| Failure::MemoryLimit)?;
    }
    store.data_mut().memory = Some(memory);
    Ok((instance, memory))
}

/// `msime_init` 之后的状态：线性内存里最后一个非零字节之前的部分，加上导出的数值型可变 global。
///
/// 复位时重新实例化：表和未导出的 global 回到模块声明的初值，数据段重新写入，再把这份内存和导出 global 盖上去。只靠原地复写内存做不到这一点：wasmi 不给宿主访问未导出的 global 和表，而它们同样能把上一次事件的内容带进下一次 `before_commit`。代价是作者要知道 `msime_init` 改过的未导出 global 不会保留（Rust 编译出的 `__stack_pointer` 在函数返回时已经复原，不受影响），这一条写进作者文档。
struct Snapshot {
    memory: Box<[u8]>,
    /// 新实例刚写完数据段时最后一个非零字节之后的位置。复位时快照之外、这个位置之前的部分要清零，才和快照逐字节相同。
    data_extent: usize,
    globals: Vec<(Box<str>, Val)>,
}

fn nonzero_extent(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .rposition(|byte| *byte != 0)
        .map_or(0, |index| index + 1)
}

impl Snapshot {
    fn take(
        store: &Store<HostState>,
        instance: Instance,
        memory: Memory,
        data_extent: usize,
    ) -> Self {
        let data = memory.data(store);
        let memory = data[..nonzero_extent(data)].into();
        let globals = instance
            .exports(store)
            .filter_map(|export| {
                let name: Box<str> = export.name().into();
                let global = export.into_global()?;
                Some((name, global))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .filter(|(_, global)| global.ty(store).mutability().is_mut())
            .filter_map(|(name, global)| match global.get(store) {
                value @ (Val::I32(_) | Val::I64(_) | Val::F32(_) | Val::F64(_)) => {
                    Some((name, value))
                }
                _ => None,
            })
            .collect();
        Self {
            memory,
            data_extent,
            globals,
        }
    }

    fn restore(
        &self,
        store: &mut Store<HostState>,
        instance: Instance,
        memory: Memory,
    ) -> Result<(), Failure> {
        let data = memory.data_mut(&mut *store);
        let restored = self.memory.len();
        data[..restored].copy_from_slice(&self.memory);
        if self.data_extent > restored {
            data[restored..self.data_extent].fill(0);
        }
        for (name, value) in &self.globals {
            instance
                .get_global(&*store, name)
                .ok_or(Failure::Instantiate)?
                .set(&mut *store, value.clone())
                .map_err(|_| Failure::Instantiate)?;
        }
        Ok(())
    }
}

/// 一个会话里一个插件的实例。只在创建它的线程上使用。
pub struct ScriptInstance {
    module: ScriptModule,
    store: Store<HostState>,
    on_event: TypedFunc<i32, i32>,
    usable: bool,
    turn: Option<u64>,
    turn_fuel_left: u64,
    /// `before_commit` 本轮还剩的 fuel，和其余钩子的轮配额分开记，见 [`Self::deliver`]。
    before_commit_fuel_left: u64,
    init_fuel: u64,
    snapshot: Option<Snapshot>,
    /// 最近一次投递暂存的动作。
    actions: Actions,
}

impl ScriptInstance {
    /// 实例化并初始化：预留线性内存、检查 `msime_abi()`、调用 `msime_init()`（有的话），声明了 `before_commit` 时取快照。`grants` 和清单申请的能力取交集。`init_fuel` 不超过 [`limits::INIT_FUEL`]，宿主按 focus 的初始化合计传入剩余额度。
    pub fn new(
        module: &ScriptModule,
        grants: CapabilitySet,
        init_fuel: u64,
    ) -> Result<Self, InitFailure> {
        Self::with_debug(module, grants, init_fuel, DebugSinks::default())
    }

    /// 同 [`ScriptInstance::new`]，调试输出在实例化之前接上。只有作者工具用它；执行路径和产品里的完全一样，调试输出只旁观。
    pub fn with_debug(
        module: &ScriptModule,
        grants: CapabilitySet,
        init_fuel: u64,
        debug: DebugSinks,
    ) -> Result<Self, InitFailure> {
        let grants = grants.intersection(module.declaration.capabilities);
        let budget = init_fuel.min(limits::INIT_FUEL);
        let mut store = new_store(module, grants);
        store.data_mut().log_sink = debug.log;
        store.data_mut().action_sink = debug.actions;
        let fail = |store: &Store<HostState>, failure| InitFailure {
            failure,
            fuel: budget - store.get_fuel().unwrap_or(0),
        };
        if store.set_fuel(budget).is_err() {
            return Err(fail(&store, Failure::Instantiate));
        }
        let (instance, memory) = match instantiate(&mut store, module) {
            Ok(pair) => pair,
            Err(failure) => return Err(fail(&store, failure)),
        };
        let data_extent = nonzero_extent(memory.data(&store));

        let typed = |store: &Store<HostState>, name| {
            instance
                .get_typed_func::<(), i32>(store, name)
                .map_err(|_| Failure::Instantiate)
        };
        let abi_version = typed(&store, abi::EXPORT_ABI).and_then(|func| {
            func.call(&mut store, ())
                .map_err(|error| Failure::of(&error))
        });
        match abi_version {
            Ok(abi::ABI_VERSION) => {}
            Ok(_) => return Err(fail(&store, Failure::AbiMismatch)),
            Err(failure) => return Err(fail(&store, failure)),
        }
        if module.has_init {
            let status = typed(&store, abi::EXPORT_INIT).and_then(|func| {
                func.call(&mut store, ())
                    .map_err(|error| Failure::of(&error))
            });
            match status {
                Ok(status) if status >= 0 => {}
                Ok(_) => return Err(fail(&store, Failure::InitFailed)),
                Err(failure) => return Err(fail(&store, failure)),
            }
        }
        let on_event = match instance.get_typed_func::<i32, i32>(&store, abi::EXPORT_ON_EVENT) {
            Ok(func) => func,
            Err(_) => return Err(fail(&store, Failure::Instantiate)),
        };
        let init_fuel = budget - store.get_fuel().unwrap_or(0);
        let snapshot = module
            .declaration
            .hooks
            .contains(Hook::BeforeCommit)
            .then(|| Snapshot::take(&store, instance, memory, data_extent));
        Ok(Self {
            module: module.clone(),
            store,
            on_event,
            usable: true,
            turn: None,
            turn_fuel_left: limits::TURN_FUEL,
            before_commit_fuel_left: limits::BEFORE_COMMIT_FUEL,
            init_fuel,
            snapshot,
            actions: Actions::default(),
        })
    }

    pub fn module(&self) -> &ScriptModule {
        &self.module
    }

    /// 实际生效的能力：授权和清单申请的交集。
    pub fn grants(&self) -> CapabilitySet {
        self.store.data().grants
    }

    /// 初始化用掉的 fuel，计入 focus 的初始化合计。
    pub fn init_fuel(&self) -> u64 {
        self.init_fuel
    }

    /// 本轮还剩多少 fuel（`before_commit` 之外的钩子）。轮次还没开始时是完整的一轮。
    pub fn turn_fuel_left(&self) -> u64 {
        self.turn_fuel_left
    }

    /// `before_commit` 本轮还剩多少 fuel。轮次还没开始时是完整的一份。
    pub fn before_commit_fuel_left(&self) -> u64 {
        self.before_commit_fuel_left
    }

    /// 接着插件这一轮已经用掉的配额。宿主在同一轮里重建实例时（上一个实例因为换字段或属性变化被丢弃）把这一轮两份配额各还剩多少交给新实例，轮配额按插件算，不按实例算。
    pub fn resume_turn(&mut self, turn: u64, fuel_left: u64, before_commit_fuel_left: u64) {
        self.turn = Some(turn);
        self.turn_fuel_left = fuel_left.min(limits::TURN_FUEL);
        self.before_commit_fuel_left = before_commit_fuel_left.min(limits::BEFORE_COMMIT_FUEL);
    }

    /// 持久实例上出过 trap 之后返回 false：wasm 的状态（例如影子栈指针）可能停在一半，宿主应当丢弃这个实例。
    pub fn is_usable(&self) -> bool {
        self.usable
    }

    /// 最近一次投递暂存的动作：只有那次是修改类钩子、并且正常返回（[`Outcome::Delivered`]）时才可能非空。
    pub fn actions(&self) -> &Actions {
        &self.actions
    }

    /// 设置或清除调试输出。只有作者工具设它。
    pub fn set_log_sink(&mut self, sink: Option<LogSink>) {
        self.store.data_mut().log_sink = sink;
    }

    /// 投递一个事件。`turn` 是宿主的轮次计数（每个非 PASSTHROUGH 的 `msime_client_key` 加一），换了轮次就重新给满本轮配额；本次能用的 fuel 是本轮剩余和本钩子上限中较小的那个。本轮已经用完时不运行，是 [`Outcome::TurnExhausted`]；被本轮剩下的配额截断时，这一次用掉的不到本钩子上限的一半也是 [`Outcome::TurnExhausted`]，达到一半就和超出本钩子上限一样是 fuel 耗尽的 [`Outcome::Failed`]。`before_commit` 在 `msime_init` 之后的快照上运行，看不到之前任何事件留下的状态。
    ///
    /// `before_commit` 用自己的一份按轮配额（[`limits::BEFORE_COMMIT_FUEL`]），不从其余钩子的轮配额里扣，也不让它们扣：要是共用一份，持久实例在同一轮里先烧掉多少 fuel 就能决定快照上的改写会不会因为 fuel 耗尽被丢掉，之前事件攒下的状态借此影响上屏。它只被同一轮里之前的 `before_commit` 用掉，那些同样只看得到各自的上屏文本。
    pub fn deliver(&mut self, turn: u64, event: &Event<'_>) -> Outcome {
        self.actions.clear();
        if !self.module.declaration.hooks.contains(event.hook) {
            return Outcome::NotSubscribed;
        }
        if !self.usable {
            return Outcome::Failed {
                failure: Failure::Unusable,
                fuel: 0,
            };
        }
        if self.turn != Some(turn) {
            self.turn = Some(turn);
            self.turn_fuel_left = limits::TURN_FUEL;
            self.before_commit_fuel_left = limits::BEFORE_COMMIT_FUEL;
        }
        let before_commit = event.hook == Hook::BeforeCommit;
        let left = if before_commit {
            self.before_commit_fuel_left
        } else {
            self.turn_fuel_left
        };
        let hook_limit = limits::hook_fuel(event.hook);
        let allowed = left.min(hook_limit);
        if allowed == 0 {
            return Outcome::TurnExhausted { fuel: 0 };
        }
        let (result, fuel) = if before_commit && self.snapshot.is_some() {
            self.run_on_snapshot(allowed, event)
        } else {
            let result = run(
                &mut self.store,
                self.on_event,
                allowed,
                event,
                &mut self.actions,
            );
            if matches!(result.0, Err(failure) if failure != Failure::NegativeReturn) {
                self.usable = false;
            }
            result
        };
        if before_commit {
            self.before_commit_fuel_left -= fuel;
        } else {
            self.turn_fuel_left -= fuel;
        }
        match result {
            Ok(status) => Outcome::Delivered { status, fuel },
            // 截断它的是本轮剩下的配额，不是本钩子的上限。wasmi 在剩下的 fuel 不够下一段指令时就停，留下的零头本轮也用不上，记为用完，宿主据此在本轮里不再为它重建实例。用完的是这个钩子所用的那一份：`before_commit` 的配额截断不影响其余钩子的，反过来也一样。
            //
            // 这一次自己用掉的不到本钩子上限的一半时不算失败：没经过 `msime_client_key` 的派发只拿到上一次按键剩下的零头，被截断怪不到插件。用掉的达到一半就和超出自己上限一样算失败，否则轮配额和单次上限相等（`candidates`）或相近时，一个死循环只要同一轮里之前的钩子花过一点 fuel，就总是先撞上轮配额，熔断永远落不到它身上。
            Err(Failure::OutOfFuel) if allowed < hook_limit => {
                if before_commit {
                    self.before_commit_fuel_left = 0;
                } else {
                    self.turn_fuel_left = 0;
                }
                if fuel >= hook_limit / 2 {
                    Outcome::Failed {
                        failure: Failure::OutOfFuel,
                        fuel,
                    }
                } else {
                    Outcome::TurnExhausted { fuel }
                }
            }
            Err(failure) => Outcome::Failed { failure, fuel },
        }
    }

    /// 在一个新实例上复位到快照再运行，用完即弃。持久实例不受影响，观察类钩子攒下的状态照样留着，但它们进不了这一次。
    fn run_on_snapshot(&mut self, allowed: u64, event: &Event<'_>) -> (Result<i32, Failure>, u64) {
        let Some(snapshot) = &self.snapshot else {
            return (Err(Failure::Instantiate), 0);
        };
        let grants = self.store.data().grants;
        let mut store = new_store(&self.module, grants);
        // 实例化不跑任何 wasm 代码（start 函数已被拒绝），这里的 fuel 只为满足 wasmi 的计量，不算插件的。
        if store.set_fuel(limits::INIT_FUEL).is_err() {
            return (Err(Failure::Instantiate), 0);
        }
        let prepared = instantiate(&mut store, &self.module).and_then(|(instance, memory)| {
            snapshot.restore(&mut store, instance, memory)?;
            instance
                .get_typed_func::<i32, i32>(&store, abi::EXPORT_ON_EVENT)
                .map_err(|_| Failure::Instantiate)
        });
        let on_event = match prepared {
            Ok(func) => func,
            Err(failure) => return (Err(failure), 0),
        };
        store.data_mut().log_sink = self.store.data_mut().log_sink.take();
        store.data_mut().action_sink = self.store.data_mut().action_sink.take();
        let outcome = run(&mut store, on_event, allowed, event, &mut self.actions);
        self.store.data_mut().log_sink = store.data_mut().log_sink.take();
        self.store.data_mut().action_sink = store.data_mut().action_sink.take();
        outcome
    }
}

/// 在 `store` 上跑一次 `msime_on_event`，返回结果和用掉的 fuel。正常返回时把暂存的动作交给 `actions`，否则 `actions` 为空。
fn run(
    store: &mut Store<HostState>,
    on_event: TypedFunc<i32, i32>,
    allowed: u64,
    event: &Event<'_>,
    actions: &mut Actions,
) -> (Result<i32, Failure>, u64) {
    actions.clear();
    if store.set_fuel(allowed).is_err() {
        return (Err(Failure::Instantiate), 0);
    }
    let state = store.data_mut();
    state.context.load(state.grants, event);
    let result = on_event.call(&mut *store, event.hook.code());
    let context = &mut store.data_mut().context;
    if matches!(result, Ok(status) if status >= 0) {
        std::mem::swap(&mut context.actions, actions);
    }
    context.clear();
    let fuel = allowed - store.get_fuel().unwrap_or(0).min(allowed);
    let result = match result {
        Ok(status) if status >= 0 => Ok(status),
        Ok(_) => Err(Failure::NegativeReturn),
        Err(error) => Err(Failure::of(&error)),
    };
    (result, fuel)
}

/// 绑在模块自己的 Engine 上的链接表，编译时建一次，随模块共享。宿主函数都是不捕获状态的函数项。
pub(crate) fn linker(engine: &Engine) -> Linker<HostState> {
    let mut linker = Linker::new(engine);
    let module = abi::IMPORT_MODULE;
    let define = |result: Result<&mut Linker<HostState>, wasmi::errors::LinkerError>| {
        // 名字都来自 `abi::IMPORTS`，不会重复；重复是编程错误，测试会在第一次实例化时撞上。
        result.map(drop).expect("import names are unique")
    };
    define(linker.func_wrap(module, "event_scalar", event_scalar));
    define(linker.func_wrap(module, "log", log));
    define(linker.func_wrap(module, "composition_text", composition_text));
    define(linker.func_wrap(module, "composition_scalar", composition_scalar));
    define(linker.func_wrap(module, "commit_text", commit_text));
    define(linker.func_wrap(module, "commit_scalar", commit_scalar));
    define(linker.func_wrap(module, "candidate_count", candidate_count));
    define(linker.func_wrap(module, "candidate_text", candidate_text));
    define(linker.func_wrap(module, "candidate_code", candidate_code));
    define(linker.func_wrap(module, "candidate_annotation", candidate_annotation));
    define(linker.func_wrap(module, "candidate_meta", candidate_meta));
    define(linker.func_wrap(module, "rewrite_commit", rewrite_commit));
    define(linker.func_wrap(module, "candidates_order", candidates_order));
    define(linker.func_wrap(module, "candidates_hide", candidates_hide));
    define(linker.func_wrap(module, "candidates_annotate", candidates_annotate));
    linker
}

/// import 也计 fuel：每次调用 [`limits::IMPORT_CALL_FUEL`]，每复制 [`limits::IMPORT_BYTES_PER_FUEL`] 字节再加 1。不够时把 fuel 清零并按 fuel 耗尽 trap。
fn charge(caller: &mut Caller<'_, HostState>, bytes: usize) -> Result<(), Error> {
    let cost = limits::IMPORT_CALL_FUEL + bytes as u64 / limits::IMPORT_BYTES_PER_FUEL;
    charge_exact(caller, cost)
}

fn charge_exact(caller: &mut Caller<'_, HostState>, cost: u64) -> Result<(), Error> {
    let fuel = caller.get_fuel()?;
    if fuel < cost {
        caller.set_fuel(0)?;
        return Err(TrapCode::OutOfFuel.into());
    }
    caller.set_fuel(fuel - cost)
}

/// 指针越界直接 trap，不返回错误码。检查的是整个 `[ptr, ptr + len)`，与实际复制多少无关。
fn checked_range(
    caller: &Caller<'_, HostState>,
    ptr: i32,
    len: i32,
) -> Result<(Memory, usize, usize), Error> {
    let memory = caller
        .data()
        .memory
        .ok_or(Error::from(TrapCode::MemoryOutOfBounds))?;
    let start = ptr as u32 as usize;
    let len = len as u32 as usize;
    match start.checked_add(len) {
        Some(end) if end <= memory.data_size(caller) => Ok((memory, start, len)),
        _ => Err(TrapCode::MemoryOutOfBounds.into()),
    }
}

/// 文本类 import 的公共部分：能力 → 字段 → 越界 → 容量 → 复制。
fn copy_text(
    mut caller: Caller<'_, HostState>,
    capability: Capability,
    dst: i32,
    cap: i32,
    select: impl Fn(&CallContext) -> Result<&str, i32>,
) -> Result<i32, Error> {
    charge(&mut caller, 0)?;
    if !caller.data().grants.contains(capability) {
        return Ok(status::DENIED);
    }
    let len = match select(&caller.data().context) {
        Ok(text) => text.len(),
        Err(code) => return Ok(code),
    };
    let (memory, start, cap) = checked_range(&caller, dst, cap)?;
    if len > cap {
        return Ok(status::LIMIT);
    }
    charge_exact(&mut caller, len as u64 / limits::IMPORT_BYTES_PER_FUEL)?;
    let (data, state) = memory.data_and_store_mut(&mut caller);
    let Ok(text) = select(&state.context) else {
        return Ok(status::ABSENT);
    };
    data[start..start + len].copy_from_slice(text.as_bytes());
    Ok(len as i32)
}

/// 标量类 import 的公共部分。
fn scalar(
    mut caller: Caller<'_, HostState>,
    capability: Option<Capability>,
    read: impl Fn(&CallContext) -> i64,
) -> Result<i64, Error> {
    charge(&mut caller, 0)?;
    if capability.is_some_and(|capability| !caller.data().grants.contains(capability)) {
        return Ok(i64::from(status::DENIED));
    }
    Ok(read(&caller.data().context))
}

fn event_scalar(caller: Caller<'_, HostState>, field: i32) -> Result<i64, Error> {
    scalar(caller, None, |context| context.event_scalar(field))
}

fn log(mut caller: Caller<'_, HostState>, level: i32, src: i32, len: i32) -> Result<(), Error> {
    let (memory, start, len) = checked_range(&caller, src, len)?;
    charge(&mut caller, len)?;
    let (data, state) = memory.data_and_store_mut(&mut caller);
    if let Some(sink) = state.log_sink.as_mut() {
        sink(level, &String::from_utf8_lossy(&data[start..start + len]));
    }
    Ok(())
}

fn composition_text(
    caller: Caller<'_, HostState>,
    field: i32,
    dst: i32,
    cap: i32,
) -> Result<i32, Error> {
    copy_text(caller, Capability::ReadComposition, dst, cap, |context| {
        context.composition_text(field)
    })
}

fn composition_scalar(caller: Caller<'_, HostState>, field: i32) -> Result<i64, Error> {
    scalar(caller, Some(Capability::ReadComposition), |context| {
        context.composition_scalar(field)
    })
}

fn commit_text(
    caller: Caller<'_, HostState>,
    field: i32,
    dst: i32,
    cap: i32,
) -> Result<i32, Error> {
    copy_text(caller, Capability::ReadCommit, dst, cap, |context| {
        context.commit_text(field)
    })
}

fn commit_scalar(caller: Caller<'_, HostState>, field: i32) -> Result<i64, Error> {
    scalar(caller, Some(Capability::ReadCommit), |context| {
        context.commit_scalar(field)
    })
}

fn candidate_count(caller: Caller<'_, HostState>) -> Result<i32, Error> {
    scalar(caller, Some(Capability::ReadCandidates), |context| {
        if context.candidates_present {
            context.row_count as i64
        } else {
            i64::from(status::ABSENT)
        }
    })
    .map(|value| value as i32)
}

fn candidate_text(
    caller: Caller<'_, HostState>,
    index: i32,
    dst: i32,
    cap: i32,
) -> Result<i32, Error> {
    copy_text(caller, Capability::ReadCandidates, dst, cap, |context| {
        context.row(index).map(|row| row.text.as_str())
    })
}

fn candidate_code(
    caller: Caller<'_, HostState>,
    index: i32,
    dst: i32,
    cap: i32,
) -> Result<i32, Error> {
    copy_text(caller, Capability::ReadCandidates, dst, cap, |context| {
        context.row(index).map(|row| row.code.as_str())
    })
}

fn candidate_annotation(
    caller: Caller<'_, HostState>,
    index: i32,
    dst: i32,
    cap: i32,
) -> Result<i32, Error> {
    copy_text(caller, Capability::ReadCandidates, dst, cap, |context| {
        context.row(index).map(|row| row.annotation.as_str())
    })
}

fn candidate_meta(caller: Caller<'_, HostState>, index: i32, field: i32) -> Result<i64, Error> {
    scalar(caller, Some(Capability::ReadCandidates), |context| {
        context.candidate_meta(index, field)
    })
}

/// 修改类 import 的前置检查：授权里有这项能力，并且正在它所属的钩子里。不满足时返回 `DENIED`。
fn modification_allowed(
    caller: &Caller<'_, HostState>,
    capability: Capability,
    hook: Hook,
) -> bool {
    let state = caller.data();
    state.grants.contains(capability) && state.context.hook == Some(hook)
}

fn try_rewrite_commit(
    caller: &mut Caller<'_, HostState>,
    src: i32,
    len: i32,
) -> Result<i32, Error> {
    charge(caller, 0)?;
    if !modification_allowed(caller, Capability::RewriteCommit, Hook::BeforeCommit)
        || caller.data().context.scalars.rewritable != Some(true)
    {
        return Ok(status::DENIED);
    }
    let context = &caller.data().context;
    let Some(body) = context.body.get() else {
        return Ok(status::ABSENT);
    };
    // 上限按原文算：后面的插件拿到的 body 已经被前面的改写过，按它算会和宿主采用时的判断对不上。
    let original_utf16 = context
        .original_body_utf16
        .unwrap_or_else(|| body.encode_utf16().count());
    let limit = limits::rewrite_utf16_limit(original_utf16);
    let (memory, start, len) = checked_range(caller, src, len)?;
    if len > limits::MAX_REWRITE_BYTES {
        return Ok(status::LIMIT);
    }
    charge_exact(caller, len as u64 / limits::IMPORT_BYTES_PER_FUEL)?;
    let (data, state) = memory.data_and_store_mut(&mut *caller);
    let Ok(text) = std::str::from_utf8(&data[start..start + len]) else {
        return Ok(status::INVALID);
    };
    if text.is_empty() || abi::has_forbidden_plugin_char(text) {
        return Ok(status::INVALID);
    }
    if text.encode_utf16().count() > limit {
        return Ok(status::LIMIT);
    }
    let rewrite = state
        .context
        .actions
        .rewrite
        .get_or_insert_with(String::new);
    rewrite.clear();
    rewrite.push_str(text);
    Ok(0)
}

/// `candidates_order` 和 `candidates_hide` 的公共部分：能力和钩子 → 有候选 → 读 `count` 个小端 u32 行号 → 每个都在窗口里、不重复。成功时交出行号和个数，否则是返回码。
fn candidate_rows(
    caller: &mut Caller<'_, HostState>,
    capability: Capability,
    src: i32,
    count: i32,
) -> Result<Result<([u32; limits::MAX_CANDIDATE_ROWS], usize), i32>, Error> {
    charge(caller, 0)?;
    if !modification_allowed(caller, capability, Hook::Candidates) {
        return Ok(Err(status::DENIED));
    }
    let context = &caller.data().context;
    if !context.candidates_present {
        return Ok(Err(status::ABSENT));
    }
    let rows = context.row_count;
    let Some(count) = usize::try_from(count).ok().filter(|count| *count <= rows) else {
        return Ok(Err(status::INVALID));
    };
    // 行数不超过窗口的 20 行，乘 4 不会溢出。
    let (memory, start, len) = checked_range(caller, src, (count * 4) as i32)?;
    charge_exact(caller, len as u64 / limits::IMPORT_BYTES_PER_FUEL)?;
    let data = memory.data(&*caller);
    let mut indices = [0u32; limits::MAX_CANDIDATE_ROWS];
    for slot in 0..count {
        let at = start + slot * 4;
        let index = u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
        if index as usize >= rows || indices[..slot].contains(&index) {
            return Ok(Err(status::INVALID));
        }
        indices[slot] = index;
    }
    Ok(Ok((indices, count)))
}

fn try_candidates_order(
    caller: &mut Caller<'_, HostState>,
    src: i32,
    count: i32,
) -> Result<i32, Error> {
    let (indices, count) = match candidate_rows(caller, Capability::RerankCandidates, src, count)? {
        Ok(rows) => rows,
        Err(code) => return Ok(code),
    };
    let order = &mut caller.data_mut().context.actions.order;
    order.clear();
    order.extend_from_slice(&indices[..count]);
    Ok(0)
}

fn try_candidates_hide(
    caller: &mut Caller<'_, HostState>,
    src: i32,
    count: i32,
) -> Result<i32, Error> {
    let (indices, count) = match candidate_rows(caller, Capability::FilterCandidates, src, count)? {
        Ok(rows) => rows,
        Err(code) => return Ok(code),
    };
    let hidden = &mut caller.data_mut().context.actions.hidden;
    for index in &indices[..count] {
        if !hidden.contains(index) {
            hidden.push(*index);
        }
    }
    Ok(0)
}

fn try_candidates_annotate(
    caller: &mut Caller<'_, HostState>,
    index: i32,
    src: i32,
    len: i32,
) -> Result<i32, Error> {
    charge(caller, 0)?;
    if !modification_allowed(caller, Capability::AnnotateCandidates, Hook::Candidates) {
        return Ok(status::DENIED);
    }
    let context = &caller.data().context;
    if !context.candidates_present {
        return Ok(status::ABSENT);
    }
    let Some(index) = u32::try_from(index)
        .ok()
        .filter(|index| (*index as usize) < context.row_count)
    else {
        return Ok(status::INVALID);
    };
    let (memory, start, len) = checked_range(caller, src, len)?;
    // 32 个 UTF-16 码元最多 96 个 UTF-8 字节，更长的不用解码就知道放不下。
    if len > limits::MAX_ANNOTATION_UTF16 * 3 {
        return Ok(status::LIMIT);
    }
    charge_exact(caller, len as u64 / limits::IMPORT_BYTES_PER_FUEL)?;
    let (data, state) = memory.data_and_store_mut(&mut *caller);
    let Ok(text) = std::str::from_utf8(&data[start..start + len]) else {
        return Ok(status::INVALID);
    };
    if abi::has_forbidden_plugin_char(text) {
        return Ok(status::INVALID);
    }
    if text.encode_utf16().count() > limits::MAX_ANNOTATION_UTF16 {
        return Ok(status::LIMIT);
    }
    let annotations = &mut state.context.actions.annotations;
    match annotations.iter_mut().find(|(row, _)| *row == index) {
        Some((_, annotation)) => {
            annotation.clear();
            annotation.push_str(text);
        }
        None => annotations.push((index, text.to_owned())),
    }
    Ok(0)
}

/// 设了调试输出时，把修改类 import 的这次调用和交还给插件的返回码交给它。只是旁观：返回码由 import 本身决定，设不设它都一样；trap 不是返回码，不报告。
fn observed(
    caller: &mut Caller<'_, HostState>,
    import: &'static str,
    result: Result<i32, Error>,
) -> Result<i32, Error> {
    if let (Ok(status), Some(sink)) = (&result, caller.data_mut().action_sink.as_mut()) {
        sink(import, *status);
    }
    result
}

fn rewrite_commit(mut caller: Caller<'_, HostState>, src: i32, len: i32) -> Result<i32, Error> {
    let result = try_rewrite_commit(&mut caller, src, len);
    observed(&mut caller, "rewrite_commit", result)
}

fn candidates_order(mut caller: Caller<'_, HostState>, src: i32, count: i32) -> Result<i32, Error> {
    let result = try_candidates_order(&mut caller, src, count);
    observed(&mut caller, "candidates_order", result)
}

fn candidates_hide(mut caller: Caller<'_, HostState>, src: i32, count: i32) -> Result<i32, Error> {
    let result = try_candidates_hide(&mut caller, src, count);
    observed(&mut caller, "candidates_hide", result)
}

fn candidates_annotate(
    mut caller: Caller<'_, HostState>,
    index: i32,
    src: i32,
    len: i32,
) -> Result<i32, Error> {
    let result = try_candidates_annotate(&mut caller, index, src, len);
    observed(&mut caller, "candidates_annotate", result)
}
