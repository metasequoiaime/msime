use super::*;

// 前缀复用候选仅供实验；生产解码沿用固定基线的完整词对散列。
pub(super) fn decode_graph(
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

    // 同一列里很多假设经由同一个词到达，只是更早的历史不同；它们对每条出边查到的二元分完全一样。按前一个词把这一列所有出边的二元分记下来，同一个词只查一遍表。
    let row_capacity = options.beam.max(options.nbest);
    let mut bigram_rows: Vec<Vec<f32>> = Vec::new();
    let mut bigram_row_indices: HashMap<&str, usize> = HashMap::new();
    // 这一列的出边，按原来的顺序（先图里的，再 `extra` 的）收集一次，每个假设都按这个顺序展开。
    let mut outgoing: Vec<&Edge> = Vec::new();
    for pos in 0..n {
        keep_beam(&mut columns[pos], options.beam);
        let (done, ahead) = columns.split_at_mut(pos + 1);
        let current = &done[pos];
        // 行存储跨列复用；前词索引和有效行数只属于当前列，评分必须重新填满。
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
                    let prefix = table.bigram_prefix(previous);
                    match memo.as_deref_mut() {
                        Some(memo) => scores.extend(outgoing.iter().map(|edge| {
                            score_key(
                                memo,
                                table,
                                prefix.and_then(|prefix| prefix.key(&edge.word)),
                            )
                        })),
                        None => scores.extend(outgoing.iter().map(|edge| {
                            prefix
                                .and_then(|prefix| prefix.key(&edge.word))
                                .map_or(0.0, |key| table.score(key))
                        })),
                    }
                    used_bigram_rows += 1;
                    bigram_row_indices.insert(previous, row);
                    row
                };
                &bigram_rows[row]
            });
            // 前驱列已经剪枝且不会再改变；前驱列为零时没有更早的词，起点也填零。
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

    // 小末列沿用线性去重；收齐唯一预算后不再物化后续假设，大末列保留原哈希回退。
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

pub(super) fn score_key(memo: &mut BigramMemo, table: &Arc<NgramTable>, key: Option<u64>) -> f32 {
    let Some(key) = key else {
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
