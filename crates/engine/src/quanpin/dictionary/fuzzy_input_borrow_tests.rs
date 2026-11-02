use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::pinyin::fuzzy::variant_view_tests::{
    legacy_syllables as fuzzy_syllables, storage_savings,
};
use crate::quanpin::fixture::Fixture;
use crate::types::fuzzy_rule;

// 固定 bdf8b5f39daf1a7b325501d428356ef0d5bbc9c3 的完整查询与 beam，只改名、转接冻结入口并移除注释。
impl QuanpinDictionary {
    fn legacy_input_fuzzy_candidates(
        &mut self,
        segmentation: &str,
        options: FuzzyPinyinOptions,
    ) -> Vec<WordItem> {
        if options.rules == 0 || !self.database.is_open() {
            return Vec::new();
        }
        self.reset_cache_if_database_changed();
        let hash = fuzzy_cache_hash(options.rules, segmentation);
        if let Some(cached) = self.fuzzy_cache.get_ref(&hash) {
            if fuzzy_cache_key_matches(cached, options.rules, segmentation) {
                return cached.candidates.clone();
            }
        }
        self.validate_row_caches();
        let segments = split_segments(segmentation);
        let mut result = Vec::new();
        let mut budget = FUZZY_PATH_BUDGET;
        let mut typed = segmentation;
        for count in (1..=segments.len()).rev() {
            if budget <= 1 {
                break;
            }
            if count < segments.len() {
                typed = typed.rsplit_once('\'').map_or("", |(prefix, _)| prefix);
            }
            let prefix = &segments[..count];
            let limit = FUZZY_SEGMENTATION_LIMIT.min(budget);
            // 路径照样算出来（纯计算，不查库），预算照旧按路径数扣；只有查库这一步走缓存。
            let paths = legacy_input_segmentations(prefix, options, limit);
            if paths.is_empty() {
                continue;
            }
            budget -= paths.len();
            if append_cached_fuzzy_rows(
                &mut self.fuzzy_row_cache,
                options.rules,
                limit,
                typed,
                &mut result,
            )
            .is_err()
            {
                append_queried_fuzzy_rows(
                    &self.database,
                    &mut self.fuzzy_row_cache,
                    &paths,
                    options.rules,
                    limit,
                    typed,
                    &mut result,
                );
            }
        }
        self.fuzzy_cache.insert(
            hash,
            CachedFuzzyCandidates {
                rules: options.rules,
                segmentation: segmentation.to_owned(),
                candidates: result.clone(),
            },
        );
        result
    }
}

fn legacy_input_segmentations(
    segments: &[String],
    options: FuzzyPinyinOptions,
    limit: usize,
) -> Vec<Vec<String>> {
    if options.rules == 0 || segments.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut paths: Vec<Vec<String>> = vec![Vec::new()];
    for syllable in segments {
        let mut alternatives = fuzzy_syllables(syllable, options);
        if let [alternative] = alternatives.as_mut_slice() {
            let last_index = paths.len() - 1;
            for (index, path) in paths.iter_mut().enumerate() {
                path.push(if index == last_index {
                    std::mem::take(alternative)
                } else {
                    alternative.clone()
                });
            }
            continue;
        }
        let capacity = limit.min(paths.len().saturating_mul(alternatives.len()));
        let mut next = Vec::with_capacity(capacity);
        'beam: for mut path in paths {
            let take = (limit - next.len()).min(alternatives.len());
            let last_parent = next.len() + take == capacity;
            let take_alternative = |alternative: &mut String| {
                if last_parent {
                    std::mem::take(alternative)
                } else {
                    alternative.clone()
                }
            };
            for alternative in alternatives.iter_mut().take(take.saturating_sub(1)) {
                let mut extended = Vec::with_capacity(path.len() + 1);
                extended.extend_from_slice(&path);
                extended.push(take_alternative(alternative));
                next.push(extended);
            }
            if take == 0 {
                break 'beam;
            }
            path.push(take_alternative(&mut alternatives[take - 1]));
            next.push(path);
            if next.len() == limit {
                break 'beam;
            }
        }
        paths = next;
    }
    paths.retain(|path| path.as_slice() != segments);
    paths
}

fn assert_items(actual: &Vec<WordItem>, expected: &Vec<WordItem>) {
    assert_eq!(actual, expected);
    assert_eq!(actual.capacity(), expected.capacity());
    for (actual, expected) in actual.iter().zip(expected) {
        for (actual, expected) in [
            (&actual.pinyin, &expected.pinyin),
            (&actual.word, &expected.word),
            (&actual.canonical_pinyin, &expected.canonical_pinyin),
            (&actual.corrected_from, &expected.corrected_from),
        ] {
            assert_eq!(actual.capacity(), expected.capacity());
        }
        assert_eq!(
            actual.sentence_words.capacity(),
            expected.sentence_words.capacity()
        );
    }
}

fn fixture(rows: usize) -> Fixture {
    let fixture = Fixture::new();
    for index in 0..rows {
        let weight = if index % 3 == 0 { i64::MAX } else { -7 };
        for key in ["zhan", "zhan'hao", "zhan'ma"] {
            fixture.insert(key, &format!("合成{index:04}😀"), weight);
        }
    }
    fixture
}

fn assert_state(
    before: &QuanpinDictionary,
    after: &QuanpinDictionary,
    typed: &str,
    options: FuzzyPinyinOptions,
) {
    if options.rules == 0 {
        return;
    }
    let hash = fuzzy_cache_hash(options.rules, typed);
    let expected = before.fuzzy_cache.get_ref(&hash).unwrap();
    let actual = after.fuzzy_cache.get_ref(&hash).unwrap();
    assert_eq!(actual.rules, expected.rules);
    assert_eq!(actual.segmentation, expected.segmentation);
    assert_eq!(
        actual.segmentation.capacity(),
        expected.segmentation.capacity()
    );
    assert_items(&actual.candidates, &expected.candidates);
    let segments = split_segments(typed);
    let mut budget = FUZZY_PATH_BUDGET;
    for count in (1..=segments.len()).rev() {
        if budget <= 1 {
            break;
        }
        let limit = FUZZY_SEGMENTATION_LIMIT.min(budget);
        let paths = legacy_input_segmentations(&segments[..count], options, limit);
        if paths.is_empty() {
            continue;
        }
        budget -= paths.len();
        let prefix = join_segments(&segments[..count]);
        let hash = fuzzy_row_cache_hash(options.rules, limit, &prefix);
        let expected = before.fuzzy_row_cache.get_ref(&hash).unwrap();
        let actual = after.fuzzy_row_cache.get_ref(&hash).unwrap();
        assert_eq!(actual.rules, expected.rules);
        assert_eq!(actual.limit, expected.limit);
        assert_eq!(actual.typed, expected.typed);
        assert_eq!(actual.typed.capacity(), expected.typed.capacity());
        assert_eq!(actual.rows, expected.rows);
        assert_eq!(actual.rows.capacity(), expected.rows.capacity());
        for (actual, expected) in actual.rows.iter().zip(&expected.rows) {
            assert_eq!(actual.key.capacity(), expected.key.capacity());
            assert_eq!(actual.value.capacity(), expected.value.capacity());
        }
    }
}

fn query_variant_savings(typed: &str, options: FuzzyPinyinOptions) -> usize {
    if options.rules == 0 {
        return 0;
    }
    let segments = split_segments(typed);
    let mut budget = FUZZY_PATH_BUDGET;
    let mut saved = 0;
    for count in (1..=segments.len()).rev() {
        if budget <= 1 {
            break;
        }
        let limit = FUZZY_SEGMENTATION_LIMIT.min(budget);
        saved += storage_savings(&segments[..count], options, limit);
        budget -= legacy_input_segmentations(&segments[..count], options, limit).len();
    }
    saved
}

#[test]
fn full_fuzzy_queries_borrow_each_nonempty_input_segment() {
    for rows in [0, 1, 129] {
        let fixture = fixture(rows);
        for typed in [
            "",
            "x",
            "ni",
            "hao'ma",
            "zan",
            "zan'hao",
            "zan'ma",
            "zan'",
            "'zan",
            "zan''hao",
            "'",
            "''",
            "合成'😀",
            "ZAN'hao",
            "zan'zan'zan'zan'zan'zan'zan'zan",
        ] {
            for rules in [0, fuzzy_rule::Z_ZH, fuzzy_rule::AN_ANG, fuzzy_rule::ALL] {
                let options = FuzzyPinyinOptions { rules };
                for mode in 0..3 {
                    let mut before = QuanpinDictionary::new(&fixture.paths);
                    let mut after = QuanpinDictionary::new(&fixture.paths);
                    if mode != 0 {
                        assert_items(
                            &after.fuzzy_candidates(typed, options),
                            &before.legacy_input_fuzzy_candidates(typed, options),
                        );
                        if mode == 1 {
                            before.fuzzy_cache.clear();
                            after.fuzzy_cache.clear();
                        }
                    }
                    let skipped = super::fuzzy_row_projection_tests::skipped_path_allocations(
                        &after, typed, options,
                    );
                    let (expected, old_count) =
                        count(|| before.legacy_input_fuzzy_candidates(typed, options));
                    let (mut actual, new_count) = count(|| after.fuzzy_candidates(typed, options));
                    assert_items(&actual, &expected);
                    assert_state(&before, &after, typed, options);
                    let saved = if rules == 0 || mode == 2 {
                        0
                    } else {
                        typed.split('\'').filter(|part| !part.is_empty()).count()
                            + query_variant_savings(typed, options)
                            + skipped
                    };
                    assert_eq!(
                        new_count + saved,
                        old_count,
                        "输入音节 {typed}/{rules}/每页={rows}/缓存={mode}，目标省{saved}"
                    );
                    if typed == "zan" && rows == 129 && rules == fuzzy_rule::Z_ZH {
                        assert_eq!(actual.len(), FUZZY_ROW_LIMIT);
                    }
                    if let Some(item) = actual.first_mut() {
                        item.word.push_str("独立返回");
                        assert_items(
                            &after.fuzzy_candidates(typed, options),
                            &before.legacy_input_fuzzy_candidates(typed, options),
                        );
                        after.reset_cache();
                        drop(after);
                        assert!(actual[0].word.ends_with("独立返回"));
                    }
                }
            }
        }
    }
}

#[test]
fn owned_and_borrowed_fuzzy_paths_keep_the_frozen_beam_cost_and_capacity() {
    for pieces in [
        vec![],
        vec![""],
        vec!["guo"],
        vec!["zan"],
        vec!["zan", "fa"],
        vec!["zan", "", "hao"],
        vec!["合成", "😀"],
        vec!["ZAN", "hao"],
        vec!["zan'hao"],
        vec!["guo", "zan", "guo"],
        vec!["zan"; 8],
        vec!["guo"; 40],
    ] {
        let owned: Vec<String> = pieces.iter().map(|piece| (*piece).to_owned()).collect();
        for rules in [0, fuzzy_rule::Z_ZH, fuzzy_rule::ALL] {
            let options = FuzzyPinyinOptions { rules };
            for limit in [0, 1, 2, 3, 5, 64, 128, usize::MAX] {
                let _ = fuzzy_syllables("zan", options);
                let (expected, old_count) =
                    count(|| legacy_input_segmentations(&owned, options, limit));
                let (actual, new_count) = count(|| fuzzy_segmentations(&owned, options, limit));
                let (borrowed, borrowed_count) =
                    count(|| fuzzy_segmentations(&pieces, options, limit));
                assert_eq!(actual, expected);
                assert_eq!(borrowed, expected);
                let saved = storage_savings(&owned, options, limit);
                assert_eq!(new_count + saved, old_count);
                assert_eq!(borrowed_count + saved, old_count);
                let capacity = if expected.is_empty() {
                    0
                } else {
                    expected.capacity()
                };
                assert_eq!(actual.capacity(), capacity);
                assert_eq!(borrowed.capacity(), capacity);
                for (actual, expected) in actual.iter().zip(&expected) {
                    assert_eq!(actual.capacity(), expected.capacity());
                    for (actual, expected) in actual.iter().zip(expected) {
                        assert_eq!(actual.capacity(), expected.capacity());
                    }
                }
                for (actual, expected) in borrowed.iter().zip(&expected) {
                    assert_eq!(actual.capacity(), expected.capacity());
                    for (actual, expected) in actual.iter().zip(expected) {
                        assert_eq!(actual.capacity(), expected.capacity());
                    }
                }
            }
        }
    }
}

#[test]
fn closed_database_skips_input_copy_and_external_invalidation_rebuilds_owned_rows() {
    let fixture = fixture(1);
    let options = FuzzyPinyinOptions {
        rules: fuzzy_rule::Z_ZH,
    };
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    for typed in ["zan'hao", "zan'ma", "zan'fa"] {
        assert_items(
            &after.fuzzy_candidates(typed, options),
            &before.legacy_input_fuzzy_candidates(typed, options),
        );
    }
    fixture.insert("zhan'fa", "合成外部写入", 999);
    let mut typed = String::from("zan'fa");
    let (expected, old_count) = count(|| before.legacy_input_fuzzy_candidates(&typed, options));
    let (actual, new_count) = count(|| after.fuzzy_candidates(&typed, options));
    assert_items(&actual, &expected);
    assert_eq!(
        new_count + 2 + query_variant_savings("zan'fa", options),
        old_count
    );
    assert!(actual.iter().any(|item| item.word == "合成外部写入"));
    typed.clear();
    assert_state(&before, &after, "zan'fa", options);
    after.reset_cache();
    drop(after);
    assert_eq!(actual, expected);
    before.database = PinyinDatabase::open(&fixture.database().with_file_name("合成缺失.db"));
    let (actual, allocations) = count(|| before.fuzzy_candidates("zan'hao", options));
    assert!(actual.is_empty());
    assert_eq!(allocations, 0);
}
