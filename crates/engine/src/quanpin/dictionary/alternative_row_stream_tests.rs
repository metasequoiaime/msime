use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;
use crate::quanpin::series::{
    ALTERNATIVE_SEGMENTATION_FIRST_PAGE_SIZE, ALTERNATIVE_SEGMENTATION_PROMOTION_RATIO,
    BEST_ALTERNATIVE_SEGMENTATION_INDEX,
};
use std::collections::HashSet;

const SMALL_UNIQUE_ROWS: usize = 64;

// 固定 5aff168989 的词典合并与排序去重实现，独立于新的行迭代器核心。
impl QuanpinDictionary {
    fn legacy_merge_alternatives(
        &mut self,
        raw: &str,
        primary_segmentation: &str,
        primary_segments: &[String],
        alternatives: &[Vec<String>],
        result: Vec<WordItem>,
    ) -> Vec<WordItem> {
        let rows = self.database.query_exact_segmentations_keyed_flat(
            alternatives,
            ALTERNATIVE_SEGMENTATION_CANDIDATE_LIMIT,
        );
        if rows.is_empty() {
            return result;
        }
        let primary_full = self.query_single_path(raw, primary_segmentation, primary_segments);
        let alternative_full = rows
            .into_iter()
            .map(|row| {
                WordItem::new(
                    row.key.clone(),
                    row.value,
                    row.weight,
                    CandidateSource::Database,
                    row.key,
                )
            })
            .collect();
        legacy_merge_alternative_segmentations(result, primary_full, alternative_full)
    }
}

fn rows_equal(actual: &Vec<WordItem>, expected: &Vec<WordItem>) {
    assert_eq!(actual, expected);
    assert_eq!(actual.capacity(), expected.capacity());
    for (actual, expected) in actual.iter().zip(expected) {
        for (actual, expected) in [
            (&actual.pinyin, &expected.pinyin),
            (&actual.canonical_pinyin, &expected.canonical_pinyin),
            (&actual.word, &expected.word),
            (&actual.corrected_from, &expected.corrected_from),
        ] {
            assert_eq!(actual.capacity(), expected.capacity());
        }
        assert_eq!(
            actual.sentence_words.capacity(),
            expected.sentence_words.capacity()
        );
        for (actual, expected) in actual.sentence_words.iter().zip(&expected.sentence_words) {
            assert_eq!(actual.capacity(), expected.capacity());
        }
    }
}

#[test]
fn alternative_merge_streams_hot_primary_rows() {
    let fixture = Fixture::new();
    for (key, word, weight) in [
        ("xi'an", "合成主词", 100),
        ("xi'an", "合成次词", 50),
        ("xian", "合成替词", 80),
        ("xian", "合成主词", 40),
    ] {
        fixture.insert(key, word, weight);
    }
    let alternatives = vec![vec!["xian".to_owned()]];
    for primary in ["xi'an", "missing"] {
        let segments = primary.split('\'').map(str::to_owned).collect::<Vec<_>>();
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        let mut retained = None;
        for hot in [false, true] {
            let mut rows = vec![WordItem::new(
                "xi'an",
                "合成整句😀",
                -100,
                CandidateSource::Generated,
                "xi'an",
            )];
            rows[0].sentence_association = true;
            rows[0].sentence_words = vec!["合成".into(), "整句😀".into()];
            rows[0].corrected_from = "synthetic".into();
            let old_rows = rows.clone();
            let (expected, old) = count(|| {
                before.legacy_merge_alternatives(
                    "xian",
                    primary,
                    &segments,
                    &alternatives,
                    old_rows,
                )
            });
            let (actual, new) = count(|| {
                after.merge_alternative_full("xian", primary, &segments, &alternatives, rows)
            });
            rows_equal(&actual, &expected);
            let saved = usize::from(hot && primary != "missing");
            eprintln!("{primary} hot={hot}: {old}->{new}");
            assert_eq!(new + saved, old);
            retained = Some(actual);
        }
        let mut retained = retained.unwrap();
        let expected = retained.clone();
        after.reset_cache();
        drop(after);
        assert_eq!(retained, expected);
        retained[0].word.push_str("修改");
        assert_ne!(retained, expected);
    }
}

fn legacy_merge_alternative_segmentations(
    result: Vec<WordItem>,
    primary_full: Vec<WordItem>,
    alternatives: Vec<WordItem>,
) -> Vec<WordItem> {
    let Some(best_alternative) = alternatives.first() else {
        return result;
    };
    let promote = best_alternative
        .weight
        .saturating_mul(ALTERNATIVE_SEGMENTATION_PROMOTION_RATIO)
        >= primary_full.first().map_or(0, |item| item.weight);
    let best_word_pointer = best_alternative.word.as_ptr();
    let duplicate_best_word = (promote
        && primary_full
            .iter()
            .chain(alternatives.iter().skip(1))
            .any(|item| item.word == best_alternative.word))
    .then(|| best_alternative.word.to_string());

    let capacity = result
        .len()
        .saturating_add(primary_full.len())
        .saturating_add(alternatives.len());
    let mut merged = Vec::with_capacity(capacity);
    merged.extend(primary_full);
    merged.extend(alternatives);
    merged.sort_by_key(|item| std::cmp::Reverse(item.weight));
    retain_unique_sorted_rows(&mut merged);

    if promote {
        let at = merged
            .iter()
            .position(|item| item.word.as_ptr() == best_word_pointer)
            .or_else(|| {
                duplicate_best_word
                    .as_deref()
                    .and_then(|word| merged.iter().position(|item| item.word == word))
            });
        if let Some(at) = at {
            if at >= ALTERNATIVE_SEGMENTATION_FIRST_PAGE_SIZE {
                let promoted = merged.remove(at);
                merged.insert(BEST_ALTERNATIVE_SEGMENTATION_INDEX, promoted);
            }
        }
    }

    append_unique_words(&mut merged, result);
    merged
}

fn retain_unique_sorted_rows(rows: &mut Vec<WordItem>) {
    if rows.len() <= SMALL_UNIQUE_ROWS {
        let mut write = 0;
        for read in 0..rows.len() {
            if rows[..write]
                .iter()
                .any(|item| item.word == rows[read].word)
            {
                continue;
            }
            if write != read {
                rows.swap(write, read);
            }
            write += 1;
        }
        rows.truncate(write);
        return;
    }
    let mut seen = HashSet::with_capacity(rows.len());
    let duplicates = rows
        .iter()
        .enumerate()
        .filter_map(|(index, item)| (!seen.insert(item.word.as_str())).then_some(index))
        .collect::<Vec<_>>();
    drop(seen);
    let mut duplicates = duplicates.into_iter().peekable();
    let mut write = 0;
    for read in 0..rows.len() {
        if duplicates.peek() == Some(&read) {
            duplicates.next();
            continue;
        }
        if write != read {
            rows.swap(write, read);
        }
        write += 1;
    }
    rows.truncate(write);
}

#[test]
fn alternative_stream_preserves_promotion_ties_and_metadata() {
    for width in [0, 1, 16, 64, 96] {
        for duplicate in [false, true] {
            for best_weight in [-1, 0, 100, i64::MAX] {
                let mut primary: Vec<WordItem> = (0..width)
                    .map(|index| {
                        let mut row = WordItem::new(
                            "xi'an",
                            format!("合成主{index}😀"),
                            10_000 - index as i64,
                            CandidateSource::Database,
                            "xi'an",
                        );
                        row.fixed_position = (index % 6) as i32;
                        row.fuzzy = index % 2 == 0;
                        row.corrected_from = "synthetic".into();
                        row.sentence_words = vec!["合成".into(), "边界😀".into()];
                        row
                    })
                    .collect();
                let best_word = if duplicate && width > 0 {
                    primary[width - 1].word.to_string()
                } else {
                    "合成替词😀".into()
                };
                let alternatives = vec![
                    WordItem::new(
                        "xian",
                        &best_word,
                        best_weight,
                        CandidateSource::Database,
                        "xian",
                    ),
                    WordItem::new(
                        "xian",
                        &best_word,
                        best_weight,
                        CandidateSource::UserDatabase,
                        "xian",
                    ),
                ];
                let mut result = vec![WordItem::new(
                    "xi",
                    "合成整句😀",
                    0,
                    CandidateSource::Generated,
                    "xi'an",
                )];
                result[0].sentence_association = true;
                result.extend(primary.iter().cloned());
                let old_result = result.clone();
                let old_alternatives = alternatives.clone();
                let (expected, old) = count(|| {
                    legacy_merge_alternative_segmentations(
                        old_result,
                        primary.clone(),
                        old_alternatives,
                    )
                });
                let (actual, new) =
                    count(|| merge_alternative_rows(result, primary.iter().cloned(), alternatives));
                rows_equal(&actual, &expected);
                // 冻结实现为重复的最佳词面复制一份字符串，新实现只克隆 `SharedString`；两条备选同词，晋升时必有重复，于是再省一次。
                let promoted = best_weight.saturating_mul(ALTERNATIVE_SEGMENTATION_PROMOTION_RATIO)
                    >= primary.first().map_or(0, |item| item.weight);
                assert_eq!(
                    new + usize::from(width > 0) + usize::from(promoted),
                    old,
                    "width={width}, duplicate={duplicate}, weight={best_weight}"
                );
                let mut owned = primary.clone();
                for row in &mut owned {
                    row.word.reserve(32);
                }
                let owned_alternatives = vec![WordItem::new(
                    "xian",
                    "合成替词😀",
                    100,
                    CandidateSource::Database,
                    "xian",
                )];
                let expected = legacy_merge_alternative_segmentations(
                    Vec::new(),
                    owned.clone(),
                    owned_alternatives.clone(),
                );
                let actual =
                    merge_alternative_segmentations(Vec::new(), owned.clone(), owned_alternatives);
                rows_equal(&actual, &expected);
                primary.clear();
            }
        }
    }
}

#[test]
fn alternative_stream_does_not_consume_primary_without_alternatives() {
    let primary = [WordItem::new(
        "xi'an",
        "合成主词",
        100,
        CandidateSource::Database,
        "xi'an",
    )];
    let mut result = vec![WordItem::new(
        "xi",
        "合成尾词",
        1,
        CandidateSource::Generated,
        "xi",
    )];
    result.reserve(40);
    let pointer = result.as_ptr();
    let capacity = result.capacity();
    let (actual, allocations) =
        count(|| merge_alternative_rows(result, primary.iter().cloned(), Vec::new()));
    assert_eq!(actual.as_ptr(), pointer);
    assert_eq!(actual.capacity(), capacity);
    assert_eq!(allocations, 0);
}

#[test]
fn alternative_stream_keeps_cached_fields_and_fifo_age() {
    let fixture = Fixture::new();
    fixture.insert("xian", "合成替词", 80);
    for empty in [false, true] {
        let mut row = WordItem::new(
            "xi'an",
            "合成缓存😀",
            100,
            CandidateSource::CloudSuggestion,
            "xi'an",
        );
        row.word.reserve(40);
        row.corrected_from = "synthetic".into();
        row.corrected_from.reserve(40);
        row.fixed_position = 3;
        row.fuzzy = true;
        row.sentence_association = true;
        row.sentence_words = vec!["合成".into(), "边界😀".into()];
        row.sentence_words[0].reserve(40);
        row.sentence_words.reserve(8);
        let rows = if empty { Vec::new() } else { vec![row] };
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        before.cache = FifoCache::new(2);
        after.cache = FifoCache::new(2);
        before.cache.insert("xi'an".into(), rows.clone().into());
        after.cache.insert("xi'an".into(), rows.into());
        before.cache.insert("second".into(), Vec::new().into());
        after.cache.insert("second".into(), Vec::new().into());
        let before_cached = before.cache.get_ref_by("xi'an").unwrap().clone();
        let after_cached = after.cache.get_ref_by("xi'an").unwrap().clone();
        let segments = vec!["xi".into(), "an".into()];
        let alternatives = vec![vec!["xian".into()]];
        let expected =
            before.legacy_merge_alternatives("xian", "xi'an", &segments, &alternatives, Vec::new());
        let mut actual =
            after.merge_alternative_full("xian", "xi'an", &segments, &alternatives, Vec::new());
        rows_equal(&actual, &expected);
        assert_eq!(before.cache.get_ref_by("xi'an").unwrap(), &before_cached);
        assert_eq!(after.cache.get_ref_by("xi'an").unwrap(), &after_cached);
        actual[0].word.push_str("修改");
        assert_eq!(after.cache.get_ref_by("xi'an").unwrap(), &after_cached);
        before.cache.insert("third".into(), Vec::new().into());
        after.cache.insert("third".into(), Vec::new().into());
        for dictionary in [&before, &after] {
            assert!(dictionary.cache.get_ref_by("xi'an").is_none());
            assert!(dictionary.cache.get_ref_by("second").is_some());
        }
    }
}
