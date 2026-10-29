//! Private aggregate typing statistics shared by native hosts and settings UI.
//! Committed text is classified in memory and is never serialized.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::SystemTime;
use unicode_general_category::{get_general_category, GeneralCategory};
use unicode_segmentation::UnicodeSegmentation;

/// Only a guard against loading a hostile or garbage file, not a retention limit. It must stay far above anything `Forever` can produce, because a document over it cannot be read at all and the whole history is lost with it; a day costs a few hundred bytes, so 64 MiB covers centuries.
const MAX_DOCUMENT_BYTES: u64 = 64 * 1_048_576;
const STATISTICS_FILE: &str = "typing-statistics.json";
const LOCK_FILE: &str = "typing-statistics.lock";
const MAX_COMMIT_BYTES: usize = 40_000;
const MAX_COMMIT_SCALARS: usize = 10_000;
const MAX_COUNT: u64 = 9_000_000_000_000_000;
/// Buckets in a day, one per local hour.
pub const HOURS: usize = 24;
/// A day cannot hold more active time than it has milliseconds.
const MAX_ACTIVE_MS_PER_DAY: u64 = 24 * 60 * 60 * 1000;
/// A pause of at most this much between two consecutive commits counts as active typing time.
///
/// Taken from the Windows baseline, which calibrated it on real input: at five seconds ordinary
/// thinking pauses were counted as typing and the speed reading came out too low. It is the one
/// number here that decides what "active" means, so it is a constant with a reason rather than a
/// literal in the middle of `record`.
const ACTIVE_GAP_LIMIT_MS: u64 = 10_000;
/// 一次 `record_voice` 最多记 10 分钟：更长的数字只能来自宿主计时出错，记下来会让「动口不动手」一次解锁。
pub const MAX_VOICE_MS_PER_CALL: u64 = 600_000;
/// `skins_tried` 最多记这么多款。徽章只要 5 款，上限只防止文件被一个出错的宿主无限撑大。
pub const MAX_SKINS_TRIED: usize = 64;
/// 连续输入（`current_run`、`longest_run`）只在 Android 上记：只有 Android 的统计页显示它，其他宿主写出的统计文件保持原样。测试里也打开，好覆盖计法。
const TRACKS_TYPING_RUNS: bool = cfg!(any(target_os = "android", test));
/// `record` 攒下的上屏，距上次读写这个目录的统计文件满这么久后，由下一次上屏一起写出。
///
/// 每次上屏都整份读、解析、重写并 fsync 统计文件（Apple 上是 F_FULLFSYNC），在低端手机上英文模式每个字母一次，耗电发热；30 秒写一次是维护者定下的跨平台约定，宿主另在键盘收起、输入框失焦和进程退出前调 `flush`。进程被杀时最多丢这么久的计数，与按键热力图的批次相同。
pub const COMMIT_FLUSH_INTERVAL_MS: u64 = 30_000;
/// 一个目录最多攒这么多次上屏，到了就写，不再等 [`COMMIT_FLUSH_INTERVAL_MS`]：只为给内存设个上限，正常打字 30 秒到不了。
const MAX_PENDING_COMMITS: usize = 1024;

mod metrics;
/// 每日计数上传到云端（用户打开「上传到云端」后）。
pub mod upload;
pub use metrics::{
    achievement_specs, summarize, AchievementGroup, AchievementSpec, AchievementSummary, DayCount,
    HabitsSummary, KeysSummary, OverviewSummary, PeakWindow, SummaryInputs, TypingSummary,
};

/// Statistics are off until the user turns them on.
///
/// The Windows baseline ships them disabled and says so in its own feature list, and it is the
/// right way round for something that counts what a person types: a feature like this should be
/// asked for rather than opted out of. A document written before this field existed keeps
/// whatever it says; only a fresh profile gets the default.
fn enabled_by_default() -> bool {
    false
}

/// How long recorded days are kept.
///
/// Copied from the Windows baseline's `[statistics] retention`, including that an unrecognised
/// value is read as `Forever`: a preference this side does not understand must not be taken as
/// permission to delete anything.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StatisticsRetention {
    #[default]
    #[serde(rename = "forever")]
    Forever,
    #[serde(rename = "30d")]
    Days30,
    #[serde(rename = "90d")]
    Days90,
    #[serde(rename = "180d")]
    Days180,
    #[serde(rename = "365d")]
    Days365,
}

impl StatisticsRetention {
    /// The window in days, or `None` for "keep everything".
    pub fn days(self) -> Option<u32> {
        match self {
            Self::Forever => None,
            Self::Days30 => Some(30),
            Self::Days90 => Some(90),
            Self::Days180 => Some(180),
            Self::Days365 => Some(365),
        }
    }

    /// Parse the stored spelling. Anything else is `Forever`, never a shorter window.
    pub fn parse(value: &str) -> Self {
        match value {
            "30d" => Self::Days30,
            "90d" => Self::Days90,
            "180d" => Self::Days180,
            "365d" => Self::Days365,
            _ => Self::Forever,
        }
    }
}

/// An unknown retention value is `Forever` rather than a parse failure: a damaged or newer
/// preference must not make the whole document unreadable, and must never delete more.
fn retention_or_forever<'de, D>(deserializer: D) -> Result<StatisticsRetention, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer).unwrap_or_default();
    Ok(StatisticsRetention::parse(&value))
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TypingSource {
    Quanpin,
    NineKey,
    /// 全拼 14 键。
    FourteenKey,
    /// 全拼十八键，Android、iOS 和鸿蒙的触屏键盘都记在这里。
    EighteenKey,
    Shuangpin,
    Ziranma,
    Microsoft,
    Shoudao,
    #[serde(rename = "smart_abc")]
    SmartAbc,
    Wubi,
    Japanese,
    Korean,
    Cantonese,
    Zhuyin,
    Vietnamese,
    Tibetan,
    Stroke,
    Handwriting,
    English,
    Local,
    Ai,
    Reply,
    Voice,
    Unknown,
}

impl TypingSource {
    fn id(self) -> &'static str {
        match self {
            Self::Quanpin => "quanpin",
            Self::NineKey => "nineKey",
            Self::FourteenKey => "fourteenKey",
            Self::EighteenKey => "eighteenKey",
            Self::Shuangpin => "shuangpin",
            Self::Ziranma => "ziranma",
            Self::Microsoft => "microsoft",
            Self::Shoudao => "shoudao",
            Self::SmartAbc => "smart_abc",
            Self::Wubi => "wubi",
            Self::Japanese => "japanese",
            Self::Korean => "korean",
            Self::Cantonese => "cantonese",
            Self::Zhuyin => "zhuyin",
            Self::Vietnamese => "vietnamese",
            Self::Tibetan => "tibetan",
            Self::Stroke => "stroke",
            Self::Handwriting => "handwriting",
            Self::English => "english",
            Self::Local => "local",
            Self::Ai => "ai",
            Self::Reply => "reply",
            Self::Voice => "voice",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TypingBreakdown {
    #[serde(default)]
    pub characters: BTreeMap<String, u64>,
    #[serde(default)]
    pub sources: BTreeMap<String, u64>,
}

impl TypingBreakdown {
    fn add(&mut self, character: &str, source: TypingSource) -> Result<(), TypingStatisticsError> {
        checked_increment(&mut self.characters, character, 1)?;
        checked_increment(&mut self.sources, source.id(), 1)
    }

    fn merge(&mut self, other: &Self) -> Result<(), TypingStatisticsError> {
        for (key, count) in &other.characters {
            checked_increment(&mut self.characters, key, *count)?;
        }
        for (key, count) in &other.sources {
            checked_increment(&mut self.sources, key, *count)?;
        }
        Ok(())
    }

    pub fn including_unclassified(&self, total: u64) -> Self {
        let mut value = self.clone();
        let character_total = value
            .characters
            .values()
            .fold(0_u64, |sum, count| sum.saturating_add(*count));
        let source_total = value
            .sources
            .values()
            .fold(0_u64, |sum, count| sum.saturating_add(*count));
        *value.characters.entry("unknown".to_owned()).or_default() +=
            total.saturating_sub(character_total);
        *value.sources.entry("unknown".to_owned()).or_default() +=
            total.saturating_sub(source_total);
        value
    }
}

/// Where in the candidate list a commit came from, counted and nothing else.
///
/// This is the field counterpart of the evaluation sets' top-1: `ranks[0]` over the total is how
/// often the first candidate was the one wanted, measured on what the user actually types rather
/// than on 60 hand-written sentences. No text, no pinyin and no context are involved, which is
/// what makes it safe to keep — the same rule the rest of this module follows.
///
/// A candidate page holds nine, so ranks past that are counted together: beyond the first page the
/// distinction between the eleventh and the twelfth says nothing anyone would act on.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SelectionCounts {
    /// Commits from positions 1 through `RANKS`, `ranks[0]` being the first candidate.
    #[serde(default)]
    pub ranks: Vec<u64>,
    /// Commits from further down the list than `RANKS`.
    #[serde(default)]
    pub beyond: u64,
}

/// One candidate page. Positions past this are counted in `beyond`.
pub const RANKS: usize = 9;

impl SelectionCounts {
    fn add(&mut self, position: usize, count: u64) -> Result<(), TypingStatisticsError> {
        if position == 0 {
            return Err(TypingStatisticsError::InvalidPosition);
        }
        if position > RANKS {
            self.beyond = self
                .beyond
                .checked_add(count)
                .filter(|count| *count <= MAX_COUNT)
                .ok_or(TypingStatisticsError::CountExhausted)?;
            return Ok(());
        }
        if self.ranks.len() < RANKS {
            self.ranks.resize(RANKS, 0);
        }
        let slot = &mut self.ranks[position - 1];
        *slot = slot
            .checked_add(count)
            .filter(|count| *count <= MAX_COUNT)
            .ok_or(TypingStatisticsError::CountExhausted)?;
        Ok(())
    }

    /// 把 `other` 的每个位置加进来：整批加上或报错，报错时 `self` 可能已加了一部分，调用方丢掉它。
    fn merge(&mut self, other: &Self) -> Result<(), TypingStatisticsError> {
        for (slot, count) in other.ranks.iter().enumerate() {
            if *count > 0 {
                self.add(slot + 1, *count)?;
            }
        }
        if other.beyond > 0 {
            self.add(RANKS + 1, other.beyond)?;
        }
        Ok(())
    }

    /// Commits counted here, which is the denominator for any rate drawn from `ranks`.
    pub fn total(&self) -> u64 {
        self.ranks
            .iter()
            .fold(self.beyond, |sum, count| sum.saturating_add(*count))
    }
}

/// 候选上屏的效率计数，只有累计值，不分天。
///
/// 和 `selections` 一样由 host-api 在派发选择时计数、批量写入：host-api 拿不到宿主的本地日期，所以这里也没有日期轴。只有次数和按键数，不含任何输入码或文字。
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitEfficiency {
    /// 计入的上屏次数，是 `sentence_commits` 和 `prediction_commits` 的分母。
    #[serde(default)]
    pub commits: u64,
    /// 产生这些候选的输入码里实际按下的字母和数字个数。
    #[serde(default)]
    pub typed_keys: u64,
    /// 同样的上屏文字用全拼打出来需要的按键数，`1 - typed_keys / spelled_keys` 就是少按的比例。
    #[serde(default)]
    pub spelled_keys: u64,
    /// 来自整句（生成、兜底、神经网络或在线候选）的上屏次数。
    #[serde(default)]
    pub sentence_commits: u64,
    /// 没有组合串、直接从联想候选上屏的次数。
    #[serde(default)]
    pub prediction_commits: u64,
}

impl CommitEfficiency {
    /// 在内存里累加一次上屏，供宿主攒批时使用；饱和在 `MAX_COUNT`，不会因为计数溢出丢掉整批。
    pub fn count_commit(
        &mut self,
        typed_keys: u64,
        spelled_keys: u64,
        sentence: bool,
        prediction: bool,
    ) {
        let add =
            |value: &mut u64, amount: u64| *value = value.saturating_add(amount).min(MAX_COUNT);
        add(&mut self.commits, 1);
        add(&mut self.typed_keys, typed_keys);
        add(&mut self.spelled_keys, spelled_keys);
        add(&mut self.sentence_commits, u64::from(sentence));
        add(&mut self.prediction_commits, u64::from(prediction));
    }

    /// 这一批是否什么都没有计到；空批不碰磁盘。
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// 把另一批计数并进来，整批成功或整批不变：任何一项超过 `MAX_COUNT` 都拒绝。
    pub fn merge(&mut self, other: &Self) -> Result<(), TypingStatisticsError> {
        let add = |value: u64, amount: u64| {
            value
                .checked_add(amount)
                .filter(|sum| *sum <= MAX_COUNT)
                .ok_or(TypingStatisticsError::CountExhausted)
        };
        let merged = Self {
            commits: add(self.commits, other.commits)?,
            typed_keys: add(self.typed_keys, other.typed_keys)?,
            spelled_keys: add(self.spelled_keys, other.spelled_keys)?,
            sentence_commits: add(self.sentence_commits, other.sentence_commits)?,
            prediction_commits: add(self.prediction_commits, other.prediction_commits)?,
        };
        if !merged.is_consistent() {
            return Err(TypingStatisticsError::InvalidDocument);
        }
        *self = merged;
        Ok(())
    }

    /// 整句和联想都是上屏的一种，次数不可能超过上屏总数。
    fn is_consistent(&self) -> bool {
        [
            self.commits,
            self.typed_keys,
            self.spelled_keys,
            self.sentence_commits,
            self.prediction_commits,
        ]
        .iter()
        .all(|count| *count <= MAX_COUNT)
            && self.sentence_commits <= self.commits
            && self.prediction_commits <= self.commits
    }
}

/// 一段不停顿的连续输入：相邻两次上屏的间隔都在活跃间隔以内。
///
/// `day` 是这段输入开始的那一天；跨过午夜的一段仍是同一段，记在开始的那天。
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypingRun {
    #[serde(default)]
    pub characters: u64,
    #[serde(default)]
    pub day: String,
}

impl TypingRun {
    /// 还没有任何一段连续输入。
    pub fn is_empty(&self) -> bool {
        self.characters == 0 && self.day.is_empty()
    }

    fn validate(&self) -> Result<(), TypingStatisticsError> {
        if self.characters > MAX_COUNT
            || (self.characters > 0 && !crate::calendar::is_valid_day(&self.day))
            || (self.characters == 0 && !self.day.is_empty())
        {
            return Err(TypingStatisticsError::InvalidDocument);
        }
        Ok(())
    }
}

/// `skins_tried` 接受的皮肤 ID：内置和外部皮肤的 `safe_id`，或者社区包的 UUID。
pub fn is_valid_skin_id(id: &str) -> bool {
    crate::skin::catalog::safe_id(id) || (id.len() == 36 && uuid::Uuid::try_parse(id).is_ok())
}

/// Every key id `dailyKeys` may hold, and the only ones `record_keys` accepts.
///
/// W3C `KeyboardEvent.code` names for physical keys, so every desktop host and the settings page agree on one spelling without a translation table, plus the on-screen keys a soft keyboard has and a physical one does not. A soft 26-key letter is `KeyA`..`KeyZ`, its space, return, backspace and shift are `Space`, `Enter`, `Backspace` and `ShiftLeft`, and a symbol-layer key is the ANSI key that types that character. A key with no entry here is not counted at all: an id invented by one host would be a key no other host or page can draw, and a closed list is also what keeps a free-form string, and with it anything typed, out of the file.
pub const KEY_IDS: &[&str] = &[
    // Letters.
    "KeyA",
    "KeyB",
    "KeyC",
    "KeyD",
    "KeyE",
    "KeyF",
    "KeyG",
    "KeyH",
    "KeyI",
    "KeyJ",
    "KeyK",
    "KeyL",
    "KeyM",
    "KeyN",
    "KeyO",
    "KeyP",
    "KeyQ",
    "KeyR",
    "KeyS",
    "KeyT",
    "KeyU",
    "KeyV",
    "KeyW",
    "KeyX",
    "KeyY",
    "KeyZ",
    // Digit row.
    "Digit0",
    "Digit1",
    "Digit2",
    "Digit3",
    "Digit4",
    "Digit5",
    "Digit6",
    "Digit7",
    "Digit8",
    "Digit9",
    // ANSI punctuation.
    "Backquote",
    "Minus",
    "Equal",
    "BracketLeft",
    "BracketRight",
    "Backslash",
    "Semicolon",
    "Quote",
    "Comma",
    "Period",
    "Slash",
    // International layouts (ISO, JIS, Korean).
    "IntlBackslash",
    "IntlRo",
    "IntlYen",
    "Lang1",
    "Lang2",
    "Convert",
    "NonConvert",
    "KanaMode",
    // Editing and whitespace.
    "Space",
    "Enter",
    "Backspace",
    "Tab",
    "Escape",
    "Delete",
    "Insert",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    // Modifiers.
    "CapsLock",
    "ShiftLeft",
    "ShiftRight",
    "ControlLeft",
    "ControlRight",
    "AltLeft",
    "AltRight",
    "MetaLeft",
    "MetaRight",
    "Fn",
    "ContextMenu",
    // Function row.
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
    // Numeric keypad.
    "Numpad0",
    "Numpad1",
    "Numpad2",
    "Numpad3",
    "Numpad4",
    "Numpad5",
    "Numpad6",
    "Numpad7",
    "Numpad8",
    "Numpad9",
    "NumpadDecimal",
    "NumpadEnter",
    "NumpadAdd",
    "NumpadSubtract",
    "NumpadMultiply",
    "NumpadDivide",
    "NumLock",
    // On-screen keyboards only: the nine-key grid cells named by the digit printed on them (`Nine1` is the punctuation and separator cell), the nine-key side-column punctuation keys, and the symbol, layer, language, globe, emoji and voice keys.
    "Nine0",
    "Nine1",
    "Nine2",
    "Nine3",
    "Nine4",
    "Nine5",
    "Nine6",
    "Nine7",
    "Nine8",
    "Nine9",
    "SoftPunctuation",
    "SoftSymbol",
    "SoftLayer",
    "SoftLanguage",
    "SoftGlobe",
    "SoftEmoji",
    "SoftVoice",
    // 屏幕键盘专有：全拼 14 键的各键，按键面上的字母命名（`FourteenQW` 是 Q、W 合在一起的键）。不记到 `KeyQ` 这些 26 键字母上，否则会混进 26 键的热力图。
    "FourteenQW",
    "FourteenER",
    "FourteenTY",
    "FourteenUI",
    "FourteenOP",
    "FourteenAS",
    "FourteenDF",
    "FourteenGH",
    "FourteenJK",
    "FourteenL",
    "FourteenZX",
    "FourteenCV",
    "FourteenBN",
    "FourteenM",
    // 十八键共用引擎分组，保留独立的键位统计。
    "EighteenQ",
    "EighteenWE",
    "EighteenRT",
    "EighteenY",
    "EighteenU",
    "EighteenIO",
    "EighteenP",
    "EighteenA",
    "EighteenSD",
    "EighteenFG",
    "EighteenH",
    "EighteenJK",
    "EighteenL",
    "EighteenZ",
    "EighteenXC",
    "EighteenV",
    "EighteenBN",
    "EighteenM",
];

/// Whether `id` is one of [`KEY_IDS`].
///
/// A set rather than a scan of the list, because `validate` runs this for every key of every retained day on every read, and `Forever` retains years of them.
pub fn is_known_key_id(id: &str) -> bool {
    static KNOWN: OnceLock<HashSet<&'static str>> = OnceLock::new();
    KNOWN
        .get_or_init(|| KEY_IDS.iter().copied().collect())
        .contains(id)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypingStatistics {
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub days: BTreeMap<String, u64>,
    #[serde(default)]
    pub detail: TypingBreakdown,
    #[serde(default)]
    pub daily_details: BTreeMap<String, TypingBreakdown>,
    /// Absent from files written before this existed, which `default` turns into an empty
    /// histogram rather than a parse failure.
    ///
    /// Aggregate only, with no per-day axis, and that is a decision rather than an omission. A
    /// day here would have to be the user's day to sit beside `daily_details`, and the host is
    /// the only thing that knows which day that is — `record` takes one as an argument for
    /// exactly that reason. Candidate selection reaches this crate through a call that carries no
    /// day and would need six platform signatures changed to carry one, and a UTC day quietly
    /// disagreeing with the local day next to it is worse than no axis at all. The rate this
    /// exists to give — how often the first candidate was the right one — does not need one.
    #[serde(default)]
    pub selections: SelectionCounts,
    /// Active typing time per day, in milliseconds.
    ///
    /// Active means the gap to the previous commit was positive and no longer than
    /// [`ACTIVE_GAP_LIMIT_MS`]; anything longer is a break and contributes nothing. It exists to
    /// be a denominator: characters alone say how much was typed, not how fast, and wall-clock
    /// time between the first and last commit of a day would divide by the whole working day.
    ///
    /// A day absent here has no measured active time, which is not the same as zero characters —
    /// documents written before this existed have counts for their days and no entry here, and
    /// every metric derived from it has to treat that as "unknown" rather than "instant".
    #[serde(default)]
    pub daily_active_ms: BTreeMap<String, u64>,
    /// Characters per local hour, [`HOURS`] buckets per day.
    ///
    /// The hour comes from the host for the same reason the day does: only the host knows which
    /// timezone the user is in, and an hour axis quietly disagreeing with the day beside it would
    /// be worse than no axis. A host that does not send one still records characters; its days
    /// simply have no hourly breakdown.
    #[serde(default)]
    pub daily_hours: BTreeMap<String, Vec<u64>>,
    /// Milliseconds since the Unix epoch of the last counted commit.
    ///
    /// State, not history: every commit overwrites it, so it says when typing last happened and
    /// nothing about what was typed or when anything before it was. It has to be in the file
    /// because the store is constructed per call and has nowhere else to keep the previous
    /// commit's instant, which is the only thing the gap can be measured against.
    #[serde(default)]
    pub last_commit_ms: u64,
    /// Presses per key per local day: day, then a [`KEY_IDS`] entry, then how many times that key went down that day.
    ///
    /// Counts and nothing else. There is no hour, no order and no pairing of keys, because any of those would start to say what was typed rather than how hard each key works; a day of counts per key says only the latter. The day comes from the host for the same reason `record` takes one, and it is the day the presses happened on, not the day the host got round to flushing them.
    ///
    /// Not tied to `days`: keys are pressed on days that commit nothing (navigation, deleting, typing into an app with the input method in English), so a day may appear here and nowhere else. Absent from files written before this existed, which `default` reads as no key history.
    #[serde(default)]
    pub daily_keys: BTreeMap<String, BTreeMap<String, u64>>,
    /// How long recorded days are kept.
    #[serde(default, deserialize_with = "retention_or_forever")]
    pub retention: StatisticsRetention,
    /// The last day the retention window was applied.
    ///
    /// The baseline prunes on the first write of each day rather than on every write, so this is
    /// what "first" is measured against. It is a day key, not a clock reading.
    #[serde(default)]
    pub last_pruned_day: String,
    /// 上屏效率的累计计数，见 [`CommitEfficiency`]。旧文件没有这一项，读成全零。下面六项只有 Android 写入；为空时不序列化，其他宿主写出的文件与加这几项之前相同。
    #[serde(default, skip_serializing_if = "CommitEfficiency::is_empty")]
    pub efficiency: CommitEfficiency,
    /// 正在进行的这段连续输入，由 `record_at` 按活跃间隔累加或重新开始（只在 [`TRACKS_TYPING_RUNS`] 时）。
    #[serde(default, skip_serializing_if = "TypingRun::is_empty")]
    pub current_run: TypingRun,
    /// 单次最长的连续输入；并列时保留先出现的那段。
    #[serde(default, skip_serializing_if = "TypingRun::is_empty")]
    pub longest_run: TypingRun,
    /// 每个本地日的语音输入时长，单位毫秒，由 `record_voice` 写入。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub daily_voice_ms: BTreeMap<String, u64>,
    /// 用过的皮肤 ID，最多 [`MAX_SKINS_TRIED`] 个，由 `record_skin` 写入。
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub skins_tried: BTreeSet<String>,
    /// 已解锁的徽章 ID 和解锁那天。只增不减：保留期删掉旧数据以后徽章也不会重新锁上；只有 reset 会清空。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub achievements: BTreeMap<String, String>,
    /// 清空代号：每次 [`TypingStatisticsStore::reset`] 换一个随机值，旧文件和从没清空过的文件读成 0（不序列化）。
    ///
    /// 只给攒批用：进程攒着的上屏记下开始攒时文件的代号，写出时代号变了，说明这期间别的进程清空过统计，这批上屏是清空之前打的，丢掉，不写回清空后的文件。用随机值而不是计数，是因为清空次数也是在说用户，清空要清得什么都不剩。
    #[serde(default, skip_serializing_if = "is_zero")]
    pub generation: u64,
}

impl Default for TypingStatistics {
    fn default() -> Self {
        Self {
            // Same answer as the serde default, and it has to be: this is what a missing file
            // returns, which is exactly the fresh profile the default is about.
            enabled: enabled_by_default(),
            total: 0,
            days: BTreeMap::new(),
            detail: TypingBreakdown::default(),
            daily_details: BTreeMap::new(),
            selections: SelectionCounts::default(),
            daily_active_ms: BTreeMap::new(),
            daily_hours: BTreeMap::new(),
            daily_keys: BTreeMap::new(),
            last_commit_ms: 0,
            retention: StatisticsRetention::Forever,
            last_pruned_day: String::new(),
            efficiency: CommitEfficiency::default(),
            current_run: TypingRun::default(),
            longest_run: TypingRun::default(),
            daily_voice_ms: BTreeMap::new(),
            skins_tried: BTreeSet::new(),
            achievements: BTreeMap::new(),
            generation: 0,
        }
    }
}

impl TypingStatistics {
    pub fn breakdown(&self, days: Option<&[String]>) -> TypingBreakdown {
        let Some(days) = days else {
            return self.detail.including_unclassified(self.total);
        };
        let mut result = TypingBreakdown::default();
        let mut total = 0_u64;
        for day in days {
            total = total.saturating_add(self.days.get(day).copied().unwrap_or(0));
            if let Some(detail) = self.daily_details.get(day) {
                let _ = result.merge(detail);
            }
        }
        result.including_unclassified(total)
    }

    fn validate(&self) -> Result<(), TypingStatisticsError> {
        // No cap on the number of days: `Forever` keeps every day, as the baseline's stats_daily does, and the document size limit in `read_locked` is what bounds a file.
        if self.total > MAX_COUNT {
            return Err(TypingStatisticsError::InvalidDocument);
        }
        // A legacy document may keep a running total for days it no longer lists, so the
        // retained days can add up to less than `total`. They can never add up to more: every
        // recorded character increments both counters, and accepting the inverse would make a
        // daily view report more characters than the aggregate it belongs to.
        if self
            .days
            .values()
            .try_fold(0_u64, |sum, count| sum.checked_add(*count))
            .is_none_or(|sum| sum > self.total)
        {
            return Err(TypingStatisticsError::InvalidDocument);
        }
        if self.selections.ranks.len() > RANKS
            || self.selections.beyond > MAX_COUNT
            || self.selections.ranks.iter().any(|count| *count > MAX_COUNT)
            || self
                .selections
                .ranks
                .iter()
                .try_fold(self.selections.beyond, |sum, count| sum.checked_add(*count))
                .is_none_or(|sum| sum > MAX_COUNT)
        {
            return Err(TypingStatisticsError::InvalidDocument);
        }
        validate_counts(&self.detail, self.total)?;
        for (day, count) in &self.days {
            validate_day(day)?;
            if *count > self.total {
                return Err(TypingStatisticsError::InvalidDocument);
            }
            if let Some(detail) = self.daily_details.get(day) {
                validate_counts(detail, *count)?;
            }
        }
        if self
            .daily_details
            .keys()
            .any(|day| !self.days.contains_key(day))
        {
            return Err(TypingStatisticsError::InvalidDocument);
        }
        for (day, active_ms) in &self.daily_active_ms {
            // A day that has active time but no characters is not a document this code can
            // produce, and letting it through would put a day on the calendar that nobody typed
            // on.
            if !self.days.contains_key(day) || *active_ms > MAX_ACTIVE_MS_PER_DAY {
                return Err(TypingStatisticsError::InvalidDocument);
            }
        }
        for (day, hours) in &self.daily_hours {
            let Some(total) = self.days.get(day) else {
                return Err(TypingStatisticsError::InvalidDocument);
            };
            // Not equality: days recorded before hosts sent an hour have characters and no
            // buckets, so the buckets can only ever be a subset of the day.
            if hours.len() != HOURS
                || hours
                    .iter()
                    .try_fold(0_u64, |sum, count| sum.checked_add(*count))
                    .is_none_or(|sum| sum > *total)
            {
                return Err(TypingStatisticsError::InvalidDocument);
            }
        }
        // Checked against the calendar rather than against `days`, which a key-only day is legitimately missing from.
        for (day, keys) in &self.daily_keys {
            validate_day(day).map_err(|_| TypingStatisticsError::InvalidDocument)?;
            validate_key_counts(keys)?;
        }
        if !self.efficiency.is_consistent() {
            return Err(TypingStatisticsError::InvalidDocument);
        }
        self.current_run.validate()?;
        self.longest_run.validate()?;
        for (day, milliseconds) in &self.daily_voice_ms {
            // 语音时长和字数无关：识别结果可能被用户删掉不上屏，所以只按日历校验日期，不要求这天在 `days` 里。
            if !crate::calendar::is_valid_day(day) || *milliseconds > MAX_ACTIVE_MS_PER_DAY {
                return Err(TypingStatisticsError::InvalidDocument);
            }
        }
        if self.skins_tried.len() > MAX_SKINS_TRIED
            || self.skins_tried.iter().any(|id| !is_valid_skin_id(id))
        {
            return Err(TypingStatisticsError::InvalidDocument);
        }
        if self
            .achievements
            .iter()
            .any(|(id, day)| !metrics::is_achievement_id(id) || !crate::calendar::is_valid_day(day))
        {
            return Err(TypingStatisticsError::InvalidDocument);
        }
        Ok(())
    }

    /// Drop every recorded day outside the retention window, counting back from `today`.
    ///
    /// The comparison is on the day key, which sorts as a date because it is `YYYY-MM-DD`; no
    /// calendar arithmetic is needed beyond producing the boundary. `Forever` removes nothing,
    /// and a day in the future - a clock that was wrong when it was recorded - is kept rather
    /// than silently deleted, because the alternative is losing real typing to a bad clock.
    pub fn apply_retention(&mut self, today: &str) {
        let Some(days) = self.retention.days() else {
            return;
        };
        let Some(boundary) = crate::calendar::shift_day(today, -i64::from(days)) else {
            return;
        };
        // Like the baseline's ClearThrough, which deletes the stats_daily rows its overview sums, a cleanup takes the pruned days out of the running totals too, so "累计", the category split and the daily average cover the retained window. A legacy day without a breakdown only lowers `total`; `breakdown(None)` reports the rest as unclassified.
        for (_, count) in self.days.range(..boundary.clone()) {
            self.total = self.total.saturating_sub(*count);
        }
        for (_, detail) in self.daily_details.range(..boundary.clone()) {
            for (key, count) in &detail.characters {
                if let Some(value) = self.detail.characters.get_mut(key) {
                    *value = value.saturating_sub(*count);
                }
            }
            for (key, count) in &detail.sources {
                if let Some(value) = self.detail.sources.get_mut(key) {
                    *value = value.saturating_sub(*count);
                }
            }
        }
        self.days.retain(|day, _| *day >= boundary);
        self.daily_details.retain(|day, _| *day >= boundary);
        self.daily_active_ms.retain(|day, _| *day >= boundary);
        self.daily_hours.retain(|day, _| *day >= boundary);
        self.daily_keys.retain(|day, _| *day >= boundary);
        self.daily_voice_ms.retain(|day, _| *day >= boundary);
        // Subtraction keeps whatever `total` and `detail` hold beyond the per-day records, which a document written by an older build can have. Where that leaves the counters out of step with each other - `total` below the retained days, or a category sum above `total` - validate() would reject the document this write produces, so fall back to what the retained days themselves say.
        let retained = self
            .days
            .values()
            .fold(0_u64, |sum, count| sum.saturating_add(*count));
        self.total = self.total.max(retained);
        let sum = |values: &BTreeMap<String, u64>| {
            values
                .values()
                .fold(0_u64, |sum, count| sum.saturating_add(*count))
        };
        if sum(&self.detail.characters) > self.total || sum(&self.detail.sources) > self.total {
            let mut rebuilt = TypingBreakdown::default();
            for detail in self.daily_details.values() {
                let _ = rebuilt.merge(detail);
            }
            self.detail = rebuilt;
        }
    }

    /// Active milliseconds recorded for `day`, or `None` when that day predates the measurement.
    pub fn active_ms(&self, day: &str) -> Option<u64> {
        self.daily_active_ms.get(day).copied()
    }

    /// The day's per-hour character counts, or `None` when the host sent no hour for it.
    pub fn hours(&self, day: &str) -> Option<&[u64]> {
        self.daily_hours.get(day).map(Vec::as_slice)
    }

    /// 记一次上屏：字数、分类、到这次的间隔算出的活跃时长、连续输入、小时分布，每天第一次写入时执行保留期。攒批写出时按到达顺序逐条调用，结果与逐条写入相同。
    fn apply_commit(&mut self, commit: &PendingCommit) -> Result<(), TypingStatisticsError> {
        let (addition, count, day, now_ms) = (
            &commit.addition,
            commit.count,
            commit.day.as_str(),
            commit.now_ms,
        );
        self.total = self
            .total
            .checked_add(count)
            .filter(|total| *total <= MAX_COUNT)
            .ok_or(TypingStatisticsError::CountExhausted)?;
        checked_increment(&mut self.days, day, count)?;
        self.detail.merge(addition)?;
        self.daily_details
            .entry(day.to_owned())
            .or_default()
            .merge(addition)?;

        // Attribute the gap to the day and hour of *this* commit, the way the Windows baseline
        // does: the pause belongs to the typing it precedes, and a session that crosses midnight
        // therefore leaves its last pause on the new day rather than extending the old one.
        let active_ms = active_gap_ms(self.last_commit_ms, now_ms);
        if active_ms > 0 {
            let day_active = self.daily_active_ms.entry(day.to_owned()).or_default();
            *day_active = day_active
                .saturating_add(active_ms)
                .min(MAX_ACTIVE_MS_PER_DAY);
        }
        // 连续输入：间隔算作活跃时就接着上一段累加，否则从这次上屏重新开始一段，开始日是这次上屏的日期。只有 Android 的统计页显示它。
        if TRACKS_TYPING_RUNS {
            if active_ms > 0 && self.current_run.characters > 0 {
                self.current_run.characters = self
                    .current_run
                    .characters
                    .saturating_add(count)
                    .min(MAX_COUNT);
            } else {
                self.current_run = TypingRun {
                    characters: count,
                    day: day.to_owned(),
                };
            }
            if self.current_run.characters > self.longest_run.characters {
                self.longest_run = self.current_run.clone();
            }
        }
        // Never moves backwards. A clock set back would otherwise make every later commit look
        // like it followed a huge pause, and the first one after the correction would be counted
        // as a fresh session instead of the continuation it is.
        if now_ms > self.last_commit_ms {
            self.last_commit_ms = now_ms;
        }

        if let Some(hour) = commit.hour.filter(|hour| usize::from(*hour) < HOURS) {
            let buckets = self
                .daily_hours
                .entry(day.to_owned())
                .or_insert_with(|| vec![0; HOURS]);
            // A file edited by hand could carry a short vector; resize rather than panic on the
            // index, because a malformed bucket list must not cost the user the count itself.
            if buckets.len() != HOURS {
                buckets.resize(HOURS, 0);
            }
            let bucket = &mut buckets[usize::from(hour)];
            *bucket = bucket
                .checked_add(count)
                .filter(|count| *count <= MAX_COUNT)
                .ok_or(TypingStatisticsError::CountExhausted)?;
        }

        // The retention setting is the only thing that deletes days, matching the baseline's
        // RetentionCutoff/ClearThrough: `Forever` keeps every one, and `MAX_DOCUMENT_BYTES` is what
        // bounds the file. It runs on the first write of each day; doing it on every write would
        // read the whole history on every commit.
        if self.last_pruned_day != day {
            self.apply_retention(day);
            self.last_pruned_day = day.to_owned();
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TypingStatisticsError {
    #[error("typing statistics storage failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid typing statistics document: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid typing statistics day")]
    InvalidDay,
    #[error("typing statistics commit is too large")]
    CommitTooLarge,
    #[error("typing statistics document is invalid")]
    InvalidDocument,
    #[error("typing statistics count exhausted")]
    CountExhausted,
    #[error("candidate position is not one-based")]
    InvalidPosition,
    #[error("typing statistics key id is unknown or its count is zero")]
    InvalidKey,
    #[error("typing statistics voice duration is zero or longer than one call may record")]
    InvalidVoiceDuration,
    #[error("typing statistics skin id is not a skin id")]
    InvalidSkinId,
}

/// 设置页读到的统计状态：统计本身，加上统计文件写过没有、最后一次写在什么时候。
///
/// 共享设置页（`typing-statistics.tsx` 的 `TypingStatisticsStatus`）靠后两项说明「统计没有数据」的原因：文件从未写过是键盘还没记过，写过却是零多半是刚清空。Tauri 外壳和 Windows 原生设置窗口都由 [`TypingStatisticsStore::status`] 给出它，字段名与页面读的相同。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypingStatisticsStatus {
    pub statistics: TypingStatistics,
    /// `ready`：统计文件存在；`neverWritten`：还没有写过。
    pub availability: &'static str,
    /// 统计文件的修改时间，Unix 毫秒；文件不存在或读不到修改时间时为 `None`。
    pub last_written_ms: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct TypingStatisticsStore {
    directory: PathBuf,
    upload_operation: Arc<Mutex<()>>,
}

/// 一次攒着还没写进文件的上屏：只有分类计数、日期、小时和时刻，不留原文。
struct PendingCommit {
    addition: TypingBreakdown,
    count: u64,
    day: String,
    hour: Option<u8>,
    now_ms: u64,
}

/// 本进程为一个统计目录攒着的上屏，和上次读写那个文件时看到的状态。
struct PendingCommits {
    /// 上次读写文件时的统计开关。关着时 `record` 不攒，直接答 0；满一个间隔后重读一次文件。
    enabled: bool,
    /// 上次读写文件的时刻（`record_at` 的时钟），间隔从这里算。
    since_ms: u64,
    /// 文件里的累计字数加上 `commits` 的字数，成就里程碑据此判断，不必为此另读一遍文件。
    total: u64,
    /// 上次读写文件时它的清空代号（[`TypingStatistics::generation`]）。写出时文件的代号不同，就是别的进程在这期间清空过，`commits` 整批丢掉。
    generation: u64,
    commits: Vec<PendingCommit>,
}

impl PendingCommits {
    /// 该写了：攒满上限，或距上次读写满一个间隔。时钟往回拨也算满，免得拨回去以后一直不写。
    fn due(&self, now_ms: u64) -> bool {
        self.commits.len() >= MAX_PENDING_COMMITS
            || now_ms.abs_diff(self.since_ms) >= COMMIT_FLUSH_INTERVAL_MS
    }
}

/// 按目录攒着的上屏，整个进程一份：同一进程里不同的 store 实例（每次 FFI 调用都新建一个）看到同一批，读这个目录的任何操作都先把它写进去。
///
/// 锁的顺序固定为先这把互斥锁、再统计文件锁，持有文件锁时不再取这把锁，所以不会互相等死。`record` 写文件时一直拿着它，同一进程里的上屏因此按到达的顺序写进文件，活跃时长和连续输入的间隔不会被打乱。
static PENDING_COMMITS: Mutex<BTreeMap<PathBuf, PendingCommits>> = Mutex::new(BTreeMap::new());

fn pending_commits() -> MutexGuard<'static, BTreeMap<PathBuf, PendingCommits>> {
    PENDING_COMMITS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 本进程为一个统计目录攒着的选词位置，和本进程上次读写那个文件时看到的统计开关与清空代号。
#[derive(Default)]
struct PendingSelections {
    /// 本进程上次读到或写下统计文件时它的 `(开关, 清空代号)`；本进程还没读过这个文件时为 `None`。
    file: Option<(bool, u64)>,
    /// 攒着还没写进文件的选词位置。`file` 为 `None` 时攒下的，由下一次读到的开关和代号决定去留。
    counts: SelectionCounts,
}

impl PendingSelections {
    /// 记下本进程刚读到或刚写下的文件状态。文件里的统计关着，或代号变了（别的进程或本进程清空过，攒着的选词是清空之前计的），攒着的选词整批丢掉，和攒着的上屏一样。
    fn observe(&mut self, value: &TypingStatistics) {
        if !value.enabled
            || self
                .file
                .is_some_and(|(_, generation)| generation != value.generation)
        {
            self.counts = SelectionCounts::default();
        }
        self.file = Some((value.enabled, value.generation));
    }
}

/// 按目录攒着的选词位置，整个进程一份，和 [`PENDING_COMMITS`] 一起写出（见 `write_pending_locked`）。
///
/// 选词位置在宿主的输入线程上交过来（host-api 的 `flush_selections`），所以单独一把锁：它只在加计数、取计数和记文件状态的一瞬间拿着，拿着它时不取别的锁、不读写文件。[`PENDING_COMMITS`] 在写文件时一直拿着，输入线程取它就可能等别的线程的 fsync。锁的顺序是先 [`PENDING_COMMITS`]、再文件锁、最后这把，不会互相等死。
static PENDING_SELECTIONS: Mutex<BTreeMap<PathBuf, PendingSelections>> =
    Mutex::new(BTreeMap::new());

fn pending_selections() -> MutexGuard<'static, BTreeMap<PathBuf, PendingSelections>> {
    PENDING_SELECTIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 本进程攒着选词位置的目录。
fn directories_holding_selections() -> Vec<PathBuf> {
    pending_selections()
        .iter()
        .filter(|(_, entry)| entry.counts.total() > 0)
        .map(|(directory, _)| directory.clone())
        .collect()
}

/// 把本进程为所有目录攒着的上屏和选词位置都写出去。尽力而为：写不进的那批丢掉，统计不能成为别的操作失败的原因。
///
/// 给不知道具体目录、又能在当前线程上等磁盘的收尾点用（Windows 的失焦、桌面宿主的会话销毁、`msime_client_flush_all`）；在 UI 线程上的收尾点用 [`flush_pending_commits_in_background`]。知道目录的宿主调 [`TypingStatisticsStore::flush`]。
pub fn flush_pending_commits() {
    let mut directories: BTreeSet<PathBuf> = pending_commits().keys().cloned().collect();
    directories.extend(directories_holding_selections());
    for directory in directories {
        let _ = TypingStatisticsStore::new(directory).flush();
    }
}

/// [`flush_pending_commits`]，但在新开的后台线程上写，调用方不等磁盘，也不等别的线程手里的 [`PENDING_COMMITS`]。
///
/// 给可能在 UI 线程或主线程上的收尾点用（移动宿主的失焦和会话销毁、macOS 和 Linux 的失焦）：那里不能整份读写统计文件再 fsync，也不能等统计工作线程正在做的那次写。本进程没有攒着任何目录的上屏或选词时什么都不做，不开线程；锁正被别的线程拿着时照样开线程，由它去等。开不了线程时退回到在当前线程上写：丢计数比卡一下更糟。
pub fn flush_pending_commits_in_background() {
    let no_commits_held = match PENDING_COMMITS.try_lock() {
        Ok(pending) => pending.is_empty(),
        Err(std::sync::TryLockError::Poisoned(pending)) => pending.into_inner().is_empty(),
        Err(std::sync::TryLockError::WouldBlock) => false,
    };
    if no_commits_held && directories_holding_selections().is_empty() {
        return;
    }
    if std::thread::Builder::new()
        .name("msime-statistics-flush".into())
        .spawn(flush_pending_commits)
        .is_err()
    {
        flush_pending_commits();
    }
}

struct StatisticsLock {
    directory: crate::file_lock::PrivateDirectory,
    _lock: crate::file_lock::ScopedFileLock,
}

impl TypingStatisticsStore {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            upload_operation: Arc::new(Mutex::new(())),
        }
    }

    /// Where the statistics file lives, for hosts that offer to reveal it in a file manager.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// 串行化共享此 store 的云端上传和删除操作。
    ///
    /// 文件锁保护本地状态，但不能覆盖读取和写回状态之间的网络请求。
    /// 宿主必须让这把锁覆盖完整的云端操作，避免关闭上传后的 `DELETE`
    /// 与已经发出的 `PUT` 交错。
    pub fn lock_upload_operation(&self) -> MutexGuard<'_, ()> {
        self.upload_operation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn path(&self) -> PathBuf {
        self.directory.join(STATISTICS_FILE)
    }

    /// 取统计文件锁，并先把本进程为这个目录攒着的上屏和选词位置写进文件：读这个目录的任何操作都看得到它们，改开关、改保留期和清空都排在它们之后。
    ///
    /// 攒着的那批写不进时丢掉，不让这次操作跟着失败：文件本身坏了，操作自己的读照样会报出来。写过以后状态也一并丢掉，下一次 `record` 重新读文件里的开关。
    fn lock(&self) -> Result<StatisticsLock, TypingStatisticsError> {
        let mut pending = pending_commits();
        let lock = self.lock_file()?;
        let (commits, generation) = pending
            .remove(&self.directory)
            .map_or((Vec::new(), 0), |entry| (entry.commits, entry.generation));
        if !commits.is_empty() || self.holds_selections() {
            let _ = self.write_pending_locked(&lock, &commits, generation);
        }
        Ok(lock)
    }

    /// 本进程为这个目录攒着选词位置。
    fn holds_selections(&self) -> bool {
        pending_selections()
            .get(&self.directory)
            .is_some_and(|entry| entry.counts.total() > 0)
    }

    /// 记下本进程刚读到或刚写下的文件状态，见 [`PendingSelections::observe`]。
    fn observe_file(&self, value: &TypingStatistics) {
        pending_selections()
            .entry(self.directory.clone())
            .or_default()
            .observe(value);
    }

    /// 只取统计文件锁，不碰攒着的上屏。调用方必须已经拿着 [`PENDING_COMMITS`]，见那里的锁顺序。
    fn lock_file(&self) -> Result<StatisticsLock, TypingStatisticsError> {
        if let Some(parent) = self.directory.parent() {
            crate::storage::reject_symlink(parent)?;
        }
        if !crate::storage::create_directory_and_check(&self.directory)? {
            return Err(TypingStatisticsError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "typing statistics directory is not a real directory",
            )));
        }
        let directory = crate::file_lock::open_private_directory(&self.directory)?;
        let lock = crate::file_lock::open_private_lock_file_at(
            &directory,
            std::ffi::OsStr::new(LOCK_FILE),
        )?;
        let lock = crate::file_lock::ScopedFileLock::exclusive(lock)?;
        Ok(StatisticsLock {
            directory,
            _lock: lock,
        })
    }

    fn read_locked(
        &self,
        lock: &StatisticsLock,
    ) -> Result<TypingStatistics, TypingStatisticsError> {
        let file = match crate::file_lock::open_private_file_at(
            &lock.directory,
            std::ffi::OsStr::new(STATISTICS_FILE),
        ) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(TypingStatistics::default());
            }
            Err(_) => return Err(TypingStatisticsError::InvalidDocument),
        };
        let bytes = crate::bounded_io::read_bounded_file(file, MAX_DOCUMENT_BYTES, || {
            TypingStatisticsError::InvalidDocument
        })?;
        let value: TypingStatistics = serde_json::from_slice(&bytes)?;
        value.validate()?;
        self.observe_file(&value);
        Ok(value)
    }

    fn write_locked(
        &self,
        lock: &StatisticsLock,
        value: &TypingStatistics,
    ) -> Result<(), TypingStatisticsError> {
        value.validate()?;
        let bytes = serde_json::to_vec(value)?;
        crate::file_lock::write_private_file_at(
            &lock.directory,
            std::ffi::OsStr::new(STATISTICS_FILE),
            &bytes,
        )?;
        self.observe_file(value);
        Ok(())
    }

    /// 在文件锁内重读统计，按到达的顺序把攒着的上屏逐条记进去，连同本进程攒着的选词位置，再写一次。
    ///
    /// 重读是因为别的进程（设置页）可能在这期间清空了统计、改了保留期或关掉了统计：改了保留期就合并进它写下的文件；统计关着，或文件的清空代号已经不是 `generation`（这批上屏是清空之前打的，攒批之前它们早已在文件里、随清空一起删掉），整批丢掉不写。每条上屏带着自己的时刻、日期和小时，记法与当初逐条写入完全相同。选词位置在重读之后取走，重读已按同样的规矩丢掉了不该写的那些（[`PendingSelections::observe`]）；读不出文件时它们留在内存里，等下一次写。上屏和选词都没有要写的就不写。整批成功或文件不变。返回重读（并在有要写的时合并）后的统计。
    fn write_pending_locked(
        &self,
        lock: &StatisticsLock,
        commits: &[PendingCommit],
        generation: u64,
    ) -> Result<TypingStatistics, TypingStatisticsError> {
        let mut value = self.read_locked(lock)?;
        let selections = pending_selections()
            .get_mut(&self.directory)
            .map(|entry| std::mem::take(&mut entry.counts))
            .unwrap_or_default();
        let commits = if value.enabled && value.generation == generation {
            commits
        } else {
            &[]
        };
        if commits.is_empty() && selections.total() == 0 {
            return Ok(value);
        }
        for commit in commits {
            value.apply_commit(commit)?;
        }
        value.selections.merge(&selections)?;
        self.write_locked(lock, &value)?;
        Ok(value)
    }

    /// 把本进程为这个目录攒着的上屏和选词位置写进统计文件。两样都没有攒着时不碰磁盘。
    ///
    /// 宿主在键盘收起、输入框失焦和进程或服务退出前，在它平时调统计的那个工作线程上调用。之后的 `record` 重新读文件里的开关。写不进时这一批丢掉，不留着下次重试。
    pub fn flush(&self) -> Result<(), TypingStatisticsError> {
        let mut pending = pending_commits();
        let (commits, generation) = pending
            .remove(&self.directory)
            .map_or((Vec::new(), 0), |entry| (entry.commits, entry.generation));
        if commits.is_empty() && !self.holds_selections() {
            return Ok(());
        }
        let lock = self.lock_file()?;
        self.write_pending_locked(&lock, &commits, generation)
            .map(|_| ())
    }

    pub fn load(&self) -> Result<TypingStatistics, TypingStatisticsError> {
        let lock = self.lock()?;
        self.read_locked(&lock)
    }

    pub fn last_written(&self) -> Result<Option<SystemTime>, TypingStatisticsError> {
        crate::storage::reject_symlink(&self.directory)?;
        match fs::symlink_metadata(self.path()) {
            Ok(metadata) if metadata.file_type().is_file() => Ok(metadata.modified().ok()),
            Ok(_) => Err(TypingStatisticsError::InvalidDocument),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// 把刚读到或刚写下的 `statistics` 连同统计文件的写入状态交给设置页。
    pub fn status(
        &self,
        statistics: TypingStatistics,
    ) -> Result<TypingStatisticsStatus, TypingStatisticsError> {
        let last_written = self.last_written()?;
        Ok(TypingStatisticsStatus {
            statistics,
            availability: if last_written.is_some() {
                "ready"
            } else {
                "neverWritten"
            },
            last_written_ms: last_written.and_then(|time| {
                time.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
            }),
        })
    }

    /// Count one commit's characters against `day`, and the gap since the previous commit as
    /// active time.
    ///
    /// `hour` is the local hour the commit happened in. `None` records the characters without an
    /// hourly breakdown, which is what a host that cannot resolve a local hour should send rather
    /// than guessing one.
    ///
    /// 不是每次都写文件：计数先攒在本进程里，距上次读写这个目录满 [`COMMIT_FLUSH_INTERVAL_MS`] 后的下一次上屏、[`Self::flush`] 或这个目录上的任何其他操作才把它们写进去，见 [`Self::record_with_total_at`]。
    pub fn record(
        &self,
        text: &str,
        source: TypingSource,
        day: &str,
        hour: Option<u8>,
    ) -> Result<u64, TypingStatisticsError> {
        self.record_at(text, source, day, hour, epoch_millis(SystemTime::now()))
    }

    /// `record` with the instant supplied, so the active-time rules can be tested without
    /// sleeping. Production always passes the current time.
    pub fn record_at(
        &self,
        text: &str,
        source: TypingSource,
        day: &str,
        hour: Option<u8>,
        now_ms: u64,
    ) -> Result<u64, TypingStatisticsError> {
        self.record_with_total_at(text, source, day, hour, now_ms)
            .map(|(count, _)| count)
    }

    /// `record`，另外答记完这次以后的累计字数（文件里的加上本进程攒着的），宿主据此判断这次上屏是否越过成就里程碑。
    pub fn record_with_total(
        &self,
        text: &str,
        source: TypingSource,
        day: &str,
        hour: Option<u8>,
    ) -> Result<(u64, u64), TypingStatisticsError> {
        self.record_with_total_at(text, source, day, hour, epoch_millis(SystemTime::now()))
    }

    /// 给这次上屏分类计数，攒进本进程这个目录的批次，答 `(这次记下的字数, 累计字数)`。
    ///
    /// 这个目录在本进程里还没有状态时（第一次上屏，或刚 flush、刚有别的操作写过文件）读一次文件，只为知道统计开关和累计字数，不写；统计关着时答 0、不攒，满一个间隔后再读一次开关。距上次读写满 [`COMMIT_FLUSH_INTERVAL_MS`] 或攒满上限时，连同这次和本进程攒着的选词位置一起在文件锁内合并写出，见 `write_pending_locked`；那时文件里的统计已被关掉，这次答 0。
    pub fn record_with_total_at(
        &self,
        text: &str,
        source: TypingSource,
        day: &str,
        hour: Option<u8>,
        now_ms: u64,
    ) -> Result<(u64, u64), TypingStatisticsError> {
        validate_day(day)?;
        if text.len() > MAX_COMMIT_BYTES || text.chars().count() > MAX_COMMIT_SCALARS {
            return Err(TypingStatisticsError::CommitTooLarge);
        }
        let mut addition = TypingBreakdown::default();
        let mut count = 0_u64;
        for grapheme in text.graphemes(true) {
            if grapheme.chars().all(char::is_whitespace) {
                continue;
            }
            addition.add(classify(grapheme), source)?;
            count += 1;
        }
        let mut pending = pending_commits();
        // 统计关着时每个间隔重读一次开关：设置页打开统计以后，不必等到失焦才开始计。
        if pending
            .get(&self.directory)
            .is_some_and(|entry| !entry.enabled && entry.due(now_ms))
        {
            pending.remove(&self.directory);
        }
        let entry = match pending.entry(self.directory.clone()) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                let lock = self.lock_file()?;
                let value = self.read_locked(&lock)?;
                entry.insert(PendingCommits {
                    enabled: value.enabled,
                    since_ms: now_ms,
                    total: value.total,
                    generation: value.generation,
                    commits: Vec::new(),
                })
            }
        };
        if !entry.enabled || count == 0 {
            return Ok((0, entry.total));
        }
        entry.total = entry
            .total
            .checked_add(count)
            .filter(|total| *total <= MAX_COUNT)
            .ok_or(TypingStatisticsError::CountExhausted)?;
        entry.commits.push(PendingCommit {
            addition,
            count,
            day: day.to_owned(),
            hour,
            now_ms,
        });
        if !entry.due(now_ms) {
            return Ok((count, entry.total));
        }
        let commits = std::mem::take(&mut entry.commits);
        match self
            .lock_file()
            .and_then(|lock| self.write_pending_locked(&lock, &commits, entry.generation))
        {
            Ok(value) => {
                entry.enabled = value.enabled;
                entry.since_ms = now_ms;
                entry.total = value.total;
                entry.generation = value.generation;
                Ok((if value.enabled { count } else { 0 }, value.total))
            }
            Err(error) => {
                // 这一批丢掉；下一次上屏重新读文件，文件还坏着就照旧逐次报错。
                pending.remove(&self.directory);
                Err(error)
            }
        }
    }

    /// Count one commit by the one-based position it was chosen from.
    ///
    /// Separate from `record` because the two count different things: `record` counts characters,
    /// this counts commits, and dividing one by the other would mean nothing. The enable flag and
    /// the lock are shared, so turning statistics off turns this off with them and no second
    /// switch appears in settings for a user to misread.
    pub fn record_selection(&self, position: usize) -> Result<(), TypingStatisticsError> {
        self.record_selections(&[(position, 1)]).map(|_| ())
    }

    /// Count several commits at once, each `(position, count)` pair adding `count` commits from that one-based position, under one lock, one read and at most one write.
    ///
    /// This writes on the calling thread: a full read, fsync and rename. A host handing selections over from its input thread uses [`Self::queue_selections`] instead. An empty batch touches nothing on disk. The batch is applied whole or not at all: an invalid position or an exhausted count leaves the document as it was. Statistics being off drops the batch without writing, the same answer `record_selection` gives.
    ///
    /// 返回这次读到的统计开关；批次全是零、没有读文件时为 `None`。
    pub fn record_selections(
        &self,
        selections: &[(usize, u64)],
    ) -> Result<Option<bool>, TypingStatisticsError> {
        if selections.iter().all(|(_, count)| *count == 0) {
            return Ok(None);
        }
        let lock = self.lock()?;
        let mut value = self.read_locked(&lock)?;
        if !value.enabled {
            return Ok(Some(false));
        }
        for &(position, count) in selections {
            value.selections.add(position, count)?;
        }
        self.write_locked(&lock, &value)?;
        Ok(Some(true))
    }

    /// 把一批选词位置攒进本进程这个目录的批次，和攒着的上屏一起写出；不取统计文件锁，也不碰磁盘。返回本进程所知的统计开关（[`Self::known_enabled`]）。
    ///
    /// 给在输入线程上交出选词的宿主用（host-api 每满一批、失焦和销毁会话时）：那里不能整份读写统计文件再 fsync，也不能等别的线程手里那次写。攒着的选词由写上屏批次的地方一并写出：[`Self::flush`]、[`flush_pending_commits`] 和它的后台版本、满间隔的 `record`，以及这个目录上的任何其他操作（统计页读取、上传），所以读统计的地方总看得到它们。所知的开关关着时整批丢掉不攒；写出时文件里的统计关着，或文件已被清空过（攒下时所知的清空代号与文件的不同），也整批丢掉，和攒着的上屏一样。本进程还没读过这个文件时照攒，由第一次读到的开关和代号决定去留。整批攒下或都不攒：位置是 0 或计数用尽时报错，之前攒着的不受影响。
    pub fn queue_selections(
        &self,
        selections: &[(usize, u64)],
    ) -> Result<Option<bool>, TypingStatisticsError> {
        let mut pending = pending_selections();
        let entry = pending.entry(self.directory.clone()).or_default();
        let known = entry.file.map(|(enabled, _)| enabled);
        if known == Some(false) {
            return Ok(known);
        }
        let mut counts = entry.counts.clone();
        for &(position, count) in selections {
            if count > 0 {
                counts.add(position, count)?;
            }
        }
        entry.counts = counts;
        Ok(known)
    }

    /// 本进程上次读到或写下这个目录的统计文件时看到的统计开关，不碰磁盘；本进程还没读过这个文件时为 `None`。
    ///
    /// 给不能在当前线程上读文件、又想在统计关着时少做点事的调用方（host-api 在输入线程上决定要不要攒上屏效率）。它可能落后于别的进程刚做的改动：写入时统计文件自己的开关作数。
    pub fn known_enabled(&self) -> Option<bool> {
        pending_selections()
            .get(&self.directory)
            .and_then(|entry| entry.file)
            .map(|(enabled, _)| enabled)
    }

    /// Count key presses for `day`, each entry adding `count` presses of one [`KEY_IDS`] key, under one lock, one read and at most one write. Returns how many presses were added.
    ///
    /// `day` is the local day the presses happened on. A host that batches across midnight flushes the old day's counts under the old day before it counts anything for the new one; stamping them with the day of the flush would move typing onto a day it did not happen on.
    ///
    /// The batch is applied whole or not at all. An unknown key id or a zero count rejects it before the document is opened, because either one means the host is sending something this contract does not describe, and keeping the valid part would hide that. An empty batch touches nothing on disk, and statistics being off drops the batch without writing, the same answers `record_selections` gives. Retention runs on the first write of a day, exactly as `record` does it.
    pub fn record_keys(
        &self,
        day: &str,
        keys: &BTreeMap<String, u64>,
    ) -> Result<u64, TypingStatisticsError> {
        validate_day(day)?;
        if keys
            .iter()
            .any(|(key, count)| *count == 0 || !is_known_key_id(key))
        {
            return Err(TypingStatisticsError::InvalidKey);
        }
        let presses = keys
            .values()
            .try_fold(0_u64, |sum, count| sum.checked_add(*count))
            .filter(|sum| *sum <= MAX_COUNT)
            .ok_or(TypingStatisticsError::CountExhausted)?;
        if presses == 0 {
            return Ok(0);
        }
        let lock = self.lock()?;
        let mut value = self.read_locked(&lock)?;
        if !value.enabled {
            return Ok(0);
        }
        let day_keys = value.daily_keys.entry(day.to_owned()).or_default();
        for (key, count) in keys {
            checked_increment(day_keys, key, *count)?;
        }
        if value.last_pruned_day != day {
            value.apply_retention(day);
            value.last_pruned_day = day.to_owned();
        }
        self.write_locked(&lock, &value)?;
        Ok(presses)
    }

    /// 记一次语音输入的时长，计在宿主给出的本地日 `day` 上，返回记入的毫秒数。
    ///
    /// `milliseconds` 为 0 或超过 [`MAX_VOICE_MS_PER_CALL`] 时整次拒绝，因为那说明宿主的计时出了错。统计关闭时不写，返回 0；一天累计不超过一天的毫秒数。保留期和 `record` 一样在每天第一次写入时执行。
    pub fn record_voice(&self, day: &str, milliseconds: u64) -> Result<u64, TypingStatisticsError> {
        validate_day(day)?;
        if milliseconds == 0 || milliseconds > MAX_VOICE_MS_PER_CALL {
            return Err(TypingStatisticsError::InvalidVoiceDuration);
        }
        let lock = self.lock()?;
        let mut value = self.read_locked(&lock)?;
        if !value.enabled {
            return Ok(0);
        }
        let day_voice = value.daily_voice_ms.entry(day.to_owned()).or_default();
        let before = *day_voice;
        *day_voice = day_voice
            .saturating_add(milliseconds)
            .min(MAX_ACTIVE_MS_PER_DAY);
        let added = *day_voice - before;
        if value.last_pruned_day != day {
            value.apply_retention(day);
            value.last_pruned_day = day.to_owned();
        }
        self.write_locked(&lock, &value)?;
        Ok(added)
    }

    /// 记下用过一款皮肤，返回这次是否新增了一项。
    ///
    /// ID 不是 [`is_valid_skin_id`] 认可的形状时拒绝。已经记过、已满 [`MAX_SKINS_TRIED`] 款或统计关闭时不写，返回 `false`。
    pub fn record_skin(&self, id: &str) -> Result<bool, TypingStatisticsError> {
        if !is_valid_skin_id(id) {
            return Err(TypingStatisticsError::InvalidSkinId);
        }
        let lock = self.lock()?;
        let mut value = self.read_locked(&lock)?;
        if !value.enabled
            || value.skins_tried.contains(id)
            || value.skins_tried.len() >= MAX_SKINS_TRIED
        {
            return Ok(false);
        }
        value.skins_tried.insert(id.to_owned());
        self.write_locked(&lock, &value)?;
        Ok(true)
    }

    /// 把宿主攒下的一批上屏效率计数写入，一次加锁、一次读、最多一次写。
    ///
    /// 空批不碰磁盘，统计关闭时丢弃整批不写，和 `record_selections` 的答案相同。整批成功或整批不变。
    pub fn record_efficiency(&self, batch: &CommitEfficiency) -> Result<(), TypingStatisticsError> {
        if batch.is_empty() {
            return Ok(());
        }
        let lock = self.lock()?;
        let mut value = self.read_locked(&lock)?;
        if !value.enabled {
            return Ok(());
        }
        value.efficiency.merge(batch)?;
        self.write_locked(&lock, &value)?;
        Ok(())
    }

    /// 算出 `today` 这天的派生指标，并在锁内把新解锁的徽章记进 `achievements`。
    ///
    /// 指标本身由纯函数 [`summarize`] 算出；这里只负责让解锁只增不减。统计关闭时照样返回指标，但不写文件：关掉统计以后不应再有任何东西被记下。没有新解锁时也不写。
    pub fn summary(
        &self,
        today: &str,
        inputs: &SummaryInputs,
    ) -> Result<TypingSummary, TypingStatisticsError> {
        validate_day(today)?;
        let lock = self.lock()?;
        let mut value = self.read_locked(&lock)?;
        let summary = summarize(&value, today, inputs);
        if !value.enabled {
            return Ok(summary);
        }
        let mut changed = false;
        for achievement in &summary.achievements {
            if let Some(day) = &achievement.unlocked_day {
                if !value.achievements.contains_key(achievement.id) {
                    value
                        .achievements
                        .insert(achievement.id.to_owned(), day.clone());
                    changed = true;
                }
            }
        }
        if changed {
            self.write_locked(&lock, &value)?;
        }
        Ok(summary)
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<TypingStatistics, TypingStatisticsError> {
        let lock = self.lock()?;
        let mut value = self.read_locked(&lock)?;
        value.enabled = enabled;
        self.write_locked(&lock, &value)?;
        Ok(value)
    }

    /// Choose how long recorded days are kept.
    ///
    /// `today` is the caller's local day, for the same reason `record` takes one. A window that
    /// has just been narrowed applies immediately rather than at the next day boundary: the user
    /// asked for those days to be gone, and waiting would leave them visible on the page they
    /// asked from.
    pub fn set_retention(
        &self,
        retention: StatisticsRetention,
        today: &str,
    ) -> Result<TypingStatistics, TypingStatisticsError> {
        validate_day(today)?;
        let lock = self.lock()?;
        let mut value = self.read_locked(&lock)?;
        value.retention = retention;
        value.apply_retention(today);
        value.last_pruned_day = today.to_owned();
        self.write_locked(&lock, &value)?;
        Ok(value)
    }

    pub fn reset(&self) -> Result<TypingStatistics, TypingStatisticsError> {
        let lock = self.lock()?;
        let mut value = self.read_locked(&lock)?;
        value.total = 0;
        value.days.clear();
        value.detail = TypingBreakdown::default();
        value.daily_details.clear();
        // Reset means reset. Leaving the selection histogram behind would keep counting after a
        // user asked for it to stop existing, which is the one thing this module must not do.
        value.selections = SelectionCounts::default();
        value.daily_active_ms.clear();
        value.daily_hours.clear();
        value.daily_keys.clear();
        // Including when typing last happened: it is the only field that survives a reset by
        // saying anything about the user at all.
        value.last_commit_ms = 0;
        value.efficiency = CommitEfficiency::default();
        value.current_run = TypingRun::default();
        value.longest_run = TypingRun::default();
        value.daily_voice_ms.clear();
        value.skins_tried.clear();
        // 徽章只增不减的唯一例外：用户要求清空统计时，解锁记录也一并清空。
        value.achievements.clear();
        // 别的进程攒着的清空前的上屏据此认出清空，不写回来。
        value.generation = next_generation(value.generation);
        self.write_locked(&lock, &value)?;
        Ok(value)
    }
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

/// 清空后的代号：随机、非零、与清空前不同。系统随机数不可用时退回到在旧值上加一，只要不同就够用。
fn next_generation(previous: u64) -> u64 {
    let mut bytes = [0_u8; 8];
    let random = if getrandom::fill(&mut bytes).is_ok() {
        u64::from_le_bytes(bytes)
    } else {
        previous.wrapping_add(1)
    };
    if random == 0 || random == previous {
        previous.wrapping_add(1).max(1)
    } else {
        random
    }
}

/// Milliseconds since the Unix epoch, saturating at zero for clocks set before 1970.
fn epoch_millis(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// How much of the gap between two commits counts as active typing.
///
/// Zero for the first commit ever (`previous` is 0), for a gap longer than the limit, and for any
/// non-positive gap — which covers both a clock set backwards and two commits landing in the same
/// millisecond.
fn active_gap_ms(previous_ms: u64, now_ms: u64) -> u64 {
    if previous_ms == 0 || now_ms <= previous_ms {
        return 0;
    }
    let gap = now_ms - previous_ms;
    if gap > ACTIVE_GAP_LIMIT_MS {
        return 0;
    }
    gap
}

fn checked_increment(
    values: &mut BTreeMap<String, u64>,
    key: &str,
    amount: u64,
) -> Result<(), TypingStatisticsError> {
    let value = values.entry(key.to_owned()).or_default();
    *value = value
        .checked_add(amount)
        .filter(|count| *count <= MAX_COUNT)
        .ok_or(TypingStatisticsError::CountExhausted)?;
    Ok(())
}

fn validate_counts(value: &TypingBreakdown, total: u64) -> Result<(), TypingStatisticsError> {
    for values in [&value.characters, &value.sources] {
        if values.len() > 64
            || values.keys().any(|key| {
                key.is_empty()
                    || key.len() > 32
                    || !key
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            })
            || values.values().any(|count| *count > total)
            || values
                .values()
                .try_fold(0_u64, |sum, count| sum.checked_add(*count))
                .is_none_or(|sum| sum > total)
        {
            return Err(TypingStatisticsError::InvalidDocument);
        }
    }
    Ok(())
}

/// A day's key counts: whitelisted ids only, so never more entries than [`KEY_IDS`] has, each within `MAX_COUNT`.
///
/// Not compared with the day's character count. Presses and committed characters measure different things — a pinyin syllable is several presses for one character, a deletion is a press for none — so neither bounds the other.
fn validate_key_counts(keys: &BTreeMap<String, u64>) -> Result<(), TypingStatisticsError> {
    if keys.len() > KEY_IDS.len()
        || keys
            .iter()
            .any(|(key, count)| *count > MAX_COUNT || !is_known_key_id(key))
    {
        return Err(TypingStatisticsError::InvalidDocument);
    }
    Ok(())
}

fn validate_day(day: &str) -> Result<(), TypingStatisticsError> {
    crate::calendar::is_valid_day(day)
        .then_some(())
        .ok_or(TypingStatisticsError::InvalidDay)
}

fn classify(grapheme: &str) -> &'static str {
    let mut scalars = grapheme.chars();
    let Some(first) = scalars.next() else {
        return "symbol";
    };
    let code = first as u32;
    if is_han(code) {
        return "han";
    }
    if is_emoji(grapheme) {
        return "emoji";
    }
    match get_general_category(first) {
        GeneralCategory::DecimalNumber => "number",
        GeneralCategory::UppercaseLetter
        | GeneralCategory::LowercaseLetter
        | GeneralCategory::TitlecaseLetter
        | GeneralCategory::ModifierLetter
        | GeneralCategory::OtherLetter => {
            if is_latin(code) {
                "latin"
            } else {
                "otherLetter"
            }
        }
        GeneralCategory::ConnectorPunctuation
        | GeneralCategory::DashPunctuation
        | GeneralCategory::OpenPunctuation
        | GeneralCategory::ClosePunctuation
        | GeneralCategory::InitialPunctuation
        | GeneralCategory::FinalPunctuation
        | GeneralCategory::OtherPunctuation => "punctuation",
        _ => "symbol",
    }
}

fn is_han(code: u32) -> bool {
    (0x3400..=0x4dbf).contains(&code)
        || (0x4e00..=0x9fff).contains(&code)
        || (0xf900..=0xfaff).contains(&code)
        || (0x20000..=0x323af).contains(&code)
}

fn is_latin(code: u32) -> bool {
    (0x41..=0x5a).contains(&code)
        || (0x61..=0x7a).contains(&code)
        || (0xc0..=0x24f).contains(&code)
        || (0x1e00..=0x1eff).contains(&code)
        || (0xab30..=0xab6f).contains(&code)
        || (0xff21..=0xff3a).contains(&code)
        || (0xff41..=0xff5a).contains(&code)
}

fn is_emoji(grapheme: &str) -> bool {
    grapheme.chars().any(|scalar| {
        let code = scalar as u32;
        (0x1f000..=0x1faff).contains(&code)
            || (0x2600..=0x27bf).contains(&code)
            || code == 0x20e3
            || code == 0xfe0f
    })
}

#[cfg(test)]
mod key_tests;

#[cfg(test)]
mod selection_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod extension_tests;

#[cfg(test)]
mod batch_tests;
