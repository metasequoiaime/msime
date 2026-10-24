use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;

// 冻结旧查询逐次复制主标记；预热后已有容量的测量另计一次节省。

// 固定 194ec019b6a65ac90317ec24b3b1173636bee471 的完整方法，只改名、转接旧查询链并移除注释。
impl QuanpinDictionary {
    fn legacy_initial_key_query_exact(
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
            result = self.legacy_initial_key_query_series(
                raw,
                primary_segmentation,
                &resolution.corrected,
            );
            let literal = if segmentation.is_empty() {
                let literal_segmentation = join_segments(&segments);
                self.legacy_initial_key_query_series(raw, &literal_segmentation, &segments)
            } else {
                self.legacy_initial_key_query_series(raw, segmentation, &segments)
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
                let rows =
                    self.legacy_initial_key_query_series(raw, &join_segments(costlier), costlier);
                append_unique_words(&mut result, rows);
            }
        } else {
            result = self.legacy_initial_key_query_series(raw, primary_segmentation, &segments);
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
        result
    }

    fn legacy_initial_key_query_series(
        &mut self,
        raw: &str,
        segmentation: &str,
        segments: &[String],
    ) -> Vec<WordItem> {
        if segments.is_empty() {
            return self.legacy_initial_key_query_single_path(raw, segmentation, segments);
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
            let mut rows =
                self.legacy_initial_key_query_single_path(raw, partial_segmentation, partial);
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
                let rows = self.legacy_initial_key_query_single_path(
                    raw,
                    &fallback_segmentation,
                    &fallback,
                );
                append_unique_words(&mut result, rows);
            }
        }
        result
    }

    fn legacy_initial_key_query_single_path(
        &mut self,
        raw: &str,
        segmentation: &str,
        segments: &[String],
    ) -> Vec<WordItem> {
        let cache_key = path_cache_key(raw, segmentation);
        if let Some(cached) = self.cache.get_ref_by(cache_key) {
            return cached.copy_rows(cache_key);
        }
        let result = self.legacy_initial_key_query_database(segments, segmentation);
        // 与新入口共用当前页表示，只隔离首字母键的构造。
        self.cache.insert(
            cache_key.to_owned(),
            PathPage::from_dictionary(cache_key, &result),
        );
        result
    }

    fn legacy_initial_key_query_database(
        &mut self,
        segments: &[String],
        segmentation: &str,
    ) -> Vec<WordItem> {
        if segments.len() == 1 && segments[0].len() == 1 {
            let matched_code = if segmentation.is_empty() {
                segments[0].as_str()
            } else {
                segmentation
            };
            let key = format!("\u{1}{}", segments[0]);
            if let Some(rows) = self.segment_row_cache.get_ref(&key) {
                return matched_items(rows.iter().cloned(), matched_code);
            }
            let rows = self
                .database
                .query_initial(&segments[0], INITIAL_CANDIDATE_LIMIT);
            self.segment_row_cache.insert(key, rows.clone());
            return matched_items(rows, matched_code);
        }
        let code = segmentation.is_empty().then(|| join_segments(segments));
        let key = code.as_deref().unwrap_or(segmentation);
        if let Some(rows) = self.segment_row_cache.get_ref_by(key) {
            return matched_items(rows.iter().cloned(), key);
        }
        let rows =
            self.database
                .query_segments_keyed_flat(segments, UNLIMITED_ROWS, QuerySource::Quanpin);
        self.segment_row_cache.insert(key.to_owned(), rows.clone());
        matched_items(rows, key)
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
    for index in 0..32 {
        fixture.insert("ni", &format!("合成首字母{index:02}"), 200 - index);
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

fn assert_cached_rows(before: &QuanpinDictionary, after: &QuanpinDictionary, key: &str) {
    let expected = before.segment_row_cache.get_ref_by(key).unwrap();
    let actual = after.segment_row_cache.get_ref_by(key).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.capacity(), expected.capacity());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.key.capacity(), expected.key.capacity());
        assert_eq!(actual.value.capacity(), expected.value.capacity());
    }
}

#[test]
fn initial_row_hits_skip_only_the_temporary_key_for_every_single_byte_input() {
    let fixture = fixture();
    for byte in 0u8..=127 {
        let segments = vec![char::from(byte).to_string()];
        let key = format!("\u{1}{}", segments[0]);
        for display in ["", segments[0].as_str(), "合成显示😀"] {
            let mut before = QuanpinDictionary::new(&fixture.paths);
            let mut after = QuanpinDictionary::new(&fixture.paths);
            for warm in [false, true] {
                let (expected, old_count) =
                    count(|| before.legacy_initial_key_query_database(&segments, display));
                let (mut actual, new_count) = count(|| after.query_database(&segments, display));
                assert_rows(&actual, &expected);
                if byte == b'n' {
                    assert_eq!(actual.len(), INITIAL_CANDIDATE_LIMIT);
                }
                assert_cached_rows(&before, &after, &key);
                assert_eq!(
                    new_count + usize::from(warm),
                    old_count,
                    "字节 {byte}/显示={display:?}/热={warm}"
                );
                if let Some(first) = actual.first_mut() {
                    first.word.push_str("独立副本");
                    assert_rows(
                        &after.query_database(&segments, display),
                        &before.legacy_initial_key_query_database(&segments, display),
                    );
                    assert_cached_rows(&before, &after, &key);
                }
            }
            after.reset_cache();
            assert_rows(
                &after.query_database(&segments, display),
                &before.legacy_initial_key_query_database(&segments, display),
            );
        }
    }
}

#[test]
fn noninitial_segments_preserve_query_cost_and_owned_results() {
    let fixture = fixture();
    for (pieces, display) in [
        (vec![], ""),
        (vec![""], ""),
        (vec!["ni"], "ni"),
        (vec!["ni", "hao"], "ni'hao"),
        (vec!["ni", "hao"], ""),
        (vec!["é"], "é"),
        (vec!["合成"], "合成"),
        (vec!["😀"], "😀"),
        (vec!["n", "hao"], "n'hao"),
    ] {
        let segments: Vec<String> = pieces.into_iter().map(str::to_owned).collect();
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        for _ in 0..2 {
            let (expected, old_count) =
                count(|| before.legacy_initial_key_query_database(&segments, display));
            let (mut actual, new_count) = count(|| after.query_database(&segments, display));
            assert_rows(&actual, &expected);
            assert_eq!(new_count, old_count);
            if let Some(first) = actual.first_mut() {
                first.word.push_str("独立副本");
                assert!(actual[0].word.ends_with("独立副本"));
            }
        }
        drop(after);
    }
}

#[test]
fn full_queries_save_a_key_only_when_initial_rows_outlive_path_answers() {
    let fixture = fixture();
    for (raw, segmentation) in [
        ("n", "n"),
        ("n", ""),
        ("nhao", "n'hao"),
        ("z", "z"),
        ("!", "!"),
        ("ni", "ni"),
        ("", ""),
    ] {
        let mut warm_tables = QuanpinDictionary::new(&fixture.paths);
        drop(warm_tables.query_exact(raw, segmentation, 0));
        for mode in 0..3 {
            let mut before = QuanpinDictionary::new(&fixture.paths);
            let mut after = QuanpinDictionary::new(&fixture.paths);
            before.sentence_association.word_lattice = false;
            after.sentence_association.word_lattice = false;
            if mode != 0 {
                assert_eq!(
                    after.query_exact(raw, segmentation, 0),
                    before.legacy_initial_key_query_exact(raw, segmentation, 0)
                );
                if mode == 1 {
                    before.cache.clear();
                    before.series_cache.clear();
                    after.cache.clear();
                    after.series_cache.clear();
                }
            }
            let segments = warm_tables.resolve_segments(raw, segmentation);
            let key = segments.first().map(|first| format!("\u{1}{first}"));
            let initial_hit = mode == 1
                && segments.first().is_some_and(|first| first.len() == 1)
                && key
                    .as_ref()
                    .is_some_and(|key| before.segment_row_cache.contains(key));
            let (expected, old_count) =
                count(|| before.legacy_initial_key_query_exact(raw, segmentation, 0));
            let (mut actual, new_count) = count(|| after.query_exact(raw, segmentation, 0));
            assert_rows(&actual, &expected);
            assert_state(&before, &after);
            assert_eq!(
                new_count + usize::from(initial_hit) + usize::from(mode != 0 && !raw.is_empty()),
                old_count,
                "完整查询 {raw}/{segmentation}/缓存={mode}"
            );
            if let Some(first) = actual.first_mut() {
                first.word.push_str("独立副本");
                assert_rows(
                    &after.query_exact(raw, segmentation, 0),
                    &before.legacy_initial_key_query_exact(raw, segmentation, 0),
                );
                after.reset_cache();
                drop(after);
                assert!(actual[0].word.ends_with("独立副本"));
            }
        }
    }
}

#[test]
fn stack_lookup_preserves_fifo_age_and_database_invalidation() {
    let fixture = fixture();
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    before.segment_row_cache = FifoCache::new(2);
    after.segment_row_cache = FifoCache::new(2);
    for code in ["n", "m", "n", "h"] {
        let segments = vec![code.to_owned()];
        assert_rows(
            &after.query_database(&segments, code),
            &before.legacy_initial_key_query_database(&segments, code),
        );
    }
    for dictionary in [&before, &after] {
        assert!(!dictionary.segment_row_cache.contains(&"\u{1}n".to_owned()));
        assert!(dictionary.segment_row_cache.contains(&"\u{1}m".to_owned()));
        assert!(dictionary.segment_row_cache.contains(&"\u{1}h".to_owned()));
    }
    assert_eq!(
        after.query_exact("n", "n", 0),
        before.legacy_initial_key_query_exact("n", "n", 0)
    );
    fixture.insert("ni", "合成外部写入", 1000);
    let (expected, old_count) = count(|| before.legacy_initial_key_query_exact("n", "n", 0));
    let (actual, new_count) = count(|| after.query_exact("n", "n", 0));
    assert_rows(&actual, &expected);
    assert!(actual.iter().any(|item| item.word == "合成外部写入"));
    assert_eq!(new_count + 1, old_count, "版本重建仅省去已复用的主标记副本");
}
