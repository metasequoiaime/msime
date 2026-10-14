use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    for (key, word) in [
        ("ni", "合成单音节😀"),
        ("ni'hao", "合成双音节"),
        ("nve", "合成别名"),
        ("gua", "合成低成本"),
        ("gai", "合成高成本"),
        ("quan'li", "合成删首"),
    ] {
        fixture.insert(key, word, 100);
    }
    fixture
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

// 冷路径精确预算排除进程共享 n 元表缓存的并发淘汰；词网格开启时仍对照完整行为与容量。
fn observed_query(measure: bool, query: impl FnOnce() -> Vec<WordItem>) -> (Vec<WordItem>, usize) {
    if measure {
        count(query)
    } else {
        (query(), 0)
    }
}

#[test]
fn hot_series_skips_the_unused_segment_list() {
    let fixture = fixture();
    for (raw, segmentation, types) in [
        ("ni", "ni", 0),
        ("nihao", "ni'hao", 0),
        ("ni'hao", "ni'hao", 0),
        ("nihao", "", 0),
        ("nue", "nue", 0),
        ("nue", "", 0),
        (
            "gau",
            "",
            autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR,
        ),
        ("uanli", "", autocorrect_type::DELETION),
        ("zzz", "", 0),
    ] {
        for word_lattice in [false, true] {
            for empty_segments_cache in [false, true] {
                let mut before = QuanpinDictionary::new(&fixture.paths);
                let mut after = QuanpinDictionary::new(&fixture.paths);
                before.sentence_association.word_lattice = word_lattice;
                after.sentence_association.word_lattice = word_lattice;
                assert_eq!(
                    after.query_exact(raw, segmentation, types),
                    before.legacy_segments_query_exact(raw, segmentation, types)
                );
                drop(before.legacy_segments_query_exact("ma", "ma", 0));
                drop(after.query_exact("ma", "ma", 0));
                if empty_segments_cache {
                    before.segmentation_cache.clear();
                    after.segmentation_cache.clear();
                }
                let (_, segment_cost) = count(|| before.resolve_segments(raw, segmentation));
                if empty_segments_cache {
                    before.segmentation_cache.clear();
                }
                let (expected, old_count) =
                    count(|| before.legacy_segments_query_exact(raw, segmentation, types));
                let (mut actual, new_count) = count(|| after.query_exact(raw, segmentation, types));
                assert_eq!(actual, expected);
                assert_eq!(actual.capacity(), expected.capacity());
                assert_state(&before, &after);
                println!("系列音节 {raw}/{segmentation}/{types}/词网格={word_lattice}/空切分缓存={empty_segments_cache}：申请 {old_count}→{new_count}，音节成本 {segment_cost}");
                assert_eq!(new_count + segment_cost, old_count, "热系列不需要构造音节");
                if empty_segments_cache && segmentation.is_empty() {
                    assert!(
                        after.segmentation_cache.get_ref_by(raw).is_none(),
                        "热命中不填充未消费的切分备忘"
                    );
                    assert!(before.segmentation_cache.get_ref_by(raw).is_some());
                }
                if let Some(first) = actual.first_mut() {
                    first.word.push_str("独立副本");
                    assert_eq!(after.query_exact(raw, segmentation, types), expected);
                    after.reset_cache();
                    drop(after);
                    assert!(actual[0].word.ends_with("独立副本"));
                }
            }
        }
    }
}

#[test]
fn misses_resolve_segments_once_and_keep_cold_cost() {
    for word_lattice in [false, true] {
        let fixture = fixture();
        for (raw, segmentation, types) in [
            ("ni", "ni", 0),
            ("nihao", "ni'hao", 0),
            ("nihao", "", 0),
            ("nue", "", 0),
            (
                "gau",
                "",
                autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR,
            ),
            ("uanli", "", autocorrect_type::DELETION),
        ] {
            let mut warm = QuanpinDictionary::new(&fixture.paths);
            drop(warm.query_exact(raw, segmentation, types));
            for mode in 0..6 {
                let mut before = QuanpinDictionary::new(&fixture.paths);
                let mut after = QuanpinDictionary::new(&fixture.paths);
                before.sentence_association.word_lattice = word_lattice;
                after.sentence_association.word_lattice = word_lattice;
                if mode != 0 {
                    assert_eq!(
                        after.query_exact(raw, segmentation, types),
                        before.legacy_segments_query_exact(raw, segmentation, types)
                    );
                    match mode {
                        1 => {
                            before.series_cache.clear();
                            after.series_cache.clear();
                        }
                        2 => {
                            before.resolution_cache.clear();
                            after.resolution_cache.clear();
                        }
                        3 => {
                            let hash = resolution_cache_hash(types, raw, segmentation);
                            let mut mismatch =
                                before.resolution_cache.get_ref(&hash).unwrap().clone();
                            mismatch.key = "合成哈希冲突".into();
                            before.resolution_cache.insert(hash, mismatch.clone());
                            after.resolution_cache.insert(hash, mismatch);
                        }
                        4 => {
                            before.reset_cache();
                            after.reset_cache();
                        }
                        5 => {
                            before.series_cache.clear();
                            after.series_cache.clear();
                            before.segmentation_cache.clear();
                            after.segmentation_cache.clear();
                        }
                        _ => unreachable!(),
                    }
                }
                let (expected, old_count) = observed_query(!word_lattice, || {
                    before.legacy_segments_query_exact(raw, segmentation, types)
                });
                let (actual, new_count) = observed_query(!word_lattice, || {
                    after.query_exact(raw, segmentation, types)
                });
                assert_eq!(actual, expected);
                assert_eq!(actual.capacity(), expected.capacity());
                assert_state(&before, &after);
                if !word_lattice {
                    let slot_copy = if matches!(mode, 0 | 1 | 4 | 5) {
                        super::slot_key_ownership_tests::discarded_slot_copy_cost(
                            &before,
                            raw,
                            segmentation,
                            types,
                        )
                    } else {
                        0
                    };
                    assert_eq!(
                        new_count + slot_copy,
                        old_count,
                        "音节成本不变，仅重建的未登记槽键省一次复制：{raw}/{mode}"
                    );
                }
            }
        }
    }
}

#[test]
fn database_change_resolves_segments_before_clearing_their_cache() {
    for word_lattice in [false, true] {
        let fixture = fixture();
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        before.sentence_association.word_lattice = word_lattice;
        after.sentence_association.word_lattice = word_lattice;
        for raw in ["ni", "nihao"] {
            assert_eq!(
                after.query_exact(raw, "", 0),
                before.legacy_segments_query_exact(raw, "", 0)
            );
            assert!(after.segmentation_cache.get_ref_by(raw).is_some());
            fixture.insert(
                if raw == "ni" { "ni" } else { "ni'hao" },
                "合成外部写入",
                500,
            );
            let (expected, old_count) = observed_query(!word_lattice, || {
                before.legacy_segments_query_exact(raw, "", 0)
            });
            let (actual, new_count) =
                observed_query(!word_lattice, || after.query_exact(raw, "", 0));
            assert_eq!(actual, expected);
            assert_eq!(actual.capacity(), expected.capacity());
            assert!(actual.iter().any(|item| item.word == "合成外部写入"));
            if !word_lattice {
                let slot_copy =
                    super::slot_key_ownership_tests::discarded_slot_copy_cost(&before, raw, "", 0);
                assert_eq!(
                    new_count + slot_copy,
                    old_count,
                    "版本变化清空前复用音节，未登记的槽键另省一次复制"
                );
            }
            assert_state(&before, &after);
            assert!(before.segmentation_cache.get_ref_by(raw).is_none());
            assert!(after.segmentation_cache.get_ref_by(raw).is_none());
        }
    }
}

#[test]
fn personal_change_rebuilds_with_the_original_segments_and_owned_output() {
    for word_lattice in [false, true] {
        let fixture = fixture();
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        before.sentence_association.word_lattice = word_lattice;
        after.sentence_association.word_lattice = word_lattice;
        assert_eq!(
            after.query_exact("nihao", "", 0),
            before.legacy_segments_query_exact("nihao", "", 0)
        );
        let previous = after.personal_version;
        after
            .personal
            .record(&[PersonalTransition {
                earlier: String::new(),
                previous: "合成上文".into(),
                word: "合成双音节".into(),
                times: 1,
            }])
            .unwrap();
        assert_ne!(after.personal.version(), previous);
        let (expected, old_count) = observed_query(!word_lattice, || {
            before.legacy_segments_query_exact("nihao", "", 0)
        });
        let (actual, new_count) =
            observed_query(!word_lattice, || after.query_exact("nihao", "", 0));
        assert_eq!(actual, expected);
        assert_eq!(actual.capacity(), expected.capacity());
        if !word_lattice {
            assert_eq!(new_count, old_count);
        }
        assert_state(&before, &after);
        assert_eq!(after.personal_version, after.personal.version());
    }
}

#[test]
fn deferred_segment_memo_keeps_total_cost_lower_when_the_series_later_misses() {
    for word_lattice in [false, true] {
        for (raw, types, key) in [
            ("nihao", 0, "ni'hao"),
            ("nue", 0, "nve"),
            (
                "gau",
                autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR,
                "gua",
            ),
        ] {
            let fixture = fixture();
            let mut before = QuanpinDictionary::new(&fixture.paths);
            let mut after = QuanpinDictionary::new(&fixture.paths);
            before.sentence_association.word_lattice = word_lattice;
            after.sentence_association.word_lattice = word_lattice;
            assert_eq!(
                after.query_exact(raw, "", types),
                before.legacy_segments_query_exact(raw, "", types)
            );
            fixture.insert(key, "合成版本更新", 500);
            assert_eq!(
                after.query_exact(raw, "", types),
                before.legacy_segments_query_exact(raw, "", types)
            );
            assert!(before.segmentation_cache.get_ref_by(raw).is_none());
            assert!(after.segmentation_cache.get_ref_by(raw).is_none());

            let (expected, old_hot) = observed_query(!word_lattice, || {
                before.legacy_segments_query_exact(raw, "", types)
            });
            let (actual, new_hot) =
                observed_query(!word_lattice, || after.query_exact(raw, "", types));
            assert_eq!(actual, expected);
            assert_state(&before, &after);
            let warm_segments = if word_lattice {
                0
            } else {
                count(|| before.resolve_segments(raw, "")).1
            };
            assert!(after.segmentation_cache.get_ref_by(raw).is_none());
            before.series_cache.clear();
            after.series_cache.clear();
            let (expected, old_miss) = observed_query(!word_lattice, || {
                before.legacy_segments_query_exact(raw, "", types)
            });
            let (actual, new_miss) =
                observed_query(!word_lattice, || after.query_exact(raw, "", types));
            assert_eq!(actual, expected);
            assert_eq!(actual.capacity(), expected.capacity());
            assert_state(&before, &after);
            if !word_lattice {
                println!("音节延迟序列 {raw}：热 {old_hot}→{new_hot}，随后未命中 {old_miss}→{new_miss}，原热音节 {warm_segments}");
                assert_eq!(
                    new_hot
                        + new_miss
                        + warm_segments
                        + super::slot_key_ownership_tests::discarded_slot_copy_cost(
                            &before, raw, "", types
                        ),
                    old_hot + old_miss,
                    "首次实际消费才填切分备忘，累计分配不增加"
                );
            }
        }
    }
}

// 固定 a62792a3cf336a144088ba68428cd553ba74377f 的完整查询，只改名并移除注释。
impl QuanpinDictionary {
    fn legacy_segments_query_exact(
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
        let segments = self.resolve_segments(raw, segmentation);
        let association = self.sentence_association;
        let has_rerankers = !self.rerankers.is_empty();
        let context = &self.rescoring_context;
        let (resolution_hash, (resolution, pinyin_segmentation, marking_alternatives, slot)) =
            project_cached_resolution(
                &mut self.resolution_cache,
                raw,
                segmentation,
                &segments,
                types,
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
            self.reset_cache_if_database_changed();
            if let Some(cached) = self.series_cache.get(&slot) {
                return cached;
            }
        }

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
        self.series_cache.insert(slot.clone(), result.clone());
        if segments.len() >= 2 || resolution.corrected.len() >= 2 {
            self.remember_personal_scored_key(slot);
        }
        result
    }
}
