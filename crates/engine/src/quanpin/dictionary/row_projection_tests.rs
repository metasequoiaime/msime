use super::*;
use crate::ime::personal_rerank::allocations::{count, measure};
use crate::quanpin::fixture::Fixture;

// 固定 1ee789e042c6f8c0040798d13a884ac90f3a5038 的完整查询，只改方法名以便并列调用。
impl QuanpinDictionary {
    fn legacy_query_database(&mut self, segments: &[String], segmentation: &str) -> Vec<WordItem> {
        if segments.len() == 1 && segments[0].len() == 1 {
            let matched_code = if segmentation.is_empty() {
                segments[0].as_str()
            } else {
                segmentation
            };
            // 单字母的首字母表查询也走行缓存；键前加 `\u{1}`，和切分键分开。
            let key = format!("\u{1}{}", segments[0]);
            let rows = if let Some(rows) = self.segment_row_cache.get_ref(&key) {
                rows.clone()
            } else {
                let rows = self
                    .database
                    .query_initial(&segments[0], INITIAL_CANDIDATE_LIMIT);
                self.segment_row_cache.insert(key, rows.clone());
                rows
            };
            let mut rows = initial_items(rows);
            for item in &mut rows {
                item.canonical_pinyin = std::mem::replace(&mut item.pinyin, matched_code.into());
            }
            return rows;
        }
        // 音节已在 `resolve_segments` 规范化一次，直接使用标准键查找。
        let code = segmentation.is_empty().then(|| join_segments(segments));
        let key = code.as_deref().unwrap_or(segmentation);
        let rows = if let Some(rows) = self.segment_row_cache.get_ref_by(key) {
            rows.clone()
        } else {
            let rows = self.database.query_segments_keyed_flat(
                segments,
                UNLIMITED_ROWS,
                QuerySource::Quanpin,
            );
            self.segment_row_cache.insert(key.to_owned(), rows.clone());
            rows
        };
        if let Some(code) = code.as_deref() {
            rows.into_iter()
                .map(|row| {
                    WordItem::new(
                        code,
                        row.value,
                        row.weight,
                        CandidateSource::Database,
                        row.key,
                    )
                })
                .collect()
        } else {
            rows.into_iter()
                .map(|row| {
                    WordItem::new(
                        segmentation,
                        row.value,
                        row.weight,
                        CandidateSource::Database,
                        row.key,
                    )
                })
                .collect()
        }
    }

    fn legacy_longer_phrase_candidates(
        &mut self,
        segmentation: &str,
        segments: &[String],
    ) -> Vec<WordItem> {
        let rows = if let Some(rows) = self.longer_row_cache.get_ref_by(segmentation) {
            rows.clone()
        } else {
            let rows = self.database.query_longer_phrases(
                segments,
                LONGER_PHRASE_EXTRA_SYLLABLES,
                LONGER_PHRASE_LIMIT,
            );
            self.longer_row_cache
                .insert(segmentation.to_owned(), rows.clone());
            rows
        };
        rows.into_iter()
            .map(|row| {
                WordItem::new(
                    segmentation,
                    row.value,
                    row.weight,
                    CandidateSource::Database,
                    row.key,
                )
            })
            .collect()
    }
}

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    for index in 0..32 {
        fixture.insert("ni", &format!("合成单字{index:02}"), 1000 - index);
        fixture.insert("ni'hao", &format!("合成双字{index:02}"), 2000 - index);
    }
    fixture
        .insert("ni'hao", "合成双字负权", -7)
        .insert("ni'hao", "合成双字宽权", i64::MAX)
        .insert("ni'hao", "合成双字平权", 2000);
    fixture
        .insert("nu", "合成平权", 1000)
        .insert("na", "合成负权", -7)
        .insert("nai", "合成宽权", i64::MAX)
        .insert("ju", "合成别名", 20);
    for index in 0..4 {
        fixture.insert("ni'hao'ma", &format!("合成补全三{index}"), 500 - index);
        fixture.insert("ni'hao'ma'a", &format!("合成补全四{index}"), 500 - index);
        fixture.insert("ni'hao'ma'a'ba", &format!("合成补全五{index}"), -index);
    }
    fixture
}

fn capacities(items: &[WordItem]) -> Vec<(usize, usize, usize)> {
    items
        .iter()
        .map(|item| {
            (
                item.pinyin.capacity(),
                item.canonical_pinyin.capacity(),
                item.word.capacity(),
            )
        })
        .collect()
}

fn assert_cached_pages_equal(
    expected: &FifoCache<String, Vec<DictRow>>,
    actual: &FifoCache<String, Vec<DictRow>>,
    key: &str,
) {
    let expected = expected.get_ref_by(key).unwrap();
    let actual = actual.get_ref_by(key).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.capacity(), expected.capacity());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.key.capacity(), expected.key.capacity());
        assert_eq!(actual.value.capacity(), expected.value.capacity());
    }
}

#[test]
fn cold_and_warm_database_projection_preserves_fields_capacity_and_limits() {
    let fixture = fixture();
    for (syllables, segmentation) in [
        (vec!["n"], "n"),
        (vec!["n"], ""),
        (vec!["n"], "合成显示码"),
        (vec!["ni", "hao"], "ni'hao"),
        (vec!["ni", "hao"], ""),
        (vec!["ju"], "jv"),
        (vec!["ni", "ha"], "ni'ha"),
        (vec!["ni", "hao", "ma"], "ni'hao'ma"),
        (vec!["zzzz"], "zzzz"),
        (vec![], ""),
        (vec![""], ""),
    ] {
        let segments: Vec<String> = syllables.into_iter().map(str::to_owned).collect();
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        for round in 0..2 {
            let (expected, old_count) =
                count(|| before.legacy_query_database(&segments, segmentation));
            let (actual, new_count) = count(|| after.query_database(&segments, segmentation));
            assert_eq!(actual, expected, "{segments:?}/{segmentation:?}/{round}");
            assert_eq!(actual.capacity(), expected.capacity());
            assert_eq!(capacities(&actual), capacities(&expected));
            let initial = segments.len() == 1 && segments[0].len() == 1;
            if initial && segments[0] == "n" {
                assert_eq!(actual.len(), INITIAL_CANDIDATE_LIMIT);
            }
            // 冻结实现每行为匹配码新建一个 `SharedString`（字符串加 `Arc`）。
            let saved = if initial {
                2 * actual.len()
                    + usize::from(round == 1 && !actual.is_empty())
                    + usize::from(round == 1)
            } else {
                usize::from(round == 1 && !actual.is_empty())
            };
            assert_eq!(
                new_count + saved,
                old_count,
                "{segments:?}/{segmentation:?}/{round}"
            );
            let key = if initial {
                format!("\u{1}{}", segments[0])
            } else if segmentation.is_empty() {
                join_segments(&segments)
            } else {
                segmentation.to_owned()
            };
            assert_cached_pages_equal(&before.segment_row_cache, &after.segment_row_cache, &key);
        }
    }
}

#[test]
fn cold_and_warm_continuations_keep_rows_and_cached_storage() {
    let fixture = fixture();
    for (syllables, segmentation) in [
        (vec!["ni", "hao"], "ni'hao"),
        (vec!["ni", "ha"], "ni'ha"),
        (vec!["wo", "men"], "wo'men"),
        (vec![], ""),
        (vec!["zzzz"], "zzzz"),
    ] {
        let segments: Vec<String> = syllables.into_iter().map(str::to_owned).collect();
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        for round in 0..2 {
            let (expected, old_count) =
                count(|| before.legacy_longer_phrase_candidates(segmentation, &segments));
            let (actual, new_count) =
                count(|| after.longer_phrase_candidates(segmentation, &segments));
            assert_eq!(actual, expected);
            assert_eq!(actual.capacity(), expected.capacity());
            assert_eq!(capacities(&actual), capacities(&expected));
            assert_eq!(
                new_count + usize::from(round == 1 && !actual.is_empty()),
                old_count
            );
            assert_cached_pages_equal(
                &before.longer_row_cache,
                &after.longer_row_cache,
                segmentation,
            );
        }
    }
}

#[test]
fn warm_projection_heap_only_contains_owned_output() {
    let fixture = fixture();
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    for (syllables, segmentation) in [
        (vec!["n"], "n"),
        (vec!["ni", "hao"], "ni'hao"),
        (vec!["ni", "hao"], ""),
    ] {
        let segments: Vec<String> = syllables.into_iter().map(str::to_owned).collect();
        drop(before.legacy_query_database(&segments, segmentation));
        drop(after.query_database(&segments, segmentation));
        let (expected, old_heap) =
            measure(|| before.legacy_query_database(&segments, segmentation));
        let (actual, new_heap) = measure(|| after.query_database(&segments, segmentation));
        assert_eq!(actual, expected);
        assert_eq!(new_heap.remaining_bytes, old_heap.remaining_bytes);
        assert_eq!(new_heap.minimum_bytes, 0);
        assert_eq!(old_heap.minimum_bytes, 0);
        assert!(new_heap.peak_bytes < old_heap.peak_bytes);
        println!(
            "热投影 {segmentation:?}/{} 行: {}→{} 次, 峰值 {}→{} 字节, 返回 {} 字节",
            actual.len(),
            old_heap.allocations,
            new_heap.allocations,
            old_heap.peak_bytes,
            new_heap.peak_bytes,
            new_heap.remaining_bytes
        );
    }
}

#[test]
fn warm_reads_keep_fifo_age_and_returned_items_survive_cache_reset() {
    let fixture = fixture();
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    before.segment_row_cache = FifoCache::new(2);
    after.segment_row_cache = FifoCache::new(2);
    for dictionary in [&mut before, &mut after] {
        drop(dictionary.query_database(&["n".to_owned()], "n"));
        drop(dictionary.query_database(&["ni".to_owned(), "hao".to_owned()], "ni'hao"));
    }
    let n = ["n".to_owned()];
    let expected = before.legacy_query_database(&n, "n");
    let mut actual = after.query_database(&n, "n");
    assert_eq!(actual, expected);
    actual[0].word.push_str("修改副本");
    assert_eq!(after.query_database(&n, "n"), expected);
    actual[0] = expected[0].clone();
    let ju = ["ju".to_owned()];
    drop(before.legacy_query_database(&ju, "ju"));
    drop(after.query_database(&ju, "ju"));
    for dictionary in [&before, &after] {
        assert!(dictionary.segment_row_cache.get_ref_by("\u{1}n").is_none());
        assert!(dictionary.segment_row_cache.get_ref_by("ni'hao").is_some());
        assert!(dictionary.segment_row_cache.get_ref_by("ju").is_some());
    }
    after.reset_cache();
    drop(after);
    assert_eq!(actual, expected);
}
