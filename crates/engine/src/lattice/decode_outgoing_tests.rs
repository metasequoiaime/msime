use super::super::ngram::tests::{pair_table, triple_table};
use super::super::personal::tests::record_run;
use super::*;

// 复用已固定的旧参考，避免把相同源码重复挂成另一个模块。
use super::prefix_tests::legacy;

#[test]
fn outgoing_edges_need_no_owned_pointer_buffer() {
    use super::capacity_tests::{assert_paths, graph};
    use crate::ime::personal_rerank::allocations::measure;
    for width in [1, 4, 8, 32, 65] {
        let graph = graph(1, width, 0, false);
        let options = LatticeOptions::default();
        let (old, before) = measure(|| legacy::decode_graph(&graph, &options, None));
        let (new, after) = measure(|| decode_graph(&graph, &options, None));
        assert_paths(&new, &old);
        assert_eq!(new.capacity(), old.capacity());
        assert_eq!(after.remaining_bytes, before.remaining_bytes);
        assert_eq!(after.minimum_bytes, before.minimum_bytes);
        assert!(
            after.allocations < before.allocations,
            "宽度 {width}: {before:?} -> {after:?}"
        );
        assert!(after.peak_bytes < before.peak_bytes);
        eprintln!("outgoing width={width}: {before:?} -> {after:?}");
    }
}

#[test]
fn borrowed_outgoing_preserves_paths_memos_and_return_storage() {
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
                for mode in 0..32 {
                    let old_memo = RefCell::new(BigramMemo::default());
                    let new_memo = RefCell::new(BigramMemo::default());
                    let options = LatticeOptions {
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
                        let (new, after) = measure(|| decode_graph(&graph, &options, extra));
                        assert_paths(&new, &old);
                        assert_eq!(new.capacity(), old.capacity());
                        assert_eq!(after.remaining_bytes, before.remaining_bytes);
                        assert_eq!(after.minimum_bytes, before.minimum_bytes);
                        assert!(after.allocations <= before.allocations);
                        let column_metadata = (columns + 1)
                            * (std::mem::size_of::<super::BeamColumn<'_>>()
                                - std::mem::size_of::<Vec<Hyp<'_>>>());
                        assert!(after.peak_bytes <= before.peak_bytes + column_metadata);
                        assert_eq!(new_memo.borrow().table, old_memo.borrow().table);
                        assert_eq!(new_memo.borrow().scores, old_memo.borrow().scores);
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "同机 release 配对计时，普通 CI 不依赖共享机器速度"]
fn outgoing_borrow_release_comparison() {
    use super::capacity_tests::{assert_paths, graph};
    use std::{hint::black_box, time::Instant};
    let directory = tempfile::tempdir().unwrap();
    let table = Arc::new(pair_table(
        directory.path(),
        "pairs.bin",
        &[(SENTENCE_START, "甲", 1.0), ("甲", "乙", -2.0)],
    ));
    let mut regressions = Vec::new();
    for mode in 0..3 {
        for with_extra in [false, true] {
            for (columns, width) in [(0, 0), (1, 1), (4, 0), (4, 8), (12, 32), (32, 32)] {
                let graph = graph(columns, width, 0, false);
                let extra = super::capacity_tests::graph(columns, width, 1, true);
                let extra = with_extra.then_some(&extra);
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
                    &decode_graph(&graph, &options, extra),
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
                            black_box(decode_graph(
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
                for pair in 0..40 {
                    let mut elapsed = [0.0; 2];
                    for arm in if pair % 2 == 0 { [0, 1] } else { [1, 0] } {
                        elapsed[arm] = run(arm, repeats);
                    }
                    ratios.push(elapsed[1] / elapsed[0]);
                }
                ratios.sort_by(f64::total_cmp);
                let median = (ratios[19] + ratios[20]) * 0.5;
                eprintln!("outgoing mode={mode} extra={with_extra} columns={columns} width={width} repeats={repeats} median={median:.6} p10={:.6} p90={:.6}",ratios[4],ratios[35]);
                if median >= 1.05 {
                    regressions.push(format!("{mode}/{with_extra}/{columns}/{width}: {median}"));
                }
            }
        }
    }
    assert!(
        regressions.is_empty(),
        "回退超过预算: {}",
        regressions.join("; ")
    );
}
