use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::quanpin::fixture::Fixture;

fn cached_rows(count: usize, source: CandidateSource) -> Vec<WordItem> {
    let mut rows = Vec::with_capacity(count + 1);
    for index in 0..count {
        let mut row = WordItem::new(
            "ni",
            format!("合成本地{index}"),
            index as i64,
            CandidateSource::Database,
            "ni",
        );
        row.corrected_from = "合成纠错".into();
        row.sentence_association = true;
        row.sentence_words = vec!["合成词甲".to_owned(), "合成词乙".to_owned()];
        rows.push(row);
    }
    rows.push(WordItem::new("ni", "合成旧在线", 1, source, ""));
    rows
}

fn batches() -> Vec<(Vec<String>, CandidateSource)> {
    vec![
        (vec!["合成新在线".to_owned()], CandidateSource::AiSuggestion),
        (
            vec!["合成新在线".to_owned()],
            CandidateSource::CloudSuggestion,
        ),
        (vec![], CandidateSource::AiSuggestion),
        (vec!["坏\n".to_owned()], CandidateSource::AiSuggestion),
        (
            (0..11).map(|index| format!("超额{index}")).collect(),
            CandidateSource::AiSuggestion,
        ),
        (vec!["不合法来源".to_owned()], CandidateSource::Database),
    ]
}

// 固定 4f499433c5 的完整回写入口，仅改名并移除注释。
impl QuanpinDictionary {
    pub(super) fn frozen_insert_online_words(
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
        let cache_key =
            self.project_resolution(raw, segmentation, autocorrect_types, |resolution| {
                match resolution {
                    Cow::Borrowed(resolution) => resolution.cache_key.clone(),
                    Cow::Owned(resolution) => resolution.cache_key,
                }
            });
        let key = self.series_slot(&cache_key);
        let mut list = self.series_cache.get(&key).unwrap_or_default();
        if !replace_online_candidate_batch(&mut list, raw, words, source) {
            return false;
        }
        self.series_cache.insert(key, list);
        true
    }
}

fn slot(dictionary: &mut QuanpinDictionary, raw: &str) -> String {
    let cache_key =
        dictionary.project_resolution(raw, raw, 0, |resolution| resolution.cache_key.clone());
    dictionary.series_slot(&cache_key)
}

#[test]
fn cached_online_update_avoids_page_clones() {
    let fixture = Fixture::new();
    for width in [0, 1, 63, 64, 128] {
        for source in [
            CandidateSource::AiSuggestion,
            CandidateSource::CloudSuggestion,
        ] {
            let original = cached_rows(width, source);
            let (_, clone_cost) = count(|| original.clone());
            for (batch, batch_source) in batches() {
                let mut before = QuanpinDictionary::new(&fixture.paths);
                let mut after = QuanpinDictionary::new(&fixture.paths);
                let key = slot(&mut before, "ni");
                assert_eq!(slot(&mut after, "ni"), key);
                before.series_cache.insert(key.clone(), original.clone());
                after.series_cache.insert(key.clone(), original.clone());
                let (expected, old_count) = count(|| {
                    before.frozen_insert_online_words("ni", "ni", 0, &batch, batch_source)
                });
                let (actual, new_count) =
                    count(|| after.insert_online_words("ni", "ni", 0, &batch, batch_source));
                assert_eq!(actual, expected);
                assert_eq!(
                    after.series_cache.get_ref(&key),
                    before.series_cache.get_ref(&key)
                );
                // 另一来源的新行可能扩容；它只影响旧新容器的增长，不是页字段复制。
                if batch.is_empty() {
                    assert_eq!(new_count, old_count);
                } else if batch_source == source || !actual {
                    assert_eq!(
                        new_count + clone_cost,
                        old_count,
                        "width={width} source={source:?} batch={batch_source:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn cached_online_update_keeps_fifo_and_owned_snapshots() {
    let fixture = Fixture::new();
    let mut dictionary = QuanpinDictionary::new(&fixture.paths);
    dictionary.series_cache = FifoCache::new(2);
    let first = slot(&mut dictionary, "ni");
    let second = slot(&mut dictionary, "hao");
    let third = slot(&mut dictionary, "ma");
    let original = cached_rows(3, CandidateSource::AiSuggestion);
    dictionary
        .series_cache
        .insert(first.clone(), original.clone());
    dictionary
        .series_cache
        .insert(second.clone(), original.clone());
    let snapshot = dictionary.series_cache.get(&first).unwrap();
    let fields: Vec<_> = dictionary.series_cache.get_ref(&first).unwrap()[..3]
        .iter()
        .map(|row| {
            (
                row.word.as_ptr(),
                row.canonical_pinyin.as_ptr(),
                row.sentence_words[0].as_ptr(),
            )
        })
        .collect();
    assert!(dictionary.insert_online_words(
        "ni",
        "ni",
        0,
        &["合成新在线".to_owned()],
        CandidateSource::AiSuggestion
    ));
    assert_eq!(snapshot, original);
    let stored = dictionary.series_cache.get_ref(&first).unwrap();
    for (row, pointers) in stored[..3].iter().zip(fields) {
        assert_eq!(
            (
                row.word.as_ptr(),
                row.canonical_pinyin.as_ptr(),
                row.sentence_words[0].as_ptr()
            ),
            pointers
        );
    }
    dictionary.series_cache.insert(third.clone(), Vec::new());
    assert!(dictionary.series_cache.get_ref(&first).is_none());
    assert!(dictionary.series_cache.get_ref(&second).is_some());
    assert!(dictionary.series_cache.get_ref(&third).is_some());
    dictionary.reset_cache();
    assert_eq!(snapshot, original);
}

#[test]
fn missing_online_slots_keep_success_and_rejection_policy() {
    let fixture = Fixture::new();
    for (batch, source) in batches() {
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        let key = slot(&mut before, "ni");
        assert_eq!(slot(&mut after, "ni"), key);
        let (expected, old_count) =
            count(|| before.frozen_insert_online_words("ni", "ni", 0, &batch, source));
        let (actual, new_count) =
            count(|| after.insert_online_words("ni", "ni", 0, &batch, source));
        assert_eq!((actual, new_count), (expected, old_count));
        assert_eq!(
            after.series_cache.get_ref(&key),
            before.series_cache.get_ref(&key)
        );
    }
}

fn retained_bytes(rows: &Vec<WordItem>) -> usize {
    rows.capacity() * std::mem::size_of::<WordItem>()
        + rows
            .iter()
            .map(|row| {
                row.pinyin.capacity()
                    + row.canonical_pinyin.capacity()
                    + row.word.capacity()
                    + row.corrected_from.capacity()
                    + row.sentence_words.capacity() * std::mem::size_of::<String>()
                    + row
                        .sentence_words
                        .iter()
                        .map(String::capacity)
                        .sum::<usize>()
            })
            .sum::<usize>()
}

fn replacement_sequence() -> Vec<(Vec<String>, CandidateSource)> {
    vec![
        (
            (0..10).map(|index| format!("合成批次{index}")).collect(),
            CandidateSource::AiSuggestion,
        ),
        (vec!["合成缩小".to_owned()], CandidateSource::AiSuggestion),
        (
            vec!["合成本地0".to_owned()],
            CandidateSource::CloudSuggestion,
        ),
        (
            vec!["合成云行".to_owned()],
            CandidateSource::CloudSuggestion,
        ),
        (
            vec!["合成缩小".to_owned(), "合成缩小".to_owned()],
            CandidateSource::AiSuggestion,
        ),
        (vec!["坏\n".to_owned()], CandidateSource::CloudSuggestion),
    ]
}

fn time_batch(iterations: usize, mut update: impl FnMut() -> bool) -> u128 {
    let started = std::time::Instant::now();
    for _ in 0..iterations {
        std::hint::black_box(update());
    }
    started.elapsed().as_nanos()
}

#[test]
fn repeated_online_updates_preserve_fields_and_expose_retained_capacity() {
    let fixture = Fixture::new();
    let rows = cached_rows(16, CandidateSource::AiSuggestion);
    let mut before = QuanpinDictionary::new(&fixture.paths);
    let mut after = QuanpinDictionary::new(&fixture.paths);
    let key = slot(&mut before, "ni");
    assert_eq!(slot(&mut after, "ni"), key);
    before.series_cache.insert(key.clone(), rows.clone());
    after.series_cache.insert(key.clone(), rows.clone());
    let held_before = retained_bytes(after.series_cache.get_ref(&key).unwrap());
    for (step, (batch, source)) in replacement_sequence().into_iter().enumerate() {
        assert_eq!(
            after.insert_online_words("ni", "ni", 0, &batch, source),
            before.frozen_insert_online_words("ni", "ni", 0, &batch, source)
        );
        assert_eq!(
            after.series_cache.get_ref(&key).unwrap(),
            before.series_cache.get_ref(&key).unwrap()
        );
        let old_bytes = retained_bytes(before.series_cache.get_ref(&key).unwrap());
        let new_bytes = retained_bytes(after.series_cache.get_ref(&key).unwrap());
        println!("ONLINE_CAPACITY cache=quanpin step={step} before={held_before} old={old_bytes} new={new_bytes}");
        // 更新仅保留既有页的富余容量和至多十行的新批次，没有新增槽或无限增长。
        let maximum = held_before + 2 * 10 * std::mem::size_of::<WordItem>() + 1024;
        assert!(new_bytes <= maximum);
        assert!(
            new_bytes <= old_bytes,
            "cache={} step={step}: {new_bytes} > {old_bytes}",
            "quanpin"
        );
    }
}

// 只在临时关闭计数分配器后的 release 程序中手动运行，完整保留首轮配对观测。
#[test]
#[ignore]
fn online_cache_paired_latency() {
    let fixture = Fixture::new();
    for width in [16, 128, 512] {
        for variant in 0..3 {
            let source = if variant == 1 {
                CandidateSource::CloudSuggestion
            } else {
                CandidateSource::AiSuggestion
            };
            let batch = vec![if variant == 2 {
                "坏\n"
            } else {
                "合成计时回包"
            }
            .to_owned()];
            let rows = cached_rows(width, source);
            let mut before = QuanpinDictionary::new(&fixture.paths);
            let mut after = QuanpinDictionary::new(&fixture.paths);
            let key = slot(&mut before, "ni");
            assert_eq!(slot(&mut after, "ni"), key);
            before.series_cache.insert(key.clone(), rows.clone());
            after.series_cache.insert(key.clone(), rows.clone());
            assert_eq!(
                after.insert_online_words("ni", "ni", 0, &batch, source),
                before.frozen_insert_online_words("ni", "ni", 0, &batch, source)
            );
            assert_eq!(
                after.series_cache.get_ref(&key).unwrap(),
                before.series_cache.get_ref(&key).unwrap()
            );
            let mut iterations = 32;
            while time_batch(iterations, || {
                before.frozen_insert_online_words("ni", "ni", 0, &batch, source)
            }) < 2_000_000
                && iterations < 1 << 20
            {
                iterations *= 2;
            }
            for pair in 0..40 {
                let (old_ns, new_ns) = if pair % 2 == 0 {
                    (
                        time_batch(iterations, || {
                            before.frozen_insert_online_words("ni", "ni", 0, &batch, source)
                        }),
                        time_batch(iterations, || {
                            after.insert_online_words("ni", "ni", 0, &batch, source)
                        }),
                    )
                } else {
                    let new_ns = time_batch(iterations, || {
                        after.insert_online_words("ni", "ni", 0, &batch, source)
                    });
                    (
                        time_batch(iterations, || {
                            before.frozen_insert_online_words("ni", "ni", 0, &batch, source)
                        }),
                        new_ns,
                    )
                };
                println!("ONLINE_PAIR cache=quanpin width={width} variant={variant} pair={pair} iterations={iterations} old_ns={old_ns} new_ns={new_ns}");
            }
        }
    }
}

#[test]
fn cached_online_updates_keep_resolved_input_slots() {
    let fixture = Fixture::new();
    for (key, word) in [
        ("ni", "合成单词"),
        ("ni'hao", "合成短语"),
        ("nve", "合成别名"),
        ("gua", "合成换位"),
        ("gai", "合成邻键"),
    ] {
        fixture.insert(key, word, 100);
    }
    for (raw, segmentation, types) in [
        ("nihao", "ni'hao", 0),
        ("nihao", "", 0),
        ("ni'hao", "ni'hao", 0),
        ("nue", "nue", 0),
        (
            "gau",
            "",
            autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR,
        ),
    ] {
        let mut before = QuanpinDictionary::new(&fixture.paths);
        let mut after = QuanpinDictionary::new(&fixture.paths);
        let fuzzy = FuzzyPinyinOptions { rules: 0 };
        assert_eq!(
            after.query(raw, segmentation, types, fuzzy),
            before.query(raw, segmentation, types, fuzzy)
        );
        let cache_key = before.project_resolution(raw, segmentation, types, |resolution| {
            resolution.cache_key.clone()
        });
        if raw == "gau" {
            assert!(cache_key.starts_with("C:"));
        }
        let slot = before.series_slot(&cache_key);
        for source in [
            CandidateSource::AiSuggestion,
            CandidateSource::CloudSuggestion,
        ] {
            let pending = if source == CandidateSource::AiSuggestion {
                CandidateSource::AiPending
            } else {
                CandidateSource::CloudPending
            };
            for dictionary in [&mut before, &mut after] {
                dictionary
                    .series_cache
                    .get_mut(&slot)
                    .unwrap()
                    .push(WordItem::new(raw, "合成占位", 0, pending, ""));
            }
            let batch = vec!["合成异步回包".to_owned()];
            assert_eq!(
                after.insert_online_words(raw, segmentation, types, &batch, source),
                before.frozen_insert_online_words(raw, segmentation, types, &batch, source)
            );
            assert_eq!(
                after.series_cache.get_ref(&slot),
                before.series_cache.get_ref(&slot)
            );
            assert_eq!(
                after.query(raw, segmentation, types, fuzzy),
                before.query(raw, segmentation, types, fuzzy)
            );
        }
    }
}
