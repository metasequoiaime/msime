use super::super::ngram::tests::{pair_table, triple_table};
use super::super::personal::tests::record_run;
use super::*;

#[path = "decode_capacity_legacy_tests.rs"]
mod legacy;

fn chain(columns: usize, gap: bool) -> Graph {
    (0..columns)
        .map(|start| {
            if gap && start == 0 {
                return Vec::new();
            }
            vec![Edge {
                end: start + 1,
                word: "合成".to_owned(),
                key: "he'cheng".to_owned(),
                log_prob: -0.25,
                typo: false,
            }]
        })
        .collect()
}

#[test]
fn sparse_and_unreachable_columns_do_not_reserve_full_beams() {
    use crate::ime::personal_rerank::allocations::measure;
    for columns in [1, 4, 32] {
        for gap in [false, true] {
            let graph = chain(columns, gap);
            let options = LatticeOptions::default();
            let (expected, before) = measure(|| legacy::decode_graph(&graph, &options, None));
            let (actual, after) = measure(|| decode_graph(&graph, &options, None));
            assert_eq!(actual, expected);
            assert_eq!(after.remaining_bytes, before.remaining_bytes);
            assert!(after.allocations <= before.allocations);
            // 每列至多一条存活路径；旧版每列至少预留 beam 条，起点也一样。
            let saved = (columns + 1) * (options.beam - 1) * std::mem::size_of::<Hyp<'_>>();
            assert!(
                before.peak_bytes - after.peak_bytes >= saved,
                "{columns}/{gap}: {before:?} -> {after:?}"
            );
            eprintln!("column-capacity {columns}/{gap}: {before:?} -> {after:?}");
        }
    }
}

pub(super) fn graph(columns: usize, width: usize, seed: usize, extra: bool) -> Graph {
    const WORDS: [&str; 8] = ["甲", "乙", "丙", "丁", "甲乙", "乙丙", "测试", "甲"];
    (0..columns)
        .map(|start| {
            (0..width)
                .filter(|index| !extra || (index + start + seed).is_multiple_of(3))
                .map(|index| Edge {
                    end: (start + 1 + (index + seed) % 3).min(columns),
                    word: WORDS[(index + start * 3 + seed) % WORDS.len()].to_owned(),
                    key: format!("k{start}'k{index}"),
                    log_prob: -(((index + start + seed) % 7) as f64) * 0.125,
                    typo: extra,
                })
                .collect()
        })
        .collect()
}

pub(super) fn assert_paths(actual: &[SentencePath], expected: &[SentencePath]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.sentence, expected.sentence);
        assert_eq!(actual.key, expected.key);
        assert_eq!(actual.words, expected.words);
        assert_eq!(actual.typo_edges, expected.typo_edges);
        assert_eq!(actual.log_prob.to_bits(), expected.log_prob.to_bits());
    }
}

#[test]
fn bounded_columns_preserve_legacy_decode() {
    let directory = tempfile::tempdir().unwrap();
    let bigram = Arc::new(pair_table(
        directory.path(),
        "pairs.bin",
        &[
            (SENTENCE_START, "甲", 0.75),
            ("甲", "乙", -0.25),
            ("乙", "测试", 1.25),
        ],
    ));
    let trigram = Arc::new(triple_table(
        directory.path(),
        "triples.bin",
        &[
            (SENTENCE_START, "甲", "乙", 0.75),
            ("乙", "测试", "甲乙", -0.5),
        ],
    ));
    let mut personal = PersonalNgram::default();
    record_run(&mut personal, &["甲", "乙", "测试", "甲乙"], 7);
    for seed in 0..4 {
        for columns in [0, 1, 4, 8, 16] {
            for width in [0, 1, 4, 16] {
                let graph = graph(columns, width, seed, false);
                let extra = self::graph(columns, width, seed + 1, true);
                for beam in [0, 1, 4, 32] {
                    for nbest in [0, 1, 5, 12, 64] {
                        for mode in 0..32 {
                            let actual_memo = RefCell::new(BigramMemo::default());
                            let expected_memo = RefCell::new(BigramMemo::default());
                            let options = LatticeOptions {
                                beam,
                                nbest,
                                bigram: (mode & 1 != 0).then(|| Arc::clone(&bigram)),
                                trigram: (mode & 2 != 0).then(|| Arc::clone(&trigram)),
                                personal: (mode & 4 != 0).then_some(&personal),
                                bigram_memo: (mode & 8 != 0).then_some(&actual_memo),
                                ..LatticeOptions::default()
                            };
                            let old_options = LatticeOptions {
                                bigram_memo: (mode & 8 != 0).then_some(&expected_memo),
                                ..options.clone()
                            };
                            let extra = (mode & 16 != 0).then_some(&extra);
                            for _ in 0..2 {
                                assert_paths(
                                    &decode_graph(&graph, &options, extra),
                                    &legacy::decode_graph(&graph, &old_options, extra),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "同机 release 配对计时，普通 CI 不依赖共享机器速度"]
fn column_capacity_release_comparison() {
    use std::hint::black_box;
    use std::time::Instant;
    let directory = tempfile::tempdir().unwrap();
    let table = Arc::new(pair_table(
        directory.path(),
        "pairs.bin",
        &[
            (SENTENCE_START, "甲", 0.75),
            ("甲", "乙", -0.25),
            ("乙", "测试", 1.25),
        ],
    ));
    for mode in 0..3 {
        for (columns, width) in [(1, 1), (4, 4), (4, 32), (12, 8), (12, 32), (32, 32)] {
            let graph = graph(columns, width, 0, false);
            let old_memo = RefCell::new(BigramMemo::default());
            let new_memo = RefCell::new(BigramMemo::default());
            let options = LatticeOptions {
                bigram: (mode > 0).then(|| Arc::clone(&table)),
                bigram_memo: (mode == 2).then_some(&new_memo),
                ..LatticeOptions::default()
            };
            let old_options = LatticeOptions {
                bigram_memo: (mode == 2).then_some(&old_memo),
                ..options.clone()
            };
            assert_paths(
                &decode_graph(&graph, &options, None),
                &legacy::decode_graph(&graph, &old_options, None),
            );
            // 每臂扩大十倍以降低亚毫秒批次的抖动；表备忘在正式计时前已经预热。
            let repeats = (200_000 / (columns * width)).clamp(200, 20_000);
            let mut ratios = Vec::new();
            for pair in 0..40 {
                let mut elapsed = [0.0; 2];
                for arm in if pair % 2 == 0 { [0, 1] } else { [1, 0] } {
                    let started = Instant::now();
                    for _ in 0..repeats {
                        if arm == 0 {
                            black_box(legacy::decode_graph(
                                black_box(&graph),
                                black_box(&old_options),
                                None,
                            ));
                        } else {
                            black_box(decode_graph(black_box(&graph), black_box(&options), None));
                        }
                    }
                    elapsed[arm] = started.elapsed().as_secs_f64();
                }
                ratios.push(elapsed[1] / elapsed[0]);
            }
            ratios.sort_by(f64::total_cmp);
            let median = (ratios[19] + ratios[20]) * 0.5;
            eprintln!("column-capacity mode={mode} columns={columns} width={width} repeats={repeats} median-new/old={median:.6} p10={:.6} p90={:.6}", ratios[4], ratios[35]);
            assert!(
                median < 1.05,
                "{mode}/{columns}/{width} 回退超过预算: {median}"
            );
        }
    }
}
