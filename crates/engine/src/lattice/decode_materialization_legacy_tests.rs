// 冻结 febb4c85c6 的解码、容量、去重与重排函数；仅调整入口可见性并移除注释。
use super::*;

struct BeamColumn<'g> {
    hyps: Vec<Hyp<'g>>,
    streamed: bool,
}

#[derive(Clone, Copy)]
struct Hyp<'g> {
    score: f64,
    prev: (usize, usize),
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

fn column_capacities(graph: &Graph, extra: Option<&Graph>, beam: usize) -> Vec<usize> {
    let mut capacities = vec![0usize; graph.len() + 1];
    capacities[0] = 1;
    for (start, edges) in graph.iter().enumerate() {
        let surviving = capacities[start].min(beam);
        for edge in edges
            .iter()
            .chain(extra.into_iter().flat_map(|extra| &extra[start]))
        {
            if edge.end <= graph.len() {
                capacities[edge.end] = capacities[edge.end].saturating_add(surviving);
            }
        }
    }
    capacities
}

fn active_personal<'a>(options: &LatticeOptions<'a>) -> Option<&'a PersonalNgram> {
    options
        .personal
        .filter(|personal| !personal.is_empty() && options.personal_weight != 0.0)
}

fn personal_transition(
    personal: &PersonalNgram,
    context: &super::personal::PersonalContext,
    word: &str,
    static_bonus: f64,
    options: &LatticeOptions<'_>,
) -> f64 {
    let mu = context.confidence;
    let reference = options.personal_reference_probability * static_bonus.exp();
    if mu <= 0.0 || reference <= 0.0 {
        return 0.0;
    }
    let lift = ((1.0 - mu) + mu * personal.probability(context, word) / reference).ln();
    options.personal_weight * options.personal_max_bonus.min(lift)
}

pub(crate) fn decode_graph(
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
            cursor = Some(&columns[column].hyps[at]);
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

const SMALL_SENTENCE_PATHS: usize = 64;

fn retain_unique_sentences(paths: &mut Vec<SentencePath>, take: usize) {
    if paths.len() <= SMALL_SENTENCE_PATHS {
        let mut write = 0;
        for read in 0..paths.len() {
            if paths[..write]
                .iter()
                .any(|path| path.sentence == paths[read].sentence)
            {
                continue;
            }
            if write != read {
                paths.swap(write, read);
            }
            write += 1;
        }
        paths.truncate(write.min(take));
        return;
    }
    let mut sentences = HashSet::with_capacity(paths.len());
    let duplicates = paths
        .iter()
        .enumerate()
        .filter_map(|(index, path)| (!sentences.insert(path.sentence.as_str())).then_some(index))
        .collect::<Vec<_>>();
    drop(sentences);
    let mut duplicates = duplicates.into_iter().peekable();
    let mut write = 0;
    for read in 0..paths.len() {
        if duplicates.peek() == Some(&read) {
            duplicates.next();
            continue;
        }
        if write != read {
            paths.swap(write, read);
        }
        write += 1;
    }
    paths.truncate(write.min(take));
}

fn rescore_with_trigram(paths: &mut [SentencePath], options: &LatticeOptions<'_>) {
    let Some(trigram) = options.trigram.as_deref() else {
        return;
    };
    if options.trigram_weight == 0.0 || paths.len() < 2 {
        return;
    }
    for path in paths.iter_mut() {
        let mut bonus = 0.0;
        for (index, word) in path.words.iter().enumerate() {
            let before = if index >= 2 {
                path.words[index - 2].as_str()
            } else {
                SENTENCE_START
            };
            let previous = if index >= 1 {
                path.words[index - 1].as_str()
            } else {
                SENTENCE_START
            };
            bonus += f64::from(trigram.trigram(before, previous, word));
        }
        path.log_prob += options.trigram_weight * bonus;
    }
    paths.sort_by(|a, b| b.log_prob.total_cmp(&a.log_prob));
}
