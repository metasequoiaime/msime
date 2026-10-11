//! 生命周期核心：每次 FFI 调用返回前，从调用前后的组字采样和 runtime 的派发事实推导出一串事件，同步交给本会话的消费者。
//!
//! 只有经 `msime_client_set_lifecycle` 选择加入的会话才有 [`LifecycleState`]；没选择加入的会话（移动端全部如此）一个事件都不推导，transition JSON 逐字节不变。事件本身不带文本：文本只在调用期间留在 transition 里。
//!
//! 会话属于创建它的线程，所以这里不起线程、不建全局总线、不回调宿主。推导是纯函数（[`derive`]），只读 `Copy` 的采样，按键路径上不分配内存；消费者各自包一层 `catch_unwind`，panic 不会落到 `response()` 那一层把整次上屏变成 `"internal runtime failure"`。
//!
//! 同一次调用内的顺序是：`input` → `selection_retracted` → `candidate_selected` → `before_commit` → `commit` → `composition_end` 或 `composition_start` → `composition_update` → `candidates_shown` 或 `candidates_hidden`，focus 的 `focus_in`/`focus_out` 排在最后，destroy 的 `session_end` 也排在最后。
//!
//! 消费者有两类：内置的音效、特效和音乐（`FEEDBACK` 位），以及脚本插件（`crate::scripts`，只在桌面开发构建里、进程开关打开时才有事件可收）。旁路调用的视图事件只给内置消费者。

use crate::key_sound::{self, KeyClass, SessionSound};
use crate::scripts::{CallTexts, ScriptSet};
use msime_engine::LocalInputMode;
use msime_input_runtime::{
    DispatchFacts, Runtime, SelectionFact, SelectionHow, SelectionOutcome, SelectionRetraction,
    View,
};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

/// 核心驱动按键音、上屏音、打字特效和背景音乐。本会话的 `msime_client_key_sound`、`msime_client_commit_sound`、`msime_client_typing_effect`（事件 0-4）和 `msime_client_music_set_active` 随之变为空操作。
pub const FEEDBACK: u32 = 0x1;
/// 之后的上屏只能观察，不能改写。可随时经 `msime_client_set_lifecycle` 切换。
pub const COMMIT_OBSERVE_ONLY: u32 = 0x4;
/// 宿主自己暂存部分选词（Windows）：部分选词（`candidate_selected` 的 outcome 是 Partial）上屏的那一段还在宿主的前缀里，没进文档，内置消费者不为它出上屏音、不画上屏特效，等整段送出的那次上屏再出。生命周期事件照常发。
pub const HOST_HOLDS_PIECES: u32 = 0x8;
/// `msime_client_set_lifecycle` 接受的全部位，其余位被忽略。
pub const SUPPORTED_FLAGS: u32 = FEEDBACK | COMMIT_OBSERVE_ONLY | HOST_HOLDS_PIECES;

/// 字段属性位，宿主经 `msime_client_set_field_traits` 报告。
pub const TRAIT_SECURE: u32 = 0x01;
pub const TRAIT_PRIVATE: u32 = 0x02;
pub const TRAIT_NO_SPELLCHECK: u32 = 0x04;
pub const TRAIT_FULLSCREEN: u32 = 0x08;
pub const TRAIT_VOICE_ACTIVE: u32 = 0x10;
pub const TRAIT_HOST_ENGLISH: u32 = 0x20;
pub const TRAIT_ELEVATED: u32 = 0x40;
/// 宿主确实知道这个字段的属性。没有这一位时属性按未知处理：上屏不可改写（将来脚本也不运行）；但 SECURE 和 FULLSCREEN 位本身仍然生效，声音、特效和音乐照样按它们关闭或静音，宁可多静音一次。
pub const TRAIT_KNOWN: u32 = 0x8000_0000;
/// 认得的全部字段属性位，其余位被丢掉。
pub const SUPPORTED_TRAITS: u32 = TRAIT_SECURE
    | TRAIT_PRIVATE
    | TRAIT_NO_SPELLCHECK
    | TRAIT_FULLSCREEN
    | TRAIT_VOICE_ACTIVE
    | TRAIT_HOST_ENGLISH
    | TRAIT_ELEVATED
    | TRAIT_KNOWN;

/// `msime_client_key` 的低 7 位：按键类别 0-3，同 `msime_client_key_sound`。
pub const KEY_CLASS_MASK: u32 = 0x7F;
/// 指针点选或翻页：开启新的一轮，但不出声、不计连击。
pub const KEY_POINTER: u32 = 0x80;
/// 按住不放的自动重复，同 typing_effect 的位 8。
pub const KEY_REPEAT: u32 = 0x100;
/// 交给应用的键（Windows 经 Aux 管道异步上报）：不开新的一轮，也不承诺先于应用处理。
pub const KEY_PASSTHROUGH: u32 = 0x400;

/// `msime_client_command` 的命令 19：宿主自己写入文本后丢弃组字。
pub const COMMAND_CANCEL_AFTER_HOST_COMMIT: u32 = 19;
/// `msime_client_command` 的命令 20：焦点即将离开或会话即将关闭，宿主替用户结束组字。对会话同 `MSIME_FINISH_COMPOSITION`，上屏来源记 `FocusLoss`。
pub const COMMAND_FINISH_ON_FOCUS_LOSS: u32 = 20;

/// 一次物理按下，来自 `msime_client_key`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyDown {
    /// 按键类别 0-3；指针调用时为 0。
    pub class: u8,
    pub pointer: bool,
    pub repeat: bool,
    pub passthrough: bool,
}

/// 打字前那次派发是哪一类动作。判别值是将来脚本 ABI 的一部分，只能追加。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum InputClass {
    /// 字母、数字等字符键，含 14 键、滑行。
    Character = 0,
    /// 标点，`punctuation` 与 `punctuation_ascii` 两条路由。
    Punctuation = 1,
    /// 选候选：点选、数字、确认高亮行、以词定字、九键拼音选择。
    Selection = 2,
    /// 翻页和移动高亮。
    Navigation = 3,
    /// 退格、删除和光标移动。
    Edit = 4,
    /// 不选候选地结束组字：原样上屏、上屏读音、结束组字。
    Commit = 5,
    /// 取消（Esc）。
    Cancel = 6,
    /// 命令 19：宿主已经自己写入了文本，丢弃组字。
    HostCommit = 7,
    /// 其余：候选的置顶、删除、固定位置，假名切换，打开候选列表，清缓存，九键筛选。
    Other = 8,
    /// 命令 20：焦点即将离开或会话即将关闭，宿主替用户结束组字（macOS IMK 的 `commitComposition:`）。上屏同 focus(false)，来源记 `FocusLoss`。
    FocusLoss = 9,
}

impl InputClass {
    /// 派发前就要读出类别：动作随后被交给 runtime。`host` 是命令 19、20 指定的类别（`HostCommit`、`FocusLoss`），它们对会话的动作同 Cancel、Finish，只有生命周期里的类别不同。
    pub fn of(action: &msime_input_runtime::Action, host: Option<Self>) -> Self {
        use msime_engine::host::Command;
        use msime_input_runtime::Action;
        if let Some(class) = host {
            return class;
        }
        match action {
            Action::Character { .. } | Action::GridKey(_) | Action::Glide { .. } => Self::Character,
            Action::Punctuation(_) | Action::PunctuationAscii(_) => Self::Punctuation,
            Action::Select(_)
            | Action::SelectAnyCandidate(_)
            | Action::SelectEdge(..)
            | Action::SelectHighlighted
            | Action::ChooseNineKeySpelling(_) => Self::Selection,
            Action::NextPage
            | Action::PreviousPage
            | Action::NextCandidate
            | Action::PreviousCandidate
            | Action::FirstCandidate
            | Action::LastCandidate => Self::Navigation,
            Action::SegmentBackspace | Action::SegmentMoveLeft | Action::SegmentMoveRight => {
                Self::Edit
            }
            Action::Finish => Self::Commit,
            Action::Command(command) => match command {
                Command::Backspace
                | Command::DeleteForward
                | Command::MoveLeft
                | Command::MoveRight
                | Command::MoveHome
                | Command::MoveEnd
                | Command::ConversionLeft
                | Command::ConversionRight => Self::Edit,
                Command::CommitCandidate => Self::Selection,
                Command::CommitRaw | Command::CommitRawWithoutLearning | Command::CommitReading => {
                    Self::Commit
                }
                Command::Cancel => Self::Cancel,
                Command::CycleKanaVariant | Command::ConvertHanja => Self::Other,
            },
            Action::ResetCache
            | Action::PinCandidate(_)
            | Action::RemoveCandidate(_)
            | Action::FixCandidatePosition(..)
            | Action::ClearCandidatePosition(_)
            | Action::SetNineKeyFilter { .. } => Self::Other,
        }
    }
}

/// 本地模式的封闭枚举，事件里不带模式名字符串。判别值只能追加。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LocalModeClass {
    None = 0,
    TemporaryEnglish = 1,
    TemporaryJapanese = 2,
    SuperJianpin = 3,
    Emoji = 4,
    Kaomoji = 5,
    Mention = 6,
    Url = 7,
    Command = 8,
    Expression = 9,
    Unicode = 10,
    DateTime = 11,
    QuickPhrase = 12,
    /// 名字不认识（runtime 刷新失败时的 `"unknown"`）。
    Unknown = 255,
}

impl LocalModeClass {
    pub fn of(name: &str) -> Self {
        match LocalInputMode::from_name(name) {
            Some(LocalInputMode::None) => Self::None,
            Some(LocalInputMode::TemporaryEnglish) => Self::TemporaryEnglish,
            Some(LocalInputMode::TemporaryJapanese) => Self::TemporaryJapanese,
            Some(LocalInputMode::SuperJianpin) => Self::SuperJianpin,
            Some(LocalInputMode::Emoji) => Self::Emoji,
            Some(LocalInputMode::Kaomoji) => Self::Kaomoji,
            Some(LocalInputMode::Mention) => Self::Mention,
            Some(LocalInputMode::Url) => Self::Url,
            Some(LocalInputMode::Command) => Self::Command,
            Some(LocalInputMode::Expression) => Self::Expression,
            Some(LocalInputMode::Unicode) => Self::Unicode,
            Some(LocalInputMode::DateTime) => Self::DateTime,
            Some(LocalInputMode::QuickPhrase) => Self::QuickPhrase,
            None => Self::Unknown,
        }
    }
}

/// 上屏文本是怎么来的。判别值只能追加。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CommitOrigin {
    /// 选中了候选（`candidate_selected` 的 outcome 是 Committed 或 Partial）。
    Selection = 0,
    /// 原样上屏的读音（`CommitRaw`、bare 前缀）。
    Raw = 1,
    /// 暂存的词段随这次上屏送出，而不是本次选中的候选。
    HeldFlush = 2,
    /// 方案自己的文本：韩文音节、藏文、注音的空格、笔画字母、标点本身等。
    SchemeText = 3,
    /// `msime_client_focus(false)` 或命令 20 上屏的组字。不可改写，也不出声、不画。
    FocusLoss = 4,
}

/// 组字为什么结束；候选隐藏时也用它说明原因。判别值只能追加。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EndReason {
    /// 随上屏结束。
    Commit = 0,
    /// 取消（Esc）。
    Cancel = 1,
    /// 退格或删除删空了组字。
    Deleted = 2,
    /// focus 的任一方向。
    Focus = 3,
    /// `msime_client_destroy`。
    Destroyed = 4,
    /// 命令 19：宿主自己写入了文本。
    HostCommit = 5,
    /// 其余：旁路调用、清缓存等。
    Other = 6,
}

/// 生命周期事件，不带任何文本。判别值顺序就是 ABI 名的顺序，只能追加。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleEvent {
    FocusIn,
    FocusOut,
    TraitsChanged {
        traits: u32,
    },
    /// `msime_client_set_active(true)`。
    Activated,
    /// `msime_client_set_active(false)`。
    Deactivated,
    KeyDown(KeyDown),
    /// 打字前，排在本次派发的 runtime 调用之前。
    Input {
        class: InputClass,
        local_mode: LocalModeClass,
    },
    SelectionRetracted(SelectionRetraction),
    CandidateSelected(SelectionFact),
    /// 上屏前，只在派发类调用里出现。本阶段只是内部发出点，不改写。`body_chars` + `tail_chars` 是上屏文本的字符数（Unicode 标量）；tail 是标点或顶字的尾巴，只读。
    BeforeCommit {
        origin: CommitOrigin,
        body_chars: u32,
        tail_chars: u32,
        rewritable: bool,
    },
    Commit {
        origin: CommitOrigin,
        chars: u32,
        rewritten: bool,
        /// 同一个 transition 里接着有新的组字（韩文音节延续），这时不发 end 和 start。
        continues: bool,
        /// 引擎生成的文本（计算器、命令、提及），不算打字。
        generated: bool,
    },
    CompositionEnd(EndReason),
    CompositionStart,
    CompositionUpdate,
    /// 每次调用最多一次。`count` 是当前页的候选数，`highlighted` 是高亮行在页内的下标。
    CandidatesShown {
        count: u16,
        page: u16,
        highlighted: Option<u16>,
    },
    CandidatesHidden(EndReason),
    /// destroy 时最后一个事件。
    SessionEnd,
}

// 事件按值在按键路径上传递，必须保持小而 `Copy`。
const _: () = assert!(std::mem::size_of::<LifecycleEvent>() <= 24);

/// FFI 调用边界一侧的组字状态。调用前后各采样一次，不用 `is_idle()`：它把 `snapshot_valid` 也算了进去。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sample {
    pub composing: bool,
    pub composition_epoch: u64,
    pub candidates_epoch: u64,
    pub candidates_visible: bool,
}

impl Sample {
    pub fn of<E: msime_input_runtime::InputEngine>(runtime: &Runtime<E>) -> Self {
        Self {
            composing: runtime.has_composition(),
            composition_epoch: runtime.composition_epoch(),
            candidates_epoch: runtime.candidates_epoch(),
            candidates_visible: runtime.candidate_page_len() > 0,
        }
    }
}

/// 调用后视图里 `candidates_shown` 需要的几项。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageSample {
    pub count: u16,
    pub page: u16,
    pub highlighted: Option<u16>,
}

impl PageSample {
    pub fn of(view: &View) -> Self {
        let narrow = |value: usize| u16::try_from(value).unwrap_or(u16::MAX);
        Self {
            count: narrow(view.candidates.len()),
            page: narrow(view.page),
            highlighted: view
                .candidates
                .iter()
                .position(|candidate| candidate.highlighted)
                .map(narrow),
        }
    }
}

/// 这次调用上屏的文本，只取推导要用的几项。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommitSample {
    /// 上屏文本的字符数（Unicode 标量），全角转换之后的。
    pub chars: u32,
    /// `commit_context.local_mode == "none"`。
    pub plain: bool,
    /// `commit_context.typing_statistics`：不是引擎生成的文本。
    pub typed: bool,
}

impl CommitSample {
    pub fn of(transition: &msime_input_runtime::Transition) -> Option<Self> {
        let commit = transition.commit.as_deref()?;
        let context = transition.commit_context.as_ref();
        Some(Self {
            chars: u32::try_from(commit.chars().count()).unwrap_or(u32::MAX),
            plain: context.is_none_or(|context| context.local_mode == "none"),
            typed: context.is_none_or(|context| context.typing_statistics),
        })
    }
}

/// 产生事件的是哪一类调用。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    /// 派发类调用（`dispatch`），带着打字前算好的类别。
    Dispatch(InputClass),
    /// `msime_client_focus`。
    Focus(bool),
    /// `apply_online_*`、`rerank_settled` 等旁路调用：只给核心发视图类事件。
    SideChannel,
    Destroy,
}

/// 推导时要看的会话设置。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub flags: u32,
    pub traits: u32,
}

impl Context {
    /// 字段属性已知，且不是 SECURE、PRIVATE 或 ELEVATED。上屏能否改写和脚本能否运行都看它。
    pub(crate) fn ordinary_field(self) -> bool {
        self.traits & TRAIT_KNOWN != 0
            && self.traits & (TRAIT_SECURE | TRAIT_PRIVATE | TRAIT_ELEVATED) == 0
    }
}

/// 从采样和派发事实推导本次调用的事件，按顺序交给 `emit`。纯函数：不读会话、不分配内存。`input` 不在这里：它排在 runtime 调用之前，由调用方先发。
#[allow(clippy::too_many_arguments)]
pub fn derive(
    call: Call,
    context: Context,
    pre: Sample,
    post: Sample,
    facts: DispatchFacts,
    commit: Option<CommitSample>,
    page: PageSample,
    mut emit: impl FnMut(LifecycleEvent),
) {
    let dispatch = matches!(call, Call::Dispatch(_));
    if dispatch {
        if let Some(retraction) = facts.retracted {
            emit(LifecycleEvent::SelectionRetracted(retraction));
        }
        if let Some(selection) = facts.selection {
            emit(LifecycleEvent::CandidateSelected(selection));
        }
    }
    // 一次 transition 同时带着上屏和新组字（韩文音节延续）：发 commit{continues}，不发 end 和 start。
    let continues = commit.is_some() && pre.composing && post.composing;
    if let Some(commit) = commit {
        let origin = commit_origin(call, facts);
        if dispatch {
            let tail_chars = facts.tail_chars.min(commit.chars);
            emit(LifecycleEvent::BeforeCommit {
                origin,
                body_chars: commit.chars - tail_chars,
                tail_chars,
                rewritable: rewritable(origin, context, facts, commit),
            });
        }
        emit(LifecycleEvent::Commit {
            origin,
            chars: commit.chars,
            rewritten: false,
            continues,
            generated: !commit.typed,
        });
    }
    let reason = end_reason(call, commit.is_some());
    let started = !pre.composing && post.composing;
    if pre.composing && !post.composing {
        emit(LifecycleEvent::CompositionEnd(reason));
    } else if started {
        emit(LifecycleEvent::CompositionStart);
    }
    if post.composing && (started || pre.composition_epoch != post.composition_epoch) {
        emit(LifecycleEvent::CompositionUpdate);
    }
    if post.candidates_visible
        && (!pre.candidates_visible || pre.candidates_epoch != post.candidates_epoch)
    {
        emit(LifecycleEvent::CandidatesShown {
            count: page.count,
            page: page.page,
            highlighted: page.highlighted,
        });
    } else if pre.candidates_visible && !post.candidates_visible {
        emit(LifecycleEvent::CandidatesHidden(reason));
    }
    match call {
        Call::Focus(true) => emit(LifecycleEvent::FocusIn),
        Call::Focus(false) => emit(LifecycleEvent::FocusOut),
        Call::Destroy => emit(LifecycleEvent::SessionEnd),
        Call::Dispatch(_) | Call::SideChannel => {}
    }
}

fn commit_origin(call: Call, facts: DispatchFacts) -> CommitOrigin {
    // 命令 20 是宿主替失焦结束组字，和 focus(false) 的上屏一样处理，即使引擎把它记成了选中候选。
    if matches!(
        call,
        Call::Focus(false) | Call::Dispatch(InputClass::FocusLoss)
    ) {
        return CommitOrigin::FocusLoss;
    }
    match facts.selection {
        Some(SelectionFact {
            outcome: SelectionOutcome::Committed | SelectionOutcome::Partial,
            ..
        }) => CommitOrigin::Selection,
        _ if facts.held_flushed => CommitOrigin::HeldFlush,
        _ if facts.raw => CommitOrigin::Raw,
        _ => CommitOrigin::SchemeText,
    }
}

/// 上屏能不能被改写（第 4 阶段才真的改写）。以词定字一律不可改写：macOS 开着繁体输出时按宿主自己注入的键重算这段上屏，改写结果会被覆盖。
fn rewritable(
    origin: CommitOrigin,
    context: Context,
    facts: DispatchFacts,
    commit: CommitSample,
) -> bool {
    matches!(
        origin,
        CommitOrigin::Selection | CommitOrigin::Raw | CommitOrigin::HeldFlush
    ) && commit.plain
        && commit.typed
        && !facts
            .selection
            .is_some_and(|selection| selection.how == SelectionHow::Edge)
        && context.flags & COMMIT_OBSERVE_ONLY == 0
        && context.ordinary_field()
}

fn end_reason(call: Call, committed: bool) -> EndReason {
    match call {
        Call::Destroy => EndReason::Destroyed,
        Call::Focus(_) => EndReason::Focus,
        Call::SideChannel => EndReason::Other,
        Call::Dispatch(_) if committed => EndReason::Commit,
        Call::Dispatch(InputClass::HostCommit) => EndReason::HostCommit,
        Call::Dispatch(InputClass::Cancel) => EndReason::Cancel,
        Call::Dispatch(InputClass::Edit) => EndReason::Deleted,
        Call::Dispatch(_) => EndReason::Other,
    }
}

/// 背景音乐的所有者会话，0 是没有所有者（会话号从 1 开始）。播放器全进程只有一个，音乐开关以后写的为准，所以由这里仲裁：只有所有者能开关音乐。
static MUSIC_OWNER: AtomicU64 = AtomicU64::new(0);
/// 消费者 panic 只报告一次，免得每个键刷一行。
static CONSUMER_PANIC_REPORTED: AtomicBool = AtomicBool::new(false);

/// 一个选择加入的会话的生命周期状态，挂在 `HostSession.lifecycle` 上。
pub(crate) struct LifecycleState {
    handle: u64,
    flags: u32,
    traits: u32,
    /// 宿主经 `msime_client_set_active` 报告的激活状态。
    active: bool,
    /// 本次调用里内置消费者算出的打字特效，FEEDBACK 时由 `msime_client_key` 返回、或写进 transition 的 `feedback`。每次调用开始时清零。
    effect: u32,
    /// 这个会话的脚本插件。移动端是什么都不做的空壳。
    scripts: ScriptSet,
    /// 本次调用的上屏是宿主暂存的一段部分选词（HOST_HOLDS_PIECES 下 outcome 为 Partial），每次调用结束推导事件前重算。
    held_piece: bool,
    /// 测试用的消费者：记录事件或故意 panic。
    #[cfg(test)]
    pub(crate) probe: Option<Probe>,
}

/// 测试注入的消费者。
#[cfg(test)]
pub(crate) type Probe = Box<dyn FnMut(&LifecycleEvent)>;

impl LifecycleState {
    pub(crate) fn new(handle: u64, flags: u32) -> Self {
        Self {
            handle,
            flags: flags & SUPPORTED_FLAGS,
            traits: 0,
            active: false,
            effect: 0,
            scripts: ScriptSet::default(),
            held_piece: false,
            #[cfg(test)]
            probe: None,
        }
    }

    /// 记下脚本插件的位置：`root` 是 `<state root>/plugins`（宿主没给状态根目录时为 `None`，脚本不运行），健康文件写在 `user_data` 下。
    pub(crate) fn bind_scripts(&mut self, root: Option<std::path::PathBuf>, user_data: &str) {
        self.scripts.bind(root, user_data);
    }

    /// `msime_client_focus(true)` 开始时：丢弃上一个字段的脚本实例，按戳重读授权。
    pub(crate) fn scripts_focus_in(&mut self) {
        self.scripts.focus_in();
    }

    /// 测量直接对着脚本集合投递。
    #[cfg(test)]
    pub(crate) fn scripts_mut(&mut self) -> &mut ScriptSet {
        &mut self.scripts
    }

    pub(crate) fn flags(&self) -> u32 {
        self.flags
    }

    pub(crate) fn feedback(&self) -> bool {
        self.flags & FEEDBACK != 0
    }

    pub(crate) fn context(&self) -> Context {
        Context {
            flags: self.flags,
            traits: self.traits,
        }
    }

    /// 换一组选择加入的位。去掉 FEEDBACK 时交还音乐：之后由宿主自己调 `msime_client_music_set_active`。
    pub(crate) fn set_flags(&mut self, sound: &SessionSound, flags: u32) -> u32 {
        let flags = flags & SUPPORTED_FLAGS;
        if self.feedback() && flags & FEEDBACK == 0 {
            self.release_music(sound);
        }
        self.flags = flags;
        flags
    }

    /// 记下新的字段属性；变了才发 `traits_changed`。返回现在生效的属性。
    pub(crate) fn set_traits(&mut self, sound: &SessionSound, traits: u32) -> u32 {
        let traits = traits & SUPPORTED_TRAITS;
        if traits != self.traits {
            self.traits = traits;
            self.emit(sound, LifecycleEvent::TraitsChanged { traits });
        }
        // 不论变没变，脚本都把属性记为确认过；变成 SECURE、PRIVATE、ELEVATED 或未知时，正在跑的脚本实例当场丢弃。
        self.scripts.traits_reported(self.context());
        self.traits
    }

    pub(crate) fn set_active(&mut self, sound: &SessionSound, active: bool) {
        self.active = active;
        let event = if active {
            LifecycleEvent::Activated
        } else {
            LifecycleEvent::Deactivated
        };
        self.emit(sound, event);
    }

    /// 一次 `msime_client_key`。返回 FEEDBACK 下这个键的打字特效，位布局同 `msime_client_typing_effect`；类别不认识时什么都不做，返回 0。
    pub(crate) fn key(&mut self, sound: &SessionSound, key: u32) -> u32 {
        let pointer = key & KEY_POINTER != 0;
        let class = if pointer { 0 } else { key & KEY_CLASS_MASK };
        if KeyClass::from_code(class).is_none() {
            return 0;
        }
        self.effect = 0;
        // 交给应用的键不开新的一轮，脚本的 fuel 也按轮给；它也不确认字段属性：Windows 上这类键走 Aux 管道，和 Main 管道上的属性帧没有先后保证，可能先于新字段的属性到达。
        if key & KEY_PASSTHROUGH == 0 {
            self.scripts.key_reported();
            self.scripts.next_turn();
        }
        self.emit(
            sound,
            LifecycleEvent::KeyDown(KeyDown {
                class: class as u8,
                pointer,
                repeat: key & KEY_REPEAT != 0,
                passthrough: key & KEY_PASSTHROUGH != 0,
            }),
        );
        self.effect
    }

    /// 派发前：清零本次的特效，发 `input`。`input_char` 是这次打的字符，只交给授予了 `read_composition` 的脚本。
    pub(crate) fn before_dispatch(
        &mut self,
        sound: &SessionSound,
        class: InputClass,
        local_mode: &str,
        input_char: Option<char>,
    ) {
        self.effect = 0;
        let local_mode = LocalModeClass::of(local_mode);
        self.emit(sound, LifecycleEvent::Input { class, local_mode });
        self.scripts
            .input(self.context(), class, local_mode, input_char);
    }

    /// 调用结束后推导并发出其余事件。返回本次的打字特效（只有 FEEDBACK 下的上屏会写它）。脚本收不到任何文本，见 [`Self::after_texts`]。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn after(
        &mut self,
        sound: &SessionSound,
        call: Call,
        pre: Sample,
        post: Sample,
        facts: DispatchFacts,
        commit: Option<CommitSample>,
        page: PageSample,
    ) -> u32 {
        self.after_texts(
            sound,
            call,
            pre,
            post,
            facts,
            commit,
            page,
            &CallTexts::default(),
        )
    }

    /// 同 [`Self::after`]，脚本插件按授权从 `texts` 读这次调用的文本。旁路调用的事件不交给脚本；失焦和 destroy 在最后一个事件之后丢弃全部脚本实例。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn after_texts(
        &mut self,
        sound: &SessionSound,
        call: Call,
        pre: Sample,
        post: Sample,
        facts: DispatchFacts,
        commit: Option<CommitSample>,
        page: PageSample,
        texts: &CallTexts<'_>,
    ) -> u32 {
        if !matches!(call, Call::Dispatch(_)) {
            self.effect = 0;
        }
        self.held_piece = self.flags & HOST_HOLDS_PIECES != 0
            && matches!(
                facts.selection,
                Some(SelectionFact {
                    outcome: SelectionOutcome::Partial,
                    ..
                })
            );
        let context = self.context();
        let to_scripts = call != Call::SideChannel;
        derive(call, context, pre, post, facts, commit, page, |event| {
            self.emit(sound, event);
            if to_scripts {
                self.scripts.deliver(&event, context, texts);
            }
        });
        if call == Call::Destroy {
            self.release_music(sound);
        }
        if matches!(call, Call::Focus(false) | Call::Destroy) {
            self.scripts.release();
        }
        self.effect
    }

    /// 把一个事件交给每个消费者，各自隔离 panic。
    fn emit(&mut self, sound: &SessionSound, event: LifecycleEvent) {
        #[cfg(test)]
        if let Some(probe) = self.probe.as_mut() {
            isolate(|| probe(&event));
        }
        if self.feedback() {
            isolate(|| self.feedback_consumer(sound, event));
        }
    }

    /// 内置消费者：按键音、上屏音、打字特效和背景音乐。SECURE 字段里不出任何声音和特效，音乐关闭；全屏前台时声音静音、特效照画（同 Windows 的 `sound_allowed`）。
    fn feedback_consumer(&mut self, sound: &SessionSound, event: LifecycleEvent) {
        let secure = self.traits & TRAIT_SECURE != 0;
        let muted = self.traits & TRAIT_FULLSCREEN != 0;
        let muted_flag = if muted { key_sound::FLAG_MUTED } else { 0 };
        match event {
            LifecycleEvent::KeyDown(key) if !key.pointer && !secure => {
                let Some(class) = KeyClass::from_code(u32::from(key.class)) else {
                    return;
                };
                if !muted {
                    play(sound, Cue::Key(class));
                }
                let repeat = if key.repeat {
                    key_sound::FLAG_AUTO_REPEAT
                } else {
                    0
                };
                self.effect = key_sound::typing_effect(
                    sound,
                    class as u32 | repeat | muted_flag,
                    Instant::now(),
                );
            }
            // 失焦上屏（focus(false) 和命令 20）不出声也不画：IBus 根本不插入这段运行时副本，macOS 失焦时出提交音也是错的。宿主暂存的一段部分选词还没进文档，同样不出声、不画，等整段送出的那次上屏。
            LifecycleEvent::Commit { origin, .. }
                if origin != CommitOrigin::FocusLoss && !secure && !self.held_piece =>
            {
                if !muted {
                    play(sound, Cue::Commit);
                }
                self.effect = key_sound::typing_effect(
                    sound,
                    key_sound::EVENT_COMMIT | muted_flag,
                    Instant::now(),
                );
            }
            LifecycleEvent::Activated => {
                MUSIC_OWNER.store(self.handle, Ordering::Release);
                self.sync_music(sound);
            }
            LifecycleEvent::Deactivated | LifecycleEvent::TraitsChanged { .. } => {
                self.sync_music(sound);
            }
            _ => {}
        }
    }

    /// 所有者会话把音乐状态下发给播放器：是所有者、处于激活、字段既不是 SECURE 也不在全屏前台。不是所有者时什么都不做，别的会话关不掉所有者的音乐。
    fn sync_music(&self, sound: &SessionSound) {
        if let Some(playing) = self.music_wanted() {
            key_sound::music_active(sound, playing);
        }
    }

    /// 这个会话要下发的音乐状态；不是所有者时为 `None`。
    fn music_wanted(&self) -> Option<bool> {
        (MUSIC_OWNER.load(Ordering::Acquire) == self.handle)
            .then_some(self.active && self.traits & (TRAIT_SECURE | TRAIT_FULLSCREEN) == 0)
    }

    /// 是所有者时关掉音乐并让出所有权（destroy、去掉 FEEDBACK）。
    fn release_music(&self, sound: &SessionSound) {
        if MUSIC_OWNER
            .compare_exchange(self.handle, 0, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            isolate(|| {
                key_sound::music_active(sound, false);
            });
        }
    }
}

/// 一次调用开始时记下的东西：调用的类别和调用前的采样，调用结束后据此推导事件。
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pending {
    call: Call,
    before: Sample,
}

/// `HostSession` 一侧的接线。每个方法在会话没选择加入时都立即返回，不采样、不推导。
impl crate::HostSession {
    /// 本会话的按键音、上屏音、特效和音乐由核心驱动：旧的 `key_sound`、`commit_sound`、`typing_effect`（0-4）和 `music_set_active` 对它是空操作。
    pub(crate) fn feedback_driven(&self) -> bool {
        self.lifecycle
            .as_deref()
            .is_some_and(LifecycleState::feedback)
    }

    /// 派发前：采样并发 `input`。`host` 是命令 19、20 指定的类别，见 [`InputClass::of`]。
    pub(crate) fn lifecycle_input(
        &mut self,
        action: &msime_input_runtime::Action,
        host: Option<InputClass>,
    ) -> Option<Pending> {
        let state = self.lifecycle.as_deref_mut()?;
        let before = Sample::of(&self.runtime);
        let class = InputClass::of(action, host);
        state.before_dispatch(
            &self.sound,
            class,
            self.runtime.local_mode(),
            typed_char(action),
        );
        Some(Pending {
            call: Call::Dispatch(class),
            before,
        })
    }

    /// focus 和旁路调用开始时只采样，不发事件。
    pub(crate) fn lifecycle_begin(&self, call: Call) -> Option<Pending> {
        self.lifecycle.as_ref()?;
        Some(Pending {
            call,
            before: Sample::of(&self.runtime),
        })
    }

    /// 派发或 focus 的 transition 已经完成（全角转换之后）：推导其余事件。返回 FEEDBACK 下本次的打字特效，没开 FEEDBACK 时为 `None`。
    pub(crate) fn lifecycle_after(
        &mut self,
        pending: Option<Pending>,
        facts: DispatchFacts,
        transition: &msime_input_runtime::Transition,
    ) -> Option<u32> {
        let pending = pending?;
        let after = Sample::of(&self.runtime);
        let state = self.lifecycle.as_deref_mut()?;
        let texts = CallTexts {
            view: Some(&transition.view),
            commit: transition.commit.as_deref(),
            commit_local_mode: transition
                .commit_context
                .as_ref()
                .map(|context| context.local_mode.as_str()),
            facts,
            selected: self.runtime.selected_text(),
        };
        let effect = state.after_texts(
            &self.sound,
            pending.call,
            pending.before,
            after,
            facts,
            CommitSample::of(transition),
            PageSample::of(&transition.view),
            &texts,
        );
        state.feedback().then_some(effect)
    }

    /// 旁路调用之后：只有视图类事件，交给核心，不投递给脚本。
    pub(crate) fn lifecycle_view(&mut self, pending: Option<Pending>, view: &View) {
        let Some(pending) = pending else {
            return;
        };
        let after = Sample::of(&self.runtime);
        if let Some(state) = self.lifecycle.as_deref_mut() {
            state.after(
                &self.sound,
                pending.call,
                pending.before,
                after,
                DispatchFacts::default(),
                None,
                PageSample::of(view),
            );
        }
    }

    /// 旁路调用之后、手上没有现成视图时用：只在选择加入时才构造视图。
    pub(crate) fn lifecycle_settle(&mut self, pending: Option<Pending>) {
        if pending.is_some() {
            let view = self.runtime.view();
            self.lifecycle_view(pending, &view);
        }
    }

    /// 调用报错时也要收尾：runtime 可能在报错前已经改了组字（失焦取消了组字后刷新失败、引擎报错前已刷新、撤回暂存的选择后重打失败），不推导的话这段组字的 end 和候选的 hidden 就丢了，下一次调用的调用前采样已经不在组字，start 和 end 从此失配。没有上屏；选择没能完成，不报 `candidate_selected`。focus 照样发 `focus_in`/`focus_out`，消费者的焦点状态跟宿主走。原样交回 `result`。
    pub(crate) fn lifecycle_on_error<T, E>(
        &mut self,
        pending: Option<Pending>,
        result: Result<T, E>,
    ) -> Result<T, E> {
        if let (Err(_), Some(pending)) = (&result, pending) {
            let facts = DispatchFacts {
                selection: None,
                ..self.runtime.facts()
            };
            let view = self.runtime.view();
            let after = Sample::of(&self.runtime);
            if let Some(state) = self.lifecycle.as_deref_mut() {
                let texts = CallTexts {
                    view: Some(&view),
                    facts,
                    ..CallTexts::default()
                };
                state.after_texts(
                    &self.sound,
                    pending.call,
                    pending.before,
                    after,
                    facts,
                    None,
                    PageSample::of(&view),
                    &texts,
                );
            }
        }
        result
    }

    /// `msime_client_focus(true)` 开始时，紧挨着插件表的刷新：丢弃上一个字段留下的脚本实例，按戳重读脚本授权。
    pub(crate) fn lifecycle_focus_in(&mut self) {
        if let Some(state) = self.lifecycle.as_deref_mut() {
            state.scripts_focus_in();
        }
    }

    /// 会话被销毁：还在组字时先发 `composition_end{Destroyed}`，最后发 `session_end`；是音乐所有者时关掉音乐。脚本实例随之丢弃。
    pub(crate) fn lifecycle_destroy(&mut self) {
        let before = Sample::of(&self.runtime);
        if let Some(state) = self.lifecycle.as_deref_mut() {
            state.after(
                &self.sound,
                Call::Destroy,
                before,
                Sample::default(),
                DispatchFacts::default(),
                None,
                PageSample::default(),
            );
        }
    }
}

/// 只给集成测试 `tests/lifecycle_alloc.rs` 用：对本线程上一个选择加入的会话，拿给定的采样和事实走一遍真实的 [`LifecycleState::after`]（推导加全部消费者），不经过 runtime，也不构造视图和 JSON。内置消费者的上屏分支只有经过真实的会话状态才走得到，零分配断言要量到它。返回本次的打字特效；会话不存在、重入调用或没选择加入时为 `None`。
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn replay(
    handle: u64,
    call: Call,
    pre: Sample,
    post: Sample,
    facts: DispatchFacts,
    commit: Option<CommitSample>,
    page: PageSample,
) -> Option<u32> {
    crate::SESSIONS.with(|sessions| {
        let mut sessions = sessions.try_borrow_mut().ok()?;
        let session = sessions.get_mut(&handle)?;
        let state = session.lifecycle.as_deref_mut()?;
        Some(state.after(&session.sound, call, pre, post, facts, commit, page))
    })
}

/// 打字前那次派发打的字符：字符键、标点和 14 键的一键。
fn typed_char(action: &msime_input_runtime::Action) -> Option<char> {
    use msime_input_runtime::Action;
    match action {
        Action::Character { value, .. }
        | Action::Punctuation(value)
        | Action::PunctuationAscii(value)
        | Action::GridKey(value) => Some(char::from(*value)),
        _ => None,
    }
}

/// 内置消费者要出的一个声音。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cue {
    Key(KeyClass),
    Commit,
}

/// 把声音交给播放器。测试里先记进本线程的 [`CUES`]：设置里关着声音时播放器什么都不收，只看播放器的回答分不出消费者有没有守住 SECURE、全屏和失焦。
fn play(sound: &SessionSound, cue: Cue) {
    #[cfg(test)]
    CUES.with(|cues| cues.borrow_mut().push(cue));
    match cue {
        Cue::Key(class) => key_sound::key(sound, class),
        Cue::Commit => key_sound::commit(sound),
    };
}

#[cfg(test)]
thread_local! {
    /// 本线程上内置消费者要过的声音，测试读完自己清空。
    pub(crate) static CUES: std::cell::RefCell<Vec<Cue>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// 运行一个消费者，吞掉它的 panic 并报告一次。
fn isolate(consumer: impl FnOnce()) {
    if catch_unwind(AssertUnwindSafe(consumer)).is_err()
        && !CONSUMER_PANIC_REPORTED.swap(true, Ordering::AcqRel)
    {
        crate::diagnostics::report(
            "lifecycle consumer panicked",
            "the event was dropped and input continued",
        );
    }
}

#[cfg(test)]
mod tests;
