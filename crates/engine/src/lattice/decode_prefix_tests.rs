use super::super::ngram::tests::{pair_table, triple_table};
use super::super::personal::tests::record_run;
use super::*;

#[path = "decode_prefix_legacy_tests.rs"]
pub(super) mod legacy;

#[path = "decode_prefix_experiment_tests.rs"]
mod experiment;

#[cfg(debug_assertions)]
#[test]
fn repeated_previous_bytes_are_hashed_once_per_score_row() {
    use super::super::ngram::hash_work::measure;
    use super::capacity_tests::assert_paths;
    let directory = tempfile::tempdir().unwrap();
    let table = Arc::new(pair_table(
        directory.path(),
        "pairs.bin",
        &[("合成前词", "后词0", 1.5)],
    ));
    let previous = "合成前词";
    for width in [1, 4, 32, 65] {
        let graph = vec![
            vec![Edge {
                end: 1,
                word: previous.to_owned(),
                key: "a".to_owned(),
                log_prob: 0.0,
                typo: false,
            }],
            (0..width)
                .map(|index| Edge {
                    end: 2,
                    word: format!("后词{index}"),
                    key: "b".to_owned(),
                    log_prob: -(index as f64),
                    typo: false,
                })
                .collect(),
        ];
        for memo_enabled in [false, true] {
            let old_memo = RefCell::new(BigramMemo::default());
            let new_memo = RefCell::new(BigramMemo::default());
            let options = LatticeOptions {
                beam: width,
                bigram: Some(Arc::clone(&table)),
                bigram_memo: memo_enabled.then_some(&new_memo),
                ..LatticeOptions::default()
            };
            let old_options = LatticeOptions {
                bigram_memo: memo_enabled.then_some(&old_memo),
                ..options.clone()
            };
            for _ in 0..2 {
                let (old, before) = measure(|| legacy::decode_graph(&graph, &old_options, None));
                let (new, after) = measure(|| experiment::decode_graph(&graph, &options, None));
                assert_paths(&new, &old);
                assert_eq!(
                    before - after,
                    (width - 1) * previous.len(),
                    "{width}/{memo_enabled}"
                );
            }
        }
    }
}

#[test]
fn row_prefix_keys_and_memo_transitions_match_complete_word_keys() {
    let directory = tempfile::tempdir().unwrap();
    let tables = [
        Arc::new(pair_table(
            directory.path(),
            "first.bin",
            &[
                ("合成前词", "后词", 1.5),
                ("", "后词", -0.0),
                ("a\0b", "c", -2.0),
            ],
        )),
        Arc::new(pair_table(directory.path(), "empty.bin", &[])),
        Arc::new(pair_table(
            directory.path(),
            "second.bin",
            &[("合成前词", "后词", -3.0), (SENTENCE_START, "后词", 0.75)],
        )),
    ];
    let mut old = BigramMemo::default();
    let mut new = BigramMemo::default();
    for full in [false, true] {
        for &at in &[0, 1, 2, 0] {
            let table = &tables[at];
            if full {
                for index in 0..BIGRAM_MEMO_CAPACITY {
                    old.scores.insert(index as u64, index as f32);
                    new.scores.insert(index as u64, index as f32);
                }
            }
            for previous in ["", SENTENCE_START, "合成前词", "a\0b", "合成𠀀🙂"] {
                let prefix = table.bigram_prefix(previous);
                for _ in 0..2 {
                    for next in ["", "后词", "c", "a\0b", "合成𠀀🙂"] {
                        let key = prefix.and_then(|prefix| prefix.key(next));
                        assert_eq!(key, table.bigram_key(previous, next));
                        let expected = legacy::score(&mut old, table, previous, next);
                        let actual = experiment::score_key(&mut new, table, key);
                        assert_eq!(actual.to_bits(), expected.to_bits());
                        assert_eq!(new.table, old.table);
                        assert_eq!(new.scores, old.scores);
                        assert_eq!(actual.to_bits(), table.bigram(previous, next).to_bits());
                    }
                }
            }
        }
    }
    // 大量键与独立旧入口逐项核对，包含多字节、内嵌分隔符及空串。
    for table in &tables {
        for index in 0..2048 {
            let previous = format!("合成{index}\0𠀀");
            let prefix = table.bigram_prefix(&previous);
            for next in ["", "词", "\0", "🙂", "后词'尾"] {
                assert_eq!(
                    prefix.and_then(|prefix| prefix.key(next)),
                    table.bigram_key(&previous, next)
                );
            }
        }
    }
}

#[test]
fn prefix_decode_keeps_complete_paths_and_allocation_requests() {
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
                let graph = graph(columns, width, seed, false);
                let extra = super::capacity_tests::graph(columns, width, seed + 1, true);
                for mode in 0..32 {
                    let old_memo = RefCell::new(BigramMemo::default());
                    let new_memo = RefCell::new(BigramMemo::default());
                    let options = LatticeOptions {
                        bigram: (mode & 1 != 0).then(|| Arc::clone(&table)),
                        bigram_memo: (mode & 8 != 0).then_some(&new_memo),
                        trigram: (mode & 2 != 0).then(|| Arc::clone(&trigram)),
                        personal: (mode & 4 != 0).then_some(&personal),
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
                            measure(|| experiment::decode_graph(&graph, &options, extra));
                        assert_paths(&new, &old);
                        assert_eq!(new.capacity(), old.capacity());
                        assert_eq!(after.allocations, before.allocations);
                        assert_eq!(after.peak_bytes, before.peak_bytes);
                        assert_eq!(after.remaining_bytes, before.remaining_bytes);
                        assert_eq!(after.minimum_bytes, before.minimum_bytes);
                        assert_eq!(new_memo.borrow().scores, old_memo.borrow().scores);
                        assert_eq!(new_memo.borrow().table, old_memo.borrow().table);
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "同机 release 配对计时，普通 CI 不依赖共享机器速度"]
// 隔离后的测试函数入口不同于最初两轮生产候选计时，结果必须另行记录。
fn bigram_prefix_release_comparison() {
    use super::capacity_tests::graph;
    use std::{hint::black_box, time::Instant};
    let directory = tempfile::tempdir().unwrap();
    let table = Arc::new(pair_table(
        directory.path(),
        "pairs.bin",
        &[(SENTENCE_START, "甲", 1.0), ("甲", "乙", -2.0)],
    ));
    let empty_table = Arc::new(pair_table(directory.path(), "empty.bin", &[]));
    let mut regressions = Vec::new();
    for mode in 0..4 {
        for (columns, width, empty) in [
            (0, 0, false),
            (1, 1, false),
            (4, 4, false),
            (4, 32, false),
            (12, 8, false),
            (12, 32, false),
            (32, 32, false),
            (2, 1, true),
            (4, 4, true),
        ] {
            let mut graph = graph(columns, width, 0, false);
            if empty {
                if columns == 2 {
                    for edge in &mut graph[0] {
                        edge.word = "合成前词测试词".to_owned();
                    }
                }
                for edge in graph.last_mut().unwrap() {
                    edge.word.clear();
                }
            }
            let old_memo = RefCell::new(BigramMemo::default());
            let new_memo = RefCell::new(BigramMemo::default());
            let options = LatticeOptions {
                bigram: (mode > 0)
                    .then(|| Arc::clone(if mode == 3 { &empty_table } else { &table })),
                bigram_memo: (mode == 2).then_some(&new_memo),
                ..LatticeOptions::default()
            };
            let old_options = LatticeOptions {
                bigram_memo: (mode == 2).then_some(&old_memo),
                ..options.clone()
            };
            let run = |arm, repeats| {
                let start = Instant::now();
                for _ in 0..repeats {
                    if arm == 0 {
                        black_box(legacy::decode_graph(
                            black_box(&graph),
                            black_box(&old_options),
                            None,
                        ));
                    } else {
                        black_box(experiment::decode_graph(
                            black_box(&graph),
                            black_box(&options),
                            None,
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
                let mut times = [0.0; 2];
                for arm in if pair % 2 == 0 { [0, 1] } else { [1, 0] } {
                    times[arm] = run(arm, repeats);
                }
                ratios.push(times[1] / times[0]);
            }
            ratios.sort_by(f64::total_cmp);
            let median = (ratios[19] + ratios[20]) * 0.5;
            eprintln!("prefix mode={mode} columns={columns} width={width} empty={empty} repeats={repeats} median={median:.6} p10={:.6} p90={:.6}",ratios[4],ratios[35]);
            if median >= 1.05 {
                regressions.push(format!("{mode}/{columns}/{width}/{empty}: {median}"));
            }
        }
    }
    assert!(
        regressions.is_empty(),
        "回退超过预算: {}",
        regressions.join("; ")
    );
}
