use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;

// 冻结 5d5095eb350e249cd115958bc816854536eea8b1 的完整筛选入口，仅改名并移除注释。
impl QuanpinDictionary {
    fn before_empty_query_filtered_rows(
        &mut self,
        raw: &str,
        segmentation: &str,
        fuzzy: FuzzyPinyinOptions,
        mut keep: impl FnMut(&WordItem, bool) -> bool,
    ) -> Vec<WordItem> {
        if fuzzy.rules != 0 {
            let mut rows = self.query(raw, segmentation, 0, fuzzy);
            rows.retain(|row| keep(row, false));
            return rows;
        }
        let mut result =
            self.query_exact_projected(raw, segmentation, 0, ColdRows::Borrow, |rows, cached| {
                match rows {
                    Cow::Owned(mut rows) => {
                        rows.retain(|row| keep(row, cached));
                        rows
                    }
                    Cow::Borrowed(rows) => {
                        let mut result = Vec::with_capacity(rows.len());
                        result.extend(rows.iter().filter(|row| keep(row, cached)).cloned());
                        result
                    }
                }
            });
        mark_autocorrect_candidates(
            &mut result,
            raw,
            &self.pinyin_segmentation,
            &self.alternative_segmentations,
        );
        result
    }
}

// 未通过性能门槛的候选只保留在测试中，生产入口继续沿用完整页预留。
impl QuanpinDictionary {
    fn after_empty_query_filtered_rows(
        &mut self,
        raw: &str,
        segmentation: &str,
        fuzzy: FuzzyPinyinOptions,
        mut keep: impl FnMut(&WordItem, bool) -> bool,
    ) -> Vec<WordItem> {
        if fuzzy.rules != 0 {
            // 模糊合并的去重和站位必须先完成，避免被筛掉的词让另一条同文行复活。
            let mut rows = self.query(raw, segmentation, 0, fuzzy);
            rows.retain(|row| keep(row, false));
            return rows;
        }
        let mut result =
            self.query_exact_projected(raw, segmentation, 0, ColdRows::Borrow, |rows, cached| {
                match rows {
                    Cow::Owned(mut rows) => {
                        rows.retain(|row| keep(row, cached));
                        rows
                    }
                    Cow::Borrowed(rows) => {
                        let mut accepted = rows.iter().filter(|row| keep(row, cached));
                        let Some(first) = accepted.next() else {
                            return Vec::new();
                        };
                        // 非空页保留完整行数容量；全拒绝时无需申请结果页。
                        let mut result = Vec::with_capacity(rows.len());
                        result.push(first.clone());
                        result.extend(accepted.cloned());
                        result
                    }
                }
            });
        mark_autocorrect_candidates(
            &mut result,
            raw,
            &self.pinyin_segmentation,
            &self.alternative_segmentations,
        );
        result
    }
}

#[test]
fn empty_projection_skips_exactly_one_hot_page_allocation() {
    for n in [1, 4, 32, 128] {
        let fixture = Fixture::new();
        for index in 0..n {
            fixture.insert("ni", &format!("合成空投影{index}😀"), index as i64);
        }
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        let fuzzy = FuzzyPinyinOptions { rules: 0 };
        drop(before.query("ni", "ni", 0, fuzzy));
        drop(after.query("ni", "ni", 0, fuzzy));
        let (expected, old_count) = count(|| {
            before.before_empty_query_filtered_rows("ni", "ni", fuzzy, |_, cached| {
                assert!(cached);
                false
            })
        });
        let (actual, new_count) = count(|| {
            after.after_empty_query_filtered_rows("ni", "ni", fuzzy, |_, cached| {
                assert!(cached);
                false
            })
        });
        assert!(actual.is_empty() && expected.is_empty());
        assert_eq!(actual.capacity(), 0, "行数={n}");
        assert_eq!(new_count + 1, old_count, "行数={n}");
    }
}

#[test]
fn empty_projection_preserves_stateful_visits_rows_and_cache_storage() {
    for n in [0, 1, 4, 32, 128] {
        let fixture = Fixture::new();
        fixture.table("ni");
        for index in 0..n {
            fixture.insert("ni", &format!("合成顺序{index}😀"), index as i64);
        }
        fixture.insert("ni'hao", "合成补全", 10);
        fixture.insert("zi", "合成平舌", 20);
        fixture.insert("zhi", "合成翘舌", 30);
        fixture.insert("nve", "合成别名", 40);
        fixture.insert("shang", "合成纠错", 50);
        for (raw, segmentation) in [
            ("ni", "ni"),
            ("zi", "zi"),
            ("nue", ""),
            ("sahng", ""),
            ("!", ""),
            ("", ""),
        ] {
            for rules in [0, crate::types::fuzzy_rule::Z_ZH] {
                let total = QuanpinDictionary::new(&fixture.paths)
                    .query(raw, segmentation, 0, FuzzyPinyinOptions { rules })
                    .len();
                for mode in 0..5 {
                    let mut before = QuanpinDictionary::new(&fixture.paths);
                    let mut after = QuanpinDictionary::new(&fixture.paths);
                    let fuzzy = FuzzyPinyinOptions { rules };
                    for phase in 0..4 {
                        if phase == 2 {
                            before.reset_cache();
                            after.reset_cache();
                        }
                        let mut old_visits = Vec::new();
                        let mut new_visits = Vec::new();
                        let keep = |visits: &mut Vec<(String, bool)>, row: &WordItem, cached| {
                            visits.push((row.word.to_string(), cached));
                            match mode {
                                0 => false,
                                1 => visits.len() == 1,
                                2 => visits.len() == total,
                                3 => visits.len().is_multiple_of(2),
                                _ => true,
                            }
                        };
                        let expected = before.before_empty_query_filtered_rows(
                            raw,
                            segmentation,
                            fuzzy,
                            |row, cached| keep(&mut old_visits, row, cached),
                        );
                        let actual = after.after_empty_query_filtered_rows(
                            raw,
                            segmentation,
                            fuzzy,
                            |row, cached| keep(&mut new_visits, row, cached),
                        );
                        assert_eq!(new_visits, old_visits);
                        if rules == 0 && actual.is_empty() {
                            assert_eq!(actual, expected);
                            assert_eq!(actual.capacity(), 0);
                        } else {
                            super::filtered_query_tests::assert_rows(&actual, &expected, false);
                        }
                        assert_eq!(after.pinyin_segmentation, before.pinyin_segmentation);
                        assert_eq!(
                            after.alternative_segmentations,
                            before.alternative_segmentations
                        );
                        assert_eq!(after.personal_scored_keys, before.personal_scored_keys);
                        assert_eq!(after.personal_version, before.personal_version);
                        assert_eq!(after.typo_generation, before.typo_generation);
                        assert_eq!(
                            after.current_autocorrect_types,
                            before.current_autocorrect_types
                        );
                        if !raw.is_empty() {
                            let slot = before.series_slot(&series_cache_key(raw, segmentation, 0));
                            match (
                                before.series_cache.get_ref(&slot),
                                after.series_cache.get_ref(&slot),
                            ) {
                                (Some(expected), Some(actual)) => {
                                    super::filtered_query_tests::assert_rows(
                                        actual, expected, false,
                                    )
                                }
                                (None, None) => {}
                                _ => panic!("缓存槽存在性不一致"),
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn empty_projection_reads_keep_fifo_age_and_database_invalidation() {
    let fixture = Fixture::new();
    for key in ["ni", "hao", "ma"] {
        fixture.insert(key, "合成缓存", 10);
    }
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    let fuzzy = FuzzyPinyinOptions { rules: 0 };
    before.series_cache = FifoCache::new(2);
    after.series_cache = FifoCache::new(2);
    for key in ["ni", "hao", "ni", "ma"] {
        let expected = before.before_empty_query_filtered_rows(key, key, fuzzy, |_, _| false);
        let actual = after.after_empty_query_filtered_rows(key, key, fuzzy, |_, _| false);
        assert_eq!(actual, expected);
        assert_eq!(actual.capacity(), 0);
    }
    for dictionary in [&before, &after] {
        assert!(dictionary
            .series_cache
            .get_ref(&dictionary.series_slot(&series_cache_key("ni", "ni", 0)))
            .is_none());
        assert!(dictionary
            .series_cache
            .get_ref(&dictionary.series_slot(&series_cache_key("hao", "hao", 0)))
            .is_some());
        assert!(dictionary
            .series_cache
            .get_ref(&dictionary.series_slot(&series_cache_key("ma", "ma", 0)))
            .is_some());
    }
    fixture.insert("hao", "合成新增", 100);
    let mut old_flags = Vec::new();
    let mut new_flags = Vec::new();
    let expected = before.before_empty_query_filtered_rows("hao", "hao", fuzzy, |_, cached| {
        old_flags.push(cached);
        true
    });
    let actual = after.after_empty_query_filtered_rows("hao", "hao", fuzzy, |_, cached| {
        new_flags.push(cached);
        true
    });
    assert_eq!(new_flags, old_flags);
    assert!(!new_flags.is_empty() && new_flags.iter().all(|cached| !cached));
    super::filtered_query_tests::assert_rows(&actual, &expected, false);
    assert!(actual.iter().any(|row| row.word == "合成新增"));
}

#[test]
#[ignore = "完整入口配对计时，需使用普通分配器且没有并行构建"]
fn empty_projection_paired_latency() {
    use std::hint::black_box;
    use std::time::{Duration, Instant};

    fn run(
        dictionary: &mut QuanpinDictionary,
        old: bool,
        key: &str,
        mode: usize,
        total: usize,
        cold: bool,
        batch: usize,
    ) {
        for _ in 0..batch {
            if cold {
                dictionary.series_cache.clear();
            }
            let mut index = 0;
            let keep = |_: &WordItem, _: bool| {
                index += 1;
                match mode {
                    0 => false,
                    1 => index == total,
                    _ => true,
                }
            };
            let fuzzy = FuzzyPinyinOptions { rules: 0 };
            let rows = if old {
                dictionary.before_empty_query_filtered_rows(key, key, fuzzy, keep)
            } else {
                dictionary.after_empty_query_filtered_rows(key, key, fuzzy, keep)
            };
            black_box(&rows);
            drop(rows);
        }
    }

    let mut failures = Vec::new();
    for key in ["ni", "ni'hao"] {
        for n in [1, 4, 32, 128] {
            let fixture = Fixture::new();
            for index in 0..n {
                fixture.insert(key, &format!("合成配对{index}😀"), index as i64);
            }
            if key.contains('\'') {
                fixture.insert("ni", "合成前缀", 1);
                fixture.insert("hao", "合成尾部", 1);
            }
            let total = QuanpinDictionary::new(&fixture.paths)
                .query(key, key, 0, FuzzyPinyinOptions { rules: 0 })
                .len();
            for cold in [false, true] {
                for mode in 0..3 {
                    let mut before = QuanpinDictionary::new(&fixture.paths);
                    let mut after = QuanpinDictionary::new(&fixture.paths);
                    run(&mut before, true, key, mode, total, cold, 4);
                    run(&mut after, false, key, mode, total, cold, 4);
                    let mut batch = 1;
                    loop {
                        let started = Instant::now();
                        run(&mut before, true, key, mode, total, cold, batch);
                        if started.elapsed() >= Duration::from_millis(10) {
                            break;
                        }
                        batch *= 2;
                    }
                    let mut ratios = Vec::new();
                    for pair in 0..40 {
                        let mut elapsed = [Duration::ZERO; 2];
                        for arm in if pair % 2 == 0 { [0, 1] } else { [1, 0] } {
                            let started = Instant::now();
                            if arm == 0 {
                                run(&mut before, true, key, mode, total, cold, batch);
                            } else {
                                run(&mut after, false, key, mode, total, cold, batch);
                            }
                            elapsed[arm] = started.elapsed();
                        }
                        ratios.push(elapsed[1].as_secs_f64() / elapsed[0].as_secs_f64());
                        println!("pair key={key} n={n} cold={cold} mode={mode} pair={pair} batch={batch} old_ns={} new_ns={}", elapsed[0].as_nanos(), elapsed[1].as_nanos());
                    }
                    ratios.sort_by(f64::total_cmp);
                    let ratio = (ratios[19] + ratios[20]) / 2.0;
                    let budget = if !cold && n >= 32 && mode == 0 {
                        0.98
                    } else {
                        1.05
                    };
                    println!("cell key={key} n={n} cold={cold} mode={mode} ratio={ratio:.6} budget={budget}");
                    if ratio >= budget {
                        failures.push((key, n, cold, mode, ratio, budget));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "完整矩阵预算失败：{failures:?}");
}
