//! 86、98 五笔词组补充表与新世纪五笔词组表共用的编码与排名规则。

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::curation::PinyinCuration;
use crate::places_supplement::is_han;

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
    msime_engine::wubi_phrase_code(word, |c, letters| {
        prefix(codes, c, letters).map(str::to_owned)
    })
    .expect("only words of two or more characters are coded")
}

pub(crate) fn is_word(value: &str) -> bool {
    value.chars().count() >= 2 && value.chars().all(is_han)
}

pub(crate) struct RankedEntry<'a> {
    pub word: &'a str,
    pub code: String,
    pub weight: i64,
}

pub(crate) struct Ranked<'a> {
    pub entries: Vec<RankedEntry<'a>>,
    pub underivable: Vec<(String, char)>,
    pub unique_codes_shared: usize,
}

/// 拼音来源里每个词的两种权重，各取同词所有读音的最大值。
pub(crate) struct Frequencies<'a> {
    /// 判 100 门槛用（[`pinyin_words`]）。
    pub threshold: HashMap<&'a str, i64>,
    /// 同码排序用（[`rank`]）。
    pub order: HashMap<&'a str, i64>,
}

/// 语料里同一词可能出现多次；取最大权重。行按 `curation` 读（[`PinyinCuration::wubi_rows`]）：否决的行不算；权重覆盖只有第四列写了 `wubi` 的频率修正才改同码排序，而且不会让一个词掉到门槛以下，相对全拼键的降权（碎片、非标准读音、custom 越位）五笔不理。
pub(crate) fn frequencies<'a>(
    sources: impl IntoIterator<Item = &'a str>,
    curation: &'a PinyinCuration,
) -> Frequencies<'a> {
    let mut frequencies = Frequencies {
        threshold: HashMap::new(),
        order: HashMap::new(),
    };
    for (value, threshold, order) in sources
        .into_iter()
        .flat_map(|source| curation.wubi_rows(source))
    {
        let slot = frequencies.threshold.entry(value).or_insert(threshold);
        *slot = (*slot).max(threshold);
        let slot = frequencies.order.entry(value).or_insert(order);
        *slot = (*slot).max(order);
    }
    frequencies
}

/// 拼音来源里能做五笔词组的词：`frequency` 里权重不低于 `minimum` 的，加上 `manual`（区划表、人工词条）里不设门槛的。`curation` 整词否决的词一个也不收，`manual` 里被否决的读音也不算。
pub(crate) fn pinyin_words<'a>(
    frequency: &'a HashMap<&'a str, i64>,
    minimum: i64,
    manual: &[&'a str],
    curation: &'a PinyinCuration,
) -> BTreeSet<&'a str> {
    frequency
        .iter()
        .filter(|(_, weight)| **weight >= minimum)
        .map(|(word, _)| *word)
        .chain(
            manual
                .iter()
                .flat_map(|source| curation.rows(source))
                .map(|(word, _, _)| word),
        )
        .filter(|word| is_word(word) && !curation.vetoes_word(word))
        .collect()
}

/// 既有行定义全码、同码最低权重和唯一四码；候选已由各版本独立筛选。
pub(crate) fn rank<'a>(
    rows: &[(String, &str, i64)],
    candidates: impl IntoIterator<Item = &'a str>,
    frequency: &HashMap<&str, i64>,
    ceiling_limit: i64,
) -> Ranked<'a> {
    let codes = full_codes(rows);
    let mut lowest: HashMap<&str, i64> = HashMap::new();
    let mut per_code: HashMap<&str, usize> = HashMap::new();
    for (code, _, weight) in rows {
        let slot = lowest.entry(code.as_str()).or_insert(*weight);
        *slot = (*slot).min(*weight);
        *per_code.entry(code.as_str()).or_default() += 1;
    }
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
        words.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let ceiling = match lowest.get(code.as_str()) {
            Some(lowest) => ceiling_limit.min(lowest - 1),
            None => ceiling_limit,
        };
        if code.len() == 4 && per_code.get(code.as_str()) == Some(&1) {
            unique_codes_shared += 1;
        }
        for (rank, (word, _)) in words.into_iter().enumerate() {
            entries.push(RankedEntry {
                word,
                code: code.clone(),
                weight: (ceiling - rank as i64).max(0),
            });
        }
    }
    Ranked {
        entries,
        underivable,
        unique_codes_shared,
    }
}

/// 86、98 补充表表头列出的上游。
pub(crate) const UPSTREAMS: [&str; 4] = [
    "rime-ice",
    "rime-ice-supplement",
    "98wubi-tables",
    "fcitx5-table-extra",
];

/// 来源引用来自已校验的构建锁，不把上游提交再硬编码一遍；`names` 是要列出的 reference，按给出的顺序。
pub(crate) fn upstreams(lock: &crate::sources::Lock, names: &[&str]) -> String {
    names
        .iter()
        .filter_map(|name| lock.references.get(*name))
        .map(|source| format!("{} 提交 {}", source.repository, source.commit))
        .collect::<Vec<_>>()
        .join("、")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_merge_sources_keep_manual_low_weights_and_deduplicate_readings() {
        let base =
            "日工\tri'gong\t100\n工人\tgong'ren\t99\n日\tri\t10000\n工人日\tgong'ren'ri\t100\n";
        let supplement = "工人\tgong'ren\t150\n工人\tgong'reng\t10\n人民\tren'min\t99\n";
        let custom = "工人民\tgong'ren'min\t1\nASCII\ta\t100\n";
        let places = "工人日民\tgong'ren'ri'min\t1\n";
        let curation = PinyinCuration::default();
        let frequency = frequencies([base, supplement, places, custom], &curation);
        assert_eq!(frequency.threshold["工人"], 150);
        assert_eq!(frequency.order["工人"], 150);
        let words = pinyin_words(&frequency.threshold, 100, &[places, custom], &curation);
        assert_eq!(
            words.into_iter().collect::<Vec<_>>(),
            ["工人", "工人日", "工人日民", "工人民", "日工"]
        );
    }

    /// 否决的词和读音不进候选；人工词条里被否决的 工人民 也不收。权重覆盖不会让词掉出门槛：工人日 写了 `wubi` 降到 99，仍是候选，只是排序按 99；人民 写了 `wubi` 升到 100 就是候选；没写 `wubi` 的 民工 升到 100 只改拼音，五笔仍按 99 算。
    #[test]
    fn candidates_follow_the_curation_tables() {
        let base = "日工\tri'gong\t100\n工人\tgong'ren\t150\n工人\tgong'reng\t10\n工人日\tgong'ren'ri\t100\n人民\tren'min\t99\n民工\tmin'gong\t99\n";
        let custom = "工人民\tgong'ren'min\t1\n";
        let curation = PinyinCuration::parse(
            "日工\n工人民\n工人\tgong'ren\n",
            "工人日\tgong'ren'ri\t99\twubi\n人民\tren'min\t100\twubi\n民工\tmin'gong\t100\n",
        )
        .unwrap();
        let frequency = frequencies([base, custom], &curation);
        assert_eq!(frequency.threshold["工人"], 10);
        assert!(!frequency.threshold.contains_key("日工"));
        assert_eq!(frequency.threshold["工人日"], 100);
        assert_eq!(frequency.order["工人日"], 99);
        assert_eq!(frequency.order["民工"], 99);
        let words = pinyin_words(&frequency.threshold, 100, &[custom], &curation);
        assert_eq!(words.into_iter().collect::<Vec<_>>(), ["人民", "工人日"]);
    }
}
