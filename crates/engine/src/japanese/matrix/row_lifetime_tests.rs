use super::super::romaji::convert_romaji;
use super::consumed_row_tests::fixture;
use super::*;
use crate::ime::personal_rerank::allocations::{measure, Measurement};
use std::hint::black_box;

#[derive(Clone, Copy)]
enum RowLifetime {
    Retain,
    Drop,
    Clear,
}

#[derive(Clone, Copy)]
enum Strategy {
    Retain,
    Drop,
    Clear,
    Production,
}

const STRATEGIES: [Strategy; 4] = [
    Strategy::Retain,
    Strategy::Drop,
    Strategy::Clear,
    Strategy::Production,
];

// 冻结 4c867d6be89579e603938797b18ba2369a2ffbfb 正文，共同核心只改变消费完毕的行生命周期。
#[inline(never)]
fn row_lifetime_search(
    dictionary: &JapaneseDictionary,
    conversion: &RomajiConversion,
    limit: usize,
    lifetime: RowLifetime,
) -> Vec<JapaneseConversion> {
    let mut output = Output {
        items: Vec::with_capacity(limit),
        limit,
    };
    let reading = conversion.hiragana.as_str();
    let pending = conversion.pending.as_str();
    let limit = output.limit;
    if limit == 0 {
        return output.items;
    }
    let pending_kana = kana_for_romaji_prefix_view(pending);
    if reading.is_empty() {
        for kana in pending_kana {
            dictionary.for_each_prefix_lemma_view(kana, 24, |lemma| {
                output.push(lemma.surface, i64::from(lemma.word_cost));
            });
            if output.full() {
                return output.items;
            }
        }
        return output.items;
    }

    let boundaries: Vec<usize> = reading
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(reading.len()))
        .collect();
    let mora_count = boundaries.len() - 1;
    let mut rows: Vec<Row> = (0..=mora_count).map(|_| Row::new()).collect();
    rows[0].nodes.push(Node {
        text: String::new(),
        cost: 0,
        right_id: 0,
    });

    for start in 0..mora_count {
        if rows[start].nodes.is_empty() {
            continue;
        }
        let mut previous_row = std::mem::take(&mut rows[start]);
        let start_byte = boundaries[start];
        let max_end = mora_count.min(start + MAX_LEMMA_MORA);
        for end in start + 1..=max_end {
            let key = &reading[start_byte..boundaries[end]];
            dictionary.for_each_exact_lemma_view(key, 24, |lemma| {
                for previous in &previous_row.nodes {
                    let cost = previous.cost
                        + i64::from(lemma.word_cost)
                        + i64::from(dictionary.connection_cost(previous.right_id, lemma.left_id));
                    rows[end].extend(&previous.text, lemma.surface, cost, lemma.right_id);
                }
            });
        }

        let kana = &reading[start_byte..boundaries[start + 1]];
        for previous in &previous_row.nodes {
            rows[start + 1].extend(
                &previous.text,
                kana,
                previous.cost + i64::from(UNKNOWN_KANA_COST),
                0,
            );
        }

        match lifetime {
            RowLifetime::Retain => rows[start] = previous_row,
            RowLifetime::Drop => drop(previous_row),
            RowLifetime::Clear => {
                previous_row.nodes.clear();
                rows[start] = previous_row;
            }
        }
    }

    let mut finals = std::mem::take(&mut rows[mora_count]).nodes;
    for node in &mut finals {
        node.cost += i64::from(dictionary.connection_cost(node.right_id, 0));
    }
    finals.sort_by_key(|node| node.cost);
    let mut finals = finals.into_iter();
    if let Some(best) = finals.next() {
        output.push_owned(best.text, best.cost);
    }

    if !pending.is_empty() {
        if let Some(first) = pending_kana.first() {
            let suffix_capacity = if pending_kana.len() == 1 {
                first.len()
            } else {
                pending_kana
                    .iter()
                    .map(|kana| kana.len())
                    .max()
                    .unwrap_or(0)
            };
            let mut key = String::with_capacity(reading.len() + suffix_capacity);
            key.push_str(reading);
            for kana in pending_kana {
                // 只替换假名后缀，容量覆盖最长后缀，避免循环中扩容。
                key.truncate(reading.len());
                key.push_str(kana);
                dictionary.for_each_exact_lemma_view(&key, 16, |lemma| {
                    output.push(lemma.surface, i64::from(lemma.word_cost));
                });
            }
        }
        dictionary.for_each_continuing_lemma_view(reading, pending_kana, 48, |lemma| {
            output.push(lemma.surface, i64::from(lemma.word_cost));
        });
    }

    for node in finals {
        output.push_owned(node.text, node.cost);
    }

    for end in (1..=mora_count).rev() {
        if output.full() {
            break;
        }
        dictionary.for_each_exact_lemma_view(&reading[..boundaries[end]], 16, |lemma| {
            output.push(lemma.surface, i64::from(lemma.word_cost));
        });
    }
    output.items
}

#[inline(never)]
fn run_search(
    dictionary: &JapaneseDictionary,
    conversion: &RomajiConversion,
    limit: usize,
    strategy: Strategy,
) -> Vec<JapaneseConversion> {
    match strategy {
        Strategy::Production => search_converted(dictionary, conversion, limit),
        Strategy::Retain => row_lifetime_search(dictionary, conversion, limit, RowLifetime::Retain),
        Strategy::Drop => row_lifetime_search(dictionary, conversion, limit, RowLifetime::Drop),
        Strategy::Clear => row_lifetime_search(dictionary, conversion, limit, RowLifetime::Clear),
    }
}

fn verify_case(
    dictionary: &JapaneseDictionary,
    conversion: &RomajiConversion,
    limit: usize,
) -> [Measurement; 4] {
    let expected = search_converted(dictionary, conversion, limit);
    // 每个策略预热和量化都在计时之外，不释放测量前的拥有存储。
    for strategy in STRATEGIES {
        drop(run_search(dictionary, conversion, limit, strategy));
    }
    let measurements = STRATEGIES.map(|strategy| {
        let (actual, measured) = measure(|| run_search(dictionary, conversion, limit, strategy));
        assert_eq!(actual, expected, "limit={limit}");
        assert_eq!(measured.minimum_bytes, 0);
        assert!(measured.remaining_bytes >= 0);
        measured
    });
    let [retain, release, clear, production] = measurements;
    for measured in [release, clear, production] {
        assert_eq!(measured.allocations, retain.allocations);
        assert_eq!(measured.remaining_bytes, retain.remaining_bytes);
    }
    assert_eq!(production.peak_bytes, release.peak_bytes);
    assert!(release.peak_bytes <= clear.peak_bytes);
    assert!(clear.peak_bytes <= retain.peak_bytes);
    if conversion.hiragana.chars().count() >= 32 && limit > 0 {
        assert!(release.peak_bytes < retain.peak_bytes);
        assert!(clear.peak_bytes < retain.peak_bytes);
    }
    measurements
}

#[test]
fn row_lifetime_strategies_preserve_queries_and_allocation_boundaries() {
    for dense in [false, true] {
        let dictionary = fixture(dense);
        for reading in [
            "",
            "か",
            "かき",
            "くく",
            "甲😀か",
            &"か".repeat(32),
            &"😀".repeat(64),
        ] {
            for pending in ["", "k", "K", "sh", "xt", "d", "z", "q", "あ", "toolong"] {
                let conversion = RomajiConversion {
                    hiragana: reading.to_owned(),
                    pending: pending.to_owned(),
                    complete: pending.is_empty(),
                };
                for limit in [0, 1, 2, 8, 16, 32] {
                    verify_case(&dictionary, &conversion, limit);
                }
            }
        }
    }
}

#[test]
fn row_lifetime_unknown_unicode_records_three_peak_tradeoffs() {
    let dictionary = fixture(false);
    for length in [32, 128, 512] {
        let conversion = RomajiConversion {
            hiragana: "😀".repeat(length),
            pending: String::new(),
            complete: true,
        };
        for limit in [1, 16] {
            let [retain, release, clear, production] = verify_case(&dictionary, &conversion, limit);
            assert!(release.peak_bytes < clear.peak_bytes);
            assert!(release.peak_bytes <= length * 400 + 2048);
            assert_eq!(production.peak_bytes, release.peak_bytes);
            eprintln!("行生命周期内存：字符={length} 限额={limit} 保留={} 释放={} 清空={} 生产={} 分配={}", retain.peak_bytes, release.peak_bytes, clear.peak_bytes, production.peak_bytes, release.allocations);
        }
    }
}

fn balanced_orders() -> [[usize; 4]; 24] {
    let mut orders = [[0; 4]; 24];
    let mut next = 0;
    for first in 0..4 {
        for second in 0..4 {
            for third in 0..4 {
                for fourth in 0..4 {
                    let order = [first, second, third, fourth];
                    if first != second
                        && first != third
                        && first != fourth
                        && second != third
                        && second != fourth
                        && third != fourth
                    {
                        orders[next] = order;
                        next += 1;
                    }
                }
            }
        }
    }
    orders
}

#[test]
fn row_lifetime_orders_balance_positions_and_pair_precedence() {
    let orders = balanced_orders();
    let mut unique = orders;
    unique.sort_unstable();
    assert!(unique.windows(2).all(|pair| pair[0] != pair[1]));
    for strategy in 0..4 {
        for position in 0..4 {
            assert_eq!(
                orders
                    .iter()
                    .filter(|order| order[position] == strategy)
                    .count(),
                6
            );
        }
        for other in 0..4 {
            if strategy == other {
                continue;
            }
            assert_eq!(
                orders
                    .iter()
                    .filter(|order| {
                        order.iter().position(|index| *index == strategy)
                            < order.iter().position(|index| *index == other)
                    })
                    .count(),
                12
            );
        }
    }
}

fn percentiles(mut samples: Vec<f64>) -> [f64; 3] {
    samples.sort_unstable_by(f64::total_cmp);
    [10, 50, 90].map(|percent| samples[(samples.len() - 1) * percent / 100])
}

#[inline(never)]
fn time_batch(
    dictionary: &JapaneseDictionary,
    conversion: &RomajiConversion,
    limit: usize,
    strategy: Strategy,
    iterations: usize,
) -> std::time::Duration {
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        drop(black_box(run_search(
            black_box(dictionary),
            black_box(conversion),
            black_box(limit),
            black_box(strategy),
        )));
    }
    start.elapsed()
}

#[test]
#[ignore = "本地 release 短批次配对计时；不设置 CI 时间阈值"]
fn benchmark_row_lifetime_paired_timings() {
    use std::time::{Duration, Instant};

    // 空计时区间仅量化读取开销；不把它从搜索耗时中扣除。
    let mut timer_samples = Vec::with_capacity(256);
    for _ in 0..256 {
        let start = Instant::now();
        let elapsed = black_box(start.elapsed());
        timer_samples.push(elapsed.as_nanos() as f64);
    }
    eprintln!(
        "行生命周期时钟开销：p10/p50/p90_ns={:?}",
        percentiles(timer_samples)
    );
    let orders = balanced_orders();
    for (name, dense, input, iterations) in [
        ("稀疏单假名", false, "ka".to_owned(), 64),
        ("未知双假名", false, "kiki".to_owned(), 64),
        ("密集单假名", true, "ka".to_owned(), 64),
        ("密集双假名", true, "kaka".to_owned(), 32),
        ("稀疏32", false, "ka".repeat(32), 4),
        ("密集32", true, "ka".repeat(32), 2),
        ("未知128", false, "😀".repeat(128), 4),
        ("未知512", false, "😀".repeat(512), 1),
        ("假名及待定", false, "kak".to_owned(), 64),
        ("仅待定", false, "k".to_owned(), 256),
        ("空", false, "".to_owned(), 2048),
    ] {
        let dictionary = fixture(dense);
        let conversion = if input.starts_with('😀') {
            RomajiConversion {
                hiragana: input,
                pending: String::new(),
                complete: true,
            }
        } else {
            convert_romaji(&input)
        };
        for limit in [0, 1, 16] {
            let iterations = if limit == 0 { 2048 } else { iterations };
            let measured = verify_case(&dictionary, &conversion, limit);
            // 所有统计写入都在区间之外；四种策略都预热完整短批次。
            for strategy in STRATEGIES {
                black_box(time_batch(
                    &dictionary,
                    &conversion,
                    limit,
                    strategy,
                    iterations,
                ));
            }
            let mut groups = Vec::with_capacity(240);
            for cycle in 0..10 {
                for ordinal in 0..orders.len() {
                    let order = orders[(ordinal + cycle * 7) % orders.len()];
                    let mut timings = [Duration::ZERO; 4];
                    for index in order {
                        timings[index] = time_batch(
                            &dictionary,
                            &conversion,
                            limit,
                            STRATEGIES[index],
                            iterations,
                        );
                    }
                    groups.push(timings);
                }
            }
            let absolute = std::array::from_fn::<_, 4, _>(|index| {
                percentiles(
                    groups
                        .iter()
                        .map(|group| group[index].as_secs_f64() * 1e6)
                        .collect(),
                )
            });
            // 比值从同组计算，然后排序；不拿两份独立中位数相除。
            let ratios =
                [(1, 0), (2, 0), (3, 0), (3, 1), (2, 1), (3, 2)].map(|(numerator, denominator)| {
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
            eprintln!("行生命周期配对：case={name} limit={limit} iterations={iterations} groups={} batches_us_p10_p50_p90={absolute:.3?} drop_retain={:.4?} clear_retain={:.4?} production_retain={:.4?} production_drop={:.4?} clear_drop={:.4?} production_clear={:.4?} peak={:?} allocations={}", groups.len(), ratios[0], ratios[1], ratios[2], ratios[3], ratios[4], ratios[5], measured.map(|measurement| measurement.peak_bytes), measured[0].allocations);
        }
    }
}
