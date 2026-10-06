//! Word-level Simplified-to-Traditional conversion, matching OpenCC's `s2t.json`.
//!
//! The reference Windows server converts committed text and candidates with OpenCC's standard `s2t` configuration. Character-by-character platform tables (`LCMapStringEx`, ICU's transliterator) cannot resolve one-to-many characters such as 发 (發/髮), 干 (乾/幹/干) or 面 (面/麵); OpenCC resolves them with a phrase table first. This module reproduces that configuration from the same OpenCC data, pinned to the commit the reference vendors, so a host gets the reference's output without linking OpenCC's C++ library.
//!
//! The configuration, from OpenCC's `data/config/s2t.json`:
//!
//! - normalization: one maximum-forward-match pass over `CJK_Compatibility_Ideographs`;
//! - conversion: one maximum-forward-match pass over a short-circuit group whose first member is the union of `STPhrases` and `STPhrases_GeneratedFromRegionalPhrases` and whose second member is `STCharacters`.
//!
//! Where a dictionary lists several values the first is taken, as OpenCC does. Text no dictionary matches passes through unchanged; a complete ideographic description sequence (`⿰女尔`) passes through as one unit, so its components are never converted on their own.

use std::collections::HashMap;
use std::sync::OnceLock;

const CJK_COMPATIBILITY_IDEOGRAPHS: &str =
    include_str!("../data/opencc/CJK_Compatibility_Ideographs.txt");
const ST_PHRASES: &str = include_str!("../data/opencc/STPhrases.txt");
const ST_PHRASES_REGIONAL: &str =
    include_str!("../data/opencc/STPhrases_GeneratedFromRegionalPhrases.txt");
const ST_CHARACTERS: &str = include_str!("../data/opencc/STCharacters.txt");

/// Bounds from OpenCC's `UTF8Util::NextIdeographicDescriptionSequenceLength`.
const MAX_IDS_DEPTH: usize = 16;
const MAX_IDS_CODE_POINTS: usize = 64;

/// One OpenCC dictionary: key to its first value, plus the longest key in characters.
struct Table {
    entries: HashMap<&'static str, &'static str>,
    longest: usize,
}

fn source_line_capacity(sources: &[&str]) -> usize {
    sources.iter().map(|source| source.lines().count()).sum()
}

impl Table {
    /// Parse OpenCC's text format. Earlier sources win on duplicate keys, which is what the union group does when two members match the same length.
    fn parse(sources: &[&'static str]) -> Self {
        let mut entries = HashMap::with_capacity(source_line_capacity(sources));
        let mut longest = 0;
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
                longest = longest.max(key.chars().count());
                entries.entry(key).or_insert(value);
            }
        }
        Self { entries, longest }
    }

    /// The longest key that prefixes `text`, as (key byte length, value).
    fn longest_prefix(&self, text: &str) -> Option<(usize, &'static str)> {
        let mut ends = Vec::with_capacity(self.longest);
        for (offset, character) in text.char_indices().take(self.longest) {
            ends.push(offset + character.len_utf8());
        }
        ends.into_iter()
            .rev()
            .find_map(|end| self.entries.get(&text[..end]).map(|value| (end, *value)))
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

/// Convert Simplified Chinese to Traditional Chinese the way OpenCC's `s2t.json` does.
///
/// Characters with no Traditional form (ASCII, kana, emoji, already-Traditional text) are returned unchanged.
pub fn simplified_to_traditional(text: &str) -> String {
    let tables = tables();
    let normalized = convert_pass(text, |rest| tables.normalization.longest_prefix(rest));
    convert_pass(&normalized, |rest| {
        tables
            .phrases
            .longest_prefix(rest)
            .or_else(|| tables.characters.longest_prefix(rest))
    })
}

/// Map each Traditional character to the Simplified character `STCharacters` derives it from, one character at a time.
///
/// This is not OpenCC's `t2s`: no phrase table, so a Traditional character that several Simplified ones share takes the first line that lists it. It exists to find a lookup key for a dictionary keyed by Simplified text, such as the candidate gloss tables, when the candidate itself is Traditional (a Korean Hanja like 韓, or a candidate under Traditional output). Characters no line lists, including ones that are already Simplified, pass through unchanged, and a compatibility ideograph is normalized first.
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

/// OpenCC's `Conversion::AppendConverted`: maximum forward match, unmatched text copied through.
fn convert_pass(text: &str, matcher: impl Fn(&str) -> Option<(usize, &'static str)>) -> String {
    let mut output = String::with_capacity(text.len() + text.len() / 5);
    let mut position = 0;
    while position < text.len() {
        let rest = &text[position..];
        if let Some((length, value)) = matcher(rest) {
            output.push_str(value);
            position += length;
            continue;
        }
        let length = ideographic_description_sequence_length(rest)
            .unwrap_or_else(|| rest.chars().next().map_or(rest.len(), char::len_utf8));
        output.push_str(&rest[..length]);
        position += length;
    }
    output
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
}
