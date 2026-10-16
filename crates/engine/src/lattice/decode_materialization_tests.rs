use super::super::personal;
use super::capacity_tests::assert_paths;
use super::*;
use crate::ime::personal_rerank::allocations::measure;

#[path = "decode_materialization_legacy_tests.rs"]
pub(super) mod legacy;

fn repeated_graph(columns: usize, width: usize, unique: usize) -> Graph {
    (0..columns)
        .map(|start| {
            (0..if start == 0 { width } else { 1 })
                .map(|index| Edge {
                    end: start + 1,
                    word: format!("合成{}", if start == 0 { index % unique } else { start }),
                    key: format!("k{start}\'k{index}"),
                    log_prob: -(index as f64),
                    typo: (start + index) % 3 == 0,
                })
                .collect()
        })
        .collect()
}

#[test]
fn duplicate_paths_clone_words_only_when_retained() {
    for columns in [1, 4, 12] {
        let graph = repeated_graph(columns, 32, 4);
        let options = LatticeOptions {
            beam: 32,
            nbest: 32,
            ..LatticeOptions::default()
        };
        let (old, before) = measure(|| legacy::decode_graph(&graph, &options, None));
        let (new, after) = measure(|| decode_graph(&graph, &options, None));
        assert_paths(&new, &old);
        assert_eq!(new.len(), 4);
        assert_eq!(new.capacity(), old.capacity());
        for (new, old) in new.iter().zip(&old) {
            assert_eq!(new.words.capacity(), old.words.capacity());
        }
        assert_eq!(after.remaining_bytes, before.remaining_bytes);
        // 四条输出各省一份键指针页，二十八条重复项省两个向量及全部词串，合计扣去一次共享回溯页。
        let saved = 4 - 1 + 28 * (columns + 2);
        assert_eq!(
            before.allocations - after.allocations,
            saved,
            "{columns}: {before:?} -> {after:?}"
        );
        assert!(after.peak_bytes < before.peak_bytes);
        eprintln!("trace-materialization {columns}: {before:?} -> {after:?}");
    }
}

#[test]
fn trace_buffer_keeps_owned_fields_and_empty_key_separators() {
    for width in [1, 4, 32, 64, 65, 80] {
        for columns in [0, 1, 4] {
            for kind in 0..4 {
                let mut graph = repeated_graph(columns, width, 4);
                for (start, edges) in graph.iter_mut().enumerate() {
                    for (index, edge) in edges.iter_mut().enumerate() {
                        edge.key = if (start + index) % 2 == 0 {
                            String::new()
                        } else {
                            "'🔤'".to_owned()
                        };
                        if kind == 1 && start % 2 == 0 || kind == 2 {
                            edge.word.clear();
                        } else if kind == 3 {
                            edge.word = "同句😀".to_owned();
                        }
                    }
                }
                for nbest in [0, 1, 6, 80] {
                    let options = LatticeOptions {
                        beam: 80,
                        nbest,
                        ..LatticeOptions::default()
                    };
                    let (old, before) = measure(|| legacy::decode_graph(&graph, &options, None));
                    let (new, after) = measure(|| decode_graph(&graph, &options, None));
                    assert_paths(&new, &old);
                    assert_eq!(new.capacity(), old.capacity());
                    for (new, old) in new.iter().zip(&old) {
                        assert_eq!(new.words.capacity(), old.words.capacity());
                        assert_eq!(new.sentence.capacity(), old.sentence.capacity());
                        assert_eq!(new.key.capacity(), old.key.capacity());
                        assert!(new
                            .words
                            .iter()
                            .zip(&old.words)
                            .all(|(new, old)| new.capacity() == old.capacity()));
                    }
                    assert_eq!(after.remaining_bytes, before.remaining_bytes);
                    assert!(after.allocations <= before.allocations);
                    assert!(
                        after.peak_bytes <= before.peak_bytes,
                        "{columns}/{width}/{kind}/{nbest}: {before:?} -> {after:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn borrowed_trace_preserves_language_models_and_extra_graph() {
    use super::super::ngram::tests::{pair_table, triple_table};
    use super::super::personal::tests::record_run;
    use super::capacity_tests::graph;
    let directory = tempfile::tempdir().unwrap();
    let bigram = Arc::new(pair_table(
        directory.path(),
        "pairs.bin",
        &[(SENTENCE_START, "甲", 0.75), ("甲", "乙", -0.25)],
    ));
    let trigram = Arc::new(triple_table(
        directory.path(),
        "triples.bin",
        &[(SENTENCE_START, "甲", "乙", 0.75)],
    ));
    let mut personal = PersonalNgram::default();
    record_run(&mut personal, &["甲", "乙", "测试"], 7);
    for columns in [0, 1, 4] {
        for width in [0, 1, 8, 32, 65] {
            let graph = graph(columns, width, 0, false);
            let extra = super::capacity_tests::graph(columns, width, 1, true);
            for beam in [0, 1, 32, 80] {
                for nbest in [0, 1, 6, 80] {
                    for mode in 0..32 {
                        let old_memo = RefCell::new(BigramMemo::default());
                        let new_memo = RefCell::new(BigramMemo::default());
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
                            for (new, old) in new.iter().zip(&old) {
                                assert_eq!(new.words.capacity(), old.words.capacity());
                            }
                            assert_eq!(new_memo.borrow().table, old_memo.borrow().table);
                            assert_eq!(new_memo.borrow().scores, old_memo.borrow().scores);
                        }
                    }
                }
            }
        }
    }
}
