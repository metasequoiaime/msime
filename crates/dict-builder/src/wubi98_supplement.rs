//! `wubi98-supplement`：生成 msime-dictionary 的 `sources/wubi/wubi98-supplement.txt`，即 98 五笔码表（`sources/wubi/wubi98.txt` 与 `sources/wubi/wubi98-fcitx.txt`）缺少的多字词，编码按词组规则由这两张表自己的单字编码推出。
//!
//! 两张 98 表在任何编码下都没有的基本区汉字词，来自 86 极点码表、权重不低于 `wubi86_supplement::MIN_PINYIN_WEIGHT` 的 rime-ice 主表和补充表，或人工维护的 custom/words.txt、区划补充表。门槛与 86 版共用，拼音来源不限字数，人工词条与区划名称不设权重门槛。
//!
//! 98 版的词组规则与 86 版相同：取每个字的全码（两张表里该字单独出现时最长的编码），二字词各取前两码；三字词取前两字的首码和第三字的前两码；更长的词取第一、二、三字和末字的首码。表里没有单独编码的字，或几个全码在所需码位上不一致的字，含它的词不收。在两张 98 表自己的 76,150 个词上，这套规则还原了全部编码。
//!
//! 补充表从不移动 98 表的行。表里各行的权重按构建的规则算出（`msime::wubi98_rows`：主表按编码内先后给 10 的倍数，Fcitx 补充表的行给 1）。补充词在其编码下的权重低于该编码原有的最低权重，并且不高于 `wubi86_supplement::SUPPLEMENT_CEILING`，也就低于主表所有的行（最低是 10），所以在前缀列表里也排在它们之后。同一编码下的几个补充词按 rime-ice 权重从高到低、再按词排序，从这个上限起依次减 1，最低为 0；构建把本文件插在两张 98 表之后，所以同权重时原有行在前，同一编码内保持本文件的顺序。原本四码只有一个词、输入法会自动上屏的编码，一旦有补充词共用就不再自动上屏；报告会统计这类编码。

use std::collections::{BTreeSet, HashSet};
use std::fmt::Write as _;

use anyhow::{bail, Result};

use crate::curation::PinyinCuration;
use crate::msime;
use crate::wubi86_supplement;
use crate::wubi86_supplement::{MIN_PINYIN_WEIGHT, REPORT_EXAMPLES, SUPPLEMENT_CEILING};
use crate::wubi_supplement::{frequencies, is_word, pinyin_words, rank};

pub const OUTPUT: &str = "sources/wubi/wubi98-supplement.txt";
pub const WUBI98: &str = wubi86_supplement::WUBI98;
pub const WUBI98_FCITX: &str = wubi86_supplement::WUBI98_FCITX;
pub const JIDIAN: &str = wubi86_supplement::JIDIAN;
pub const BASE: &str = wubi86_supplement::BASE;
pub const PINYIN_SUPPLEMENT: &str = wubi86_supplement::PINYIN_SUPPLEMENT;
pub const PLACES: &str = wubi86_supplement::PLACES;
pub const CUSTOM: &str = wubi86_supplement::CUSTOM;

pub struct Inputs<'a> {
    /// `sources/wubi/wubi98.txt`，已从 UTF-16LE 解码。
    pub wubi98: &'a str,
    pub wubi98_fcitx: &'a str,
    pub jidian: &'a str,
    /// 新布局下 rime-wubi 补进来的 86 行（[`crate::wubi86_table::RIME_WUBI`]），旧布局为 `None`。
    pub rime_wubi: Option<&'a str>,
    /// 读 86 原表时比的词频（[`crate::wubi86_table::Frequencies`]）；这里只取 86 原表的词，不看权重，传进来是为了和构建读到同一组行。
    pub frequencies: &'a crate::wubi86_table::Frequencies,
    pub base: &'a str,
    pub pinyin_supplement: &'a str,
    pub places: &'a str,
    pub custom: &'a str,
    /// msime-dictionary `curation/` 的拼音否决表和权重覆盖表：整词否决的词不收，权重覆盖只取第四列写了 `wubi` 的频率修正，见 [`PinyinCuration::wubi_rows`]。
    pub curation: &'a PinyinCuration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub word: String,
    pub code: String,
    pub weight: i64,
    pub from_jidian: bool,
    pub from_pinyin: bool,
}

pub struct Supplement {
    /// 按编码排序，同一编码内最优的在前。
    pub entries: Vec<Entry>,
    /// 推不出编码的候选词，以及导致失败的那个字。
    pub underivable: Vec<(String, char)>,
    /// 含构建不写入的字（扩展区、私用区、兼容汉字，见 `msime::unsupported_wubi_value`）而不收的候选词个数。
    pub outside_basic_cjk: usize,
    /// 原本恰好只有一行、现在也有了补充行的四码编码数。
    pub unique_codes_shared: usize,
}

pub fn build(inputs: &Inputs) -> Result<Supplement> {
    let (weighted, _) = msime::wubi98_rows(inputs.wubi98, &[inputs.wubi98_fcitx]);
    if weighted.is_empty() {
        bail!("{WUBI98} and {WUBI98_FCITX} have no rows");
    }
    // 构建不把整词否决的词写进 wubi98，同码最低权重和全码也就不算它们。
    let rows: Vec<(String, &str, i64)> = weighted
        .iter()
        .filter(|(_, value, _)| !inputs.curation.vetoes_word(value))
        .map(|(code, value, weight)| (code.clone(), value.as_str(), *weight))
        .collect();
    let wubi98_words: HashSet<&str> = rows
        .iter()
        .filter(|(_, value, _)| value.chars().count() >= 2)
        .map(|(_, value, _)| *value)
        .collect();
    let frequency = frequencies(
        [
            inputs.base,
            inputs.pinyin_supplement,
            inputs.places,
            inputs.custom,
        ],
        inputs.curation,
    );

    let jidian: BTreeSet<&str> =
        crate::wubi86_table::rows(inputs.jidian, inputs.rime_wubi, inputs.frequencies)
            .flatten()
            .map(|(_, value, _)| value)
            .filter(|value| value.chars().count() >= 2)
            .collect();
    if jidian.is_empty() {
        bail!("{JIDIAN} has no words");
    }
    let pinyin = pinyin_words(
        &frequency.threshold,
        MIN_PINYIN_WEIGHT,
        &[inputs.places, inputs.custom],
        inputs.curation,
    );

    let mut outside_basic_cjk = 0;
    let candidates: BTreeSet<&str> = jidian
        .iter()
        .chain(pinyin.iter())
        .copied()
        .filter(|word| {
            is_word(word) && !wubi98_words.contains(word) && !inputs.curation.vetoes_word(word)
        })
        .filter(|word| {
            let outside = msime::unsupported_wubi_value(word);
            outside_basic_cjk += usize::from(outside);
            !outside
        })
        .collect();

    let ranked = rank(&rows, candidates, &frequency.order, SUPPLEMENT_CEILING);
    let entries = ranked
        .entries
        .into_iter()
        .map(|row| Entry {
            word: row.word.to_owned(),
            code: row.code,
            weight: row.weight,
            from_jidian: jidian.contains(row.word),
            from_pinyin: pinyin.contains(row.word),
        })
        .collect();
    Ok(Supplement {
        entries,
        underivable: ranked.underivable,
        outside_basic_cjk,
        unique_codes_shared: ranked.unique_codes_shared,
    })
}

/// 表头记录的事实，让读者能复现本文件。
pub struct Provenance<'a> {
    /// 每个输入的 `(path, sha256)`。
    pub inputs: &'a [(&'a str, &'a str)],
    /// 运行生成器的 msime 提交；构建器有未提交改动时带 `-dirty` 后缀。
    pub generator_commit: &'a str,
    /// 已校验的上游仓库与提交。
    pub upstreams: &'a str,
}

pub fn render(supplement: &Supplement, provenance: &Provenance) -> String {
    let inputs = provenance
        .inputs
        .iter()
        .map(|(path, sha256)| format!("{path}（SHA-256 {sha256}）"))
        .collect::<Vec<_>>()
        .join("、");
    let table = crate::wubi86_table::described(provenance.inputs);
    let mut out = String::new();
    let _ = writeln!(out, "# 98 五笔词组补充表，由 msime 仓库提交 {} 的 crates/dict-builder/src/wubi98_supplement.rs 以 `msime-dict-build wubi98-supplement --dictionary <msime-dictionary checkout> --cache <dir> --out sources/wubi/wubi98-supplement.txt` 生成；不要手工编辑。", provenance.generator_commit);
    let _ = writeln!(out, "# 输入：{inputs}。");
    let _ = writeln!(
        out,
        "# 上游：{}；jidian 与区划表的来源、提交和许可另见 NOTICE.md 及输入文件头。",
        provenance.upstreams
    );
    let _ = writeln!(out, "# 收录：{WUBI98} 和 {WUBI98_FCITX} 在任何编码下都没有、且全部由基本区汉字组成（不含 CJK 扩展 A 和 BMP 以外的字）的词，满足其一即收：在 86 五笔的 {table} 里是两字及以上的词；或在 {BASE} 里是权重不低于 {MIN_PINYIN_WEIGHT} 的两字及以上词，或 {PINYIN_SUPPLEMENT} 里达到同一门槛的词，或 {PLACES}、{CUSTOM} 的人工维护词（后两者不设权重门槛；9999 这类人工填写的权重不与词频区分）。");
    let _ = writeln!(out, "# 编码：按 98 版词组规则，取 {WUBI98} 和 {WUBI98_FCITX} 里单字的全码（该字单独出现时最长的编码）：二字词各取前两码；三字词取前两字的首码和第三字的前两码；四字及以上取第一、二、三字和末字的首码。单字表里没有、或几个全码在所需码位上不一致的字，含它的词不收。");
    let _ = writeln!(out, "# 去重与权重：对照集合是 {WUBI98} 和 {WUBI98_FCITX} 的全部两字及以上词条，已有的词不论编码一律不收。原有行的权重按构建的规则算出（主表按编码内先后给 10 的倍数，{WUBI98_FCITX} 的行给 1）。同一编码下补充词排在原有行之后：最高权重取 {SUPPLEMENT_CEILING} 与该编码原有最低权重减 1 中较小者，几个补充词按 {BASE}、{PINYIN_SUPPLEMENT}、{PLACES}、{CUSTOM} 中同词最大权重从高到低（没有的排最后，再按词）依次减 1，最低为 0；构建的 wubi98 阶段在两张 98 表之后并入本文件，同权重时原有行在前。");
    for entry in &supplement.entries {
        let _ = writeln!(out, "{}\t{}\t{}", entry.word, entry.code, entry.weight);
    }
    out
}

/// 生成器对收录和舍弃内容的说明，每组一行，供评审查看。
pub fn report(supplement: &Supplement) -> Vec<String> {
    let count = |predicate: fn(&Entry) -> bool| {
        supplement
            .entries
            .iter()
            .filter(|entry| predicate(entry))
            .count()
    };
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
            "{} words under {} codes: {} from {JIDIAN}, {} from pinyin inputs (rime-ice weight >= {MIN_PINYIN_WEIGHT}, curated words at any weight), {} from both",
            supplement.entries.len(),
            codes.len(),
            count(|entry| entry.from_jidian),
            count(|entry| entry.from_pinyin),
            count(|entry| entry.from_jidian && entry.from_pinyin)
        ),
        format!(
            "{} four-letter codes that one 98 row answered alone now have supplement rows too, so they no longer commit by themselves",
            supplement.unique_codes_shared
        ),
        format!(
            "{} candidates left out for a character the build does not write (outside the basic CJK block, private use or a compatibility ideograph)",
            supplement.outside_basic_cjk
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

    // 98 主表：value<TAB>code，同一编码内越靠前权重越高。部 门 冲 凉 冻 各有全码。
    const WUBI98_FIXTURE: &str = "\u{feff}部\tukbh\r\n门\tuyhn\r\n冲\tukh\r\n冲\tukhh\r\n凉\tuyiy\r\n冻\tuyaa\r\n部门\tukuy\r\n工\taaaa\r\n人\twwww\r\n民\tnavn\r\n人民\twwna\r\n日\tjjjj\r\n";

    fn build_with(wubi98: &str, fcitx: &str, jidian: &str, base: &str) -> Supplement {
        build(&Inputs {
            wubi98,
            wubi98_fcitx: fcitx,
            jidian,
            rime_wubi: None,
            frequencies: &crate::wubi86_table::Frequencies::default(),
            base,
            pinyin_supplement: "",
            places: "",
            custom: "",
            curation: &PinyinCuration::default(),
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
    fn a_jidian_word_takes_the_98_code_below_the_98_row() {
        let supplement = build_with(
            WUBI98_FIXTURE,
            "",
            "冲凉\tukuy\t10\r\n",
            "冲凉\tchong'liang\t9190\n",
        );
        assert_eq!(rows(&supplement), [("冲凉", "ukuy", 9)]);
        assert!(supplement.entries[0].from_jidian && supplement.entries[0].from_pinyin);
        assert_eq!(supplement.unique_codes_shared, 1);
    }

    #[test]
    fn words_either_98_table_has_are_left_out() {
        let supplement = build_with(
            WUBI98_FIXTURE,
            "ukuy 冲凉\n",
            "人民\twwna\t20\r\n冲凉\tukuy\t10\r\n",
            "人民\tren'min\t90000\n",
        );
        assert!(supplement.entries.is_empty());
    }

    #[test]
    fn a_code_with_an_fcitx_row_gives_zero() {
        // Fcitx 行的权重是 1，补充词只能是 0，并靠 rowid 排在它之后。
        let supplement = build_with(WUBI98_FIXTURE, "ukuy 冲冻\n", "冲凉\tukuy\t10\r\n", "");
        assert_eq!(rows(&supplement), [("冲凉", "ukuy", 0)]);
    }

    #[test]
    fn pinyin_words_include_longer_words_at_the_threshold() {
        let base = "工人\tgong'ren\t100\n日工\tri'gong\t99\n工人民\tgong'ren'min\t100\n";
        let supplement = build_with(WUBI98_FIXTURE, "", "日日\tjjjj\t10\r\n", base);
        assert_eq!(
            rows(&supplement),
            [
                ("工人", "aaww", 9),
                ("工人民", "awna", 9),
                ("日日", "jjjj", 9)
            ]
        );
        assert!(supplement.entries[0].from_pinyin && !supplement.entries[0].from_jidian);
    }

    #[test]
    fn extension_characters_are_left_out() {
        // 𠂇 是 BMP 以外的字，即使 98 表给了它编码也不收含它的词。
        let wubi98 = format!("{WUBI98_FIXTURE}𠂇\tjjja\r\n");
        let supplement = build_with(&wubi98, "", "日𠂇\tjjjj\t10\r\n", "");
        assert!(supplement.entries.is_empty());
        assert_eq!(supplement.outside_basic_cjk, 1);
    }

    #[test]
    fn supplementary_and_manual_inputs_reach_the_version_specific_generator() {
        let supplement = build(&Inputs {
            wubi98: WUBI98_FIXTURE,
            wubi98_fcitx: "",
            jidian: "日日\tjjjj\t10\n",
            rime_wubi: None,
            frequencies: &crate::wubi86_table::Frequencies::default(),
            base: "",
            pinyin_supplement: "工人\tgong'ren\t100\n",
            places: "工人日\tgong'ren'ri\t1\n",
            custom: "工人民\tgong'ren'min\t1\n工人\tgong'ren\t100\n",
            curation: &PinyinCuration::default(),
        })
        .unwrap();
        assert_eq!(
            rows(&supplement),
            [
                ("工人", "aaww", 9),
                ("工人日", "awjj", 9),
                ("工人民", "awna", 9),
                ("日日", "jjjj", 9)
            ]
        );
    }

    /// 整词否决的词不进补充表（极点表和区划表的词也一样），读音否决改的是拼音那边的 100 门槛，写了 `wubi` 的权重覆盖（日工 升到 100）也算进门槛；98 主表里被整词否决的词不再算 98 表已有。
    #[test]
    fn curation_tables_reach_the_generator() {
        let curation = PinyinCuration::parse(
            "日日\n工人日\n人民\n工人\tgong'ren\n",
            "日工\tri'gong\t100\twubi\n",
        )
        .unwrap();
        let supplement = build(&Inputs {
            wubi98: WUBI98_FIXTURE,
            wubi98_fcitx: "",
            jidian: "日日\tjjjj\t10\n人民\twwna\t20\n",
            rime_wubi: None,
            frequencies: &crate::wubi86_table::Frequencies::default(),
            base: "日工\tri'gong\t99\n",
            pinyin_supplement: "工人\tgong'ren\t100\n",
            places: "工人日\tgong'ren'ri\t1\n",
            custom: "工人民\tgong'ren'min\t1\n工人\tgong'ren\t100\n",
            curation: &curation,
        })
        .unwrap();
        assert_eq!(
            rows(&supplement),
            [("工人民", "awna", 9), ("日工", "jjaa", 9)]
        );
    }

    #[test]
    fn words_of_one_code_step_down_by_rime_ice_weight() {
        let supplement = build_with(
            WUBI98_FIXTURE,
            "",
            "冲凉\tukuy\t10\r\n冲冻\tukuy\t10\r\n",
            "冲冻\tchong'dong\t20\n冲凉\tchong'liang\t9190\n",
        );
        assert_eq!(
            rows(&supplement),
            [("冲凉", "ukuy", 9), ("冲冻", "ukuy", 8)]
        );
    }

    #[test]
    fn a_character_without_a_full_code_leaves_its_words_out() {
        let supplement = build_with(WUBI98_FIXTURE, "", "工鑫\taaqq\t10\r\n", "");
        assert!(supplement.entries.is_empty());
        assert_eq!(supplement.underivable, [("工鑫".to_owned(), '鑫')]);
    }

    #[test]
    fn rendered_rows_parse_and_merge_after_the_98_rows() {
        let supplement = build_with(WUBI98_FIXTURE, "", "冲凉\tukuy\t10\r\n", "");
        let rendered = render(
            &supplement,
            &Provenance {
                inputs: &[(WUBI98, "0")],
                generator_commit: "0123456789abcdef0123456789abcdef01234567",
                upstreams: "fixture",
            },
        );
        assert!(rendered
            .lines()
            .next()
            .unwrap()
            .contains("msime 仓库提交 0123456789abcdef0123456789abcdef01234567"));
        let dir = tempfile::tempdir().unwrap();
        let mut primary = vec![0xff, 0xfe];
        for unit in WUBI98_FIXTURE.trim_start_matches('\u{feff}').encode_utf16() {
            primary.extend_from_slice(&unit.to_le_bytes());
        }
        let primary_path = dir.path().join("wubi98.txt");
        std::fs::write(&primary_path, primary).unwrap();
        let generated = dir.path().join("wubi98-supplement.txt");
        std::fs::write(&generated, rendered).unwrap();
        let mut connection = rusqlite::Connection::open_in_memory().unwrap();
        msime::build_wubi98_sources(
            &mut connection,
            &primary_path,
            &[],
            &[&generated],
            &crate::curation::PinyinVetoes::default(),
        )
        .unwrap();
        let ukuy: Vec<(String, i64)> = connection
            .prepare(
                "SELECT value, weight FROM wubi98 WHERE key = 'ukuy' ORDER BY weight DESC, rowid",
            )
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(ukuy, [("部门".to_owned(), 10), ("冲凉".to_owned(), 9)]);
    }
}
