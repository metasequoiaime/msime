use super::super::decoder::test_model;
use super::super::romaji::convert_romaji;
use super::*;

// 固定 f37203c77 的完整矩阵查询正文；查询键差由冻结的后续 Vec 查询隔离。
fn reference_current_search(
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
            for lemma in dictionary.exact_lemma_views(key, 24) {
                for previous in &previous_row.nodes {
                    let cost = previous.cost
                        + i64::from(lemma.word_cost)
                        + i64::from(dictionary.connection_cost(previous.right_id, lemma.left_id));
                    rows[end].extend(&previous.text, lemma.surface, cost, lemma.right_id);
                }
            }
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
        for kana in pending_kana {
            for lemma in dictionary.exact_lemma_views(&join_text(reading, kana), 16) {
                output.push(lemma.surface, i64::from(lemma.word_cost));
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

#[test]
fn matrix_pending_key_output_and_allocation_match_current_baseline() {
    let dictionary = JapaneseDictionary::from_bytes(
        test_model::bytes(
            &[
                ("かか", "仮仮", 0, 0, -500),
                ("かき", "仮木", 0, 0, -500),
                ("かきゃ", "仮甲", 0, 0, 0),
                ("かくぁ", "仮乙", 0, 0, 15),
                ("かっ", "仮促", 0, 0, 20),
                ("甲😀かか", "仮長", 0, 0, -40),
            ],
            1,
            &[0],
        )
        .into_boxed_slice(),
    )
    .expect("合成词库");
    for reading in ["", "か", "甲😀か", &"か".repeat(24)] {
        for pending in ["", "k", "sh", "xt", "q", "K", "d", "z", "あ", "toolong"] {
            let conversion = RomajiConversion {
                hiragana: reading.to_owned(),
                pending: pending.to_owned(),
                complete: pending.is_empty(),
            };
            let _ = search_converted(&dictionary, &conversion, 12);
            for limit in [0, 1, 2, 8, 12, 32] {
                let (expected, old_allocations) =
                    crate::ime::personal_rerank::allocations::count(|| {
                        reference_current_search(&dictionary, &conversion, limit)
                    });
                let (actual, new_allocations) =
                    crate::ime::personal_rerank::allocations::count(|| {
                        search_converted(&dictionary, &conversion, limit)
                    });
                assert_eq!(
                    actual,
                    expected,
                    "读音长度 {}，待定 {pending}，限额 {limit}",
                    reading.len()
                );
                // 后续精确词条流式消费另省结果容器，键差仍对照固定的单缓冲 Vec 查询。
                let (_, buffered_allocations) =
                    crate::ime::personal_rerank::allocations::count(|| {
                        super::inline_row_tests::vector_reference_search(
                            &dictionary,
                            &conversion,
                            limit,
                        )
                    });
                let suffixes = kana_for_romaji_prefix_view(pending);
                let saved = if reading.is_empty() || limit == 0 {
                    0
                } else {
                    suffixes.len().saturating_sub(1)
                };
                assert_eq!(
                    buffered_allocations + saved,
                    old_allocations,
                    "键分配差值：{pending}"
                );
                assert!(new_allocations <= buffered_allocations);
            }
        }
    }
}

#[test]
#[ignore = "本地 release 与固定历史矩阵对照；包含精确词条流式消费，不设置 CI 时间阈值"]
fn benchmark_matrix_pending_reading_keys() {
    use std::hint::black_box;
    use std::time::Instant;
    let dictionary = JapaneseDictionary::from_bytes(
        test_model::bytes(
            &[
                ("かか", "仮仮", 0, 0, 500),
                ("かき", "仮木", 0, 0, 600),
                ("かっ", "仮促", 0, 0, 700),
            ],
            1,
            &[0],
        )
        .into_boxed_slice(),
    )
    .expect("合成词库");
    for input in ["kak", "kaxt", "kar", "kaq", "ka"] {
        let conversion = convert_romaji(input);
        let _ = search_converted(&dictionary, &conversion, 12);
        let mut old_batches = Vec::new();
        let mut new_batches = Vec::new();
        for batch in 0..10 {
            let mut old = || {
                let start = Instant::now();
                for _ in 0..1000 {
                    black_box(reference_current_search(
                        &dictionary,
                        black_box(&conversion),
                        12,
                    ));
                }
                old_batches.push(start.elapsed());
            };
            let mut new = || {
                let start = Instant::now();
                for _ in 0..1000 {
                    black_box(search_converted(&dictionary, black_box(&conversion), 12));
                }
                new_batches.push(start.elapsed());
            };
            if batch % 2 == 0 {
                old();
                new();
            } else {
                new();
                old();
            }
        }
        assert_eq!(
            search_converted(&dictionary, &conversion, 12),
            reference_current_search(&dictionary, &conversion, 12)
        );
        old_batches.sort_unstable();
        new_batches.sort_unstable();
        eprintln!(
            "matrix {input}：中位批次 {:?}→{:?}/1000次",
            old_batches[5], new_batches[5]
        );
    }
}
