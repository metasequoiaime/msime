use super::super::decoder::test_model;
use super::super::romaji::convert_romaji;
use super::*;
use crate::ime::personal_rerank::allocations::{measure, Measurement};

// 冻结 0f6bf002a 的完整搜索正文，保留已消费行到查询结束，只隔离释放时机。
fn retained_reference_search(
    dictionary: &JapaneseDictionary,
    conversion: &RomajiConversion,
    limit: usize,
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
            for lemma in dictionary.prefix_lemma_views(kana, 24) {
                output.push(lemma.surface, i64::from(lemma.word_cost));
                if output.full() {
                    return output.items;
                }
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
        let previous_row = std::mem::take(&mut rows[start]);
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

        rows[start] = previous_row;
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

fn fixture(dense: bool) -> JapaneseDictionary {
    let surfaces: Vec<_> = (0..24).map(|index| format!("合成語{index:02}")).collect();
    let mut entries = vec![
        ("か", "仮", 0, 1, -500),
        ("か", "仮", 1, 0, 10),
        ("かか", "仮仮", 1, 0, -700),
        ("かかかかかかかかかかかかかかかか", "合成十六", 0, 1, -800),
        ("かき", "仮木", 1, 1, -300),
        ("かきゃ", "仮甲", 0, 0, 20),
        ("かっ", "仮促", 0, 0, 30),
        ("き", "木", 1, 0, -100),
        ("甲😀か", "合成長", 0, 1, -40),
    ];
    if dense {
        entries.extend(surfaces.iter().enumerate().map(|(index, surface)| {
            (
                "か",
                surface.as_str(),
                (index % 2) as u16,
                (index % 2) as u16,
                index as i32 % 7 - 12,
            )
        }));
    }
    entries.sort_by_key(|entry| entry.0);
    JapaneseDictionary::from_bytes(
        test_model::bytes(&entries, 2, &[0, -20, 80, 10]).into_boxed_slice(),
    )
    .expect("合成词库")
}

fn assert_measurement_scope(measured: Measurement) {
    assert_eq!(measured.minimum_bytes, 0, "不能释放测量前的存储");
    assert!(measured.remaining_bytes >= 0);
}

#[test]
fn consumed_rows_release_storage_and_preserve_complete_queries() {
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
                let _ = search_converted(&dictionary, &conversion, 16);
                for limit in [0, 1, 2, 8, 16, 32] {
                    let (expected, old) =
                        measure(|| retained_reference_search(&dictionary, &conversion, limit));
                    let (actual, new) =
                        measure(|| search_converted(&dictionary, &conversion, limit));
                    assert_eq!(
                        actual, expected,
                        "dense={dense}, pending={pending}, limit={limit}"
                    );
                    assert_measurement_scope(old);
                    assert_measurement_scope(new);
                    assert_eq!(new.allocations, old.allocations);
                    assert_eq!(new.remaining_bytes, old.remaining_bytes);
                    assert!(new.peak_bytes <= old.peak_bytes);
                    if reading.chars().count() >= 32 && limit > 0 {
                        assert!(
                            new.peak_bytes < old.peak_bytes,
                            "已消费行应提前释放：{}→{}",
                            old.peak_bytes,
                            new.peak_bytes
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn unknown_unicode_peak_is_linear_after_consumed_rows_are_released() {
    let dictionary = fixture(false);
    for length in [32, 128, 512] {
        let conversion = RomajiConversion {
            hiragana: "😀".repeat(length),
            pending: String::new(),
            complete: true,
        };
        let _ = search_converted(&dictionary, &conversion, 16);
        let (expected, old) = measure(|| retained_reference_search(&dictionary, &conversion, 16));
        let (actual, new) = measure(|| search_converted(&dictionary, &conversion, 16));
        assert_eq!(actual, expected);
        assert_eq!(
            actual,
            [JapaneseConversion {
                text: conversion.hiragana.clone(),
                cost: i64::from(UNKNOWN_KANA_COST) * length as i64
            }]
        );
        assert_measurement_scope(old);
        assert_measurement_scope(new);
        assert_eq!(new.allocations, old.allocations);
        assert_eq!(new.remaining_bytes, old.remaining_bytes);
        eprintln!(
            "未知Unicode {length}：峰值请求字节 {}→{}，分配 {}→{}",
            old.peak_bytes, new.peak_bytes, old.allocations, new.allocations
        );
        assert!(new.peak_bytes < old.peak_bytes);
        // 固定八项行容器、边界与两份未知文本，留出跨架构布局余量。
        assert!(new.peak_bytes <= length * 400 + 2048);
    }
}

#[test]
#[ignore = "本地 release 与冻结历史行查询交替对照；不设置 CI 时间阈值"]
fn benchmark_consumed_row_release() {
    use std::hint::black_box;
    use std::time::Instant;
    for (dense, input, iterations) in [
        (false, "ka".to_owned(), 20_000),
        (false, "kiki".to_owned(), 20_000),
        (true, "ka".to_owned(), 10_000),
        (true, "kaka".to_owned(), 10_000),
        (false, "ka".repeat(32), 1000),
        (true, "ka".repeat(32), 1000),
        (false, "😀".repeat(128), 1000),
        (false, "😀".repeat(512), 100),
        (false, "kak".to_owned(), 10_000),
        (false, "k".to_owned(), 20_000),
        (false, "".to_owned(), 20_000),
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
        let _ = search_converted(&dictionary, &conversion, 16);
        for limit in [0, 1, 16] {
            let (expected, old) =
                measure(|| retained_reference_search(&dictionary, &conversion, limit));
            let (actual, new) = measure(|| search_converted(&dictionary, &conversion, limit));
            assert_eq!(actual, expected);
            assert_eq!(new.allocations, old.allocations);
            assert_measurement_scope(old);
            assert_measurement_scope(new);
            let mut timings = [Vec::new(), Vec::new()];
            for batch in 0..10 {
                for index in if batch % 2 == 0 { [0, 1] } else { [1, 0] } {
                    let start = Instant::now();
                    for _ in 0..iterations {
                        black_box(if index == 0 {
                            retained_reference_search(
                                black_box(&dictionary),
                                black_box(&conversion),
                                black_box(limit),
                            )
                        } else {
                            search_converted(
                                black_box(&dictionary),
                                black_box(&conversion),
                                black_box(limit),
                            )
                        });
                    }
                    timings[index].push(start.elapsed());
                }
            }
            for samples in &mut timings {
                samples.sort_unstable();
            }
            eprintln!("行释放：密集={dense}，读音字符={}，待定={}，限额={limit}，分配={}→{}，峰值字节={}→{}，中位批次={:?}→{:?}/{iterations}次", conversion.hiragana.chars().count(), conversion.pending.len(), old.allocations, new.allocations, old.peak_bytes, new.peak_bytes, timings[0][5], timings[1][5]);
        }
    }
}
