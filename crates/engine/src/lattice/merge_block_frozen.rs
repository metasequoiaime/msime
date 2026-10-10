// 冻结 develop 4666d90faf5f13e40cde0a6152595b321d4bc785 的合并入口与输出块；仅改名称、可见性并移除原注释。
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn frozen_merge_lattice_candidates(
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
            let mut block = Vec::with_capacity(paths.len());
            block.extend(
                paths
                    .iter()
                    .filter(|path| already.insert(path.sentence.as_str()))
                    .map(|path| {
                        frozen_sentence_row(typed_pinyin, path, CandidateSource::Generated)
                    }),
            );
            block
        } else {
            frozen_generated_block_linear(candidates, &paths, typed_pinyin)
        }
    } else {
        let mut keyboard = None;
        for reranker in rerankers
            .iter_mut()
            .filter(|reranker| reranker.source == CandidateSource::NeuralKeyboard)
        {
            let mut reranked = paths.clone();
            if reranker.rerank(&mut reranked, rescoring_context) {
                keyboard = Some(reranked);
            }
        }
        if let Some(already) = already.as_mut() {
            frozen_reranked_block(&paths, keyboard.as_deref(), options, typed_pinyin, already)
        } else {
            frozen_reranked_block_linear(
                candidates,
                &paths,
                keyboard.as_deref(),
                options,
                typed_pinyin,
            )
        }
    };
    if !block.is_empty() {
        let at = whole_sentence_insert_position(candidates, syllables);
        candidates.splice(at..at, block);
    }
    typo
}

pub(super) fn frozen_sentence_row(
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

pub(super) fn frozen_sentence_seen_linear(
    candidates: &[WordItem],
    block: &[WordItem],
    sentence: &str,
) -> bool {
    candidates
        .iter()
        .chain(block)
        .any(|item| item.word == sentence)
}

pub(super) fn frozen_generated_block_linear(
    candidates: &[WordItem],
    paths: &[SentencePath],
    typed_pinyin: &str,
) -> Vec<WordItem> {
    let mut block = Vec::with_capacity(paths.len());
    for path in paths {
        if frozen_sentence_seen_linear(candidates, &block, &path.sentence) {
            continue;
        }
        block.push(frozen_sentence_row(
            typed_pinyin,
            path,
            CandidateSource::Generated,
        ));
    }
    block
}

pub(super) fn frozen_take_linear_sentence(
    candidates: &[WordItem],
    block: &[WordItem],
    ranked: &[SentencePath],
    source: CandidateSource,
    options: &LatticeOptions<'_>,
    typed_pinyin: &str,
) -> Option<WordItem> {
    for path in ranked {
        if !frozen_sentence_seen_linear(candidates, block, &path.sentence) {
            return Some(frozen_sentence_row(typed_pinyin, path, source));
        }
        if !options.show_next_on_duplicate {
            return None;
        }
    }
    None
}

pub(super) fn frozen_reranked_block_linear(
    candidates: &[WordItem],
    paths: &[SentencePath],
    keyboard: Option<&[SentencePath]>,
    options: &LatticeOptions<'_>,
    typed_pinyin: &str,
) -> Vec<WordItem> {
    let mut block = Vec::with_capacity(2);
    if options.include_lattice_best {
        if let Some(row) = frozen_take_linear_sentence(
            candidates,
            &block,
            paths,
            CandidateSource::Generated,
            options,
            typed_pinyin,
        ) {
            block.push(row);
        }
    }
    if let Some(keyboard) = keyboard {
        if let Some(row) = frozen_take_linear_sentence(
            candidates,
            &block,
            keyboard,
            CandidateSource::NeuralKeyboard,
            options,
            typed_pinyin,
        ) {
            block.push(row);
        }
    }
    block
}

pub(super) fn frozen_reranked_block<'a>(
    paths: &'a [SentencePath],
    keyboard: Option<&'a [SentencePath]>,
    options: &LatticeOptions<'_>,
    typed_pinyin: &str,
    already: &mut HashSet<&'a str>,
) -> Vec<WordItem> {
    let first_distinct =
        |ranked: &'a [SentencePath], already: &HashSet<&'a str>| -> Option<usize> {
            for (index, path) in ranked.iter().enumerate() {
                if !already.contains(path.sentence.as_str()) {
                    return Some(index);
                }
                if !options.show_next_on_duplicate {
                    return None;
                }
            }
            None
        };
    let take =
        |ranked: &'a [SentencePath], source: CandidateSource, already: &mut HashSet<&'a str>| {
            let path = &ranked[first_distinct(ranked, already)?];
            already.insert(path.sentence.as_str());
            Some(frozen_sentence_row(typed_pinyin, path, source))
        };

    let lattice = if options.include_lattice_best {
        take(paths, CandidateSource::Generated, already)
    } else {
        None
    };
    let pick =
        keyboard.and_then(|keyboard| take(keyboard, CandidateSource::NeuralKeyboard, already));
    let mut block = Vec::with_capacity(2);
    if let Some(row) = lattice {
        block.push(row);
    }
    if let Some(row) = pick {
        block.push(row);
    }
    block
}
