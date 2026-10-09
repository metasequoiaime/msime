use super::super::decoder::test_model;
use super::super::romaji::convert_romaji;
use super::*;

// 固定基线 dbfdd576e 的批量构造、稳定排序及截断流程，独立对照提前筛选的行为。
fn reference_search(
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
    let pending_kana = kana_for_romaji_prefix(pending);
    if reading.is_empty() {
        for kana in &pending_kana {
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
    let mut rows: Vec<Vec<Node>> = (0..=mora_count)
        .map(|_| Vec::with_capacity(MAX_NODES_PER_ROW))
        .collect();
    rows[0].push(Node {
        text: String::new(),
        cost: 0,
        right_id: 0,
    });

    for start in 0..mora_count {
        if rows[start].is_empty() {
            continue;
        }
        let previous_row = std::mem::take(&mut rows[start]);
        let start_byte = boundaries[start];
        let max_end = mora_count.min(start + MAX_LEMMA_MORA);
        for end in start + 1..=max_end {
            let key = &reading[start_byte..boundaries[end]];
            for lemma in dictionary.exact_lemma_views(key, 24) {
                for previous in &previous_row {
                    let cost = previous.cost
                        + i64::from(lemma.word_cost)
                        + i64::from(dictionary.connection_cost(previous.right_id, lemma.left_id));
                    rows[end].push(Node {
                        text: join_text(&previous.text, lemma.surface),
                        cost,
                        right_id: lemma.right_id,
                    });
                }
            }
        }

        let kana = &reading[start_byte..boundaries[start + 1]];
        for previous in &previous_row {
            rows[start + 1].push(Node {
                text: join_text(&previous.text, kana),
                cost: previous.cost + i64::from(UNKNOWN_KANA_COST),
                right_id: 0,
            });
        }

        for row in &mut rows[start + 1..=max_end] {
            reference_keep_best(row);
        }
        rows[start] = previous_row;
    }

    let mut finals = std::mem::take(&mut rows[mora_count]);
    for node in &mut finals {
        node.cost += i64::from(dictionary.connection_cost(node.right_id, 0));
    }
    finals.sort_by_key(|node| node.cost);
    if let Some(best) = finals.first() {
        output.push(&best.text, best.cost);
    }

    if !pending.is_empty() {
        for kana in &pending_kana {
            for lemma in dictionary.exact_lemma_views(&join_text(reading, kana), 16) {
                output.push(lemma.surface, i64::from(lemma.word_cost));
            }
        }
        for lemma in dictionary.continuing_lemma_views(reading, &pending_kana, 48) {
            output.push(lemma.surface, i64::from(lemma.word_cost));
        }
    }

    for node in finals.iter().skip(1) {
        output.push(&node.text, node.cost);
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

fn reference_keep_best(row: &mut Vec<Node>) {
    if row.len() > MAX_NODES_PER_ROW {
        row.sort_by_key(|node| node.cost);
        row.truncate(MAX_NODES_PER_ROW);
    }
}

#[test]
fn dense_matrix_search_matches_batch_pruning_reference() {
    for seed in 0..32 {
        let readings = ["か", "かか", "かかか", "かき", "かし", "き", "きか", "し"];
        let surfaces: Vec<_> = (0..readings.len() * 27)
            .map(|index| format!("語{}", if seed % 3 == 0 { index % 5 } else { index }))
            .collect();
        let entries: Vec<_> = readings
            .iter()
            .enumerate()
            .flat_map(|(reading_index, reading)| {
                let surfaces = &surfaces;
                (0..1 + seed % 27).map(move |index| {
                    (
                        *reading,
                        surfaces[reading_index * 27 + index].as_str(),
                        ((index + seed) % 3) as u16,
                        ((index + reading_index) % 3) as u16,
                        ((index + reading_index + seed) % 5) as i32 * 20 - 40,
                    )
                })
            })
            .collect();
        let dictionary = JapaneseDictionary::from_bytes(
            test_model::bytes(&entries, 3, &[-15, 0, 40, 8, -5, 2, 90, 11, -30]).into_boxed_slice(),
        )
        .expect("合成词库加载");
        for input in [
            "",
            "ka",
            "kaka",
            "kakaka",
            "kak",
            "kik",
            "kaki",
            "kakashik",
            "ki",
            "q",
            "kiki",
            "kakashishi",
            "kakakakakakakakakakakakakakakakakaka",
        ] {
            let conversion = convert_romaji(input);
            for limit in [0, 1, 2, 8, 12, 16, 32] {
                assert_eq!(
                    search_converted(&dictionary, &conversion, limit),
                    reference_search(&dictionary, &conversion, limit),
                    "合成种子 {seed}，读音长度 {}，限额 {limit}",
                    input.len()
                );
            }
        }
    }
}

fn node_fields(nodes: &[Node]) -> Vec<(&str, i64, u16)> {
    nodes
        .iter()
        .map(|node| (node.text.as_str(), node.cost, node.right_id))
        .collect()
}

#[test]
fn a_row_keeps_insertion_order_until_the_ninth_node_even_when_rejected() {
    let mut row = Row::new();
    let costs = [9, 1, 5, 2, 5, -1, 4, 8];
    for (index, cost) in costs.into_iter().enumerate() {
        row.extend("合成", &index.to_string(), cost, index as u16);
    }
    assert_eq!(
        row.nodes.iter().map(|node| node.cost).collect::<Vec<_>>(),
        costs
    );
    let capacity = row.nodes.capacity();
    let pointer = row.nodes.as_ptr();
    row.extend("败选", "文本", 100, 9);
    assert_eq!(
        row.nodes
            .iter()
            .map(|node| node.right_id)
            .collect::<Vec<_>>(),
        [5, 1, 3, 6, 2, 4, 7, 0]
    );
    assert_eq!(row.nodes.as_ptr(), pointer);
    assert_eq!(row.nodes.capacity(), capacity);
    let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
        row.extend("同分", "败选", 9, 10);
        row.extend("高分", "败选", 100, 11);
    });
    assert_eq!(allocations, 0);
    row.extend("改善", "同分", 5, 12);
    assert_eq!(
        row.nodes
            .iter()
            .map(|node| node.right_id)
            .collect::<Vec<_>>(),
        [5, 1, 3, 6, 2, 4, 12, 7]
    );
}

#[test]
fn rows_match_stable_batch_sort_across_random_batches_without_growing() {
    for seed in 0..32_u64 {
        let mut state = seed + 1;
        let mut row = Row::new();
        let mut reference = Vec::new();
        let pointer = row.nodes.as_ptr();
        let capacity = row.nodes.capacity();
        let mut serial = 0;
        for batch in 0..24 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let count = if batch == 0 {
                seed as usize % 10
            } else {
                (state >> 32) as usize % 193
            };
            for _ in 0..count {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                let cost = ((state >> 32) % 61) as i64 - 30;
                let suffix = serial.to_string();
                let right_id = (serial % 3) as u16;
                row.extend("合成", &suffix, cost, right_id);
                reference.push(Node {
                    text: join_text("合成", &suffix),
                    cost,
                    right_id,
                });
                serial += 1;
            }
            reference_keep_best(&mut reference);
            assert_eq!(
                node_fields(&row.nodes),
                node_fields(&reference),
                "种子 {seed} 批次 {batch}"
            );
            assert!(row.nodes.len() <= MAX_NODES_PER_ROW);
            assert_eq!(row.nodes.as_ptr(), pointer);
            assert_eq!(row.nodes.capacity(), capacity);
        }
    }
}

#[test]
fn dense_two_mora_search_avoids_losing_cartesian_product_texts() {
    let singles: Vec<_> = (0..24).map(|index| format!("単{index:02}")).collect();
    let doubles: Vec<_> = (0..24).map(|index| format!("双{index:02}")).collect();
    let entries: Vec<_> = singles
        .iter()
        .map(|surface| ("か", surface.as_str(), 0, 0, 0))
        .chain(
            doubles
                .iter()
                .map(|surface| ("かか", surface.as_str(), 0, 0, 0)),
        )
        .collect();
    let dictionary =
        JapaneseDictionary::from_bytes(test_model::bytes(&entries, 1, &[0]).into_boxed_slice())
            .expect("合成词库加载");
    let conversion = convert_romaji("kaka");
    let (reference, baseline_allocations) = crate::ime::personal_rerank::allocations::count(|| {
        reference_search(&dictionary, &conversion, 16)
    });
    let (actual, allocations) = crate::ime::personal_rerank::allocations::count(|| {
        search_converted(&dictionary, &conversion, 16)
    });
    let expected: Vec<_> = doubles[..16]
        .iter()
        .map(|text| JapaneseConversion {
            text: text.clone(),
            cost: 0,
        })
        .collect();
    assert_eq!(reference, expected);
    assert_eq!(actual, expected);
    eprintln!("日文密集双假名矩阵分配：{baseline_allocations}→{allocations}");
    assert!(baseline_allocations >= 200, "原笛卡尔积物化成本");
    assert!(
        allocations <= 38,
        "胜选移入输出，同分败选无需物化：{allocations}"
    );
}

#[test]
fn duplicate_row_texts_still_occupy_ranking_slots() {
    let mut row = Row::new();
    for index in 0..MAX_NODES_PER_ROW {
        row.extend("重复", "词面", 0, index as u16);
    }
    row.extend("后到", "其他词面", 0, 9);
    assert_eq!(row.nodes.len(), MAX_NODES_PER_ROW);
    assert!(row.nodes.iter().all(|node| node.text == "重复词面"));
    assert_eq!(
        row.nodes
            .iter()
            .map(|node| node.right_id)
            .collect::<Vec<_>>(),
        (0..8).collect::<Vec<_>>()
    );
}
