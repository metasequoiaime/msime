use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;

// 固定 0ded5ef2d213ea733a6f96885d21210424c71050 的完整查询，仅改名、转接旧系列方法并移除注释。
impl QuanpinDictionary {
    fn legacy_prefix_query_exact(
        &mut self,
        raw: &str,
        segmentation: &str,
        types: u32,
    ) -> Vec<WordItem> {
        if raw.is_empty() {
            return Vec::new();
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
        let association = self.sentence_association;
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
                    let slot =
                        series_slot_key(&resolution.cache_key, association, has_rerankers, context);
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
            if let Some(cached) = self.series_cache.get(&slot) {
                return cached;
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
            result =
                self.legacy_prefix_query_series(raw, primary_segmentation, &resolution.corrected);
            let literal = if segmentation.is_empty() {
                let literal_segmentation = join_segments(&segments);
                self.legacy_prefix_query_series(raw, &literal_segmentation, &segments)
            } else {
                self.legacy_prefix_query_series(raw, segmentation, &segments)
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
                let rows = self.legacy_prefix_query_series(raw, &join_segments(costlier), costlier);
                append_unique_words(&mut result, rows);
            }
        } else {
            result = self.legacy_prefix_query_series(raw, primary_segmentation, &segments);
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
        if segments.len() >= 2 || resolution.corrected.len() >= 2 {
            self.series_cache.insert(slot.clone(), result.clone());
            self.remember_personal_scored_key(slot);
        } else {
            self.series_cache.insert(slot, result.clone());
        }
        result
    }

    fn legacy_prefix_query_series(
        &mut self,
        raw: &str,
        segmentation: &str,
        segments: &[String],
    ) -> Vec<WordItem> {
        if segments.is_empty() {
            return self.query_single_path(raw, segmentation, segments);
        }
        let mut result = Vec::new();
        let mut partial_segmentation = join_segments(segments);
        for count in (1..=segments.len()).rev() {
            if count < segments.len() {
                truncate_last_segment(&mut partial_segmentation);
            }
            let partial = &segments[..count];
            let mut rows = self.query_single_path(raw, &partial_segmentation, partial);
            if count == segments.len() {
                let exact_rows = rows
                    .iter()
                    .filter(|item| item.canonical_pinyin == partial_segmentation)
                    .count();
                let longer = self.longer_phrase_candidates(&partial_segmentation, partial);
                if !longer.is_empty() {
                    if exact_rows < self.first_page_rows.get() {
                        rows.sort_by_key(|item| std::cmp::Reverse(item.weight));
                        let lead = usize::from(
                            rows.first()
                                .is_some_and(|item| item.canonical_pinyin == partial_segmentation),
                        );
                        rows.extend(longer);
                        rows[lead..].sort_by_key(|item| std::cmp::Reverse(item.weight));
                    } else {
                        rows.extend(longer);
                    }
                }
            }
            Self::append_query_rows(&mut result, rows);
        }

        if segments.len() >= 2 && has_only_complete_pinyin_segments(segments) {
            self.merge_sentences(&mut result, raw, segmentation, segments);
        }

        if result.len() < SPARSE_FALLBACK_THRESHOLD {
            for fallback in sparse_pinyin_fallback_segments(segments) {
                if fallback.is_empty() {
                    continue;
                }
                let fallback_segmentation = join_segments(&fallback);
                let rows = self.query_single_path(raw, &fallback_segmentation, &fallback);
                append_unique_words(&mut result, rows);
            }
        }
        result
    }
}

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    for (key, word) in [
        ("ni", "合成单音节😀"),
        ("ni'hao", "合成双音节"),
        ("ni'hao'ma", "合成长词"),
        ("nve", "合成别名"),
        ("shang", "合成换位"),
        ("gua", "合成低成本"),
        ("gai", "合成高成本"),
        ("quan'li", "合成删首"),
    ] {
        fixture.insert(key, word, 100);
    }
    fixture
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

fn assert_state(before: &QuanpinDictionary, after: &QuanpinDictionary) {
    assert_eq!(after.pinyin_segmentation, before.pinyin_segmentation);
    assert_eq!(
        after.pinyin_segmentation.capacity(),
        before.pinyin_segmentation.capacity()
    );
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
}

fn joined_key_saving(segmentation: &str, segments: &[String]) -> usize {
    let joined = join_segments(segments);
    usize::from(!segments.is_empty() && !joined.is_empty() && joined == segmentation)
}

// 从实际解析的调用参数推导次数，核对在测量区外拼接，避免把判定成本混进查询。
fn exact_saving(
    dictionary: &mut QuanpinDictionary,
    raw: &str,
    segmentation: &str,
    types: u32,
) -> usize {
    if raw.is_empty() {
        return 0;
    }
    let segments = dictionary.resolve_segments(raw, segmentation);
    let resolution = &dictionary
        .resolution_cache
        .get_ref(&resolution_cache_hash(types, raw, segmentation))
        .unwrap()
        .resolution;
    if !resolution.corrected_input {
        return joined_key_saving(&resolution.segmentation, &segments);
    }
    let literal = if segmentation.is_empty() {
        join_segments(&segments)
    } else {
        segmentation.to_owned()
    };
    joined_key_saving(&resolution.segmentation, &resolution.corrected)
        + joined_key_saving(&literal, &segments)
        + resolution
            .costlier_corrected_cuts
            .iter()
            .map(|cut| joined_key_saving(&join_segments(cut), cut))
            .sum::<usize>()
}

#[test]
fn full_series_preserves_prefix_bytes_and_saves_only_matching_nonempty_keys() {
    let fixture = fixture();
    for (raw, segmentation, pieces) in [
        ("ni", "ni", vec!["ni"]),
        ("nihao", "ni'hao", vec!["ni", "hao"]),
        ("nihaoma", "ni'hao'ma", vec!["ni", "hao", "ma"]),
        ("nue", "nve", vec!["nve"]),
        ("nue", "nue", vec!["nve"]),
        ("nihao", "", vec!["ni", "hao"]),
        ("nihao", "other", vec!["ni", "hao"]),
        ("nihao", "ni''hao", vec!["ni", "", "hao"]),
        ("ni", "'ni'", vec!["", "ni", ""]),
        ("nihao", "ni'hao'ma", vec!["ni'hao", "ma"]),
        ("nihao", "ni'hao'ma", vec!["ni", "hao'ma"]),
        ("synthetic", "合成'😀", vec!["合成", "😀"]),
        ("synthetic", "合成'😀", vec!["合成'😀"]),
        ("synthetic", "", vec![""]),
        ("synthetic", "'", vec!["", ""]),
        ("synthetic", "ni", vec![]),
    ] {
        let segments: Vec<String> = pieces.into_iter().map(str::to_owned).collect();
        for lattice in [false, true] {
            for warm in [false, true] {
                let mut before = QuanpinDictionary::new(&fixture.paths);
                let mut after = QuanpinDictionary::new(&fixture.paths);
                before.sentence_association.word_lattice = lattice;
                after.sentence_association.word_lattice = lattice;
                if warm {
                    assert_eq!(
                        after.query_series(raw, segmentation, &segments),
                        before.legacy_prefix_query_series(raw, segmentation, &segments)
                    );
                }
                let (expected, old_count) = if lattice {
                    (
                        before.legacy_prefix_query_series(raw, segmentation, &segments),
                        0,
                    )
                } else {
                    count(|| before.legacy_prefix_query_series(raw, segmentation, &segments))
                };
                let (mut actual, new_count) = if lattice {
                    (after.query_series(raw, segmentation, &segments), 0)
                } else {
                    count(|| after.query_series(raw, segmentation, &segments))
                };
                assert_rows(&actual, &expected);
                assert_state(&before, &after);
                if !lattice {
                    let saved = joined_key_saving(segmentation, &segments);
                    assert_eq!(
                        new_count + saved,
                        old_count,
                        "系列 {raw}/{segmentation}/{segments:?}/热={warm}"
                    );
                }
                // 再次读取路径缓存，借用键不能改变存储候选的所有权或容量。
                assert_rows(
                    &after.query_series(raw, segmentation, &segments),
                    &before.legacy_prefix_query_series(raw, segmentation, &segments),
                );
                if let Some(first) = actual.first_mut() {
                    first.word.push_str("独立副本");
                    assert_rows(&after.query_series(raw, segmentation, &segments), &expected);
                    after.reset_cache();
                    drop(after);
                    assert!(actual[0].word.ends_with("独立副本"));
                }
            }
        }
    }
}

#[test]
fn full_exact_dispatch_counts_primary_literal_and_costlier_calls() {
    let fixture = fixture();
    let mut multiple_calls = false;
    let mut costlier_calls = false;
    for (raw, segmentation, types) in [
        ("", "", 0),
        ("ni", "ni", 0),
        ("n", "n", 0),
        ("ni", "", 0),
        ("nue", "", 0),
        ("nue", "nue", 0),
        ("nihao", "ni'hao", 0),
        ("nihao", "", 0),
        ("zzz", "", 0),
        ("!", "", 0),
        ("sahng", "", autocorrect_type::TRANSPOSITION),
        (
            "gau",
            "",
            autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR,
        ),
        ("uanli", "", autocorrect_type::DELETION),
        ("sahng", "sahng", autocorrect_type::TRANSPOSITION),
        ("nihao", "ni''hao", 0),
    ] {
        // 静态切分/纠错表的首次初始化在计数外完成，两边只比较每次查询的申请。
        let mut warm_tables = QuanpinDictionary::new(&fixture.paths);
        drop(warm_tables.query_exact(raw, segmentation, types));
        for lattice in [false, true] {
            for mode in 0..3 {
                let mut before = QuanpinDictionary::new(&fixture.paths);
                let mut after = QuanpinDictionary::new(&fixture.paths);
                before.sentence_association.word_lattice = lattice;
                after.sentence_association.word_lattice = lattice;
                if mode != 0 {
                    assert_eq!(
                        after.query_exact(raw, segmentation, types),
                        before.legacy_prefix_query_exact(raw, segmentation, types)
                    );
                    if mode == 1 {
                        before.series_cache.clear();
                        after.series_cache.clear();
                    }
                }
                let (expected, old_count) = if lattice {
                    (
                        before.legacy_prefix_query_exact(raw, segmentation, types),
                        0,
                    )
                } else {
                    count(|| before.legacy_prefix_query_exact(raw, segmentation, types))
                };
                let (mut actual, new_count) = if lattice {
                    (after.query_exact(raw, segmentation, types), 0)
                } else {
                    count(|| after.query_exact(raw, segmentation, types))
                };
                assert_rows(&actual, &expected);
                assert_state(&before, &after);
                let saved = if mode == 2 {
                    0
                } else {
                    exact_saving(&mut before, raw, segmentation, types)
                };
                multiple_calls |= saved > 1;
                if !raw.is_empty() {
                    costlier_calls |= !before
                        .resolution_cache
                        .get_ref(&resolution_cache_hash(types, raw, segmentation))
                        .unwrap()
                        .resolution
                        .costlier_corrected_cuts
                        .is_empty();
                }
                if !lattice {
                    assert_eq!(
                        new_count + saved,
                        old_count,
                        "完整查询 {raw}/{segmentation}/{types}/缓存={mode}，目标省 {saved}"
                    );
                }
                if !raw.is_empty() {
                    let resolution = &after
                        .resolution_cache
                        .get_ref(&resolution_cache_hash(types, raw, segmentation))
                        .unwrap()
                        .resolution;
                    let slot = after.series_slot(&resolution.cache_key);
                    assert_rows(
                        after.series_cache.get_ref(&slot).unwrap(),
                        before.series_cache.get_ref(&slot).unwrap(),
                    );
                }
                if let Some(first) = actual.first_mut() {
                    first.word.push_str("独立副本");
                    assert_rows(
                        &after.query_exact(raw, segmentation, types),
                        &before.legacy_prefix_query_exact(raw, segmentation, types),
                    );
                    after.reset_cache();
                    drop(after);
                    assert!(actual[0].word.ends_with("独立副本"));
                }
            }
        }
    }
    assert!(multiple_calls, "纠错夹具必须覆盖同次查询的多个系列调用");
    assert!(costlier_calls, "夹具必须覆盖高成本纠错读法");
}

#[test]
fn invalidated_hot_series_rebuilds_and_keeps_longer_candidate_order() {
    let fixture = fixture();
    fixture.insert("ni'hao", "合成次重", 1);
    fixture.insert("ni'hao'ma", "合成更重补全", 1000);
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    before.sentence_association.word_lattice = false;
    after.sentence_association.word_lattice = false;
    for page_rows in [1, 8] {
        before.set_first_page_rows(FirstPageRows::new(page_rows));
        after.set_first_page_rows(FirstPageRows::new(page_rows));
        assert_eq!(
            after.query_exact("nihao", "ni'hao", 0),
            before.legacy_prefix_query_exact("nihao", "ni'hao", 0)
        );
        fixture.insert("ni'hao", "合成外部写入", 50);
        let (expected, old_count) =
            count(|| before.legacy_prefix_query_exact("nihao", "ni'hao", 0));
        let (actual, new_count) = count(|| after.query_exact("nihao", "ni'hao", 0));
        assert_rows(&actual, &expected);
        assert_state(&before, &after);
        assert!(actual.iter().any(|item| item.word == "合成外部写入"));
        assert_eq!(new_count + 1, old_count);
        assert_eq!(actual[0].word, "合成双音节");
        let longer = actual
            .iter()
            .position(|item| item.word == "合成更重补全")
            .unwrap();
        let secondary = actual
            .iter()
            .position(|item| item.word == "合成次重")
            .unwrap();
        assert_eq!(longer > secondary, page_rows == 1);
    }
}
