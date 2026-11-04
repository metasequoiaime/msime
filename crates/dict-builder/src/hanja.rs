//! `hanja`：韩语方案把正在组字的音节转换为 Hanja 时使用的表，由 msime-dictionary 原样收录的 `sources/korean/hanja.txt`（libhangul 的 `data/hanja/hanja.txt`，BSD-3-Clause，见 `resources/licenses/libhangul-hanja-BSD-3-Clause.txt`；libhangul 提交见锁文件的 `libhangul` 引用）生成，命令是 `msime-dict-build hanja --dictionary <checkout>`。
//!
//! The source has one `key:value:comment` line per reading, words and single syllables mixed. Only single syllables are kept: the key is one precomposed Hangul syllable (U+AC00..U+D7A3) and the value one unified ideograph of the Basic Multilingual Plane, that is one Hanja in the CJK Unified Ideographs block, Extension A, or one of the twelve unified ideographs the CJK Compatibility Ideographs block holds (U+FA0E 﨎, U+FA11 﨑 and so on, which NFC leaves alone). True compatibility ideographs are dropped because NFC rewrites them; the pinned source lists none of them as a single-syllable value. Characters outside the Basic Multilingual Plane are dropped because fonts are not guaranteed to cover them. A value of two characters (the source has a few place names such as 莘洞) is not a single Hanja and is dropped too.
//!
//! 输出到 `crates/engine/src/korean/hanja.tsv`，每个读音一行 `syllable<TAB>hanja<TAB>gloss`，引擎用 `include_str!` 内嵌。音节按码位排序；训音取源文件的 훈음 注释（나라 이름 한），源文件没有时留空。同一音节重复列出的汉字只保留第一次的位置。
//!
//! 每个音节里先列有训音的字、再列没有训音的字，两组内部各按源文件的次序（libhangul 大体按使用频率排，한 下是 韓、漢、寒）。源文件约四分之三的行没有训音，其中夹着不少简体字和冷僻字，排在有训音的常用字前面：팽 下第一个是没有训音的 庄，彭、膨 在后面；시 下第一个是 十。只是指向另一个字的注释（`啍과 同字`、`畫의 俗字` 这类，每一段都含汉字）不算训音，这样的行和没有注释的行排在一组：噋 只注了「啍과 同字」，如果算有训音，它就会排到 톤 下没有注释的常用字 噸 前面。按这条规则，固定提交里有 7 个音节的首字换了（팽 庄→彭、시 十→時、즙 輯→汁、즉 則→卽、획 畵→獲、흘 汽→訖、뉵 忸→衄），没有一个换成更冷僻的字。
//!
//! 没有训音的字里，只作简体字形的字（OpenCC `STCharacters.txt` 里繁体字形不含它自己的字，如 双、兴、层，由 `simplified_only_forms` 挑出，表从同一个 `--dictionary` checkout 的 `sources/opencc/STCharacters.txt` 读，只用来排序，不写进表里）再排到最后：韩文里用的是 雙、興、層，按「有没有训音」分组之后，这些简体字形还排在只有一个有训音的字之后的第二、三位（쌍 雙 双、흥 興 兴）。有训音的简体字形（脚、强 这类韩文也用的略字）不动。固定提交里这条规则重排了 397 个音节，没有一个音节的首字变；前三位里有简体字形的音节从 24 个降到 17 个，剩下的是有训音的略字和总共只有两三个字的音节。

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{bail, Context, Result};

pub const SOURCE: &str = "sources/korean/hanja.txt";

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

/// 提取 `source` 的单音节读音，按音节码位分组。每组先列有训音的字、再列无训音的非简体字形，最后列无训音且只作简体字形的字（`simplified_only_forms` 给出的 `simplified_only`），组内保留源文件的顺序。
pub fn build(source: &str, simplified_only: &HashSet<char>) -> Result<Vec<Reading>> {
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
    for readings in groups.values_mut() {
        // 稳定排序：有训音的在前，再是没有训音的，最后是没有训音的简体字形，组内保留源文件的次序。
        readings.sort_by_key(|reading| {
            if has_reading_gloss(&reading.gloss) {
                0
            } else if simplified_only.contains(&reading.hanja) {
                2
            } else {
                1
            }
        });
    }
    Ok(groups.into_values().flatten().collect())
}

/// 注释里有没有训音：逗号分开的各段里至少有一段不含汉字（`집 가`）。整条注释只是指向别的字（`歌와 同字`、`畫의 俗字`）或为空时没有。
fn has_reading_gloss(gloss: &str) -> bool {
    let ideograph = |character: char| matches!(character, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{3ffff}');
    gloss
        .split(',')
        .map(str::trim)
        .any(|part| !part.is_empty() && !part.chars().any(ideograph))
}

/// OpenCC 简转繁字表（`STCharacters.txt`，`cantonese::parse_variants` 读出的行）里只作简体字形的字：繁体字形不含它自己（双 → 雙、兴 → 興）。台 → 臺 檯 颱 台 这类繁体字形里也有它自己的字不算。
pub fn simplified_only_forms(simplified_to_traditional: &[(char, Vec<char>)]) -> HashSet<char> {
    simplified_to_traditional
        .iter()
        .filter(|(simplified, traditional)| !traditional.contains(simplified))
        .map(|(simplified, _)| *simplified)
        .collect()
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

    fn built(source: &str) -> Result<Vec<Reading>> {
        build(source, &HashSet::new())
    }

    fn glyphs(readings: &[Reading]) -> Vec<String> {
        readings
            .iter()
            .map(|reading| format!("{}{}", reading.syllable, reading.hanja))
            .collect()
    }

    #[test]
    fn only_single_syllables_with_one_kept_hanja_remain() {
        let readings = built(SOURCE_TEXT).unwrap();
        // U+F900 and U+2F800 are compatibility ideographs, 𠀋 lies outside the BMP, 莘洞 is two characters, and the word, jamo and ASCII keys are not syllables.
        assert_eq!(
            glyphs(&readings),
            ["가家", "가佳", "가䄷", "각角", "한韓", "한漢", "한犴"]
        );
    }

    #[test]
    fn glossed_readings_come_before_unglossed_ones_in_source_order() {
        let readings = built(
            "팽:庄:
팽:彭:성씨 팽
팽:硼:
팽:膨:배 불룩할 팽
",
        )
        .unwrap();
        assert_eq!(glyphs(&readings), ["팽彭", "팽膨", "팽庄", "팽硼"]);
    }

    #[test]
    fn simplified_only_forms_without_a_gloss_come_last() {
        let simplified_only = simplified_only_forms(&[
            ('双', vec!['雙']),
            ('台', vec!['臺', '檯', '颱', '台']),
            ('脚', vec!['腳']),
        ]);
        assert_eq!(simplified_only, HashSet::from(['双', '脚']));
        // 双 没有训音、只作简体字形，排到没有训音的 䨇 后面；台 的繁体字形里有它自己，不算简体字形；脚 有训音，留在有训音的一组里。
        let readings = build(
            "쌍:雙:쌍 쌍\n쌍:双:\n쌍:䨇:\n대:台:\n대:臺:\n각:各:각각 각\n각:脚:다리 각\n각:角:뿔 각\n",
            &simplified_only,
        )
        .unwrap();
        assert_eq!(
            glyphs(&readings),
            ["각各", "각脚", "각角", "대台", "대臺", "쌍雙", "쌍䨇", "쌍双"]
        );
    }

    #[test]
    fn a_comment_that_only_points_to_another_hanja_is_not_a_gloss() {
        // 噋 只注了指向 啍 的异体说明，和没有注释的 噸 同组、按源文件次序排在 噸 后面；鑒 的注释除了指向 鑑 还有训音，算有训音；注释原样写进第三列。
        let readings = built("톤:噸:\n톤:噋:啍과 同字\n톤:鑒:鑑과 同字, 거울 감\n").unwrap();
        assert_eq!(glyphs(&readings), ["톤鑒", "톤噸", "톤噋"]);
        assert_eq!(readings[2].gloss, "啍과 同字");
        assert!(has_reading_gloss("집 가"));
        assert!(has_reading_gloss("鑑과 同字, 거울 감"));
        assert!(!has_reading_gloss("畫의 俗字"));
        assert!(!has_reading_gloss(""));
    }

    #[test]
    fn unified_ideographs_in_the_compatibility_block_are_kept() {
        // U+FA11 (﨑) and U+FA0E (﨎) sit in the compatibility block but are unified ideographs that NFC leaves alone, unlike U+F900 and U+FA0D, which NFC rewrites.
        let readings = built("기:\u{fa11}:\n쌍:\u{fa0e}:\n가:\u{f900}:\n가:\u{fa0d}:\n").unwrap();
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
        let readings = built("한:韓:\n가:佳:\n한:漢:\n가:家:\n").unwrap();
        assert_eq!(glyphs(&readings), ["가佳", "가家", "한韓", "한漢"]);
    }

    #[test]
    fn a_repeated_hanja_keeps_its_first_position_and_gloss() {
        let readings = built("가:佳:\n가:家:집 가\n가:佳:다른 뜻\n").unwrap();
        // 第二次出现的 佳 带着训音也不算，佳 仍是没有训音的行，排在 家 后面。
        assert_eq!(glyphs(&readings), ["가家", "가佳"]);
        assert_eq!(readings[1].gloss, "");
        assert_eq!(readings[0].gloss, "집 가");
    }

    #[test]
    fn the_table_is_one_line_per_reading_with_the_gloss_last() {
        let readings = built(SOURCE_TEXT).unwrap();
        let table = tsv(&readings);
        assert!(table.starts_with("가\t家\t집 가\n가\t佳\t아름다울 가\n가\t䄷\t\n"));
        assert!(table.contains("한\t韓\t나라 이름 한, 한나라 한\n"));
        assert_eq!(table.lines().count(), readings.len());
        // The same input writes the same bytes.
        assert_eq!(tsv(&built(SOURCE_TEXT).unwrap()), table);
    }

    #[test]
    fn a_malformed_line_or_an_empty_result_fails() {
        let error = built("가:家\n").unwrap_err().to_string();
        assert!(error.contains("line 1"), "{error}");
        let error = built("# only a header\n").unwrap_err().to_string();
        assert!(error.contains("no single-syllable reading"), "{error}");
    }
}
