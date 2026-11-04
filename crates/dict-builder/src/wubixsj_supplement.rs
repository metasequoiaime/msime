//! `wubixsj-supplement`：生成 msime-dictionary 的 `sources/wubi/wubixsj-supplement.txt`，即新世纪五笔表的全部词组。上游码表 `sources/wubi/wubixsj-byujiang.txt` 只取单字（来源存疑，见 `msime-engine-dictionary-NOTICE.md`）：从这张表拿的是单字全码，以及由上游权重推出的同码单字先后（见 `msime::wubixsj_rows`）；上游的词组不用，词组全部由本文件按新世纪单字全码重新编码。
//!
//! 候选词来自 86 极点码表与两张 98 表里的多字词、权重不低于 `wubi86_supplement::MIN_PINYIN_WEIGHT` 的 rime-ice 主表和补充表词，以及人工维护的 custom/words.txt 和区划补充表，限两个及以上基本区汉字。编码按与 86、98 相同的词组规则，取每个字在新世纪单字表里单独出现时最长的编码：二字词各取前两码；三字词取前两字的首码和第三字的前两码；更长的词取第一、二、三字和末字的首码。表里没有单独编码的字，或几个全码在所需码位上不一致的字，含它的词不收。
//!
//! 权重分两层，因为新世纪表本身没有词组：如果所有词组都压在 `SUPPLEMENT_CEILING` 以下，常用词会全部排在同码的生僻单字之后。
//!
//! - 第一层是 86 极点码表里有的词：按新世纪码重新编码，权重沿用它在极点表里的权重（同一个词在极点表里有几行时取最大的），所以和 86 一样，常用词组可以排在同码的生僻单字之前。单字的权重按编码内名次给 10 的倍数（`msime::wubixsj_rows`），与极点表单字和词组的 10、20、30 这一档相当。
//! - 第二层是其余候选词：与 86、98 补充表的规则相同，上限取 `SUPPLEMENT_CEILING` 与该编码下单字和第一层最低权重减 1 中较小者，同一编码下按 rime-ice 权重从高到低、再按词排序，从上限起依次减 1，最低为 0。所以第二层低于所有单字（最低是 10）；对第一层，只要该编码第一层的最低权重大于 0，第二层就严格更低，等于 0 时（极点表里权重为 0 的词）第二层与它同为 0，靠下面的行序排在它后面。
//!
//! 构建把本文件并在单字之后，同一编码内保持本文件的顺序（第一层在前）；引擎在同一编码下按权重从高到低、同权重按 rowid 排，所以同权重时单字在前、第一层在第二层前。第二层在候选里永远不会排到同码任何单字或第一层词之前，但「权重严格更低」只对单字成立。原本四码只有一个单字、输入法会自动上屏的编码，一旦有词组共用就不再自动上屏；报告会统计这类编码。

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

use anyhow::{bail, Result};

use crate::curation::PinyinCuration;
use crate::msime;
use crate::text;
use crate::wubi86_supplement::{MIN_PINYIN_WEIGHT, REPORT_EXAMPLES, SUPPLEMENT_CEILING};
use crate::wubi_supplement::{frequencies, full_codes, is_word, pinyin_words, rank, word_code};

pub const OUTPUT: &str = "sources/wubi/wubixsj-supplement.txt";
pub const WUBIXSJ: &str = "sources/wubi/wubixsj-byujiang.txt";
pub const JIDIAN: &str = crate::wubi86_supplement::JIDIAN;
pub const WUBI98: &str = crate::wubi86_supplement::WUBI98;
pub const WUBI98_FCITX: &str = crate::wubi86_supplement::WUBI98_FCITX;
pub const BASE: &str = crate::wubi86_supplement::BASE;
pub const PINYIN_SUPPLEMENT: &str = crate::wubi86_supplement::PINYIN_SUPPLEMENT;
pub const PLACES: &str = crate::wubi86_supplement::PLACES;
pub const CUSTOM: &str = crate::wubi86_supplement::CUSTOM;

/// 表头「上游」一行列出的 reference：词条与编码依据的全部上游。
pub(crate) const UPSTREAMS: [&str; 5] = [
    "wubi06-rime",
    "rime-ice",
    "rime-ice-supplement",
    "98wubi-tables",
    "fcitx5-table-extra",
];

pub struct Inputs<'a> {
    pub wubixsj: &'a str,
    pub jidian: &'a str,
    /// `sources/wubi/wubi98.txt`，已从 UTF-16LE 解码。
    pub wubi98: &'a str,
    pub wubi98_fcitx: &'a str,
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
    /// 第一层：极点表里有的词，权重沿用极点表。
    pub from_jidian: bool,
    pub from_wubi98: bool,
    pub from_pinyin: bool,
}

pub struct Supplement {
    /// 按编码排序，同一编码内第一层在前、各层内最优的在前。
    pub entries: Vec<Entry>,
    /// 推不出编码的候选词，以及导致失败的那个字。
    pub underivable: Vec<(String, char)>,
    /// 含扩展区字而不收的候选词个数。
    pub outside_basic_cjk: usize,
    /// 原本恰好只有一个单字、现在也有了词组的四码编码数。
    pub unique_codes_shared: usize,
    /// 第一层词组排到同码全部单字之前的编码数。
    pub codes_led_by_words: usize,
}

pub fn build(inputs: &Inputs) -> Result<Supplement> {
    let singles = msime::wubixsj_rows(inputs.wubixsj)?.rows;
    if singles.is_empty() {
        bail!("{WUBIXSJ} has no single-character rows");
    }
    let codes = full_codes(
        &singles
            .iter()
            .map(|(code, value, weight)| (code.clone(), value.as_str(), *weight))
            .collect::<Vec<_>>(),
    );
    let frequency = frequencies(
        [
            inputs.base,
            inputs.pinyin_supplement,
            inputs.places,
            inputs.custom,
        ],
        inputs.curation,
    );

    let mut jidian: BTreeMap<&str, i64> = BTreeMap::new();
    for (_, value, weight) in text::universal_lines(text::without_bom(inputs.jidian))
        .into_iter()
        .filter_map(|line| msime::parse_code_line(line, false))
    {
        if value.chars().count() >= 2 {
            let slot = jidian.entry(value).or_insert(weight);
            *slot = (*slot).max(weight);
        }
    }
    if jidian.is_empty() {
        bail!("{JIDIAN} has no words");
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
    let pinyin = pinyin_words(
        &frequency.threshold,
        MIN_PINYIN_WEIGHT,
        &[inputs.places, inputs.custom],
        inputs.curation,
    );

    let mut outside_basic_cjk = 0;
    let candidates: BTreeSet<&str> = jidian
        .keys()
        .copied()
        .chain(wubi98.iter().map(String::as_str))
        .chain(pinyin.iter().copied())
        .filter(|word| is_word(word) && !inputs.curation.vetoes_word(word))
        .filter(|word| {
            let outside = msime::outside_basic_cjk(word);
            outside_basic_cjk += usize::from(outside);
            !outside
        })
        .collect();

    // 第一层：极点表的词，权重沿用极点表；同码几个词按极点权重、rime-ice 权重、词排序。
    let mut first: BTreeMap<String, Vec<(&str, i64)>> = BTreeMap::new();
    let mut underivable = Vec::new();
    for word in candidates
        .iter()
        .copied()
        .filter(|word| jidian.contains_key(word))
    {
        match word_code(&codes, word) {
            Ok(code) => first.entry(code).or_default().push((word, jidian[word])),
            Err(c) => underivable.push((word.to_owned(), c)),
        }
    }
    for words in first.values_mut() {
        words.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then(frequency.order.get(b.0).cmp(&frequency.order.get(a.0)))
                .then(a.0.cmp(b.0))
        });
    }

    // 第二层排在单字和第一层之后：把它们一起当作「原有行」交给共用的排名规则。
    let mut existing: Vec<(String, &str, i64)> = singles
        .iter()
        .map(|(code, value, weight)| (code.clone(), value.as_str(), *weight))
        .collect();
    for (code, words) in &first {
        existing.extend(
            words
                .iter()
                .map(|(word, weight)| (code.clone(), *word, *weight)),
        );
    }
    let second = rank(
        &existing,
        candidates
            .iter()
            .copied()
            .filter(|word| !jidian.contains_key(word)),
        &frequency.order,
        SUPPLEMENT_CEILING,
    );
    underivable.extend(second.underivable);
    underivable.sort();

    let entry = |word: &str, code: &str, weight: i64| Entry {
        word: word.to_owned(),
        code: code.to_owned(),
        weight,
        from_jidian: jidian.contains_key(word),
        from_wubi98: wubi98.contains(word),
        from_pinyin: pinyin.contains(word),
    };
    let mut by_code: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
    for (code, words) in &first {
        by_code.entry(code.clone()).or_default().extend(
            words
                .iter()
                .map(|(word, weight)| entry(word, code, *weight)),
        );
    }
    for row in second.entries {
        by_code
            .entry(row.code.clone())
            .or_default()
            .push(entry(row.word, &row.code, row.weight));
    }

    let mut singles_per_code: HashMap<&str, (usize, i64)> = HashMap::new();
    for (code, _, weight) in &singles {
        let slot = singles_per_code
            .entry(code.as_str())
            .or_insert((0, *weight));
        slot.0 += 1;
        slot.1 = slot.1.max(*weight);
    }
    let unique_codes_shared = by_code
        .keys()
        .filter(|code| {
            code.len() == 4 && singles_per_code.get(code.as_str()).map(|s| s.0) == Some(1)
        })
        .count();
    let codes_led_by_words = first
        .iter()
        .filter(|(code, words)| {
            singles_per_code
                .get(code.as_str())
                .is_some_and(|(_, top)| words.first().is_some_and(|(_, weight)| weight > top))
        })
        .count();

    Ok(Supplement {
        entries: by_code.into_values().flatten().collect(),
        underivable,
        outside_basic_cjk,
        unique_codes_shared,
        codes_led_by_words,
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
    let mut out = String::new();
    let _ = writeln!(out, "# 新世纪五笔词组表，由 msime 仓库提交 {} 的 crates/dict-builder/src/wubixsj_supplement.rs 以 `msime-dict-build wubixsj-supplement --dictionary <msime-dictionary checkout> --cache <dir> --out sources/wubi/wubixsj-supplement.txt` 生成；不要手工编辑。", provenance.generator_commit);
    let _ = writeln!(out, "# 输入：{inputs}。");
    let _ = writeln!(
        out,
        "# 上游：{}；jidian 与区划表的来源、提交和许可另见 NOTICE.md 及输入文件头。",
        provenance.upstreams
    );
    let _ = writeln!(out, "# 收录：全部由基本区汉字组成（不含 CJK 扩展 A 和 BMP 以外的字）的两字及以上词，满足其一即收：在 {JIDIAN}、{WUBI98} 或 {WUBI98_FCITX} 里；或在 {BASE} 里权重不低于 {MIN_PINYIN_WEIGHT}，或 {PINYIN_SUPPLEMENT} 里达到同一门槛；或在 {PLACES}、{CUSTOM} 里（不设权重门槛；9999 这类人工填写的权重不与词频区分）。{WUBIXSJ} 自己的词组和权重一律不用。");
    let _ = writeln!(out, "# 编码：按与 86、98 相同的词组规则，取 {WUBIXSJ} 里基本区单字的全码（该字单独出现时最长的编码）：二字词各取前两码；三字词取前两字的首码和第三字的前两码；四字及以上取第一、二、三字和末字的首码。单字表里没有、或几个全码在所需码位上不一致的字，含它的词不收。");
    let _ = writeln!(out, "# 权重：单字按 {WUBIXSJ} 里同一编码内上游权重的名次给 10 的倍数（上游权重只用来定先后、数值不进表；构建时算出，不在本文件里）。{JIDIAN} 里有的词沿用它在该表里的最大权重；其余的词排在同一编码下单字和这些词之后：最高权重取 {SUPPLEMENT_CEILING} 与该编码已有最低权重减 1 中较小者，几个词按 {BASE}、{PINYIN_SUPPLEMENT}、{PLACES}、{CUSTOM} 中同词最大权重从高到低（没有的排最后，再按词）依次减 1，最低为 0。构建的 wubixsj 阶段在单字之后并入本文件，同权重时单字在前。");
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
            "{} words under {} codes: {} from {JIDIAN} at their jidian weight, {} below every single character ({} also in the 98 tables, {} from pinyin inputs: rime-ice weight >= {MIN_PINYIN_WEIGHT}, curated words at any weight)",
            supplement.entries.len(),
            codes.len(),
            count(|entry| entry.from_jidian),
            count(|entry| !entry.from_jidian),
            count(|entry| !entry.from_jidian && entry.from_wubi98),
            count(|entry| !entry.from_jidian && entry.from_pinyin),
        ),
        format!(
            "{} codes where a jidian word now comes before every single character of the code",
            supplement.codes_led_by_words
        ),
        format!(
            "{} four-letter codes that one single character answered alone now have words too, so they no longer commit by themselves",
            supplement.unique_codes_shared
        ),
        format!(
            "{} candidates left out for a character outside the basic CJK block",
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

    // 新世纪单字表：部 门 冲 凉 冻 工 人 民 日 各有全码，冲 另有三码简码；上游的词组行「部门」不用。
    const WUBIXSJ_FIXTURE: &str = "# Rime dictionary: wubi06\n---\nname: wubi06\n...\n部\tukbh\t50\n门\tuyhn\t50\n冲\tukh\t9000\n冲\tukhh\t50\n凉\tuyiy\t50\n冻\tuyaa\t50\n部门\tukuy\t500\n工\taaaa\t50\n人\twwww\t50\n民\tnavn\t50\n日\tjjjj\t50\n冰\tukuy\t20\n";
    const WUBI98_FIXTURE: &str = "\u{feff}人民\twwna\r\n";

    fn build_with(jidian: &str, base: &str, custom: &str) -> Supplement {
        build(&Inputs {
            wubixsj: WUBIXSJ_FIXTURE,
            jidian,
            wubi98: WUBI98_FIXTURE,
            wubi98_fcitx: "",
            base,
            pinyin_supplement: "",
            places: "",
            custom,
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
    fn jidian_words_keep_their_weight_and_others_go_below_every_single_character() {
        let supplement = build_with(
            "部门\tukuy\t30\r\n冲凉\tukuy\t10\r\n冲凉\tuk\t20\r\n工\taaaa\t20\r\n",
            "冻冲\tdong'chong\t500\n冲冻\tchong'dong\t100\n",
            "",
        );
        assert_eq!(
            rows(&supplement),
            [
                ("部门", "ukuy", 30),
                ("冲凉", "ukuy", 20),
                // 第二层在 ukuy 下的上限是单字「冰」（10）减 1。
                ("冲冻", "ukuy", 9),
                ("冻冲", "uyuk", 9),
                ("人民", "wwna", 9),
            ]
        );
        let first = &supplement.entries[0];
        assert!(first.from_jidian && !first.from_pinyin);
        assert_eq!(supplement.codes_led_by_words, 1);
        assert_eq!(supplement.unique_codes_shared, 1);
    }

    /// 渲染本表、与单字表一起建出 `wubixsj`，再按引擎的排序（权重从高到低，同权重按 rowid）列出一个编码的候选。
    fn merged(supplement: &Supplement, code: &str) -> Vec<(String, i64)> {
        merged_with(supplement, code, &crate::curation::PinyinVetoes::default())
    }

    /// 同 [`merged`]，建表时施加 `vetoes` 的整词否决。
    fn merged_with(
        supplement: &Supplement,
        code: &str,
        vetoes: &crate::curation::PinyinVetoes,
    ) -> Vec<(String, i64)> {
        let rendered = render(
            supplement,
            &Provenance {
                inputs: &[(WUBIXSJ, "0")],
                generator_commit: "0123456789abcdef0123456789abcdef01234567",
                upstreams: "fixture",
            },
        );
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("wubixsj-byujiang.txt");
        std::fs::write(&source, WUBIXSJ_FIXTURE).unwrap();
        let generated = dir.path().join("wubixsj-supplement.txt");
        std::fs::write(&generated, rendered).unwrap();
        let mut connection = rusqlite::Connection::open_in_memory().unwrap();
        msime::build_wubixsj(&mut connection, &source, &[&generated], vetoes).unwrap();
        let rows = connection
            .prepare("SELECT value, weight FROM wubixsj WHERE key = ?1 ORDER BY weight DESC, rowid")
            .unwrap()
            .query_map([code], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        rows
    }

    /// 整词否决的词不进补充表（极点表的词也一样），读音否决改的是拼音那边的 100 门槛，没写 `wubi` 的权重覆盖（冲冻 升到 120）只改拼音、五笔不理；建表时整词否决的词也不写进 `wubixsj`。
    #[test]
    fn curation_tables_reach_the_generator_and_the_table() {
        let curation =
            PinyinCuration::parse("部门\n冻冲\tdong'chong\n", "冲冻\tchong'dong\t120\n").unwrap();
        let supplement = build(&Inputs {
            wubixsj: WUBIXSJ_FIXTURE,
            jidian: "部门\tukuy\t30\r\n冲凉\tukuy\t10\r\n冲凉\tuk\t20\r\n工\taaaa\t20\r\n",
            wubi98: WUBI98_FIXTURE,
            wubi98_fcitx: "",
            base: "冻冲\tdong'chong\t500\n冲冻\tchong'dong\t99\n",
            pinyin_supplement: "",
            places: "",
            custom: "",
            curation: &curation,
        })
        .unwrap();
        assert_eq!(
            rows(&supplement),
            [("冲凉", "ukuy", 20), ("人民", "wwna", 9)]
        );
        assert_eq!(
            merged(&supplement, "ukuy"),
            [("冲凉".to_owned(), 20), ("冰".to_owned(), 10)]
        );
        assert_eq!(
            merged_with(
                &supplement,
                "ukuy",
                &crate::curation::PinyinVetoes::parse("冲凉\n").unwrap()
            ),
            [("冰".to_owned(), 10)]
        );
    }

    /// 第二层的上限是同码最低权重减 1、最低为 0：极点表里权重为 0 的第一层词没有更低的权重可给，第二层与它同为 0。这时排序靠行序，第一层仍在第二层前面。
    #[test]
    fn a_first_tier_word_at_weight_zero_ties_the_second_tier_and_still_comes_first() {
        let supplement = build_with(
            "部门\tukuy\t0\r\n",
            "冲冻\tchong'dong\t100\n冲凉\tchong'liang\t9190\n",
            "",
        );
        assert_eq!(
            rows(&supplement)
                .into_iter()
                .filter(|(_, code, _)| *code == "ukuy")
                .collect::<Vec<_>>(),
            [
                ("部门", "ukuy", 0),
                ("冲凉", "ukuy", 0),
                ("冲冻", "ukuy", 0)
            ]
        );
        assert_eq!(
            merged(&supplement, "ukuy"),
            [
                ("冰".to_owned(), 10),
                ("部门".to_owned(), 0),
                ("冲凉".to_owned(), 0),
                ("冲冻".to_owned(), 0)
            ]
        );
    }

    /// 第一层最低权重大于 0 时，第二层严格低于它，即使第二层的词多到要压到 0。
    #[test]
    fn the_second_tier_is_strictly_below_a_first_tier_word_above_zero() {
        let supplement = build_with(
            "部门\tukuy\t2\r\n",
            "冲冻\tchong'dong\t100\n冲凉\tchong'liang\t9190\n冰冻\tbing'dong\t50\n",
            "",
        );
        let ukuy: Vec<_> = rows(&supplement)
            .into_iter()
            .filter(|(_, code, _)| *code == "ukuy")
            .collect();
        assert_eq!(ukuy[0], ("部门", "ukuy", 2));
        assert!(ukuy.len() >= 3, "{ukuy:?}");
        assert!(
            ukuy[1..].iter().all(|(_, _, weight)| *weight < 2),
            "{ukuy:?}"
        );
    }

    #[test]
    fn every_second_tier_weight_is_below_every_single_character() {
        let base = "冲冻\tchong'dong\t100\n冲凉\tchong'liang\t9190\n工人\tgong'ren\t100\n工人民\tgong'ren'min\t100\n人民日\tren'min'ri\t100\n";
        let supplement = build_with("部门\tukuy\t10\r\n", base, "日日日日\tri'ri'ri'ri\t1\n");
        let lowest_single = msime::wubixsj_rows(WUBIXSJ_FIXTURE)
            .unwrap()
            .rows
            .iter()
            .map(|(_, _, weight)| *weight)
            .min()
            .unwrap();
        assert_eq!(lowest_single, 10);
        let second: Vec<_> = supplement
            .entries
            .iter()
            .filter(|entry| !entry.from_jidian)
            .collect();
        assert_eq!(second.len(), 7);
        assert!(second
            .iter()
            .all(|entry| entry.weight < lowest_single && entry.weight <= SUPPLEMENT_CEILING));
    }

    #[test]
    fn upstream_words_and_extension_characters_are_left_out() {
        // 上游的「部门」不是候选来源；𠂇 是 BMP 以外的字，含它的词不收。
        let supplement = build_with("日𠂇\tjjjj\t10\r\n", "", "");
        assert_eq!(rows(&supplement), [("人民", "wwna", 9)]);
        assert_eq!(supplement.outside_basic_cjk, 1);
    }

    #[test]
    fn a_character_without_a_full_code_leaves_its_words_out() {
        let supplement = build_with("工鑫\taaqq\t10\r\n", "", "鑫日\txin'ri\t1\n");
        assert_eq!(
            supplement.underivable,
            [("工鑫".to_owned(), '鑫'), ("鑫日".to_owned(), '鑫')]
        );
    }

    #[test]
    fn rendered_rows_parse_and_merge_after_the_single_characters() {
        let supplement = build_with("部门\tukuy\t10\r\n", "冲冻\tchong'dong\t100\n", "");
        let rendered = render(
            &supplement,
            &Provenance {
                inputs: &[(WUBIXSJ, "0")],
                generator_commit: "0123456789abcdef0123456789abcdef01234567",
                upstreams: "fixture",
            },
        );
        assert!(rendered
            .lines()
            .next()
            .unwrap()
            .contains("msime 仓库提交 0123456789abcdef0123456789abcdef01234567"));
        assert_eq!(
            merged(&supplement, "ukuy"),
            [
                ("冰".to_owned(), 10),
                ("部门".to_owned(), 10),
                ("冲冻".to_owned(), 9)
            ]
        );
    }
}
