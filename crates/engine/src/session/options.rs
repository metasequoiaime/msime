//! The public option and snapshot values of `Session` (`include/metasequoia/session.h`). Shared by the session, the nine-key session, the host facade and the golden harness.

use std::path::PathBuf;

use crate::helpcode::SharedKeymap;
use crate::paths::RuntimePaths;
use crate::punctuation::PunctuationMarks;
use crate::types::{
    CandidateSource, CommandTableEntry, EnglishInputOptions, FrequencyAdjustmentOptions,
    FuzzyPinyinOptions, LocalInputMode, LocalModeOptions, MentionEntry, MixedExpressiveOptions,
    PluginCommand, QuickPhraseEntry, SchemeSet, SchemeType, SentenceAssociationOptions,
    ShuangpinCustomTable, ShuangpinHelpcodeModes, ShuangpinProfileKind, WordItem, WubiInputOptions,
    DEFAULT_FIRST_PAGE_ROWS,
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
    /// 韩语实体键盘布局，缺省两套式。
    pub korean_layout: crate::KoreanLayout,
    /// Where `msime-cantonese.db` is; empty when the host has none. Read only when Cantonese is activated, which fails without it.
    pub cantonese_dictionary: PathBuf,
    /// Where `msime-zhuyin.db` is; empty when the host has none. Read only when Zhuyin is activated, which fails without it.
    pub zhuyin_dictionary: PathBuf,
    /// Where `msime-stroke.db` is; empty when the host has none. Read only when Stroke is activated, which fails without it.
    pub stroke_dictionary: PathBuf,
    /// `msime-japanese.dat` 的位置；为空时读资源目录里的那份。文件缺失时日文只给假名行。
    pub japanese_dictionary: PathBuf,
    pub helpcode_schema: String,
    /// 辅助码前导标点；None 保留现有 Shift 加字母。
    pub helpcode_trigger_key: Option<u8>,
    /// 宿主给的辅助码表；有它时直接装上它，不再按 `helpcode_schema` 读表（名字仍然要合法）。`Session::set_helpcode_table` 可以实时替换。
    pub helpcode_table: Option<SharedKeymap>,
    /// 内置辅助码表缺失时使用的随包资源根目录。
    pub helpcode_fallback_resources: Option<PathBuf>,
    /// `autocorrect_type` bits; 0 keeps the user's spelling. Either of transposition and neighbor also enables missing and extra letters, and on inputs of three or more complete syllables offers a sentence that reads one syllable as a typo. Committing the raw letters while a correction is offered turns correction off for that exact input.
    pub autocorrect_types: u32,
    pub helpcode: bool,
    /// 双拼辅助码的触发方式（Windows `[helpcode]` 组的双拼专属键）；只在双拼方案下生效。
    pub shuangpin_helpcode_modes: ShuangpinHelpcodeModes,
    pub chinese_punctuation: bool,
    pub paired_punctuation: bool,
    /// 0 follows the mode, 1 forces Chinese, 2 forces ASCII.
    pub punctuation_lock: i32,
    /// 中文标点下引号键和方头括号键出哪一组字，缺省是弯引号、实心括号。
    pub punctuation_marks: PunctuationMarks,
    pub learning: bool,
    /// 回车上屏英文拼写时是否把它学进英文词库（Windows `input.enter_learns_english_word`，出厂为开）。
    pub learn_english_on_enter: bool,
    /// Hand back every whole-sentence reading instead of only the best. A host that sets this must reorder and crop them itself.
    pub sentence_alternatives: bool,
    /// 宿主的每页候选数（`candidate_page_size`）：全长组的精确同音词够这么多行就不再并入更长的补全词。0 到请求里会归一成 1。
    pub first_page_rows: usize,
    /// 只出单字：中文方案的候选去掉含汉字的词和整句、只留单个汉字（见 [`SchemeType::filters_to_single_characters`]），选一个字后剩下的拼写接着组字。
    pub single_character_only: bool,
    pub fuzzy_pinyin: FuzzyPinyinOptions,
    pub frequency: FrequencyAdjustmentOptions,
    pub local_modes: LocalModeOptions,
    /// 候选混输总开关（Windows `utility.mixed_candidates`）：关掉后英文、emoji、颜文字、符号、日文混输行、行内日期时间和快捷短语都不再混进普通候选；各分成员（`english.mixed_candidates`、`expressive.*`、`local_modes.*`）原值保留，重新打开即恢复。本地模式本身（T、K、M 等）不受影响。
    pub mixed_candidates: bool,
    pub english: EnglishInputOptions,
    pub expressive: MixedExpressiveOptions,
    /// 日文混输（实验选项，默认关）：全拼、双拼组字时，整段输入像日文罗马字就混进一行日文转换（来源 `MixedJapanese`），见 `ImeSession::mixed_japanese_conversion`。需要会话带日文（`enabled_schemes` 含 `JapaneseRomaji`）和日文模型；开着时建会话就在后台预热模型。
    pub japanese_mixing: bool,
    /// 连续多语言输入所选的语言；默认空集合关闭，少于两种不生成混合行。
    pub multilingual_languages: Vec<crate::multilingual::Language>,
    pub wubi: WubiInputOptions,
    /// Learn word sequences and pick pairs; also needs `learning`.
    pub personal_context: bool,
    /// 上屏后联想；独立于整句候选，默认关闭。
    pub next_word_prediction: bool,
    pub sentence_association: SentenceAssociationOptions,
    /// Committed text the neural sentence models condition on; `Session::set_rescoring_context` updates it live.
    pub rescoring_context: String,
    /// The `/` mode's commands beyond the built-in ones; `Session::set_command_table` replaces it live.
    pub command_table: Vec<CommandTableEntry>,
    /// 脚本插件在 `/` 模式里的指令（只在桌面非空）；`Session::set_plugin_commands` 可以实时替换。
    pub plugin_commands: Vec<PluginCommand>,
    /// The `@` mode's names and places; `Session::set_mention_entries` replaces it live.
    pub mention_entries: Vec<MentionEntry>,
    /// K 模式在数据库行之后追加的宿主短语；`Session::set_quick_phrase_table` 可以实时替换。
    pub quick_phrase_table: Vec<QuickPhraseEntry>,
    /// 普通候选是否追加编码匹配的快捷短语。仅 macOS 桌面宿主打开；其他宿主保留独立 K 模式。
    pub quick_phrase_candidates: bool,
    /// 快捷短语参与调频：追加进普通候选的整组短语有自己的槽位（`quick_phrase_slots` 日志表），越过它选普通候选让整组后退（最多退到首页末位），选中组内短语让整组前移。关掉时组固定追加在末尾，槽位记录保留但不读。
    pub quick_phrase_frequency: bool,
    /// 宿主会把候选转成繁体输出：拆字（`C`）行按转换后的字形去重，免得一屏出现两个相同的繁体字。
    pub chaizi_traditional_output: bool,
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
            korean_layout: crate::KoreanLayout::Dubeolsik,
            cantonese_dictionary: PathBuf::new(),
            zhuyin_dictionary: PathBuf::new(),
            stroke_dictionary: PathBuf::new(),
            japanese_dictionary: PathBuf::new(),
            helpcode_schema: "lantian".to_owned(),
            helpcode_trigger_key: None,
            helpcode_table: None,
            helpcode_fallback_resources: None,
            autocorrect_types: 0,
            helpcode: true,
            shuangpin_helpcode_modes: ShuangpinHelpcodeModes::default(),
            chinese_punctuation: true,
            paired_punctuation: true,
            punctuation_lock: 0,
            punctuation_marks: PunctuationMarks::default(),
            learning: true,
            learn_english_on_enter: true,
            sentence_alternatives: false,
            first_page_rows: DEFAULT_FIRST_PAGE_ROWS,
            single_character_only: false,
            fuzzy_pinyin: FuzzyPinyinOptions::default(),
            frequency: FrequencyAdjustmentOptions::default(),
            local_modes: LocalModeOptions::default(),
            mixed_candidates: true,
            english: EnglishInputOptions::default(),
            expressive: MixedExpressiveOptions::default(),
            japanese_mixing: false,
            multilingual_languages: Vec::new(),
            wubi: WubiInputOptions::default(),
            personal_context: true,
            next_word_prediction: false,
            sentence_association: SentenceAssociationOptions::default(),
            rescoring_context: String::new(),
            command_table: Vec::new(),
            plugin_commands: Vec::new(),
            mention_entries: Vec::new(),
            quick_phrase_table: Vec::new(),
            quick_phrase_candidates: false,
            quick_phrase_frequency: true,
            chaizi_traditional_output: false,
        }
    }
}

/// A value copy of the composition; safe to hand to another thread. `candidate_sources`, `candidate_annotations` and `candidate_answers_key` are aligned with `candidates`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SessionSnapshot {
    /// 非组字的上屏后联想，只接受显式选择，不由空格、回车或失焦提交。
    pub prediction: bool,
    pub scheme: SchemeType,
    pub local_mode: LocalInputMode,
    /// The non-letter characters `character` takes in this state: the active local mode's `spelling_symbols` (拆字按光标处列出部件分隔符 `'` 和微软双拼的韵母 `;`), or while nothing is composed the keys that open the `/` and `@` modes. A host or runtime that would send one of these as punctuation sends it as a character instead.
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
    /// 当前唯一的连续混合行所携带的语言边界；没有该行时为空，不随普通候选重排而错位。
    pub multilingual_conversion: Option<crate::multilingual::Conversion>,
    /// Helpcodes or correction hints.
    pub candidate_annotations: Vec<String>,
    /// Whether each candidate answers the whole key rather than a prefix of it or a completion past it.
    pub candidate_answers_key: Vec<bool>,
    /// The scheme's openable candidate list is showing (the Korean Hanja list). Hosts read this instead of inferring it from the scheme and a non-empty list.
    pub candidate_list_open: bool,
    /// 整句改字时改好的整句，否则为空。宿主在它非空时把它画在行内代替拼音。
    pub conversion: String,
    /// 整句改字的光标和光标处那一段，按 `conversion` 的字（Unicode 标量）计：光标在第 `.0` 个字之前，`.0..1` 是候选所替换的那一段；光标在句末时两者都等于字数。不在改字时为 `(0, 0)`。
    pub conversion_focus: (usize, usize),
}
