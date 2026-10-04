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
        self.is_generated_or_fallback() || self.is_online() || matches!(self, Self::NeuralKeyboard)
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
    /// Cantonese in toneless Jyutping read against `msime-cantonese.db`: candidates are Traditional as stored, a candidate covering the leading syllables commits at once and leaves the rest composing, and nothing is learned.
    Cantonese = 5,
    /// Zhuyin (bopomofo) on the Dachen layout read against `msime-zhuyin.db`: keys compose syllables that convert to Traditional text as typed, a list the user opens pins a span's text without committing, Enter or any key outside the layout commits the conversion, and nothing is learned.
    Zhuyin = 6,
    /// Vietnamese through Telex or VNI: the keystrokes compose into one word in the preedit, which any key outside the spelling commits; there are no candidates.
    Vietnamese = 7,
    /// 藏文，在拉丁键盘上按 EWTS（扩展威利转写）拼写：威利原文在组字里组成一个音节串，显示为转换出的藏文；空格带音节点上屏，`/` 带垂符上屏，回车只上屏藏文；没有候选。
    Tibetan = 8,
    /// Stroke (笔画) read against `msime-stroke.db`: the keys h s p n z type the five strokes 横竖撇点折 in writing order and x stands for any one stroke; the preedit draws the strokes, candidates are single characters whose stroke code starts with the typed strokes, and nothing is learned.
    Stroke = 9,
}

impl SchemeType {
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Quanpin,
            1 => Self::Shuangpin,
            2 => Self::Wubi,
            3 => Self::JapaneseRomaji,
            4 => Self::Korean,
            5 => Self::Cantonese,
            6 => Self::Zhuyin,
            7 => Self::Vietnamese,
            8 => Self::Tibetan,
            9 => Self::Stroke,
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
            Self::Cantonese => "cantonese",
            Self::Zhuyin => "zhuyin",
            Self::Vietnamese => "vietnamese",
            Self::Tibetan => "tibetan",
            Self::Stroke => "stroke",
        }
    }

    pub fn is_pinyin(self) -> bool {
        matches!(self, Self::Quanpin | Self::Shuangpin)
    }

    // ---- Scheme traits ----
    //
    // The one source of truth for what differs between schemes. Every predicate matches exhaustively, so a new scheme cannot compile until each trait is decided for it. The values for the existing schemes encode today's behaviour; input-runtime and host-api read them through `SchemeType::from_u8`.

    /// A Chinese scheme: what 中文 returns to and what the Chinese statistics count.
    pub const fn is_chinese(self) -> bool {
        match self {
            Self::Quanpin
            | Self::Shuangpin
            | Self::Wubi
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => true,
            Self::JapaneseRomaji | Self::Korean | Self::Vietnamese | Self::Tibetan => false,
        }
    }

    /// The scheme's own text is Traditional Chinese, so no script conversion is wanted.
    pub const fn outputs_traditional_natively(self) -> bool {
        match self {
            Self::Quanpin
            | Self::Shuangpin
            | Self::Wubi
            | Self::JapaneseRomaji
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan => false,
            Self::Cantonese | Self::Stroke | Self::Zhuyin => true,
        }
    }

    /// The host's Simplified-to-Traditional conversion applies to the scheme's commits and preedit.
    pub const fn script_conversion_applies(self) -> bool {
        match self {
            Self::Quanpin | Self::Shuangpin | Self::Wubi => true,
            Self::JapaneseRomaji
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => false,
        }
    }

    /// The engine translates punctuation through the Chinese table. Korean writes half-width ASCII marks instead; Japanese goes through the table like the Chinese schemes.
    pub const fn uses_chinese_punctuation(self) -> bool {
        match self {
            Self::Quanpin
            | Self::Shuangpin
            | Self::Wubi
            | Self::JapaneseRomaji
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => true,
            Self::Korean | Self::Vietnamese | Self::Tibetan => false,
        }
    }

    /// The host's smart punctuation (context-dependent marks) may run.
    pub const fn host_smart_punctuation(self) -> bool {
        match self {
            Self::Quanpin | Self::Shuangpin | Self::Wubi | Self::Cantonese | Self::Stroke => true,
            Self::JapaneseRomaji
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Zhuyin => false,
        }
    }

    /// Commits are widened when the full-width switch is on.
    pub const fn widens_full_width(self) -> bool {
        match self {
            Self::Quanpin
            | Self::Shuangpin
            | Self::Wubi
            | Self::JapaneseRomaji
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => true,
            Self::Korean | Self::Vietnamese | Self::Tibetan => false,
        }
    }

    /// English words and emoji or kaomoji may be mixed into the candidate list.
    pub const fn allows_english_emoji_mixing(self) -> bool {
        match self {
            Self::Quanpin | Self::Shuangpin => true,
            Self::Wubi
            | Self::JapaneseRomaji
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => false,
        }
    }

    /// Shift+letter and the `/` and `@` keys open local modes while nothing is composed.
    pub const fn opens_local_modes(self) -> bool {
        match self {
            Self::Quanpin | Self::Shuangpin => true,
            Self::Wubi
            | Self::JapaneseRomaji
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => false,
        }
    }

    /// 组字中键入 `www.`、`http:` 等时进入网址模式。
    pub const fn detects_urls(self) -> bool {
        match self {
            Self::Quanpin | Self::Shuangpin | Self::Wubi => true,
            Self::JapaneseRomaji
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => false,
        }
    }

    /// Selections adjust frequencies and store sentences in the main dictionary and journal.
    pub const fn learns_into_main_dictionary(self) -> bool {
        match self {
            Self::Quanpin | Self::Shuangpin | Self::Wubi => true,
            Self::JapaneseRomaji
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => false,
        }
    }

    /// Letters committed raw that are not a complete spelling are learned as an English word.
    pub const fn learns_english_words(self) -> bool {
        match self {
            Self::Quanpin | Self::Shuangpin => true,
            Self::Wubi
            | Self::JapaneseRomaji
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => false,
        }
    }

    /// The engine raises cloud queries for the composition.
    pub const fn cloud_eligible(self) -> bool {
        match self {
            Self::Quanpin | Self::Shuangpin | Self::JapaneseRomaji => true,
            Self::Wubi
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => false,
        }
    }

    /// Candidates may carry translation glosses. Korean's are the Hanja rows, glossed and translated like Chinese ones under their 훈음.
    pub const fn shows_glosses(self) -> bool {
        match self {
            Self::Quanpin | Self::Shuangpin | Self::Wubi | Self::Korean => true,
            Self::JapaneseRomaji
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => false,
        }
    }

    /// Focus loss or a scheme switch commits the composition instead of discarding it.
    pub const fn commits_on_blur(self) -> bool {
        match self {
            Self::Korean | Self::Vietnamese | Self::Tibetan | Self::Zhuyin => true,
            Self::Quanpin
            | Self::Shuangpin
            | Self::Wubi
            | Self::JapaneseRomaji
            | Self::Cantonese
            | Self::Stroke => false,
        }
    }

    /// A partial selection is held as phrase progress rather than committed at once.
    pub const fn holds_phrase_progress(self) -> bool {
        match self {
            Self::Quanpin | Self::Shuangpin | Self::Wubi | Self::JapaneseRomaji => true,
            Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => false,
        }
    }

    /// The candidate list may be reordered by the sentence model and the personal context.
    pub const fn reranks_with_sentence_model(self) -> bool {
        match self {
            Self::Quanpin | Self::Shuangpin => true,
            Self::Wubi
            | Self::JapaneseRomaji
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => false,
        }
    }

    /// The snapshot's `reading` carries the composed text (kana, Hangul, the Zhuyin conversion, the Stroke glyphs 一丨丿丶乛) for the host to draw.
    pub const fn draws_reading(self) -> bool {
        match self {
            Self::JapaneseRomaji | Self::Korean | Self::Zhuyin | Self::Stroke => true,
            Self::Quanpin
            | Self::Shuangpin
            | Self::Wubi
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese => false,
        }
    }

    /// Candidates appear only in a list the user opens (`Command::ConvertHanja`: the Korean Hanja list, the Zhuyin list), and the list can be closed again.
    pub const fn has_openable_candidate_list(self) -> bool {
        match self {
            Self::Korean | Self::Zhuyin => true,
            Self::Quanpin
            | Self::Shuangpin
            | Self::Wubi
            | Self::JapaneseRomaji
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke => false,
        }
    }

    /// 第一次 Cancel 保留组字：关闭打开的候选列表，或把越南文、藏文的组字切回按键原文。第二次 Cancel 才丢弃它。
    pub const fn cancel_keeps_composition(self) -> bool {
        match self {
            Self::Korean | Self::Zhuyin | Self::Vietnamese | Self::Tibetan => true,
            Self::Quanpin
            | Self::Shuangpin
            | Self::Wubi
            | Self::JapaneseRomaji
            | Self::Cantonese
            | Self::Stroke => false,
        }
    }

    /// Selecting any of the scheme's candidates finishes the composition. Native wubi rows also finish, which is decided per row because a mixed wubi list holds pinyin rows too.
    pub const fn selection_completes(self) -> bool {
        match self {
            Self::JapaneseRomaji | Self::Korean | Self::Vietnamese | Self::Tibetan => true,
            Self::Quanpin
            | Self::Shuangpin
            | Self::Wubi
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => false,
        }
    }

    /// The candidate list reads the composition as pinyin, so selections advance and learn as pinyin.
    pub const fn follows_pinyin_candidates(self) -> bool {
        match self {
            Self::Quanpin | Self::Shuangpin => true,
            Self::Wubi
            | Self::JapaneseRomaji
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => false,
        }
    }

    /// An apostrophe typed inside the composition is a syllable boundary the scheme keeps.
    pub const fn accepts_apostrophe(self) -> bool {
        match self {
            Self::Quanpin | Self::Shuangpin | Self::JapaneseRomaji | Self::Cantonese => true,
            Self::Wubi
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Zhuyin
            | Self::Stroke => false,
        }
    }

    /// The caret stays at the end of the composition.
    pub const fn locks_caret(self) -> bool {
        match self {
            Self::Korean | Self::Vietnamese | Self::Tibetan | Self::Zhuyin => true,
            Self::Quanpin
            | Self::Shuangpin
            | Self::Wubi
            | Self::JapaneseRomaji
            | Self::Cantonese
            | Self::Stroke => false,
        }
    }

    /// Fuzzy pinyin rules apply.
    pub const fn supports_fuzzy(self) -> bool {
        match self {
            Self::Quanpin | Self::Shuangpin => true,
            Self::Wubi
            | Self::JapaneseRomaji
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => false,
        }
    }

    /// Typo autocorrection applies and its outcomes are learned.
    pub const fn supports_autocorrect(self) -> bool {
        match self {
            Self::Quanpin => true,
            Self::Shuangpin
            | Self::Wubi
            | Self::JapaneseRomaji
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => false,
        }
    }

    /// The nine-key grid can spell the scheme's syllables.
    pub const fn nine_key(self) -> bool {
        match self {
            Self::Quanpin => true,
            Self::Shuangpin
            | Self::Wubi
            | Self::JapaneseRomaji
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => false,
        }
    }

    /// The scheme has a helpcode switch and its candidates carry helpcode annotations.
    pub const fn helpcode(self) -> bool {
        match self {
            Self::Quanpin | Self::Shuangpin => true,
            Self::Wubi
            | Self::JapaneseRomaji
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke
            | Self::Zhuyin => false,
        }
    }
}

/// 会话允许运行的方案集合，每个方案占 `SchemeType` 序号对应的那一位。缺省是 [`SchemeSet::ALL`]，即全部方案，行为与没有这个集合时完全相同。
///
/// 收窄后，`ProviderRegistry` 只为集合里的方案构造 provider（全拼在全拼或五笔任一在集合里时构造，五笔混拼要用它），切换到集合外的方案报 `INPUT_SCHEME_NOT_ENABLED`。临时日文切到的也是日文方案，所以要用临时日文，集合里就得有 `JapaneseRomaji`；没有时临时日文的触发键不会进入这个模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SchemeSet(u16);

impl SchemeSet {
    /// 序号最大的方案。加方案时改这里，`scheme_set_all_covers_every_scheme` 会在漏改时失败。
    const LAST: SchemeType = SchemeType::Stroke;
    /// 全部方案：序号 0 到 [`Self::LAST`] 的每一位。
    pub const ALL: Self = Self((1 << (Self::LAST as u16 + 1)) - 1);
    pub const EMPTY: Self = Self(0);

    pub const fn contains(self, scheme: SchemeType) -> bool {
        self.0 & (1 << scheme as u16) != 0
    }

    #[must_use]
    pub const fn with(self, scheme: SchemeType) -> Self {
        Self(self.0 | (1 << scheme as u16))
    }

    /// 由方案列表组成的集合，重复的方案只算一次。
    pub fn of(schemes: &[SchemeType]) -> Self {
        schemes
            .iter()
            .fold(Self::EMPTY, |set, scheme| set.with(*scheme))
    }

    /// 集合里是否有读 `msime.db` 的方案：全拼、双拼和五笔的候选、学习和用户词都在它里面（五笔混拼的拼音行也是）。没有这三个方案的集合（例如只有日文、越南文或藏文的版本）随包不带 `msime.db`：代次里没有它的工作副本，用户词库只剩英文词。
    pub const fn reads_main_dictionary(self) -> bool {
        self.contains(SchemeType::Quanpin)
            || self.contains(SchemeType::Shuangpin)
            || self.contains(SchemeType::Wubi)
    }
}

impl Default for SchemeSet {
    fn default() -> Self {
        Self::ALL
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

/// 五笔码表版本。序号即宿主 ABI 值（`EngineOptions::wubi_profile`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum WubiProfileKind {
    #[default]
    Wubi86 = 0,
    Wubi98 = 1,
}

impl WubiProfileKind {
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Wubi86,
            1 => Self::Wubi98,
            _ => return None,
        })
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "wubi86" => Self::Wubi86,
            "wubi98" => Self::Wubi98,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Wubi86 => "wubi86",
            Self::Wubi98 => "wubi98",
        }
    }

    /// `msime-pinyin.db` 里这一版的码表，表名与 `name` 相同。
    pub fn table(self) -> &'static str {
        self.name()
    }

    /// 这一版的个人词条和学习记录归入的词库种类。
    pub fn dictionary_kind(self) -> PersonalDictionaryKind {
        match self {
            Self::Wubi86 => PersonalDictionaryKind::Wubi,
            Self::Wubi98 => PersonalDictionaryKind::Wubi98,
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
    /// Any other printable non-letter ASCII key a scheme claims through its spelling symbols (Zhuyin's `;`, `-`, digits and space). Kept apart from `Semicolon` and `Minus` so the shuangpin and Japanese meanings of those keys never change; the existing schemes ignore it.
    Symbol(u8),
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
    /// 丢弃组字。越南文和藏文的第一次只把显示切回按键原文，下一次才丢弃。
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
    /// Open the active scheme's candidate list, or close it when it is open: the composing syllable's Hanja in Korean, the conversion's alternatives in Zhuyin. Schemes without an openable list leave it unhandled. The name stays for the wire and the goldens.
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
    /// 在全拼、双拼、五笔的组字中键入 `www.`、`http:` 等之后：原样输入的 ASCII 网址。
    Url,
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
            Self::Url => "url",
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
            Self::Url,
        ]
        .into_iter()
        .find(|mode| mode.name() == name)
    }

    /// The non-letter characters the mode spells with. The runtime hands these to `character` even when a host reports them as punctuation, because finishing the composition on them would commit a half-typed spelling; hosts read digits here as input rather than candidate shortcuts.
    pub fn spelling_symbols(self) -> &'static str {
        match self {
            Self::Unicode => "0123456789",
            Self::Expression => crate::local::expression::SPELLING_SYMBOLS,
            Self::Url => crate::local::url::SPELLING_SYMBOLS,
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

/// 宿主提供的一条 K 模式短语（来自已启用的短语表插件）。宿主已经校验过；Engine 仍然跳过它用不了的行，而不是照单全收。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickPhraseEntry {
    /// 1..=32 个小写 ASCII 字母，即 K 之后输入的编码。
    pub key: String,
    /// 上屏的文本。
    pub text: String,
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
    /// 查询、学习和删除所用的码表版本。
    pub profile: WubiProfileKind,
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
    /// 98 五笔，写入 `wubi98` 表。序号追加在末尾，已有的值不变。
    Wubi98 = 4,
}

impl PersonalDictionaryKind {
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Pinyin,
            1 => Self::Wubi,
            2 => Self::QuickPhrase,
            3 => Self::English,
            4 => Self::Wubi98,
            _ => return None,
        })
    }

    /// 两版五笔共用编码规则（1..=4 个 a..=y 字母）与按码排序的写法，只是表不同。
    pub fn is_wubi(self) -> bool {
        matches!(self, Self::Wubi | Self::Wubi98)
    }

    /// 五笔种类对应的 `msime-pinyin.db` 码表。
    pub fn wubi_table(self) -> Option<&'static str> {
        match self {
            Self::Wubi => Some(WubiProfileKind::Wubi86.table()),
            Self::Wubi98 => Some(WubiProfileKind::Wubi98.table()),
            Self::Pinyin | Self::QuickPhrase | Self::English => None,
        }
    }

    /// The `dictionary` column value in `user_dictionary_operations`. Quick phrases are stored as `quick`.
    pub fn journal_name(self) -> &'static str {
        match self {
            Self::Pinyin => "pinyin",
            Self::Wubi => "wubi",
            Self::QuickPhrase => "quick",
            Self::English => "english",
            Self::Wubi98 => "wubi98",
        }
    }

    pub fn from_journal_name(name: &str) -> Option<Self> {
        Some(match name {
            "pinyin" => Self::Pinyin,
            "wubi" => Self::Wubi,
            "quick" => Self::QuickPhrase,
            "english" => Self::English,
            "wubi98" => Self::Wubi98,
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

#[cfg(test)]
mod tests {
    use super::{SchemeSet, SchemeType};

    type Row = (&'static str, fn(SchemeType) -> bool, [bool; 5]);
    type Named = (&'static str, fn(SchemeType) -> bool);

    const SCHEMES: [SchemeType; 5] = [
        SchemeType::Quanpin,
        SchemeType::Shuangpin,
        SchemeType::Wubi,
        SchemeType::JapaneseRomaji,
        SchemeType::Korean,
    ];

    // Each row lists Quanpin, Shuangpin, Wubi, Japanese and Korean; the values are the behaviour these schemes had before the checks became predicates.
    #[test]
    fn predicates_keep_the_behaviour_of_the_existing_schemes() {
        let table: [Row; 26] = [
            (
                "is_chinese",
                SchemeType::is_chinese,
                [true, true, true, false, false],
            ),
            (
                "outputs_traditional_natively",
                SchemeType::outputs_traditional_natively,
                [false; 5],
            ),
            (
                "script_conversion_applies",
                SchemeType::script_conversion_applies,
                [true, true, true, false, false],
            ),
            (
                "uses_chinese_punctuation",
                SchemeType::uses_chinese_punctuation,
                [true, true, true, true, false],
            ),
            (
                "host_smart_punctuation",
                SchemeType::host_smart_punctuation,
                [true, true, true, false, false],
            ),
            (
                "widens_full_width",
                SchemeType::widens_full_width,
                [true, true, true, true, false],
            ),
            (
                "allows_english_emoji_mixing",
                SchemeType::allows_english_emoji_mixing,
                [true, true, false, false, false],
            ),
            (
                "opens_local_modes",
                SchemeType::opens_local_modes,
                [true, true, false, false, false],
            ),
            (
                "learns_into_main_dictionary",
                SchemeType::learns_into_main_dictionary,
                [true, true, true, false, false],
            ),
            (
                "learns_english_words",
                SchemeType::learns_english_words,
                [true, true, false, false, false],
            ),
            (
                "cloud_eligible",
                SchemeType::cloud_eligible,
                [true, true, false, true, false],
            ),
            (
                "shows_glosses",
                SchemeType::shows_glosses,
                [true, true, true, false, true],
            ),
            (
                "commits_on_blur",
                SchemeType::commits_on_blur,
                [false, false, false, false, true],
            ),
            (
                "holds_phrase_progress",
                SchemeType::holds_phrase_progress,
                [true, true, true, true, false],
            ),
            (
                "reranks_with_sentence_model",
                SchemeType::reranks_with_sentence_model,
                [true, true, false, false, false],
            ),
            (
                "draws_reading",
                SchemeType::draws_reading,
                [false, false, false, true, true],
            ),
            (
                "has_openable_candidate_list",
                SchemeType::has_openable_candidate_list,
                [false, false, false, false, true],
            ),
            (
                "cancel_keeps_composition",
                SchemeType::cancel_keeps_composition,
                [false, false, false, false, true],
            ),
            (
                "selection_completes",
                SchemeType::selection_completes,
                [false, false, false, true, true],
            ),
            (
                "follows_pinyin_candidates",
                SchemeType::follows_pinyin_candidates,
                [true, true, false, false, false],
            ),
            (
                "accepts_apostrophe",
                SchemeType::accepts_apostrophe,
                [true, true, false, true, false],
            ),
            (
                "locks_caret",
                SchemeType::locks_caret,
                [false, false, false, false, true],
            ),
            (
                "supports_fuzzy",
                SchemeType::supports_fuzzy,
                [true, true, false, false, false],
            ),
            (
                "supports_autocorrect",
                SchemeType::supports_autocorrect,
                [true, false, false, false, false],
            ),
            (
                "nine_key",
                SchemeType::nine_key,
                [true, false, false, false, false],
            ),
            (
                "helpcode",
                SchemeType::helpcode,
                [true, true, false, false, false],
            ),
        ];
        for (name, predicate, expected) in table {
            for (scheme, want) in SCHEMES.into_iter().zip(expected) {
                assert_eq!(predicate(scheme), want, "{name} for {scheme:?}");
            }
        }
    }

    #[test]
    fn only_the_chinese_typing_schemes_detect_urls() {
        for code in 0..=9 {
            let scheme = SchemeType::from_u8(code).expect("scheme code");
            let expected = matches!(
                scheme,
                SchemeType::Quanpin | SchemeType::Shuangpin | SchemeType::Wubi
            );
            assert_eq!(scheme.detects_urls(), expected, "{scheme:?}");
        }
    }

    #[test]
    fn local_input_mode_names_round_trip() {
        use super::LocalInputMode;
        // 穷举 match：新增变体时这里编译不过，提醒把它接进链条，`from_name` 的手写数组漏掉它时下面的断言就会失败。
        fn next(mode: LocalInputMode) -> Option<LocalInputMode> {
            Some(match mode {
                LocalInputMode::None => LocalInputMode::Unicode,
                LocalInputMode::Unicode => LocalInputMode::DateTime,
                LocalInputMode::DateTime => LocalInputMode::QuickPhrase,
                LocalInputMode::QuickPhrase => LocalInputMode::Emoji,
                LocalInputMode::Emoji => LocalInputMode::Kaomoji,
                LocalInputMode::Kaomoji => LocalInputMode::SuperJianpin,
                LocalInputMode::SuperJianpin => LocalInputMode::TemporaryEnglish,
                LocalInputMode::TemporaryEnglish => LocalInputMode::TemporaryJapanese,
                LocalInputMode::TemporaryJapanese => LocalInputMode::Expression,
                LocalInputMode::Expression => LocalInputMode::Command,
                LocalInputMode::Command => LocalInputMode::Mention,
                LocalInputMode::Mention => LocalInputMode::Url,
                LocalInputMode::Url => return None,
            })
        }
        let mut names = Vec::new();
        let mut mode = Some(LocalInputMode::None);
        while let Some(current) = mode {
            assert_eq!(
                LocalInputMode::from_name(current.name()),
                Some(current),
                "{current:?}"
            );
            names.push(current.name());
            mode = next(current);
        }
        assert_eq!(names.len(), 13);
        assert!(names.contains(&"url"));
        assert_eq!(LocalInputMode::from_name("unknown"), None);
    }

    #[test]
    fn existing_scheme_codes_are_unchanged() {
        for (code, scheme) in SCHEMES.into_iter().enumerate() {
            assert_eq!(scheme as u8, code as u8);
            assert_eq!(SchemeType::from_u8(code as u8), Some(scheme));
        }
    }

    #[test]
    fn vietnamese_round_trips_through_code_seven() {
        assert_eq!(SchemeType::Vietnamese as u8, 7);
        assert_eq!(SchemeType::from_u8(7), Some(SchemeType::Vietnamese));
        assert_eq!(SchemeType::Vietnamese.name(), "vietnamese");
        assert!(!SchemeType::Vietnamese.is_pinyin());
    }

    #[test]
    fn tibetan_round_trips_through_code_eight() {
        assert_eq!(SchemeType::Tibetan as u8, 8);
        assert_eq!(SchemeType::from_u8(8), Some(SchemeType::Tibetan));
        assert_eq!(SchemeType::Tibetan.name(), "tibetan");
        assert!(!SchemeType::Tibetan.is_pinyin());
    }

    #[test]
    fn cantonese_round_trips_through_code_five() {
        assert_eq!(SchemeType::Cantonese as u8, 5);
        assert_eq!(SchemeType::from_u8(5), Some(SchemeType::Cantonese));
        assert_eq!(SchemeType::Cantonese.name(), "cantonese");
        assert!(!SchemeType::Cantonese.is_pinyin());
    }

    #[test]
    fn zhuyin_round_trips_through_code_six() {
        assert_eq!(SchemeType::Zhuyin as u8, 6);
        assert_eq!(SchemeType::from_u8(6), Some(SchemeType::Zhuyin));
        assert_eq!(SchemeType::Zhuyin.name(), "zhuyin");
        assert!(!SchemeType::Zhuyin.is_pinyin());
    }

    #[test]
    fn stroke_round_trips_through_code_nine() {
        assert_eq!(SchemeType::Stroke as u8, 9);
        assert_eq!(SchemeType::from_u8(9), Some(SchemeType::Stroke));
        assert_eq!(SchemeType::Stroke.name(), "stroke");
        assert!(!SchemeType::Stroke.is_pinyin());
        assert_eq!(SchemeType::from_u8(10), None);
    }

    // 笔画逐项照抄粤拼的取值，只有两处不同：笔画没有音节，所以不接受 `'` 分隔；预编辑画的是笔画字形，所以和注音一样由 `reading` 带给宿主。
    #[test]
    fn stroke_predicates() {
        let scheme = SchemeType::Stroke;
        let on: [Named; 6] = [
            ("is_chinese", SchemeType::is_chinese),
            (
                "outputs_traditional_natively",
                SchemeType::outputs_traditional_natively,
            ),
            (
                "uses_chinese_punctuation",
                SchemeType::uses_chinese_punctuation,
            ),
            ("host_smart_punctuation", SchemeType::host_smart_punctuation),
            ("widens_full_width", SchemeType::widens_full_width),
            ("draws_reading", SchemeType::draws_reading),
        ];
        let off: [Named; 20] = [
            (
                "cancel_keeps_composition",
                SchemeType::cancel_keeps_composition,
            ),
            (
                "script_conversion_applies",
                SchemeType::script_conversion_applies,
            ),
            (
                "allows_english_emoji_mixing",
                SchemeType::allows_english_emoji_mixing,
            ),
            ("opens_local_modes", SchemeType::opens_local_modes),
            (
                "learns_into_main_dictionary",
                SchemeType::learns_into_main_dictionary,
            ),
            ("learns_english_words", SchemeType::learns_english_words),
            ("cloud_eligible", SchemeType::cloud_eligible),
            ("shows_glosses", SchemeType::shows_glosses),
            ("commits_on_blur", SchemeType::commits_on_blur),
            ("holds_phrase_progress", SchemeType::holds_phrase_progress),
            (
                "reranks_with_sentence_model",
                SchemeType::reranks_with_sentence_model,
            ),
            (
                "has_openable_candidate_list",
                SchemeType::has_openable_candidate_list,
            ),
            ("selection_completes", SchemeType::selection_completes),
            (
                "follows_pinyin_candidates",
                SchemeType::follows_pinyin_candidates,
            ),
            ("accepts_apostrophe", SchemeType::accepts_apostrophe),
            ("locks_caret", SchemeType::locks_caret),
            ("supports_fuzzy", SchemeType::supports_fuzzy),
            ("supports_autocorrect", SchemeType::supports_autocorrect),
            ("nine_key", SchemeType::nine_key),
            ("helpcode", SchemeType::helpcode),
        ];
        for (name, predicate) in on {
            assert!(predicate(scheme), "{name}");
        }
        for (name, predicate) in off {
            assert!(!predicate(scheme), "{name}");
        }
        // 除上面两处外，每个谓词都与粤拼相同。
        for (name, predicate) in on.into_iter().chain(off) {
            if name == "accepts_apostrophe" || name == "draws_reading" {
                assert_ne!(
                    predicate(scheme),
                    predicate(SchemeType::Cantonese),
                    "{name}"
                );
            } else {
                assert_eq!(
                    predicate(scheme),
                    predicate(SchemeType::Cantonese),
                    "{name}"
                );
            }
        }
    }

    // Zhuyin is a Chinese scheme writing Traditional text, with only its Shift overlay as Chinese punctuation; its converted text is the reading the host draws, the caret stays at the end, candidates appear only in the list the user opens, the conversion commits on blur, and it learns nothing.
    #[test]
    fn zhuyin_predicates() {
        let scheme = SchemeType::Zhuyin;
        let on: [Named; 9] = [
            ("is_chinese", SchemeType::is_chinese),
            (
                "cancel_keeps_composition",
                SchemeType::cancel_keeps_composition,
            ),
            (
                "outputs_traditional_natively",
                SchemeType::outputs_traditional_natively,
            ),
            (
                "uses_chinese_punctuation",
                SchemeType::uses_chinese_punctuation,
            ),
            ("widens_full_width", SchemeType::widens_full_width),
            ("commits_on_blur", SchemeType::commits_on_blur),
            ("draws_reading", SchemeType::draws_reading),
            (
                "has_openable_candidate_list",
                SchemeType::has_openable_candidate_list,
            ),
            ("locks_caret", SchemeType::locks_caret),
        ];
        let off: [Named; 17] = [
            (
                "script_conversion_applies",
                SchemeType::script_conversion_applies,
            ),
            ("host_smart_punctuation", SchemeType::host_smart_punctuation),
            (
                "allows_english_emoji_mixing",
                SchemeType::allows_english_emoji_mixing,
            ),
            ("opens_local_modes", SchemeType::opens_local_modes),
            (
                "learns_into_main_dictionary",
                SchemeType::learns_into_main_dictionary,
            ),
            ("learns_english_words", SchemeType::learns_english_words),
            ("cloud_eligible", SchemeType::cloud_eligible),
            ("shows_glosses", SchemeType::shows_glosses),
            ("holds_phrase_progress", SchemeType::holds_phrase_progress),
            (
                "reranks_with_sentence_model",
                SchemeType::reranks_with_sentence_model,
            ),
            ("selection_completes", SchemeType::selection_completes),
            (
                "follows_pinyin_candidates",
                SchemeType::follows_pinyin_candidates,
            ),
            ("accepts_apostrophe", SchemeType::accepts_apostrophe),
            ("supports_fuzzy", SchemeType::supports_fuzzy),
            ("supports_autocorrect", SchemeType::supports_autocorrect),
            ("nine_key", SchemeType::nine_key),
            ("helpcode", SchemeType::helpcode),
        ];
        for (name, predicate) in on {
            assert!(predicate(scheme), "{name}");
        }
        for (name, predicate) in off {
            assert!(!predicate(scheme), "{name}");
        }
    }

    // Cantonese is a Chinese scheme writing Traditional text as stored, with Chinese punctuation and `'` boundaries; it learns nothing, holds no phrase progress and has none of the pinyin machinery.
    #[test]
    fn cantonese_predicates() {
        let scheme = SchemeType::Cantonese;
        let on: [Named; 6] = [
            ("is_chinese", SchemeType::is_chinese),
            (
                "outputs_traditional_natively",
                SchemeType::outputs_traditional_natively,
            ),
            (
                "uses_chinese_punctuation",
                SchemeType::uses_chinese_punctuation,
            ),
            ("host_smart_punctuation", SchemeType::host_smart_punctuation),
            ("widens_full_width", SchemeType::widens_full_width),
            ("accepts_apostrophe", SchemeType::accepts_apostrophe),
        ];
        let off: [Named; 20] = [
            (
                "cancel_keeps_composition",
                SchemeType::cancel_keeps_composition,
            ),
            (
                "script_conversion_applies",
                SchemeType::script_conversion_applies,
            ),
            (
                "allows_english_emoji_mixing",
                SchemeType::allows_english_emoji_mixing,
            ),
            ("opens_local_modes", SchemeType::opens_local_modes),
            (
                "learns_into_main_dictionary",
                SchemeType::learns_into_main_dictionary,
            ),
            ("learns_english_words", SchemeType::learns_english_words),
            ("cloud_eligible", SchemeType::cloud_eligible),
            ("shows_glosses", SchemeType::shows_glosses),
            ("commits_on_blur", SchemeType::commits_on_blur),
            ("holds_phrase_progress", SchemeType::holds_phrase_progress),
            (
                "reranks_with_sentence_model",
                SchemeType::reranks_with_sentence_model,
            ),
            ("draws_reading", SchemeType::draws_reading),
            (
                "has_openable_candidate_list",
                SchemeType::has_openable_candidate_list,
            ),
            ("selection_completes", SchemeType::selection_completes),
            (
                "follows_pinyin_candidates",
                SchemeType::follows_pinyin_candidates,
            ),
            ("locks_caret", SchemeType::locks_caret),
            ("supports_fuzzy", SchemeType::supports_fuzzy),
            ("supports_autocorrect", SchemeType::supports_autocorrect),
            ("nine_key", SchemeType::nine_key),
            ("helpcode", SchemeType::helpcode),
        ];
        for (name, predicate) in on {
            assert!(predicate(scheme), "{name}");
        }
        for (name, predicate) in off {
            assert!(!predicate(scheme), "{name}");
        }
    }

    // 越南文和藏文都在组字里拼出一个词（音节串），没有候选也不学习：失焦时上屏，光标停在末尾，第一次 Esc 显示原文，中文相关的特性全部关闭。
    #[test]
    fn vietnamese_and_tibetan_predicates() {
        let on: [Named; 4] = [
            (
                "cancel_keeps_composition",
                SchemeType::cancel_keeps_composition,
            ),
            ("commits_on_blur", SchemeType::commits_on_blur),
            ("locks_caret", SchemeType::locks_caret),
            ("selection_completes", SchemeType::selection_completes),
        ];
        let off: [Named; 22] = [
            ("is_chinese", SchemeType::is_chinese),
            (
                "outputs_traditional_natively",
                SchemeType::outputs_traditional_natively,
            ),
            (
                "script_conversion_applies",
                SchemeType::script_conversion_applies,
            ),
            (
                "uses_chinese_punctuation",
                SchemeType::uses_chinese_punctuation,
            ),
            ("host_smart_punctuation", SchemeType::host_smart_punctuation),
            ("widens_full_width", SchemeType::widens_full_width),
            (
                "allows_english_emoji_mixing",
                SchemeType::allows_english_emoji_mixing,
            ),
            ("opens_local_modes", SchemeType::opens_local_modes),
            (
                "learns_into_main_dictionary",
                SchemeType::learns_into_main_dictionary,
            ),
            ("learns_english_words", SchemeType::learns_english_words),
            ("cloud_eligible", SchemeType::cloud_eligible),
            ("shows_glosses", SchemeType::shows_glosses),
            ("holds_phrase_progress", SchemeType::holds_phrase_progress),
            (
                "reranks_with_sentence_model",
                SchemeType::reranks_with_sentence_model,
            ),
            ("draws_reading", SchemeType::draws_reading),
            (
                "has_openable_candidate_list",
                SchemeType::has_openable_candidate_list,
            ),
            (
                "follows_pinyin_candidates",
                SchemeType::follows_pinyin_candidates,
            ),
            ("accepts_apostrophe", SchemeType::accepts_apostrophe),
            ("supports_fuzzy", SchemeType::supports_fuzzy),
            ("supports_autocorrect", SchemeType::supports_autocorrect),
            ("nine_key", SchemeType::nine_key),
            ("helpcode", SchemeType::helpcode),
        ];
        for scheme in [SchemeType::Vietnamese, SchemeType::Tibetan] {
            for (name, predicate) in on {
                assert!(predicate(scheme), "{name} for {scheme:?}");
            }
            for (name, predicate) in off {
                assert!(!predicate(scheme), "{name} for {scheme:?}");
            }
        }
    }

    /// `SchemeSet::ALL` 正好是 `from_u8` 认得的全部方案：加了方案却没改 `SchemeSet::LAST` 时，新方案会落在 `ALL` 之外，full 版的会话就切不到它。
    #[test]
    fn scheme_set_all_covers_every_scheme() {
        for value in 0..=u8::MAX {
            match SchemeType::from_u8(value) {
                Some(scheme) => assert!(
                    SchemeSet::ALL.contains(scheme),
                    "{scheme:?} is outside SchemeSet::ALL"
                ),
                None => assert!(
                    u32::from(value) >= u16::BITS || SchemeSet::ALL.0 & (1 << value) == 0,
                    "SchemeSet::ALL sets bit {value}, which names no scheme"
                ),
            }
        }
    }

    #[test]
    fn only_pinyin_and_wubi_read_the_main_dictionary() {
        assert!(SchemeSet::ALL.reads_main_dictionary());
        assert!(!SchemeSet::EMPTY.reads_main_dictionary());
        for value in 0..=u8::MAX {
            let Some(scheme) = SchemeType::from_u8(value) else {
                continue;
            };
            let reads = matches!(
                scheme,
                SchemeType::Quanpin | SchemeType::Shuangpin | SchemeType::Wubi
            );
            assert_eq!(
                SchemeSet::of(&[scheme]).reads_main_dictionary(),
                reads,
                "{scheme:?}"
            );
        }
        assert!(!SchemeSet::of(&[
            SchemeType::JapaneseRomaji,
            SchemeType::Vietnamese,
            SchemeType::Tibetan,
        ])
        .reads_main_dictionary());
    }
}
