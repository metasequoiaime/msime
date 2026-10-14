use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;
use crate::types::fuzzy_rule;

// 固定 db0b15772189fc9fe7ea7afadd2f3d64e9d89cc6 的完整模糊候选与行缓存方法，只改名和转接旧入口，并移除注释。
impl QuanpinDictionary {
    fn legacy_fuzzy_candidates(
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
            let rows = legacy_cached_fuzzy_rows(
                &self.database,
                &mut self.fuzzy_row_cache,
                &paths,
                options.rules,
                limit,
                &typed,
            );
            result.reserve(rows.len());
            let mut rows = rows.into_iter();
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
                    result
                        .last()
                        .expect("the first fuzzy row was inserted")
                        .pinyin
                        .clone(),
                    row.value,
                    row.weight,
                    CandidateSource::Database,
                    row.key,
                );
                item.fuzzy = true;
                result.push(item);
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

fn legacy_cached_fuzzy_rows(
    database: &PinyinDatabase,
    cache: &mut FifoCache<u64, CachedFuzzyRows>,
    paths: &[Vec<String>],
    rules: u32,
    limit: usize,
    typed: &str,
) -> Vec<DictRow> {
    let hash = fuzzy_row_cache_hash(rules, limit, typed);
    if let Some(cached) = cache.get_ref(&hash) {
        if fuzzy_row_cache_key_matches(cached, rules, limit, typed) {
            return cached.rows.clone();
        }
    }
    let rows = database.query_exact_segmentations_keyed_flat(paths, FUZZY_ROW_LIMIT);
    cache.insert(
        hash,
        CachedFuzzyRows {
            rules,
            limit,
            typed: typed.to_owned(),
            rows: rows.clone(),
        },
    );
    rows
}

fn fixture(rows: usize) -> Fixture {
    let fixture = Fixture::new();
    for index in 0..rows {
        let weight = match index % 4 {
            0 => -7,
            1 => i64::MAX,
            _ => 100,
        };
        fixture.insert("zhan'hao", &format!("合成整词{index:04}😀"), weight);
        fixture.insert("zhan", &format!("合成前缀{index:04}😀"), weight);
        fixture.insert("zhan'ma", &format!("合成另一整词{index:04}😀"), weight);
    }
    fixture
}

fn options() -> FuzzyPinyinOptions {
    FuzzyPinyinOptions {
        rules: fuzzy_rule::Z_ZH,
    }
}

fn assert_fields_and_capacity(actual: &[WordItem], expected: &[WordItem]) {
    assert_eq!(actual, expected);
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.pinyin.capacity(), expected.pinyin.capacity());
        assert_eq!(
            actual.canonical_pinyin.capacity(),
            expected.canonical_pinyin.capacity()
        );
        assert_eq!(actual.word.capacity(), expected.word.capacity());
    }
}

#[test]
fn hot_fuzzy_rows_project_without_temporary_pages() {
    for rows in [1, 8, 24, 64, 129] {
        let fixture = fixture(rows);
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        let expected = before.legacy_fuzzy_candidates("zan'hao", options());
        let actual = after.fuzzy_candidates("zan'hao", options());
        assert_fields_and_capacity(&actual, &expected);
        assert_eq!(actual.capacity(), expected.capacity());
        before.fuzzy_cache.clear();
        after.fuzzy_cache.clear();
        let (expected, old_count) = count(|| before.legacy_fuzzy_candidates("zan'hao", options()));
        let (actual, new_count) = count(|| after.fuzzy_candidates("zan'hao", options()));
        assert_fields_and_capacity(&actual, &expected);
        assert_eq!(actual.capacity(), expected.capacity());
        println!("模糊热行每表 {rows} 行：申请 {old_count}→{new_count}");
        assert_eq!(
            new_count + 2 + 2,
            old_count,
            "两个非空前缀页各省一次临时向量，输入音节另省两个副本"
        );
    }
}

#[test]
fn distinct_inputs_share_hot_prefix_without_changing_cold_or_outer_hits() {
    let fixture = fixture(8);
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    let (expected, old_count) = count(|| before.legacy_fuzzy_candidates("zan'hao", options()));
    let (actual, new_count) = count(|| after.fuzzy_candidates("zan'hao", options()));
    assert_fields_and_capacity(&actual, &expected);
    assert_eq!(actual.capacity(), expected.capacity());
    assert_eq!(new_count + 2, old_count, "全冷查询省去两个输入音节副本");
    let (expected, old_count) = count(|| before.legacy_fuzzy_candidates("zan'ma", options()));
    let (actual, new_count) = count(|| after.fuzzy_candidates("zan'ma", options()));
    assert_fields_and_capacity(&actual, &expected);
    assert_eq!(actual.capacity(), expected.capacity());
    println!("不同输入共享模糊前缀：申请 {old_count}→{new_count}");
    assert_eq!(
        new_count + 1 + 2,
        old_count,
        "共享前缀页省一次临时向量，输入音节另省两个副本"
    );
    let (expected, old_count) = count(|| before.legacy_fuzzy_candidates("zan'ma", options()));
    let (actual, new_count) = count(|| after.fuzzy_candidates("zan'ma", options()));
    assert_fields_and_capacity(&actual, &expected);
    assert_eq!(new_count, old_count, "整条候选缓存命中保持原样");
}

#[test]
fn fuzzy_projection_keeps_empty_pages_owned_outputs_and_cache_capacities() {
    for rows in [0, 1, 8, 129] {
        let fixture = fixture(rows);
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        for typed in ["", "zan", "zan'hao", "zan'", "zan''hao", "x", "zan'hao'ma"] {
            for rules in [0, fuzzy_rule::Z_ZH, fuzzy_rule::ALL] {
                let options = FuzzyPinyinOptions { rules };
                let expected = before.legacy_fuzzy_candidates(typed, options);
                let mut actual = after.fuzzy_candidates(typed, options);
                assert_fields_and_capacity(&actual, &expected);
                assert_eq!(actual.capacity(), expected.capacity());
                assert!(actual
                    .iter()
                    .all(|item| item.fuzzy && item.source == CandidateSource::Database));
                if let Some(item) = actual.first_mut() {
                    item.word.push_str("修改副本");
                }
                let cached = after.fuzzy_candidates(typed, options);
                assert_eq!(cached, expected);
            }
        }
        let mut budget = FUZZY_PATH_BUDGET;
        let segments = split_segments("zan'hao");
        for length in (1..=segments.len()).rev() {
            let limit = FUZZY_SEGMENTATION_LIMIT.min(budget);
            let paths = fuzzy_segmentations(&segments[..length], options(), limit);
            budget -= paths.len();
            let typed = join_segments(&segments[..length]);
            let hash = fuzzy_row_cache_hash(options().rules, limit, &typed);
            let actual = after.fuzzy_row_cache.get_ref(&hash).unwrap();
            let expected = before.fuzzy_row_cache.get_ref(&hash).unwrap();
            assert_eq!(actual.rows, expected.rows);
            assert_eq!(actual.rows.capacity(), expected.rows.capacity());
        }
        let owned = after.fuzzy_candidates("zan'hao", options());
        after.reset_cache();
        drop(after);
        assert_eq!(owned, before.legacy_fuzzy_candidates("zan'hao", options()));
    }
}
