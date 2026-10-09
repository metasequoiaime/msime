//! The matrix search the provider uses for sentence conversion (schemes-lang.md §5.6, `japanese_matrix_search.cpp`), modelled on Google Pinyin's MatrixSearch: one row per mora of the converted reading, k-best nodes per row extended by lemmas whose reading covers the next morae, plus a single unknown-kana backoff so every reading has a path.

use super::decoder::JapaneseDictionary;
use super::romaji::{kana_for_romaji_prefix_view, RomajiConversion};

pub const MAX_NODES_PER_ROW: usize = 8;
pub const MAX_LEMMA_MORA: usize = 16;
pub const UNKNOWN_KANA_COST: i32 = 12_000;

fn join_text(first: &str, second: &str) -> String {
    let mut text = String::with_capacity(first.len() + second.len());
    text.push_str(first);
    text.push_str(second);
    text
}

/// A converted text and its cost (sentence cost or lemma word cost).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JapaneseConversion {
    pub text: String,
    pub cost: i64,
}

#[derive(Debug, Clone)]
struct Node {
    text: String,
    cost: i64,
    right_id: u16,
}

/// Unique by text, never empty, at most `limit`.
struct Output {
    items: Vec<JapaneseConversion>,
    limit: usize,
}

impl Output {
    fn accepts(&self, text: &str) -> bool {
        // 输出页很小，扫描已有词面去重，避免另建字符串集合。
        !text.is_empty() && !self.full() && !self.items.iter().any(|item| item.text == text)
    }

    fn push(&mut self, text: &str, cost: i64) {
        if !self.accepts(text) {
            return;
        }
        self.items.push(JapaneseConversion {
            text: text.to_owned(),
            cost,
        });
    }

    fn push_owned(&mut self, text: String, cost: i64) {
        if self.accepts(&text) {
            self.items.push(JapaneseConversion { text, cost });
        }
    }

    fn full(&self) -> bool {
        self.items.len() >= self.limit
    }
}

/// 行未超限时保留插入顺序，首次超限后按成本稳定维护前八项。
#[derive(Default)]
struct Row {
    nodes: Vec<Node>,
    ranked: bool,
}

impl Row {
    fn new() -> Self {
        Self {
            nodes: Vec::with_capacity(MAX_NODES_PER_ROW),
            ranked: false,
        }
    }

    fn extend(&mut self, prefix: &str, suffix: &str, cost: i64, right_id: u16) {
        if self.nodes.len() < MAX_NODES_PER_ROW {
            self.nodes.push(Node {
                text: join_text(prefix, suffix),
                cost,
                right_id,
            });
            return;
        }
        if !self.ranked {
            // 第九项即使败选，原批量截断也会把已有八项稳定排序。
            self.nodes.sort_by_key(|node| node.cost);
            self.ranked = true;
        }
        if cost >= self.nodes.last().expect("full row").cost {
            return;
        }
        self.nodes.pop();
        // 后到的同分项排在既有项之后，保留原稳定排序的优先级。
        let index = self.nodes.partition_point(|node| node.cost <= cost);
        self.nodes.insert(
            index,
            Node {
                text: join_text(prefix, suffix),
                cost,
                right_id,
            },
        );
    }
}

/// `SearchReading(conversion.hiragana, conversion.pending, limit)`: the best sentence, pending-kana completions, the other finals, then longest-prefix lemmas, unique by text.
pub fn search_converted(
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

#[cfg(test)]
mod tests {
    use super::super::decoder::test_model;
    use super::super::romaji::convert_romaji;
    use super::*;

    fn dictionary(
        entries: &[test_model::Entry<'_>],
        size: u32,
        matrix: &[i16],
    ) -> JapaneseDictionary {
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("msime-japanese.dat");
        std::fs::write(&path, test_model::bytes(entries, size, matrix)).expect("write model");
        JapaneseDictionary::load(&path).expect("model loads")
    }

    fn texts(results: &[JapaneseConversion]) -> Vec<&str> {
        results.iter().map(|result| result.text.as_str()).collect()
    }

    #[test]
    fn owned_sentence_output_reuses_text_storage() {
        let mut text = String::with_capacity(64);
        text.push_str("合成句子😀");
        let pointer = text.as_ptr();
        let capacity = text.capacity();
        let mut output = Output {
            items: Vec::with_capacity(2),
            limit: 2,
        };
        let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            output.push_owned(text, -123);
        });
        eprintln!("日文句子输出文本分配：{allocations}");
        assert_eq!(output.items[0].text.as_ptr(), pointer);
        assert_eq!(output.items[0].text.capacity(), capacity);
        assert_eq!(
            output.items,
            [JapaneseConversion {
                text: "合成句子😀".to_owned(),
                cost: -123
            }]
        );
        assert_eq!(allocations, 0);
    }

    #[test]
    fn borrowed_and_owned_output_preserve_first_text_and_cost() {
        let inputs = [
            ("", 0),
            ("あ", -5),
            ("あ", -99),
            ("😀", 2),
            ("合成", 1),
            ("😀", -20),
        ];
        let expected = [("あ", -5), ("😀", 2), ("合成", 1)];
        for limit in [0, 1, 2, 3, 8] {
            for mode in 0..3 {
                let mut output = Output {
                    items: Vec::with_capacity(limit),
                    limit,
                };
                for (index, (text, cost)) in inputs.iter().enumerate() {
                    if mode == 1 || (mode == 2 && index % 2 == 0) {
                        output.push_owned((*text).to_owned(), *cost);
                    } else {
                        output.push(text, *cost);
                    }
                }
                let actual: Vec<_> = output
                    .items
                    .iter()
                    .map(|item| (item.text.as_str(), item.cost))
                    .collect();
                assert_eq!(
                    actual,
                    expected[..limit.min(expected.len())],
                    "limit={limit}, mode={mode}"
                );
            }
        }
    }

    #[test]
    fn rejected_owned_output_does_not_allocate() {
        for (limit, initial, rejected) in [
            (2, Some("あ"), ""),
            (2, Some("あ"), "あ"),
            (1, Some("あ"), "😀"),
            (0, None, "合成"),
        ] {
            let mut output = Output {
                items: Vec::with_capacity(limit),
                limit,
            };
            if let Some(text) = initial {
                output.push(text, 7);
            }
            let before = output.items.clone();
            let mut text = String::with_capacity(64);
            text.push_str(rejected);
            let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                output.push_owned(text, -99);
                output.push(rejected, -99);
            });
            assert_eq!(output.items, before);
            assert_eq!(allocations, 0, "limit={limit}, rejected={rejected:?}");
        }
    }

    #[test]
    fn join_text_allocates_only_result_bytes() {
        let text = super::join_text("蚊", "な");
        assert_eq!(text, "蚊な");
        assert_eq!(text.capacity(), text.len());
    }

    #[test]
    fn unknown_reading_search_does_not_allocate_empty_ranking_heaps() {
        let dictionary = dictionary(&[("かな", "仮名", 0, 0, 500)], 1, &[0]);
        let conversion = convert_romaji("kiki");
        let (actual, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            search_converted(&dictionary, &conversion, 16)
        });
        assert_eq!(
            actual,
            [JapaneseConversion {
                text: "きき".to_owned(),
                cost: 24_000
            }]
        );
        eprintln!("日文未命中双假名矩阵搜索分配：{allocations}");
        assert!(
            allocations <= 8,
            "未命中输出应接收已有句子文本：{allocations}"
        );
    }

    #[test]
    fn dense_single_mora_search_only_materializes_competitive_nodes() {
        let surfaces: Vec<_> = (0..24).map(|index| format!("語{index:02}")).collect();
        let entries: Vec<_> = surfaces
            .iter()
            .enumerate()
            .map(|(index, surface)| ("か", surface.as_str(), 0, 0, index as i32))
            .collect();
        let dictionary = dictionary(&entries, 1, &[0]);
        let conversion = convert_romaji("ka");
        let (actual, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            search_converted(&dictionary, &conversion, 16)
        });
        let expected: Vec<_> = (0..16)
            .map(|index| JapaneseConversion {
                text: format!("語{index:02}"),
                cost: index,
            })
            .collect();
        assert_eq!(actual, expected);
        eprintln!("日文密集单假名矩阵分配：{allocations}");
        assert!(
            allocations <= 25,
            "胜选文本应移入输出，败选不应构造文本：{allocations}"
        );
    }

    #[test]
    fn sentence_search_does_not_copy_temporary_lemma_strings() {
        let dictionary = dictionary(&[("か", "蚊", 0, 0, 500)], 1, &[0]);
        let conversion = convert_romaji("ka");
        let expected = search_converted(&dictionary, &conversion, 16);
        let (actual, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            search_converted(&dictionary, &conversion, 16)
        });
        assert_eq!(actual, expected);
        assert_eq!(texts(&actual), ["蚊", "か"]);
        eprintln!("日文单假名矩阵搜索分配：{allocations}");
        assert!(
            allocations <= 11,
            "词条应借用，句子文本应移入输出：{allocations}"
        );
    }

    fn search(
        dictionary: &JapaneseDictionary,
        romaji: &str,
        limit: usize,
    ) -> Vec<JapaneseConversion> {
        search_converted(dictionary, &convert_romaji(romaji), limit)
    }

    #[test]
    fn best_path_then_other_finals_then_longest_prefix_lemmas() {
        // Two ids: 1 follows 0 cheaply, 0 follows 1 expensively.
        let dictionary = dictionary(
            &[
                ("か", "蚊", 0, 1, 500),
                ("かな", "仮名", 0, 0, 900),
                ("かなし", "悲し", 0, 0, 2_000),
                ("し", "詩", 1, 0, 400),
            ],
            2,
            &[0, 10, 1_000, 0],
        );
        let results = search(&dictionary, "kanasi", 16);
        // 仮名+詩 = 900 + 10 + 400 = 1310, 悲し = 2000, 蚊+な+詩 pays an unknown kana.
        assert_eq!(
            results[0],
            JapaneseConversion {
                text: "仮名詩".to_owned(),
                cost: 1_310
            }
        );
        assert_eq!(
            results[1],
            JapaneseConversion {
                text: "悲し".to_owned(),
                cost: 2_000
            }
        );
        assert!(texts(&results).contains(&"蚊な詩"));
        // After the finals come the lemmas covering the longest leading reading, longest first.
        let tail: Vec<&str> = texts(&results).into_iter().rev().take(2).collect();
        assert_eq!(tail, vec!["蚊", "仮名"]);
        assert_eq!(search(&dictionary, "kanasi", 1).len(), 1);
        assert!(search(&dictionary, "kanasi", 0).is_empty());
    }

    #[test]
    fn unknown_kana_keep_a_path_for_any_reading() {
        let dictionary = dictionary(&[("か", "蚊", 0, 0, 500)], 1, &[0]);
        let results = search(&dictionary, "kaki", 16);
        assert_eq!(
            results[0],
            JapaneseConversion {
                text: "蚊き".to_owned(),
                cost: 12_500
            }
        );
        assert_eq!(texts(&results), vec!["蚊き", "かき", "蚊"]);
    }

    #[test]
    fn pending_letters_complete_the_reading() {
        let dictionary = dictionary(
            &[
                ("か", "蚊", 0, 0, 500),
                ("かき", "柿", 0, 0, 300),
                ("かきごおり", "かき氷", 0, 0, 800),
                ("かく", "書く", 0, 0, 200),
                ("かさ", "傘", 0, 0, 100),
            ],
            1,
            &[0],
        );
        let results = search(&dictionary, "kak", 16);
        // Best path for か, then the exact か+k-kana lemmas by kana order, then longer continuations.
        assert_eq!(texts(&results), vec!["蚊", "柿", "書く", "かき氷", "か"]);
    }

    #[test]
    fn only_pending_letters_use_prefix_lemmas() {
        let dictionary = dictionary(
            &[
                ("か", "蚊", 0, 0, 500),
                ("かき", "柿", 0, 0, 300),
                ("き", "木", 0, 0, 100),
            ],
            1,
            &[0],
        );
        assert_eq!(texts(&search(&dictionary, "k", 16)), vec!["柿", "蚊", "木"]);
        assert_eq!(texts(&search(&dictionary, "k", 2)), vec!["柿", "蚊"]);
        assert!(search(&dictionary, "q", 16).is_empty());
    }
}

#[cfg(test)]
mod pruning_tests;

#[cfg(test)]
#[path = "matrix/pending_tests.rs"]
mod pending_tests;
