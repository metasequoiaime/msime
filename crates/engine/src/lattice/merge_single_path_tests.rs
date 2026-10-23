// 冻结 61891ad5bd 的合并核心及集合构造；仅改名称并移除注释，解码与输出助手共用当前等价实现。
use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::lattice::decode::tests::{lookup, syllables, table, Table};

/// 当前实现：解码后交给 `merge_decoded`，与生产路径同一组合。
#[allow(clippy::too_many_arguments)]
fn merge_lattice_graph(
    candidates: &mut Vec<WordItem>,
    syllables: &[String],
    typed_pinyin: &str,
    options: &LatticeOptions<'_>,
    typo_source: Option<&mut TypoEdgeSource<'_>>,
    rerankers: &mut [NeuralReranker],
    rescoring_context: &str,
    graph: crate::lattice::decode::Graph,
) -> Option<TypoSentence> {
    let decoded = decode_graph_sentences(syllables, options, typo_source, &graph);
    merge_decoded(
        candidates,
        syllables,
        typed_pinyin,
        options,
        rerankers,
        rescoring_context,
        decoded,
    )
}

#[allow(clippy::too_many_arguments)]
fn baseline_merge_lattice_graph(
    candidates: &mut Vec<WordItem>,
    syllables: &[String],
    typed_pinyin: &str,
    options: &LatticeOptions<'_>,
    typo_source: Option<&mut TypoEdgeSource<'_>>,
    rerankers: &mut [NeuralReranker],
    rescoring_context: &str,
    graph: crate::lattice::decode::Graph,
) -> Option<TypoSentence> {
    let mut paths = decode_graph(&graph, options, None);
    if paths.is_empty() {
        return None;
    }
    let typo = match typo_source {
        Some(source) if syllables.len() >= 3 => {
            let edges = source(&paths[0]);
            if edges.is_empty() {
                None
            } else {
                decode_typo_on_graph(&graph, options, &paths[0], &edges)
            }
        }
        _ => None,
    };

    if rerankers.is_empty() && options.emit > 0 {
        paths.truncate(options.emit);
    }
    let block = if rerankers.is_empty() {
        let already = baseline_existing_sentence_set(candidates, paths.len());
        if let Some(already) = already.as_ref() {
            let mut block = Vec::new();
            let capacity = paths.len();
            for path in paths {
                if already.contains(path.sentence.as_str()) {
                    continue;
                }
                if block.is_empty() {
                    block = Vec::with_capacity(capacity);
                }
                block.push(owned_sentence_row(
                    typed_pinyin,
                    path,
                    CandidateSource::Generated,
                ));
            }
            block
        } else {
            generated_block_linear(candidates, paths, typed_pinyin, options)
        }
    } else {
        let mut keyboard_order = [0; MAX_RERANK_PATHS];
        let mut keyboard = None;
        for reranker in rerankers
            .iter_mut()
            .filter(|reranker| reranker.source == CandidateSource::NeuralKeyboard)
        {
            if let Some(scored) =
                reranker.score_order_into(&paths, rescoring_context, &mut keyboard_order)
            {
                keyboard = Some(scored);
            }
        }
        if !options.include_lattice_best && keyboard.is_none() {
            return typo;
        }
        let already = baseline_existing_sentence_set(candidates, paths.len());
        let path_count = paths.len();
        let ranked = keyboard.map(|scored| {
            keyboard_order[..scored]
                .iter()
                .copied()
                .chain(scored..path_count)
        });
        if let Some(already) = already.as_ref() {
            reranked_block(
                paths,
                0..path_count,
                ranked,
                options,
                typed_pinyin,
                |sentence| already.contains(sentence),
            )
        } else {
            reranked_block(
                paths,
                0..path_count,
                ranked,
                options,
                typed_pinyin,
                |sentence| candidates.iter().any(|item| item.word == sentence),
            )
        }
    };
    if !block.is_empty() {
        let at = whole_sentence_insert_position(candidates, syllables);
        candidates.splice(at..at, block);
    }
    typo
}

fn baseline_existing_sentence_set(
    candidates: &[WordItem],
    path_count: usize,
) -> Option<HashSet<&str>> {
    let dedup_size = candidates.len().saturating_add(path_count);
    (dedup_size > SMALL_SENTENCE_MERGE).then(|| {
        let mut already = HashSet::with_capacity(dedup_size);
        already.extend(candidates.iter().map(|item| item.word.as_str()));
        already
    })
}

fn dictionary() -> Table {
    table(&[("a", &[("阿", 1000), ("啊", 500)]), ("ba", &[("吧", 1000)])])
}

fn input(count: usize) -> Vec<WordItem> {
    (0..count)
        .map(|index| {
            WordItem::new(
                "aba",
                format!("合成{index}"),
                index as i64,
                CandidateSource::CloudSuggestion,
                "",
            )
        })
        .collect()
}

fn run(
    baseline: bool,
    input: &[WordItem],
    rows: &Table,
    options: &LatticeOptions<'_>,
) -> Vec<WordItem> {
    let keys = syllables("a'ba");
    let graph = build_graph(&keys, &mut lookup(rows), options);
    let mut candidates = input.to_vec();
    let merge = if baseline {
        baseline_merge_lattice_graph
    } else {
        merge_lattice_graph
    };
    let typo = merge(
        &mut candidates,
        &keys,
        "aba",
        options,
        None,
        &mut [],
        "",
        graph,
    );
    assert!(typo.is_none());
    candidates
}

#[test]
fn single_path_merge_skips_one_existing_candidate_table() {
    let rows = dictionary();
    let options = LatticeOptions {
        emit: 1,
        ..LatticeOptions::default()
    };
    for size in [64, 65, 128, 512] {
        let candidates = input(size);
        assert_eq!(
            run(true, &candidates, &rows, &options),
            run(false, &candidates, &rows, &options)
        );
        let (old, before) = count(|| run(true, &candidates, &rows, &options));
        let (new, after) = count(|| run(false, &candidates, &rows, &options));
        assert_eq!(old, new);
        assert_eq!(old.capacity(), new.capacity());
        assert_eq!(after + 1, before, "候选={size}，单路径仍创建去重表");
    }
}

fn paths_dictionary(count: usize) -> Table {
    let mut rows = dictionary();
    rows.insert(
        "a".into(),
        (0..count)
            .map(|index| {
                let word = char::from_u32(0x4e00 + index as u32).unwrap().to_string();
                crate::lattice::decode::tests::row("a", &word, 100_000 - index as i64)
            })
            .collect(),
    );
    rows
}

fn repeated_input(size: usize, words: &[String], mode: usize) -> Vec<WordItem> {
    let mut candidates = input(size);
    if mode != 0 {
        let count = words.len().min(candidates.len());
        let offset = if mode == 1 {
            0
        } else {
            candidates.len() - count
        };
        for (slot, word) in candidates[offset..].iter_mut().zip(words) {
            slot.word.clone_from(word);
        }
    }
    candidates
}

#[test]
fn complete_merge_matches_frozen_fields_capacity_and_allocation_delta() {
    for path_count in [0, 1, 2, 6, 65] {
        let rows = paths_dictionary(path_count);
        let keys = syllables("a'ba");
        let options = LatticeOptions {
            nbest: path_count,
            beam: 128,
            span_limit: 128,
            emit: 0,
            ..LatticeOptions::default()
        };
        let graph = build_graph(&keys, &mut lookup(&rows), &options);
        let paths = decode_graph(&graph, &options, None);
        assert_eq!(paths.len(), path_count);
        let words: Vec<_> = paths.iter().map(|path| path.sentence.clone()).collect();
        for size in [0, 63, 64, 65, 128, 512] {
            for mode in 0..3 {
                let candidates = repeated_input(size, &words, mode);
                for emit in [0, 1, 2] {
                    let options = LatticeOptions {
                        emit,
                        ..options.clone()
                    };
                    let (old, before) = count(|| run(true, &candidates, &rows, &options));
                    let (new, after) = count(|| run(false, &candidates, &rows, &options));
                    let output_paths = if emit == 0 {
                        path_count
                    } else {
                        path_count.min(emit)
                    };
                    let saved = usize::from(
                        output_paths == 1 && size + output_paths > SMALL_SENTENCE_MERGE,
                    );
                    assert_eq!(
                        old, new,
                        "路径={path_count} 候选={size} 重复={mode} emit={emit}"
                    );
                    assert_eq!(old.capacity(), new.capacity());
                    assert_eq!(
                        after + saved,
                        before,
                        "路径={path_count} 候选={size} 重复={mode} emit={emit}"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "手动普通分配器 release 完整合并45形状配对"]
fn complete_merge_paired_budget() {
    use std::hint::black_box;
    use std::time::Instant;
    let rows = paths_dictionary(6);
    let keys = syllables("a'ba");
    let decode_options = LatticeOptions {
        nbest: 6,
        ..LatticeOptions::default()
    };
    let graph = build_graph(&keys, &mut lookup(&rows), &decode_options);
    let paths = decode_graph(&graph, &decode_options, None);
    assert_eq!(paths.len(), 6);
    let words: Vec<_> = paths.iter().map(|path| path.sentence.clone()).collect();
    let mut failures = Vec::new();
    println!("pair,size,emit,mode,repeats,pair,old_seconds,new_seconds");
    for size in [32, 64, 65, 128, 512] {
        for emit in [1, 2, 6] {
            let options = LatticeOptions {
                emit,
                nbest: 6,
                ..LatticeOptions::default()
            };
            for mode in 0..3 {
                let candidates = repeated_input(size, &words, mode);
                assert_eq!(
                    run(true, &candidates, &rows, &options),
                    run(false, &candidates, &rows, &options)
                );
                let elapsed = |baseline, repeats| {
                    let start = Instant::now();
                    for _ in 0..repeats {
                        black_box(run(
                            baseline,
                            black_box(&candidates),
                            black_box(&rows),
                            black_box(&options),
                        ));
                    }
                    start.elapsed().as_secs_f64()
                };
                let mut repeats = 1;
                while elapsed(true, repeats) < 0.010 {
                    repeats *= 2;
                }
                let mut ratios = Vec::new();
                for pair in 0..21 {
                    let (old, new) = if pair % 2 == 0 {
                        let old = elapsed(true, repeats);
                        (old, elapsed(false, repeats))
                    } else {
                        let new = elapsed(false, repeats);
                        (elapsed(true, repeats), new)
                    };
                    println!("pair,{size},{emit},{mode},{repeats},{pair},{old:.9},{new:.9}");
                    ratios.push(new / old);
                }
                ratios.sort_by(f64::total_cmp);
                let ratio = ratios[10];
                let budget = if emit == 1 && size >= 64 { 0.98 } else { 1.05 };
                println!("result,{size},{emit},{mode},{ratio:.6},{budget:.2}");
                if ratio >= budget {
                    failures.push((size, emit, mode, ratio, budget));
                }
            }
        }
    }
    assert!(failures.is_empty(), "完整合并配对预算失败：{failures:?}");
}
