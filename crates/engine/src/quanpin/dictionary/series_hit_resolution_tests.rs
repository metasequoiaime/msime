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

fn clone_cost(
    dictionary: &mut QuanpinDictionary,
    raw: &str,
    segmentation: &str,
    types: u32,
) -> (usize, usize) {
    let (segments, segment_copies) = count(|| dictionary.resolve_segments(raw, segmentation));
    let (resolution, copies) = count(|| dictionary.resolution(raw, segmentation, &segments, types));
    assert!(!resolution.segmentation.is_empty());
    (copies, segment_copies)
}

fn assert_state(before: &QuanpinDictionary, after: &QuanpinDictionary) {
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
}

#[test]
fn hot_series_answers_skip_the_unused_owned_resolution() {
    let fixture = fixture();
    for (raw, segmentation, types) in [
        ("ni", "ni", 0),
        ("nihao", "ni'hao", 0),
        ("ni'hao", "ni'hao", 0),
        ("nue", "nue", 0),
        (
            "gau",
            "",
            autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR,
        ),
        ("uanli", "", autocorrect_type::DELETION),
        ("zzz", "", 0),
    ] {
        for word_lattice in [false, true] {
            let mut before = QuanpinDictionary::new(&fixture.paths);
            let mut after = QuanpinDictionary::new(&fixture.paths);
            before.sentence_association.word_lattice = word_lattice;
            after.sentence_association.word_lattice = word_lattice;
            assert_eq!(
                after.query_exact(raw, segmentation, types),
                before.legacy_hit_query_exact(raw, segmentation, types)
            );
            // 切换当前标记，确认再次热命中仍发布本次解析的标记状态。
            drop(before.legacy_hit_query_exact("ma", "ma", 0));
            drop(after.query_exact("ma", "ma", 0));
            let (copies, segment_copies) = clone_cost(&mut before, raw, segmentation, types);
            let (expected, old_count) =
                count(|| before.legacy_hit_query_exact(raw, segmentation, types));
            let (actual, new_count) = count(|| after.query_exact(raw, segmentation, types));
            assert_eq!(actual, expected);
            assert_eq!(actual.capacity(), expected.capacity());
            assert_state(&before, &after);
            println!("系列热命中 {raw}/{types}/词网格={word_lattice}：申请 {old_count}→{new_count}，完整解析复制 {copies}，音节复制 {segment_copies}");
            assert_eq!(
                new_count + copies + segment_copies,
                old_count,
                "热候选只需要解析标记和槽，不构造音节"
            );
        }
    }
}

#[test]
fn cold_resolution_or_missing_series_keeps_original_cost() {
    let fixture = fixture();
    for (raw, segmentation, types) in [
        ("ni", "ni", 0),
        ("nihao", "ni'hao", 0),
        (
            "gau",
            "",
            autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR,
        ),
    ] {
        let mut warm = QuanpinDictionary::new(&fixture.paths);
        drop(warm.query_exact(raw, segmentation, types));
        for mode in 0..3 {
            let mut before = QuanpinDictionary::new(&fixture.paths);
            let mut after = QuanpinDictionary::new(&fixture.paths);
            if mode != 0 {
                assert_eq!(
                    after.query_exact(raw, segmentation, types),
                    before.legacy_hit_query_exact(raw, segmentation, types)
                );
                if mode == 1 {
                    before.series_cache.clear();
                    after.series_cache.clear();
                } else {
                    before.resolution_cache.clear();
                    after.resolution_cache.clear();
                }
            }
            let (expected, old_count) =
                count(|| before.legacy_hit_query_exact(raw, segmentation, types));
            let (actual, new_count) = count(|| after.query_exact(raw, segmentation, types));
            assert_eq!(actual, expected);
            assert_eq!(actual.capacity(), expected.capacity());
            assert_state(&before, &after);
            let slot_copy = if mode != 2 {
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
                "只有重建且未登记的槽键省一次复制"
            );
        }
    }
}

#[test]
fn changed_sources_rebuild_a_previously_hot_series_and_keep_owned_output() {
    let fixture = fixture();
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    for raw in ["ni", "nihao"] {
        assert_eq!(
            after.query_exact(raw, "", 0),
            before.legacy_hit_query_exact(raw, "", 0)
        );
    }
    fixture.insert("ni", "合成外部写入", 500);
    let (expected, old_count) = count(|| before.legacy_hit_query_exact("ni", "", 0));
    let (actual, new_count) = count(|| after.query_exact("ni", "", 0));
    assert_eq!(actual, expected);
    assert!(actual.iter().any(|item| item.word == "合成外部写入"));
    assert_eq!(
        new_count + 1,
        old_count,
        "单音节版本重建只省未登记槽键的副本"
    );
    assert_state(&before, &after);

    assert_eq!(
        after.query_exact("nihao", "ni'hao", 0),
        before.legacy_hit_query_exact("nihao", "ni'hao", 0)
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
    let (expected, old_count) = count(|| before.legacy_hit_query_exact("nihao", "ni'hao", 0));
    let (mut actual, new_count) = count(|| after.query_exact("nihao", "ni'hao", 0));
    assert_eq!(actual, expected);
    assert_eq!(new_count, old_count, "个人模型失效后的重建不得增加申请");
    assert_state(&before, &after);
    assert_eq!(after.personal_version, after.personal.version());
    actual[0].word.push_str("独立副本");
    assert_eq!(after.query_exact("nihao", "ni'hao", 0), expected);
    after.reset_cache();
    drop(after);
    assert!(actual[0].word.ends_with("独立副本"));
}

#[cfg(unix)]
// 仅手动计时使用 POSIX 时钟；按现有测试分配器惯例，把 FFI 例外限制在此函数。
#[allow(unsafe_code)]
fn thread_cpu_ns() -> u128 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // 指针指向当前栈上已初始化的结构；时钟调用同步填充它，不保留指针。
    let status = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) };
    assert_eq!(status, 0, "读取线程 CPU 时钟失败");
    time.tv_sec as u128 * 1_000_000_000 + time.tv_nsec as u128
}

#[cfg(unix)]
#[test]
#[ignore = "手动配对计时，线程 CPU 与墙钟比例不作门禁"]
fn paired_hot_and_miss_timing() {
    let fixture = fixture();
    for (raw, segmentation, types, series_miss) in [
        ("ni", "ni", 0, false),
        ("nihao", "ni'hao", 0, false),
        (
            "gau",
            "",
            autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR,
            false,
        ),
        ("uanli", "", autocorrect_type::DELETION, false),
        ("ni", "ni", 0, true),
        ("nihao", "ni'hao", 0, true),
    ] {
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        assert_eq!(
            after.query_exact(raw, segmentation, types),
            before.legacy_hit_query_exact(raw, segmentation, types)
        );
        let mut ratios = Vec::new();
        let mut wall_ratios = Vec::new();
        for pair in 0..11 {
            let mut elapsed = [0u128; 2];
            let mut wall_elapsed = [0u128; 2];
            for side in if pair % 2 == 0 { [0, 1] } else { [1, 0] } {
                let dictionary = if side == 0 { &mut before } else { &mut after };
                let started = std::time::Instant::now();
                let started_cpu = thread_cpu_ns();
                for _ in 0..10_000 {
                    if series_miss {
                        dictionary.series_cache.clear();
                    }
                    let rows = if side == 0 {
                        dictionary.legacy_hit_query_exact(raw, segmentation, types)
                    } else {
                        dictionary.query_exact(raw, segmentation, types)
                    };
                    drop(std::hint::black_box(rows));
                }
                elapsed[side] = thread_cpu_ns() - started_cpu;
                wall_elapsed[side] = started.elapsed().as_nanos();
            }
            ratios.push(elapsed[0] as f64 / elapsed[1] as f64);
            wall_ratios.push(wall_elapsed[0] as f64 / wall_elapsed[1] as f64);
        }
        ratios.sort_by(f64::total_cmp);
        wall_ratios.sort_by(f64::total_cmp);
        println!("查询配对 {raw}/{types}/系列未命中={series_miss}：旧/新线程CPU比 p10={:.3} p50={:.3} p90={:.3}；墙钟比 p10={:.3} p50={:.3} p90={:.3}", ratios[1], ratios[5], ratios[9], wall_ratios[1], wall_ratios[5], wall_ratios[9]);
    }
}

// 固定 b5fdaa710cd1cfc7aae09a14c9fca21f11d42538 的完整查询，只改名并移除注释。
impl QuanpinDictionary {
    fn legacy_hit_query_exact(
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
