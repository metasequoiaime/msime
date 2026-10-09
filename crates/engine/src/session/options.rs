//! The public option and snapshot values of `Session` (`include/metasequoia/session.h`). Shared by the session, the nine-key session, the host facade and the golden harness.

use std::path::PathBuf;

use crate::helpcode::SharedKeymap;
use crate::paths::RuntimePaths;
use crate::types::{
    CandidateSource, CommandTableEntry, EnglishInputOptions, FrequencyAdjustmentOptions,
    FuzzyPinyinOptions, LocalInputMode, LocalModeOptions, MentionEntry, MixedExpressiveOptions,
    QuickPhraseEntry, SchemeSet, SchemeType, SentenceAssociationOptions, ShuangpinCustomTable,
    ShuangpinProfileKind, WordItem, WubiInputOptions,
};
use crate::vietnamese::{InputMethod as VietnameseInputMethod, ToneStyle as VietnameseToneStyle};

/// Everything a session is built with. `learning_undo` is gone with the feature.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionOptions {
    pub paths: RuntimePaths,
    pub scheme: SchemeType,
    /// 会话允许运行的方案，缺省全部。`scheme` 和之后的 `switch_scheme` 都必须在其中；只为其中的方案构造 provider（见 [`SchemeSet`]）。
    pub enabled_schemes: SchemeSet,
    pub shuangpin_profile: ShuangpinProfileKind,
    /// `shuangpin_profile` 为 `Custom` 时的键位表。没有它或它不合法时，会话里有双拼就建不起来（`INVALID_CUSTOM_SHUANGPIN_PROFILE`），没有双拼就按小鹤；内置方案不读它。
    pub shuangpin_custom_profile: Option<ShuangpinCustomTable>,
    /// Shuangpin preedit shows the typed keys rather than the decoded quanpin.
    pub shuangpin_preedit_uses_raw: bool,
    /// How the Vietnamese scheme spells marks: Telex letters or VNI digits.
    pub vietnamese_input_method: VietnameseInputMethod,
    /// Where the Vietnamese scheme puts the tone on `oa`, `oe` and `uy`.
    pub vietnamese_tone_style: VietnameseToneStyle,
    /// Where `msime-cantonese.db` is; empty when the host has none. Read only when Cantonese is activated, which fails without it.
    pub cantonese_dictionary: PathBuf,
    /// Where `msime-zhuyin.db` is; empty when the host has none. Read only when Zhuyin is activated, which fails without it.
    pub zhuyin_dictionary: PathBuf,
    /// Where `msime-stroke.db` is; empty when the host has none. Read only when Stroke is activated, which fails without it.
    pub stroke_dictionary: PathBuf,
    /// `msime-japanese.dat` 的位置；为空时读资源目录里的那份。文件缺失时日文只给假名行。
    pub japanese_dictionary: PathBuf,
    pub helpcode_schema: String,
    /// 宿主给的辅助码表；有它时直接装上它，不再按 `helpcode_schema` 读表（名字仍然要合法）。`Session::set_helpcode_table` 可以实时替换。
    pub helpcode_table: Option<SharedKeymap>,
    /// `autocorrect_type` bits; 0 keeps the user's spelling. Either of transposition and neighbor also enables missing and extra letters, and on inputs of three or more complete syllables offers a sentence that reads one syllable as a typo. Committing the raw letters while a correction is offered turns correction off for that exact input.
    pub autocorrect_types: u32,
    pub helpcode: bool,
    pub chinese_punctuation: bool,
    pub paired_punctuation: bool,
    /// 0 follows the mode, 1 forces Chinese, 2 forces ASCII.
    pub punctuation_lock: i32,
    pub learning: bool,
    /// Hand back every whole-sentence reading instead of only the best. A host that sets this must reorder and crop them itself.
    pub sentence_alternatives: bool,
    /// 只出单字：中文方案的候选去掉含汉字的词和整句、只留单个汉字（见 [`SchemeType::filters_to_single_characters`]），选一个字后剩下的拼写接着组字。
    pub single_character_only: bool,
    pub fuzzy_pinyin: FuzzyPinyinOptions,
    pub frequency: FrequencyAdjustmentOptions,
    pub local_modes: LocalModeOptions,
    pub english: EnglishInputOptions,
    pub expressive: MixedExpressiveOptions,
    pub wubi: WubiInputOptions,
    /// Learn word sequences and pick pairs; also needs `learning`.
    pub personal_context: bool,
    pub sentence_association: SentenceAssociationOptions,
    /// Committed text the neural sentence models condition on; `Session::set_rescoring_context` updates it live.
    pub rescoring_context: String,
    /// The `/` mode's commands beyond the built-in ones; `Session::set_command_table` replaces it live.
    pub command_table: Vec<CommandTableEntry>,
    /// The `@` mode's names and places; `Session::set_mention_entries` replaces it live.
    pub mention_entries: Vec<MentionEntry>,
    /// K 模式在数据库行之后追加的宿主短语；`Session::set_quick_phrase_table` 可以实时替换。
    pub quick_phrase_table: Vec<QuickPhraseEntry>,
}

impl SessionOptions {
    /// The reference defaults (session.h:12-54) on the given paths.
    pub fn new(paths: RuntimePaths) -> Self {
        Self {
            paths,
            scheme: SchemeType::Quanpin,
            enabled_schemes: SchemeSet::ALL,
            shuangpin_profile: ShuangpinProfileKind::Xiaohe,
            shuangpin_custom_profile: None,
            shuangpin_preedit_uses_raw: true,
            vietnamese_input_method: VietnameseInputMethod::Telex,
            vietnamese_tone_style: VietnameseToneStyle::Modern,
            cantonese_dictionary: PathBuf::new(),
            zhuyin_dictionary: PathBuf::new(),
            stroke_dictionary: PathBuf::new(),
            japanese_dictionary: PathBuf::new(),
            helpcode_schema: "lantian".to_owned(),
            helpcode_table: None,
            autocorrect_types: 0,
            helpcode: true,
            chinese_punctuation: true,
            paired_punctuation: true,
            punctuation_lock: 0,
            learning: true,
            sentence_alternatives: false,
            single_character_only: false,
            fuzzy_pinyin: FuzzyPinyinOptions::default(),
            frequency: FrequencyAdjustmentOptions::default(),
            local_modes: LocalModeOptions::default(),
            english: EnglishInputOptions::default(),
            expressive: MixedExpressiveOptions::default(),
            wubi: WubiInputOptions::default(),
            personal_context: true,
            sentence_association: SentenceAssociationOptions::default(),
            rescoring_context: String::new(),
            command_table: Vec::new(),
            mention_entries: Vec::new(),
            quick_phrase_table: Vec::new(),
        }
    }
}

/// A value copy of the composition; safe to hand to another thread. `candidate_sources`, `candidate_annotations` and `candidate_answers_key` are aligned with `candidates`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SessionSnapshot {
    pub scheme: SchemeType,
    pub local_mode: LocalInputMode,
    /// The non-letter characters `character` takes in this state: the active local mode's `spelling_symbols`, or while nothing is composed the keys that open the `/` and `@` modes. A host or runtime that would send one of these as punctuation sends it as a character instead.
    pub spelling_symbols: String,
    pub preedit: String,
    pub raw_segmentation: String,
    pub normalized_segmentation: String,
    pub candidates: Vec<WordItem>,
    pub dedicated_english: bool,
    /// The ASCII source text; `caret_position` is a byte offset into it.
    pub editing_text: String,
    pub caret_position: usize,
    pub nine_key_spellings: Vec<String>,
    /// 九键组字时的读音行：首选候选覆盖的数字显示成它的拼音，其余数字按 `preedit` 原样显示（`xi'an`、`yi'c`、`ni'hao'9`）；首选不是拼音候选时为空。只用于显示，`preedit` 仍是数字。
    pub nine_key_reading: String,
    /// 九宫格候选只留单字（`set_nine_key_filter`）。
    pub nine_key_single_character: bool,
    /// 九宫格候选按笔画筛选时所选的笔顺前缀（`hspnz`），否则为空。
    pub nine_key_strokes: String,
    /// The candidates came from the wubi mixed-pinyin fallback, not the wubi table.
    pub answered_by_pinyin_fallback: bool,
    pub wubi_unique_four_code: bool,
    pub shuangpin_profile: String,
    pub candidate_sources: Vec<CandidateSource>,
    /// Helpcodes or correction hints.
    pub candidate_annotations: Vec<String>,
    /// Whether each candidate answers the whole key rather than a prefix of it or a completion past it.
    pub candidate_answers_key: Vec<bool>,
    /// The scheme's openable candidate list is showing (the Korean Hanja list). Hosts read this instead of inferring it from the scheme and a non-empty list.
    pub candidate_list_open: bool,
}
