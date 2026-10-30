//! `msime_engine_session_new` 的选项 JSON（版本 1）：核心 `SessionOptions` 的一个有文档的子集，键表在 README 的「会话选项」一节。未知键一律拒绝，缺省的键取引擎库的默认值（`SessionOptions::new`）。

use std::collections::BTreeMap;
use std::path::PathBuf;

use msime_engine::vietnamese::{InputMethod, ToneStyle};
use msime_engine::{
    autocorrect_type, fuzzy_rule, FrequencyAdjustmentMode, KoreanLayout, RuntimePaths, SchemeSet,
    SchemeType, Session, SessionOptions, ShuangpinCustomTable, ShuangpinProfileKind,
    WubiProfileKind,
};
use serde::Deserialize;

use crate::abi::{Failure, Outcome};

/// 选项 JSON 唯一认识的版本。
const VERSION: u32 = 1;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionsV1 {
    version: u32,
    paths: Paths,
    scheme: Option<Scheme>,
    enabled_schemes: Option<Vec<Scheme>>,
    shuangpin: Option<Shuangpin>,
    wubi: Option<Wubi>,
    vietnamese: Option<Vietnamese>,
    korean: Option<Korean>,
    helpcode: Option<Helpcode>,
    punctuation: Option<Punctuation>,
    learning: Option<bool>,
    personal_context: Option<bool>,
    next_word_prediction: Option<bool>,
    fuzzy_pinyin: Option<Vec<Fuzzy>>,
    autocorrect: Option<Vec<Autocorrect>>,
    english: Option<English>,
    first_page_rows: Option<u32>,
    language_dictionaries: Option<LanguageDictionaries>,
    /// ABI 1.1 起。
    frequency: Option<Frequency>,
    /// ABI 1.1 起。
    expressive: Option<Expressive>,
    /// ABI 1.1 起。
    local_modes: Option<LocalModes>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Paths {
    resources: PathBuf,
    user_data: PathBuf,
    cache: PathBuf,
    dictionaries: PathBuf,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum Scheme {
    Quanpin,
    Shuangpin,
    Wubi,
    Japanese,
    Korean,
    Cantonese,
    Zhuyin,
    Vietnamese,
    Tibetan,
    Stroke,
}

impl From<Scheme> for SchemeType {
    fn from(scheme: Scheme) -> Self {
        match scheme {
            Scheme::Quanpin => Self::Quanpin,
            Scheme::Shuangpin => Self::Shuangpin,
            Scheme::Wubi => Self::Wubi,
            Scheme::Japanese => Self::JapaneseRomaji,
            Scheme::Korean => Self::Korean,
            Scheme::Cantonese => Self::Cantonese,
            Scheme::Zhuyin => Self::Zhuyin,
            Scheme::Vietnamese => Self::Vietnamese,
            Scheme::Tibetan => Self::Tibetan,
            Scheme::Stroke => Self::Stroke,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Shuangpin {
    profile: Option<ShuangpinProfile>,
    /// `profile` 为 `custom` 时的键位表。
    custom: Option<ShuangpinCustom>,
    /// 预编辑显示按键原文（默认）而不是还原出的全拼。
    preedit_shows_keys: Option<bool>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum ShuangpinProfile {
    Xiaohe,
    Ziranma,
    Shoudao,
    Microsoft,
    SmartAbc,
    Custom,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShuangpinCustom {
    initials: BTreeMap<String, String>,
    finals: BTreeMap<String, String>,
    zero_initials: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wubi {
    profile: Option<WubiProfile>,
    mixed_pinyin: Option<bool>,
    z_wildcard: Option<bool>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum WubiProfile {
    Wubi86,
    Wubi98,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Vietnamese {
    input_method: Option<VietnameseMethod>,
    tone_style: Option<VietnameseTone>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum VietnameseMethod {
    Telex,
    Vni,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum VietnameseTone {
    Modern,
    Classic,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Korean {
    layout: Option<KoreanLayoutName>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum KoreanLayoutName {
    Dubeolsik,
    Romaja,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Helpcode {
    enabled: Option<bool>,
    /// 表名，读 `resources/helpcodes/` 下对应的文件。
    schema: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Punctuation {
    chinese: Option<bool>,
    paired: Option<bool>,
    lock: Option<PunctuationLock>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum PunctuationLock {
    Follow,
    Chinese,
    Ascii,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum Fuzzy {
    ZZh,
    CCh,
    SSh,
    NL,
    FH,
    RL,
    AnAng,
    EnEng,
    InIng,
    IanIang,
    UanUang,
}

impl Fuzzy {
    fn bit(self) -> u32 {
        match self {
            Self::ZZh => fuzzy_rule::Z_ZH,
            Self::CCh => fuzzy_rule::C_CH,
            Self::SSh => fuzzy_rule::S_SH,
            Self::NL => fuzzy_rule::N_L,
            Self::FH => fuzzy_rule::F_H,
            Self::RL => fuzzy_rule::R_L,
            Self::AnAng => fuzzy_rule::AN_ANG,
            Self::EnEng => fuzzy_rule::EN_ENG,
            Self::InIng => fuzzy_rule::IN_ING,
            Self::IanIang => fuzzy_rule::IAN_IANG,
            Self::UanUang => fuzzy_rule::UAN_UANG,
        }
    }
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum Autocorrect {
    Transposition,
    Neighbor,
    Deletion,
    Insertion,
}

impl Autocorrect {
    fn bit(self) -> u32 {
        match self {
            Self::Transposition => autocorrect_type::TRANSPOSITION,
            Self::Neighbor => autocorrect_type::NEIGHBOR,
            Self::Deletion => autocorrect_type::DELETION,
            Self::Insertion => autocorrect_type::INSERTION,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct English {
    /// 中文组字里混入英文单词。
    mixed_candidates: Option<bool>,
    /// 补全与英文领先的触发字符数，1 到 8。
    minimum_prefix: Option<u32>,
    /// 首页最多几个英文候选，1 到 3。
    first_page_count: Option<u32>,
    /// ABI 1.1 起。回车学英文词的总开关，缺省开：只开它时只学专用英文里打的词。
    learn_on_enter: Option<bool>,
    /// ABI 1.1 起。回车时专用英文之外也学：临时英文的字母、拼不成完整音节的拼音（msime 产品的策略），缺省关。要 `learn_on_enter` 也开着。
    learn_typed_spellings_on_enter: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frequency {
    mode: Option<FrequencyMode>,
    /// 同一个词要被选中几次才挪位，1 到 10。
    trigger_count: Option<u32>,
    /// `linear` 每次前移几位，1 到 10。
    linear_step: Option<u32>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum FrequencyMode {
    Disabled,
    Pin,
    Halve,
    Linear,
    Promote,
}

/// 中文组字里混入的 emoji、颜文字和符号行（全拼、双拼，输入至少两个字母）。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expressive {
    /// 要 `resources/msime-others.db`。
    emoji_candidates: Option<bool>,
    /// 要 `resources/msime-others.db`。
    kaomoji_candidates: Option<bool>,
    /// 打拼音出符号，表编进了引擎。
    symbol_candidates: Option<bool>,
}

/// 各个本地模式能不能进入。`/` 指令和 `@` 名单不在这里：它们的行要宿主喂表，这个 ABI 不收这些表，两者固定关着。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalModes {
    unicode: Option<bool>,
    date_time: Option<bool>,
    date_time_inline: Option<bool>,
    date_time_menu: Option<bool>,
    quick_phrase: Option<bool>,
    emoji: Option<bool>,
    kaomoji: Option<bool>,
    super_jianpin: Option<bool>,
    temporary_english: Option<bool>,
    temporary_japanese: Option<bool>,
    expression: Option<bool>,
    chaizi: Option<bool>,
    symbol: Option<bool>,
    shift_letter_entry: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LanguageDictionaries {
    cantonese: Option<PathBuf>,
    zhuyin: Option<PathBuf>,
    stroke: Option<PathBuf>,
    japanese: Option<PathBuf>,
}

/// 解析选项 JSON，得到会话选项；任何不认识或不合法的内容都是 `MSIME_ENGINE_INVALID_ARGUMENT`。
pub(crate) fn parse(json: &str) -> Outcome<SessionOptions> {
    let parsed: OptionsV1 = serde_json::from_str(json)
        .map_err(|error| Failure::invalid(format!("options: {error}")))?;
    if parsed.version != VERSION {
        return Err(Failure::invalid(format!(
            "options: version {} is not supported (this library reads version {VERSION})",
            parsed.version
        )));
    }
    let mut options = SessionOptions::new(RuntimePaths {
        resources: parsed.paths.resources,
        user_data: parsed.paths.user_data,
        cache: parsed.paths.cache,
        dictionaries: parsed.paths.dictionaries,
    });
    if let Some(scheme) = parsed.scheme {
        options.scheme = scheme.into();
    }
    if let Some(schemes) = parsed.enabled_schemes {
        let schemes: Vec<SchemeType> = schemes.into_iter().map(SchemeType::from).collect();
        options.enabled_schemes = SchemeSet::of(&schemes);
    }
    if let Some(shuangpin) = parsed.shuangpin {
        if let Some(profile) = shuangpin.profile {
            options.shuangpin_profile = match profile {
                ShuangpinProfile::Xiaohe => ShuangpinProfileKind::Xiaohe,
                ShuangpinProfile::Ziranma => ShuangpinProfileKind::Ziranma,
                ShuangpinProfile::Shoudao => ShuangpinProfileKind::Shoudao,
                ShuangpinProfile::Microsoft => ShuangpinProfileKind::Microsoft,
                ShuangpinProfile::SmartAbc => ShuangpinProfileKind::SmartAbc,
                ShuangpinProfile::Custom => ShuangpinProfileKind::Custom,
            };
        }
        if let Some(custom) = shuangpin.custom {
            options.shuangpin_custom_profile = Some(ShuangpinCustomTable {
                initials: custom.initials.into_iter().collect(),
                finals: custom.finals.into_iter().collect(),
                zero_initials: custom.zero_initials.into_iter().collect(),
            });
        }
        if let Some(raw) = shuangpin.preedit_shows_keys {
            options.shuangpin_preedit_uses_raw = raw;
        }
    }
    if let Some(wubi) = parsed.wubi {
        if let Some(profile) = wubi.profile {
            options.wubi.profile = match profile {
                WubiProfile::Wubi86 => WubiProfileKind::Wubi86,
                WubiProfile::Wubi98 => WubiProfileKind::Wubi98,
            };
        }
        if let Some(mixed) = wubi.mixed_pinyin {
            options.wubi.mixed_pinyin = mixed;
        }
        if let Some(wildcard) = wubi.z_wildcard {
            options.wubi.z_wildcard = wildcard;
        }
    }
    if let Some(vietnamese) = parsed.vietnamese {
        if let Some(method) = vietnamese.input_method {
            options.vietnamese_input_method = match method {
                VietnameseMethod::Telex => InputMethod::Telex,
                VietnameseMethod::Vni => InputMethod::Vni,
            };
        }
        if let Some(tone) = vietnamese.tone_style {
            options.vietnamese_tone_style = match tone {
                VietnameseTone::Modern => ToneStyle::Modern,
                VietnameseTone::Classic => ToneStyle::Classic,
            };
        }
    }
    if let Some(layout) = parsed.korean.and_then(|korean| korean.layout) {
        options.korean_layout = match layout {
            KoreanLayoutName::Dubeolsik => KoreanLayout::Dubeolsik,
            KoreanLayoutName::Romaja => KoreanLayout::Romaja,
        };
    }
    if let Some(helpcode) = parsed.helpcode {
        if let Some(enabled) = helpcode.enabled {
            options.helpcode = enabled;
        }
        if let Some(schema) = helpcode.schema {
            if !Session::is_supported_helpcode_schema(&schema) {
                return Err(Failure::invalid(format!(
                    "options: unknown helpcode schema {schema:?}"
                )));
            }
            options.helpcode_schema = schema;
        }
    }
    if let Some(punctuation) = parsed.punctuation {
        if let Some(chinese) = punctuation.chinese {
            options.chinese_punctuation = chinese;
        }
        if let Some(paired) = punctuation.paired {
            options.paired_punctuation = paired;
        }
        if let Some(lock) = punctuation.lock {
            options.punctuation_lock = match lock {
                PunctuationLock::Follow => 0,
                PunctuationLock::Chinese => 1,
                PunctuationLock::Ascii => 2,
            };
        }
    }
    if let Some(learning) = parsed.learning {
        options.learning = learning;
    }
    if let Some(personal) = parsed.personal_context {
        options.personal_context = personal;
    }
    if let Some(prediction) = parsed.next_word_prediction {
        options.next_word_prediction = prediction;
    }
    if let Some(rules) = parsed.fuzzy_pinyin {
        options.fuzzy_pinyin.rules = rules.into_iter().fold(0, |bits, rule| bits | rule.bit());
    }
    if let Some(types) = parsed.autocorrect {
        options.autocorrect_types = types.into_iter().fold(0, |bits, kind| bits | kind.bit());
    }
    if let Some(english) = parsed.english {
        if let Some(mixed) = english.mixed_candidates {
            options.english.mixed_candidates = mixed;
        }
        if let Some(prefix) = english.minimum_prefix {
            if !(1..=8).contains(&prefix) {
                return Err(Failure::invalid(
                    "options: english.minimum_prefix must be 1..=8",
                ));
            }
            options.english.minimum_prefix = prefix as usize;
        }
        if let Some(count) = english.first_page_count {
            if !(1..=3).contains(&count) {
                return Err(Failure::invalid(
                    "options: english.first_page_count must be 1..=3",
                ));
            }
            options.english.first_page_count = count as usize;
        }
        if let Some(learn) = english.learn_on_enter {
            options.learn_english_on_enter = learn;
        }
        if let Some(learn) = english.learn_typed_spellings_on_enter {
            options.learn_typed_spellings_on_enter = learn;
        }
    }
    if let Some(rows) = parsed.first_page_rows {
        if !(1..=255).contains(&rows) {
            return Err(Failure::invalid("options: first_page_rows must be 1..=255"));
        }
        options.first_page_rows = rows as usize;
    }
    if let Some(dictionaries) = parsed.language_dictionaries {
        if let Some(path) = dictionaries.cantonese {
            options.cantonese_dictionary = path;
        }
        if let Some(path) = dictionaries.zhuyin {
            options.zhuyin_dictionary = path;
        }
        if let Some(path) = dictionaries.stroke {
            options.stroke_dictionary = path;
        }
        if let Some(path) = dictionaries.japanese {
            options.japanese_dictionary = path;
        }
    }
    if let Some(frequency) = parsed.frequency {
        if let Some(mode) = frequency.mode {
            options.frequency.mode = match mode {
                FrequencyMode::Disabled => FrequencyAdjustmentMode::Disabled,
                FrequencyMode::Pin => FrequencyAdjustmentMode::Pin,
                FrequencyMode::Halve => FrequencyAdjustmentMode::Halve,
                FrequencyMode::Linear => FrequencyAdjustmentMode::Linear,
                FrequencyMode::Promote => FrequencyAdjustmentMode::Promote,
            };
        }
        if let Some(count) = frequency.trigger_count {
            options.frequency.trigger_count = frequency_count(count, "frequency.trigger_count")?;
        }
        if let Some(step) = frequency.linear_step {
            options.frequency.linear_step = frequency_count(step, "frequency.linear_step")?;
        }
    }
    if let Some(expressive) = parsed.expressive {
        let target = &mut options.expressive;
        for (value, slot) in [
            (expressive.emoji_candidates, &mut target.emoji_candidates),
            (
                expressive.kaomoji_candidates,
                &mut target.kaomoji_candidates,
            ),
            (expressive.symbol_candidates, &mut target.symbol_candidates),
        ] {
            if let Some(value) = value {
                *slot = value;
            }
        }
    }
    if let Some(modes) = parsed.local_modes {
        let target = &mut options.local_modes;
        for (value, slot) in [
            (modes.unicode, &mut target.unicode),
            (modes.date_time, &mut target.date_time),
            (modes.date_time_inline, &mut target.date_time_inline),
            (modes.date_time_menu, &mut target.date_time_menu),
            (modes.quick_phrase, &mut target.quick_phrase),
            (modes.emoji, &mut target.emoji),
            (modes.kaomoji, &mut target.kaomoji),
            (modes.super_jianpin, &mut target.super_jianpin),
            (modes.temporary_english, &mut target.temporary_english),
            (modes.temporary_japanese, &mut target.temporary_japanese),
            (modes.expression, &mut target.expression),
            (modes.chaizi, &mut target.chaizi),
            (modes.symbol, &mut target.symbol),
            (modes.shift_letter_entry, &mut target.shift_letter_entry),
        ] {
            if let Some(value) = value {
                *slot = value;
            }
        }
    }
    // 整句的其余读法要宿主自己重排和裁剪，这个 ABI 只给引擎原始顺序。
    options.sentence_alternatives = false;
    Ok(options)
}

/// 调频的次数与步长：1 到 10，与引擎建会话时的检查相同，在这里先报出是哪一键。
fn frequency_count(value: u32, key: &'static str) -> Outcome<i32> {
    if !(1..=10).contains(&value) {
        return Err(Failure::invalid(format!("options: {key} must be 1..=10")));
    }
    Ok(value as i32)
}
