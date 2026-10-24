use super::*;
use crate::ime::personal_rerank::allocations::count;

fn baseline_sentence_row(
    typed_pinyin: &str,
    path: &SentencePath,
    source: CandidateSource,
) -> WordItem {
    baseline_owned_sentence_row(typed_pinyin, path.clone(), source)
}

fn baseline_owned_sentence_row(
    typed_pinyin: &str,
    path: SentencePath,
    source: CandidateSource,
) -> WordItem {
    let mut item = WordItem::new(
        typed_pinyin,
        path.sentence,
        (path.log_prob * 1000.0) as i64,
        source,
        path.key,
    );
    item.sentence_association = true;
    item.sentence_words = path.words;
    item
}

fn baseline_sentence_seen_linear(
    candidates: &[WordItem],
    block: &[WordItem],
    sentence: &str,
) -> bool {
    candidates
        .iter()
        .chain(block)
        .any(|item| item.word == sentence)
}

fn baseline_take_linear_sentence<'a>(
    candidates: &[WordItem],
    block: &[WordItem],
    ranked: impl IntoIterator<Item = &'a SentencePath>,
    source: CandidateSource,
    options: &LatticeOptions<'_>,
    typed_pinyin: &str,
) -> Option<WordItem> {
    for path in ranked {
        if !baseline_sentence_seen_linear(candidates, block, &path.sentence) {
            return Some(baseline_sentence_row(typed_pinyin, path, source));
        }
        if !options.show_next_on_duplicate {
            return None;
        }
    }
    None
}

fn baseline_reranked_block_linear<'a>(
    candidates: &[WordItem],
    paths: &[SentencePath],
    keyboard: Option<impl IntoIterator<Item = &'a SentencePath>>,
    options: &LatticeOptions<'_>,
    typed_pinyin: &str,
) -> Vec<WordItem> {
    let mut block = Vec::new();
    if options.include_lattice_best {
        if let Some(row) = baseline_take_linear_sentence(
            candidates,
            &block,
            paths,
            CandidateSource::Generated,
            options,
            typed_pinyin,
        ) {
            block = Vec::with_capacity(2);
            block.push(row);
        }
    }
    if let Some(keyboard) = keyboard {
        if let Some(row) = baseline_take_linear_sentence(
            candidates,
            &block,
            keyboard,
            CandidateSource::NeuralKeyboard,
            options,
            typed_pinyin,
        ) {
            if block.is_empty() {
                block = Vec::with_capacity(2);
            }
            block.push(row);
        }
    }
    block
}

fn baseline_take_hashed_sentence<'a>(
    ranked: impl IntoIterator<Item = &'a SentencePath>,
    source: CandidateSource,
    options: &LatticeOptions<'_>,
    typed_pinyin: &str,
    already: &mut HashSet<&'a str>,
) -> Option<WordItem> {
    for path in ranked {
        if already.insert(path.sentence.as_str()) {
            return Some(baseline_sentence_row(typed_pinyin, path, source));
        }
        if !options.show_next_on_duplicate {
            return None;
        }
    }
    None
}

fn baseline_reranked_block<'a>(
    paths: &'a [SentencePath],
    keyboard: Option<impl IntoIterator<Item = &'a SentencePath>>,
    options: &LatticeOptions<'_>,
    typed_pinyin: &str,
    already: &mut HashSet<&'a str>,
) -> Vec<WordItem> {
    let lattice = if options.include_lattice_best {
        baseline_take_hashed_sentence(
            paths,
            CandidateSource::Generated,
            options,
            typed_pinyin,
            already,
        )
    } else {
        None
    };
    let pick = keyboard.and_then(|keyboard| {
        baseline_take_hashed_sentence(
            keyboard,
            CandidateSource::NeuralKeyboard,
            options,
            typed_pinyin,
            already,
        )
    });
    let mut block = if lattice.is_some() || pick.is_some() {
        Vec::with_capacity(2)
    } else {
        Vec::new()
    };
    if let Some(row) = lattice {
        block.push(row);
    }
    if let Some(row) = pick {
        block.push(row);
    }
    block
}

fn path(index: usize) -> SentencePath {
    SentencePath {
        sentence: format!("合成{}😀", index % 3),
        key: format!("a'ba'ca{index}"),
        log_prob: -1.25 - index as f64,
        words: (0..index % 4).map(|word| format!("合成词{word}")).collect(),
        typo_edges: index % 2,
    }
}

#[test]
fn selected_neural_rows_preserve_sources_duplicates_and_exact_copy_savings() {
    let mut cases = 0;
    for n in [0, 1, 2, 6, 65, 129] {
        let paths: Vec<_> = (0..n).map(path).collect();
        let costs: Vec<_> = paths.iter().map(|path| count(|| path.clone()).1).collect();
        let orders = [
            None,
            Some(Vec::new()),
            Some((0..n).collect::<Vec<_>>()),
            Some((0..n).rev().collect()),
            Some((0..n.min(12)).rev().chain(n.min(12)..n).collect()),
        ];
        for keyboard in &orders {
            for include_lattice_best in [false, true] {
                for show_next_on_duplicate in [false, true] {
                    let options = LatticeOptions {
                        include_lattice_best,
                        show_next_on_duplicate,
                        ..LatticeOptions::default()
                    };
                    for blocked in [0, 1, 3] {
                        for filler in [0, 65] {
                            let mut candidates: Vec<_> = (0..blocked)
                                .map(|index| {
                                    WordItem::new(
                                        "abaca",
                                        format!("合成{index}😀"),
                                        1,
                                        CandidateSource::Database,
                                        "a'ba'ca",
                                    )
                                })
                                .collect();
                            candidates.extend((0..filler).map(|index| {
                                WordItem::new(
                                    "abaca",
                                    format!("占位{index}"),
                                    1,
                                    CandidateSource::CloudSuggestion,
                                    "",
                                )
                            }));
                            for hashed in [false, true] {
                                let owned = paths.clone();
                                let mut before_seen: HashSet<_> =
                                    candidates.iter().map(|item| item.word.as_str()).collect();
                                before_seen.reserve(2);
                                let after_seen = before_seen.clone();
                                let old_keyboard = keyboard
                                    .as_ref()
                                    .map(|order| order.iter().map(|&index| &paths[index]));
                                let (old, before) = count(|| {
                                    if hashed {
                                        baseline_reranked_block(
                                            &paths,
                                            old_keyboard,
                                            &options,
                                            "abaca",
                                            &mut before_seen,
                                        )
                                    } else {
                                        baseline_reranked_block_linear(
                                            &candidates,
                                            &paths,
                                            old_keyboard,
                                            &options,
                                            "abaca",
                                        )
                                    }
                                });
                                let new_keyboard =
                                    keyboard.as_ref().map(|order| order.iter().copied());
                                let (new, after) = count(|| {
                                    reranked_block(
                                        owned,
                                        0..n,
                                        new_keyboard,
                                        &options,
                                        "abaca",
                                        |sentence| {
                                            if hashed {
                                                after_seen.contains(sentence)
                                            } else {
                                                candidates.iter().any(|item| item.word == sentence)
                                            }
                                        },
                                    )
                                });
                                assert_eq!(new, old);
                                assert_eq!(new.capacity(), old.capacity());
                                let saved: usize = new
                                    .iter()
                                    .map(|item| {
                                        let index = paths
                                            .iter()
                                            .position(|path| path.key == item.canonical_pinyin)
                                            .unwrap();
                                        costs[index]
                                    })
                                    .sum();
                                assert_eq!(after + saved, before, "路径={n}/键盘={keyboard:?}/字面={include_lattice_best}/继续={show_next_on_duplicate}/已有={blocked}/占位={filler}/哈希={hashed}");
                                // 每行一个 typed_pinyin 字符串，加上 `pinyin`、`canonical_pinyin`、`word` 各一个 `SharedString` 的 `Arc`。
                                assert_eq!(after, usize::from(!new.is_empty()) + new.len() * 4);
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, 1440);
}

#[test]
fn neural_rows_move_original_fields_and_restore_source_order() {
    let mut paths: Vec<_> = (0..3).map(path).collect();
    for path in &mut paths {
        path.sentence.reserve(96);
        path.key.reserve(64);
        path.words.reserve(8);
        for word in &mut path.words {
            word.reserve(80);
        }
    }
    let pointers: Vec<_> = paths
        .iter()
        .map(|path| {
            (
                path.sentence.as_ptr(),
                path.sentence.capacity(),
                path.key.as_ptr(),
                path.key.capacity(),
                path.words.as_ptr(),
                path.words.capacity(),
                path.words
                    .iter()
                    .map(|word| (word.as_ptr(), word.capacity()))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let options = LatticeOptions::default();
    let (rows, allocations) =
        count(|| reranked_block(paths, 2..3, Some([0]), &options, "abaca", |_| false));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].source, CandidateSource::Generated);
    assert_eq!(rows[1].source, CandidateSource::NeuralKeyboard);
    for (row, index) in rows.iter().zip([2, 0]) {
        let expected = &pointers[index];
        assert_eq!(
            (
                row.word.as_ptr(),
                row.word.capacity(),
                row.canonical_pinyin.as_ptr(),
                row.canonical_pinyin.capacity(),
                row.sentence_words.as_ptr(),
                row.sentence_words.capacity()
            ),
            (expected.0, expected.1, expected.2, expected.3, expected.4, expected.5)
        );
        assert_eq!(
            row.sentence_words
                .iter()
                .map(|word| (word.as_ptr(), word.capacity()))
                .collect::<Vec<_>>(),
            expected.6
        );
    }
    // 句子和键的缓冲区原样移进 `SharedString`，每行只为 `pinyin`、`canonical_pinyin`、`word` 各建一个 `Arc`：1 + 2 + 2 × 3。
    assert_eq!(
        allocations, 9,
        "只有输出块、两行typed_pinyin和每个字段的Arc分配"
    );
}
