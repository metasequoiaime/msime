use super::super::decoder::test_model;
use super::full_boundary_reference::search_converted as full_search;
use super::rolling_row_tests::fixture;
use super::*;
use crate::ime::personal_rerank::allocations::{count, measure};

pub(super) fn saved_boundary_buffers(conversion: &RomajiConversion, limit: usize) -> usize {
    if limit == 0 || conversion.hiragana.is_empty() {
        return 0;
    }
    // 独立量化冻结容器，包含 collect 对不同 UTF-8 字符宽度发生的扩容。
    let (_, allocations) = count(|| {
        conversion
            .hiragana
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(conversion.hiragana.len()))
            .collect::<Vec<_>>()
    });
    allocations
}

#[test]
fn bounded_boundaries_save_utf8_buffers_and_keep_complete_queries() {
    for dense in [false, true] {
        let dictionary = fixture(dense);
        let mut readings = vec![
            String::new(),
            "か".to_owned(),
            "aλ甲😀\u{301}".repeat(12),
            "甲😀か".repeat(12),
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
                drop(full_search(&dictionary, &conversion, 16));
                drop(search_converted(&dictionary, &conversion, 16));
                for limit in [0, 1, 2, 8, 12, 32] {
                    let (expected, old) = measure(|| full_search(&dictionary, &conversion, limit));
                    let (actual, new) =
                        measure(|| search_converted(&dictionary, &conversion, limit));
                    assert_eq!(
                        actual,
                        expected,
                        "密集={dense}，字符={}，待定={pending}，限额={limit}",
                        reading.chars().count()
                    );
                    assert_eq!(
                        new.allocations + saved_boundary_buffers(&conversion, limit),
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
fn bounded_boundaries_reduce_unknown_utf8_peak_across_wraps() {
    let dictionary = fixture(false);
    for unit in ["a", "λ", "甲", "😀", "aλ甲😀\u{301}"] {
        for length in [1, 16, 17, 18, 32, 33, 34, 128, 512] {
            let conversion = RomajiConversion {
                hiragana: unit.repeat(length),
                pending: String::new(),
                complete: true,
            };
            let (expected, old) = measure(|| full_search(&dictionary, &conversion, 16));
            let (actual, new) = measure(|| search_converted(&dictionary, &conversion, 16));
            assert_eq!(actual, expected);
            assert_eq!(
                actual,
                [JapaneseConversion {
                    text: conversion.hiragana.clone(),
                    cost: i64::from(UNKNOWN_KANA_COST) * conversion.hiragana.chars().count() as i64,
                }]
            );
            let saved = saved_boundary_buffers(&conversion, 16);
            assert!(saved > 0);
            assert_eq!(new.allocations + saved, old.allocations);
            assert_eq!(new.remaining_bytes, old.remaining_bytes);
            assert_eq!(new.minimum_bytes, 0);
            assert_eq!(old.minimum_bytes, 0);
            assert!(new.peak_bytes < old.peak_bytes);
            eprintln!(
                "滚动边界：字符={} 字节={} 分配={}→{} 边界分量={saved} 峰值={}→{}",
                conversion.hiragana.chars().count(),
                conversion.hiragana.len(),
                old.allocations,
                new.allocations,
                old.peak_bytes,
                new.peak_bytes
            );
        }
    }
}

#[test]
fn reversed_prefixes_survive_overwritten_boundaries_and_exceed_matrix_span() {
    let prefix17 = "😀".repeat(17);
    let prefix32 = "😀".repeat(32);
    let dictionary = JapaneseDictionary::from_bytes(
        test_model::bytes(
            &[
                (&prefix17, "合成十七", 0, 0, -20),
                (&prefix32, "合成三十二", 0, 0, -10),
            ],
            1,
            &[0],
        )
        .into_boxed_slice(),
    )
    .expect("合成跨窗口前缀词库");
    let conversion = RomajiConversion {
        hiragana: "😀".repeat(33),
        pending: String::new(),
        complete: true,
    };
    let actual = search_converted(&dictionary, &conversion, 16);
    assert_eq!(actual, full_search(&dictionary, &conversion, 16));
    assert_eq!(
        actual,
        [
            JapaneseConversion {
                text: conversion.hiragana.clone(),
                cost: 33 * i64::from(UNKNOWN_KANA_COST)
            },
            JapaneseConversion {
                text: "合成三十二".to_owned(),
                cost: -10
            },
            JapaneseConversion {
                text: "合成十七".to_owned(),
                cost: -20
            },
        ]
    );
}

#[inline(never)]
fn paired_batch(
    dictionary: &JapaneseDictionary,
    conversion: &RomajiConversion,
    limit: usize,
    bounded: bool,
    iterations: usize,
) -> std::time::Duration {
    use std::hint::black_box;
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        drop(black_box(if bounded {
            search_converted(
                black_box(dictionary),
                black_box(conversion),
                black_box(limit),
            )
        } else {
            full_search(
                black_box(dictionary),
                black_box(conversion),
                black_box(limit),
            )
        }));
    }
    start.elapsed()
}

fn percentiles(mut samples: Vec<f64>) -> [f64; 3] {
    samples.sort_unstable_by(f64::total_cmp);
    [10, 50, 90].map(|percent| samples[(samples.len() - 1) * percent / 100])
}

#[test]
#[ignore = "本地 release 完整与滚动字节边界短批次配对；不设置 CI 时间阈值"]
fn benchmark_bounded_boundaries_paired() {
    use std::hint::black_box;
    for (dense, length, unit, iterations) in [
        (false, 0, "か", 1024),
        (false, 1, "か", 64),
        (false, 2, "か", 64),
        (true, 1, "か", 64),
        (true, 2, "か", 32),
        (false, 16, "か", 4),
        (false, 17, "か", 4),
        (false, 18, "か", 4),
        (true, 17, "か", 2),
        (false, 32, "か", 2),
        (true, 32, "か", 1),
        (false, 128, "😀", 4),
        (false, 512, "😀", 1),
        (false, 128, "aλ甲😀\u{301}", 1),
    ] {
        let dictionary = fixture(dense);
        let conversion = RomajiConversion {
            hiragana: unit.repeat(length),
            pending: String::new(),
            complete: true,
        };
        for limit in [1, 16] {
            let (expected, old) = measure(|| full_search(&dictionary, &conversion, limit));
            let (actual, new) = measure(|| search_converted(&dictionary, &conversion, limit));
            assert_eq!(actual, expected);
            assert_eq!(
                new.allocations + saved_boundary_buffers(&conversion, limit),
                old.allocations
            );
            for bounded in [false, true] {
                black_box(paired_batch(
                    &dictionary,
                    &conversion,
                    limit,
                    bounded,
                    iterations,
                ));
            }
            let mut groups = Vec::with_capacity(240);
            for group in 0..240 {
                let mut times = [std::time::Duration::ZERO; 2];
                // 相邻短批次配对，两种先后顺序各一百二十次。
                for index in if group % 2 == 0 { [0, 1] } else { [1, 0] } {
                    times[index] =
                        paired_batch(&dictionary, &conversion, limit, index == 1, iterations);
                }
                groups.push(times);
            }
            let absolute = std::array::from_fn::<_, 2, _>(|index| {
                percentiles(
                    groups
                        .iter()
                        .map(|group| group[index].as_secs_f64() * 1e6)
                        .collect(),
                )
            });
            let ratios = percentiles(
                groups
                    .iter()
                    .map(|group| {
                        assert!(!group[0].is_zero());
                        group[1].as_secs_f64() / group[0].as_secs_f64()
                    })
                    .collect(),
            );
            eprintln!("滚动边界配对：密集={dense} 字符={} 字节={} 限额={limit} iterations={iterations} groups={} batches_us_p10_p50_p90={absolute:.3?} bounded_full={ratios:.4?} 分配={}→{} 峰值={}→{}",
                conversion.hiragana.chars().count(), conversion.hiragana.len(), groups.len(), old.allocations, new.allocations, old.peak_bytes, new.peak_bytes);
        }
    }
}
