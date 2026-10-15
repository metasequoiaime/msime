use super::*;

// 固定 2446934521 的旧假设布局与解码正文，只删除注释；后续优化不得同步修改。

#[derive(Clone, Copy)]
struct Hyp<'g> {
    score: f64,
    prev: Option<(usize, usize)>,
    edge: Option<&'g Edge>,
    typo_edges: usize,
}

fn scores_higher(a: &Hyp<'_>, b: &Hyp<'_>) -> bool {
    a.score > b.score
}

fn keep_beam(column: &mut Vec<Hyp<'_>>, beam: usize) {
    if column.len() <= beam {
        return;
    }
    cxx_sort::partial_sort(column, beam, &mut scores_higher);
    column.truncate(beam);
}

fn column_capacities(
    graph: &Graph,
    extra: Option<&Graph>,
    beam: usize,
    nbest: usize,
) -> Vec<usize> {
    let floor = beam.max(nbest);
    let mut incoming_edges = vec![0usize; graph.len() + 1];
    for edges in graph {
        for edge in edges {
            if edge.end <= graph.len() {
                incoming_edges[edge.end] = incoming_edges[edge.end].saturating_add(1);
            }
        }
    }
    if let Some(extra) = extra {
        for edges in extra {
            for edge in edges {
                if edge.end <= graph.len() {
                    incoming_edges[edge.end] = incoming_edges[edge.end].saturating_add(1);
                }
            }
        }
    }
    incoming_edges
        .into_iter()
        .map(|count| floor.max(count.saturating_mul(beam)))
        .collect()
}

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
    let capacities = column_capacities(graph, extra, options.beam, options.nbest);
    let mut columns: Vec<Vec<Hyp<'_>>> = capacities.into_iter().map(Vec::with_capacity).collect();
    columns[0].push(Hyp {
        score: 0.0,
        prev: None,
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
                                .map(|edge| memo.score(table, previous, &edge.word)),
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
                    Some((column, at)) if column > 0 => {
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
                    prev: Some((pos, index)),
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

    let mut paths = Vec::with_capacity(last.len());
    for hyp in &last {
        let mut words = Vec::with_capacity(n);
        let mut keys = Vec::with_capacity(n);
        let mut cursor = Some(hyp);
        while let Some(step) = cursor {
            let Some(edge) = step.edge else { break };
            words.push(edge.word.clone());
            keys.push(edge.key.as_str());
            cursor = step.prev.map(|(column, at)| &columns[column][at]);
        }
        words.reverse();
        keys.reverse();
        let sentence = words.concat();
        if sentence.is_empty() {
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
    retain_unique_sentences(&mut paths, take);
    rescore_with_trigram(&mut paths, options);
    paths
}
