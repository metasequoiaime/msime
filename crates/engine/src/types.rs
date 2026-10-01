//! Value types every module shares: candidate rows, query requests, key results and the session option structs.
//!
//! Numeric values that cross the host boundary are frozen: `CandidateSource` travels as `u8` in `candidate_sources` and hosts switch on it, `SchemeType` is serialised into the online identity, and `PersonalDictionaryKind` names are stored in the user journal.

/// Where a candidate row came from. The discriminant is the wire value; never reorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum CandidateSource {
    #[default]
    Database = 0,
    UserDatabase = 1,
    CloudSuggestion = 2,
    AiSuggestion = 3,
    EnglishDictionary = 4,
    QuickPhrase = 5,
    Emoji = 6,
    Kaomoji = 7,
    Generated = 8,
    Fallback = 9,
    /// A lattice path the desktop (accuracy) sentence model picked. The engine no longer emits it (that model runs only as the runtime's settled reranker); the value stays reserved so the wire numbering and learned rows keep their meaning.
    NeuralDesktop = 10,
    /// A lattice path the keyboard (speed) sentence model picked.
    NeuralKeyboard = 11,
}

impl CandidateSource {
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Database,
            1 => Self::UserDatabase,
            2 => Self::CloudSuggestion,
            3 => Self::AiSuggestion,
            4 => Self::EnglishDictionary,
            5 => Self::QuickPhrase,
            6 => Self::Emoji,
            7 => Self::Kaomoji,
            8 => Self::Generated,
            9 => Self::Fallback,
            10 => Self::NeuralDesktop,
            11 => Self::NeuralKeyboard,
            _ => return None,
        })
    }

    pub fn is_online(self) -> bool {
        matches!(self, Self::CloudSuggestion | Self::AiSuggestion)
    }

    pub fn is_generated_or_fallback(self) -> bool {
        matches!(self, Self::Generated | Self::Fallback)
    }

    /// Rows a direct selection stores as a user phrase, because there is no dictionary row whose frequency could be adjusted instead.
    pub fn is_sentence_learning(self) -> bool {
        self.is_generated_or_fallback()
            || self.is_online()
            || matches!(self, Self::NeuralDesktop | Self::NeuralKeyboard)
    }

    pub fn is_dictionary(self) -> bool {
        matches!(self, Self::Database | Self::UserDatabase)
    }
}

/// Input scheme. The ordinal is part of the online identity (`"<scheme>:<raw>"`) and of the host ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SchemeType {
    #[default]
    Quanpin = 0,
    Shuangpin = 1,
    Wubi = 2,
    JapaneseRomaji = 3,
    /// Korean Hangul on the Dubeolsik layout: syllables compose in the preedit and commit themselves; the only candidates are the composing syllable's Hanja, after `Command::ConvertHanja`.
    Korean = 4,
}

impl SchemeType {
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Quanpin,
            1 => Self::Shuangpin,
            2 => Self::Wubi,
            3 => Self::JapaneseRomaji,
            4 => Self::Korean,
            _ => return None,
        })
    }

    /// The name the golden fixtures use.
    pub fn name(self) -> &'static str {
        match self {
            Self::Quanpin => "quanpin",
            Self::Shuangpin => "shuangpin",
            Self::Wubi => "wubi",
            Self::JapaneseRomaji => "japanese",
            Self::Korean => "korean",
        }
    }

    pub fn is_pinyin(self) -> bool {
        matches!(self, Self::Quanpin | Self::Shuangpin)
    }
}

/// Double-pinyin keyboard profile. The ordinal is the host ABI value (`EngineOptions::shuangpin_profile`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ShuangpinProfileKind {
    #[default]
    Xiaohe = 0,
    Ziranma = 1,
    Shoudao = 2,
    Microsoft = 3,
}

impl ShuangpinProfileKind {
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Xiaohe,
            1 => Self::Ziranma,
            2 => Self::Shoudao,
            3 => Self::Microsoft,
            _ => return None,
        })
    }

    /// Exact-name lookup. The C++ `GetShuangpinProfile` fell back to xiaohe for an unknown name; callers that want that do it themselves, so an unknown name stays visible here.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "xiaohe" => Self::Xiaohe,
            "ziranma" => Self::Ziranma,
            "shoudao" => Self::Shoudao,
            "microsoft" => Self::Microsoft,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Xiaohe => "xiaohe",
            Self::Ziranma => "ziranma",
            Self::Shoudao => "shoudao",
            Self::Microsoft => "microsoft",
        }
    }
}

/// One candidate row. Every field the goldens record is here from the start, including the ones the overlays added.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WordItem {
    /// The code the row matched, as typed. Composition advance consumes exactly this.
    pub pinyin: String,
    /// The complete quanpin key from the dictionary, used when a phrase is persisted.
    pub canonical_pinyin: String,
    pub word: String,
    /// Dictionary weight; for lattice rows `trunc(log_prob * 1000)`, which is not a sort key.
    pub weight: i64,
    pub source: CandidateSource,
    /// The scheme whose provider produced the row; in mixed wubi it routes learning and removal to the right table.
    pub scheme: SchemeType,
    /// 0 when unfixed, otherwise the fixed slot 1..=5.
    pub fixed_position: i32,
    pub fuzzy: bool,
    /// Non-empty for a row read from an autocorrected spelling; holds the folded letters the user typed.
    pub corrected_from: String,
    /// Only whole-sentence association rows (lattice and neural sentences).
    pub sentence_association: bool,
    /// Word boundaries of a decoded sentence, for personal context learning.
    pub sentence_words: Vec<String>,
}

impl WordItem {
    pub fn new(
        pinyin: impl Into<String>,
        word: impl Into<String>,
        weight: i64,
        source: CandidateSource,
        canonical_pinyin: impl Into<String>,
    ) -> Self {
        Self {
            pinyin: pinyin.into(),
            canonical_pinyin: canonical_pinyin.into(),
            word: word.into(),
            weight,
            source,
            ..Self::default()
        }
    }
}

/// Fuzzy pinyin rule bits. The values are shared with platform preferences and must not change.
pub mod fuzzy_rule {
    pub const Z_ZH: u32 = 1 << 0;
    pub const C_CH: u32 = 1 << 1;
    pub const S_SH: u32 = 1 << 2;
    pub const N_L: u32 = 1 << 3;
    pub const F_H: u32 = 1 << 4;
    pub const R_L: u32 = 1 << 5;
    pub const AN_ANG: u32 = 1 << 6;
    pub const EN_ENG: u32 = 1 << 7;
    pub const IN_ING: u32 = 1 << 8;
    pub const IAN_IANG: u32 = 1 << 9;
    pub const UAN_UANG: u32 = 1 << 10;
    /// Every defined rule; the host masks its preference with this.
    pub const ALL: u32 = 0x7ff;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FuzzyPinyinOptions {
    pub rules: u32,
}

impl FuzzyPinyinOptions {
    pub fn enabled(self, rule: u32) -> bool {
        self.rules & rule != 0
    }
}

/// Quanpin autocorrect type bits (`SessionOptions::autocorrect_types`). Either of the first two also enables deletion and insertion, see `request_autocorrect_mask`.
pub mod autocorrect_type {
    pub const TRANSPOSITION: u32 = 1 << 0;
    pub const NEIGHBOR: u32 = 1 << 1;
    pub const DELETION: u32 = 1 << 2;
    pub const INSERTION: u32 = 1 << 3;
    pub const MISSING_OR_EXTRA: u32 = DELETION | INSERTION;
}

/// The mask the quanpin dictionary runs with for a request: the two user switches, and when either is on, also missing and extra letters.
pub fn request_autocorrect_mask(transposition: bool, neighbor: bool) -> u32 {
    let legacy = if transposition {
        autocorrect_type::TRANSPOSITION
    } else {
        0
    } | if neighbor {
        autocorrect_type::NEIGHBOR
    } else {
        0
    };
    if legacy == 0 {
        0
    } else {
        legacy | autocorrect_type::MISSING_OR_EXTRA
    }
}

/// Neural sentence association switches. The desktop-model switch is not among them: that model runs only as the input runtime's settled reranker, which the host gates on the preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SentenceAssociationOptions {
    pub word_lattice: bool,
    pub neural_keyboard: bool,
    pub show_next_on_duplicate: bool,
}

impl Default for SentenceAssociationOptions {
    fn default() -> Self {
        Self {
            word_lattice: true,
            neural_keyboard: false,
            show_next_on_duplicate: false,
        }
    }
}

/// What a scheme turns its composition into for the providers. Built fresh on every refresh; providers never mutate it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QueryRequest {
    pub scheme: SchemeType,
    /// Lowercased, apostrophes kept.
    pub raw_input: String,
    pub raw_input_with_cases: String,
    /// The dictionary key: no apostrophes.
    pub normalized_input: String,
    pub raw_segmentation: String,
    pub normalized_segmentation: String,
    pub segmentation: String,
    pub enable_shuangpin_helpcode: bool,
    pub enable_quanpin_helpcode: bool,
    pub enable_quanpin_autocorrect_transposition: bool,
    pub enable_quanpin_autocorrect_neighbor: bool,
    pub fuzzy_pinyin: FuzzyPinyinOptions,
    /// Hand back every whole-sentence reading rather than only the best.
    pub sentence_alternatives: bool,
    pub sentence_association: SentenceAssociationOptions,
    /// Committed text the neural sentence models condition on.
    pub rescoring_context: String,
    /// Korean only: the Hanja list of the composing syllable is open, so the request is answered with its Hanja.
    pub korean_hanja: bool,
    /// False means no query at all.
    pub valid: bool,
}

/// A key a scheme can consume. The session maps host characters onto these, so schemes never see virtual-key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemeKey {
    /// An ASCII letter, case preserved.
    Letter(u8),
    Apostrophe,
    /// The Microsoft shuangpin `ing` final.
    Semicolon,
    /// The Japanese long vowel mark.
    Minus,
    Backspace,
    /// Re-run the query without changing the composition.
    Requery,
}

/// Engine-level composition commands (the C++ `metasequoia::Command`). The host facade adds its own `CommitRawWithoutLearning` on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Command {
    Backspace = 0,
    CommitCandidate = 1,
    CommitRaw = 2,
    Cancel = 3,
    MoveLeft = 4,
    MoveRight = 5,
    MoveHome = 6,
    MoveEnd = 7,
    DeleteForward = 8,
    /// Japanese only.
    CycleKanaVariant = 9,
    /// Japanese only: commit the kana reading.
    CommitReading = 10,
    /// Korean only: open the composing syllable's Hanja list, or close it when it is open.
    ConvertHanja = 11,
}

impl Command {
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Backspace,
            1 => Self::CommitCandidate,
            2 => Self::CommitRaw,
            3 => Self::Cancel,
            4 => Self::MoveLeft,
            5 => Self::MoveRight,
            6 => Self::MoveHome,
            7 => Self::MoveEnd,
            8 => Self::DeleteForward,
            9 => Self::CycleKanaVariant,
            10 => Self::CommitReading,
            11 => Self::ConvertHanja,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Backspace => "Backspace",
            Self::CommitCandidate => "CommitCandidate",
            Self::CommitRaw => "CommitRaw",
            Self::Cancel => "Cancel",
            Self::MoveLeft => "MoveLeft",
            Self::MoveRight => "MoveRight",
            Self::MoveHome => "MoveHome",
            Self::MoveEnd => "MoveEnd",
            Self::DeleteForward => "DeleteForward",
            Self::CycleKanaVariant => "CycleKanaVariant",
            Self::CommitReading => "CommitReading",
            Self::ConvertHanja => "ConvertHanja",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        (0..=11)
            .filter_map(Self::from_u8)
            .find(|command| command.name() == name)
    }
}

/// Outcome of one key or action. A result can be handled and still carry a diagnostic, for example when learning could not be persisted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeyResult {
    pub handled: bool,
    pub commit: Option<String>,
    pub diagnostic: Option<String>,
}

impl KeyResult {
    pub fn unhandled() -> Self {
        Self::default()
    }

    pub fn handled() -> Self {
        Self {
            handled: true,
            ..Self::default()
        }
    }

    pub fn committed(text: impl Into<String>) -> Self {
        Self {
            handled: true,
            commit: Some(text.into()),
            diagnostic: None,
        }
    }

    pub fn with_diagnostic(mut self, diagnostic: Option<String>) -> Self {
        self.diagnostic = diagnostic;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CandidateEdge {
    FirstHan = 0,
    LastHan = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LocalInputMode {
    #[default]
    None,
    Unicode,
    DateTime,
    QuickPhrase,
    Emoji,
    Kaomoji,
    SuperJianpin,
    TemporaryEnglish,
    TemporaryJapanese,
    /// `V`: a calculator, Chinese numerals and dates.
    Expression,
    /// `/`: built-in commands and the host's command table.
    Command,
    /// `@`: the names and places of the host's mention list.
    Mention,
}

impl LocalInputMode {
    /// The name hosts and goldens use (`EngineSnapshot::local_mode`).
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Unicode => "unicode",
            Self::DateTime => "date_time",
            Self::QuickPhrase => "quick_phrase",
            Self::Emoji => "emoji",
            Self::Kaomoji => "kaomoji",
            Self::SuperJianpin => "super_jianpin",
            Self::TemporaryEnglish => "temporary_english",
            Self::TemporaryJapanese => "temporary_japanese",
            Self::Expression => "expression",
            Self::Command => "command",
            Self::Mention => "mention",
        }
    }

    /// The mode `name` returns the name of; `None` for a name no mode has.
    pub fn from_name(name: &str) -> Option<Self> {
        [
            Self::None,
            Self::Unicode,
            Self::DateTime,
            Self::QuickPhrase,
            Self::Emoji,
            Self::Kaomoji,
            Self::SuperJianpin,
            Self::TemporaryEnglish,
            Self::TemporaryJapanese,
            Self::Expression,
            Self::Command,
            Self::Mention,
        ]
        .into_iter()
        .find(|mode| mode.name() == name)
    }

    /// The non-letter characters the mode spells with. The runtime hands these to `character` even when a host reports them as punctuation, because finishing the composition on them would commit a half-typed spelling; hosts read digits here as input rather than candidate shortcuts.
    pub fn spelling_symbols(self) -> &'static str {
        match self {
            Self::Unicode => "0123456789",
            Self::Expression => crate::local::expression::SPELLING_SYMBOLS,
            _ => "",
        }
    }

    /// Whether the mode's rows are text the engine generated rather than words the user spelled. Their commits are never learned and never counted as typing.
    pub fn generates_text(self) -> bool {
        matches!(self, Self::Expression | Self::Command | Self::Mention)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrequencyAdjustmentMode {
    #[default]
    Disabled,
    Pin,
    Halve,
    Linear,
    Promote,
}

impl FrequencyAdjustmentMode {
    /// The mode name the ranking writers and the host options use.
    pub fn name(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Pin => "pin",
            Self::Halve => "halve",
            Self::Linear => "linear",
            Self::Promote => "promote",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "disabled" => Self::Disabled,
            "pin" => Self::Pin,
            "halve" => Self::Halve,
            "linear" => Self::Linear,
            "promote" => Self::Promote,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrequencyAdjustmentOptions {
    pub mode: FrequencyAdjustmentMode,
    /// 1..=10.
    pub trigger_count: i32,
    /// 1..=10.
    pub linear_step: i32,
}

impl Default for FrequencyAdjustmentOptions {
    fn default() -> Self {
        Self {
            mode: FrequencyAdjustmentMode::Disabled,
            trigger_count: 1,
            linear_step: 1,
        }
    }
}

/// Which local modes may be entered. The Shift+letter modes of the reference are on by default; expression (`V`), command (`/`) and mention (`@`) are off, because entering them takes a key that used to reach the host as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalModeOptions {
    pub unicode: bool,
    pub date_time: bool,
    pub quick_phrase: bool,
    pub emoji: bool,
    pub kaomoji: bool,
    pub super_jianpin: bool,
    pub temporary_english: bool,
    pub temporary_japanese: bool,
    pub expression: bool,
    pub command: bool,
    pub mention: bool,
}

impl Default for LocalModeOptions {
    fn default() -> Self {
        Self {
            unicode: true,
            date_time: true,
            quick_phrase: true,
            emoji: true,
            kaomoji: true,
            super_jianpin: true,
            temporary_english: true,
            temporary_japanese: true,
            expression: false,
            command: false,
            mention: false,
        }
    }
}

/// One command of a host-supplied command table (`/` mode). The host validates the table it read; the engine still skips a row it cannot use rather than trusting it.
///
/// `template` is literal text with three kinds of placeholder: `{date}` or `{date:FORMAT}`, `{time}` or `{time:FORMAT}`, and `{weekday}`. FORMAT is a strftime pattern (`%Y-%m-%d`), the defaults are `%Y-%m-%d` and `%H:%M`, and `{weekday}` is the Chinese day name (星期四). Any other brace, including one left unclosed, makes the row unusable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandTableEntry {
    /// 1..=32 lowercase ASCII letters, what the user types after `/`.
    pub trigger: String,
    /// Shown beside the row.
    pub title: String,
    pub template: String,
}

/// One name or place of the host's mention list (`@` mode).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionEntry {
    /// The committed text.
    pub text: String,
    /// Lowercase pinyin syllables joined by `'` (`zhang'san`), matched by prefix and by initials; empty to match an ASCII `text` by its own letters only.
    pub key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnglishInputOptions {
    pub mixed_candidates: bool,
    /// 1..=8.
    pub minimum_prefix: usize,
}

impl Default for EnglishInputOptions {
    fn default() -> Self {
        Self {
            mixed_candidates: false,
            minimum_prefix: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MixedExpressiveOptions {
    pub emoji_candidates: bool,
    pub kaomoji_candidates: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WubiInputOptions {
    /// Answer an unmatched wubi code with quanpin candidates for the same letters.
    pub mixed_pinyin: bool,
}

/// The request a host sends to its cloud or AI provider, and must hand back unchanged with the answer. The engine revalidates every field against the live composition before inserting anything.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OnlineQuery {
    pub scheme: SchemeType,
    pub generation: u64,
    pub identity: String,
    pub query_text: String,
    pub cache_key: String,
    pub pinyin_segments: Vec<String>,
    pub cloud_eligible: bool,
    pub ai_eligible: bool,
    pub session_id: u64,
}

/// What `/fy` asks the user's translation service: the English typed after the trigger, to be translated into Chinese. Unlike `OnlineQuery` it is never raised by spelling; only this command in the `/` mode produces one, and the answer is refused unless the session and the text are still the ones that asked.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandTranslationQuery {
    pub session_id: u64,
    pub text: String,
}

/// Which dictionary a personal entry or journal row belongs to. The ordinal is the host ABI value.
///
/// `non_exhaustive` because the host facade re-exports this as the bridge's `DictionaryKind`, a cxx shared enum that callers had to match with a wildcard arm (host-api `dictionary.rs`); a closed enum would turn those arms into `unreachable_patterns` warnings. Matches inside this crate stay exhaustive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
#[repr(u8)]
#[non_exhaustive]
pub enum PersonalDictionaryKind {
    #[default]
    Pinyin = 0,
    Wubi = 1,
    QuickPhrase = 2,
    English = 3,
}

impl PersonalDictionaryKind {
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Pinyin,
            1 => Self::Wubi,
            2 => Self::QuickPhrase,
            3 => Self::English,
            _ => return None,
        })
    }

    /// The `dictionary` column value in `user_dictionary_operations`. Quick phrases are stored as `quick`.
    pub fn journal_name(self) -> &'static str {
        match self {
            Self::Pinyin => "pinyin",
            Self::Wubi => "wubi",
            Self::QuickPhrase => "quick",
            Self::English => "english",
        }
    }

    pub fn from_journal_name(name: &str) -> Option<Self> {
        Some(match name {
            "pinyin" => Self::Pinyin,
            "wubi" => Self::Wubi,
            "quick" => Self::QuickPhrase,
            "english" => Self::English,
            _ => return None,
        })
    }
}

/// A user's own dictionary entry. Pinyin keys are complete syllables joined by `'`; wubi 1..=4 letters; quick phrases 1..=32 letters or digits; English 1..=64 letters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PersonalDictionaryEntry {
    pub kind: PersonalDictionaryKind,
    pub key: String,
    pub value: String,
    pub weight: i64,
}

impl PersonalDictionaryEntry {
    pub const DEFAULT_WEIGHT: i64 = 100_000;
}

impl Default for PersonalDictionaryEntry {
    fn default() -> Self {
        Self {
            kind: PersonalDictionaryKind::Pinyin,
            key: String::new(),
            value: String::new(),
            weight: Self::DEFAULT_WEIGHT,
        }
    }
}
