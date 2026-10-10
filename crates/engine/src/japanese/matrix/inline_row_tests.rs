use super::super::decoder::test_model;
use super::super::romaji::convert_romaji;
use super::inline_row_experiment::{search_converted as inline_search, Row as InlineRow};
use super::vector_row_reference::Row;
use super::*;

// 固定 33a5a3f46 的完整矩阵正文，使用冻结 Vec 行隔离本片存储变化。
pub(super) fn vector_reference_search(
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

fn fixture(dense: bool) -> JapaneseDictionary {
    let surfaces: Vec<_> = (0..24).map(|index| format!("合成{index:02}")).collect();
    let mut entries = vec![
        ("か", "仮", 0, 1, -500),
        ("かか", "仮仮", 1, 0, -700),
        ("かき", "仮木", 1, 1, -300),
        ("かきゃ", "仮甲", 0, 0, 20),
        ("かっ", "仮促", 0, 0, 30),
        ("き", "木", 1, 0, -100),
        ("甲😀か", "合成長", 0, 1, -40),
    ];
    if dense {
        entries.extend(surfaces.iter().enumerate().map(|(index, surface)| {
            (
                "か",
                surface.as_str(),
                (index % 2) as u16,
                (index % 2) as u16,
                index as i32 - 12,
            )
        }));
    }
    entries.sort_by_key(|entry| entry.0);
    JapaneseDictionary::from_bytes(
        test_model::bytes(&entries, 2, &[0, -20, 80, 10]).into_boxed_slice(),
    )
    .expect("合成词库")
}

#[test]
fn inline_row_creation_and_take_preserve_only_active_nodes_without_allocating() {
    let (mut row, allocations) = crate::ime::personal_rerank::allocations::count(InlineRow::new);
    assert_eq!(allocations, 0);
    assert!(row.nodes().is_empty());
    row.extend("合成", "😀", -77, 9);
    let pointer = row.nodes()[0].text.as_ptr();
    let (taken, allocations) =
        crate::ime::personal_rerank::allocations::count(|| std::mem::take(&mut row));
    assert_eq!(allocations, 0);
    assert!(row.nodes().is_empty());
    assert_eq!(taken.nodes()[0].text.as_ptr(), pointer);
    assert_eq!(taken.nodes()[0].cost, -77);
    assert_eq!(taken.nodes()[0].right_id, 9);
    let active: Vec<_> = taken.nodes.into_iter().take(taken.used).collect();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].text, "合成😀");
}

#[test]
fn inline_full_row_replacements_match_vector_at_each_insertion_position() {
    for replacement in [-1, 0, 5, 12, 13, 14, 99] {
        let mut inline = InlineRow::new();
        let mut vector = Row::new();
        for cost in [14, 0, 10, 2, 12, 4, 8, 6, replacement, replacement] {
            let text = format!("合成{cost}");
            inline.extend("", &text, cost, cost.unsigned_abs() as u16);
            vector.extend("", &text, cost, cost.unsigned_abs() as u16);
            let fields = |nodes: &[Node]| {
                nodes
                    .iter()
                    .map(|node| (node.text.clone(), node.cost, node.right_id))
                    .collect::<Vec<_>>()
            };
            assert_eq!(fields(inline.nodes()), fields(&vector.nodes));
        }
    }
}

#[test]
fn inline_search_matches_vector_fields_and_saves_one_buffer_per_row() {
    for dense in [false, true] {
        let dictionary = fixture(dense);
        for reading in [
            "",
            "か",
            "かき",
            "きき",
            "甲😀か",
            &"か".repeat(32),
            &"😀".repeat(64),
        ] {
            for pending in ["", "k", "K", "sh", "xt", "d", "z", "q", "あ", "toolong"] {
                let conversion = RomajiConversion {
                    hiragana: reading.to_owned(),
                    pending: pending.to_owned(),
                    complete: pending.is_empty(),
                };
                let _ = inline_search(&dictionary, &conversion, 16);
                for limit in [0, 1, 2, 8, 16, 32] {
                    let (expected, old_allocations) =
                        crate::ime::personal_rerank::allocations::count(|| {
                            vector_reference_search(&dictionary, &conversion, limit)
                        });
                    let (actual, new_allocations) =
                        crate::ime::personal_rerank::allocations::count(|| {
                            inline_search(&dictionary, &conversion, limit)
                        });
                    assert_eq!(
                        actual,
                        expected,
                        "密集={dense}，字符数={}，待定={pending}，限额={limit}",
                        reading.chars().count()
                    );
                    let saved = if reading.is_empty() || limit == 0 {
                        0
                    } else {
                        reading.chars().count() + 1
                    };
                    assert_eq!(new_allocations + saved, old_allocations);
                }
            }
        }
    }
}

#[test]
#[ignore = "本地 release 固定 Vec 行对照，交替计时；不设置 CI 时间阈值"]
fn benchmark_inline_matrix_rows() {
    use std::hint::black_box;
    use std::time::Instant;
    eprintln!(
        "布局：Node={}，旧Row={}，新Row={}，每行旧总存储={}，新总存储={}",
        std::mem::size_of::<Node>(),
        std::mem::size_of::<Row>(),
        std::mem::size_of::<InlineRow>(),
        std::mem::size_of::<Row>() + MAX_NODES_PER_ROW * std::mem::size_of::<Node>(),
        std::mem::size_of::<InlineRow>()
    );
    for (dense, input, iterations) in [
        (false, "ka".to_owned(), 2000),
        (false, "kiki".to_owned(), 2000),
        (true, "ka".to_owned(), 1000),
        (true, "kaka".to_owned(), 1000),
        (false, "ka".repeat(32), 100),
        (true, "ka".repeat(32), 100),
        (false, "😀".repeat(128), 100),
        (false, "kak".to_owned(), 1000),
        (false, "k".to_owned(), 2000),
        (false, "".to_owned(), 2000),
    ] {
        let iterations = iterations * 10;
        let dictionary = fixture(dense);
        let conversion = if input.starts_with('😀') {
            // 非罗马字长读音直接进入矩阵，避免转换器把它留在待定尾部。
            RomajiConversion {
                hiragana: input.clone(),
                pending: String::new(),
                complete: true,
            }
        } else {
            convert_romaji(&input)
        };
        for limit in [0, 16] {
            assert_eq!(
                inline_search(&dictionary, &conversion, limit),
                vector_reference_search(&dictionary, &conversion, limit)
            );
            let (_, old_allocations) = crate::ime::personal_rerank::allocations::count(|| {
                vector_reference_search(&dictionary, &conversion, limit)
            });
            let (_, new_allocations) = crate::ime::personal_rerank::allocations::count(|| {
                inline_search(&dictionary, &conversion, limit)
            });
            let mut timings = [Vec::new(), Vec::new()];
            for batch in 0..10 {
                for index in if batch % 2 == 0 { [0, 1] } else { [1, 0] } {
                    let start = Instant::now();
                    for _ in 0..iterations {
                        let result = if index == 0 {
                            vector_reference_search(
                                black_box(&dictionary),
                                black_box(&conversion),
                                black_box(limit),
                            )
                        } else {
                            inline_search(
                                black_box(&dictionary),
                                black_box(&conversion),
                                black_box(limit),
                            )
                        };
                        black_box(result);
                    }
                    timings[index].push(start.elapsed());
                }
            }
            for samples in &mut timings {
                samples.sort_unstable();
            }
            eprintln!("行计时：密集={dense}，输入={input:?}，读音字符={}，限额={limit}，分配={old_allocations}→{new_allocations}，中位批次={:?}→{:?}/{iterations}次", conversion.hiragana.chars().count(), timings[0][5], timings[1][5]);
        }
    }
}
