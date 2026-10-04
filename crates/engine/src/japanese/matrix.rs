//! The matrix search the provider uses for sentence conversion (schemes-lang.md §5.6, `japanese_matrix_search.cpp`), modelled on Google Pinyin's MatrixSearch: one row per mora of the converted reading, k-best nodes per row extended by lemmas whose reading covers the next morae, plus a single unknown-kana backoff so every reading has a path.

use super::decoder::JapaneseDictionary;
use super::romaji::{kana_for_romaji_prefix, RomajiConversion};

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
    fn push(&mut self, text: &str, cost: i64) {
        if text.is_empty() || self.full() || self.items.iter().any(|item| item.text == text) {
            return;
        }
        self.items.push(JapaneseConversion {
            text: text.to_owned(),
            cost,
        });
    }

    fn full(&self) -> bool {
        self.items.len() >= self.limit
    }
}

/// The C++ pruned with an unstable `partial_sort` on cost alone; a stable sort keeps the insertion order among equal costs so ties are deterministic.
fn keep_best(row: &mut Vec<Node>, limit: usize) {
    if row.len() <= limit {
        return;
    }
    row.sort_by_key(|node| node.cost);
    row.truncate(limit);
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
    let pending_kana = kana_for_romaji_prefix(pending);
    if reading.is_empty() {
        for kana in &pending_kana {
            for lemma in dictionary.prefix_lemmas(kana, 24) {
                output.push(&lemma.surface, i64::from(lemma.word_cost));
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
            for lemma in dictionary.exact_lemmas(key, 24) {
                for previous in &previous_row {
                    let cost = previous.cost
                        + i64::from(lemma.word_cost)
                        + i64::from(dictionary.connection_cost(previous.right_id, lemma.left_id));
                    rows[end].push(Node {
                        text: join_text(&previous.text, &lemma.surface),
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
            keep_best(row, MAX_NODES_PER_ROW);
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
            for lemma in dictionary.exact_lemmas(&join_text(reading, kana), 16) {
                output.push(&lemma.surface, i64::from(lemma.word_cost));
            }
        }
        for lemma in dictionary.prefix_lemmas_continuing(reading, &pending_kana, 48) {
            output.push(&lemma.surface, i64::from(lemma.word_cost));
        }
    }

    for node in finals.iter().skip(1) {
        output.push(&node.text, node.cost);
    }

    for end in (1..=mora_count).rev() {
        if output.full() {
            break;
        }
        for lemma in dictionary.exact_lemmas(&reading[..boundaries[end]], 16) {
            output.push(&lemma.surface, i64::from(lemma.word_cost));
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
    fn join_text_allocates_only_result_bytes() {
        let text = super::join_text("蚊", "な");
        assert_eq!(text, "蚊な");
        assert_eq!(text.capacity(), text.len());
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
