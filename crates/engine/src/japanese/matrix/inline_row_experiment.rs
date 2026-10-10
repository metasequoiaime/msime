// 固定八槽行的否决实验：保留可复现的行为和 release 对照，不进入生产查询。
use super::*;

/// 行内固定八个节点槽，只有 `used` 之前的节点参与排名与输出。
pub(super) struct Row {
    pub(super) nodes: [Node; MAX_NODES_PER_ROW],
    pub(super) used: usize,
    ranked: bool,
}

impl Row {
    pub(super) fn new() -> Self {
        Self {
            nodes: std::array::from_fn(|_| Node {
                text: String::new(),
                cost: 0,
                right_id: 0,
            }),
            used: 0,
            ranked: false,
        }
    }

    pub(super) fn nodes(&self) -> &[Node] {
        &self.nodes[..self.used]
    }

    fn nodes_mut(&mut self) -> &mut [Node] {
        &mut self.nodes[..self.used]
    }

    pub(super) fn extend(&mut self, prefix: &str, suffix: &str, cost: i64, right_id: u16) {
        if self.used < MAX_NODES_PER_ROW {
            self.nodes[self.used] = Node {
                text: join_text(prefix, suffix),
                cost,
                right_id,
            };
            self.used += 1;
            return;
        }
        if !self.ranked {
            // 第九项即使败选，原批量截断也会把已有八项稳定排序。
            self.nodes_mut().sort_by_key(|node| node.cost);
            self.ranked = true;
        }
        if cost >= self.nodes[MAX_NODES_PER_ROW - 1].cost {
            return;
        }
        // 前七项保留；最差节点旋转到插入位置后被覆盖，同分既有项仍在新项之前。
        let index = self.nodes[..MAX_NODES_PER_ROW - 1].partition_point(|node| node.cost <= cost);
        self.nodes[index..].rotate_right(1);
        self.nodes[index] = Node {
            text: join_text(prefix, suffix),
            cost,
            right_id,
        };
    }
}

impl Default for Row {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) fn search_converted(
    dictionary: &JapaneseDictionary,
    conversion: &RomajiConversion,
    limit: usize,
) -> Vec<JapaneseConversion> {
    let reading = conversion.hiragana.as_str();
    let pending = conversion.pending.as_str();
    let mut output = Output {
        items: Vec::with_capacity(limit),
        limit,
    };
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
    // 默认槽是零成本空文本，首行只启用这一个起点节点。
    rows[0].used = 1;

    for start in 0..mora_count {
        if rows[start].nodes().is_empty() {
            continue;
        }
        // 前驱行与后续目标行互不重叠，直接借用，避免反复初始化和移动整行。
        let (previous_rows, next_rows) = rows.split_at_mut(start + 1);
        let previous_row = &previous_rows[start];
        let start_byte = boundaries[start];
        let max_end = mora_count.min(start + MAX_LEMMA_MORA);
        for end in start + 1..=max_end {
            let key = &reading[start_byte..boundaries[end]];
            for lemma in dictionary.exact_lemma_views(key, 24) {
                for previous in previous_row.nodes() {
                    let cost = previous.cost
                        + i64::from(lemma.word_cost)
                        + i64::from(dictionary.connection_cost(previous.right_id, lemma.left_id));
                    next_rows[end - start - 1].extend(
                        &previous.text,
                        lemma.surface,
                        cost,
                        lemma.right_id,
                    );
                }
            }
        }

        let kana = &reading[start_byte..boundaries[start + 1]];
        for previous in previous_row.nodes() {
            next_rows[0].extend(
                &previous.text,
                kana,
                previous.cost + i64::from(UNKNOWN_KANA_COST),
                0,
            );
        }
    }

    let mut finals = std::mem::take(&mut rows[mora_count]);
    for node in finals.nodes_mut() {
        node.cost += i64::from(dictionary.connection_cost(node.right_id, 0));
    }
    finals.nodes_mut().sort_by_key(|node| node.cost);
    let mut finals = finals.nodes.into_iter().take(finals.used);
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
                for lemma in dictionary.exact_lemma_views(&key, 16) {
                    output.push(lemma.surface, i64::from(lemma.word_cost));
                }
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
        for lemma in dictionary.exact_lemma_views(&reading[..boundaries[end]], 16) {
            output.push(lemma.surface, i64::from(lemma.word_cost));
            if output.full() {
                break;
            }
        }
    }
    output.items
}
