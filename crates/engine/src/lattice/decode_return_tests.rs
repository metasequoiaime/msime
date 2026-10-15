use super::super::ngram::tests::{pair_table, triple_table};
use super::super::personal::tests::record_run;
use super::capacity_tests::{assert_paths, graph};
use super::*;

#[path = "decode_return_legacy_tests.rs"]
mod legacy;

// 旧布局预算继续隔离完整物化的成本；当前产品的提前停止由下方独立精确预算验证。
pub(super) fn full_materialization_decode(
    graph: &Graph,
    options: &LatticeOptions<'_>,
    extra: Option<&Graph>,
) -> Vec<SentencePath> {
    legacy::decode_graph(graph, options, extra)
}

fn distinct_graph(width: usize) -> Graph {
    vec![(0..width)
        .map(|index| Edge {
            end: 1,
            word: format!("合成{index}"),
            key: format!("k{index}"),
            log_prob: -(index as f64),
            typo: index % 3 == 0,
        })
        .collect()]
}

#[test]
fn unused_small_beam_paths_are_not_materialized() {
    use crate::ime::personal_rerank::allocations::measure;
    for (width, nbest) in [
        (1, 1),
        (4, 1),
        (32, 0),
        (32, 6),
        (64, 12),
        (65, 6),
        (128, 65),
    ] {
        let graph = distinct_graph(width);
        let options = LatticeOptions {
            beam: width,
            nbest,
            ..LatticeOptions::default()
        };
        let (old, before) = measure(|| legacy::decode_graph(&graph, &options, None));
        let (new, after) = measure(|| decode_graph(&graph, &options, None));
        assert_paths(&new, &old);
        assert_eq!(new.capacity(), old.capacity());
        assert_eq!(after.remaining_bytes, before.remaining_bytes);
        // 每条单词非空路径需要词向量、键向量、词串、句子和读音五次申请；大末列保留原回退。
        let saved = if width <= SMALL_SENTENCE_PATHS {
            (width - nbest.min(width)) * 5
        } else {
            0
        };
        assert_eq!(
            before.allocations - after.allocations,
            saved,
            "{width}/{nbest}"
        );
        if saved > 0 {
            assert!(after.peak_bytes < before.peak_bytes);
        } else {
            assert_eq!(after.peak_bytes, before.peak_bytes);
        }
        eprintln!("return-budget {width}/{nbest}: {before:?} -> {after:?}");
    }
}

#[test]
fn duplicates_keep_the_first_path_without_joining_unused_keys() {
    use crate::ime::personal_rerank::allocations::measure;
    let mut graph = distinct_graph(32);
    for (index, edge) in graph[0].iter_mut().enumerate() {
        edge.word = format!("合成{}", index % 4);
    }
    let options = LatticeOptions {
        nbest: 12,
        ..LatticeOptions::default()
    };
    let (old, before) = measure(|| legacy::decode_graph(&graph, &options, None));
    let (new, after) = measure(|| decode_graph(&graph, &options, None));
    assert_paths(&new, &old);
    assert_eq!(new.len(), 4);
    assert_eq!(new[0].key, "k0");
    assert_eq!(before.allocations - after.allocations, 28);
    assert_eq!(after.remaining_bytes, before.remaining_bytes);
}

#[test]
fn unique_budget_is_applied_before_trigram_reranking() {
    let directory = tempfile::tempdir().unwrap();
    let trigram = Arc::new(triple_table(
        directory.path(),
        "triples.bin",
        &[
            (SENTENCE_START, "丁", "乙", 2.0),
            (SENTENCE_START, "戊", "乙", 100.0),
        ],
    ));
    let graph = vec![
        ["甲", "丁", "戊"]
            .iter()
            .enumerate()
            .map(|(index, word)| Edge {
                end: 1,
                word: (*word).to_owned(),
                key: format!("k{index}"),
                log_prob: -(index as f64),
                typo: false,
            })
            .collect(),
        vec![Edge {
            end: 2,
            word: "乙".to_owned(),
            key: "yi".to_owned(),
            log_prob: 0.0,
            typo: false,
        }],
        vec![Edge {
            end: 3,
            word: "丙".to_owned(),
            key: "bing".to_owned(),
            log_prob: 0.0,
            typo: false,
        }],
    ];
    let options = LatticeOptions {
        nbest: 2,
        trigram: Some(trigram),
        ..LatticeOptions::default()
    };
    let paths = decode_graph(&graph, &options, None);
    assert_eq!(
        paths
            .iter()
            .map(|path| path.sentence.as_str())
            .collect::<Vec<_>>(),
        ["丁乙丙", "甲乙丙"]
    );
    assert_paths(&paths, &legacy::decode_graph(&graph, &options, None));
}

#[test]
fn early_unique_budget_preserves_complete_legacy_results() {
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
                    for nbest in [0, 1, 5, 12, 64, 65] {
                        for mode in 0..32 {
                            let new_memo = RefCell::new(BigramMemo::default());
                            let old_memo = RefCell::new(BigramMemo::default());
                            let options = LatticeOptions {
                                beam,
                                nbest,
                                bigram: (mode & 1 != 0).then(|| Arc::clone(&bigram)),
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
                                let old = legacy::decode_graph(&graph, &old_options, extra);
                                let new = decode_graph(&graph, &options, extra);
                                assert_paths(&new, &old);
                                assert_eq!(new.capacity(), old.capacity());
                            }
                        }
                    }
                }
            }
        }
    }
    for width in [0, 1, 32, 64, 65, 128] {
        for nbest in [0, 1, 6, 12, 64, 65, 129] {
            let mut graph = distinct_graph(width);
            for duplicates in [false, true] {
                if duplicates {
                    for (index, edge) in graph[0].iter_mut().enumerate() {
                        edge.word = if index % 7 == 0 {
                            String::new()
                        } else {
                            format!("合成{}", index % 9)
                        };
                    }
                }
                let options = LatticeOptions {
                    beam: width,
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
}

#[test]
#[ignore = "同机 release 配对计时，普通 CI 不依赖共享机器速度"]
fn return_budget_release_comparison() {
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
    for (mode, nbest) in [(0, 6), (1, 6), (2, 6), (2, 12)] {
        for (columns, width) in [
            (1, 1),
            (4, 4),
            (4, 32),
            (12, 8),
            (12, 32),
            (32, 32),
            (1, 65),
            (1, 128),
        ] {
            let graph = graph(columns, width, 0, false);
            let old_memo = RefCell::new(BigramMemo::default());
            let new_memo = RefCell::new(BigramMemo::default());
            let options = LatticeOptions {
                beam: if width > 64 { width } else { 32 },
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
            // 取两臂各三轮中的最快批次校准至至少 50 ms，保持事前 5% 回退预算。
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
            eprintln!("return-budget mode={mode} nbest={nbest} columns={columns} width={width} repeats={repeats} median-new/old={median:.6} p10={:.6} p90={:.6}", ratios[4], ratios[35]);
            if median >= 1.05 {
                regressions.push(format!("{mode}/{nbest}/{columns}/{width}: {median}"));
            }
        }
    }
    assert!(
        regressions.is_empty(),
        "回退超过预算: {}",
        regressions.join("; ")
    );
}
