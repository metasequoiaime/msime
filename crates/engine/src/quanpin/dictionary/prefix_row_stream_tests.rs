use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;

// 固定 fd9ba47349 的完整系列与单路径方法，只改名、转接冻结入口并移除注释。
impl QuanpinDictionary {
    fn legacy_stream_query_series(
        &mut self,
        raw: &str,
        segmentation: &str,
        segments: &[String],
    ) -> Vec<WordItem> {
        if segments.is_empty() {
            return self.legacy_stream_query_single_path(raw, segmentation, segments);
        }
        let mut result = Vec::new();
        let prefix_key = series_prefix_key(segmentation, segments);
        let mut partial_segmentation = prefix_key.as_ref();
        for count in (1..=segments.len()).rev() {
            if count < segments.len() {
                partial_segmentation = partial_segmentation
                    .rsplit_once('\'')
                    .map_or("", |(prefix, _)| prefix);
            }
            let partial = &segments[..count];
            let mut rows = self.legacy_stream_query_single_path(raw, partial_segmentation, partial);
            if count == segments.len() {
                let exact_rows = rows
                    .iter()
                    .filter(|item| item.canonical_pinyin == partial_segmentation)
                    .count();
                let longer = self.longer_phrase_candidates(partial_segmentation, partial);
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
                let rows =
                    self.legacy_stream_query_single_path(raw, &fallback_segmentation, &fallback);
                append_unique_words(&mut result, rows);
            }
        }
        result
    }

    fn legacy_stream_query_single_path(
        &mut self,
        raw: &str,
        segmentation: &str,
        segments: &[String],
    ) -> Vec<WordItem> {
        let cache_key = path_cache_key(raw, segmentation);
        if let Some(cached) = self.cache.get_ref_by(cache_key) {
            return cached.copy_rows(cache_key);
        }
        // 冷存储共用当前入口，旧路径仍将热页整体复制后追加。
        self.query_single_path_miss(cache_key, segmentation, segments)
    }
}

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    for key in [
        "ni",
        "ni'hao",
        "ni'hao'ma",
        "nve",
        "nve'hao",
        "shi",
        "shi'jie",
    ] {
        for index in 0..16 {
            fixture.insert(key, &format!("合成{key}词{index}😀"), 1000 - index);
        }
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

#[test]
fn hot_series_appends_prefixes_without_temporary_pages() {
    let fixture = fixture();
    for (raw, segmentation, cuts, saved) in [
        ("nihao", "ni'hao", vec!["ni", "hao"], 1),
        ("nihaoma", "ni'hao'ma", vec!["ni", "hao", "ma"], 2),
        ("nvehao", "nve'hao", vec!["nve", "hao"], 1),
        ("ni", "ni", vec!["ni"], 0),
        ("missing", "", vec!["missing"], 0),
        ("", "", vec![], 0),
    ] {
        let cuts: Vec<String> = cuts.into_iter().map(str::to_owned).collect();
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        before.sentence_association.word_lattice = false;
        after.sentence_association.word_lattice = false;
        let (expected, old_cold) =
            count(|| before.legacy_stream_query_series(raw, segmentation, &cuts));
        let (actual, new_cold) = count(|| after.query_series(raw, segmentation, &cuts));
        assert_rows(&actual, &expected);
        assert_eq!(new_cold, old_cold, "冷路径 {raw}");
        let (expected, old_hot) =
            count(|| before.legacy_stream_query_series(raw, segmentation, &cuts));
        let (actual, new_hot) = count(|| after.query_series(raw, segmentation, &cuts));
        assert_rows(&actual, &expected);
        eprintln!("{raw}: cold {old_cold}->{new_cold}, hot {old_hot}->{new_hot}, saved={saved}");
        assert_eq!(new_hot + saved, old_hot, "热路径 {raw}");
    }
}

#[test]
fn complete_rows_preserve_database_weight_order_when_merging() {
    let fixture = Fixture::new();
    for index in 0..4 {
        fixture.insert("ni'hao", &format!("合成同音{index}😀"), 100 - index);
    }
    for index in 0..32 {
        fixture.insert("ni'hao'ma", &format!("合成长词{index}😀"), 1000 - index);
    }
    let cuts = vec!["ni".to_owned(), "hao".to_owned()];
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    before.sentence_association.word_lattice = false;
    after.sentence_association.word_lattice = false;

    let (expected, _) = count(|| before.legacy_stream_query_series("nihao", "ni'hao", &cuts));
    let (actual, _) = count(|| after.query_series("nihao", "ni'hao", &cuts));
    assert_rows(&actual, &expected);
}

#[test]
#[ignore = "手动配对计时，需在没有并行构建时运行"]
fn complete_rows_skip_redundant_sort_paired_timing() {
    use std::hint::black_box;
    use std::time::Instant;

    let fixture = Fixture::new();
    for index in 0..4 {
        fixture.insert("ni'hao", &format!("合成同音{index}😀"), 100 - index);
    }
    for index in 0..32 {
        fixture.insert("ni'hao'ma", &format!("合成长词{index}😀"), 1000 - index);
    }
    let cuts = vec!["ni".to_owned(), "hao".to_owned()];
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    before.sentence_association.word_lattice = false;
    after.sentence_association.word_lattice = false;
    black_box(before.legacy_stream_query_series("nihao", "ni'hao", &cuts));
    black_box(after.query_series("nihao", "ni'hao", &cuts));

    let mut ratios = Vec::new();
    for round in 0..7 {
        let mut elapsed = [0u128; 2];
        for side in if round % 2 == 0 { [0, 1] } else { [1, 0] } {
            let started = Instant::now();
            for _ in 0..20_000 {
                let rows = if side == 0 {
                    before.legacy_stream_query_series("nihao", "ni'hao", &cuts)
                } else {
                    after.query_series("nihao", "ni'hao", &cuts)
                };
                black_box(rows);
            }
            elapsed[side] = started.elapsed().as_nanos();
        }
        ratios.push(elapsed[0] as f64 / elapsed[1] as f64);
    }
    ratios.sort_by(f64::total_cmp);
    println!(
        "完整切分合并旧/新墙钟比 p10={:.3} p50={:.3} p90={:.3}",
        ratios[1], ratios[3], ratios[5]
    );
}

#[test]
fn series_stream_keeps_cache_and_owned_output() {
    let fixture = fixture();
    for word_lattice in [false, true] {
        for (raw, segmentation, cuts) in [
            ("nihaoma", "ni'hao'ma", vec!["ni", "hao", "ma"]),
            ("alias", "nue'hao", vec!["nve", "hao"]),
            ("n", "n", vec!["n"]),
            ("missing", "", vec!["missing", "hao"]),
            ("空😀", "空😀'hao", vec!["空😀", "hao"]),
            ("odd", "wrong", vec!["ni", "hao"]),
            ("empty", "'", vec!["", ""]),
            ("embedded", "ni'hao'ma", vec!["ni'hao", "ma"]),
        ] {
            let cuts: Vec<String> = cuts.into_iter().map(str::to_owned).collect();
            let mut before = QuanpinDictionary::new(&fixture.paths);
            let mut after = QuanpinDictionary::new(&fixture.paths);
            before.sentence_association.word_lattice = word_lattice;
            after.sentence_association.word_lattice = word_lattice;
            for _ in 0..2 {
                let expected = before.legacy_stream_query_series(raw, segmentation, &cuts);
                let mut actual = after.query_series(raw, segmentation, &cuts);
                assert_rows(&actual, &expected);
                for length in 1..=cuts.len() {
                    let key = join_segments(&cuts[..length]);
                    if let Some(expected) = before.cache.get_ref_by(key.as_str()) {
                        assert_rows(
                            &after
                                .cache
                                .get_ref_by(key.as_str())
                                .unwrap()
                                .copy_rows(&key),
                            &expected.copy_rows(&key),
                        );
                    }
                }
                if let Some(first) = actual.first_mut() {
                    first.word.push_str("修改😀");
                    first.sentence_words.push("独立合成词".to_owned());
                }
                assert_rows(
                    &after.query_series(raw, segmentation, &cuts),
                    &before.legacy_stream_query_series(raw, segmentation, &cuts),
                );
            }
            let expected = before.legacy_stream_query_series(raw, segmentation, &cuts);
            let actual = after.query_series(raw, segmentation, &cuts);
            after.reset_cache();
            before.reset_cache();
            drop(after);
            drop(before);
            assert_rows(&actual, &expected);
        }
    }
}

#[test]
fn prefix_append_preserves_metadata_capacity_and_fifo_age() {
    let fixture = fixture();
    for capacity in [0, 4, 32] {
        for populated in [false, true] {
            if capacity == 0 && populated {
                continue;
            }
            for cached_empty in [false, true] {
                let cached = || {
                    let mut row = WordItem::new(
                        "ni",
                        "合成缓存😀",
                        -7,
                        CandidateSource::CloudSuggestion,
                        "ni",
                    );
                    row.corrected_from = "合成纠错".into();
                    row.corrected_from.reserve(60);
                    row.sentence_words = vec!["合成".to_owned(), "分词😀".to_owned()];
                    row.sentence_words.reserve(8);
                    row.sentence_association = true;
                    row.fixed_position = 3;
                    row.fuzzy = true;
                    if cached_empty {
                        Vec::new()
                    } else {
                        vec![row]
                    }
                };
                let mut before = QuanpinDictionary::new(&fixture.paths);
                let mut after = QuanpinDictionary::new(&fixture.paths);
                before.cache = FifoCache::new(2);
                after.cache = FifoCache::new(2);
                before.cache.insert("ni".to_owned(), cached().into());
                after.cache.insert("ni".to_owned(), cached().into());
                before.cache.insert("second".to_owned(), Vec::new().into());
                after.cache.insert("second".to_owned(), Vec::new().into());
                let mut expected = Vec::with_capacity(capacity);
                let mut actual = Vec::with_capacity(capacity);
                if populated {
                    expected.push(WordItem::new(
                        "seed",
                        "合成已有词",
                        9,
                        CandidateSource::Database,
                        "seed",
                    ));
                    actual = expected.clone();
                    actual.reserve(capacity - actual.len());
                }
                let cuts = vec!["ni".to_owned()];
                let rows = before.legacy_stream_query_single_path("ni", "ni", &cuts);
                QuanpinDictionary::append_query_rows(&mut expected, rows);
                after.append_single_path(&mut actual, "ni", "ni", &cuts);
                assert_rows(&actual, &expected);
                assert_rows(
                    &after.cache.get_ref_by("ni").unwrap().copy_rows("ni"),
                    &before.cache.get_ref_by("ni").unwrap().copy_rows("ni"),
                );
                before.cache.insert("third".to_owned(), Vec::new().into());
                after.cache.insert("third".to_owned(), Vec::new().into());
                assert!(before.cache.get_ref_by("ni").is_none());
                assert!(after.cache.get_ref_by("ni").is_none());
                assert!(after.cache.get_ref_by("second").is_some());
                after.reset_cache();
                drop(after);
                assert_rows(&actual, &expected);
            }
        }
    }
}
