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
impl ShuangpinDictionary {
    fn frozen_insert_word_to_series_cache(
        &mut self,
        raw: &str,
        words: &[String],
        source: CandidateSource,
    ) -> bool {
        if raw.is_empty() {
            return false;
        }
        let key = self.sentence_cache_key(raw);
        let mut list = self.series_cache.get(&key).unwrap_or_default();
        if !replace_online_candidate_batch(&mut list, raw, words, source) {
            return false;
        }
        self.series_cache.insert(key, list);
        true
    }

    fn frozen_insert_word_to_active_helpcode_cache(
        &mut self,
        raw: &str,
        words: &[String],
        source: CandidateSource,
        help_codes: &str,
    ) -> bool {
        let insert = |cache: &mut FifoCache<String, Vec<WordItem>>, key: &String| {
            let Some(mut list) = cache.get(key) else {
                return false;
            };
            if !replace_online_candidate_batch(&mut list, raw, words, source) {
                return false;
            }
            cache.insert(key.clone(), list);
            true
        };
        if !help_codes.is_empty() {
            let key = self.sentence_cache_key(&double_helpcode_cache_key(raw, help_codes));
            return insert(&mut self.double_helpcode_cache, &key);
        }
        let key = self.sentence_cache_key(raw);
        let single = insert(&mut self.single_helpcode_cache, &key);
        let reversed = insert(&mut self.reversed_single_helpcode_cache, &key);
        single || reversed
    }
}

fn dictionary(fixture: &Fixture) -> ShuangpinDictionary {
    ShuangpinDictionary::new(
        crate::shuangpin::profile::profile(crate::types::ShuangpinProfileKind::Xiaohe).unwrap(),
        &fixture.paths,
    )
}

fn active_cache(
    dictionary: &mut ShuangpinDictionary,
    mode: usize,
) -> &mut FifoCache<String, Vec<WordItem>> {
    match mode {
        0 => &mut dictionary.series_cache,
        1 => &mut dictionary.single_helpcode_cache,
        2 => &mut dictionary.reversed_single_helpcode_cache,
        3 => &mut dictionary.double_helpcode_cache,
        _ => unreachable!(),
    }
}

fn key(dictionary: &ShuangpinDictionary, mode: usize, raw: &str) -> String {
    if mode == 3 {
        dictionary.sentence_cache_key(&double_helpcode_cache_key(raw, "ab"))
    } else {
        dictionary.sentence_cache_key(raw)
    }
}

fn update(
    dictionary: &mut ShuangpinDictionary,
    frozen: bool,
    mode: usize,
    raw: &str,
    batch: &[String],
    source: CandidateSource,
) -> bool {
    match (frozen, mode) {
        (false, 0) => dictionary.insert_word_to_series_cache(raw, batch, source),
        (true, 0) => dictionary.frozen_insert_word_to_series_cache(raw, batch, source),
        (false, _) => dictionary.insert_word_to_active_helpcode_cache(
            raw,
            batch,
            source,
            if mode == 3 { "ab" } else { "" },
        ),
        (true, _) => dictionary.frozen_insert_word_to_active_helpcode_cache(
            raw,
            batch,
            source,
            if mode == 3 { "ab" } else { "" },
        ),
    }
}

#[test]
fn cached_online_update_avoids_page_clones() {
    let fixture = Fixture::new();
    for mode in 0..4 {
        for width in [0, 1, 63, 64, 128] {
            for source in [
                CandidateSource::AiSuggestion,
                CandidateSource::CloudSuggestion,
            ] {
                let original = cached_rows(width, source);
                let (_, clone_cost) = count(|| original.clone());
                for (batch, batch_source) in batches() {
                    let mut before = dictionary(&fixture);
                    let mut after = dictionary(&fixture);
                    let key = key(&before, mode, "ni");
                    active_cache(&mut before, mode).insert(key.clone(), original.clone());
                    active_cache(&mut after, mode).insert(key.clone(), original.clone());
                    let (expected, old_count) =
                        count(|| update(&mut before, true, mode, "ni", &batch, batch_source));
                    let (actual, new_count) =
                        count(|| update(&mut after, false, mode, "ni", &batch, batch_source));
                    assert_eq!(actual, expected);
                    assert_eq!(
                        active_cache(&mut after, mode).get_ref(&key),
                        active_cache(&mut before, mode).get_ref(&key)
                    );
                    if batch_source == source || !actual {
                        let copied_key = usize::from(actual && mode != 0);
                        assert_eq!(
                            new_count + clone_cost + copied_key,
                            old_count,
                            "mode={mode} width={width} source={source:?} batch={batch_source:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn cached_online_update_keeps_fifo_and_owned_snapshots() {
    let fixture = Fixture::new();
    for mode in 0..4 {
        let mut dictionary = dictionary(&fixture);
        *active_cache(&mut dictionary, mode) = FifoCache::new(2);
        let first = key(&dictionary, mode, "ni");
        let second = key(&dictionary, mode, "hao");
        let third = key(&dictionary, mode, "ma");
        let original = cached_rows(3, CandidateSource::AiSuggestion);
        active_cache(&mut dictionary, mode).insert(first.clone(), original.clone());
        active_cache(&mut dictionary, mode).insert(second.clone(), original.clone());
        let snapshot = active_cache(&mut dictionary, mode).get(&first).unwrap();
        let pointers: Vec<_> = active_cache(&mut dictionary, mode).get_ref(&first).unwrap()[..3]
            .iter()
            .map(|row| row.word.as_ptr())
            .collect();
        assert!(update(
            &mut dictionary,
            false,
            mode,
            "ni",
            &["合成新在线".to_owned()],
            CandidateSource::AiSuggestion
        ));
        assert_eq!(snapshot, original);
        for (row, pointer) in active_cache(&mut dictionary, mode).get_ref(&first).unwrap()[..3]
            .iter()
            .zip(pointers)
        {
            assert_eq!(row.word.as_ptr(), pointer);
        }
        active_cache(&mut dictionary, mode).insert(third.clone(), Vec::new());
        assert!(active_cache(&mut dictionary, mode)
            .get_ref(&first)
            .is_none());
        assert!(active_cache(&mut dictionary, mode)
            .get_ref(&second)
            .is_some());
        assert!(active_cache(&mut dictionary, mode)
            .get_ref(&third)
            .is_some());
        dictionary.reset_cache();
        assert_eq!(snapshot, original);
    }
}

#[test]
fn missing_online_slots_keep_success_and_rejection_policy() {
    let fixture = Fixture::new();
    for mode in 0..4 {
        for (batch, source) in batches() {
            let mut before = dictionary(&fixture);
            let mut after = dictionary(&fixture);
            let key = key(&before, mode, "ni");
            let (expected, old_count) =
                count(|| update(&mut before, true, mode, "ni", &batch, source));
            let (actual, new_count) =
                count(|| update(&mut after, false, mode, "ni", &batch, source));
            assert_eq!((actual, new_count), (expected, old_count));
            assert_eq!(
                active_cache(&mut after, mode).get_ref(&key),
                active_cache(&mut before, mode).get_ref(&key)
            );
        }
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
    for mode in 0..4 {
        let _ = mode;
        let rows = cached_rows(16, CandidateSource::AiSuggestion);
        let mut before = dictionary(&fixture);
        let mut after = dictionary(&fixture);
        let key = key(&before, mode, "ni");
        active_cache(&mut before, mode).insert(key.clone(), rows.clone());
        active_cache(&mut after, mode).insert(key.clone(), rows.clone());
        let held_before = retained_bytes(active_cache(&mut after, mode).get_ref(&key).unwrap());
        for (step, (batch, source)) in replacement_sequence().into_iter().enumerate() {
            assert_eq!(
                update(&mut after, false, mode, "ni", &batch, source),
                update(&mut before, true, mode, "ni", &batch, source)
            );
            assert_eq!(
                active_cache(&mut after, mode).get_ref(&key).unwrap(),
                active_cache(&mut before, mode).get_ref(&key).unwrap()
            );
            let old_bytes = retained_bytes(active_cache(&mut before, mode).get_ref(&key).unwrap());
            let new_bytes = retained_bytes(active_cache(&mut after, mode).get_ref(&key).unwrap());
            println!("ONLINE_CAPACITY cache={} step={step} before={held_before} old={old_bytes} new={new_bytes}", ["shuangpin", "single", "reversed", "double"][mode]);
            // 更新仅保留既有页的富余容量和至多十行的新批次，没有新增槽或无限增长。
            let maximum = held_before + 2 * 10 * std::mem::size_of::<WordItem>() + 1024;
            assert!(new_bytes <= maximum);
            assert!(
                new_bytes <= old_bytes,
                "cache={} step={step}: {new_bytes} > {old_bytes}",
                ["shuangpin", "single", "reversed", "double"][mode]
            );
        }
    }
}

// 只在临时关闭计数分配器后的 release 程序中手动运行，完整保留首轮配对观测。
#[test]
#[ignore]
fn online_cache_paired_latency() {
    let fixture = Fixture::new();
    for mode in 0..4 {
        let _ = mode;
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
                let mut before = dictionary(&fixture);
                let mut after = dictionary(&fixture);
                let key = key(&before, mode, "ni");
                active_cache(&mut before, mode).insert(key.clone(), rows.clone());
                active_cache(&mut after, mode).insert(key.clone(), rows.clone());
                assert_eq!(
                    update(&mut after, false, mode, "ni", &batch, source),
                    update(&mut before, true, mode, "ni", &batch, source)
                );
                assert_eq!(
                    active_cache(&mut after, mode).get_ref(&key).unwrap(),
                    active_cache(&mut before, mode).get_ref(&key).unwrap()
                );
                let mut iterations = 32;
                while time_batch(iterations, || {
                    update(&mut before, true, mode, "ni", &batch, source)
                }) < 2_000_000
                    && iterations < 1 << 20
                {
                    iterations *= 2;
                }
                for pair in 0..40 {
                    let (old_ns, new_ns) = if pair % 2 == 0 {
                        (
                            time_batch(iterations, || {
                                update(&mut before, true, mode, "ni", &batch, source)
                            }),
                            time_batch(iterations, || {
                                update(&mut after, false, mode, "ni", &batch, source)
                            }),
                        )
                    } else {
                        let new_ns = time_batch(iterations, || {
                            update(&mut after, false, mode, "ni", &batch, source)
                        });
                        (
                            time_batch(iterations, || {
                                update(&mut before, true, mode, "ni", &batch, source)
                            }),
                            new_ns,
                        )
                    };
                    println!("ONLINE_PAIR cache={} width={width} variant={variant} pair={pair} iterations={iterations} old_ns={old_ns} new_ns={new_ns}", ["shuangpin", "single", "reversed", "double"][mode]);
                }
            }
        }
    }
}

#[test]
fn single_and_reversed_helpcode_slots_update_independently() {
    let fixture = Fixture::new();
    for single in [false, true] {
        for reversed in [false, true] {
            for (batch, source) in batches() {
                let mut before = dictionary(&fixture);
                let mut after = dictionary(&fixture);
                let key = key(&before, 1, "ni");
                for (mode, present) in [(1, single), (2, reversed)] {
                    if present {
                        let rows = cached_rows(mode, CandidateSource::AiSuggestion);
                        active_cache(&mut before, mode).insert(key.clone(), rows.clone());
                        active_cache(&mut after, mode).insert(key.clone(), rows);
                    }
                }
                assert_eq!(
                    update(&mut after, false, 1, "ni", &batch, source),
                    update(&mut before, true, 1, "ni", &batch, source)
                );
                for mode in [1, 2] {
                    assert_eq!(
                        active_cache(&mut after, mode).get_ref(&key),
                        active_cache(&mut before, mode).get_ref(&key)
                    );
                }
            }
        }
    }
}
