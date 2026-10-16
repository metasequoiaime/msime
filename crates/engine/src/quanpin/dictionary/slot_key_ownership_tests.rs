use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;

// 冻结旧查询逐次复制主标记；预热后已有容量的测量另计一次节省。

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    for (key, word) in [
        ("ni", "合成单音节😀"),
        ("ni'hao", "合成双音节"),
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
    // 主切分标记复用历史容量，候选与缓存的容量仍逐项对照。
    assert!(after.pinyin_segmentation.capacity() >= before.pinyin_segmentation.capacity());
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

// 完整旧查询的计分集合说明槽键是否还有第二个消费者；仅在确认本次重建后计入这个副本。
pub(super) fn discarded_slot_copy_cost(
    dictionary: &QuanpinDictionary,
    raw: &str,
    segmentation: &str,
    types: u32,
) -> usize {
    if raw.is_empty() {
        return 0;
    }
    let hash = resolution_cache_hash(types, raw, segmentation);
    let resolution = &dictionary
        .resolution_cache
        .get_ref(&hash)
        .unwrap()
        .resolution;
    let slot = dictionary.series_slot(&resolution.cache_key);
    usize::from(!dictionary.personal_scored_keys.contains(&slot))
}

#[test]
fn only_unscored_series_misses_transfer_the_slot_key() {
    let fixture = fixture();
    for (raw, segmentation, types, scored) in [
        ("", "", 0, false),
        ("ni", "ni", 0, false),
        ("n", "n", 0, false),
        ("ni", "", 0, false),
        ("nue", "", 0, false),
        ("sahng", "", autocorrect_type::TRANSPOSITION, false),
        (
            "gau",
            "",
            autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR,
            false,
        ),
        ("uanli", "", autocorrect_type::DELETION, true),
        ("nihao", "ni'hao", 0, true),
        ("nihao", "", 0, true),
        ("zzz", "", 0, true),
        ("!", "", 0, false),
    ] {
        let mut warm = QuanpinDictionary::new(&fixture.paths);
        drop(warm.query_exact(raw, segmentation, types));
        for word_lattice in [false, true] {
            for mode in 0..4 {
                let mut before = QuanpinDictionary::new(&fixture.paths);
                let mut after = QuanpinDictionary::new(&fixture.paths);
                before.sentence_association.word_lattice = word_lattice;
                after.sentence_association.word_lattice = word_lattice;
                if mode != 0 {
                    assert_eq!(
                        after.query_exact(raw, segmentation, types),
                        before.legacy_slot_owner_query_exact(raw, segmentation, types)
                    );
                    if mode <= 2 {
                        before.series_cache.clear();
                        after.series_cache.clear();
                    }
                    if mode == 2 {
                        before.resolution_cache.clear();
                        after.resolution_cache.clear();
                    }
                }
                // 冷/未命中精确计数关闭词网格，排除进程共享 n 元表的并发淘汰；开启时保留完整行为对照。
                let (expected, old_count) = if word_lattice {
                    (
                        before.legacy_slot_owner_query_exact(raw, segmentation, types),
                        0,
                    )
                } else {
                    count(|| before.legacy_slot_owner_query_exact(raw, segmentation, types))
                };
                let (mut actual, new_count) = if word_lattice {
                    (after.query_exact(raw, segmentation, types), 0)
                } else {
                    count(|| after.query_exact(raw, segmentation, types))
                };
                assert_rows(&actual, &expected);
                assert_state(&before, &after);
                if !word_lattice {
                    let saved = usize::from(!raw.is_empty() && !scored && mode != 3);
                    println!("槽键 {raw}/{segmentation}/{mode}：申请 {old_count}→{new_count}，目标省 {saved}");
                    assert_eq!(
                        new_count + saved + usize::from(mode != 0 && !raw.is_empty()),
                        old_count
                    );
                }
                if !raw.is_empty() {
                    let hash = resolution_cache_hash(types, raw, segmentation);
                    let resolution = &after.resolution_cache.get_ref(&hash).unwrap().resolution;
                    let slot = after.series_slot(&resolution.cache_key);
                    assert_eq!(
                        slot.capacity(),
                        slot.clone().capacity(),
                        "转移槽键不能保留额外容量"
                    );
                    assert_eq!(after.personal_scored_keys.contains(&slot), scored);
                    assert_rows(
                        after.series_cache.get_ref(&slot).unwrap(),
                        before.series_cache.get_ref(&slot).unwrap(),
                    );
                }
                if let Some(first) = actual.first_mut() {
                    first.word.push_str("独立副本");
                    assert_rows(
                        &after.query_exact(raw, segmentation, types),
                        &before.legacy_slot_owner_query_exact(raw, segmentation, types),
                    );
                    after.reset_cache();
                    drop(after);
                    assert!(actual[0].word.ends_with("独立副本"));
                }
            }
        }
    }
}

#[test]
fn transferred_keys_keep_fifo_age_and_personal_invalidation() {
    let fixture = fixture();
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    before.sentence_association.word_lattice = false;
    after.sentence_association.word_lattice = false;
    before.series_cache = FifoCache::new(2);
    after.series_cache = FifoCache::new(2);
    for (raw, segmentation) in [
        ("ni", "ni"),
        ("nihao", "ni'hao"),
        ("ni", "ni"),
        ("ma", "ma"),
    ] {
        assert_eq!(
            after.query_exact(raw, segmentation, 0),
            before.legacy_slot_owner_query_exact(raw, segmentation, 0)
        );
        assert_state(&before, &after);
    }
    let single_slot = after.series_slot(&series_cache_key("ni", "ni", 0));
    let multi_slot = after.series_slot(&series_cache_key("nihao", "ni'hao", 0));
    assert!(!after.series_cache.contains(&single_slot));
    assert!(!before.series_cache.contains(&single_slot));
    assert!(after.series_cache.contains(&multi_slot));
    assert!(before.series_cache.contains(&multi_slot));
    let unscored_slot = after.series_slot(&series_cache_key("ma", "ma", 0));
    after
        .personal
        .record(&[PersonalTransition {
            earlier: String::new(),
            previous: "合成上文".into(),
            word: "合成双音节".into(),
            times: 1,
        }])
        .unwrap();
    before.drop_personal_scored_results();
    after.drop_personal_scored_results();
    assert_state(&before, &after);
    assert!(!after.series_cache.contains(&multi_slot));
    assert!(!before.series_cache.contains(&multi_slot));
    assert!(after.series_cache.contains(&unscored_slot));
    assert!(before.series_cache.contains(&unscored_slot));
    assert_eq!(
        after.query_exact("nihao", "ni'hao", 0),
        before.legacy_slot_owner_query_exact("nihao", "ni'hao", 0)
    );
    assert_state(&before, &after);
}

// 固定 2ef5e70096104461e294dd12171aeb55f2565675 的完整查询，仅改名并移除注释。
impl QuanpinDictionary {
    fn legacy_slot_owner_query_exact(
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
