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
    /// emoji 行，也用于混排的符号行（打拼音出符号，`local::symbol_candidates`）：两者选中时都整段上屏、结束组字，不学习、不调频，也不能置顶、固定或删除。
    Emoji = 6,
    Kaomoji = 7,
    Generated = 8,
    Fallback = 9,
    /// A lattice path the keyboard (speed) sentence model picked.
    NeuralKeyboard = 11,
    /// AI 候选的占位行（#6888），已退役：共享内核不再产生它，AI 回答到达时第一条直接插在第一页最后一格（`.agents/notes/implemented/feature/2026-10-16-ai-candidate-last-slot.md`）。值保留不复用；引擎和宿主对它的处理（不能上屏、不学习、不计入统计、画成暗色）照旧，旧版本留下的行仍按这些规则处理。
    AiPending = 12,
    /// 日文混输（实验选项，默认关）混进全拼、双拼候选的那一行：整段输入按罗马字转换出的最佳日文。它不属于下面任何一个谓词，所以选中时整段上屏并结束组字（与混输英文同一条路），不调词频、不记整句、不进个人上下文，也不能置顶、固定或删除；宿主不对它做简繁转换。
    MixedJapanese = 13,
    /// 脚本插件在自己的 `/` 指令里给出的行，只在桌面产生（宿主经 `Session::apply_plugin_command_rows` 写进来）。它不属于下面任何一个谓词：选中时整段上屏并结束组字，不调词频、不记整句、不进个人上下文，不能置顶、固定或删除；runtime 不把它的上屏喂进 AI 上文，宿主不为它请求释义或翻译，上屏也不转全角。
    Plugin = 14,
    /// 云候选尚在请求中；它占据云候选的固定位置，回答到达时由第一条云候选替换。值追加在已有 ABI 之后，不能复用旧值。
    CloudPending = 15,
    /// Android 联系人导入提供的手机号候选；选中后直接上屏，不学习、不管理。
    Contact = 16,
    /// octagram `.gram` 语法模型参与词格解码时，解码首选的整句行。句子仍是词格
    /// 解出来的，搭配强度表只负责逐边加分；落库/学习判定与 `Generated` 同类。
    Collocation = 17,
    /// 连续多语言转换的完整路径；整段消费，不调频、不学习跨语言整句，也不参与候选管理。
    Multilingual = 18,
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
            12 => Self::AiPending,
            13 => Self::MixedJapanese,
            14 => Self::Plugin,
            15 => Self::CloudPending,
            16 => Self::Contact,
            17 => Self::Collocation,
            18 => Self::Multilingual,
            _ => return None,
        })
    }

    pub fn is_online(self) -> bool {
        matches!(self, Self::CloudSuggestion | Self::AiSuggestion)
    }

    pub fn is_cloud_pending(self) -> bool {
        self == Self::CloudPending
    }

    /// AI 行：AI 候选和已退役的占位行。
    pub fn holds_ai_seat(self) -> bool {
        matches!(self, Self::AiSuggestion | Self::AiPending)
    }

    /// 由在线座位规则摆放的行：云候选、AI 候选和已退役的 AI 占位行。
    pub fn takes_online_seat(self) -> bool {
        self.is_online() || matches!(self, Self::AiPending | Self::CloudPending)
    }

    /// 整句猜出来的行：词格 `Generated`/`Collocation`、解码器 `Fallback`。
    /// 它们的选中都整段上屏、按词边界记进个人上下文、不落词库调频。
    pub fn is_generated_or_fallback(self) -> bool {
        matches!(self, Self::Generated | Self::Fallback | Self::Collocation)
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
    /// 韩语两套式或罗马字布局：音节在预编辑中组合，稳定前缀逐步上屏；`Command::ConvertHanja` 打开当前音节的汉字候选。
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

    /// 没有组字时，Shift+字母打开按拼音拼写查的本地模式（E、M、J、R）和计算模式 V；K、C、U、T、Y、S 和 `/`、`@` 由 `opens_table_modes` 判断（`InputSession::local_mode_for_entry`）。
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

    /// 没有组字时，Shift+K 打开短语模式，`/` 和 `@` 打开指令和提及模式，Shift+C、U、T、Y、S 打开拆字、Unicode、日期时间、临时英文和符号模式；五笔也提供这些不经过方案拼写的入口。
    pub const fn opens_table_modes(self) -> bool {
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

    /// 「只出单字」打开时，候选里去掉含汉字的词和整句，只留单个汉字。只用于选一个候选就上屏、剩下的拼写接着组字的方案；注音的候选列表是给组字里的一段改字，笔画本来只出单字。
    pub const fn filters_to_single_characters(self) -> bool {
        match self {
            Self::Quanpin | Self::Shuangpin | Self::Wubi | Self::Cantonese => true,
            Self::JapaneseRomaji
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
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

    /// 九键网格能拼出该方案的音节：全拼用拼音九宫格，注音用注音九键。
    pub const fn nine_key(self) -> bool {
        match self {
            Self::Quanpin | Self::Zhuyin => true,
            Self::Shuangpin
            | Self::Wubi
            | Self::JapaneseRomaji
            | Self::Korean
            | Self::Vietnamese
            | Self::Tibetan
            | Self::Cantonese
            | Self::Stroke => false,
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
    /// 全部方案：序号 0 到 `Self::LAST` 的每一位。
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

    /// 集合里是否有读 `msime-pinyin.db` 的方案：全拼、双拼和五笔的候选、学习和用户词都在它里面（五笔混拼的拼音行也是）。没有这三个方案的集合（例如只有日文、越南文或藏文的版本）随包不带 `msime-pinyin.db`：代次里没有它的工作副本，用户词库只剩英文词。
    pub const fn reads_main_dictionary(self) -> bool {
        self.contains(SchemeType::Quanpin)
            || self.contains(SchemeType::Shuangpin)
            || self.contains(SchemeType::Wubi)
    }

    /// 集合里是否有离不开 `msime-english.db` 的方案：全拼、双拼的英文混输、九键英文和回车学英文词，全拼、双拼、五笔的临时英文（Shift+Y）都读它，所以有这三个方案时准备代次要求资源目录里有它。其余方案（日文、韩语、越南语、藏文、粤语、注音、笔画）自己不读它，只有宿主打开专用英文（`Session::set_dedicated_english`）时才查英文补全、学回车上屏的词：这样的集合不要求它，资源目录里带了就照样复制进代次，没带时代次里没有它，专用英文只出键入的字母、回车不学词，个人词典的英文词也无处可存（编辑以 `NO_ENGLISH_DICTIONARY` 失败）。读 `msime-pinyin.db` 的集合（[`Self::reads_main_dictionary`]）一定要求它。
    pub const fn requires_english_dictionary(self) -> bool {
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

/// 双拼键位方案，序号是宿主 ABI 值（`EngineOptions::shuangpin_profile`），新增方案只追加。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ShuangpinProfileKind {
    #[default]
    Xiaohe = 0,
    Ziranma = 1,
    Shoudao = 2,
    Microsoft = 3,
    /// 用户自定义的键位，表在 [`ShuangpinCustomTable`] 里（`EngineOptions::shuangpin_custom_profile`、`SessionOptions::shuangpin_custom_profile`），没有内置的表。
    Custom = 4,
    SmartAbc = 5,
}

impl ShuangpinProfileKind {
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Xiaohe,
            1 => Self::Ziranma,
            2 => Self::Shoudao,
            3 => Self::Microsoft,
            4 => Self::Custom,
            5 => Self::SmartAbc,
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
            "custom" => Self::Custom,
            "smart_abc" => Self::SmartAbc,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Xiaohe => "xiaohe",
            Self::Ziranma => "ziranma",
            Self::Shoudao => "shoudao",
            Self::Microsoft => "microsoft",
            Self::Custom => "custom",
            Self::SmartAbc => "smart_abc",
        }
    }
}

/// 用户自定义双拼方案的键位表（共享偏好 `shuangpin_custom_profile`）。三张表与内置方案同构：`initials` 只写三个多字母声母 `zh` `ch` `sh`，单字母声母仍在自己的字母键上；`finals` 把 33 个韵母各放到一个键上；`zero_initials` 给 12 个零声母音节各一个两键编码。键是小写字母或 `;`，`ü` 写作 `v`。表合不合法由 Engine 判定（见 `shuangpin::custom`），顺序无关。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShuangpinCustomTable {
    pub initials: Vec<(String, String)>,
    pub finals: Vec<(String, String)>,
    pub zero_initials: Vec<(String, String)>,
}

/// 九键双拼方案（#6572）。序号是宿主 ABI 值，名字是共享偏好 `nine_key_shuangpin_profile` 的取值。内置方案只收录能找到完整、前后一致的公开描述的那几个，来源见 `.agents/notes/implemented/feature/2026-10-10-nine-key-shuangpin.md`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum NineKeyShuangpinKind {
    /// 米旮旯 2。
    Migala2 = 0,
    /// 米旮旯 3（用 0 键的那一版）。
    #[default]
    Migala3 = 1,
    /// 辜氏 2 低重码。
    Gushi2Low = 2,
    /// 用户自己的表，在 [`NineKeyShuangpinTable`] 里，没有内置的表。
    Custom = 3,
}

impl NineKeyShuangpinKind {
    /// 有内置表的方案，按序号。
    pub const BUILTIN: [Self; 3] = [Self::Migala2, Self::Migala3, Self::Gushi2Low];

    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Migala2,
            1 => Self::Migala3,
            2 => Self::Gushi2Low,
            3 => Self::Custom,
            _ => return None,
        })
    }

    /// 只认确切的名字，不认识的名字返回 `None`，由调用方决定怎么处理。
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "migala2" => Self::Migala2,
            "migala3" => Self::Migala3,
            "gushi2_low" => Self::Gushi2Low,
            "custom" => Self::Custom,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Migala2 => "migala2",
            Self::Migala3 => "migala3",
            Self::Gushi2Low => "gushi2_low",
            Self::Custom => "custom",
        }
    }
}

/// 九键双拼的键位表（共享偏好 `nine_key_shuangpin_custom_profile`），形状与 [`ShuangpinCustomTable`] 相同，键换成数字：`initials` 给 23 个声母（含 `zh` `ch` `sh`）各一个数字，`finals` 给 33 个韵母各一个数字，`zero_initials` 给 12 个零声母音节各一个两位数字编码。数字可以是 `0`–`9`，`ü` 写作 `v`。表合不合法由 Engine 判定（见 `nine_key_shuangpin`），顺序无关。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NineKeyShuangpinTable {
    pub initials: Vec<(String, String)>,
    pub finals: Vec<(String, String)>,
    pub zero_initials: Vec<(String, String)>,
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

pub use crate::shared_string::SharedString;

/// One candidate row. Every field the goldens record is here from the start, including the ones the overlays added.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WordItem {
    /// The code the row matched, as typed. Composition advance consumes exactly this.
    pub pinyin: SharedString,
    /// The complete quanpin key from the dictionary, used when a phrase is persisted.
    pub canonical_pinyin: SharedString,
    pub word: SharedString,
    /// Dictionary weight; for lattice rows `trunc(log_prob * 1000)`, which is not a sort key.
    pub weight: i64,
    pub source: CandidateSource,
    /// The scheme whose provider produced the row; in mixed wubi it routes learning and removal to the right table.
    pub scheme: SchemeType,
    /// 0 when unfixed, otherwise the fixed slot 1..=5.
    pub fixed_position: i32,
    pub fuzzy: bool,
    /// Non-empty for a row read from an autocorrected spelling; holds the folded letters the user typed.
    pub corrected_from: SharedString,
    /// Only whole-sentence association rows (lattice and neural sentences).
    pub sentence_association: bool,
    /// Word boundaries of a decoded sentence, for personal context learning.
    pub sentence_words: Vec<String>,
    /// 26 键双拼按触点计价的纠错整句，而且落点支持把它留在引擎给的座位上（分数不比字面整句低出 `TouchTuning::show_margin`）：runtime 不把它当整句的次选读法挪到表尾。其余行永远为假。
    pub touch_supported: bool,
    /// 26 键双拼按触点计价的纠错整句，触点支持而且明显比字面整句好，按首选规则（`shuangpin::dictionary` 的 `confident_correction`）坐了第一位。宿主据此给首选画强调色、读屏念「已纠正」（视图里的 `confident_correction`）。带它的行同时 `touch_supported`；其余行永远为假。
    pub confident_correction: bool,
}

impl WordItem {
    /// 只把真实词网格整句的分交给独立模型；字典权重和其它来源没有同一计分单位。产品门面 `ordering` 的输入，和它一样不进文档。
    #[doc(hidden)]
    pub fn static_evidence(&self) -> Option<crate::ordering::StaticEvidence> {
        if !self.sentence_association
            || !matches!(
                self.source,
                CandidateSource::Generated
                    | CandidateSource::NeuralKeyboard
                    | CandidateSource::Collocation
            )
            || self.canonical_pinyin.is_empty()
            || self.word.is_empty()
        {
            return None;
        }
        let syllables = self.canonical_pinyin.split('\'').count();
        let bonus =
            (crate::lattice::decode::PHRASE_LENGTH_BONUS * 1000.0 * syllables as f64) as i64;
        Some(crate::ordering::StaticEvidence {
            log_prob_milli_nats: self.weight.saturating_sub(bonus),
            characters: self.word.chars().count(),
        })
    }

    pub fn new(
        pinyin: impl Into<String>,
        word: impl Into<String>,
        weight: i64,
        source: CandidateSource,
        canonical_pinyin: impl Into<String>,
    ) -> Self {
        Self {
            pinyin: SharedString::from(Into::<String>::into(pinyin)),
            canonical_pinyin: SharedString::from(Into::<String>::into(canonical_pinyin)),
            word: SharedString::from(Into::<String>::into(word)),
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
#[derive(Debug, Clone, PartialEq)]
pub struct SentenceAssociationOptions {
    pub word_lattice: bool,
    /// The Engine's own keyboard-model rerank: nine-key reranks once across its segmentations, the quanpin and shuangpin dictionaries rerank inside the lattice, and the model's pick becomes a `NeuralKeyboard` row after the lattice best. Hosts leave it off: 整句联想「增强」 (the `neural_keyboard` preference) mounts the input runtime's per-keystroke reranker instead (host-api `keyboard_reranker_enabled`). It stays for evals and experiments that build the options directly (`convert_eval --keyboard-neural`).
    pub neural_keyboard: bool,
    pub show_next_on_duplicate: bool,
    /// octagram `.gram` 语法模型的路径（宿主按模型包解析好后传入）；空 = 关闭。
    /// 路径无效时词典静默退回无搭配打分，与 `msime-bigram.bin` 缺失的回退一致。
    pub collocation_model: std::path::PathBuf,
    /// 搭配项的标定系数（ln 域结果由装配方换算后按它缩放）；0 = 关闭。
    pub collocation_weight: f64,
    /// 解码最优路径是否作为 `Collocation` 整句行显示；关掉时搭配分仍参与解码。
    pub collocation_rerank: bool,
    /// octagram 六参数；默认对齐万象现行，见 `crate::lattice::octagram::GramConfig`。
    pub collocation_grammar: crate::lattice::octagram::GramConfig,
}

impl Default for SentenceAssociationOptions {
    fn default() -> Self {
        Self {
            word_lattice: true,
            neural_keyboard: false,
            show_next_on_duplicate: false,
            collocation_model: std::path::PathBuf::new(),
            collocation_weight: 1.0,
            collocation_rerank: false,
            collocation_grammar: crate::lattice::octagram::GramConfig::default(),
        }
    }
}

/// 默认的首页候选行数：宿主每页候选数（`client-core` 的 `candidate_page_size`）的缺省值。引擎只把它当「第一页能放几行」用。
pub const DEFAULT_FIRST_PAGE_ROWS: usize = 6;

/// 一次请求带的首页行数。`QueryRequest` 派生 `Default`，裸 `usize` 会默认成 0 从而把补全词整组关掉，所以用带默认值的新类型；`new` 把 0 归一成 1。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirstPageRows(usize);

impl FirstPageRows {
    pub const fn new(rows: usize) -> Self {
        Self(if rows == 0 { 1 } else { rows })
    }

    pub const fn get(self) -> usize {
        self.0
    }
}

impl Default for FirstPageRows {
    fn default() -> Self {
        Self(DEFAULT_FIRST_PAGE_ROWS)
    }
}

/// 双拼辅助码的触发方式开关（Windows v0.11.1 `[helpcode]` 组的双拼专属键），随 `QueryRequest` 交给双拼引擎。默认值与 Windows 出厂配置一致；`direct` 控制末尾直接，`trigger_*` 控制末尾间接，`mid_sentence*` 控制句中间接，`direct_helpcode*` 控制逐音节直接。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShuangpinHelpcodeModes {
    /// 只在 Tab 后输入辅助码；保留其它触发方式的设置，关闭后恢复。
    pub manual_tab: bool,
    /// 末尾直接：完整音节基座后的裸字母按辅助码读。关闭后只认触发键开启的段。
    pub direct: bool,
    /// 末尾间接触发键：反引号段。
    pub trigger_backtick: bool,
    /// 末尾间接触发键：分号段。
    pub trigger_semicolon: bool,
    /// 末尾间接触发键：段首大写。
    pub trigger_uppercase: bool,
    /// 句中（间接）辅助码总开关。
    pub mid_sentence: bool,
    pub mid_sentence_backtick: bool,
    pub mid_sentence_semicolon: bool,
    pub mid_sentence_uppercase: bool,
    /// 句中（直接）「万象式」总开关。
    pub direct_helpcode: bool,
    pub direct_helpcode_slash: bool,
    pub direct_helpcode_uppercase: bool,
}

impl Default for ShuangpinHelpcodeModes {
    /// Windows v0.11.1 出厂值：末尾直接开、末尾间接的分号开，句中间接与万象式直接关。
    fn default() -> Self {
        Self {
            manual_tab: false,
            direct: true,
            trigger_backtick: false,
            trigger_semicolon: true,
            trigger_uppercase: false,
            mid_sentence: false,
            mid_sentence_backtick: true,
            mid_sentence_semicolon: false,
            mid_sentence_uppercase: false,
            direct_helpcode: false,
            direct_helpcode_slash: true,
            direct_helpcode_uppercase: false,
        }
    }
}

/// 一个音节上的辅助码约束（`core/syllable_helpcode.h` 的 `SyllableHelpcode`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SyllableHelpcode {
    /// 受约束的音节，按去掉辅助码段之后的双拼切分计，0 起。
    pub syllable: usize,
    /// 第一码，小写字母；0 表示这一段还没敲码。
    pub first: u8,
    /// 第二码，小写字母；0 表示没有第二码，只按第一码筛。
    pub second: u8,
}

/// What a scheme turns its composition into for the providers. Built fresh on every refresh; providers never mutate it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QueryRequest {
    pub scheme: SchemeType,
    /// Lowercased, apostrophes kept.
    pub raw_input: String,
    pub raw_input_with_cases: String,
    /// 输入串里带辅助码段（'`' 句中、'|' 末尾）时，`raw_input` / `raw_input_with_cases` 是每段换成
    /// `'`（末尾的末尾段去掉）的去段串，原串（用户敲的原样）在这里。没有段时为空。
    pub raw_input_with_helpcode_marks: String,
    /// 句中辅助码约束；段敲进串里就带着，是否参与筛查看 `shuangpin_helpcode_modes.mid_sentence`。
    pub syllable_helpcodes: Vec<SyllableHelpcode>,
    /// 末尾段敲下的辅码（小写，至多两个），只在段位于输入串末尾时非空；查询时一码重排、两码筛选。
    pub end_helpcodes: String,
    /// 去段串（`raw_input_with_cases`）到原串（`raw_input_with_helpcode_marks`）的下标映射，末尾
    /// 多一项等于原串长度；选词推进与音节边界用它把去段位置换回显示位置。
    pub helpcode_source_index: Vec<usize>,
    /// 每段原样的文本及它挂在哪个音节后面，预编辑据此还原显示。
    pub helpcode_decorations: Vec<(usize, String)>,
    /// The dictionary key: no apostrophes.
    pub normalized_input: String,
    pub raw_segmentation: String,
    pub normalized_segmentation: String,
    pub segmentation: String,
    pub enable_shuangpin_helpcode: bool,
    /// 双拼辅助码的触发方式；`enable_shuangpin_helpcode` 为真时才读。
    pub shuangpin_helpcode_modes: ShuangpinHelpcodeModes,
    pub enable_quanpin_helpcode: bool,
    /// 一码辅助码是否严格排除未命中的候选；默认关闭以保持历史排序。
    pub strict_quanpin_helpcode: bool,
    /// 会话的「拼音纠错」开关。名字带 quanpin 是历史原因：双拼也按它决定是否给纠错整句。
    pub enable_quanpin_autocorrect_transposition: bool,
    pub enable_quanpin_autocorrect_neighbor: bool,
    pub fuzzy_pinyin: FuzzyPinyinOptions,
    /// Hand back every whole-sentence reading rather than only the best.
    pub sentence_alternatives: bool,
    pub sentence_association: SentenceAssociationOptions,
    /// 首页能放下的行数：全长度组的精确同音词够这么多就不再并入更长的补全词。
    pub first_page_rows: FirstPageRows,
    /// Committed text the neural sentence models condition on.
    pub rescoring_context: String,
    /// Korean only: the Hanja list of the composing syllable is open, so the request is answered with its Hanja.
    pub korean_hanja: bool,
    /// 五笔通配：码里确实有 `z`，provider 改用模式匹配而不是前缀范围扫描（见 [`WubiInputOptions::z_wildcard`]）。
    pub wubi_z_wildcard: bool,
    /// 五笔 14 键：`normalized_input` 每一位是一组字母的代表字母（`wubi::grid`），provider 展开成这几组字母的每种组合再查码表。
    pub wubi_grid: bool,
    /// 26 键双拼每个按键的触点，与 `raw_input` 逐字节对齐，没有触点的位置为 `None`。只要不是双拼、没有注册几何、一个触点都没有、输入里有 `'` 或句中辅助码生效，就必须是空 Vec 而不是全 `None`：桌面请求因此与没有这个字段时逐字节相同。
    pub key_touches: Vec<Option<crate::touch::KeyTouch>>,
    /// 触点所在的那份几何；`key_touches` 为空时为 `None`。
    pub key_geometry: Option<std::sync::Arc<crate::touch::KeyGeometry>>,
    /// 按触点计价的参数。只有 Rust 侧（评测示例）能改，见 [`crate::host::Session::set_touch_tuning`]。
    pub touch_tuning: crate::touch::TouchTuning,
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

/// Engine-level composition commands (the C++ `metasequoia::Command`, plus `CommitRawWithoutLearning`). The host facade numbers them its own way (`host::Command`).
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
    /// 整句改字的光标左移一个字：组字的首选是一个字对一个音节的全拼或双拼整句时，第一次进入改字、光标停在最后一个字前，之后每次左移一个字；进不了改字时与 `MoveLeft` 相同。
    ConversionLeft = 12,
    /// 整句改字的光标右移一个字，最远到句末；不在改字里时与 `MoveRight` 相同。
    ConversionRight = 13,
    /// 像 `CommitRaw` 一样原样上屏，但不把上屏的字母学成英文词：专用英文里上屏预编辑原文并结束组字（`CommitRaw` 会学这个词），其余情况与 `CommitRaw` 相同，`SessionOptions::learn_typed_spellings_on_enter` 也不起作用。拼写纠错的「回车上屏原文就关掉这段输入的纠错」照常记下。由 [`crate::Session::command`] 处理，组字的各条内部路径收不到它。
    CommitRawWithoutLearning = 14,
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
            12 => Self::ConversionLeft,
            13 => Self::ConversionRight,
            14 => Self::CommitRawWithoutLearning,
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
            Self::ConversionLeft => "ConversionLeft",
            Self::ConversionRight => "ConversionRight",
            Self::CommitRawWithoutLearning => "CommitRawWithoutLearning",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        (0..=14)
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

    /// `diagnostic` 的稳定数字码（映射表见 `diagnostics::DIAGNOSTIC_CODES`）；没有诊断时为 None。
    pub fn diagnostic_code(&self) -> Option<crate::diagnostics::DiagnosticCode> {
        self.diagnostic
            .as_deref()
            .map(crate::diagnostics::DiagnosticCode::from_message)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CandidateEdge {
    FirstHan = 0,
    LastHan = 1,
}

impl CandidateEdge {
    /// 引擎以词定字使用的首个或末个汉字；没有汉字时返回空串。
    pub fn character(self, text: &str) -> &str {
        match self {
            Self::FirstHan => crate::text::first_han_char(text),
            Self::LastHan => crate::text::last_han_char(text),
        }
    }
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
    /// `C`：拆字反查，按部件的拼音找字（`huohuohuo` 得 焱）。
    Chaizi,
    /// `S`：符号模式，按分组码（`xl`、`sx`、`szq`、`jt`）或符号名（`alpha`、`infinity`）查符号。
    Symbol,
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
            Self::Chaizi => "chaizi",
            Self::Symbol => "symbol",
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
            Self::Chaizi,
            Self::Symbol,
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
            Self::TemporaryJapanese => "-",
            _ => "",
        }
    }

    /// Whether the mode's rows are text the engine generated rather than words the user spelled. Their commits are never learned and never counted as typing.
    pub fn generates_text(self) -> bool {
        matches!(self, Self::Expression | Self::Command | Self::Mention)
    }

    /// 去掉临时模式预编辑开头的入口字母（`Y`、`R`）。它是模式标记、不是用户打的字，所以进词库的词不能带它；回车原样上屏时它算文本（#7616），那一处不走这里。
    pub fn without_entry_marker(self, text: &str) -> &str {
        match self {
            Self::TemporaryEnglish | Self::TemporaryJapanese => text.get(1..).unwrap_or_default(),
            _ => text,
        }
    }

    /// 候选注释由 Engine 自己给出、不补辅助码的模式：V、`/`、`@` 的行不是按拼音拼出来的，辅助码说明不了它们；拆字的行注释的是字的读音，用户正是不会读这个字才来查，辅助码夹在读音里只会添乱。和 `generates_text` 分开，是因为拆字上屏的字是用户挑出来的，照常计入打字统计。
    pub fn annotates_rows(self) -> bool {
        matches!(
            self,
            Self::Expression | Self::Command | Self::Mention | Self::Chaizi
        )
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

/// Which local modes may be entered. The Shift+letter modes of the reference are on by default, symbol mode (`S`) among them, as the reference's factory configuration ships it on; expression (`V`) follows the reference's `v_mode` factory and is on, command (`/`) and mention (`@`) are off, because entering them takes a key that used to reach the host as text. 拆字 (`C`) 在共享偏好层默认开（对齐参考的 `radical_mode` 出厂值），host-api 把未写偏好的文档读作开。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalModeOptions {
    pub unicode: bool,
    /// Shift+T 日期时间快捷输入模式（参考 `utility.date_time_mode`）。行内日期时间混入普通候选由 `date_time_inline` 单独开关（参考 `utility.date_time_candidates`，出厂关）。
    pub date_time: bool,
    /// 全拼输入 `riqi`/`sj`/`xq`/`nongli` 时把当前日期、时间、星期或农历接在对应的词后面（参考 `utility.date_time_candidates`）。与 T 模式独立：T 模式关着时行内混入仍可开。
    pub date_time_inline: bool,
    /// 「📅日期」入口：日期时间混进普通候选时，第三位放一个入口，选中后候选窗换成这一组的全部格式（参考 `utility.date_time_menu`，出厂开）。依附 `date_time_inline`。
    pub date_time_menu: bool,
    pub quick_phrase: bool,
    pub emoji: bool,
    pub kaomoji: bool,
    pub super_jianpin: bool,
    pub temporary_english: bool,
    pub temporary_japanese: bool,
    pub expression: bool,
    pub command: bool,
    pub mention: bool,
    /// `C`：拆字反查。
    pub chaizi: bool,
    /// `S`：符号模式。
    pub symbol: bool,
    /// 总开关：没有组字时 Shift+字母（U/T/K/E/M/J/Y/R/V/C/S）能否进入对应的快捷模式。关掉后这些键照常打出大写字母，各模式自己的开关不变；`/` 和 `@` 不经过它。
    pub shift_letter_entry: bool,
}

impl Default for LocalModeOptions {
    fn default() -> Self {
        Self {
            unicode: true,
            date_time: true,
            date_time_inline: true,
            date_time_menu: true,
            quick_phrase: true,
            emoji: true,
            kaomoji: true,
            super_jianpin: true,
            temporary_english: true,
            temporary_japanese: true,
            expression: false,
            command: false,
            mention: false,
            chaizi: false,
            symbol: true,
            shift_letter_entry: true,
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

/// 一条脚本插件指令（`/` 模式）：插件在清单里声明的触发词和标题。宿主把会运行的插件声明的指令按执行顺序交给引擎，引擎按 `local::command::usable_plugin_commands` 决定哪些生效（用户指令表赢、插件先到先得），不照单全收。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginCommand {
    /// 2..=16 个小写 ASCII 字母，即 `/` 之后输入的触发词。
    pub trigger: String,
    /// 1..=8 个字符，由 runtime 画在插件注释格里，不进引擎的注释。
    pub title: String,
    /// 清单里写了 `arguments = "extended"`：完整打出触发词之后，参数除了小写字母还收数字和 [`crate::local::command::EXTENDED_ARGUMENT_SYMBOLS`]，这时宿主把数字当输入、不当选词键（见 `InputSession::spelling_symbols`）。
    pub extended: bool,
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
    /// 联系人手机号；普通名单为空，联系人候选命中姓名拼音时提交这一栏。
    pub phone: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnglishInputOptions {
    pub mixed_candidates: bool,
    /// 补全与英文领先的触发字符数，范围 1..=8；26 键中更短的完整词可选，但不抢占中文首选。
    pub minimum_prefix: usize,
    /// 中文状态下首页最多展示几个英文候选（Windows `general.cn_en_mixed_input_first_page_count`），范围 1..=3：第一行英文进优先位置之后，其余英文紧跟它继续占首页的空位，放不下的照旧接在列表末尾。
    pub first_page_count: usize,
}

impl Default for EnglishInputOptions {
    fn default() -> Self {
        Self {
            mixed_candidates: false,
            minimum_prefix: 2,
            first_page_count: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MixedExpressiveOptions {
    pub emoji_candidates: bool,
    pub kaomoji_candidates: bool,
    /// 打拼音出符号：整段组字正好拼出某个符号的名称时混入符号行（`local::symbol_candidates`）。
    pub symbol_candidates: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WubiInputOptions {
    /// Answer an unmatched wubi code with quanpin candidates for the same letters.
    pub mixed_pinyin: bool,
    /// 查询、学习和删除所用的码表版本。
    pub profile: WubiProfileKind,
    /// `z` 通配：码里的 `z` 匹配码表该位置的任意一个字母（含首位）。
    ///
    /// 与混输无关：混输里 `z` 是普通拼音字母，这里它是未知码位。两者都关时 `z` 不进码。
    pub z_wildcard: bool,
}

/// 在线请求的输入语义；缺省沿用原有的拼音及方案转换路径。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnlineInputMode {
    #[default]
    Pinyin,
    English,
}

impl OnlineInputMode {
    pub fn is_pinyin(&self) -> bool {
        *self == Self::Pinyin
    }
}

/// 宿主交给云或 AI 服务的查询，回包必须原样带回；引擎在插入候选前核对当前组字的每个字段。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OnlineQuery {
    pub mode: OnlineInputMode,
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

/// 插件指令要宿主去问的那一次：`/` 模式里输入以某个生效的插件触发词开头、这份预编辑串还没有缓存的行时才有。宿主把触发词和参数交给触发词的主人，再把结果连同这个值交回 `Session::apply_plugin_command_rows`；会话或预编辑串已经变了的回答被拒绝。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PluginCommandQuery {
    pub session_id: u64,
    /// 整段本地预编辑串，含开头的 `/`。
    pub preedit: String,
    pub trigger: String,
    /// 触发词后面的小写字母，可以为空（`/uuid`）。
    pub argument: String,
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
    /// 粤语，编码是不带声调的粤拼音节，以单个空格连接（与 `msime-cantonese.db` 的 `entries.key` 同一写法）。只记在用户日志里，没有工作表：粤语方案查词时从日志读出合进候选（`cantonese::UserWords`）。
    Cantonese = 5,
}

impl PersonalDictionaryKind {
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Pinyin,
            1 => Self::Wubi,
            2 => Self::QuickPhrase,
            3 => Self::English,
            4 => Self::Wubi98,
            5 => Self::Cantonese,
            _ => return None,
        })
    }

    /// 两版五笔共用编码规则（1..=4 个 a..=y 字母）与按码排序的写法，只是表不同。
    pub fn is_wubi(self) -> bool {
        matches!(self, Self::Wubi | Self::Wubi98)
    }

    /// 不写 `msime-pinyin.db` 的种类：英文词写在 `msime-english.db`，粤语词只记在日志里。代次里没有主词库时只有它们能编辑、列出。
    pub fn stored_without_main_dictionary(self) -> bool {
        matches!(self, Self::English | Self::Cantonese)
    }

    /// 五笔种类对应的 `msime-pinyin.db` 码表。
    pub fn wubi_table(self) -> Option<&'static str> {
        match self {
            Self::Wubi => Some(WubiProfileKind::Wubi86.table()),
            Self::Wubi98 => Some(WubiProfileKind::Wubi98.table()),
            Self::Pinyin | Self::QuickPhrase | Self::English | Self::Cantonese => None,
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
            Self::Cantonese => "cantonese",
        }
    }

    pub fn from_journal_name(name: &str) -> Option<Self> {
        Some(match name {
            "pinyin" => Self::Pinyin,
            "wubi" => Self::Wubi,
            "quick" => Self::QuickPhrase,
            "english" => Self::English,
            "wubi98" => Self::Wubi98,
            "cantonese" => Self::Cantonese,
            _ => return None,
        })
    }
}

/// A user's own dictionary entry. Pinyin keys are complete syllables joined by `'`; wubi 1..=4 letters; quick phrases 1..=32 letters or digits; English 1..=64 letters; 粤语是不带声调的粤拼音节，以单个空格连接。
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
    use super::{CandidateSource, SchemeSet, SchemeType, WordItem};

    /// 线上的来源编号按值往返；10 空着（原 `NeuralDesktop`，未发布就删了）。日文混输行（13）不属于任何谓词：上屏走整段结束组字的路，不学习，不坐在线座位。
    #[test]
    fn candidate_sources_round_trip_and_mixed_japanese_matches_no_predicate() {
        for value in 0..=u8::MAX {
            match CandidateSource::from_u8(value) {
                Some(source) => assert_eq!(source as u8, value),
                None => assert!(value == 10 || value > 14, "{value}"),
            }
        }
        let japanese = CandidateSource::MixedJapanese;
        assert_eq!(CandidateSource::from_u8(13), Some(japanese));
        assert!(!japanese.is_dictionary());
        assert!(!japanese.is_generated_or_fallback());
        assert!(!japanese.is_sentence_learning());
        assert!(!japanese.is_online());
        assert!(!japanese.takes_online_seat());
        assert!(!japanese.holds_ai_seat());
    }

    /// 插件指令行（14）同样不属于任何谓词：不学习、不坐在线座位。
    #[test]
    fn plugin_rows_match_no_predicate() {
        let plugin = CandidateSource::Plugin;
        assert_eq!(CandidateSource::from_u8(14), Some(plugin));
        assert_eq!(plugin as u8, 14);
        assert!(!plugin.is_dictionary());
        assert!(!plugin.is_generated_or_fallback());
        assert!(!plugin.is_sentence_learning());
        assert!(!plugin.is_online());
        assert!(!plugin.takes_online_seat());
        assert!(!plugin.holds_ai_seat());
    }

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
                LocalInputMode::Url => LocalInputMode::Chaizi,
                LocalInputMode::Chaizi => LocalInputMode::Symbol,
                LocalInputMode::Symbol => return None,
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
        assert_eq!(names.len(), 15);
        assert!(names.contains(&"url"));
        assert!(names.contains(&"chaizi"));
        assert!(names.contains(&"symbol"));
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

    // Zhuyin is a Chinese scheme writing Traditional text, with only its Shift overlay as Chinese punctuation; its converted text is the reading the host draws, the caret stays at the end, candidates appear only in the list the user opens, the conversion commits on blur, and it learns nothing. 注音九键也拼它的音节，所以 `nine_key` 为真。
    #[test]
    fn zhuyin_predicates() {
        let scheme = SchemeType::Zhuyin;
        let on: [Named; 10] = [
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
            ("nine_key", SchemeType::nine_key),
        ];
        let off: [Named; 16] = [
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

    /// 只有全拼、双拼、五笔离不开 `msime-english.db`；读主库的集合一定也要求它，准备代次时据此先查英文库再查主库。
    #[test]
    fn only_pinyin_and_wubi_require_the_english_dictionary() {
        assert!(SchemeSet::ALL.requires_english_dictionary());
        assert!(!SchemeSet::EMPTY.requires_english_dictionary());
        for value in 0..=u8::MAX {
            let Some(scheme) = SchemeType::from_u8(value) else {
                continue;
            };
            let set = SchemeSet::of(&[scheme]);
            let requires = matches!(
                scheme,
                SchemeType::Quanpin | SchemeType::Shuangpin | SchemeType::Wubi
            );
            assert_eq!(set.requires_english_dictionary(), requires, "{scheme:?}");
            assert!(!set.reads_main_dictionary() || set.requires_english_dictionary());
        }
        assert!(!SchemeSet::of(&[
            SchemeType::JapaneseRomaji,
            SchemeType::Korean,
            SchemeType::Vietnamese,
            SchemeType::Tibetan,
            SchemeType::Cantonese,
            SchemeType::Zhuyin,
            SchemeType::Stroke,
        ])
        .requires_english_dictionary());
    }

    #[test]
    fn lattice_static_evidence_removes_syllable_bonus_and_counts_unicode_characters() {
        let mut item = WordItem::new("xian", "西安", 36000, CandidateSource::Generated, "xi'an");
        assert_eq!(item.static_evidence(), None);
        item.sentence_association = true;
        let expected = Some(crate::ordering::StaticEvidence {
            log_prob_milli_nats: -4000,
            characters: 2,
        });
        assert_eq!(item.static_evidence(), expected);
        item.source = CandidateSource::NeuralKeyboard;
        assert_eq!(item.static_evidence(), expected);
        item.source = CandidateSource::Database;
        assert_eq!(item.static_evidence(), None);
        item.source = CandidateSource::Generated;
        item.canonical_pinyin.clear();
        assert_eq!(item.static_evidence(), None);
        item.canonical_pinyin = "xi'an".into();
        item.word.clear();
        assert_eq!(item.static_evidence(), None);
    }
}
