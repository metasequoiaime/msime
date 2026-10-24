use super::*;
use crate::ime::personal_rerank::allocations::count;
use crate::lattice::decode::tests::{lookup, row, syllables, table};

#[test]
fn generated_merge_moves_fields_without_changing_output_or_typo() {
    let keys = syllables("a'ba'ca");
    assert!(has_only_complete_pinyin_segments(&keys));
    let mut cases = 0;
    for n in [0, 1, 2, 6, 12, 65, 129] {
        let mut rows = table(&[("ba", &[("吧", 1000)]), ("ca", &[("嚓", 1000)])]);
        rows.insert(
            "a".into(),
            (0..n)
                .map(|index| {
                    row(
                        "a",
                        &char::from_u32(0x4e00 + index as u32).unwrap().to_string(),
                        100_000 - index as i64,
                    )
                })
                .collect(),
        );
        if n > 0 {
            rows.insert("a'ba".into(), vec![row("a'ba", "一吧", 200_000)]);
        }
        for emit in [0, 1, 3, usize::MAX] {
            let options = LatticeOptions {
                nbest: n,
                beam: (n + 1).max(32),
                span_limit: n.max(32),
                emit,
                ..LatticeOptions::default()
            };
            let graph = build_graph(&keys, &mut lookup(&rows), &options);
            let decoded = decode_graph(&graph, &options, None);
            assert_eq!(decoded.len(), n);
            let costs: Vec<_> = decoded
                .iter()
                .map(|path| count(|| path.clone()).1)
                .collect();
            for filler in [0, 61, 64, 65] {
                for duplicates in [0, 1, n] {
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
                    for typo_enabled in [false, true] {
                        let run = |baseline: bool| {
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
                                &mut [],
                                "",
                            );
                            (candidates, typo, literal)
                        };
                        let (old, before) = count(|| run(true));
                        let (new, after) = count(|| run(false));
                        assert_eq!(new, old, "路径={n}/emit={emit}/占位={filler}/重复={duplicates}/纠错={typo_enabled}");
                        assert_eq!(new.0.capacity(), old.0.capacity());
                        let saved: usize = new
                            .0
                            .iter()
                            .filter(|item| item.source == CandidateSource::Generated)
                            .map(|item| {
                                costs[decoded
                                    .iter()
                                    .position(|path| path.sentence == item.word)
                                    .unwrap()]
                            })
                            .sum();
                        // 字段转移预算保持精确，单路径另省一次集合申请。
                        let paths = if emit == 0 { n } else { n.min(emit) };
                        let dedup_saved =
                            usize::from(paths == 1 && input.len() + paths > SMALL_SENTENCE_MERGE);
                        assert_eq!(after + saved + dedup_saved, before, "路径={n}/emit={emit}/占位={filler}/重复={duplicates}/纠错={typo_enabled}");
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 672);
}

#[test]
fn generated_rows_reuse_path_field_storage() {
    let mut paths = Vec::new();
    for index in 0..3 {
        let mut sentence = String::with_capacity(96);
        sentence.push_str(&format!("合成{index}😀"));
        let mut key = String::with_capacity(64);
        key.push_str("a'ba'ca");
        let mut words = Vec::with_capacity(8);
        let mut word = String::with_capacity(80);
        word.push_str(&sentence);
        words.push(word);
        paths.push(SentencePath {
            sentence,
            key,
            log_prob: -1.5,
            words,
            typo_edges: index,
        });
    }
    let original: Vec<_> = paths
        .iter()
        .map(|path| {
            (
                path.sentence.as_ptr(),
                path.sentence.capacity(),
                path.key.as_ptr(),
                path.key.capacity(),
                path.words.as_ptr(),
                path.words.capacity(),
                path.words[0].as_ptr(),
                path.words[0].capacity(),
            )
        })
        .collect();
    let (rows, allocations) =
        count(|| generated_block_linear(&[], paths, "abaca", &LatticeOptions::default()));
    assert_eq!(rows.len(), 3);
    for (item, expected) in rows.iter().zip(original) {
        assert_eq!(
            (
                item.word.as_ptr(),
                item.word.capacity(),
                item.canonical_pinyin.as_ptr(),
                item.canonical_pinyin.capacity(),
                item.sentence_words.as_ptr(),
                item.sentence_words.capacity(),
                item.sentence_words[0].as_ptr(),
                item.sentence_words[0].capacity()
            ),
            expected
        );
    }
    // 句子和键的缓冲区原样移进 `SharedString`，每行只为 `pinyin`、`canonical_pinyin`、`word` 各建一个 `Arc`：1 + 3 + 3 × 3。
    assert_eq!(
        allocations, 13,
        "只有输出块、三行typed_pinyin和每个字段的Arc需分配"
    );
}

// 冻结 adb6100b435990ef8077e93736e112b787839988 的完整合并及助手，仅改名、可见性和移除注释。
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
        if let Some(already) = already.as_mut() {
            let mut block = Vec::new();
            for path in &paths {
                if !already.insert(path.sentence.as_str()) {
                    continue;
                }
                if block.is_empty() {
                    block = Vec::with_capacity(paths.len());
                }
                block.push(baseline_sentence_row(
                    typed_pinyin,
                    path,
                    CandidateSource::Generated,
                ));
            }
            block
        } else {
            baseline_generated_block_linear(candidates, &paths, typed_pinyin)
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
        let ranked = keyboard.as_deref().map(|order| {
            order
                .iter()
                .copied()
                .chain(order.len()..paths.len())
                .map(|index| &paths[index])
        });
        if let Some(already) = already.as_mut() {
            baseline_reranked_block(&paths, ranked, options, typed_pinyin, already)
        } else {
            baseline_reranked_block_linear(candidates, &paths, ranked, options, typed_pinyin)
        }
    };
    if !block.is_empty() {
        let at = whole_sentence_insert_position(candidates, syllables);
        candidates.splice(at..at, block);
    }
    typo
}

fn baseline_sentence_row(
    typed_pinyin: &str,
    path: &SentencePath,
    source: CandidateSource,
) -> WordItem {
    let mut item = WordItem::new(
        typed_pinyin,
        path.sentence.clone(),
        (path.log_prob * 1000.0) as i64,
        source,
        path.key.clone(),
    );
    item.sentence_association = true;
    item.sentence_words = path.words.clone();
    item
}

fn baseline_sentence_seen_linear(
    candidates: &[WordItem],
    block: &[WordItem],
    sentence: &str,
) -> bool {
    candidates
        .iter()
        .chain(block)
        .any(|item| item.word == sentence)
}

fn baseline_generated_block_linear(
    candidates: &[WordItem],
    paths: &[SentencePath],
    typed_pinyin: &str,
) -> Vec<WordItem> {
    let mut block = Vec::new();
    for path in paths {
        if baseline_sentence_seen_linear(candidates, &block, &path.sentence) {
            continue;
        }
        if block.is_empty() {
            block = Vec::with_capacity(paths.len());
        }
        block.push(baseline_sentence_row(
            typed_pinyin,
            path,
            CandidateSource::Generated,
        ));
    }
    block
}

fn baseline_take_linear_sentence<'a>(
    candidates: &[WordItem],
    block: &[WordItem],
    ranked: impl IntoIterator<Item = &'a SentencePath>,
    source: CandidateSource,
    options: &LatticeOptions<'_>,
    typed_pinyin: &str,
) -> Option<WordItem> {
    for path in ranked {
        if !baseline_sentence_seen_linear(candidates, block, &path.sentence) {
            return Some(baseline_sentence_row(typed_pinyin, path, source));
        }
        if !options.show_next_on_duplicate {
            return None;
        }
    }
    None
}

fn baseline_reranked_block_linear<'a>(
    candidates: &[WordItem],
    paths: &[SentencePath],
    keyboard: Option<impl IntoIterator<Item = &'a SentencePath>>,
    options: &LatticeOptions<'_>,
    typed_pinyin: &str,
) -> Vec<WordItem> {
    let mut block = Vec::new();
    if options.include_lattice_best {
        if let Some(row) = baseline_take_linear_sentence(
            candidates,
            &block,
            paths,
            CandidateSource::Generated,
            options,
            typed_pinyin,
        ) {
            block = Vec::with_capacity(2);
            block.push(row);
        }
    }
    if let Some(keyboard) = keyboard {
        if let Some(row) = baseline_take_linear_sentence(
            candidates,
            &block,
            keyboard,
            CandidateSource::NeuralKeyboard,
            options,
            typed_pinyin,
        ) {
            if block.is_empty() {
                block = Vec::with_capacity(2);
            }
            block.push(row);
        }
    }
    block
}

fn baseline_take_hashed_sentence<'a>(
    ranked: impl IntoIterator<Item = &'a SentencePath>,
    source: CandidateSource,
    options: &LatticeOptions<'_>,
    typed_pinyin: &str,
    already: &mut HashSet<&'a str>,
) -> Option<WordItem> {
    for path in ranked {
        if already.insert(path.sentence.as_str()) {
            return Some(baseline_sentence_row(typed_pinyin, path, source));
        }
        if !options.show_next_on_duplicate {
            return None;
        }
    }
    None
}

fn baseline_reranked_block<'a>(
    paths: &'a [SentencePath],
    keyboard: Option<impl IntoIterator<Item = &'a SentencePath>>,
    options: &LatticeOptions<'_>,
    typed_pinyin: &str,
    already: &mut HashSet<&'a str>,
) -> Vec<WordItem> {
    let lattice = if options.include_lattice_best {
        baseline_take_hashed_sentence(
            paths,
            CandidateSource::Generated,
            options,
            typed_pinyin,
            already,
        )
    } else {
        None
    };
    let pick = keyboard.and_then(|keyboard| {
        baseline_take_hashed_sentence(
            keyboard,
            CandidateSource::NeuralKeyboard,
            options,
            typed_pinyin,
            already,
        )
    });
    let mut block = if lattice.is_some() || pick.is_some() {
        Vec::with_capacity(2)
    } else {
        Vec::new()
    };
    if let Some(row) = lattice {
        block.push(row);
    }
    if let Some(row) = pick {
        block.push(row);
    }
    block
}
