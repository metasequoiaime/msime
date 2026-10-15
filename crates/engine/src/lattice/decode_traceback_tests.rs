use super::super::ngram::tests::{pair_table, triple_table};
use super::super::personal::tests::record_run;
use super::capacity_tests::{assert_paths, graph};
use super::*;

#[path = "decode_traceback_legacy_tests.rs"]
mod legacy;

#[path = "decode_traceback_experiment_tests.rs"]
mod experiment;

use experiment::{decode_graph, Hyp};

#[test]
fn traceback_counts_all_marked_edges_even_with_an_empty_word() {
    let words = ["甲", "", "乙", "丙"];
    let graph: Graph = words
        .iter()
        .enumerate()
        .map(|(at, word)| {
            vec![Edge {
                end: at + 1,
                word: (*word).to_owned(),
                key: format!("k{at}"),
                log_prob: -0.25,
                typo: at % 2 != 0,
            }]
        })
        .collect();
    let paths = decode_graph(&graph, &LatticeOptions::default(), None);
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].sentence, "甲乙丙");
    assert_eq!(paths[0].typo_edges, 2);
    assert_paths(
        &paths,
        &legacy::decode_graph(&graph, &LatticeOptions::default(), None),
    );
}

#[test]
fn deriving_typo_counts_reduces_hypothesis_requests() {
    use crate::ime::personal_rerank::allocations::measure;
    for (columns, width) in [(1, 1), (4, 8), (12, 32)] {
        let graph = graph(columns, width, 0, false);
        let options = LatticeOptions::default();
        let (old, before) = measure(|| legacy::decode_graph(&graph, &options, None));
        let (new, after) = measure(|| decode_graph(&graph, &options, None));
        assert_paths(&new, &old);
        assert_eq!(after.allocations, before.allocations);
        assert_eq!(after.remaining_bytes, before.remaining_bytes);
        let fields = std::mem::size_of::<f64>() + 3 * std::mem::size_of::<usize>();
        let alignment = std::mem::align_of::<Hyp<'_>>();
        let expected_size = fields.div_ceil(alignment) * alignment;
        assert_eq!(std::mem::size_of::<Hyp<'_>>(), expected_size);
        if expected_size < std::mem::size_of::<legacy::Hyp<'_>>() {
            assert!(after.peak_bytes < before.peak_bytes);
        } else {
            assert_eq!(after.peak_bytes, before.peak_bytes);
        }
        eprintln!("traceback-count {columns}/{width}: {before:?} -> {after:?}");
    }
}

#[test]
fn derived_typo_counts_preserve_legacy_decode() {
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
#[ignore = "否决方案的同机 release 配对计时，普通 CI 不依赖共享机器速度"]
fn traceback_typo_count_rejected_release_comparison() {
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
    let mut regressions = Vec::new();
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
            let run = |arm: usize, repeats: usize| {
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
                started.elapsed().as_secs_f64()
            };
            let mut repeats = (200_000 / (columns * width)).clamp(200, 20_000);
            // 两臂各采三次，以最快批次校准至至少 50 ms，避免把调度停顿当作足够的批次工作量。
            loop {
                let mut fastest = f64::INFINITY;
                for _ in 0..3 {
                    for arm in 0..2 {
                        fastest = fastest.min(run(arm, repeats));
                    }
                }
                if fastest >= 0.05 {
                    break;
                }
                assert!(repeats < 2_000_000, "校准批次未达到目标时长");
                repeats *= 2;
            }
            let mut ratios = Vec::new();
            for pair in 0..40 {
                let mut elapsed = [0.0; 2];
                for arm in if pair % 2 == 0 { [0, 1] } else { [1, 0] } {
                    elapsed[arm] = run(arm, repeats);
                }
                ratios.push(elapsed[1] / elapsed[0]);
            }
            ratios.sort_by(f64::total_cmp);
            let median = (ratios[19] + ratios[20]) * 0.5;
            eprintln!("traceback-count mode={mode} columns={columns} width={width} repeats={repeats} median-new/old={median:.6} p10={:.6} p90={:.6}", ratios[4], ratios[35]);
            if median >= 1.05 {
                regressions.push(format!("{mode}/{columns}/{width}: {median}"));
            }
        }
    }
    assert!(
        regressions.is_empty(),
        "回退超过预算: {}",
        regressions.join("; ")
    );
}
