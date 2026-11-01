//! Word-level Simplified-to-Traditional conversion, matching OpenCC's `s2t.json`, `s2tw.json` and `s2twp.json`.
//!
//! The reference Windows server converts committed text and candidates with OpenCC's standard `s2t` configuration. Character-by-character platform tables (`LCMapStringEx`, ICU's transliterator) cannot resolve one-to-many characters such as 发 (發/髮), 干 (乾/幹/干) or 面 (面/麵); OpenCC resolves them with a phrase table first. This module reproduces that configuration from the same OpenCC data, pinned to the commit the reference vendors, so a host gets the reference's output without linking OpenCC's C++ library.
//!
//! The configuration, from OpenCC's `data/config/s2t.json`:
//!
//! - normalization: one maximum-forward-match pass over `CJK_Compatibility_Ideographs`;
//! - conversion: one maximum-forward-match pass over a short-circuit group whose first member is the union of `STPhrases` and `STPhrases_GeneratedFromRegionalPhrases` and whose second member is `STCharacters`.
//!
//! 臺灣字形 `s2tw.json` 与臺灣惯用词 `s2twp.json` 在同样的规范化之后，先按 `STPhrases` 与地区词组的并集做最大正向匹配分段（命中的词组各成一段，连续未命中的字并成一段），再逐段先做上面的 `s2t` 转换、然后在同一段内做第二遍最大正向匹配：`s2tw` 依次查 `TWVariantsPhrases`、`TWVariants`，`s2twp` 在它们前面再加 `TWPhrases`，组内先命中的词典优先。第二遍不跨段，所以臺灣词组只在 `s2t` 的词组边界之内生效，与 OpenCC 相同。
//!
//! 繁转简 `traditional_to_simplified` 对应 OpenCC 的 `t2s.json`，但不带 `TSCharactersExt`（上游把它标为可能输出豆腐块）：同样先做规范化，再对 `TSPhrases` 与 `TSCharacters` 组成的短路组做一遍最大正向匹配。`cantonese_to_simplified` 是粤拼输出简体用的版本，在字表之前先保留几个粤语用字（见 `CANTONESE_KEPT`）。
//!
//! Where a dictionary lists several values the first is taken, as OpenCC does. Text no dictionary matches passes through unchanged; a complete ideographic description sequence (`⿰女尔`) passes through as one unit, so its components are never converted on their own.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

const CJK_COMPATIBILITY_IDEOGRAPHS: &str = include_str!(concat!(
    env!("MSIME_OPENCC_DIR"),
    "/CJK_Compatibility_Ideographs.txt"
));
const ST_PHRASES: &str = include_str!(concat!(env!("MSIME_OPENCC_DIR"), "/STPhrases.txt"));
const ST_PHRASES_REGIONAL: &str = include_str!(concat!(
    env!("MSIME_OPENCC_DIR"),
    "/STPhrases_GeneratedFromRegionalPhrases.txt"
));
const ST_CHARACTERS: &str = include_str!(concat!(env!("MSIME_OPENCC_DIR"), "/STCharacters.txt"));
const TW_PHRASES: &str = include_str!(concat!(env!("MSIME_OPENCC_DIR"), "/TWPhrases.txt"));
const TW_VARIANTS_PHRASES: &str =
    include_str!(concat!(env!("MSIME_OPENCC_DIR"), "/TWVariantsPhrases.txt"));
const TW_VARIANTS: &str = include_str!(concat!(env!("MSIME_OPENCC_DIR"), "/TWVariants.txt"));
const TS_PHRASES: &str = include_str!(concat!(env!("MSIME_OPENCC_DIR"), "/TSPhrases.txt"));
const TS_CHARACTERS: &str = include_str!(concat!(env!("MSIME_OPENCC_DIR"), "/TSCharacters.txt"));

/// 粤拼输出简体时不转的粤语用字。`TSCharacters` 把嗰转成 𠮶（扩展 B 区）、喎转成 㖞（扩展 A 区），常见字体缺字，粤语的简体写法也通常保留原字；其余粤语用字（嘅、咗、冇、喺、嚟、啲、佢、嘢）字表本来就没有条目，原样通过。
const CANTONESE_KEPT: [&str; 2] = ["嗰", "喎"];

/// 繁体输出用哪套 OpenCC 配置；偏好里存成配置名。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum TraditionalVariant {
    /// `s2t.json`：OpenCC 标准繁体。
    #[default]
    #[serde(rename = "s2t")]
    Standard,
    /// `s2tw.json`：臺灣正体字形。
    #[serde(rename = "s2tw")]
    Taiwan,
    /// `s2twp.json`：臺灣正体字形加臺灣惯用词。
    #[serde(rename = "s2twp")]
    TaiwanPhrases,
}

impl TraditionalVariant {
    /// C 接口里的编号：0 `s2t`、1 `s2tw`、2 `s2twp`。
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::Standard),
            1 => Some(Self::Taiwan),
            2 => Some(Self::TaiwanPhrases),
            _ => None,
        }
    }
}

/// Bounds from OpenCC's `UTF8Util::NextIdeographicDescriptionSequenceLength`.
const MAX_IDS_DEPTH: usize = 16;
const MAX_IDS_CODE_POINTS: usize = 64;

/// 一张 OpenCC 词典：键到首值的映射、最长键字符数和有效键的首字节集合。
struct Table {
    entries: HashMap<&'static str, &'static str>,
    longest: usize,
    initial_bytes: [u64; 4],
}

fn source_line_capacity(sources: &[&str]) -> usize {
    sources.iter().map(|source| source.lines().count()).sum()
}

impl Table {
    /// 解析 OpenCC 文本；重复键优先使用前面的来源，与等长匹配的词典组规则一致。
    fn parse(sources: &[&'static str]) -> Self {
        let mut entries = HashMap::with_capacity(source_line_capacity(sources));
        let mut longest = 0;
        let mut initial_bytes = [0; 4];
        for source in sources {
            for line in source.lines() {
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let Some((key, values)) = line.split_once('\t') else {
                    continue;
                };
                let Some(value) = values.split_whitespace().next() else {
                    continue;
                };
                if key.is_empty() {
                    continue;
                }
                let first = usize::from(key.as_bytes()[0]);
                initial_bytes[first / 64] |= 1 << (first % 64);
                longest = longest.max(key.chars().count());
                entries.entry(key).or_insert(value);
            }
        }
        Self {
            entries,
            longest,
            initial_bytes,
        }
    }

    /// 返回匹配 `text` 的最长前缀键及其首值，键长以字节计。
    fn longest_prefix(&self, text: &str) -> Option<(usize, &'static str)> {
        let first = usize::from(*text.as_bytes().first()?);
        // 首字节只排除不可能的键；同字节的不同字符仍需原来的最长匹配。
        if self.initial_bytes[first / 64] & (1 << (first % 64)) == 0 {
            return None;
        }
        let (offset, character) = text.char_indices().take(self.longest).last()?;
        let prefix = &text[..offset + character.len_utf8()];
        prefix.char_indices().rev().find_map(|(offset, character)| {
            let end = offset + character.len_utf8();
            self.entries.get(&prefix[..end]).map(|value| (end, *value))
        })
    }
}

struct Tables {
    normalization: Table,
    phrases: Table,
    characters: Table,
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| Tables {
        normalization: Table::parse(&[CJK_COMPATIBILITY_IDEOGRAPHS]),
        phrases: Table::parse(&[ST_PHRASES, ST_PHRASES_REGIONAL]),
        characters: Table::parse(&[ST_CHARACTERS]),
    })
}

/// 臺灣配置第二遍用的三张表；只在第一次选用臺灣变体时解析。
struct TaiwanTables {
    phrases: Table,
    variants_phrases: Table,
    variants: Table,
}

fn taiwan_tables() -> &'static TaiwanTables {
    static TABLES: OnceLock<TaiwanTables> = OnceLock::new();
    TABLES.get_or_init(|| TaiwanTables {
        phrases: Table::parse(&[TW_PHRASES]),
        variants_phrases: Table::parse(&[TW_VARIANTS_PHRASES]),
        variants: Table::parse(&[TW_VARIANTS]),
    })
}

/// Convert Simplified Chinese to Traditional Chinese the way OpenCC's `s2t.json` does.
///
/// Characters with no Traditional form (ASCII, kana, emoji, already-Traditional text) are returned unchanged.
pub fn simplified_to_traditional(text: &str) -> String {
    convert_to_traditional(text, TraditionalVariant::Standard)
}

/// 按所选 OpenCC 配置把简体转换成繁体；规则见模块说明。
pub fn convert_to_traditional(text: &str, variant: TraditionalVariant) -> String {
    let tables = tables();
    let normalized = convert_pass(text, |rest| tables.normalization.longest_prefix(rest));
    let standard = |rest: &str| {
        tables
            .phrases
            .longest_prefix(rest)
            .or_else(|| tables.characters.longest_prefix(rest))
    };
    let taiwan = taiwan_tables();
    let regional = |rest: &str| match variant {
        TraditionalVariant::Standard => None,
        TraditionalVariant::Taiwan => taiwan
            .variants_phrases
            .longest_prefix(rest)
            .or_else(|| taiwan.variants.longest_prefix(rest)),
        TraditionalVariant::TaiwanPhrases => taiwan
            .phrases
            .longest_prefix(rest)
            .or_else(|| taiwan.variants_phrases.longest_prefix(rest))
            .or_else(|| taiwan.variants.longest_prefix(rest)),
    };
    if variant == TraditionalVariant::Standard {
        return match convert_pass(&normalized, standard) {
            Cow::Borrowed(_) => normalized.into_owned(),
            Cow::Owned(converted) => converted,
        };
    }
    let capacity = normalized.len() + normalized.len() / 5;
    let mut output = String::with_capacity(capacity);
    // 每段的 `s2t` 结果写进同一块暂存区再做第二遍，整次转换只申请这两块缓冲。
    let mut stage = String::with_capacity(capacity);
    for_each_segment(
        &normalized,
        |rest| tables.phrases.longest_prefix(rest),
        |segment| {
            stage.clear();
            append_pass(segment, standard, &mut stage);
            append_pass(&stage, regional, &mut output);
        },
    );
    output
}

/// OpenCC 的 `MaxMatchSegmentation`：词典命中的前缀各成一段，相邻的未命中部分并成一段。
fn for_each_segment<'a>(
    text: &'a str,
    matcher: impl Fn(&str) -> Option<(usize, &'static str)>,
    mut segment: impl FnMut(&'a str),
) {
    let mut start = 0;
    let mut position = 0;
    while position < text.len() {
        let rest = &text[position..];
        if let Some((length, _)) = matcher(rest) {
            if start < position {
                segment(&text[start..position]);
            }
            segment(&rest[..length]);
            position += length;
            start = position;
            continue;
        }
        position += unmatched_length(rest);
    }
    if start < text.len() {
        segment(&text[start..]);
    }
}

/// 未命中词典时前进的长度：一个完整的表意文字描述序列，否则一个字符。
fn unmatched_length(rest: &str) -> usize {
    ideographic_description_sequence_length(rest)
        .unwrap_or_else(|| rest.chars().next().map_or(rest.len(), char::len_utf8))
}

/// 与 `convert_pass` 相同的扫描，结果追加到 `output`。
fn append_pass(
    text: &str,
    matcher: impl Fn(&str) -> Option<(usize, &'static str)>,
    output: &mut String,
) {
    let mut position = 0;
    while position < text.len() {
        let rest = &text[position..];
        if let Some((length, value)) = matcher(rest) {
            output.push_str(value);
            position += length;
            continue;
        }
        let length = unmatched_length(rest);
        output.push_str(&rest[..length]);
        position += length;
    }
}

/// 繁转简两张表；只在第一次转换时解析。
struct SimplifiedTables {
    phrases: Table,
    characters: Table,
}

fn simplified_tables() -> &'static SimplifiedTables {
    static TABLES: OnceLock<SimplifiedTables> = OnceLock::new();
    TABLES.get_or_init(|| SimplifiedTables {
        phrases: Table::parse(&[TS_PHRASES]),
        characters: Table::parse(&[TS_CHARACTERS]),
    })
}

/// 按 OpenCC 的 `t2s.json` 把繁体转换成简体，不带 `TSCharactersExt`：先规范化兼容汉字，再按 `TSPhrases`、`TSCharacters` 的顺序做最大正向匹配，词组优先（乾淨→干净，乾隆不变）。没有简体形式的字原样保留。
pub fn traditional_to_simplified(text: &str) -> String {
    to_simplified(text, false)
}

/// 粤拼候选的繁转简：与 [`traditional_to_simplified`] 相同，只是 `CANTONESE_KEPT` 里的粤语用字保持原样。
pub fn cantonese_to_simplified(text: &str) -> String {
    to_simplified(text, true)
}

fn to_simplified(text: &str, keep_cantonese: bool) -> String {
    let normalized = convert_pass(text, |rest| tables().normalization.longest_prefix(rest));
    let tables = simplified_tables();
    let converted = convert_pass(&normalized, |rest| {
        tables.phrases.longest_prefix(rest).or_else(|| {
            if keep_cantonese {
                if let Some(kept) = CANTONESE_KEPT.iter().find(|kept| rest.starts_with(**kept)) {
                    return Some((kept.len(), *kept));
                }
            }
            tables.characters.longest_prefix(rest)
        })
    });
    match converted {
        Cow::Borrowed(_) => normalized.into_owned(),
        Cow::Owned(converted) => converted,
    }
}

/// Map each Traditional character to the Simplified character `STCharacters` derives it from, one character at a time.
///
/// This is not OpenCC's `t2s` (that is [`traditional_to_simplified`]): no phrase table, so a Traditional character that several Simplified ones share takes the first line that lists it. It exists to find a lookup key for a dictionary keyed by Simplified text, such as the candidate gloss tables, when the candidate itself is Traditional (a Korean Hanja like 韓, or a candidate under Traditional output). Characters no line lists, including ones that are already Simplified, pass through unchanged, and a compatibility ideograph is normalized first.
pub fn traditional_to_simplified_characters(text: &str) -> String {
    static INVERSE: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    let inverse = INVERSE.get_or_init(|| {
        let mut entries = HashMap::with_capacity(source_line_capacity(&[ST_CHARACTERS]));
        for line in ST_CHARACTERS.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((simplified, traditional)) = line.split_once('\t') else {
                continue;
            };
            for value in traditional.split_whitespace() {
                // A character listed as its own Traditional form (皇 for 皇) must stay itself rather than map to a sibling that happens to come first.
                if value != simplified {
                    entries.entry(value).or_insert(simplified);
                }
            }
        }
        entries
    });
    let normalized = convert_pass(text, |rest| tables().normalization.longest_prefix(rest));
    let mut output = String::with_capacity(normalized.len());
    for character in normalized.chars() {
        let mut buffer = [0_u8; 4];
        let key: &str = character.encode_utf8(&mut buffer);
        match inverse.get(key) {
            Some(simplified) => output.push_str(simplified),
            None => output.push(character),
        }
    }
    output
}

/// 按 OpenCC 最大正向匹配扫描；未变化时借用原文，首次变化后才物化结果。
fn convert_pass(
    text: &str,
    matcher: impl Fn(&str) -> Option<(usize, &'static str)>,
) -> Cow<'_, str> {
    let mut output: Option<String> = None;
    let mut position = 0;
    while position < text.len() {
        let rest = &text[position..];
        if let Some((length, value)) = matcher(rest) {
            if output.is_some() || &rest[..length] != value {
                let output = output.get_or_insert_with(|| {
                    let mut output = String::with_capacity(text.len() + text.len() / 5);
                    output.push_str(&text[..position]);
                    output
                });
                output.push_str(value);
            }
            position += length;
            continue;
        }
        let length = unmatched_length(rest);
        if let Some(output) = output.as_mut() {
            output.push_str(&rest[..length]);
        }
        position += length;
    }
    match output {
        Some(output) => Cow::Owned(output),
        None => Cow::Borrowed(text),
    }
}

fn ideographic_description_operator_arity(character: char) -> usize {
    match character {
        '\u{2FF2}' | '\u{2FF3}' => 3,
        '\u{2FFE}' | '\u{2FFF}' => 1,
        '\u{2FF0}'..='\u{2FFD}' => 2,
        _ => 0,
    }
}

/// Byte length of a complete ideographic description sequence at the start of `text`, if one starts there.
fn ideographic_description_sequence_length(text: &str) -> Option<usize> {
    let first = text.chars().next()?;
    if ideographic_description_operator_arity(first) == 0 {
        return None;
    }
    let mut code_points = 0;
    consume_ideographic_description_sequence(text, MAX_IDS_DEPTH, &mut code_points)
}

/// OpenCC's `ConsumeIdeographicDescriptionSequence`; `None` covers both its incomplete and invalid outcomes, which the conversion loop treats alike.
fn consume_ideographic_description_sequence(
    text: &str,
    depth_left: usize,
    code_points: &mut usize,
) -> Option<usize> {
    if depth_left == 0 || *code_points >= MAX_IDS_CODE_POINTS {
        return None;
    }
    let first = text.chars().next()?;
    *code_points += 1;
    let mut offset = first.len_utf8();
    for _ in 0..ideographic_description_operator_arity(first) {
        if offset >= text.len() {
            return None;
        }
        offset +=
            consume_ideographic_description_sequence(&text[offset..], depth_left - 1, code_points)?;
    }
    Some(offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traditional_characters_map_back_to_their_simplified_form() {
        for (traditional, simplified) in [
            ("韓", "韩"),
            ("漢", "汉"),
            ("閑", "闲"),
            ("寒", "寒"),
            ("大韓民國", "大韩民国"),
            ("한", "한"),
            ("abc", "abc"),
        ] {
            assert_eq!(
                traditional_to_simplified_characters(traditional),
                simplified,
                "{traditional}"
            );
        }
    }

    #[test]
    fn phrases_resolve_one_to_many_characters() {
        // Each of these goes wrong character by character: 发, 干, 面, 后, 里, 松, 余, 复, 借 have more than one Traditional form.
        for (simplified, traditional) in [
            ("头发", "頭髮"),
            ("发展", "發展"),
            ("干面", "乾麪"),
            ("皇后", "皇后"),
            ("后天", "後天"),
            ("里面", "裏面"),
            ("蓬松", "蓬鬆"),
            ("余下", "餘下"),
            ("答复", "答覆"),
            ("凭借", "憑藉"),
            ("汉语输入法", "漢語輸入法"),
        ] {
            assert_eq!(
                simplified_to_traditional(simplified),
                traditional,
                "{simplified}"
            );
        }
    }

    #[test]
    fn taiwan_variants_follow_opencc_configurations() {
        // 期望值是 OpenCC 用本目录这几份词典按 `s2t.json`、`s2tw.json`、`s2twp.json` 转换的结果。
        for (simplified, standard, taiwan, taiwan_phrases) in [
            ("头发", "頭髮", "頭髮", "頭髮"),
            ("里面", "裏面", "裡面", "裡面"),
            ("着急", "着急", "著急", "著急"),
            ("为什么", "爲什麼", "為什麼", "為什麼"),
            ("干面", "乾麪", "乾麵", "乾麵"),
            ("软件", "軟件", "軟件", "軟體"),
            ("鼠标", "鼠標", "鼠標", "滑鼠"),
            ("代码", "代碼", "代碼", "程式碼"),
            ("网络", "網絡", "網絡", "網路"),
            ("打印机", "打印機", "打印機", "印表機"),
            ("内存", "內存", "內存", "記憶體"),
            ("信息", "信息", "信息", "資訊"),
            ("出租车", "出租車", "出租車", "計程車"),
            ("视频", "視頻", "視頻", "影片"),
            (
                "以太网路由器",
                "以太網路由器",
                "以太網路由器",
                "乙太網路路由器",
            ),
            ("汉语输入法", "漢語輸入法", "漢語輸入法", "漢語輸入法"),
            ("⿰发发", "⿰发发", "⿰发发", "⿰发发"),
            ("", "", "", ""),
            ("hello 😀", "hello 😀", "hello 😀", "hello 😀"),
        ] {
            assert_eq!(
                convert_to_traditional(simplified, TraditionalVariant::Standard),
                standard,
                "s2t {simplified}"
            );
            assert_eq!(
                convert_to_traditional(simplified, TraditionalVariant::Taiwan),
                taiwan,
                "s2tw {simplified}"
            );
            assert_eq!(
                convert_to_traditional(simplified, TraditionalVariant::TaiwanPhrases),
                taiwan_phrases,
                "s2twp {simplified}"
            );
        }
    }

    #[test]
    fn segmentation_splits_like_opencc_max_match() {
        let table = Table::parse(&["甲乙\t甲乙\n"]);
        let segments = |text| {
            let mut segments = Vec::new();
            for_each_segment(
                text,
                |rest| table.longest_prefix(rest),
                |segment| segments.push(segment),
            );
            segments
        };
        assert_eq!(segments("丙甲乙丁戊"), ["丙", "甲乙", "丁戊"]);
        assert_eq!(segments("甲乙甲乙"), ["甲乙", "甲乙"]);
        assert_eq!(segments(""), Vec::<&str>::new());
        // 完整的描述序列整体并进未命中段，其中的部件不会被当成词组起点。
        assert_eq!(segments("⿰甲乙"), ["⿰甲乙"]);
    }

    #[test]
    fn variant_codes_and_names_round_trip() {
        for (code, name, variant) in [
            (0, "\"s2t\"", TraditionalVariant::Standard),
            (1, "\"s2tw\"", TraditionalVariant::Taiwan),
            (2, "\"s2twp\"", TraditionalVariant::TaiwanPhrases),
        ] {
            assert_eq!(TraditionalVariant::from_code(code), Some(variant));
            assert_eq!(serde_json::to_string(&variant).unwrap(), name);
            assert_eq!(
                serde_json::from_str::<TraditionalVariant>(name).unwrap(),
                variant
            );
        }
        assert_eq!(TraditionalVariant::from_code(3), None);
    }

    #[test]
    fn text_without_a_traditional_form_is_unchanged() {
        for text in [
            "",
            "abc",
            "ASCII 123",
            "a,b.c",
            "\t\n",
            "hello 😀",
            "かな",
            "漢語",
        ] {
            assert_eq!(simplified_to_traditional(text), text);
        }
    }

    #[test]
    fn traditional_to_simplified_prefers_phrases_like_opencc_t2s() {
        for (traditional, simplified) in [
            ("關係", "关系"),
            ("頭髮", "头发"),
            ("發展", "发展"),
            ("乾淨", "干净"),
            ("乾隆", "乾隆"),
            ("乾坤", "乾坤"),
            ("漢語輸入法", "汉语输入法"),
            ("已经是简体", "已经是简体"),
            ("abc 123 😀", "abc 123 😀"),
            ("", ""),
        ] {
            assert_eq!(traditional_to_simplified(traditional), simplified);
            assert_eq!(cantonese_to_simplified(traditional), simplified);
        }
    }

    #[test]
    fn cantonese_characters_survive_simplification() {
        // 字表里没有条目的粤语用字原样通过；有简体的照常转换。
        assert_eq!(
            cantonese_to_simplified("佢哋喺度嘅嘢咗冇嚟啲唔係"),
            "佢哋喺度嘅嘢咗冇嚟啲唔系"
        );
        assert_eq!(cantonese_to_simplified("諗講囉搵"), "谂讲啰揾");
        // 嗰、喎在字表里会变成扩展区的生僻字，粤拼输出保留原字；通用的 t2s 照上游转换。
        assert_eq!(cantonese_to_simplified("嗰個喎"), "嗰个喎");
        assert_eq!(traditional_to_simplified("嗰喎"), "𠮶㖞");
    }

    #[test]
    fn simplification_keeps_ideographic_description_sequences_whole() {
        assert_eq!(cantonese_to_simplified("⿰發發"), "⿰發發");
        assert_eq!(cantonese_to_simplified("發⿰發發"), "发⿰發發");
    }

    #[test]
    fn ideographic_description_sequences_pass_through_whole() {
        // 发 inside a complete sequence is a component, not a character, so it is left alone.
        assert_eq!(simplified_to_traditional("⿰发发"), "⿰发发");
        // An incomplete sequence is not one: the operator passes through and what follows converts.
        assert_eq!(simplified_to_traditional("⿰发"), "⿰發");
    }

    #[test]
    fn duplicate_keys_keep_the_earlier_source() {
        let table = Table::parse(&["a\tfirst\n", "a\tsecond\nab\tpair other\n"]);
        assert_eq!(table.longest_prefix("abc"), Some((2, "pair")));
        assert_eq!(table.longest_prefix("a"), Some((1, "first")));
        assert_eq!(table.longest, 2);
    }

    #[test]
    fn table_capacity_estimate_counts_source_lines() {
        assert_eq!(source_line_capacity(&["a\tb\nc\td\n", "e\tf\n"]), 3);
    }

    #[test]
    fn conversion_pass_preserves_prefix_and_replacement_boundaries() {
        let table = Table::parse(&["keep\tkeep\n变\texpanded-value\n長長\t短\na漢\t雙\n发\t發\n"]);
        for (input, expected) in [
            ("", ""),
            ("keep😀𠀀", "keep😀𠀀"),
            ("keep😀变𠀀", "keep😀expanded-value𠀀"),
            ("变長長变", "expanded-value短expanded-value"),
            ("keep長長a漢", "keep短雙"),
            ("𠀀😀a漢かな", "𠀀😀雙かな"),
            ("⿰发发", "⿰发发"),
            ("⿰发", "⿰發"),
        ] {
            assert_eq!(
                &*convert_pass(input, |rest| table.longest_prefix(rest)),
                expected,
                "{input}"
            );
        }
    }

    #[test]
    fn longest_prefix_handles_dictionary_initial_bytes() {
        let table = Table::parse(&[
            "#注释\t忽略\ninvalid\n\t忽略\nz\t  \nPN结\tPN結\nSQL注入\tSQL注入\nSQL注入攻击\tSQL注入攻擊\n\0\t零\n~\t符号\n\u{7f}\t边界\né\t重音\n漢\t汉\n𠀀\t辅助平面\n",
            "PN结\t后来源\n😀\t表情\n",
        ]);
        for (text, expected) in [
            ("PN结!", Some((5, "PN結"))),
            ("SQL注入攻击!", Some((15, "SQL注入攻擊"))),
            ("SQL注入?", Some((9, "SQL注入"))),
            ("\0tail", Some((1, "零"))),
            ("~tail", Some((1, "符号"))),
            ("\u{7f}tail", Some((1, "边界"))),
            ("é漢", Some((2, "重音"))),
            ("漢字", Some((3, "汉"))),
            ("𠀀字", Some((4, "辅助平面"))),
            ("😀字", Some((4, "表情"))),
            ("", None),
            ("z字", None),
            ("N字", None),
            ("Pwrong", None),
            ("ê字", None),
            ("汉字", None),
            ("😁字", None),
        ] {
            assert_eq!(table.longest_prefix(text), expected, "{text:?}");
        }
    }

    #[test]
    fn longest_prefix_matches_table_keys_across_utf8_boundaries() {
        let table = Table::parse(&[
            "a\tfirst\nab\tpair\nabc\ttriple\na漢\tmixed\na漢😀\temoji\n😀\tface\n😀𠀀\tplane\né\taccent\né漢\taccent-pair\n漢\tcharacter\n漢字\tphrase\n𠀀\tsupplementary\n",
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcdefghijklmnopqrstuvwxyz\tlong\n",
        ]);
        let empty = Table::parse(&[]);
        for text in [
            "", "a", "abcd", "ab漢", "a漢😀!", "😀𠀀漢", "é漢字", "漢字😀", "𠀀x", "missing",
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcdefghijklmnopqrstuvwxyz!",
        ] {
            // 按词条选择最长前缀，独立于查找端点的遍历方式。
            let expected = table.entries.iter()
                .filter(|(key, _)| text.starts_with(**key))
                .max_by_key(|(key, _)| key.len())
                .map(|(key, value)| (key.len(), *value));
            assert_eq!(table.longest_prefix(text), expected, "{text}");
            assert_eq!(empty.longest_prefix(text), None);
            for (end, _) in text.char_indices() {
                let prefix = &text[..end];
                let expected = table.entries.iter()
                    .filter(|(key, _)| prefix.starts_with(**key))
                    .max_by_key(|(key, _)| key.len())
                    .map(|(key, value)| (key.len(), *value));
                assert_eq!(table.longest_prefix(prefix), expected, "{prefix}");
            }
        }
    }
}
