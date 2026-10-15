// 未接入生产的 entry 候选；最终入口收益不足，详见拒绝笔记。
use super::*;

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
    for pos in 0..n {
        keep_beam(&mut columns[pos], options.beam);
        let (done, ahead) = columns.split_at_mut(pos + 1);
        let current = &done[pos];
        // 行存储跨列复用；前词索引和有效行数只属于当前列，评分必须重新填满。
        let mut used_bigram_rows = 0;
        bigram_row_indices.clear();
        // 只读原始切片，评分与展开都按主图、附加图的相同顺序重新遍历。
        let literal = &graph[pos];
        let extra_edges = extra.map_or(&[][..], |extra| extra[pos].as_slice());
        let outgoing_len = literal.len() + extra_edges.len();
        let outgoing = literal.iter().chain(extra_edges.iter());
        // 没有评分表或存活假设时保持零容量；取得 entry 前按列预留一次。
        if bigram_table.is_some() && !current.is_empty() && bigram_rows.is_empty() {
            bigram_rows.reserve_exact(row_capacity);
            bigram_row_indices.reserve(row_capacity);
        }
        for (index, hyp) in current.iter().enumerate() {
            // 起点用句首标记，其余列用假设到达时的词。
            let previous = hyp.edge.map_or(SENTENCE_START, |edge| edge.word.as_str());
            let bonuses = bigram_table.map(|table| {
                let row = *bigram_row_indices.entry(previous).or_insert_with(|| {
                    let row = used_bigram_rows;
                    if row == bigram_rows.len() {
                        bigram_rows.push(Vec::new());
                    }
                    let scores = &mut bigram_rows[row];
                    scores.clear();
                    scores.reserve_exact(outgoing_len);
                    match memo.as_deref_mut() {
                        Some(memo) => scores.extend(
                            outgoing
                                .clone()
                                .map(|edge| memo.score(table, previous, &edge.word)),
                        ),
                        None => scores.extend(
                            outgoing
                                .clone()
                                .map(|edge| table.bigram(previous, &edge.word)),
                        ),
                    }
                    used_bigram_rows += 1;
                    row
                });
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
            for (at, edge) in outgoing.clone().enumerate() {
                let mut score = hyp.score + edge.log_prob;
                let static_bonus = bonuses.map_or(0.0, |bonuses| f64::from(bonuses[at]));
                if bigram.is_some() {
                    // 只奖励静态表偏好的搭配；负分和缺失项同样不惩罚，避免稀有缺失搭配反超常用词（#7583）。
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

    // 末列不会被回溯当作前驱，可以先剪枝再排序。
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
