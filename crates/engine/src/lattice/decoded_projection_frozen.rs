// 冻结 8f777281cf 的整句合并及其全部输出助手，仅移除注释；解码与未变更的插入位置算法共用生产入口。
use super::*;

pub(crate) fn merge_decoded(
    candidates: &mut Vec<WordItem>,
    syllables: &[String],
    typed_pinyin: &str,
    options: &LatticeOptions<'_>,
    rerankers: &mut [NeuralReranker],
    rescoring_context: &str,
    decoded: DecodedSentences,
) -> Option<TypoSentence> {
    let DecodedSentences { mut paths, typo } = decoded;
    if paths.is_empty() {
        return None;
    }

    if rerankers.is_empty() && options.emit > 0 {
        paths.truncate(options.emit);
    }
    let block = if rerankers.is_empty() {
        let already = existing_sentence_set(candidates, paths.len());
        if let Some(already) = already.as_ref() {
            let mut block = Vec::new();
            let capacity = paths.len();
            for path in paths {
                if already.contains(path.sentence.as_str()) {
                    continue;
                }
                if !block.is_empty()
                    && options.include_collocation_best
                    && !options.include_lattice_best
                {
                    break;
                }
                if block.is_empty() {
                    block = Vec::with_capacity(capacity);
                }
                block.push(owned_sentence_row(
                    typed_pinyin,
                    path,
                    lattice_row_source(&block, options),
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
        if !options.include_lattice_best && !options.include_collocation_best && keyboard.is_none()
        {
            return typo;
        }
        let already = existing_sentence_set(candidates, paths.len());
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

fn existing_sentence_set(candidates: &[WordItem], path_count: usize) -> Option<HashSet<&str>> {
    let dedup_size = candidates.len().saturating_add(path_count);
    (path_count > 1 && dedup_size > SMALL_SENTENCE_MERGE).then(|| {
        let mut already = HashSet::with_capacity(dedup_size);
        already.extend(candidates.iter().map(|item| item.word.as_str()));
        already
    })
}

fn owned_sentence_row(typed_pinyin: &str, path: SentencePath, source: CandidateSource) -> WordItem {
    let mut item = WordItem::new(
        typed_pinyin,
        path.sentence,
        (path.log_prob * 1000.0) as i64,
        source,
        path.key,
    );
    item.sentence_association = true;
    item.sentence_words = path.words;
    item
}

fn sentence_seen_linear(candidates: &[WordItem], block: &[WordItem], sentence: &str) -> bool {
    candidates
        .iter()
        .chain(block)
        .any(|item| item.word == sentence)
}

fn lattice_row_source(block: &[WordItem], options: &LatticeOptions<'_>) -> CandidateSource {
    if block.is_empty() && options.include_collocation_best {
        CandidateSource::Collocation
    } else {
        CandidateSource::Generated
    }
}

fn generated_block_linear(
    candidates: &[WordItem],
    paths: Vec<SentencePath>,
    typed_pinyin: &str,
    options: &LatticeOptions<'_>,
) -> Vec<WordItem> {
    let mut block = Vec::new();
    let capacity = paths.len();
    for path in paths {
        if sentence_seen_linear(candidates, &block, &path.sentence) {
            continue;
        }
        if !block.is_empty() && options.include_collocation_best && !options.include_lattice_best {
            break;
        }
        if block.is_empty() {
            block = Vec::with_capacity(capacity);
        }
        block.push(owned_sentence_row(
            typed_pinyin,
            path,
            lattice_row_source(&block, options),
        ));
    }
    block
}

fn take_sentence_index(
    paths: &[SentencePath],
    ranked: impl IntoIterator<Item = usize>,
    previous: Option<usize>,
    options: &LatticeOptions<'_>,
    is_listed: &impl Fn(&str) -> bool,
) -> Option<usize> {
    for index in ranked {
        let sentence = &paths[index].sentence;
        let seen = is_listed(sentence)
            || previous.is_some_and(|selected| paths[selected].sentence == *sentence);
        if !seen {
            return Some(index);
        }
        if !options.show_next_on_duplicate {
            return None;
        }
    }
    None
}

fn reranked_block(
    paths: Vec<SentencePath>,
    lattice: impl IntoIterator<Item = usize>,
    keyboard: Option<impl IntoIterator<Item = usize>>,
    options: &LatticeOptions<'_>,
    typed_pinyin: &str,
    is_listed: impl Fn(&str) -> bool,
) -> Vec<WordItem> {
    let literal = (options.include_lattice_best || options.include_collocation_best)
        .then(|| take_sentence_index(&paths, lattice, None, options, &is_listed))
        .flatten();
    let pick = keyboard
        .and_then(|keyboard| take_sentence_index(&paths, keyboard, literal, options, &is_listed));
    let mut rows = [None, None];
    for (index, path) in paths.into_iter().enumerate() {
        if literal == Some(index) {
            rows[0] = Some(owned_sentence_row(
                typed_pinyin,
                path,
                if options.include_collocation_best {
                    CandidateSource::Collocation
                } else {
                    CandidateSource::Generated
                },
            ));
        } else if pick == Some(index) {
            rows[1] = Some(owned_sentence_row(
                typed_pinyin,
                path,
                CandidateSource::NeuralKeyboard,
            ));
        }
    }
    let mut block = Vec::new();
    for row in rows.into_iter().flatten() {
        if block.is_empty() {
            block = Vec::with_capacity(2);
        }
        block.push(row);
    }
    block
}
