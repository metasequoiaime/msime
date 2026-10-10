//! `wubi86-supplement`：生成 msime-dictionary 的 `sources/wubi/wubi86-supplement.txt`，即 86 五笔码表（`sources/wubi/wubi86-jidian.txt`）缺少的多字词，编码按 86 版词组规则由该表自己的单字编码推出。
//!
//! 一个词在极点码表的任何编码下都没有，并且满足以下其一时收录：98 五笔码表（`sources/wubi/wubi98.txt` 和 `sources/wubi/wubi98-fcitx.txt`）列有它；或 `sources/pinyin/rime-ice.txt` 里它是权重不低于 `MIN_PINYIN_WEIGHT` 的二字词。只算全部由汉字组成的词。98 码表是人工整理的词表，所以两字及以上的词都从中收录；rime-ice 的权重来自语料，语料也会产生词的碎片（被他、请把），所以从它只收超过阈值的二字词。9999 这类人工填写的权重不与词频区分。
//!
//! 编码按 86 版词组规则，取每个字的全码，即极点码表里该字单独出现时最长的编码：二字词各取前两码；三字词取前两字的首码和第三字的前两码；更长的词取第一、二、三字和末字的首码。码表没有单独编码的字，或几个全码在词所需码位上不一致的字（部首和几百个生僻字），含它的词不收。在极点码表自己的词上，这套规则还原了 63,127 个编码中的 63,095 个；其余是码表自己的简码和拆法（占比 hxx、蹂躏 kcka）。
//!
//! 补充表从不移动极点码表的行。补充词在其编码下的权重低于该编码在极点码表里的最低权重，并且不高于 `SUPPLEMENT_CEILING`，也就低于极点码表所有有排名的词（它们的最低权重是 10），所以在前缀列表里也排在它们之后。同一编码下的几个补充词按 rime-ice 权重从高到低、再按词排序，从这个上限起依次减 1，最低为 0；构建把本文件并在极点码表之后，所以同权重时极点码表的行在前，同一编码内保持本文件的顺序。原本四码只有一个极点词、输入法会自动上屏的编码，一旦有补充词共用就不再自动上屏；报告会统计这类编码。

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

/// 86 码表缺少的二字词，在 rime-ice 里达到这个权重才收录。
pub const MIN_PINYIN_WEIGHT: i64 = 5000;

/// 补充行的最高权重：比极点码表有排名的词的最低权重低 1。
pub(crate) const SUPPLEMENT_CEILING: i64 = 9;

/// 每行报告列出的示例个数。
pub(crate) const REPORT_EXAMPLES: usize = 20;

pub struct Inputs<'a> {
    pub jidian: &'a str,
    /// `sources/wubi/wubi98.txt`，已从 UTF-16LE 解码。
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
    /// 按编码排序，同一编码内最优的在前。
    pub entries: Vec<Entry>,
    /// 推不出编码的候选词，以及导致失败的那个字。
    pub underivable: Vec<(String, char)>,
    /// 原本恰好只有一行极点词、现在也有了补充行的四码编码数。
    pub unique_codes_shared: usize,
}

/// 极点码表的行：`(code, value, weight)`。
fn jidian_rows(source: &str) -> Vec<(String, &str, i64)> {
    text::universal_lines(text::without_bom(source))
        .into_iter()
        .filter_map(|line| msime::parse_code_line(line, false))
        .collect()
}

/// 每个字的全码：码表里该字单独出现时最长的编码。
pub(crate) fn full_codes(rows: &[(String, &str, i64)]) -> HashMap<char, Vec<String>> {
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

/// `c` 的全码的前 `letters` 码；所有全码都有这几码且一致时才返回。
pub(crate) fn prefix(codes: &HashMap<char, Vec<String>>, c: char, letters: usize) -> Option<&str> {
    let full = codes.get(&c)?;
    let first = full.first()?.get(..letters)?;
    full.iter()
        .all(|code| code.get(..letters) == Some(first))
        .then_some(first)
}

/// `word` 的 86 词组编码，或没有可用全码的那个字。
pub(crate) fn word_code(
    codes: &HashMap<char, Vec<String>>,
    word: &str,
) -> std::result::Result<String, char> {
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

pub(crate) fn is_word(value: &str) -> bool {
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
        // rime-ice 权重高的在前，rime-ice 没有的排最后，再按词排序，使顺序不依赖哈希顺序。
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

/// 表头记录的事实，让读者能复现本文件。
pub struct Provenance<'a> {
    /// 每个输入的 `(path, sha256)`。
    pub inputs: &'a [(&'a str, &'a str)],
    /// 运行生成器的 msime 提交；构建器有未提交改动时带 `-dirty` 后缀。
    pub generator_commit: &'a str,
}

pub fn render(supplement: &Supplement, provenance: &Provenance) -> String {
    let inputs = provenance
        .inputs
        .iter()
        .map(|(path, sha256)| format!("{path}（SHA-256 {sha256}）"))
        .collect::<Vec<_>>()
        .join("、");
    let mut out = String::new();
    let _ = writeln!(out, "# 86 五笔词组补充表，由 msime 仓库提交 {} 的 crates/dict-builder/src/wubi86_supplement.rs 以 `msime-dict-build wubi86-supplement --dictionary <msime-dictionary checkout> --cache <dir> --out sources/wubi/wubi86-supplement.txt` 生成；不要手工编辑。", provenance.generator_commit);
    let _ = writeln!(out, "# 输入：{inputs}。");
    let _ = writeln!(out, "# 收录：{JIDIAN} 在任何编码下都没有、且全部由汉字组成的词，满足其一即收：在 98 五笔的 {WUBI98} 或 {WUBI98_FCITX} 里是两字及以上的词；或在 {BASE} 里是权重不低于 {MIN_PINYIN_WEIGHT} 的二字词（9999 这类人工填写的权重不与词频区分）。");
    let _ = writeln!(out, "# 编码：按 86 版词组规则，取 {JIDIAN} 里单字的全码（该字单独出现时最长的编码）：二字词各取前两码；三字词取前两字的首码和第三字的前两码；四字及以上取第一、二、三字和末字的首码。单字表里没有、或几个全码在所需码位上不一致的字，含它的词不收。");
    let _ = writeln!(out, "# 去重与权重：对照集合是 {JIDIAN} 的全部两字及以上词条，已有的词不论编码一律不收。同一编码下补充词排在 {JIDIAN} 原有词条之后：最高权重取 {SUPPLEMENT_CEILING} 与该编码原有最低权重减 1 中较小者，几个补充词按 {BASE} 的权重从高到低（没有的排最后，再按词）依次减 1，最低为 0；构建的 wubi 阶段在 {JIDIAN} 之后并入本文件，同权重时原有词条在前。");
    for entry in &supplement.entries {
        let _ = writeln!(out, "{}\t{}\t{}", entry.word, entry.code, entry.weight);
    }
    out
}

/// 生成器对收录和舍弃内容的说明，每组一行，供评审查看。
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
                generator_commit: "0123456789abcdef0123456789abcdef01234567",
            },
        );
        let first = rendered.lines().next().unwrap();
        assert!(
            first.contains("msime 仓库提交 0123456789abcdef0123456789abcdef01234567"),
            "{first}"
        );
        assert!(!first.contains("dictionary-sources.lock.json"));
        let parsed: Vec<_> = text::universal_lines(&rendered)
            .into_iter()
            .filter_map(|line| msime::parse_code_line(line, false))
            .collect();
        assert_eq!(parsed, [("ukuy".to_owned(), "冲凉", 9)]);
    }
}
