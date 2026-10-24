use super::filtered_query_tests::assert_rows;
use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;

// 冻结 6fa568ce12fa7d6c129fce7c4428eb4e8a1717d1 的完整入口与投影核心，仅改名并移除注释。
impl QuanpinDictionary {
    fn before_recycle_query_filtered_rows(
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
        let mut result = self.before_recycle_query_exact_projected(
            raw,
            segmentation,
            0,
            ColdRows::Borrow,
            |rows, cached| match rows {
                Cow::Owned(mut rows) => {
                    rows.retain(|row| keep(row, cached));
                    rows
                }
                Cow::Borrowed(rows) => {
                    let mut result = Vec::with_capacity(rows.len());
                    result.extend(rows.iter().filter(|row| keep(row, cached)).cloned());
                    result
                }
            },
        );
        mark_autocorrect_candidates(
            &mut result,
            raw,
            &self.pinyin_segmentation,
            &self.alternative_segmentations,
        );
        result
    }

    fn before_recycle_query_exact_projected(
        &mut self,
        raw: &str,
        segmentation: &str,
        types: u32,
        cold: ColdRows,
        project: impl FnOnce(Cow<'_, [WordItem]>, bool) -> Vec<WordItem>,
    ) -> Vec<WordItem> {
        if raw.is_empty() {
            return project(Cow::Owned(Vec::new()), false);
        }
        self.current_autocorrect_types = types;
        if types & TYPO_EDGE_TYPES != 0 {
            self.typo_profile.refresh_if_changed();
            let generation = self.typo_profile.generation();
            if generation != self.typo_generation {
                self.typo_generation = generation;
                self.series_cache.clear();
                self.sentence_decode_cache.clear();
                self.personal_scored_keys.clear();
            }
        }
        let mut segments = None;
        let segmentation_cache = &mut self.segmentation_cache;
        let association = self.sentence_association.clone();
        let has_rerankers = !self.rerankers.is_empty();
        let context = &self.rescoring_context;
        let pinyin_segmentation = &mut self.pinyin_segmentation;
        let (resolution_hash, (resolution, marking_alternatives, slot)) =
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
                    pinyin_segmentation.clone_from(&resolution.segmentation);
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
                    (owned, alternatives, slot)
                },
            );
        self.alternative_segmentations = marking_alternatives;
        self.drop_personal_scored_results();
        if self.series_cache.contains(&slot) {
            if self.database.database_changed() {
                segments.get_or_insert_with(|| self.resolve_segments(raw, segmentation));
                self.reset_cache();
            }
            if let Some(cached) = self.series_cache.get_ref(&slot) {
                return project(Cow::Borrowed(cached), true);
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
        let (cached, projected) = match cold {
            ColdRows::Transfer => {
                let cached = result.clone();
                (cached, project(Cow::Owned(result), false))
            }
            ColdRows::Borrow => {
                let projected = project(Cow::Borrowed(&result), false);
                if projected.len() == result.len() {
                    (projected, result)
                } else {
                    result.shrink_to_fit();
                    (result, projected)
                }
            }
        };
        if segments.len() >= 2
            || resolution.corrected.len() >= 2
            || alternatives
                .iter()
                .any(|path| path.len() >= 2 && is_literal_segmentation(raw, path))
        {
            self.series_cache.insert(slot.clone(), cached);
            self.remember_personal_scored_key(slot);
        } else {
            self.series_cache.insert(slot, cached);
        }
        projected
    }
}

fn slot(index: usize) -> String {
    format!("合成回收槽{index:03}")
}

fn prime(dictionary: &mut QuanpinDictionary, rows: &[WordItem], full: bool) {
    dictionary.series_cache.clear();
    for index in 0..if full {
        CACHE_CAPACITY
    } else {
        CACHE_CAPACITY - 1
    } {
        dictionary.series_cache.insert(slot(index), rows.to_vec());
    }
}

#[test]
fn full_cold_projection_reuses_exactly_the_evicted_clone_allocations() {
    for n in [1, 4, 32, 128] {
        let fixture = Fixture::new();
        for index in 0..n {
            fixture.insert("ni", &format!("合成回收{index:03}😀"), index as i64);
        }
        for full in [false, true] {
            let mut before = QuanpinDictionary::new(&fixture.paths);
            let mut after = QuanpinDictionary::new(&fixture.paths);
            let fuzzy = FuzzyPinyinOptions { rules: 0 };
            let rows = before.query("ni", "ni", 0, fuzzy);
            drop(after.query("ni", "ni", 0, fuzzy));
            assert_eq!(rows.len(), n);
            prime(&mut before, &rows, full);
            prime(&mut after, &rows, full);
            let (_, clone_count) = count(|| rows.to_vec());
            let (expected, old_count) = count(|| {
                before.before_recycle_query_filtered_rows("ni", "ni", fuzzy, |_, cached| {
                    assert!(!cached);
                    true
                })
            });
            let (actual, new_count) = count(|| {
                after.query_filtered_rows("ni", "ni", fuzzy, |_, cached| {
                    assert!(!cached);
                    true
                })
            });
            assert_rows(&actual, &expected, false);
            let key = before.series_slot(&series_cache_key("ni", "ni", 0));
            assert_rows(
                after.series_cache.get_ref(&key).unwrap(),
                before.series_cache.get_ref(&key).unwrap(),
                false,
            );
            assert_eq!(after.series_cache.contains(&slot(0)), !full);
            assert_eq!(
                new_count + if full { clone_count } else { 0 },
                old_count,
                "n={n} full={full} clone={clone_count}"
            );
            println!(
                "recycle n={n} full={full} old={old_count} new={new_count} clone={clone_count}"
            );
        }
    }
}

#[test]
fn recycled_projection_preserves_filters_marking_storage_and_fifo() {
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
        ("xian", ""),
        ("nue", ""),
        ("zi", "zi"),
        ("sahng", ""),
        ("!", ""),
        ("", ""),
    ] {
        for rules in [0, crate::types::fuzzy_rule::Z_ZH] {
            for mode in 0..5 {
                let mut before = QuanpinDictionary::new(&fixture.paths);
                let mut after = QuanpinDictionary::new(&fixture.paths);
                let fuzzy = FuzzyPinyinOptions { rules };
                let seed = before.query("ni", "ni", 0, fuzzy);
                drop(after.query("ni", "ni", 0, fuzzy));
                prime(&mut before, &seed, true);
                prime(&mut after, &seed, true);
                for phase in 0..3 {
                    if phase == 2 {
                        before.reset_cache();
                        after.reset_cache();
                    }
                    let mut old_visits = Vec::new();
                    let mut new_visits = Vec::new();
                    let keep = |visits: &mut Vec<(WordItem, bool)>, row: &WordItem, cached| {
                        visits.push((row.clone(), cached));
                        match mode {
                            0 => true,
                            1 => false,
                            2 => visits.len() == 1,
                            3 => visits.len().is_multiple_of(2),
                            _ => row.canonical_pinyin.contains('\''),
                        }
                    };
                    let mut expected = before.before_recycle_query_filtered_rows(
                        raw,
                        segmentation,
                        fuzzy,
                        |row, cached| keep(&mut old_visits, row, cached),
                    );
                    let mut actual =
                        after.query_filtered_rows(raw, segmentation, fuzzy, |row, cached| {
                            keep(&mut new_visits, row, cached)
                        });
                    assert_eq!(new_visits, old_visits, "{raw}/{rules}/{mode}/{phase}");
                    assert_rows(&actual, &expected, false);
                    for index in 0..CACHE_CAPACITY {
                        let key = slot(index);
                        match (
                            after.series_cache.get_ref(&key),
                            before.series_cache.get_ref(&key),
                        ) {
                            (Some(actual), Some(expected)) => assert_rows(actual, expected, false),
                            (None, None) => {}
                            _ => panic!("FIFO 槽不一致：{raw}/{rules}/{mode}/{phase}/{index}"),
                        }
                    }
                    if !raw.is_empty() {
                        let segments = before.resolve_segments(raw, segmentation);
                        let resolution = resolve_series_query(raw, &segments, 0);
                        let key = before.series_slot(&resolution.cache_key);
                        assert_rows(
                            after.series_cache.get_ref(&key).unwrap(),
                            before.series_cache.get_ref(&key).unwrap(),
                            false,
                        );
                    }
                    // 返回结果可以独立改写，下一次热查询仍读取完整原页。
                    for rows in [&mut actual, &mut expected] {
                        if let Some(row) = rows.first_mut() {
                            row.word.push_str("合成独立改写");
                            row.sentence_words.clear();
                        }
                    }
                }
                before
                    .series_cache
                    .insert("合成后续槽".into(), seed.clone());
                after.series_cache.insert("合成后续槽".into(), seed);
                for index in 0..CACHE_CAPACITY {
                    assert_eq!(
                        after.series_cache.contains(&slot(index)),
                        before.series_cache.contains(&slot(index))
                    );
                }
            }
        }
    }
}

#[test]
fn cold_predicate_panic_keeps_fifo_pages_and_their_age() {
    let fixture = Fixture::new();
    for index in 0..4 {
        fixture.insert("ni", &format!("合成异常{index}"), index);
    }
    for panic_at in 0..4 {
        let mut dictionary = QuanpinDictionary::new(&fixture.paths);
        let fuzzy = FuzzyPinyinOptions { rules: 0 };
        let rows = dictionary.query("ni", "ni", 0, fuzzy);
        prime(&mut dictionary, &rows, true);
        let mut visits = 0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dictionary.query_filtered_rows("ni", "ni", fuzzy, |_, _| {
                assert_ne!(visits, panic_at, "合成谓词异常");
                visits += 1;
                true
            })
        }));
        assert!(result.is_err());
        for index in 0..CACHE_CAPACITY {
            assert_rows(
                dictionary.series_cache.get_ref(&slot(index)).unwrap(),
                &rows,
                false,
            );
        }
        let key = dictionary.series_slot(&series_cache_key("ni", "ni", 0));
        assert!(!dictionary.series_cache.contains(&key));
        drop(dictionary.query_filtered_rows("ni", "ni", fuzzy, |_, _| true));
        assert!(!dictionary.series_cache.contains(&slot(0)));
        assert!(dictionary.series_cache.contains(&slot(1)));
    }
}

#[test]
fn recycled_pages_follow_database_invalidation() {
    let fixture = Fixture::new();
    fixture.insert("ni", "合成旧行", 1);
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    let fuzzy = FuzzyPinyinOptions { rules: 0 };
    let rows = before.query("ni", "ni", 0, fuzzy);
    drop(after.query("ni", "ni", 0, fuzzy));
    prime(&mut before, &rows, true);
    prime(&mut after, &rows, true);
    for phase in 0..3 {
        if phase == 1 {
            fixture.insert("ni", "合成更新行", 2);
        }
        let expected = before.before_recycle_query_filtered_rows("ni", "ni", fuzzy, |_, _| true);
        let actual = after.query_filtered_rows("ni", "ni", fuzzy, |_, _| true);
        assert_rows(&actual, &expected, false);
        assert_eq!(actual.len(), if phase == 0 { 1 } else { 2 });
    }
}

#[test]
fn page_copy_preserves_all_fields_and_clone_capacities_across_shapes() {
    use crate::types::SchemeType;

    for new_len in [0, 1, 4, 32] {
        for old_len in [0, 1, 4, 64] {
            for spare in [0, 1, 64] {
                let make = |index| WordItem {
                    pinyin: "ni'hao".into(),
                    canonical_pinyin: "ni'hao'ma".into(),
                    word: format!("合成字段{index:03}😀"),
                    weight: i64::MIN + index as i64,
                    source: CandidateSource::Generated,
                    scheme: SchemeType::Shuangpin,
                    fixed_position: index as i32,
                    fuzzy: index % 2 == 0,
                    corrected_from: "合成纠错".into(),
                    sentence_association: true,
                    sentence_words: vec!["合成边界一".into(), "合成边界二😀".into()],
                };
                let mut source: Vec<_> = (0..new_len).map(make).collect();
                for row in &mut source {
                    row.pinyin.reserve(64);
                    row.word.reserve(64);
                    row.sentence_words.reserve(4);
                    row.sentence_words[0].reserve(64);
                }
                let expected = source.clone();
                let mut previous = Vec::with_capacity(old_len + spare);
                for index in 0..old_len {
                    let mut row = make(index + 100);
                    row.pinyin = "合成旧音".into();
                    row.word.reserve(spare);
                    row.weight = i64::MAX;
                    row.source = CandidateSource::Database;
                    row.scheme = SchemeType::Quanpin;
                    row.fixed_position = -1;
                    row.fuzzy = !row.fuzzy;
                    row.corrected_from.clear();
                    row.sentence_association = false;
                    row.sentence_words.push("合成旧边界".into());
                    previous.push(row);
                }
                let actual = page_copy::clone_rows_reusing(&source, previous);
                assert_rows(&actual, &expected, false);
                if let Some(row) = source.first_mut() {
                    row.word.clear();
                    row.sentence_words.clear();
                    assert_rows(&actual, &expected, false);
                }
            }
        }
    }
}

#[test]
fn exact_page_copy_reuses_outer_and_nested_buffers_without_allocating() {
    let mut row = WordItem::new("ni", "合成新值😀", -7, CandidateSource::Generated, "ni'hao");
    row.corrected_from = "合成新码".into();
    row.sentence_association = true;
    row.sentence_words = vec!["合成新段".into(), "合成边界😀".into()];
    let source = vec![row];
    let mut previous = source.clone();
    previous[0].word = "合成旧值😀".into();
    previous[0].sentence_words[0] = "合成旧段".into();
    let outer = previous.as_ptr();
    let word = previous[0].word.as_ptr();
    let words = previous[0].sentence_words.as_ptr();
    let inner = previous[0].sentence_words[0].as_ptr();
    let (actual, allocations) = count(|| page_copy::clone_rows_reusing(&source, previous));
    assert_eq!(allocations, 0);
    assert_rows(&actual, &source, false);
    assert_eq!(actual.as_ptr(), outer);
    assert_eq!(actual[0].word.as_ptr(), word);
    assert_eq!(actual[0].sentence_words.as_ptr(), words);
    assert_eq!(actual[0].sentence_words[0].as_ptr(), inner);
}
