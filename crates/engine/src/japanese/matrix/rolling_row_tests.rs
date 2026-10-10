use super::super::decoder::test_model;
use super::linear_row_reference::search_converted as linear_search;
use super::*;
use crate::ime::personal_rerank::allocations::measure;

pub(super) fn saved_row_buffers(conversion: &RomajiConversion, limit: usize) -> usize {
    if limit == 0 {
        return 0;
    }
    conversion
        .hiragana
        .chars()
        .count()
        .saturating_sub(MAX_LEMMA_MORA)
}

pub(super) fn fixture(dense: bool) -> JapaneseDictionary {
    let readings: Vec<_> = [1, 2, 3, 15, 16, 17, 32]
        .map(|length| "か".repeat(length))
        .into();
    let surfaces: Vec<_> = (0..24)
        .map(|index| format!("合成{}😀", index % 7))
        .collect();
    let mut entries = vec![("き", "木", 0, 1, -100), ("甲😀か", "合成長", 1, 0, -40)];
    for reading in &readings {
        for (index, surface) in surfaces.iter().enumerate().take(if dense { 24 } else { 3 }) {
            entries.push((
                reading.as_str(),
                surface.as_str(),
                (index % 2) as u16,
                ((index + reading.len()) % 2) as u16,
                index as i32 % 7 - 20,
            ));
        }
    }
    entries.sort_by_key(|entry| entry.0);
    JapaneseDictionary::from_bytes(
        test_model::bytes(&entries, 2, &[0, -20, 80, -5]).into_boxed_slice(),
    )
    .expect("合成词库")
}

#[test]
fn rolling_unknown_rows_bound_buffers_and_preserve_heap_scope() {
    let dictionary = fixture(false);
    for length in [1, 16, 17, 18, 32, 33, 34, 35, 128, 512] {
        let conversion = RomajiConversion {
            hiragana: "😀".repeat(length),
            pending: String::new(),
            complete: true,
        };
        let (expected, old) = measure(|| linear_search(&dictionary, &conversion, 16));
        let (actual, new) = measure(|| search_converted(&dictionary, &conversion, 16));
        assert_eq!(actual, expected);
        assert_eq!(
            actual,
            [JapaneseConversion {
                text: conversion.hiragana.clone(),
                cost: i64::from(UNKNOWN_KANA_COST) * length as i64,
            }]
        );
        eprintln!(
            "未知读音 {length}：分配 {}→{}，峰值请求字节 {}→{}",
            old.allocations, new.allocations, old.peak_bytes, new.peak_bytes
        );
        assert_eq!(
            new.allocations
                + saved_row_buffers(&conversion, 16)
                + super::boundary_tests::saved_boundary_buffers(&conversion, 16),
            old.allocations
        );
        assert_eq!(new.remaining_bytes, old.remaining_bytes);
        assert_eq!(old.minimum_bytes, 0);
        assert_eq!(new.minimum_bytes, 0);
        assert!(new.peak_bytes <= old.peak_bytes);
        if length > MAX_LEMMA_MORA {
            assert!(new.peak_bytes < old.peak_bytes);
        }
    }
}

#[test]
fn maximum_span_paths_survive_every_window_wrap() {
    let reading = "か".repeat(MAX_LEMMA_MORA);
    let dictionary = JapaneseDictionary::from_bytes(
        test_model::bytes(&[(&reading, "最大跨度", 1, 1, -100)], 2, &[0; 4]).into_boxed_slice(),
    )
    .expect("合成最大跨度词库");
    for segments in [1, 2, 3, 4, 8] {
        let conversion = RomajiConversion {
            hiragana: reading.repeat(segments),
            pending: String::new(),
            complete: true,
        };
        let expected = linear_search(&dictionary, &conversion, 16);
        let actual = search_converted(&dictionary, &conversion, 16);
        assert_eq!(actual, expected);
        assert_eq!(
            actual[0],
            JapaneseConversion {
                text: "最大跨度".repeat(segments),
                cost: -100 * segments as i64,
            }
        );
    }
}

#[test]
fn rolling_rows_match_linear_query_across_wraps_and_maximum_spans() {
    for dense in [false, true] {
        let dictionary = fixture(dense);
        let mut readings = vec![
            String::new(),
            "か".to_owned(),
            "甲😀か".repeat(12),
            "未知😀".repeat(12),
        ];
        readings.extend(
            [15, 16, 17, 18, 31, 32, 33, 34, 35, 48, 51, 64].map(|length| "か".repeat(length)),
        );
        for reading in &readings {
            for pending in ["", "k", "K", "sh", "xt", "q", "あ"] {
                let conversion = RomajiConversion {
                    hiragana: reading.clone(),
                    pending: pending.to_owned(),
                    complete: pending.is_empty(),
                };
                let _ = linear_search(&dictionary, &conversion, 16);
                let _ = search_converted(&dictionary, &conversion, 16);
                for limit in [0, 1, 2, 8, 12, 32] {
                    let (expected, old) =
                        measure(|| linear_search(&dictionary, &conversion, limit));
                    let (actual, new) =
                        measure(|| search_converted(&dictionary, &conversion, limit));
                    assert_eq!(
                        actual,
                        expected,
                        "密集={dense}，字符数={}，待定={pending}，限额={limit}",
                        reading.chars().count()
                    );
                    assert_eq!(
                        new.allocations
                            + saved_row_buffers(&conversion, limit)
                            + super::boundary_tests::saved_boundary_buffers(&conversion, limit),
                        old.allocations
                    );
                    assert_eq!(new.remaining_bytes, old.remaining_bytes);
                    assert_eq!(new.minimum_bytes, 0);
                    assert_eq!(old.minimum_bytes, 0);
                    assert!(new.peak_bytes <= old.peak_bytes);
                }
            }
        }
    }
}

#[test]
#[ignore = "本地 release 滚动行与边界组合对固定线性查询交替对照；不设置 CI 时间阈值"]
fn benchmark_rolling_matrix_rows() {
    use std::hint::black_box;
    use std::time::Instant;
    for (dense, length, unknown, iterations) in [
        (false, 1, false, 20000),
        (false, 2, false, 20000),
        (true, 1, false, 10000),
        (true, 2, false, 10000),
        (false, 16, false, 2000),
        (false, 17, false, 2000),
        (true, 17, false, 1000),
        (false, 32, false, 1000),
        (true, 32, false, 1000),
        (false, 128, true, 1000),
        (false, 512, true, 100),
    ] {
        let dictionary = fixture(dense);
        let conversion = RomajiConversion {
            hiragana: if unknown {
                "😀".repeat(length)
            } else {
                "か".repeat(length)
            },
            pending: String::new(),
            complete: true,
        };
        for limit in [1, 16] {
            let (expected, old) = measure(|| linear_search(&dictionary, &conversion, limit));
            let (actual, new) = measure(|| search_converted(&dictionary, &conversion, limit));
            assert_eq!(actual, expected);
            assert_eq!(
                new.allocations
                    + saved_row_buffers(&conversion, limit)
                    + super::boundary_tests::saved_boundary_buffers(&conversion, limit),
                old.allocations
            );
            let mut timings = [Vec::new(), Vec::new()];
            for batch in 0..10 {
                for index in if batch % 2 == 0 { [0, 1] } else { [1, 0] } {
                    let start = Instant::now();
                    for _ in 0..iterations {
                        black_box(if index == 0 {
                            linear_search(
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
            eprintln!("组合查询（滚动行与边界）：密集={dense}，字符数={length}，未知={unknown}，限额={limit}，分配={}→{}，峰值={}→{}，中位批次={:?}→{:?}/{iterations}次",
                old.allocations, new.allocations, old.peak_bytes, new.peak_bytes, timings[0][5], timings[1][5]);
        }
    }
}

#[inline(never)]
fn paired_batch(
    dictionary: &JapaneseDictionary,
    conversion: &RomajiConversion,
    limit: usize,
    rolling: bool,
    iterations: usize,
) -> std::time::Duration {
    use std::hint::black_box;
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        drop(black_box(if rolling {
            search_converted(
                black_box(dictionary),
                black_box(conversion),
                black_box(limit),
            )
        } else {
            linear_search(
                black_box(dictionary),
                black_box(conversion),
                black_box(limit),
            )
        }));
    }
    start.elapsed()
}

fn paired_percentiles(mut samples: Vec<f64>) -> [f64; 3] {
    samples.sort_unstable_by(f64::total_cmp);
    [10, 50, 90].map(|percent| samples[(samples.len() - 1) * percent / 100])
}

#[test]
#[ignore = "本地 release 滚动行与边界组合对线性查询短批次配对计时；不设置 CI 时间阈值"]
fn benchmark_rolling_matrix_rows_paired() {
    use std::hint::black_box;
    for (dense, length, unknown, iterations) in [
        (false, 0, false, 1024),
        (false, 1, false, 64),
        (false, 2, false, 64),
        (true, 1, false, 64),
        (true, 2, false, 32),
        (false, 16, false, 4),
        (false, 17, false, 4),
        (true, 17, false, 2),
        (false, 32, false, 2),
        (true, 32, false, 1),
        (false, 128, true, 4),
        (false, 512, true, 1),
    ] {
        let dictionary = fixture(dense);
        let conversion = RomajiConversion {
            hiragana: if unknown {
                "😀".repeat(length)
            } else {
                "か".repeat(length)
            },
            pending: String::new(),
            complete: true,
        };
        for limit in [1, 16] {
            let (expected, old) = measure(|| linear_search(&dictionary, &conversion, limit));
            let (actual, new) = measure(|| search_converted(&dictionary, &conversion, limit));
            assert_eq!(actual, expected);
            assert_eq!(
                new.allocations
                    + saved_row_buffers(&conversion, limit)
                    + super::boundary_tests::saved_boundary_buffers(&conversion, limit),
                old.allocations
            );
            for rolling in [false, true] {
                black_box(paired_batch(
                    &dictionary,
                    &conversion,
                    limit,
                    rolling,
                    iterations,
                ));
            }
            let mut groups = Vec::with_capacity(240);
            for group in 0..240 {
                let mut timings = [std::time::Duration::ZERO; 2];
                // 相邻短批次组成一对，两种先后顺序各出现一百二十次。
                for index in if group % 2 == 0 { [0, 1] } else { [1, 0] } {
                    timings[index] =
                        paired_batch(&dictionary, &conversion, limit, index == 1, iterations);
                }
                groups.push(timings);
            }
            let absolute = std::array::from_fn::<_, 2, _>(|index| {
                paired_percentiles(
                    groups
                        .iter()
                        .map(|group| group[index].as_secs_f64() * 1e6)
                        .collect(),
                )
            });
            let ratios = paired_percentiles(
                groups
                    .iter()
                    .map(|group| {
                        assert!(!group[0].is_zero());
                        group[1].as_secs_f64() / group[0].as_secs_f64()
                    })
                    .collect(),
            );
            eprintln!("组合配对（滚动行与边界）：密集={dense} 字符={length} 未知={unknown} 限额={limit} iterations={iterations} groups={} batches_us_p10_p50_p90={absolute:.3?} rolling_linear={ratios:.4?} 分配={}→{} 峰值={}→{}", groups.len(), old.allocations, new.allocations, old.peak_bytes, new.peak_bytes);
        }
    }
}
