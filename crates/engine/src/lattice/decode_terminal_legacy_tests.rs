// 固定 833f43b2c1344b8ac8923187f3a2016c0b5cddb1 的完整解码，仅移除注释。
use super::*;

struct BeamColumn<'g> {
    hyps: Vec<Hyp<'g>>,
    streamed: bool,
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
    let capacities = column_capacities(graph, extra, options.beam);
    let stream_capacity = options.beam.saturating_add(1);
    let mut columns: Vec<BeamColumn<'_>> = capacities
        .into_iter()
        .enumerate()
        .map(|(position, capacity)| BeamColumn {
            hyps: Vec::with_capacity(if position < n && capacity > stream_capacity {
                stream_capacity
            } else {
                capacity
            }),
            streamed: position < n && capacity > stream_capacity,
        })
        .collect();
    columns[0].hyps.push(Hyp {
        score: 0.0,
        prev: (0, 0),
        edge: None,
        typo_edges: 0,
    });

    let row_capacity = options.beam.max(options.nbest);
    let mut bigram_rows: Vec<Vec<f32>> = Vec::new();
    let mut bigram_row_indices: HashMap<&str, usize> = HashMap::new();
    for pos in 0..n {
        if columns[pos].streamed {
            cxx_sort::finish_partial(&mut columns[pos].hyps, options.beam, &mut scores_higher);
        } else {
            keep_beam(&mut columns[pos].hyps, options.beam);
        }
        let (done, ahead) = columns.split_at_mut(pos + 1);
        let current = &done[pos].hyps;
        let mut used_bigram_rows = 0;
        bigram_row_indices.clear();
        let literal = &graph[pos];
        let extra_edges = extra.map_or(&[][..], |extra| extra[pos].as_slice());
        let outgoing_len = literal.len() + extra_edges.len();
        let outgoing = literal.iter().chain(extra_edges.iter());
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
                    bigram_row_indices.insert(previous, row);
                    row
                };
                &bigram_rows[row]
            });
            let context = personal.map(|personal| {
                let earlier = match hyp.prev {
                    (column, at) if column > 0 => {
                        done[column].hyps[at].edge.map(|edge| edge.word.as_str())
                    }
                    _ => None,
                };
                personal.context(earlier, Some(previous))
            });
            for (at, edge) in outgoing.clone().enumerate() {
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
                let next = Hyp {
                    score,
                    prev: (pos, index),
                    edge: Some(edge),
                    typo_edges: hyp.typo_edges + usize::from(edge.typo),
                };
                let target = &mut ahead[edge.end - pos - 1];
                if target.streamed {
                    cxx_sort::push_partial(
                        &mut target.hyps,
                        options.beam,
                        next,
                        &mut scores_higher,
                    );
                } else {
                    target.hyps.push(next);
                }
            }
        }
    }

    let mut last = std::mem::take(&mut columns[n].hyps);
    if last.is_empty() {
        return Vec::new();
    }
    keep_beam(&mut last, options.beam.max(options.nbest));
    cxx_sort::sort(&mut last, &mut scores_higher);
    let take = options.nbest.min(last.len());

    let inline_unique = last.len() <= SMALL_SENTENCE_PATHS;
    let mut paths: Vec<SentencePath> = Vec::with_capacity(last.len());
    let mut trace = Vec::new();
    for hyp in &last {
        if inline_unique && paths.len() == take {
            break;
        }
        trace.clear();
        trace.reserve_exact(n);
        let mut cursor = Some(hyp);
        while let Some(step) = cursor {
            let Some(edge) = step.edge else { break };
            trace.push(edge);
            let (column, at) = step.prev;
            cursor = Some(&columns[column].hyps[at]);
        }
        trace.reverse();
        let mut sentence = String::with_capacity(trace.iter().map(|edge| edge.word.len()).sum());
        for edge in &trace {
            sentence.push_str(&edge.word);
        }
        if sentence.is_empty()
            || (inline_unique && paths.iter().any(|path| path.sentence == sentence))
        {
            continue;
        }
        let mut words = Vec::with_capacity(n);
        let mut key = String::with_capacity(
            trace.iter().map(|edge| edge.key.len()).sum::<usize>() + trace.len().saturating_sub(1),
        );
        for (index, edge) in trace.iter().enumerate() {
            words.push(edge.word.clone());
            if index != 0 {
                key.push('\'');
            }
            key.push_str(&edge.key);
        }
        paths.push(SentencePath {
            sentence,
            key,
            log_prob: hyp.score,
            words,
            typo_edges: hyp.typo_edges,
        });
    }
    drop(trace);
    if !inline_unique {
        retain_unique_sentences(&mut paths, take);
    }
    rescore_with_trigram(&mut paths, options);
    paths
}
