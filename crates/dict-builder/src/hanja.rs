//! `hanja`：韩语方案把正在组字的音节转换为 Hanja 时使用的表，由 libhangul 的 `data/hanja/hanja.txt`（BSD-3-Clause，见 `resources/licenses/libhangul-hanja-BSD-3-Clause.txt`）生成；提交由 `resources/dictionary-sources.lock.json` 在 `ko/` 下固定。
//!
//! The source has one `key:value:comment` line per reading, words and single syllables mixed. Only single syllables are kept: the key is one precomposed Hangul syllable (U+AC00..U+D7A3) and the value one unified ideograph of the Basic Multilingual Plane, that is one Hanja in the CJK Unified Ideographs block, Extension A, or one of the twelve unified ideographs the CJK Compatibility Ideographs block holds (U+FA0E 﨎, U+FA11 﨑 and so on, which NFC leaves alone). True compatibility ideographs are dropped because NFC rewrites them; the pinned source lists none of them as a single-syllable value. Characters outside the Basic Multilingual Plane are dropped because fonts are not guaranteed to cover them. A value of two characters (the source has a few place names such as 莘洞) is not a single Hanja and is dropped too.
//!
//! The output is `crates/engine/src/korean/hanja.tsv`, one `syllable<TAB>hanja<TAB>gloss` line per reading, which the engine embeds with `include_str!`. Syllables come in code point order and each syllable's Hanja in the source's order, which libhangul keeps by frequency of use (韓, 漢, 寒 for 한). The gloss is the source's 훈음 comment (나라 이름 한), empty where the source has none. A Hanja the source lists twice under one syllable is kept at its first position.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{bail, Context, Result};

pub const SOURCE: &str = "ko/hanja.txt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    pub syllable: char,
    pub hanja: char,
    pub gloss: String,
}

pub fn is_hangul_syllable(character: char) -> bool {
    ('\u{ac00}'..='\u{d7a3}').contains(&character)
}

/// The twelve code points of the CJK Compatibility Ideographs block that are unified ideographs (Unified_Ideograph=Yes, no decomposition), so NFC leaves them alone. The rest of the block are true compatibility ideographs.
const UNIFIED_IN_COMPATIBILITY_BLOCK: [char; 12] = [
    '\u{fa0e}', '\u{fa0f}', '\u{fa11}', '\u{fa13}', '\u{fa14}', '\u{fa1f}', '\u{fa21}', '\u{fa23}',
    '\u{fa24}', '\u{fa27}', '\u{fa28}', '\u{fa29}',
];

/// The unified ideographs of the Basic Multilingual Plane the table keeps: CJK Unified Ideographs, Extension A, and the twelve unified ideographs of the compatibility block.
pub fn is_kept_hanja(character: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&character)
        || ('\u{3400}'..='\u{4dbf}').contains(&character)
        || UNIFIED_IN_COMPATIBILITY_BLOCK.contains(&character)
}

/// The single-syllable readings of `source`, grouped by syllable in code point order with each group in source order.
pub fn build(source: &str) -> Result<Vec<Reading>> {
    let mut groups: BTreeMap<char, Vec<Reading>> = BTreeMap::new();
    let mut seen = HashSet::new();
    for (index, line) in source.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.splitn(3, ':');
        let (Some(key), Some(value), Some(comment)) = (fields.next(), fields.next(), fields.next())
        else {
            bail!("{SOURCE}: line {} is not key:value:comment", index + 1);
        };
        let mut key_characters = key.chars();
        let (Some(syllable), None) = (key_characters.next(), key_characters.next()) else {
            continue;
        };
        if !is_hangul_syllable(syllable) {
            continue;
        }
        let mut value_characters = value.chars();
        let (Some(hanja), None) = (value_characters.next(), value_characters.next()) else {
            continue;
        };
        if !is_kept_hanja(hanja) {
            continue;
        }
        let gloss = comment.trim();
        if gloss.contains(['\t', '\r']) {
            bail!(
                "{SOURCE}: line {} has a control character in its comment",
                index + 1
            );
        }
        if !seen.insert((syllable, hanja)) {
            continue;
        }
        groups.entry(syllable).or_default().push(Reading {
            syllable,
            hanja,
            gloss: gloss.to_owned(),
        });
    }
    if groups.is_empty() {
        bail!("{SOURCE}: no single-syllable reading");
    }
    Ok(groups.into_values().flatten().collect())
}

pub fn tsv(readings: &[Reading]) -> String {
    let mut text = String::new();
    for reading in readings {
        let _ = writeln!(
            text,
            "{}\t{}\t{}",
            reading.syllable, reading.hanja, reading.gloss
        );
    }
    text
}

pub fn write(readings: &[Reading], out: &Path) -> Result<()> {
    std::fs::write(out, tsv(readings)).with_context(|| format!("writing {}", out.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE_TEXT: &str = "# Copyright (c) 2005,2006 Choe Hwanjin\n#\n\nㄱ자집:ㄱ字집:\n가:家:집 가\n가:佳:아름다울 가\n가结:家結:\n가:䄷:\n가:\u{f900}:\n가:\u{2f800}:\n가:𠀋:\n가:家:집 가\n신:莘洞:지명\n한:韓:나라 이름 한, 한나라 한\n한:漢:한수 한\n한:犴:\n각:角:뿔 각\nkey:value:\n";

    fn glyphs(readings: &[Reading]) -> Vec<String> {
        readings
            .iter()
            .map(|reading| format!("{}{}", reading.syllable, reading.hanja))
            .collect()
    }

    #[test]
    fn only_single_syllables_with_one_kept_hanja_remain() {
        let readings = build(SOURCE_TEXT).unwrap();
        // U+F900 and U+2F800 are compatibility ideographs, 𠀋 lies outside the BMP, 莘洞 is two characters, and the word, jamo and ASCII keys are not syllables.
        assert_eq!(
            glyphs(&readings),
            ["가家", "가佳", "가䄷", "각角", "한韓", "한漢", "한犴"]
        );
    }

    #[test]
    fn unified_ideographs_in_the_compatibility_block_are_kept() {
        // U+FA11 (﨑) and U+FA0E (﨎) sit in the compatibility block but are unified ideographs that NFC leaves alone, unlike U+F900 and U+FA0D, which NFC rewrites.
        let readings = build("기:\u{fa11}:\n쌍:\u{fa0e}:\n가:\u{f900}:\n가:\u{fa0d}:\n").unwrap();
        assert_eq!(glyphs(&readings), ["기\u{fa11}", "쌍\u{fa0e}"]);
        let unified: Vec<char> = ('\u{f900}'..='\u{faff}')
            .filter(|&character| is_kept_hanja(character))
            .collect();
        assert_eq!(
            unified,
            [
                '\u{fa0e}', '\u{fa0f}', '\u{fa11}', '\u{fa13}', '\u{fa14}', '\u{fa1f}', '\u{fa21}',
                '\u{fa23}', '\u{fa24}', '\u{fa27}', '\u{fa28}', '\u{fa29}'
            ]
        );
    }

    #[test]
    fn syllables_sort_by_code_point_and_keep_their_source_order() {
        let readings = build("한:韓:\n가:佳:\n한:漢:\n가:家:\n").unwrap();
        assert_eq!(glyphs(&readings), ["가佳", "가家", "한韓", "한漢"]);
    }

    #[test]
    fn a_repeated_hanja_keeps_its_first_position_and_gloss() {
        let readings = build("가:家:집 가\n가:佳:\n가:家:다른 뜻\n").unwrap();
        assert_eq!(glyphs(&readings), ["가家", "가佳"]);
        assert_eq!(readings[0].gloss, "집 가");
    }

    #[test]
    fn the_table_is_one_line_per_reading_with_the_gloss_last() {
        let readings = build(SOURCE_TEXT).unwrap();
        let table = tsv(&readings);
        assert!(table.starts_with("가\t家\t집 가\n가\t佳\t아름다울 가\n가\t䄷\t\n"));
        assert!(table.contains("한\t韓\t나라 이름 한, 한나라 한\n"));
        assert_eq!(table.lines().count(), readings.len());
        // The same input writes the same bytes.
        assert_eq!(tsv(&build(SOURCE_TEXT).unwrap()), table);
    }

    #[test]
    fn a_malformed_line_or_an_empty_result_fails() {
        let error = build("가:家\n").unwrap_err().to_string();
        assert!(error.contains("line 1"), "{error}");
        let error = build("# only a header\n").unwrap_err().to_string();
        assert!(error.contains("no single-syllable reading"), "{error}");
    }
}
