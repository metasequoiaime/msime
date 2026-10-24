use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;

// 用固定回写层隔离解析投影收益；真实生产回写与容量由 online_cache_update_tests 独立对照。

// 固定 67f70bc3e690e2d57f4523f704d57a9cd214faea 的完整在线回写、首字母展开和解析方法，只改名、转接旧解析并移除注释。
impl QuanpinDictionary {
    fn legacy_slot_insert_online_words(
        &mut self,
        raw: &str,
        segmentation: &str,
        autocorrect_types: u32,
        words: &[String],
        source: CandidateSource,
    ) -> bool {
        if raw.is_empty() || words.is_empty() {
            return false;
        }
        let segments = self.resolve_segments(raw, segmentation);
        let resolution =
            self.legacy_slot_resolution(raw, segmentation, &segments, autocorrect_types);
        let key = self.series_slot(&resolution.cache_key);
        let mut list = self.series_cache.get(&key).unwrap_or_default();
        if !replace_online_candidate_batch(&mut list, raw, words, source) {
            return false;
        }
        self.series_cache.insert(key, list);
        true
    }

    fn legacy_slot_expand_initial_candidates(
        &mut self,
        raw: &str,
        segmentation: &str,
        autocorrect_types: u32,
        code: &str,
        candidates: &mut Vec<WordItem>,
    ) -> bool {
        if code.len() != 1 {
            return false;
        }
        let is_limited_initial =
            |item: &WordItem| item.source == CandidateSource::Database && item.pinyin == code;
        let limited_count = candidates
            .iter()
            .filter(|item| is_limited_initial(item))
            .count();
        if limited_count != INITIAL_CANDIDATE_LIMIT {
            return false;
        }
        let expanded = matched_items(self.database.query_initial(code, UNLIMITED_ROWS), code);
        if expanded.len() <= limited_count {
            return false;
        }
        let mut merged = Vec::with_capacity(candidates.len() - limited_count + expanded.len());
        let mut inserted = false;
        for item in candidates.drain(..) {
            if is_limited_initial(&item) {
                if !inserted {
                    merged.extend(expanded.iter().cloned());
                    inserted = true;
                }
                continue;
            }
            merged.push(item);
        }
        *candidates = merged;
        self.cache.insert(code.to_string(), expanded.into());
        let segments = self.resolve_segments(raw, segmentation);
        let resolution =
            self.legacy_slot_resolution(raw, segmentation, &segments, autocorrect_types);
        let key = self.series_slot(&series_cache_key(
            raw,
            &resolution.segmentation,
            autocorrect_types,
        ));
        self.series_cache.insert(key, candidates.clone());
        true
    }

    fn legacy_slot_resolution(
        &mut self,
        raw: &str,
        segmentation: &str,
        segments: &[String],
        types: u32,
    ) -> SeriesResolution {
        let hash = resolution_cache_hash(types, raw, segmentation);
        if let Some(cached) = self.resolution_cache.get_ref(&hash) {
            if resolution_cache_key_matches(&cached.key, types, raw, segmentation) {
                return cached.resolution.clone();
            }
        }
        let resolution = resolve_series_query(raw, segments, types);
        let key = format!("{types}\u{1f}{raw}\u{1f}{segmentation}");
        self.resolution_cache.insert(
            hash,
            CachedResolution {
                key,
                resolution: resolution.clone(),
            },
        );
        resolution
    }
}

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    for index in 0..32 {
        fixture.insert("ni", &format!("合成首字母{index:02}"), 1000 - index);
    }
    for (key, word) in [
        ("ni'hao", "合成整词"),
        ("shang", "合成换位"),
        ("quan'li", "合成删首"),
        ("nve", "合成别名"),
        ("gua", "合成甲"),
        ("gai", "合成乙"),
    ] {
        fixture.insert(key, word, 100);
    }
    fixture
}

fn query(
    dictionary: &mut QuanpinDictionary,
    raw: &str,
    segmentation: &str,
    types: u32,
) -> Vec<WordItem> {
    dictionary.query(raw, segmentation, types, FuzzyPinyinOptions { rules: 0 })
}

fn clone_cost(
    dictionary: &mut QuanpinDictionary,
    raw: &str,
    segmentation: &str,
    types: u32,
) -> (SeriesResolution, usize, usize) {
    let (segments, segment_copies) = count(|| dictionary.resolve_segments(raw, segmentation));
    let (resolution, copies) =
        count(|| dictionary.legacy_slot_resolution(raw, segmentation, &segments, types));
    (resolution, copies, segment_copies)
}

fn assert_layout(actual: &SeriesResolution, expected: &SeriesResolution) {
    assert_eq!(actual, expected);
    assert_eq!(
        actual.segmentation.capacity(),
        expected.segmentation.capacity()
    );
    assert_eq!(actual.cache_key.capacity(), expected.cache_key.capacity());
    assert_eq!(actual.corrected.capacity(), expected.corrected.capacity());
    for (actual, expected) in actual.corrected.iter().zip(&expected.corrected) {
        assert_eq!(actual.capacity(), expected.capacity());
    }
    for (actual, expected) in [
        (
            &actual.alternative_corrected_cuts,
            &expected.alternative_corrected_cuts,
        ),
        (
            &actual.costlier_corrected_cuts,
            &expected.costlier_corrected_cuts,
        ),
    ] {
        assert_eq!(actual.capacity(), expected.capacity());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.capacity(), expected.capacity());
            for (actual, expected) in actual.iter().zip(expected) {
                assert_eq!(actual.capacity(), expected.capacity());
            }
        }
    }
}

fn assert_slot(
    before: &QuanpinDictionary,
    after: &QuanpinDictionary,
    raw: &str,
    segmentation: &str,
    types: u32,
) {
    let hash = resolution_cache_hash(types, raw, segmentation);
    let expected = before.resolution_cache.get_ref(&hash).unwrap();
    let actual = after.resolution_cache.get_ref(&hash).unwrap();
    assert_eq!(actual.key, expected.key);
    assert_eq!(actual.key.capacity(), expected.key.capacity());
    assert_layout(&actual.resolution, &expected.resolution);
    let slot = after.series_slot(&actual.resolution.cache_key);
    let actual = after.series_cache.get_ref(&slot);
    let expected = before.series_cache.get_ref(&slot);
    assert_eq!(actual, expected);
    if let (Some(actual), Some(expected)) = (actual, expected) {
        assert_eq!(actual.capacity(), expected.capacity());
    }
}

#[test]
fn online_updates_copy_only_the_hot_resolution_key() {
    let fixture = fixture();
    let transposition = autocorrect_type::TRANSPOSITION;
    let neighbor = autocorrect_type::NEIGHBOR;
    let deletion = autocorrect_type::DELETION;
    let words = vec!["合成云甲".to_owned()];
    for (raw, segmentation, types) in [
        ("nihao", "ni'hao", 0),
        ("nihao", "", 0),
        ("ni'hao", "ni'hao", 0),
        ("nue", "nue", 0),
        ("nue", "", 0),
        ("gau", "", transposition | neighbor),
        ("sahng", "", transposition),
        ("uanli", "", deletion),
    ] {
        for source in [
            CandidateSource::CloudSuggestion,
            CandidateSource::AiSuggestion,
        ] {
            for empty_segments_cache in [false, true] {
                let mut before = QuanpinDictionary::new(&fixture.paths);
                let mut after = QuanpinDictionary::new(&fixture.paths);
                assert_eq!(
                    query(&mut after, raw, segmentation, types),
                    query(&mut before, raw, segmentation, types)
                );
                if empty_segments_cache {
                    before.segmentation_cache.clear();
                    after.segmentation_cache.clear();
                }
                let (resolution, copies, segment_copies) =
                    clone_cost(&mut before, raw, segmentation, types);
                if empty_segments_cache {
                    before.segmentation_cache.clear();
                }
                assert!(copies > 1);
                let (expected, old_count) = count(|| {
                    before.legacy_slot_insert_online_words(raw, segmentation, types, &words, source)
                });
                let (actual, new_count) = count(|| {
                    after.frozen_insert_online_words(raw, segmentation, types, &words, source)
                });
                assert_eq!(actual, expected);
                assert!(actual);
                assert_slot(&before, &after, raw, segmentation, types);
                println!("在线回写 {raw}/{types}/{source:?}：申请 {old_count}→{new_count}，完整解析复制 {copies}，音节构造 {segment_copies}，空切分缓存={empty_segments_cache}");
                assert_eq!(
                    new_count + copies - 1 + segment_copies,
                    old_count,
                    "只需复制一个拥有型缓存键，不构造未消费的音节"
                );
                if empty_segments_cache && segmentation.is_empty() {
                    assert!(after.segmentation_cache.get_ref_by(raw).is_none());
                }
                assert_eq!(
                    query(&mut after, raw, segmentation, types),
                    query(&mut before, raw, segmentation, types)
                );
                assert!(after
                    .series_cache
                    .get_ref(&after.series_slot(&resolution.cache_key))
                    .unwrap()
                    .iter()
                    .any(|item| item.word == "合成云甲"));
            }
        }
    }
}

#[test]
fn cold_online_updates_keep_the_original_allocation_and_capacity_policy() {
    let fixture = fixture();
    let words = vec!["合成冷回写".to_owned()];
    for (raw, segmentation, types) in [
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
        // 先初始化进程共享的纠错表，再用两个新词典比较各自尚未命中的解析缓存。
        let mut warm = QuanpinDictionary::new(&fixture.paths);
        assert!(warm.legacy_slot_insert_online_words(
            raw,
            segmentation,
            types,
            &words,
            CandidateSource::CloudSuggestion
        ));
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        let (expected, old_count) = count(|| {
            before.legacy_slot_insert_online_words(
                raw,
                segmentation,
                types,
                &words,
                CandidateSource::CloudSuggestion,
            )
        });
        let (actual, new_count) = count(|| {
            after.frozen_insert_online_words(
                raw,
                segmentation,
                types,
                &words,
                CandidateSource::CloudSuggestion,
            )
        });
        assert_eq!(actual, expected);
        assert_eq!(new_count, old_count, "冷解析不得多复制一次结果或键");
        assert_slot(&before, &after, raw, segmentation, types);
    }
}

#[test]
fn initial_expansion_projects_the_unmarked_slot_and_keeps_cold_cost() {
    let fixture = fixture();
    for (raw, segmentation, types) in [
        ("n", "n", 0),
        ("n", "", 0),
        ("sahng", "", autocorrect_type::TRANSPOSITION),
        (
            "gau",
            "",
            autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR,
        ),
    ] {
        for hot in [false, true] {
            for empty_segments_cache in [false, true] {
                let mut before = QuanpinDictionary::new(&fixture.paths);
                let mut after = QuanpinDictionary::new(&fixture.paths);
                let mut expected = query(&mut before, "n", "n", 0);
                let mut actual = query(&mut after, "n", "n", 0);
                let _ = before.database.query_initial("n", UNLIMITED_ROWS);
                let _ = after.database.query_initial("n", UNLIMITED_ROWS);
                let segments = before.resolve_segments(raw, segmentation);
                let _ = after.resolve_segments(raw, segmentation);
                let resolution = before.legacy_slot_resolution(raw, segmentation, &segments, types);
                if raw == "gau" {
                    assert!(resolution.corrected_input);
                    assert!(resolution.cache_key.starts_with("C:"));
                }
                let segments = after.resolve_segments(raw, segmentation);
                let _ = after.legacy_slot_resolution(raw, segmentation, &segments, types);
                if empty_segments_cache {
                    before.segmentation_cache.clear();
                    after.segmentation_cache.clear();
                }
                let (_, copies, segment_copies) = clone_cost(&mut before, raw, segmentation, types);
                if empty_segments_cache {
                    before.segmentation_cache.clear();
                }
                if !hot {
                    before.resolution_cache.clear();
                    after.resolution_cache.clear();
                }
                let (old_result, old_count) = count(|| {
                    before.legacy_slot_expand_initial_candidates(
                        raw,
                        segmentation,
                        types,
                        "n",
                        &mut expected,
                    )
                });
                let (new_result, new_count) = count(|| {
                    after.expand_initial_candidates(raw, segmentation, types, "n", &mut actual)
                });
                assert_eq!(new_result, old_result);
                assert!(new_result);
                assert_eq!(actual, expected);
                assert_eq!(actual.capacity(), expected.capacity());
                assert_slot(&before, &after, raw, segmentation, types);
                let key =
                    after.series_slot(&series_cache_key(raw, &resolution.segmentation, types));
                assert!(!key.starts_with("C:"));
                assert_eq!(after.series_cache.get_ref(&key), Some(&actual));
                assert_eq!(before.series_cache.get_ref(&key), Some(&expected));
                println!("首字母回写 {raw}/{hot}：申请 {old_count}→{new_count}，音节构造 {segment_copies}，空切分缓存={empty_segments_cache}");
                assert_eq!(
                    new_count + if hot { copies + segment_copies } else { 0 },
                    old_count
                );
                if hot && empty_segments_cache && segmentation.is_empty() {
                    assert!(after.segmentation_cache.get_ref_by(raw).is_none());
                }
            }
        }
    }
}

#[test]
fn projection_keeps_collisions_fifo_rejected_batches_and_independent_outputs() {
    let fixture = fixture();
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    before.resolution_cache = FifoCache::new(2);
    after.resolution_cache = FifoCache::new(2);
    let words = vec!["合成云行".to_owned()];
    for raw in ["ni", "hao", "ni", "ma"] {
        assert_eq!(
            before.legacy_slot_insert_online_words(
                raw,
                raw,
                0,
                &words,
                CandidateSource::CloudSuggestion
            ),
            after.frozen_insert_online_words(raw, raw, 0, &words, CandidateSource::CloudSuggestion)
        );
        assert_slot(&before, &after, raw, raw, 0);
    }
    let oldest = resolution_cache_hash(0, "ni", "ni");
    assert!(before.resolution_cache.get_ref(&oldest).is_none());
    assert!(after.resolution_cache.get_ref(&oldest).is_none());
    for dictionary in [&mut before, &mut after] {
        dictionary.resolution_cache.insert(
            resolution_cache_hash(0, "nihao", "ni'hao"),
            CachedResolution {
                key: "0\u{1f}wrong\u{1f}ni'hao".to_owned(),
                resolution: SeriesResolution {
                    segmentation: "错误切分".to_owned(),
                    cache_key: "错误槽".to_owned(),
                    ..Default::default()
                },
            },
        );
    }
    assert_eq!(
        before.legacy_slot_insert_online_words(
            "nihao",
            "ni'hao",
            0,
            &words,
            CandidateSource::CloudSuggestion
        ),
        after.frozen_insert_online_words(
            "nihao",
            "ni'hao",
            0,
            &words,
            CandidateSource::CloudSuggestion
        )
    );
    assert_slot(&before, &after, "nihao", "ni'hao", 0);
    for (raw, words, source) in [
        (
            "",
            vec!["合成空键".to_owned()],
            CandidateSource::CloudSuggestion,
        ),
        ("nihao", vec![], CandidateSource::AiSuggestion),
        (
            "nihao",
            vec!["坏\u{7}".to_owned()],
            CandidateSource::AiSuggestion,
        ),
        (
            "nihao",
            vec!["合成错误来源".to_owned()],
            CandidateSource::Database,
        ),
    ] {
        let expected = before.legacy_slot_insert_online_words(raw, "ni'hao", 0, &words, source);
        let actual = after.frozen_insert_online_words(raw, "ni'hao", 0, &words, source);
        assert_eq!(actual, expected);
        assert!(!actual);
    }
    let mut owned = query(&mut after, "nihao", "ni'hao", 0);
    assert_eq!(owned, query(&mut before, "nihao", "ni'hao", 0));
    owned[0].word.push_str("修改副本");
    assert_ne!(owned, query(&mut after, "nihao", "ni'hao", 0));
    after.reset_cache();
    drop(after);
    assert!(owned[0].word.ends_with("修改副本"));
}

#[test]
fn hot_rejected_batches_skip_segments_without_changing_cached_answers() {
    let fixture = fixture();
    for (words, source) in [
        (vec!["坏\u{7}".to_owned()], CandidateSource::AiSuggestion),
        (vec!["合成错误来源".to_owned()], CandidateSource::Database),
    ] {
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        assert_eq!(
            query(&mut after, "nihao", "", 0),
            query(&mut before, "nihao", "", 0)
        );
        before.segmentation_cache.clear();
        after.segmentation_cache.clear();
        let (_, copies, segment_copies) = clone_cost(&mut before, "nihao", "", 0);
        before.segmentation_cache.clear();
        let (expected, old_count) =
            count(|| before.legacy_slot_insert_online_words("nihao", "", 0, &words, source));
        let (actual, new_count) =
            count(|| after.frozen_insert_online_words("nihao", "", 0, &words, source));
        assert_eq!(actual, expected);
        assert!(!actual);
        assert_eq!(new_count + copies - 1 + segment_copies, old_count);
        assert!(after.segmentation_cache.get_ref_by("nihao").is_none());
        assert_slot(&before, &after, "nihao", "", 0);
        assert_eq!(
            query(&mut after, "nihao", "", 0),
            query(&mut before, "nihao", "", 0)
        );
    }
}
