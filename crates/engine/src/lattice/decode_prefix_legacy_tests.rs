use super::*;

// 固定 59f20f26b5 的完整解码正文；假设与排序等未变的 helper 共用，旧键构造独立保留。
pub(in super::super) fn decode_graph(
    graph: &Graph,
    options: &LatticeOptions<'_>,
    extra: Option<&Graph>,
) -> Vec<SentencePath> {
    let n = graph.len();
    let personal = active_personal(options);
    let bigram_table = options.bigram.as_ref();
    let bigram = options.bigram.as_deref();
    let mut memo = options.bigram_memo.map(RefCell::borrow_mut);
    let capacities = column_capacities(graph, extra, options.beam);
    let mut columns: Vec<Vec<Hyp<'_>>> = capacities.into_iter().map(Vec::with_capacity).collect();
    columns[0].push(Hyp {
        score: 0.0,
        prev: (0, 0),
        edge: None,
        typo_edges: 0,
    });

    let row_capacity = options.beam.max(options.nbest);
    let mut bigram_rows: Vec<Vec<f32>> = Vec::new();
    let mut bigram_row_indices: HashMap<&str, usize> = HashMap::new();
    let mut outgoing: Vec<&Edge> = Vec::new();
    for pos in 0..n {
        keep_beam(&mut columns[pos], options.beam);
        let (done, ahead) = columns.split_at_mut(pos + 1);
        let current = &done[pos];
        let mut used_bigram_rows = 0;
        bigram_row_indices.clear();
        outgoing.clear();
        outgoing.extend(
            graph[pos]
                .iter()
                .chain(extra.into_iter().flat_map(|extra| extra[pos].iter())),
        );
        for (index, hyp) in current.iter().enumerate() {
            let previous = hyp.edge.map_or(SENTENCE_START, |edge| edge.word.as_str());
            let bonuses = bigram_table.map(|table| {
                let row = if let Some(&row) = bigram_row_indices.get(previous) {
                    row
                } else {
                    let row = used_bigram_rows;
                    if row == bigram_rows.len() {
                        if bigram_rows.is_empty() {
                            bigram_rows.reserve_exact(row_capacity);
                            bigram_row_indices.reserve(row_capacity);
                        }
                        bigram_rows.push(Vec::new());
                    }
                    let scores = &mut bigram_rows[row];
                    scores.clear();
                    scores.reserve_exact(outgoing.len());
                    match memo.as_deref_mut() {
                        Some(memo) => scores.extend(
                            outgoing
                                .iter()
                                .map(|edge| score(memo, table, previous, &edge.word)),
                        ),
                        None => scores.extend(
                            outgoing
                                .iter()
                                .map(|edge| table.bigram(previous, &edge.word)),
                        ),
                    }
                    used_bigram_rows += 1;
                    bigram_row_indices.insert(previous, row);
                    row
                };
                &bigram_rows[row]
            });
            let context = personal.map(|personal| {
                let earlier = match hyp.prev {
                    (column, at) if column > 0 => {
                        done[column][at].edge.map(|edge| edge.word.as_str())
                    }
                    _ => None,
                };
                personal.context(earlier, Some(previous))
            });
            for (at, &edge) in outgoing.iter().enumerate() {
                let mut score = hyp.score + edge.log_prob;
                let static_bonus = bonuses.map_or(0.0, |bonuses| f64::from(bonuses[at]));
                if bigram.is_some() {
                    score += options.bigram_weight * static_bonus.max(0.0);
                }
                if let (Some(personal), Some(context)) = (personal, context.as_ref()) {
                    if context.confidence > 0.0 {
                        score += personal_transition(
                            personal,
                            context,
                            &edge.word,
                            static_bonus,
                            options,
                        );
                    }
                }
                ahead[edge.end - pos - 1].push(Hyp {
                    score,
                    prev: (pos, index),
                    edge: Some(edge),
                    typo_edges: hyp.typo_edges + usize::from(edge.typo),
                });
            }
        }
    }

    let mut last = std::mem::take(&mut columns[n]);
    if last.is_empty() {
        return Vec::new();
    }
    keep_beam(&mut last, options.beam.max(options.nbest));
    cxx_sort::sort(&mut last, &mut scores_higher);
    let take = options.nbest.min(last.len());

    let inline_unique = last.len() <= SMALL_SENTENCE_PATHS;
    let mut paths: Vec<SentencePath> = Vec::with_capacity(last.len());
    for hyp in &last {
        if inline_unique && paths.len() == take {
            break;
        }
        let mut words = Vec::with_capacity(n);
        let mut keys = Vec::with_capacity(n);
        let mut cursor = Some(hyp);
        while let Some(step) = cursor {
            let Some(edge) = step.edge else { break };
            words.push(edge.word.clone());
            keys.push(edge.key.as_str());
            let (column, at) = step.prev;
            cursor = Some(&columns[column][at]);
        }
        words.reverse();
        keys.reverse();
        let sentence = words.concat();
        if sentence.is_empty()
            || (inline_unique && paths.iter().any(|path| path.sentence == sentence))
        {
            continue;
        }
        paths.push(SentencePath {
            sentence,
            key: keys.join("'"),
            log_prob: hyp.score,
            words,
            typo_edges: hyp.typo_edges,
        });
    }
    if !inline_unique {
        retain_unique_sentences(&mut paths, take);
    }
    rescore_with_trigram(&mut paths, options);
    paths
}

pub(super) fn score(
    memo: &mut BigramMemo,
    table: &Arc<NgramTable>,
    previous: &str,
    next: &str,
) -> f32 {
    let Some(key) = table.bigram_key(previous, next) else {
        return 0.0;
    };
    let address = Arc::as_ptr(table) as usize;
    if memo.table != address {
        memo.scores.clear();
        memo.table = address;
    }
    if let Some(&score) = memo.scores.get(&key) {
        return score;
    }
    if memo.scores.len() >= BIGRAM_MEMO_CAPACITY {
        memo.scores.clear();
    }
    let score = table.score(key);
    memo.scores.insert(key, score);
    score
}
