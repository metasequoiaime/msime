use super::vector_row_reference::Row;
use super::*;

// 固定 961f0514a 的精确词条流式查询，继续补全仍返回视图向量，供单项分配对照。
pub(super) fn search_converted(
    dictionary: &JapaneseDictionary,
    conversion: &RomajiConversion,
    limit: usize,
) -> Vec<JapaneseConversion> {
    let mut output = Output {
        items: Vec::with_capacity(limit),
        limit,
    };
    let reading = conversion.hiragana.as_str();
    let pending = conversion.pending.as_str();
    let limit = output.limit;
    if limit == 0 {
        return output.items;
    }
    let pending_kana = kana_for_romaji_prefix_view(pending);
    if reading.is_empty() {
        for kana in pending_kana {
            for lemma in dictionary.prefix_lemma_views(kana, 24) {
                output.push(lemma.surface, i64::from(lemma.word_cost));
                if output.full() {
                    return output.items;
                }
            }
        }
        return output.items;
    }

    let boundaries: Vec<usize> = reading
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(reading.len()))
        .collect();
    let mora_count = boundaries.len() - 1;
    let mut rows: Vec<Row> = (0..=mora_count).map(|_| Row::new()).collect();
    rows[0].nodes.push(Node {
        text: String::new(),
        cost: 0,
        right_id: 0,
    });

    for start in 0..mora_count {
        if rows[start].nodes.is_empty() {
            continue;
        }
        let previous_row = std::mem::take(&mut rows[start]);
        let start_byte = boundaries[start];
        let max_end = mora_count.min(start + MAX_LEMMA_MORA);
        for end in start + 1..=max_end {
            let key = &reading[start_byte..boundaries[end]];
            dictionary.for_each_exact_lemma_view(key, 24, |lemma| {
                for previous in &previous_row.nodes {
                    let cost = previous.cost
                        + i64::from(lemma.word_cost)
                        + i64::from(dictionary.connection_cost(previous.right_id, lemma.left_id));
                    rows[end].extend(&previous.text, lemma.surface, cost, lemma.right_id);
                }
            });
        }

        let kana = &reading[start_byte..boundaries[start + 1]];
        for previous in &previous_row.nodes {
            rows[start + 1].extend(
                &previous.text,
                kana,
                previous.cost + i64::from(UNKNOWN_KANA_COST),
                0,
            );
        }

        rows[start] = previous_row;
    }

    let mut finals = std::mem::take(&mut rows[mora_count]).nodes;
    for node in &mut finals {
        node.cost += i64::from(dictionary.connection_cost(node.right_id, 0));
    }
    finals.sort_by_key(|node| node.cost);
    let mut finals = finals.into_iter();
    if let Some(best) = finals.next() {
        output.push_owned(best.text, best.cost);
    }

    if !pending.is_empty() {
        if let Some(first) = pending_kana.first() {
            let suffix_capacity = if pending_kana.len() == 1 {
                first.len()
            } else {
                pending_kana
                    .iter()
                    .map(|kana| kana.len())
                    .max()
                    .unwrap_or(0)
            };
            let mut key = String::with_capacity(reading.len() + suffix_capacity);
            key.push_str(reading);
            for kana in pending_kana {
                // 只替换假名后缀，容量覆盖最长后缀，避免循环中扩容。
                key.truncate(reading.len());
                key.push_str(kana);
                dictionary.for_each_exact_lemma_view(&key, 16, |lemma| {
                    output.push(lemma.surface, i64::from(lemma.word_cost));
                });
            }
        }
        for lemma in dictionary.continuing_lemma_views(reading, pending_kana, 48) {
            output.push(lemma.surface, i64::from(lemma.word_cost));
        }
    }

    for node in finals {
        output.push_owned(node.text, node.cost);
    }

    for end in (1..=mora_count).rev() {
        if output.full() {
            break;
        }
        dictionary.for_each_exact_lemma_view(&reading[..boundaries[end]], 16, |lemma| {
            output.push(lemma.surface, i64::from(lemma.word_cost));
        });
    }
    output.items
}
