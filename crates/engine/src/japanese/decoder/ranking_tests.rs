use super::*;
use crate::ime::personal_rerank::allocations::measure;
use std::cell::Cell;

fn synthetic_cost(id: u32) -> i32 {
    match id % 17 {
        0 => i32::MIN,
        1 => i32::MAX,
        _ => (id.wrapping_mul(13) % 7) as i32 - 3,
    }
}

#[test]
fn ranked_heap_reads_cost_once_per_scanned_id() {
    for length in [0, 1, 2, 8, 24, 70, 256] {
        let mut ids: Vec<u32> = (0..length).map(|id| id % 37).collect();
        ids.reverse();
        for limit in [0, 1, 2, 8, 24, 64, 65, 300] {
            let calls = Cell::new(0);
            let actual = best_ids_from_iter(ids.iter().copied(), limit, |id| {
                calls.set(calls.get() + 1);
                synthetic_cost(id)
            });
            let expected =
                ranking_reference::best_ids_from_iter(ids.iter().copied(), limit, synthetic_cost);
            assert_eq!(actual, expected);
            assert_eq!(
                calls.get(),
                if limit == 0 { 0 } else { ids.len() },
                "成本只能在扫描时读取：条目={length}，限额={limit}"
            );
        }
    }
    for seed in 0..16_u64 {
        let mut ids: Vec<u32> = (0..256).map(|id| id % 73).collect();
        ids.extend([u32::MAX, 0, u32::MAX - 1]);
        let mut state = seed;
        for index in (1..ids.len()).rev() {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            ids.swap(index, state as usize % (index + 1));
        }
        for limit in [0, 1, 8, 24, 64, 300] {
            let calls = Cell::new(0);
            let actual = best_ids_from_iter(ids.iter().copied(), limit, |id| {
                calls.set(calls.get() + 1);
                synthetic_cost(id)
            });
            let mut expected = ids.clone();
            expected.sort_unstable_by_key(|&id| (synthetic_cost(id), id));
            expected.truncate(limit);
            assert_eq!(actual, expected);
            assert_eq!(calls.get(), if limit == 0 { 0 } else { ids.len() });
        }
    }
    assert!(best_ids_from_iter(
        std::iter::from_fn(|| -> Option<u32> { panic!("零限额不读取输入") }),
        0,
        |_| panic!("零限额不读取成本")
    )
    .is_empty());
}

#[test]
fn ranked_heap_bounds_peak_without_duplicate_id_storage() {
    eprintln!(
        "排名堆项字节：旧={}，单键={}",
        std::mem::size_of::<((i32, u32), u32)>(),
        std::mem::size_of::<(i32, u32)>()
    );
    for length in [1, 3, 24, 70, 256] {
        for limit in [1, 2, 8, 24, 64, 65, 300] {
            let (expected, old) =
                measure(|| ranking_reference::best_ids_from_iter(0..length, limit, synthetic_cost));
            let (actual, new) = measure(|| best_ids_from_iter(0..length, limit, synthetic_cost));
            assert_eq!(actual, expected);
            assert!(new.allocations <= old.allocations);
            assert!(new.remaining_bytes <= old.remaining_bytes);
            assert_eq!(old.minimum_bytes, 0);
            assert_eq!(new.minimum_bytes, 0);
            eprintln!(
                "排名堆：条目={length} 限额={limit} 分配={}→{} 峰值={}→{} 返回={}→{}",
                old.allocations,
                new.allocations,
                old.peak_bytes,
                new.peak_bytes,
                old.remaining_bytes,
                new.remaining_bytes
            );
            assert!(new.peak_bytes < old.peak_bytes);
        }
    }
}

fn fixture(length: usize) -> JapaneseDictionary {
    let surfaces: Vec<_> = (0..length).map(|id| format!("合成{id}😀")).collect();
    let entries: Vec<_> = surfaces
        .iter()
        .enumerate()
        .map(|(id, text)| {
            (
                match id % 5 {
                    0 => "か",
                    1 => "かな",
                    2 => "かない",
                    3 => "かん",
                    _ => "甲😀",
                },
                text.as_str(),
                0,
                0,
                synthetic_cost(id as u32),
            )
        })
        .collect();
    let mut entries = entries;
    entries.sort_by_key(|entry| entry.0);
    JapaneseDictionary::from_bytes(test_model::bytes(&entries, 1, &[0]).into_boxed_slice())
        .expect("合成排名词库")
}

#[derive(Debug, Clone, Copy)]
enum Query<'a> {
    Exact(&'a str),
    Prefix(&'a str),
    Continuing(&'a str, &'a [&'a str]),
}

fn independently_ranked<'a>(
    dictionary: &'a JapaneseDictionary,
    lookup: Query<'_>,
    limit: usize,
) -> Vec<JapaneseLemmaRef<'a>> {
    let mut ids = Vec::new();
    match lookup {
        Query::Exact(reading) if !reading.is_empty() => {
            ids.extend(
                (0..dictionary.token_count as u32)
                    .filter(|&id| dictionary.lemma_ref(id).reading == reading),
            );
        }
        Query::Prefix(prefix) if !prefix.is_empty() => {
            ids.extend(
                (0..dictionary.token_count as u32)
                    .filter(|&id| dictionary.lemma_ref(id).reading.starts_with(prefix)),
            );
        }
        Query::Continuing(prefix, suffixes) if !prefix.is_empty() => {
            for suffix in suffixes {
                ids.extend((0..dictionary.token_count as u32).filter(|&id| {
                    dictionary
                        .lemma_ref(id)
                        .reading
                        .strip_prefix(prefix)
                        .is_some_and(|remaining| {
                            !remaining.is_empty() && remaining.starts_with(suffix)
                        })
                }));
            }
        }
        _ => {}
    }
    ids.sort_unstable_by_key(|&id| (dictionary.lemma_ref(id).word_cost, id));
    ids.truncate(limit);
    ids.into_iter().map(|id| dictionary.lemma_ref(id)).collect()
}

fn query<'a>(
    dictionary: &'a JapaneseDictionary,
    query: Query<'_>,
    limit: usize,
    reference: bool,
) -> Vec<JapaneseLemmaRef<'a>> {
    match (query, reference) {
        (Query::Exact(reading), false) => dictionary.exact_lemma_views(reading, limit),
        (Query::Exact(reading), true) => {
            dictionary
                .ranked_reference_exact_lemmas_with(reading, limit, |id| dictionary.lemma_ref(id))
        }
        (Query::Prefix(prefix), false) => dictionary.prefix_lemma_views(prefix, limit),
        (Query::Prefix(prefix), true) => {
            dictionary
                .ranked_reference_prefix_lemmas_with(prefix, limit, |id| dictionary.lemma_ref(id))
        }
        (Query::Continuing(prefix, suffixes), false) => {
            dictionary.continuing_lemma_views(prefix, suffixes, limit)
        }
        (Query::Continuing(prefix, suffixes), true) => {
            dictionary.ranked_reference_continuing_with(prefix, suffixes, limit, |id| {
                dictionary.lemma_ref(id)
            })
        }
    }
}

#[test]
fn ranked_heap_complete_queries_preserve_fields_overlaps_and_borrows() {
    for length in [5, 70, 350] {
        let dictionary = fixture(length);
        for lookup in [
            Query::Exact(""),
            Query::Exact("く"),
            Query::Exact("かな"),
            Query::Exact("甲😀"),
            Query::Prefix(""),
            Query::Prefix("く"),
            Query::Prefix("か"),
            Query::Prefix("かな"),
            Query::Prefix("甲😀"),
            Query::Continuing("", &["な"]),
            Query::Continuing("く", &["な"]),
            Query::Continuing("か", &[]),
            Query::Continuing("か", &[""]),
            Query::Continuing("か", &["な", "ない", "な", "", "ん"]),
            Query::Continuing("か", &["な", "ない", "ん"]),
            Query::Continuing("か", &["無", "ん"]),
            Query::Continuing("甲", &["😀"]),
        ] {
            for limit in [0, 1, 2, 8, 24, 64, 65, 70, 100, 400] {
                let (expected, old) = measure(|| query(&dictionary, lookup, limit, true));
                let (actual, new) = measure(|| query(&dictionary, lookup, limit, false));
                assert_eq!(actual, expected, "条目={length}，{lookup:?}，限额={limit}");
                assert_eq!(actual, independently_ranked(&dictionary, lookup, limit));
                assert_eq!(actual, heap_query(&dictionary, lookup, limit));
                assert!(new.allocations <= old.allocations);
                assert_eq!(new.remaining_bytes, old.remaining_bytes);
                assert_eq!(new.minimum_bytes, 0);
                assert_eq!(old.minimum_bytes, 0);
                assert!(new.peak_bytes <= old.peak_bytes);
            }
        }
        let mut saved = None;
        {
            let reading = String::from("かな");
            dictionary.for_each_exact_lemma_view(&reading, 1, |view| saved = Some(view));
        }
        assert!(saved.is_some());
        let saved = saved.unwrap();
        assert_eq!(saved, dictionary.lemma_ref(saved.token_id));
        for (suffixes, limit) in [(&[""][..], 48), (&["な", "ない", "ん"][..], 48)] {
            let (expected, old) = measure(|| {
                dictionary.ranked_reference_continuing_with("か", suffixes, limit, |_| ())
            });
            let (actual, new) =
                measure(|| dictionary.prefix_lemmas_continuing_with("か", suffixes, limit, |_| ()));
            assert_eq!(actual, expected);
            assert!(new.allocations <= old.allocations);
            assert_eq!(new.remaining_bytes, 0);
            assert_eq!(old.remaining_bytes, 0);
            assert!(new.peak_bytes < old.peak_bytes);
        }
    }
}

fn heap_query<'a>(
    dictionary: &'a JapaneseDictionary,
    query: Query<'_>,
    limit: usize,
) -> Vec<JapaneseLemmaRef<'a>> {
    match query {
        Query::Exact(reading) => {
            dictionary
                .ranked_heap_sort_exact_lemmas_with(reading, limit, |id| dictionary.lemma_ref(id))
        }
        Query::Prefix(prefix) => {
            dictionary
                .ranked_heap_sort_prefix_lemmas_with(prefix, limit, |id| dictionary.lemma_ref(id))
        }
        Query::Continuing(prefix, suffixes) => dictionary
            .ranked_heap_sort_prefix_lemmas_continuing_with(prefix, suffixes, limit, |id| {
                dictionary.lemma_ref(id)
            }),
    }
}

#[inline(never)]
fn paired_batch(
    dictionary: &JapaneseDictionary,
    lookup: Query<'_>,
    limit: usize,
    strategy: usize,
    iterations: usize,
) -> std::time::Duration {
    use std::hint::black_box;
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        drop(black_box(match strategy {
            0 => query(
                black_box(dictionary),
                black_box(lookup),
                black_box(limit),
                true,
            ),
            1 => query(
                black_box(dictionary),
                black_box(lookup),
                black_box(limit),
                false,
            ),
            2 => heap_query(black_box(dictionary), black_box(lookup), black_box(limit)),
            _ => unreachable!(),
        }));
    }
    start.elapsed()
}

fn percentiles(mut samples: Vec<f64>) -> [f64; 3] {
    samples.sort_unstable_by(f64::total_cmp);
    [10, 50, 90].map(|percent| samples[(samples.len() - 1) * percent / 100])
}

fn compare_ranked_query(
    dictionary: &JapaneseDictionary,
    lookup: Query<'_>,
    limit: usize,
    iterations: usize,
    label: &str,
    independent: bool,
) {
    use std::hint::black_box;
    let permutations = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let (expected, old) = measure(|| query(dictionary, lookup, limit, true));
    let (actual, new) = measure(|| query(dictionary, lookup, limit, false));
    let (heap, alternative) = measure(|| heap_query(dictionary, lookup, limit));
    assert_eq!(actual, expected);
    assert_eq!(heap, expected);
    if independent {
        assert_eq!(actual, independently_ranked(dictionary, lookup, limit));
    }
    assert!(new.allocations <= old.allocations);
    assert!(new.peak_bytes <= old.peak_bytes);
    assert_eq!(new.remaining_bytes, old.remaining_bytes);
    assert_eq!(new.minimum_bytes, 0);
    assert_eq!(alternative.minimum_bytes, 0);
    assert_eq!(old.minimum_bytes, 0);
    for strategy in 0..3 {
        black_box(paired_batch(
            dictionary, lookup, limit, strategy, iterations,
        ));
    }
    let mut groups = Vec::with_capacity(240);
    for group in 0..240 {
        let mut times = [std::time::Duration::ZERO; 3];
        // 三种策略的六种排列各四十次；每个位置及每对先后均衡。
        for strategy in permutations[group % permutations.len()] {
            times[strategy] = paired_batch(dictionary, lookup, limit, strategy, iterations);
        }
        groups.push(times);
    }
    let absolute = std::array::from_fn::<_, 3, _>(|index| {
        percentiles(
            groups
                .iter()
                .map(|group| group[index].as_secs_f64() * 1e6)
                .collect(),
        )
    });
    let ratios = [(1, 0), (2, 0), (1, 2)].map(|(numerator, denominator)| {
        percentiles(
            groups
                .iter()
                .map(|group| {
                    assert!(!group[denominator].is_zero());
                    group[numerator].as_secs_f64() / group[denominator].as_secs_f64()
                })
                .collect(),
        )
    });
    eprintln!("排名输出配对：样本={label} 查询={lookup:?} 限额={limit} iterations={iterations} groups={} batches_us_p10_p50_p90={absolute:.3?} production_old={:.4?} heap_old={:.4?} production_heap={:.4?} 分配={:?} 峰值={:?}", groups.len(), ratios[0], ratios[1], ratios[2], [old.allocations, new.allocations, alternative.allocations], [old.peak_bytes, new.peak_bytes, alternative.peak_bytes]);
}

#[test]
#[ignore = "本地 release 三种完整排名查询短配对；不设 CI 时间阈值"]
fn benchmark_ranked_heap_output_paired() {
    for (length, lookup, iterations) in [
        (5, Query::Exact(""), 512),
        (5, Query::Exact("く"), 128),
        (5, Query::Exact("かな"), 64),
        (5, Query::Prefix("か"), 128),
        (350, Query::Exact("かな"), 16),
        (350, Query::Prefix("かな"), 16),
        (350, Query::Prefix("か"), 16),
        (350, Query::Continuing("く", &[""]), 128),
        (350, Query::Continuing("か", &[""]), 8),
        (350, Query::Continuing("か", &["な", "ない", "ん"]), 8),
        (3500, Query::Exact("甲😀"), 4),
        (3500, Query::Continuing("か", &["な", "ない", "ん"]), 2),
    ] {
        let dictionary = fixture(length);
        for limit in [1, 24, 64, 65] {
            compare_ranked_query(
                &dictionary,
                lookup,
                limit,
                iterations,
                &format!("合成/{length}"),
                true,
            );
        }
    }
}

#[test]
#[ignore = "本地 release 校验后的发布词库排名查询配对；需要显式模型路径，不设 CI 时间阈值"]
fn benchmark_ranked_heap_published_queries() {
    let Some(path) = std::env::var_os("MSIME_JAPANESE_BENCH_MODEL") else {
        eprintln!("skipped: MSIME_JAPANESE_BENCH_MODEL 未指向校验过的公开发布模型");
        return;
    };
    let dictionary = JapaneseDictionary::load(Path::new(&path)).expect("已校验的公开词库");
    // 查询全部为测试中固定的合成字符串，不读取用户输入或个人词条。
    for (lookup, iterations) in [
        (Query::Exact("か"), 16),
        (Query::Exact("し"), 16),
        (Query::Exact("かな"), 16),
        (Query::Exact("かんじ"), 16),
        (Query::Exact("にほん"), 16),
        (Query::Prefix("か"), 1),
        (Query::Prefix("かん"), 2),
        (Query::Prefix("しし"), 8),
        (Query::Continuing("か", &["な", "ない", "ん"]), 1),
        (Query::Continuing("かん", &["じ", "し", "じゃ"]), 2),
        (Query::Prefix("合成未存在😀"), 64),
    ] {
        for limit in [1, 16, 24, 48, 65] {
            compare_ranked_query(&dictionary, lookup, limit, iterations, "发布词库", false);
        }
    }
}
