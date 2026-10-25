use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;

fn old_filtered(
    dictionary: &mut QuanpinDictionary,
    raw: &str,
    segmentation: &str,
    fuzzy: FuzzyPinyinOptions,
    mut keep: impl FnMut(&str, &str, &str, bool) -> bool,
) -> Vec<WordItem> {
    let mut rows = dictionary.query(raw, segmentation, 0, fuzzy);
    rows.retain(|row| keep(&row.word, &row.pinyin, &row.canonical_pinyin, row.fuzzy));
    rows
}

pub(super) fn assert_rows(actual: &Vec<WordItem>, expected: &Vec<WordItem>, allow_smaller: bool) {
    let capacity = |actual: usize, expected: usize| {
        if allow_smaller {
            assert!(actual <= expected, "容量 {actual}>{expected}");
        } else {
            assert_eq!(actual, expected);
        }
    };
    assert_eq!(actual, expected);
    capacity(actual.capacity(), expected.capacity());
    for (actual, expected) in actual.iter().zip(expected) {
        // `WordItem` 的文本字段是 `SharedString`。它们的 `String` 容量取决于
        // 是从缓存克隆还是新建行，不能再拿旧实现的容量作分配契约。
        capacity(
            actual.sentence_words.capacity(),
            expected.sentence_words.capacity(),
        );
    }
}

#[test]
fn filtered_hot_answers_do_not_clone_rejected_row_fields() {
    for n in [1, 4, 32, 128] {
        let fixture = Fixture::new();
        for index in 0..n {
            fixture.insert("ni", &format!("合成{index}😀"), index as i64);
        }
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        let fuzzy = FuzzyPinyinOptions { rules: 0 };
        let rows = before.query("ni", "ni", 0, fuzzy);
        assert_eq!(rows.len(), n);
        drop(after.query("ni", "ni", 0, fuzzy));
        for keep_count in [0, 1, n / 2, n] {
            let mut old_index = 0;
            let mut new_index = 0;
            let (expected, old_count) = count(|| {
                old_filtered(&mut before, "ni", "ni", fuzzy, |_, _, _, _| {
                    old_index += 1;
                    old_index <= keep_count
                })
            });
            let (actual, new_count) = count(|| {
                after.query_filtered("ni", "ni", fuzzy, |_, _, _, _| {
                    new_index += 1;
                    new_index <= keep_count
                })
            });
            assert_rows(&actual, &expected, false);
            assert_eq!((old_index, new_index), (n, n));
            let (_, rejected_copies) = count(|| rows[keep_count..].iter().cloned().for_each(drop));
            assert_eq!(
                new_count + rejected_copies,
                old_count,
                "行数={n}/保留={keep_count}"
            );
        }
    }
}

#[test]
fn filtered_cold_answers_do_not_clone_rejected_row_fields() {
    for n in [1, 4, 32, 128] {
        let fixture = Fixture::new();
        for index in 0..n {
            fixture.insert("ni", &format!("合成冷页{index}😀"), index as i64);
        }
        for keep_count in [0, 1, n / 2, n] {
            let mut before = QuanpinDictionary::new(&fixture.paths);
            let mut after = QuanpinDictionary::new(&fixture.paths);
            let fuzzy = FuzzyPinyinOptions { rules: 0 };
            let mut old_index = 0;
            let mut new_index = 0;
            let (expected, old_count) = count(|| {
                old_filtered(&mut before, "ni", "ni", fuzzy, |_, _, _, _| {
                    old_index += 1;
                    old_index <= keep_count
                })
            });
            let (actual, new_count) = count(|| {
                after.query_filtered("ni", "ni", fuzzy, |_, _, _, _| {
                    new_index += 1;
                    new_index <= keep_count
                })
            });
            assert_eq!(actual, expected);
            assert_eq!((old_index, new_index), (n, n));
            let all_rows = before.query("ni", "ni", 0, fuzzy);
            let (_, rejected_copies) =
                count(|| all_rows[keep_count..].iter().cloned().for_each(drop));
            assert_eq!(
                new_count + rejected_copies,
                old_count,
                "行数={n}/保留={keep_count}"
            );
            assert_eq!(after.query("ni", "ni", 0, fuzzy), all_rows);
        }
    }
}

#[test]
fn filtered_queries_preserve_cold_fuzzy_marking_and_cache_state() {
    let fixture = Fixture::new();
    for (key, word, weight) in [
        ("ni", "合成甲", 500),
        ("ni", "合成乙", -2),
        ("ni'hao", "合成补全", 100),
        ("xi", "合成西", 90),
        ("xi'an", "合成分段", 80),
        ("xian", "合成单段", 70),
        ("nve", "合成别名", 60),
        ("zi", "合成平舌", 50),
        ("zhi", "合成翘舌", 40),
        ("shang", "合成换位", 30),
    ] {
        fixture.insert(key, word, weight);
    }
    for (raw, segmentation) in [
        ("ni", "ni"),
        ("nihao", "ni'hao"),
        ("nihao", ""),
        ("xian", ""),
        ("nue", ""),
        ("zi", "zi"),
        ("sahng", ""),
        ("zzz", ""),
        ("!", ""),
        ("", ""),
    ] {
        for rules in [0, crate::types::fuzzy_rule::Z_ZH] {
            for predicate in 0..4 {
                let mut before = QuanpinDictionary::new(&fixture.paths);
                let mut after = QuanpinDictionary::new(&fixture.paths);
                let fuzzy = FuzzyPinyinOptions { rules };
                for phase in 0..4 {
                    if phase == 2 {
                        before.reset_cache();
                        after.reset_cache();
                    }
                    let keep =
                        |word: &str, reading: &str, canonical: &str, fuzzy: bool| match predicate {
                            0 => true,
                            1 => false,
                            2 => word.ends_with('甲') || canonical.contains('\''),
                            _ => !fuzzy && reading.len() <= 3,
                        };
                    let (expected, old_count) =
                        count(|| old_filtered(&mut before, raw, segmentation, fuzzy, keep));
                    let (mut actual, new_count) =
                        count(|| after.query_filtered(raw, segmentation, fuzzy, keep));
                    assert_rows(&actual, &expected, rules == 0 && phase % 2 == 0);
                    assert!(
                        new_count <= old_count + usize::from(rules == 0 && phase % 2 == 0),
                        "{raw}/{segmentation}/{rules}/{predicate}/{phase}: {new_count}>{old_count}"
                    );
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
                    // 修改本次拥有结果不能污染后续热槽。
                    for row in &mut actual {
                        row.word.clear();
                        row.pinyin.clear();
                        row.canonical_pinyin.clear();
                        row.sentence_words.clear();
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "完整入口配对计时，需在没有并行构建时手动运行"]
fn cold_filtered_query_paired_latency() {
    use std::hint::black_box;
    use std::time::{Duration, Instant};

    fn run(
        dictionary: &mut QuanpinDictionary,
        old: bool,
        key: &str,
        keep_count: usize,
        batch: usize,
    ) {
        for _ in 0..batch {
            dictionary.series_cache.clear();
            let mut index = 0;
            let keep = |_: &str, _: &str, _: &str, _: bool| {
                index += 1;
                index <= keep_count
            };
            let rows = if old {
                dictionary.before_cold_query_filtered(
                    key,
                    key,
                    FuzzyPinyinOptions { rules: 0 },
                    keep,
                )
            } else {
                dictionary.query_filtered(key, key, FuzzyPinyinOptions { rules: 0 }, keep)
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
            for keep_count in [0, total / 2, total] {
                let mut before = QuanpinDictionary::new(&fixture.paths);
                let mut after = QuanpinDictionary::new(&fixture.paths);
                run(&mut before, true, key, keep_count, 4);
                run(&mut after, false, key, keep_count, 4);
                let mut batch = 1;
                loop {
                    let start = Instant::now();
                    run(&mut before, true, key, keep_count, batch);
                    if start.elapsed() >= Duration::from_millis(10) {
                        break;
                    }
                    batch *= 2;
                }
                let mut ratios = Vec::new();
                for pair in 0..40 {
                    let mut elapsed = [Duration::ZERO; 2];
                    for arm in if pair % 2 == 0 { [0, 1] } else { [1, 0] } {
                        let start = Instant::now();
                        if arm == 0 {
                            run(&mut before, true, key, keep_count, batch);
                        } else {
                            run(&mut after, false, key, keep_count, batch);
                        }
                        elapsed[arm] = start.elapsed();
                    }
                    ratios.push(elapsed[1].as_secs_f64() / elapsed[0].as_secs_f64());
                }
                ratios.sort_by(f64::total_cmp);
                let ratio = (ratios[19] + ratios[20]) / 2.0;
                let budget = if n >= 32 && keep_count <= total / 2 {
                    0.98
                } else {
                    1.05
                };
                println!("cold key={key} n={n} keep={keep_count} batch={batch} ratio={ratio:.6} budget={budget}");
                if ratio >= budget {
                    failures.push((n, keep_count, ratio, budget));
                }
            }
        }
    }
    assert!(failures.is_empty(), "完整矩阵预算失败：{failures:?}");
}

#[test]
fn filtered_cold_pages_report_retained_capacity() {
    fn bytes(rows: &Vec<WordItem>) -> usize {
        rows.capacity() * std::mem::size_of::<WordItem>()
            + rows
                .iter()
                .map(|row| {
                    row.word.capacity()
                        + row.pinyin.capacity()
                        + row.canonical_pinyin.capacity()
                        + row.corrected_from.capacity()
                        + row.sentence_words.capacity() * std::mem::size_of::<String>()
                        + row
                            .sentence_words
                            .iter()
                            .map(String::capacity)
                            .sum::<usize>()
                })
                .sum::<usize>()
    }
    for key in ["ni", "ni'hao"] {
        for n in [1, 4, 32, 128] {
            let fixture = Fixture::new();
            for index in 0..n {
                fixture.insert(key, &format!("合成容量{index}😀"), index as i64);
            }
            if key.contains('\'') {
                // 短前缀组追加后，整页容器可能持有超过行数的容量。
                fixture.insert("ni", "合成前缀", 1);
                fixture.insert("hao", "合成尾部", 1);
            }
            let total = QuanpinDictionary::new(&fixture.paths)
                .query(key, key, 0, FuzzyPinyinOptions { rules: 0 })
                .len();
            for keep_count in [0, total / 2, total] {
                let mut before = QuanpinDictionary::new(&fixture.paths);
                let mut after = QuanpinDictionary::new(&fixture.paths);
                let fuzzy = FuzzyPinyinOptions { rules: 0 };
                let mut old_index = 0;
                let mut new_index = 0;
                let (expected, old_count) = count(|| {
                    old_filtered(&mut before, key, key, fuzzy, |_, _, _, _| {
                        old_index += 1;
                        old_index <= keep_count
                    })
                });
                let (actual, new_count) = count(|| {
                    after.query_filtered(key, key, fuzzy, |_, _, _, _| {
                        new_index += 1;
                        new_index <= keep_count
                    })
                });
                assert_eq!(actual, expected);
                let slot = before.series_slot(&series_cache_key(key, key, 0));
                let old_cache = before.series_cache.get_ref(&slot).unwrap();
                let new_cache = after.series_cache.get_ref(&slot).unwrap();
                assert_eq!(new_cache, old_cache);
                if keep_count == old_cache.len() {
                    assert_rows(&actual, &expected, false);
                }
                let old_total = bytes(&expected) + bytes(old_cache);
                let new_total = bytes(&actual) + bytes(new_cache);
                // 全保留时应沿用原页作为输出，避免为缓存另付缩容代价。
                let shrink = usize::from(
                    keep_count < old_cache.len() && expected.capacity() > old_cache.len(),
                );
                let (_, rejected_copies) =
                    count(|| old_cache[keep_count..].iter().cloned().for_each(drop));
                assert_eq!(new_count + rejected_copies, old_count + shrink);
                // 记录逻辑请求容量，不把它当作进程 RSS 或峰值。
                println!("retained key={key} n={n} keep={keep_count} cache={}/{} total={old_total}/{new_total}", bytes(old_cache), bytes(new_cache));
                assert!(new_total <= old_total);
                assert!(bytes(new_cache) <= bytes(old_cache));
            }
        }
    }
}

// 固定 a0560876cf 的完整筛选和查询核心；仅改名并移除注释。
impl QuanpinDictionary {
    fn before_cold_query_filtered(
        &mut self,
        raw: &str,
        segmentation: &str,
        fuzzy: FuzzyPinyinOptions,
        mut keep: impl FnMut(&str, &str, &str, bool) -> bool,
    ) -> Vec<WordItem> {
        if fuzzy.rules != 0 {
            let mut rows = self.query(raw, segmentation, 0, fuzzy);
            rows.retain(|row| keep(&row.word, &row.pinyin, &row.canonical_pinyin, row.fuzzy));
            return rows;
        }
        self.before_cold_query_exact_projected(
            raw,
            segmentation,
            0,
            |rows, primary, alternatives| {
                let mut result = match rows {
                    Cow::Owned(mut rows) => {
                        rows.retain(|row| {
                            keep(&row.word, &row.pinyin, &row.canonical_pinyin, row.fuzzy)
                        });
                        rows
                    }
                    Cow::Borrowed(rows) => {
                        let mut result = Vec::with_capacity(rows.len());
                        result.extend(
                            rows.iter()
                                .filter(|row| {
                                    keep(&row.word, &row.pinyin, &row.canonical_pinyin, row.fuzzy)
                                })
                                .cloned(),
                        );
                        result
                    }
                };
                mark_autocorrect_candidates(&mut result, raw, primary, alternatives);
                result
            },
        )
    }

    fn before_cold_query_exact_projected<R>(
        &mut self,
        raw: &str,
        segmentation: &str,
        types: u32,
        project: impl FnOnce(Cow<'_, [WordItem]>, &str, &[String]) -> R,
    ) -> R {
        if raw.is_empty() {
            return project(
                Cow::Owned(Vec::new()),
                &self.pinyin_segmentation,
                &self.alternative_segmentations,
            );
        }
        self.current_autocorrect_types = types;
        if types & TYPO_EDGE_TYPES != 0 {
            self.typo_profile.refresh_if_changed();
            let generation = self.typo_profile.generation();
            if generation != self.typo_generation {
                self.typo_generation = generation;
                self.series_cache.clear();
                self.personal_scored_keys.clear();
            }
        }
        let mut segments = None;
        let segmentation_cache = &mut self.segmentation_cache;
        let association = self.sentence_association.clone();
        let has_rerankers = !self.rerankers.is_empty();
        let context = &self.rescoring_context;
        let (resolution_hash, (resolution, pinyin_segmentation, marking_alternatives, slot)) =
            project_cached_resolution_with(
                &mut self.resolution_cache,
                raw,
                segmentation,
                types,
                || {
                    let segments = segments.get_or_insert_with(|| {
                        resolve_cached_segments(segmentation_cache, raw, segmentation)
                    });
                    resolve_series_query(raw, segments, types)
                },
                |resolution| {
                    let pinyin_segmentation = resolution.segmentation.clone();
                    let alternatives = resolution
                        .alternative_corrected_cuts
                        .iter()
                        .chain(&resolution.costlier_corrected_cuts)
                        .map(|cut| join_segments(cut))
                        .collect();
                    let slot = series_slot_key(
                        &resolution.cache_key,
                        &association,
                        has_rerankers,
                        context,
                    );
                    let owned = match resolution {
                        Cow::Borrowed(_) => None,
                        Cow::Owned(resolution) => Some(resolution),
                    };
                    (owned, pinyin_segmentation, alternatives, slot)
                },
            );
        self.pinyin_segmentation = pinyin_segmentation;
        self.alternative_segmentations = marking_alternatives;
        self.drop_personal_scored_results();
        if self.series_cache.contains(&slot) {
            if self.database.database_changed() {
                segments.get_or_insert_with(|| self.resolve_segments(raw, segmentation));
                self.reset_cache();
            }
            if let Some(cached) = self.series_cache.get_ref(&slot) {
                return project(
                    Cow::Borrowed(cached),
                    &self.pinyin_segmentation,
                    &self.alternative_segmentations,
                );
            }
        }

        let segments = segments.unwrap_or_else(|| self.resolve_segments(raw, segmentation));

        let resolution = resolution.unwrap_or_else(|| {
            self.resolution_cache
                .get_ref(&resolution_hash)
                .expect("候选失效检查必须保留解析备忘")
                .resolution
                .clone()
        });

        self.validate_row_caches();
        let alternatives = alternative_segmentations(raw, &segments, &resolution, types);
        let primary_segmentation = resolution.segmentation.as_str();
        let mut result;
        if resolution.corrected_input {
            result = self.query_series(raw, primary_segmentation, &resolution.corrected);
            let literal = if segmentation.is_empty() {
                let literal_segmentation = join_segments(&segments);
                self.query_series(raw, &literal_segmentation, &segments)
            } else {
                self.query_series(raw, segmentation, &segments)
            };
            append_unique_words(&mut result, literal);
            if !alternatives.is_empty() {
                result = self.merge_alternatives(
                    raw,
                    primary_segmentation,
                    &resolution.corrected,
                    &alternatives,
                    result,
                );
            }
            for costlier in &resolution.costlier_corrected_cuts {
                let rows = self.query_series(raw, &join_segments(costlier), costlier);
                append_unique_words(&mut result, rows);
            }
        } else {
            result = self.query_series(raw, primary_segmentation, &segments);
            if !alternatives.is_empty() {
                result = self.merge_alternatives(
                    raw,
                    primary_segmentation,
                    &segments,
                    &alternatives,
                    result,
                );
            }
        }
        // 保留原分配对照，只同步备选系列新增的个人模型依赖。
        if segments.len() >= 2
            || resolution.corrected.len() >= 2
            || alternatives
                .iter()
                .any(|path| path.len() >= 2 && is_literal_segmentation(raw, path))
        {
            self.series_cache.insert(slot.clone(), result.clone());
            self.remember_personal_scored_key(slot);
        } else {
            self.series_cache.insert(slot, result.clone());
        }
        project(
            Cow::Owned(result),
            &self.pinyin_segmentation,
            &self.alternative_segmentations,
        )
    }
}

#[test]
fn cold_rank_filter_preserves_generated_row_capacity() {
    let fixture = Fixture::new();
    fixture.insert("ni", "你", 100);
    fixture.insert("ni'hao", "你好", 1000);
    fixture.insert("ma", "吗", 100);
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    let fuzzy = FuzzyPinyinOptions { rules: 0 };
    let mut expected = before.query_filtered("nihaoma", "ni'hao'ma", fuzzy, |_, _, _, _| true);
    let generated = expected
        .iter()
        .find(|row| row.source == CandidateSource::Generated)
        .expect("合成词网格应产生整句");
    assert!(generated.sentence_words.capacity() > generated.sentence_words.len());
    expected.retain(|row| row.word != "你");
    let mut actual = after.query_filtered_rows("nihaoma", "ni'hao'ma", fuzzy, |row, cached| {
        !cached || row.word != "你"
    });
    actual.retain(|row| row.word != "你");
    assert_rows(&actual, &expected, false);
}

#[test]
fn rank_filter_hot_flag_follows_actual_valid_series_hits() {
    let fixture = Fixture::new();
    fixture.insert("ni", "你", 100);
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    let fuzzy = FuzzyPinyinOptions { rules: 0 };
    let check = |dictionary: &mut QuanpinDictionary, fuzzy, expected| {
        let mut visited = 0;
        let rows = dictionary.query_filtered_rows("ni", "ni", fuzzy, |_, cached| {
            assert_eq!(cached, expected);
            visited += 1;
            true
        });
        assert!(visited > 0);
        assert!(!rows.is_empty());
    };
    check(&mut dictionary, fuzzy, false);
    check(&mut dictionary, fuzzy, true);
    fixture.insert("ni", "合成新增", 200);
    check(&mut dictionary, fuzzy, false);
    check(&mut dictionary, fuzzy, true);
    dictionary.reset_cache();
    check(&mut dictionary, fuzzy, false);
    check(
        &mut dictionary,
        FuzzyPinyinOptions {
            rules: crate::types::fuzzy_rule::Z_ZH,
        },
        false,
    );
}
