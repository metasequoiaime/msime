use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;
use crate::types::fuzzy_rule;

// 固定 81988699f365c76910b4ec7d5e761be2018ec547 的完整查询与行追加方法，只改名、转接旧入口并移除注释。
impl QuanpinDictionary {
    fn legacy_typed_fuzzy_candidates(
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
        for count in (1..=segments.len()).rev() {
            if budget <= 1 {
                break;
            }
            let prefix = &segments[..count];
            let limit = FUZZY_SEGMENTATION_LIMIT.min(budget);
            let typed = join_segments(prefix);
            let paths = fuzzy_segmentations(prefix, options, limit);
            if paths.is_empty() {
                continue;
            }
            budget -= paths.len();
            legacy_typed_append_cached_fuzzy_rows(
                &self.database,
                &mut self.fuzzy_row_cache,
                &paths,
                options.rules,
                limit,
                typed,
                &mut result,
            );
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

fn legacy_typed_append_cached_fuzzy_rows(
    database: &PinyinDatabase,
    cache: &mut FifoCache<u64, CachedFuzzyRows>,
    paths: &[Vec<String>],
    rules: u32,
    limit: usize,
    typed: String,
    result: &mut Vec<WordItem>,
) {
    let hash = fuzzy_row_cache_hash(rules, limit, &typed);
    if let Some(cached) = cache.get_ref(&hash) {
        if fuzzy_row_cache_key_matches(cached, rules, limit, &typed) {
            legacy_typed_append_fuzzy_items(cached.rows.iter().cloned(), typed, result);
            return;
        }
    }
    let rows = database.query_exact_segmentations_keyed_flat(paths, FUZZY_ROW_LIMIT);
    cache.insert(
        hash,
        CachedFuzzyRows {
            rules,
            limit,
            typed: typed.clone(),
            path_count: paths.len(),
            rows: rows.clone(),
        },
    );
    legacy_typed_append_fuzzy_items(rows.into_iter(), typed, result);
}

fn legacy_typed_append_fuzzy_items(
    mut rows: impl ExactSizeIterator<Item = DictRow>,
    typed: String,
    result: &mut Vec<WordItem>,
) {
    result.reserve(rows.len());
    if let Some(row) = rows.next() {
        let mut item = WordItem::new(
            typed,
            row.value,
            row.weight,
            CandidateSource::Database,
            row.key,
        );
        item.fuzzy = true;
        result.push(item);
    }
    for row in rows {
        let mut item = WordItem::new(
            result.last().expect("首个模糊候选已插入").pinyin.clone(),
            row.value,
            row.weight,
            CandidateSource::Database,
            row.key,
        );
        item.fuzzy = true;
        result.push(item);
    }
}

fn assert_rows(actual: &Vec<WordItem>, expected: &Vec<WordItem>) {
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

fn fixture(rows: usize) -> Fixture {
    let fixture = Fixture::new();
    for index in 0..rows {
        let weight = match index % 4 {
            0 => -7,
            1 => i64::MAX,
            _ => 100,
        };
        for (key, word) in [
            ("zhan'hao", "合成整词"),
            ("zhan", "合成前缀"),
            ("zhan'ma", "合成另一整词"),
        ] {
            fixture.insert(key, &format!("{word}{index:04}😀"), weight);
        }
    }
    fixture
}

// 独立按旧循环重新拼接，在测量外核对前缀、预算与页内容，不使用生产借用路径。
fn prefix_plan(typed: &str, options: FuzzyPinyinOptions) -> Vec<(String, usize, bool)> {
    let segments = split_segments(typed);
    let mut budget = FUZZY_PATH_BUDGET;
    let mut result = Vec::new();
    for count in (1..=segments.len()).rev() {
        if budget <= 1 {
            break;
        }
        let limit = FUZZY_SEGMENTATION_LIMIT.min(budget);
        let paths = fuzzy_segmentations(&segments[..count], options, limit);
        if !paths.is_empty() {
            budget -= paths.len();
        }
        result.push((join_segments(&segments[..count]), limit, !paths.is_empty()));
    }
    result
}

fn assert_fuzzy_state(
    before: &QuanpinDictionary,
    after: &QuanpinDictionary,
    typed: &str,
    options: FuzzyPinyinOptions,
) -> usize {
    if options.rules == 0 {
        return 0;
    }
    let outer_hash = fuzzy_cache_hash(options.rules, typed);
    let expected = before.fuzzy_cache.get_ref(&outer_hash).unwrap();
    let actual = after.fuzzy_cache.get_ref(&outer_hash).unwrap();
    assert_eq!(actual.rules, expected.rules);
    assert_eq!(actual.segmentation, expected.segmentation);
    assert_eq!(
        actual.segmentation.capacity(),
        expected.segmentation.capacity()
    );
    assert_rows(&actual.candidates, &expected.candidates);
    let mut saved = 0;
    for (prefix, limit, has_paths) in prefix_plan(typed, options) {
        let has_rows = if has_paths {
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
            !actual.rows.is_empty()
        } else {
            false
        };
        saved += usize::from(!prefix.is_empty() && !has_rows);
    }
    saved
}

#[test]
fn full_fuzzy_queries_own_typed_prefixes_only_for_real_consumers() {
    let mut saw_budget_stop = false;
    let mut saw_empty_page = false;
    let mut saw_no_path = false;
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
                let plan = prefix_plan(typed, options);
                saw_budget_stop |= plan.len() < split_segments(typed).len() && rules != 0;
                saw_no_path |= plan
                    .iter()
                    .any(|(prefix, _, has_paths)| !prefix.is_empty() && !has_paths)
                    && rules != 0;
                for mode in 0..3 {
                    let mut before = QuanpinDictionary::new(&fixture.paths);
                    let mut after = QuanpinDictionary::new(&fixture.paths);
                    if mode != 0 {
                        assert_eq!(
                            after.fuzzy_candidates(typed, options),
                            before.legacy_typed_fuzzy_candidates(typed, options)
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
                        count(|| before.legacy_typed_fuzzy_candidates(typed, options));
                    let (mut actual, new_count) = count(|| after.fuzzy_candidates(typed, options));
                    assert_rows(&actual, &expected);
                    if rows == 129 && typed == "zan" && rules == fuzzy_rule::Z_ZH {
                        assert_eq!(actual.len(), FUZZY_ROW_LIMIT);
                    }
                    let unused = assert_fuzzy_state(&before, &after, typed, options);
                    // 冻结查询仍复制输入音节，在计量外独立加入后续切片的节省。
                    let input_copies = if rules == 0 {
                        0
                    } else {
                        split_segments(typed)
                            .iter()
                            .filter(|part| !part.is_empty())
                            .count()
                    };
                    let saved = if mode == 2 {
                        0
                    } else {
                        unused + input_copies + skipped
                    };
                    saw_empty_page |=
                        rules != 0 && plan.iter().any(|(_, _, paths)| *paths) && actual.is_empty();
                    assert_eq!(
                        new_count + saved,
                        old_count,
                        "模糊 {typed}/{rules}/每页={rows}/缓存={mode}，目标省{saved}"
                    );
                    if let Some(first) = actual.first_mut() {
                        first.word.push_str("独立副本");
                        assert_rows(
                            &after.fuzzy_candidates(typed, options),
                            &before.legacy_typed_fuzzy_candidates(typed, options),
                        );
                        after.reset_cache();
                        drop(after);
                        assert!(actual[0].word.ends_with("独立副本"));
                    }
                }
            }
        }
    }
    assert!(saw_budget_stop && saw_empty_page && saw_no_path);
}

#[test]
fn supplied_owned_typed_keeps_first_row_capacity_and_cold_cache_capacity() {
    for rows in [0, 1, 8] {
        let fixture = fixture(rows);
        let old_database = PinyinDatabase::open(&fixture.database());
        let new_database = PinyinDatabase::open(&fixture.database());
        let options = FuzzyPinyinOptions {
            rules: fuzzy_rule::Z_ZH,
        };
        let paths = fuzzy_segmentations(&split_segments("zan"), options, 64);
        let mut before = FifoCache::new(2);
        let mut after = FifoCache::new(2);
        for _ in 0..2 {
            let mut old_typed = String::with_capacity(256);
            old_typed.push_str("zan");
            let mut new_typed = String::with_capacity(256);
            new_typed.push_str("zan");
            let mut expected = Vec::new();
            let mut actual = Vec::new();
            let (_, old_count) = count(|| {
                legacy_typed_append_cached_fuzzy_rows(
                    &old_database,
                    &mut before,
                    &paths,
                    options.rules,
                    64,
                    old_typed,
                    &mut expected,
                )
            });
            let (_, new_count) = count(|| {
                if let Err(new_typed) =
                    append_cached_fuzzy_rows(&mut after, options.rules, 64, new_typed, &mut actual)
                {
                    append_queried_fuzzy_rows(
                        &new_database,
                        &mut after,
                        &paths,
                        options.rules,
                        64,
                        new_typed,
                        &mut actual,
                    );
                }
            });
            assert_rows(&actual, &expected);
            assert_eq!(new_count, old_count);
            if let Some(first) = actual.first() {
                assert_eq!(first.pinyin.capacity(), 256);
            }
            let hash = fuzzy_row_cache_hash(options.rules, 64, "zan");
            assert_eq!(after.get_ref(&hash).unwrap().typed.capacity(), 3);
            assert_eq!(
                after.get_ref(&hash).unwrap().typed,
                before.get_ref(&hash).unwrap().typed
            );
        }
    }
}

#[test]
fn distinct_queries_empty_pages_fifo_and_invalidation_keep_the_old_state() {
    let fixture = fixture(1);
    let options = FuzzyPinyinOptions {
        rules: fuzzy_rule::Z_ZH,
    };
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    before.fuzzy_row_cache = FifoCache::new(2);
    after.fuzzy_row_cache = FifoCache::new(2);
    for typed in ["zan'hao", "zan'ma", "zan'fa"] {
        assert_rows(
            &after.fuzzy_candidates(typed, options),
            &before.legacy_typed_fuzzy_candidates(typed, options),
        );
    }
    let prefix_limit = FUZZY_SEGMENTATION_LIMIT;
    for dictionary in [&before, &after] {
        assert!(dictionary
            .fuzzy_row_cache
            .get_ref(&fuzzy_row_cache_hash(
                options.rules,
                prefix_limit,
                "zan'hao"
            ))
            .is_none());
        assert!(dictionary
            .fuzzy_row_cache
            .get_ref(&fuzzy_row_cache_hash(options.rules, prefix_limit, "zan'ma"))
            .is_none());
        assert!(dictionary
            .fuzzy_row_cache
            .get_ref(&fuzzy_row_cache_hash(options.rules, prefix_limit, "zan'fa"))
            .is_some());
        assert!(dictionary
            .fuzzy_row_cache
            .get_ref(&fuzzy_row_cache_hash(options.rules, prefix_limit, "zan"))
            .is_some());
    }
    assert_rows(
        &after.fuzzy_candidates("zan'fa", options),
        &before.legacy_typed_fuzzy_candidates("zan'fa", options),
    );
    fixture.insert("zhan'fa", "合成外部写入", 999);
    let (expected, old_count) = count(|| before.legacy_typed_fuzzy_candidates("zan'fa", options));
    let (actual, new_count) = count(|| after.fuzzy_candidates("zan'fa", options));
    assert_rows(&actual, &expected);
    assert!(actual.iter().any(|item| item.word == "合成外部写入"));
    assert_eq!(
        new_count + 2,
        old_count,
        "每个前缀都有候选，仍省去两个输入音节副本"
    );
}
