use super::*;

#[path = "decode_collocation_tail_frozen.rs"]
mod frozen;

#[cfg(debug_assertions)]
thread_local! {
    static TAIL_CALLS: Cell<Option<TailCounts>> = const { Cell::new(None) };
}

#[cfg(debug_assertions)]
#[derive(Clone, Copy, Debug, Default)]
struct TailCounts {
    traversals: usize,
    strings: usize,
}

#[cfg(debug_assertions)]
pub(super) fn record_tail_string() {
    TAIL_CALLS.with(|calls| {
        if let Some(mut value) = calls.get() {
            value.strings += 1;
            calls.set(Some(value));
        }
    });
}

#[cfg(debug_assertions)]
pub(super) fn record_tail() {
    TAIL_CALLS.with(|calls| {
        if let Some(mut value) = calls.get() {
            value.traversals += 1;
            calls.set(Some(value));
        }
    });
}

#[cfg(debug_assertions)]
fn count_tails<T>(run: impl FnOnce() -> T) -> (T, TailCounts) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            TAIL_CALLS.with(|calls| calls.set(None));
        }
    }
    TAIL_CALLS.with(|calls| assert!(calls.replace(Some(TailCounts::default())).is_none()));
    let reset = Reset;
    let value = run();
    let count = TAIL_CALLS.with(|calls| calls.get().unwrap());
    drop(reset);
    (value, count)
}

fn edge(end: usize, word: String, score: f64) -> Edge {
    Edge {
        end,
        key: format!("reading-{word}"),
        word,
        log_prob: score,
        typo: false,
    }
}

#[cfg(debug_assertions)]
#[test]
fn one_tail_per_hypothesis_regardless_of_fanout() {
    let scorer = |_: &str, _: &str, _: bool| 0.25;
    let options = LatticeOptions {
        collocation_scorer: Some(&scorer),
        collocation_weight: 1.0,
        ..LatticeOptions::default()
    };
    for width in [1, 2, 8, 32, 128] {
        let graph = vec![
            vec![edge(1, "合成上文🙂".into(), -1.0)],
            (0..width)
                .map(|index| edge(2, format!("词{index}"), -2.0))
                .collect(),
        ];
        let (before, old_calls) =
            count_tails(|| frozen::frozen_decode_graph(&graph, &options, None));
        let (after, new_calls) = count_tails(|| decode_graph(&graph, &options, None));
        assert_eq!(after, before);
        assert_eq!(old_calls.traversals, width + 1);
        assert_eq!(old_calls.strings, 2 * width + 1);
        assert_eq!(new_calls.traversals, 2, "扇出 {width} 不得重复回溯相同假设");
        assert_eq!(new_calls.strings, 3, "扇出 {width} 不得逐边校验相同尾窗");
    }
}

#[cfg(debug_assertions)]
#[test]
fn inactive_or_edgeless_hypotheses_do_not_build_tails() {
    let scorer = |_: &str, _: &str, _: bool| panic!("不应调用评分器");
    for weight in [0.0, -1.0, f64::NAN] {
        let options = LatticeOptions {
            collocation_scorer: Some(&scorer),
            collocation_weight: weight,
            ..LatticeOptions::default()
        };
        let graph = vec![vec![edge(1, "甲".into(), 0.0)]];
        assert_eq!(
            count_tails(|| decode_graph(&graph, &options, None))
                .1
                .traversals,
            0
        );
    }
    let scorer = |_: &str, _: &str, _: bool| 1.0;
    let options = LatticeOptions {
        collocation_scorer: Some(&scorer),
        collocation_weight: 1.0,
        ..LatticeOptions::default()
    };
    for graph in [vec![], vec![vec![]], vec![vec![], vec![]]] {
        assert_eq!(
            count_tails(|| decode_graph(&graph, &options, None))
                .1
                .traversals,
            0
        );
    }
    let graph = vec![vec![edge(1, "甲".into(), 0.0)], vec![]];
    assert_eq!(
        count_tails(|| decode_graph(&graph, &options, None))
            .1
            .traversals,
        1
    );
    assert_eq!(
        count_tails(|| decode_graph(&graph, &LatticeOptions::default(), None))
            .1
            .traversals,
        0
    );
}

fn fixture(columns: usize, width: usize, ties: bool) -> Graph {
    (0..columns)
        .map(|start| {
            (start + 1..=columns.min(start + 3))
                .flat_map(|end| {
                    (0..width).map(move |index| {
                        edge(
                            end,
                            format!("甲🙂a-{start}-{index}-长词跨越八码点"),
                            if ties { 0.0 } else { -(index as f64) * 0.125 },
                        )
                    })
                })
                .collect()
        })
        .collect()
}

fn assert_paths_equal(before: &[SentencePath], after: &[SentencePath]) {
    assert_eq!(before.len(), after.len());
    for (old, new) in before.iter().zip(after) {
        assert_eq!(old.sentence, new.sentence);
        assert_eq!(old.key, new.key);
        assert_eq!(old.words, new.words);
        assert_eq!(old.typo_edges, new.typo_edges);
        assert_eq!(old.log_prob.to_bits(), new.log_prob.to_bits());
    }
}

#[test]
fn complete_decode_and_scorer_events_match_frozen_entry() {
    use crate::lattice::ngram::tests::{pair_table, triple_table};
    use crate::lattice::personal::PersonalTransition;

    let directory = tempfile::tempdir().unwrap();
    let first = "甲🙂a-0-0-长词跨越八码点";
    let second = "甲🙂a-1-0-长词跨越八码点";
    let bigram = Arc::new(pair_table(
        directory.path(),
        "bigram.bin",
        &[(first, second, 1.5)],
    ));
    let trigram = Arc::new(triple_table(
        directory.path(),
        "trigram.bin",
        &[(SENTENCE_START, first, second, -0.75)],
    ));
    let mut personal = PersonalNgram::default();
    personal.add(&PersonalTransition {
        earlier: String::new(),
        previous: first.into(),
        word: second.into(),
        times: 20,
    });
    for columns in [0, 1, 4, 10] {
        for width in [1, 4] {
            for ties in [false, true] {
                let graph = fixture(columns, width, ties);
                let mut extra = fixture(columns, 1, ties);
                for edges in &mut extra {
                    for edge in edges {
                        edge.typo = true;
                        edge.word.push('乙');
                    }
                }
                for (beam, nbest) in [(0, 0), (1, 1), (4, 2), (4, 8), (32, 6)] {
                    for use_extra in [false, true] {
                        for mode in 0..4 {
                            let events = RefCell::new(Vec::new());
                            let scorer = |tail: &str, word: &str, rear: bool| {
                                events
                                    .borrow_mut()
                                    .push((tail.to_owned(), word.to_owned(), rear));
                                -((tail.chars().count() + word.len()) as f64) * 0.03125
                                    + f64::from(rear)
                            };
                            let memo = RefCell::new(BigramMemo::default());
                            let options = LatticeOptions {
                                beam,
                                nbest,
                                collocation_scorer: (mode != 0)
                                    .then_some(&scorer as &CollocationScorer<'_>),
                                collocation_weight: if mode == 1 { 0.0 } else { 1.5 },
                                bigram: (mode == 3).then(|| bigram.clone()),
                                trigram: (mode == 3).then(|| trigram.clone()),
                                bigram_memo: (mode == 3).then_some(&memo),
                                personal: (mode == 3).then_some(&personal),
                                ..LatticeOptions::default()
                            };
                            let extra = use_extra.then_some(&extra);
                            let before = frozen::frozen_decode_graph(&graph, &options, extra);
                            let old_events = events.replace(Vec::new());
                            let after = decode_graph(&graph, &options, extra);
                            assert_paths_equal(&before, &after);
                            assert_eq!(*events.borrow(), old_events);
                        }
                    }
                }
            }
        }
    }
}

#[inline(never)]
fn current_decode(graph: &Graph, options: &LatticeOptions<'_>) -> Vec<SentencePath> {
    decode_graph(graph, options, None)
}

#[inline(never)]
fn old_decode(graph: &Graph, options: &LatticeOptions<'_>) -> Vec<SentencePath> {
    frozen::frozen_decode_graph(graph, options, None)
}

#[allow(clippy::assertions_on_constants)]
#[test]
#[ignore = "完整解码入口配对计时，需 release、普通分配器与独立进程监测"]
fn collocation_tail_complete_decode_pairs() {
    use crate::lattice::octagram::{encode, GramConfig, GramDb};
    use std::hint::black_box;
    use std::time::{Duration, Instant};

    assert!(std::env::var_os("MSIME_TAIL_SYSTEM_ALLOCATOR").is_some());
    assert!(!cfg!(debug_assertions), "必须以 release 运行");
    let directory = tempfile::tempdir().unwrap();
    let path = crate::lattice::octagram::fixture::gram_file(
        directory.path(),
        "synthetic.gram",
        &[([encode("甲"), encode("乙")].concat(), 200_000)],
    );
    let db = GramDb::shared(&path).unwrap();
    let config = GramConfig::default();
    let gram = |tail: &str, word: &str, rear: bool| db.query(tail, word, rear, &config);
    let cheap = |tail: &str, word: &str, rear: bool| {
        -((tail.len() + word.len()) as f64) * 0.03125 + f64::from(rear)
    };
    for columns in [4, 12] {
        for width in [1, 4, 16] {
            let graph = fixture(columns, width, false);
            for mode in ["off", "cheap", "gram"] {
                let scorer: Option<&CollocationScorer<'_>> = match mode {
                    "cheap" => Some(&cheap),
                    "gram" => Some(&gram),
                    _ => None,
                };
                let options = LatticeOptions {
                    collocation_scorer: scorer,
                    collocation_weight: 1.0,
                    ..LatticeOptions::default()
                };
                assert_paths_equal(
                    &old_decode(&graph, &options),
                    &current_decode(&graph, &options),
                );
                let batch = |old: bool, repeats: usize| {
                    let start = Instant::now();
                    for _ in 0..repeats {
                        black_box(if old {
                            old_decode(black_box(&graph), black_box(&options))
                        } else {
                            current_decode(black_box(&graph), black_box(&options))
                        });
                    }
                    start.elapsed()
                };
                let mut repeats = 1;
                while batch(true, repeats).min(batch(false, repeats)) < Duration::from_millis(10) {
                    repeats *= 2;
                }
                for pair in 0..21 {
                    let (before, after) = if pair % 2 == 0 {
                        (batch(true, repeats), batch(false, repeats))
                    } else {
                        let after = batch(false, repeats);
                        (batch(true, repeats), after)
                    };
                    eprintln!(
                        "tail-pair,{}",
                        serde_json::json!({
                            "columns": columns, "width": width, "mode": mode,
                            "pair": pair, "repeats": repeats,
                            "old_ns": before.as_nanos() as u64,
                            "new_ns": after.as_nanos() as u64
                        })
                    );
                }
            }
        }
    }
}
