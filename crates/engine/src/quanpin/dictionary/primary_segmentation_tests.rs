use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    for (key, word) in [
        ("ni", "合成单字😀"),
        ("ni'hao", "合成双字"),
        ("ni'hao'ma", "合成补全"),
        ("nve", "合成别名"),
        ("gua", "合成低成本"),
        ("gai", "合成高成本"),
        ("shang", "合成换位"),
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
    assert_eq!(
        after.current_autocorrect_types,
        before.current_autocorrect_types
    );
}

#[test]
fn series_misses_borrow_the_owned_primary_segmentation() {
    let fixture = fixture();
    for (raw, segmentation, types) in [
        ("ni", "ni", 0),
        ("nihao", "ni'hao", 0),
        ("ni'hao", "ni'hao", 0),
        ("nihao", "", 0),
        ("nue", "nue", 0),
        (
            "gau",
            "",
            autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR,
        ),
        ("sahng", "", autocorrect_type::TRANSPOSITION),
        ("uanli", "", autocorrect_type::DELETION),
    ] {
        // 进程共享表先初始化；冷用例仍从新词典的空解析缓存和空行缓存开始。
        let mut warm = QuanpinDictionary::new(&fixture.paths);
        drop(warm.query_exact(raw, segmentation, types));
        for hot_resolution in [false, true] {
            let mut before = QuanpinDictionary::new(&fixture.paths);
            let mut after = QuanpinDictionary::new(&fixture.paths);
            if hot_resolution {
                assert_eq!(
                    after.query_exact(raw, segmentation, types),
                    before.legacy_primary_query_exact(raw, segmentation, types)
                );
                before.series_cache.clear();
                after.series_cache.clear();
            }
            let (expected, old_count) =
                count(|| before.legacy_primary_query_exact(raw, segmentation, types));
            let (actual, new_count) = count(|| after.query_exact(raw, segmentation, types));
            assert_eq!(actual, expected);
            assert_eq!(actual.capacity(), expected.capacity());
            assert!(!after.pinyin_segmentation.is_empty());
            assert_state(&before, &after);
            let segments = after.resolve_segments(raw, segmentation);
            let resolution = after.resolution(raw, segmentation, &segments, types);
            if raw == "gau" {
                assert!(resolution.corrected_input);
                assert!(!resolution.costlier_corrected_cuts.is_empty());
            }
            let slot = after.series_slot(&resolution.cache_key);
            assert_eq!(after.series_cache.get_ref(&slot), Some(&actual));
            assert_eq!(before.series_cache.get_ref(&slot), Some(&expected));
            println!("主切分 {raw}/{types}/解析热={hot_resolution}：申请 {old_count}→{new_count}");
            let slot_copy = usize::from(!before.personal_scored_keys.contains(&slot));
            assert_eq!(
                new_count + 1 + slot_copy,
                old_count,
                "非空主切分借用，未登记的槽键直接转移"
            );
        }
    }
}

#[test]
fn series_hits_skip_resolution_and_segment_copies_and_empty_input_keeps_owned_results() {
    let fixture = fixture();
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    for (raw, segmentation, types) in [
        ("", "", 0),
        ("ni", "ni", 0),
        ("nihao", "ni'hao", 0),
        (
            "gau",
            "",
            autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR,
        ),
    ] {
        assert_eq!(
            after.query_exact(raw, segmentation, types),
            before.legacy_primary_query_exact(raw, segmentation, types)
        );
        let (expected, old_count) =
            count(|| before.legacy_primary_query_exact(raw, segmentation, types));
        let (mut actual, new_count) = count(|| after.query_exact(raw, segmentation, types));
        assert_eq!(actual, expected);
        assert_eq!(actual.capacity(), expected.capacity());
        let copies = if raw.is_empty() {
            0
        } else {
            let (segments, segment_copies) = count(|| before.resolve_segments(raw, segmentation));
            let (_, copies) = count(|| before.resolution(raw, segmentation, &segments, types));
            copies + segment_copies
        };
        assert_eq!(
            new_count + copies,
            old_count,
            "系列热命中省去完整解析与音节复制，空输入保持原样"
        );
        assert_state(&before, &after);
        if !actual.is_empty() {
            actual[0].word.push_str("修改副本");
            assert_eq!(after.query_exact(raw, segmentation, types), expected);
        }
    }
    let mut owned = after.query_exact("ni", "ni", 0);
    owned[0].word.push_str("独立结果");
    after.reset_cache();
    drop(after);
    assert!(owned[0].word.ends_with("独立结果"));
}

// 固定 a582955502f2111d94a85490abcb0395d792c78e 的完整查询方法，仅改名并移除注释。
impl QuanpinDictionary {
    fn legacy_primary_query_exact(
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
        let resolution = self.resolution(raw, segmentation, &segments, types);
        self.pinyin_segmentation = resolution.segmentation.clone();
        self.alternative_segmentations = resolution
            .alternative_corrected_cuts
            .iter()
            .chain(&resolution.costlier_corrected_cuts)
            .map(|cut| join_segments(cut))
            .collect();

        self.drop_personal_scored_results();
        let slot = self.series_slot(&resolution.cache_key);
        if self.series_cache.contains(&slot) {
            self.reset_cache_if_database_changed();
            if let Some(cached) = self.series_cache.get(&slot) {
                return cached;
            }
        }

        self.validate_row_caches();
        let alternatives = alternative_segmentations(raw, &segments, &resolution, types);
        let primary_segmentation = resolution.segmentation.clone();
        let mut result;
        if resolution.corrected_input {
            result = self.query_series(raw, &primary_segmentation, &resolution.corrected);
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
                    &primary_segmentation,
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
            result = self.query_series(raw, &primary_segmentation, &segments);
            if !alternatives.is_empty() {
                result = self.merge_alternatives(
                    raw,
                    &primary_segmentation,
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
