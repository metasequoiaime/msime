use super::super::ngram::tests::{pair_table, triple_table};
use super::super::personal::tests::record_run;
use super::*;

#[path = "decode_terminal_legacy_tests.rs"]
mod legacy;

#[test]
fn terminal_stream_reduces_dense_decode_peak() {
    use super::capacity_tests::{assert_paths, graph};
    use crate::ime::personal_rerank::allocations::measure;
    for (columns, width, beam, nbest) in [(1, 512, 4, 12), (12, 32, 32, 5), (32, 32, 32, 5)] {
        let graph = graph(columns, width, 0, false);
        let options = LatticeOptions {
            beam,
            nbest,
            ..LatticeOptions::default()
        };
        let (old, before) = measure(|| legacy::decode_graph(&graph, &options, None));
        let (new, after) = measure(|| decode_graph(&graph, &options, None));
        assert_paths(&new, &old);
        assert_eq!(new.capacity(), old.capacity());
        assert_eq!(after.remaining_bytes, before.remaining_bytes);
        assert_eq!(after.allocations, before.allocations);
        eprintln!("terminal-stream {columns}/{width}/{beam}/{nbest}: {before:?} -> {after:?}");
        assert!(
            after.peak_bytes * 5 < before.peak_bytes * 4,
            "末列临时峰值未减少两成"
        );
    }
}

#[test]
fn terminal_stream_preserves_paths_memos_and_return_storage() {
    use super::capacity_tests::{assert_paths, graph};
    use crate::ime::personal_rerank::allocations::measure;
    let directory = tempfile::tempdir().unwrap();
    let table = Arc::new(pair_table(
        directory.path(),
        "pairs.bin",
        &[(SENTENCE_START, "甲", 1.0), ("甲", "乙", -2.0)],
    ));
    let trigram = Arc::new(triple_table(
        directory.path(),
        "triples.bin",
        &[(SENTENCE_START, "甲", "乙", 0.75)],
    ));
    let mut personal = PersonalNgram::default();
    record_run(&mut personal, &["甲", "乙", "测试", "甲乙"], 7);
    for seed in 0..4 {
        for columns in [0, 1, 4, 12] {
            for width in [0, 1, 8, 32] {
                let mut graph = graph(columns, width, seed, false);
                if seed % 2 != 0 && columns > 1 {
                    graph[0].clear();
                }
                let extra = super::capacity_tests::graph(columns, width, seed + 1, true);
                for (beam, nbest) in [(0, 0), (0, 5), (1, 5), (4, 0), (4, 65), (32, 5), (65, 65)] {
                    for mode in 0..32 {
                        let old_memo = RefCell::new(BigramMemo::default());
                        let new_memo = RefCell::new(BigramMemo::default());
                        let options = LatticeOptions {
                            beam,
                            nbest,
                            bigram: (mode & 1 != 0).then(|| Arc::clone(&table)),
                            trigram: (mode & 2 != 0).then(|| Arc::clone(&trigram)),
                            personal: (mode & 4 != 0).then_some(&personal),
                            bigram_memo: (mode & 8 != 0).then_some(&new_memo),
                            ..LatticeOptions::default()
                        };
                        let old_options = LatticeOptions {
                            bigram_memo: (mode & 8 != 0).then_some(&old_memo),
                            ..options.clone()
                        };
                        let extra = (mode & 16 != 0).then_some(&extra);
                        for _ in 0..2 {
                            let (old, before) =
                                measure(|| legacy::decode_graph(&graph, &old_options, extra));
                            let (new, after) =
                                measure(|| super::decode_graph(&graph, &options, extra));
                            assert_eq!(
                                new.len(),
                                old.len(),
                                "seed={seed} columns={columns} width={width} beam={beam} nbest={nbest} mode={mode}"
                            );
                            assert_paths(&new, &old);
                            assert_eq!(new.capacity(), old.capacity());
                            assert_eq!(after.remaining_bytes, before.remaining_bytes);
                            assert_eq!(after.minimum_bytes, before.minimum_bytes);
                            assert!(after.allocations <= before.allocations);
                            assert!(after.peak_bytes <= before.peak_bytes, "seed={seed} columns={columns} width={width} mode={mode}: {before:?} -> {after:?}");
                            assert_eq!(new_memo.borrow().table, old_memo.borrow().table);
                            assert_eq!(new_memo.borrow().scores, old_memo.borrow().scores);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn terminal_stream_preserves_nonfinite_ties_and_extreme_limits() {
    use super::capacity_tests::{assert_paths, graph};
    for (beam, nbest) in [
        (0, 0),
        (0, 12),
        (1, 0),
        (4, 12),
        (32, 5),
        (65, 65),
        (usize::MAX, usize::MAX),
    ] {
        for width in [0, 1, 4, 5, 12, 13, 14, 32, 33, 34, 65, 66, 67, 128] {
            for special in [false, true] {
                let mut graph = graph(1, width, 0, false);
                for (index, edge) in graph[0].iter_mut().enumerate() {
                    edge.word = format!("合成{index}");
                    if special {
                        edge.log_prob =
                            [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.0, 0.0, 1.0][index % 6];
                    }
                }
                let options = LatticeOptions {
                    beam,
                    nbest,
                    ..LatticeOptions::default()
                };
                let old = legacy::decode_graph(&graph, &options, None);
                let new = decode_graph(&graph, &options, None);
                assert_paths(&new, &old);
                assert_eq!(new.capacity(), old.capacity());
            }
        }
    }
    assert_eq!(
        std::mem::size_of::<super::BeamColumn<'_>>(),
        std::mem::size_of::<Vec<Hyp<'_>>>() + std::mem::size_of::<usize>()
    );
}

#[test]
#[ignore = "同机 release 配对计时，普通 CI 不依赖共享机器速度"]
fn terminal_stream_release_comparison() {
    use super::capacity_tests::{assert_paths, graph};
    use std::{hint::black_box, time::Instant};
    let directory = tempfile::tempdir().unwrap();
    let table = Arc::new(pair_table(
        directory.path(),
        "pairs.bin",
        &[(SENTENCE_START, "甲", 1.0), ("甲", "乙", -2.0)],
    ));
    let mut regressions = Vec::new();
    let mut scored_ratios = Vec::new();
    for mode in 0..3 {
        for with_extra in [false, true] {
            for (columns, width, beam, nbest) in [
                (0, 0, 32, 5),
                (1, 1, 32, 5),
                (1, 128, 4, 12),
                (4, 0, 32, 5),
                (4, 8, 32, 5),
                (12, 32, 32, 5),
                (32, 32, 32, 5),
                (4, 32, 4, 65),
            ] {
                let graph = graph(columns, width, 0, false);
                let extra = super::capacity_tests::graph(columns, width, 1, true);
                let extra = with_extra.then_some(&extra);
                let old_memo = RefCell::new(BigramMemo::default());
                let new_memo = RefCell::new(BigramMemo::default());
                let options = LatticeOptions {
                    beam,
                    nbest,
                    bigram: (mode > 0).then(|| Arc::clone(&table)),
                    bigram_memo: (mode == 2).then_some(&new_memo),
                    ..LatticeOptions::default()
                };
                let old_options = LatticeOptions {
                    bigram_memo: (mode == 2).then_some(&old_memo),
                    ..options.clone()
                };
                assert_paths(
                    &super::decode_graph(&graph, &options, extra),
                    &legacy::decode_graph(&graph, &old_options, extra),
                );
                let run = |arm, repeats| {
                    let start = Instant::now();
                    for _ in 0..repeats {
                        if arm == 0 {
                            black_box(legacy::decode_graph(
                                black_box(&graph),
                                black_box(&old_options),
                                black_box(extra),
                            ));
                        } else {
                            black_box(super::decode_graph(
                                black_box(&graph),
                                black_box(&options),
                                black_box(extra),
                            ));
                        }
                    }
                    start.elapsed().as_secs_f64()
                };
                let mut repeats = (200_000 / (columns.max(1) * width.max(1))).clamp(200, 20_000);
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
                    assert!(repeats < 2_000_000);
                    repeats *= 2;
                }
                let mut ratios = Vec::new();
                let mut raw_seconds = Vec::new();
                for pair in 0..21 {
                    let mut elapsed = [0.0; 2];
                    for arm in if pair % 2 == 0 { [0, 1] } else { [1, 0] } {
                        elapsed[arm] = run(arm, repeats);
                    }
                    raw_seconds.push(elapsed);
                    ratios.push(elapsed[1] / elapsed[0]);
                }
                eprintln!("terminal-stream raw_seconds={raw_seconds:?}");
                ratios.sort_by(f64::total_cmp);
                let median = ratios[10];
                eprintln!("terminal-stream mode={mode} extra={with_extra} columns={columns} width={width} beam={beam} nbest={nbest} repeats={repeats} median={median:.6} p10={:.6} p90={:.6}",ratios[2],ratios[18]);
                if mode > 0 && columns > 1 && width > 0 {
                    scored_ratios.push(median);
                }
                if median >= 1.05 {
                    regressions.push(format!("{mode}/{with_extra}/{columns}/{width}: {median}"));
                }
            }
        }
    }
    let scored_ratio = (scored_ratios.iter().map(|ratio| ratio.ln()).sum::<f64>()
        / scored_ratios.len() as f64)
        .exp();
    eprintln!("terminal-stream scored multi-column geometric mean={scored_ratio:.6}");
    assert!(
        scored_ratio < 1.0,
        "有表多列形状没有总体收益: {scored_ratio}"
    );
    assert!(
        regressions.is_empty(),
        "回退超过预算: {}",
        regressions.join("; ")
    );
}
