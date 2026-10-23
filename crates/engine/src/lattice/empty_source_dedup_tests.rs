use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::lattice::decode::tests::{lookup, row, syllables, table};
use crate::lattice::merge::block_buffer_tests::favouring_model;
use std::sync::Arc;

#[allow(clippy::too_many_arguments)]
fn baseline_merge_lattice_candidates(
    candidates: &mut Vec<WordItem>,
    syllables: &[String],
    lookup: &mut LatticeLookup<'_>,
    typed_pinyin: &str,
    options: &LatticeOptions<'_>,
    typo_source: Option<&mut TypoEdgeSource<'_>>,
    rerankers: &mut [NeuralReranker],
    rescoring_context: &str,
) -> Option<TypoSentence> {
    if syllables.len() < 2 || !has_only_complete_pinyin_segments(syllables) {
        return None;
    }
    let graph = build_graph(syllables, lookup, options);
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
    let dedup_size = candidates.len().saturating_add(paths.len());
    let mut already =
        (dedup_size > SMALL_SENTENCE_MERGE).then(|| HashSet::with_capacity(dedup_size));
    if let Some(already) = already.as_mut() {
        already.extend(candidates.iter().map(|item| item.word.as_str()));
    }
    let block = if rerankers.is_empty() {
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
        let mut keyboard = None;
        for reranker in rerankers
            .iter_mut()
            .filter(|reranker| reranker.source == CandidateSource::NeuralKeyboard)
        {
            if let Some(order) = reranker.score_order(&paths, rescoring_context) {
                keyboard = Some(order);
            }
        }
        let path_count = paths.len();
        let ranked = keyboard
            .as_deref()
            .map(|order| order.iter().copied().chain(order.len()..path_count));
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

#[test]
fn absent_sentence_sources_skip_only_the_unused_large_dedup_table() {
    let keys = syllables("a'ba'ca");
    assert!(has_only_complete_pinyin_segments(&keys));
    let mut cases = 0;
    let mut saved_cases = 0;
    for n in [0, 1, 2, 6, 13, 65] {
        let characters: Vec<_> = (0..n)
            .map(|index| char::from_u32(0x4e00 + index as u32).unwrap())
            .collect();
        let mut rows = table(&[("ba", &[("吧", 1000)]), ("ca", &[("嚓", 1000)])]);
        rows.insert(
            "a".into(),
            characters
                .iter()
                .enumerate()
                .map(|(index, value)| row("a", &value.to_string(), 100_000 - index as i64))
                .collect(),
        );
        let vocabulary: Vec<_> = characters
            .iter()
            .copied()
            .chain(['吧', '嚓', '叭', '合', '成', '😀'])
            .collect();
        let model = Arc::new(favouring_model(
            &vocabulary,
            characters.last().map(std::slice::from_ref).unwrap_or(&[]),
        ));
        for filler in [0, 62, 63, 64] {
            for duplicates in [0, n] {
                for include_lattice_best in [false, true] {
                    for show_next_on_duplicate in [false, true] {
                        for emit in [0, 1] {
                            let options = LatticeOptions {
                                nbest: n,
                                beam: n.max(32),
                                span_limit: n.max(32),
                                emit,
                                include_lattice_best,
                                show_next_on_duplicate,
                                ..LatticeOptions::default()
                            };
                            let graph = build_graph(&keys, &mut lookup(&rows), &options);
                            let decoded = decode_graph(&graph, &options, None);
                            assert_eq!(decoded.len(), n);
                            let mut input: Vec<_> = decoded
                                .iter()
                                .take(duplicates)
                                .map(|path| {
                                    WordItem::new(
                                        "abaca",
                                        &path.sentence,
                                        123,
                                        CandidateSource::Database,
                                        "a'ba'ca",
                                    )
                                })
                                .collect();
                            input.extend((0..filler).map(|index| {
                                WordItem::new(
                                    "abaca",
                                    format!("占位{index}"),
                                    456,
                                    CandidateSource::CloudSuggestion,
                                    "",
                                )
                            }));
                            for mode in 0..4 {
                                let make_models = || match mode {
                                    0 => vec![],
                                    1 => vec![NeuralReranker::new(
                                        CandidateSource::Generated,
                                        model.clone(),
                                    )],
                                    2 => vec![NeuralReranker::new(
                                        CandidateSource::NeuralKeyboard,
                                        model.clone(),
                                    )],
                                    _ => vec![
                                        NeuralReranker::new(
                                            CandidateSource::NeuralKeyboard,
                                            model.clone(),
                                        ),
                                        NeuralReranker::new(
                                            CandidateSource::Generated,
                                            model.clone(),
                                        ),
                                        NeuralReranker::new(
                                            CandidateSource::NeuralKeyboard,
                                            model.clone(),
                                        ),
                                    ],
                                };
                                for warm in [false, true] {
                                    for typo_enabled in [false, true] {
                                        let mut old_models = make_models();
                                        let mut new_models = make_models();
                                        let run =
                                            |baseline: bool, models: &mut [NeuralReranker]| {
                                                let mut candidates = input.clone();
                                                let mut literal = None;
                                                let mut source = |path: &SentencePath| {
                                                    literal = Some(path.clone());
                                                    vec![super::super::decode::TypoEdge {
                                                        start: 1,
                                                        end: 3,
                                                        key: "ba'ca".into(),
                                                        value: "叭嚓".into(),
                                                        weight: 900_000,
                                                        penalty: 0.5,
                                                    }]
                                                };
                                                let merge = if baseline {
                                                    baseline_merge_lattice_candidates
                                                } else {
                                                    merge_lattice_candidates
                                                };
                                                let typo = merge(
                                                    &mut candidates,
                                                    &keys,
                                                    &mut lookup(&rows),
                                                    "abaca",
                                                    &options,
                                                    if typo_enabled {
                                                        Some(&mut source)
                                                    } else {
                                                        None
                                                    },
                                                    models,
                                                    if warm { "合成😀上下文" } else { "" },
                                                );
                                                (candidates, typo, literal)
                                            };
                                        if warm {
                                            assert_eq!(
                                                run(true, &mut old_models),
                                                run(false, &mut new_models)
                                            );
                                        }
                                        let (old, before) = count(|| run(true, &mut old_models));
                                        let (new, after) = count(|| run(false, &mut new_models));
                                        assert_eq!(new, old);
                                        assert_eq!(new.0.capacity(), old.0.capacity());
                                        let no_sources = mode > 0
                                            && !include_lattice_best
                                            && (mode == 1 || n < 2);
                                        let saved = usize::from(
                                            n > 0
                                                && no_sources
                                                && input.len() + n > SMALL_SENTENCE_MERGE,
                                        );
                                        let order_savings = if n < 2 {
                                            0
                                        } else {
                                            usize::from(mode == 2) + 2 * usize::from(mode == 3)
                                        };
                                        // 有来源的单路径另外少建一张集合，无来源收益继续单独计量。
                                        let paths = if mode == 0 && options.emit > 0 {
                                            n.min(options.emit)
                                        } else {
                                            n
                                        };
                                        let single_path_savings = usize::from(
                                            paths == 1
                                                && !no_sources
                                                && input.len() + paths > SMALL_SENTENCE_MERGE,
                                        );
                                        assert_eq!(after + saved + order_savings + single_path_savings, before, "路径={n}/占位={filler}/重复={duplicates}/字面={include_lattice_best}/继续={show_next_on_duplicate}/emit={emit}/模型={mode}/热={warm}/纠错={typo_enabled}");
                                        if n > 0 && no_sources {
                                            assert_eq!(new.0, input);
                                            if typo_enabled {
                                                assert!(new.1.is_some());
                                                assert_eq!(new.2.as_ref().unwrap(), &decoded[0]);
                                            }
                                        }
                                        for (old_model, new_model) in
                                            old_models.iter_mut().zip(&mut new_models)
                                        {
                                            assert_eq!(
                                                old_model.score_order(&decoded, "合成后续上下文"),
                                                new_model.score_order(&decoded, "合成后续上下文")
                                            );
                                        }
                                        saved_cases += saved;
                                        cases += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, 6144);
    assert_eq!(saved_cases, 544);
    eprintln!("无来源去重表: {cases}组中{saved_cases}组精确少一次申请");
}
