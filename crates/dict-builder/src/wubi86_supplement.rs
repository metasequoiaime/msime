//! `wubi86-supplement`: generates msime-dictionary's `sources/wubi/wubi86-supplement.txt`, the multi-character words the 86 wubi table (`sources/wubi/wubi86-jidian.txt`) lacks, coded by the 86 word rules from the table's own single-character codes.
//!
//! A word is taken when the jidian table has it under no code and either the 98 wubi tables (`sources/wubi/wubi98.txt` and `sources/wubi/wubi98-fcitx.txt`) list it, or `sources/pinyin/rime-ice.txt` has it as a two-character word at a weight of at least `MIN_PINYIN_WEIGHT`. Only words made entirely of Han characters count. The 98 tables are curated word lists, so every word of two or more characters is taken from them; rime-ice's weights come from a corpus that also yields fragments (被他, 请把), so from it only two-character words above the threshold are taken. Its hand-filled weights such as 9999 are not told apart from frequencies.
//!
//! The code follows the 86 word rules over each character's full code, the longest code the jidian table gives the character alone: a two-character word takes the first two letters of each character, a three-character word the first letter of the first two and the first two letters of the third, a longer word the first letter of the first, second, third and last characters. A character the table does not code alone, or whose full codes disagree on the letters a word needs (radicals and a few hundred rare characters), leaves its words out. Over the jidian table's own words the rules reproduce 63,095 of 63,127 codes; the rest are the table's own short codes and decompositions (占比 hxx, 蹂躏 kcka).
//!
//! The supplement never moves a jidian row. Under its code a word is weighted below the lowest jidian row of that code and no higher than `SUPPLEMENT_CEILING`, below every ranked jidian word (whose lowest weight is 10), so in a prefix list it also follows them. Several words of one code are ordered by their rime-ice weight, highest first, then by word, and step down one each from that ceiling to 0; the build inserts the file after the jidian table, so equal weights keep jidian rows first and the file's order within a code. A code a jidian word answered alone on four letters, which the input method commits by itself, stops doing so once a supplement word shares it; the report counts those codes.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

use anyhow::{bail, Result};

use crate::msime;
use crate::places_supplement::{is_han, weighted_rows};
use crate::text;

pub const OUTPUT: &str = "sources/wubi/wubi86-supplement.txt";
pub const JIDIAN: &str = "sources/wubi/wubi86-jidian.txt";
pub const WUBI98: &str = "sources/wubi/wubi98.txt";
pub const WUBI98_FCITX: &str = "sources/wubi/wubi98-fcitx.txt";
pub const BASE: &str = "sources/pinyin/rime-ice.txt";

/// The rime-ice weight from which a two-character word missing from the 86 table is taken.
pub const MIN_PINYIN_WEIGHT: i64 = 5000;

/// The highest weight a supplement row gets: one below the lowest weight of a ranked jidian word.
const SUPPLEMENT_CEILING: i64 = 9;

/// How many examples a report line names.
const REPORT_EXAMPLES: usize = 20;

pub struct Inputs<'a> {
    pub jidian: &'a str,
    /// `sources/wubi/wubi98.txt`, already decoded from UTF-16LE.
    pub wubi98: &'a str,
    pub wubi98_fcitx: &'a str,
    pub base: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub word: String,
    pub code: String,
    pub weight: i64,
    pub from_wubi98: bool,
    pub from_pinyin: bool,
}

pub struct Supplement {
    /// Ordered by code, then best first within a code.
    pub entries: Vec<Entry>,
    /// Candidates no code could be derived for, with the character that stopped them.
    pub underivable: Vec<(String, char)>,
    /// Four-letter codes that had exactly one jidian row and now have supplement rows too.
    pub unique_codes_shared: usize,
}

/// The jidian table's rows: `(code, value, weight)`.
fn jidian_rows(source: &str) -> Vec<(String, &str, i64)> {
    text::universal_lines(text::without_bom(source))
        .into_iter()
        .filter_map(|line| msime::parse_code_line(line, false))
        .collect()
}

/// Each character's full codes: the longest codes the table gives it alone.
fn full_codes(rows: &[(String, &str, i64)]) -> HashMap<char, Vec<String>> {
    let mut codes: HashMap<char, Vec<String>> = HashMap::new();
    for (code, value, _) in rows {
        let mut chars = value.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            continue;
        };
        let slot = codes.entry(c).or_default();
        match slot.first().map(String::len) {
            Some(longest) if longest > code.len() => {}
            Some(longest) if longest == code.len() => {
                if !slot.contains(code) {
                    slot.push(code.clone());
                }
            }
            _ => *slot = vec![code.clone()],
        }
    }
    codes
}

/// The first `letters` letters of `c`'s full code, when every full code has them and they agree.
fn prefix(codes: &HashMap<char, Vec<String>>, c: char, letters: usize) -> Option<&str> {
    let full = codes.get(&c)?;
    let first = full.first()?.get(..letters)?;
    full.iter()
        .all(|code| code.get(..letters) == Some(first))
        .then_some(first)
}

/// The 86 word code of `word`, or the character that has no usable full code.
fn word_code(codes: &HashMap<char, Vec<String>>, word: &str) -> std::result::Result<String, char> {
    let chars: Vec<char> = word.chars().collect();
    let parts: Vec<(char, usize)> = match chars.len() {
        0 | 1 => unreachable!("only words of two or more characters are coded"),
        2 => vec![(chars[0], 2), (chars[1], 2)],
        3 => vec![(chars[0], 1), (chars[1], 1), (chars[2], 2)],
        n => vec![
            (chars[0], 1),
            (chars[1], 1),
            (chars[2], 1),
            (chars[n - 1], 1),
        ],
    };
    let mut code = String::with_capacity(4);
    for (c, letters) in parts {
        code.push_str(prefix(codes, c, letters).ok_or(c)?);
    }
    Ok(code)
}

fn is_word(value: &str) -> bool {
    value.chars().count() >= 2 && value.chars().all(is_han)
}

pub fn build(inputs: &Inputs) -> Result<Supplement> {
    let rows = jidian_rows(inputs.jidian);
    if rows.is_empty() {
        bail!("{JIDIAN} has no rows");
    }
    let codes = full_codes(&rows);
    let jidian_words: HashSet<&str> = rows
        .iter()
        .filter(|(_, value, _)| value.chars().count() >= 2)
        .map(|(_, value, _)| *value)
        .collect();
    let mut lowest: HashMap<&str, i64> = HashMap::new();
    let mut per_code: HashMap<&str, usize> = HashMap::new();
    for (code, _, weight) in &rows {
        let slot = lowest.entry(code.as_str()).or_insert(*weight);
        *slot = (*slot).min(*weight);
        *per_code.entry(code.as_str()).or_default() += 1;
    }

    let mut frequency: HashMap<&str, i64> = HashMap::new();
    for (value, _, weight) in weighted_rows(inputs.base) {
        let slot = frequency.entry(value).or_insert(weight);
        *slot = (*slot).max(weight);
    }

    let mut wubi98: BTreeSet<String> = BTreeSet::new();
    for line in text::universal_lines(text::without_bom(inputs.wubi98)) {
        if let Some((_, value)) = msime::parse_wubi98_line(line) {
            wubi98.insert(value.to_owned());
        }
    }
    for line in text::universal_lines(inputs.wubi98_fcitx) {
        if let Some((_, value)) = msime::parse_fcitx_wubi98_line(line) {
            wubi98.insert(value);
        }
    }
    if wubi98.is_empty() {
        bail!("{WUBI98} and {WUBI98_FCITX} have no rows");
    }
    let pinyin: BTreeSet<&str> = frequency
        .iter()
        .filter(|(word, weight)| word.chars().count() == 2 && **weight >= MIN_PINYIN_WEIGHT)
        .map(|(word, _)| *word)
        .collect();

    let candidates: BTreeSet<&str> = wubi98
        .iter()
        .map(String::as_str)
        .chain(pinyin.iter().copied())
        .filter(|word| is_word(word) && !jidian_words.contains(word))
        .collect();

    let mut by_code: BTreeMap<String, Vec<(&str, Option<i64>)>> = BTreeMap::new();
    let mut underivable = Vec::new();
    for word in candidates {
        match word_code(&codes, word) {
            Ok(code) => by_code
                .entry(code)
                .or_default()
                .push((word, frequency.get(word).copied())),
            Err(c) => underivable.push((word.to_owned(), c)),
        }
    }

    let mut entries = Vec::new();
    let mut unique_codes_shared = 0;
    for (code, mut words) in by_code {
        // Highest rime-ice weight first, words rime-ice lacks last, then by word so the order does not depend on hash order.
        words.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let ceiling = match lowest.get(code.as_str()) {
            Some(lowest) => SUPPLEMENT_CEILING.min(lowest - 1),
            None => SUPPLEMENT_CEILING,
        };
        if code.len() == 4 && per_code.get(code.as_str()) == Some(&1) {
            unique_codes_shared += 1;
        }
        for (rank, (word, _)) in words.into_iter().enumerate() {
            entries.push(Entry {
                word: word.to_owned(),
                code: code.clone(),
                weight: (ceiling - rank as i64).max(0),
                from_wubi98: wubi98.contains(word),
                from_pinyin: pinyin.contains(word),
            });
        }
    }
    Ok(Supplement {
        entries,
        underivable,
        unique_codes_shared,
    })
}

/// Facts the header records, so a reader can reproduce the file.
pub struct Provenance<'a> {
    /// `(path, sha256)` of every input.
    pub inputs: &'a [(&'a str, &'a str)],
}

pub fn render(supplement: &Supplement, provenance: &Provenance) -> String {
    let inputs = provenance
        .inputs
        .iter()
        .map(|(path, sha256)| format!("{path}（SHA-256 {sha256}）"))
        .collect::<Vec<_>>()
        .join("、");
    let mut out = String::new();
    let _ = writeln!(out, "# 86 五笔词组补充表，由 msime 仓库 crates/dict-builder/src/wubi86_supplement.rs 的 `msime-dict-build wubi86-supplement --cache <dir> --out sources/wubi/wubi86-supplement.txt` 生成，生成器所在的提交就是在 resources/dictionary-sources.lock.json 里固定本文件的 msime 提交；不要手工编辑。");
    let _ = writeln!(out, "# 输入：{inputs}。");
    let _ = writeln!(out, "# 收录：{JIDIAN} 在任何编码下都没有、且全部由汉字组成的词，满足其一即收：在 98 五笔的 {WUBI98} 或 {WUBI98_FCITX} 里是两字及以上的词；或在 {BASE} 里是权重不低于 {MIN_PINYIN_WEIGHT} 的二字词（9999 这类人工填写的权重不与词频区分）。");
    let _ = writeln!(out, "# 编码：按 86 版词组规则，取 {JIDIAN} 里单字的全码（该字单独出现时最长的编码）：二字词各取前两码；三字词取前两字的首码和第三字的前两码；四字及以上取第一、二、三字和末字的首码。单字表里没有、或几个全码在所需码位上不一致的字，含它的词不收。");
    let _ = writeln!(out, "# 去重与权重：对照集合是 {JIDIAN} 的全部两字及以上词条，已有的词不论编码一律不收。同一编码下补充词排在 {JIDIAN} 原有词条之后：最高权重取 {SUPPLEMENT_CEILING} 与该编码原有最低权重减 1 中较小者，几个补充词按 {BASE} 的权重从高到低（没有的排最后，再按词）依次减 1，最低为 0；构建的 wubi 阶段在 {JIDIAN} 之后并入本文件，同权重时原有词条在前。");
    for entry in &supplement.entries {
        let _ = writeln!(out, "{}\t{}\t{}", entry.word, entry.code, entry.weight);
    }
    out
}

/// The generator's account of what it took and left out, one line per group, for review.
pub fn report(supplement: &Supplement) -> Vec<String> {
    let both = supplement
        .entries
        .iter()
        .filter(|entry| entry.from_wubi98 && entry.from_pinyin)
        .count();
    let wubi98 = supplement
        .entries
        .iter()
        .filter(|entry| entry.from_wubi98)
        .count();
    let pinyin = supplement
        .entries
        .iter()
        .filter(|entry| entry.from_pinyin)
        .count();
    let codes: HashSet<&str> = supplement
        .entries
        .iter()
        .map(|entry| entry.code.as_str())
        .collect();
    let examples = supplement
        .underivable
        .iter()
        .take(REPORT_EXAMPLES)
        .map(|(word, c)| format!("{word} ({c})"))
        .collect::<Vec<_>>()
        .join(" ");
    vec![
        format!(
            "{} words under {} codes: {wubi98} from the 98 tables, {pinyin} from {BASE} at weight {MIN_PINYIN_WEIGHT} or above, {both} from both",
            supplement.entries.len(),
            codes.len()
        ),
        format!(
            "{} four-letter codes that one jidian row answered alone now have supplement rows too, so they no longer commit by themselves",
            supplement.unique_codes_shared
        ),
        format!(
            "{} candidates left out for a character without a usable full code: {examples}",
            supplement.underivable.len()
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const JIDIAN_FIXTURE: &str = "\u{feff}部\tukbh\t10\r\n门\tuyhn\t10\r\n冲\tukh\t10\r\n冲\tukhh\t10\r\n凉\tuyiy\t10\r\n冻\tuyaa\t10\r\n部门\tukuy\t10\r\n工\ta\t20\r\n工\taaaa\t20\r\n人\tw\t10\r\n人\twwww\t10\r\n民\tnav\t10\r\n民\tnavn\t10\r\n人民\twwna\t20\r\n艹\taghh\t0\r\n艹\tghgh\t0\r\n日\tjjjj\t10\r\n";

    fn build_with(jidian: &str, wubi98: &str, fcitx: &str, base: &str) -> Supplement {
        build(&Inputs {
            jidian,
            wubi98,
            wubi98_fcitx: fcitx,
            base,
        })
        .unwrap()
    }

    fn rows(supplement: &Supplement) -> Vec<(&str, &str, i64)> {
        supplement
            .entries
            .iter()
            .map(|entry| (entry.word.as_str(), entry.code.as_str(), entry.weight))
            .collect()
    }

    #[test]
    fn a_98_word_takes_the_86_code_below_the_jidian_row() {
        let supplement = build_with(
            JIDIAN_FIXTURE,
            "冲凉\tukuy\r\n",
            "",
            "冲凉\tchong'liang\t9190\n",
        );
        assert_eq!(rows(&supplement), [("冲凉", "ukuy", 9)]);
        assert!(supplement.entries[0].from_wubi98 && supplement.entries[0].from_pinyin);
        assert_eq!(supplement.unique_codes_shared, 1);
    }

    #[test]
    fn three_and_four_character_words_follow_the_86_rules() {
        let supplement = build_with(JIDIAN_FIXTURE, "工人民\tx\r\n工人日民\tx\r\n", "", "");
        assert_eq!(
            rows(&supplement),
            [("工人日民", "awjn", 9), ("工人民", "awna", 9)]
        );
        assert_eq!(supplement.unique_codes_shared, 0);
    }

    #[test]
    fn pinyin_words_need_two_characters_and_the_threshold() {
        let base = "工人\tgong'ren\t5000\n日工\tri'gong\t4999\n工人民\tgong'ren'min\t90000\n";
        let supplement = build_with(JIDIAN_FIXTURE, "日日\tx\r\n", "", base);
        assert_eq!(
            rows(&supplement),
            [("工人", "aaww", 9), ("日日", "jjjj", 9)]
        );
        assert!(supplement.entries[0].from_pinyin && !supplement.entries[0].from_wubi98);
    }

    #[test]
    fn words_the_jidian_table_has_under_any_code_are_left_out() {
        let supplement = build_with(
            JIDIAN_FIXTURE,
            "人民\tx\r\n部门\tx\r\n",
            "",
            "人民\tren'min\t90000\n",
        );
        assert!(supplement.entries.is_empty());
    }

    #[test]
    fn words_of_one_code_step_down_by_rime_ice_weight() {
        let supplement = build_with(
            JIDIAN_FIXTURE,
            "冲凉\tx\r\n",
            "ukuy 冲冻\n",
            "冲冻\tchong'dong\t20\n冲凉\tchong'liang\t9190\n",
        );
        assert_eq!(
            rows(&supplement),
            [("冲凉", "ukuy", 9), ("冲冻", "ukuy", 8)]
        );
    }

    #[test]
    fn a_code_whose_lowest_jidian_row_weighs_zero_gives_zero() {
        let jidian = format!("{JIDIAN_FIXTURE}𠂇\taajj\t0\r\n");
        let supplement = build_with(&jidian, "工日\tx\r\n", "", "");
        assert_eq!(rows(&supplement), [("工日", "aajj", 0)]);
    }

    #[test]
    fn a_character_without_an_agreeing_full_code_leaves_its_words_out() {
        let supplement = build_with(JIDIAN_FIXTURE, "艹工\tx\r\n工鑫\tx\r\n", "", "");
        assert!(supplement.entries.is_empty());
        let mut underivable = supplement.underivable.clone();
        underivable.sort();
        assert_eq!(
            underivable,
            [("工鑫".to_owned(), '鑫'), ("艹工".to_owned(), '艹')]
        );
    }

    #[test]
    fn rendered_rows_parse_as_wubi86_rows() {
        let supplement = build_with(JIDIAN_FIXTURE, "冲凉\tukuy\r\n", "", "");
        let rendered = render(
            &supplement,
            &Provenance {
                inputs: &[(JIDIAN, "0")],
            },
        );
        let parsed: Vec<_> = text::universal_lines(&rendered)
            .into_iter()
            .filter_map(|line| msime::parse_code_line(line, false))
            .collect();
        assert_eq!(parsed, [("ukuy".to_owned(), "冲凉", 9)]);
    }
}
