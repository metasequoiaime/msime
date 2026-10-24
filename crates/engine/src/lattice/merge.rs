//! Merging decoded sentences into the candidate list (quanpin.md §10.4, overlays.md §1.6.2).
//!
//! Ranking when merging into an existing list: exact SQLite full-key hits (Database / UserDatabase) first, then the lattice's whole sentences, then prefixes and everything else. The lattice never displaces a leading exact full-cover row (高碳钢 for gktjgh). Lattice rows weigh `trunc(log_prob * 1000)`, often negative: list order is the ranking, never sort these rows by weight.

use std::borrow::Cow;
use std::collections::HashSet;

use super::decode::{
    build_graph, build_graph_borrowed, decode_graph, decode_typo_on_graph,
    decode_typo_on_graph_with_policy, LatticeOptions, SentencePath, TypoSentence,
};
use super::neural::{NeuralReranker, MAX_RERANK_PATHS};
use super::{LatticeLookup, TypoEdgeSource};
use crate::pinyin::syllables::has_only_complete_pinyin_segments;
use crate::types::{CandidateSource, WordItem};

// 短句子合并直接扫描已有行，避免默认小批次为临时哈希表分配堆内存。
const SMALL_SENTENCE_MERGE: usize = 64;

/// Decode `syllables` and insert the sentence rows (Generated, and the NeuralKeyboard pick when the keyboard reranker is given) as one block at `whole_sentence_insert_position`. Nothing for fewer than two syllables or an incomplete one. With `typo_source` and three or more syllables, also returns the typo sentence for the caller to place; the source receives the literal best path and returns the planned typo edges.
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub fn merge_lattice_candidates(
    candidates: &mut Vec<WordItem>,
    syllables: &[String],
    lookup: &mut LatticeLookup<'_>,
    typed_pinyin: &str,
    options: &LatticeOptions<'_>,
    typo_source: Option<&mut TypoEdgeSource<'_>>,
    rerankers: &mut [NeuralReranker],
    rescoring_context: &str,
) -> Option<TypoSentence> {
    merge_lattice_candidates_with_typo_policy(
        candidates,
        syllables,
        lookup,
        typed_pinyin,
        options,
        typo_source,
        rerankers,
        rescoring_context,
        false,
    )
}

/// 双拼的纠错整句即使分数低于字面整句也要保留为候选；全拼沿用只接受胜过字面整句的规则。
#[allow(clippy::too_many_arguments)]
pub(crate) fn merge_lattice_candidates_with_typo_policy(
    candidates: &mut Vec<WordItem>,
    syllables: &[String],
    lookup: &mut LatticeLookup<'_>,
    typed_pinyin: &str,
    options: &LatticeOptions<'_>,
    typo_source: Option<&mut TypoEdgeSource<'_>>,
    rerankers: &mut [NeuralReranker],
    rescoring_context: &str,
    allow_nonleading_typo: bool,
) -> Option<TypoSentence> {
    // Two syllables are enough since lattice_reading: a two-syllable key can miss the exact lookup and otherwise leave prefix-range rows ahead of the sentence. Abbreviated segments (g'k't) are refused so `canonical_pinyin` stays a complete pronunciation (WL:541-545).
    let decoded = decode_lattice_with_typo_policy(
        syllables,
        lookup,
        options,
        typo_source,
        allow_nonleading_typo,
    )?;
    merge_decoded(
        candidates,
        syllables,
        typed_pinyin,
        options,
        rerankers,
        rescoring_context,
        decoded,
    )
}

/// 一次整句解码的结果：未截断的路径和纠错整句。只取决于音节、查到的词库行和解码参数，与已有候选、重排上下文无关，所以可以按音节缓存后再交给 `merge_decoded`。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DecodedSentences {
    pub(crate) paths: Vec<SentencePath>,
    pub(crate) typo: Option<TypoSentence>,
}

/// `merge_lattice_candidates` 的解码一半：少于两个音节或含不完整音节时为 `None`，与合并时不插入任何行一致。
pub(crate) fn decode_lattice(
    syllables: &[String],
    lookup: &mut LatticeLookup<'_>,
    options: &LatticeOptions<'_>,
    typo_source: Option<&mut TypoEdgeSource<'_>>,
) -> Option<DecodedSentences> {
    decode_lattice_with_typo_policy(syllables, lookup, options, typo_source, false)
}

fn decode_lattice_with_typo_policy(
    syllables: &[String],
    lookup: &mut LatticeLookup<'_>,
    options: &LatticeOptions<'_>,
    typo_source: Option<&mut TypoEdgeSource<'_>>,
    allow_nonleading_typo: bool,
) -> Option<DecodedSentences> {
    if syllables.len() < 2 || !has_only_complete_pinyin_segments(syllables) {
        return None;
    }
    let graph = build_graph(syllables, lookup, options);
    Some(decode_graph_sentences_with_policy(
        syllables,
        options,
        typo_source,
        &graph,
        allow_nonleading_typo,
    ))
}

/// 借用缓存行建图的 `decode_lattice`：词典缓存行在建图期间保持借用，避免缓存命中先复制整页行再复制图边字段。
pub(crate) fn decode_lattice_borrowed<'rows>(
    syllables: &[String],
    lookup: &mut impl FnMut(&[String]) -> &'rows [super::decode::LatticeLexeme],
    options: &LatticeOptions<'_>,
    typo_source: Option<&mut TypoEdgeSource<'_>>,
) -> Option<DecodedSentences> {
    if syllables.len() < 2 || !has_only_complete_pinyin_segments(syllables) {
        return None;
    }
    let graph = build_graph_borrowed(syllables, lookup, options);
    Some(decode_graph_sentences_with_policy(
        syllables,
        options,
        typo_source,
        &graph,
        false,
    ))
}

#[cfg(test)]
fn decode_graph_sentences(
    syllables: &[String],
    options: &LatticeOptions<'_>,
    typo_source: Option<&mut TypoEdgeSource<'_>>,
    graph: &super::decode::Graph,
) -> DecodedSentences {
    decode_graph_sentences_with_policy(syllables, options, typo_source, graph, false)
}

fn decode_graph_sentences_with_policy(
    syllables: &[String],
    options: &LatticeOptions<'_>,
    typo_source: Option<&mut TypoEdgeSource<'_>>,
    graph: &super::decode::Graph,
    allow_nonleading_typo: bool,
) -> DecodedSentences {
    let paths = decode_graph(graph, options, None);
    if paths.is_empty() {
        return DecodedSentences { paths, typo: None };
    }
    // Typo spans cover two or three syllables, so a shorter input would only ever be replaced whole. Decoded from the same graph so the dictionary is not asked twice, and from the unreranked best so the typo sentence answers the literal reading.
    let typo = match typo_source {
        Some(source) if syllables.len() >= 3 => {
            let edges = source(&paths[0]);
            if edges.is_empty() {
                None
            } else {
                if allow_nonleading_typo {
                    decode_typo_on_graph_with_policy(graph, options, &paths[0], &edges, true)
                } else {
                    decode_typo_on_graph(graph, options, &paths[0], &edges)
                }
            }
        }
        _ => None,
    };
    DecodedSentences { paths, typo }
}

/// `merge_lattice_candidates` 的合并一半：把解码出的整句按 `emit`、重排和去重插进候选，返回纠错整句交给调用方安放。
pub(crate) fn merge_decoded(
    candidates: &mut Vec<WordItem>,
    syllables: &[String],
    typed_pinyin: &str,
    options: &LatticeOptions<'_>,
    rerankers: &mut [NeuralReranker],
    rescoring_context: &str,
    decoded: DecodedSentences,
) -> Option<TypoSentence> {
    let DecodedSentences { paths, typo } = decoded;
    if paths.is_empty() {
        return None;
    }
    merge_decoded_paths(
        candidates,
        syllables,
        typed_pinyin,
        options,
        rerankers,
        rescoring_context,
        Cow::Owned(paths),
        typo,
    )
}

/// 缓存命中时先借用路径，只为最终入选的候选复制字段；缓存内容与容量保持不变。
pub(crate) fn merge_decoded_borrowed(
    candidates: &mut Vec<WordItem>,
    syllables: &[String],
    typed_pinyin: &str,
    options: &LatticeOptions<'_>,
    rerankers: &mut [NeuralReranker],
    rescoring_context: &str,
    decoded: &DecodedSentences,
) -> Option<TypoSentence> {
    merge_decoded_paths(
        candidates,
        syllables,
        typed_pinyin,
        options,
        rerankers,
        rescoring_context,
        Cow::Borrowed(&decoded.paths),
        decoded.typo.clone(),
    )
}

#[allow(clippy::too_many_arguments)]
// 在两个入口保留已知的拥有方式，避免冷路径为借用支持增加一层调用。
#[inline(always)]
fn merge_decoded_paths(
    candidates: &mut Vec<WordItem>,
    syllables: &[String],
    typed_pinyin: &str,
    options: &LatticeOptions<'_>,
    rerankers: &mut [NeuralReranker],
    rescoring_context: &str,
    mut paths: Cow<'_, [SentencePath]>,
    typo: Option<TypoSentence>,
) -> Option<TypoSentence> {
    if paths.is_empty() {
        return None;
    }

    if rerankers.is_empty() && options.emit > 0 {
        paths = match paths {
            Cow::Owned(mut paths) => {
                paths.truncate(options.emit);
                Cow::Owned(paths)
            }
            Cow::Borrowed(paths) => Cow::Borrowed(&paths[..paths.len().min(options.emit)]),
        };
    }
    let block = if rerankers.is_empty() {
        let already = existing_sentence_set(candidates, paths.len());
        // `emit` 先截断，备选路径用于三元重排；只把实际显示的默认行转为拥有候选。
        if let Some(already) = already.as_ref() {
            let mut block = Vec::new();
            let capacity = paths.len();
            // `decode_graph` 已消除同句的全部切分，此处只需过滤已有候选。
            for path in sentence_path_iter(paths) {
                if already.contains(path.sentence.as_str()) {
                    continue;
                }
                // 只放搭配行时词格自己的备选不上榜；
                // 搭配开关没开时这个开关本来就被忽略（无重排基线语义）。
                if !block.is_empty()
                    && options.include_collocation_best
                    && !options.include_lattice_best
                {
                    break;
                }
                if block.is_empty() {
                    block = Vec::with_capacity(capacity);
                }
                block.push(owned_sentence_row(
                    typed_pinyin,
                    path.into_owned(),
                    lattice_row_source(&block, options),
                ));
            }
            block
        } else {
            generated_block_linear(candidates, paths, typed_pinyin, options)
        }
    } else {
        let mut keyboard_order = [0; MAX_RERANK_PATHS];
        let mut keyboard = None;
        for reranker in rerankers
            .iter_mut()
            .filter(|reranker| reranker.source == CandidateSource::NeuralKeyboard)
        {
            if let Some(scored) =
                reranker.score_order_into(&paths, rescoring_context, &mut keyboard_order)
            {
                keyboard = Some(scored);
            }
        }
        // 所有模型处理完后确认没有可输出来源，保留已完成的纠错结果。
        if !options.include_lattice_best && !options.include_collocation_best && keyboard.is_none()
        {
            return typo;
        }
        let already = existing_sentence_set(candidates, paths.len());
        // 各模型始终评分原始路径；最后成功的排列接回恒等尾部，再移交选中字段。
        let path_count = paths.len();
        let ranked = keyboard.map(|scored| {
            keyboard_order[..scored]
                .iter()
                .copied()
                .chain(scored..path_count)
        });
        if let Some(already) = already.as_ref() {
            reranked_block(
                paths,
                0..path_count,
                ranked,
                options,
                typed_pinyin,
                |sentence| already.contains(sentence),
            )
        } else {
            reranked_block(
                paths,
                0..path_count,
                ranked,
                options,
                typed_pinyin,
                |sentence| candidates.iter().any(|item| item.word == sentence),
            )
        }
    };
    if !block.is_empty() {
        let at = whole_sentence_insert_position(candidates, syllables);
        candidates.splice(at..at, block);
    }
    typo
}

// 单路径只需扫描一次候选；多路径仍按候选加路径总数决定是否建立集合。
fn existing_sentence_set(candidates: &[WordItem], path_count: usize) -> Option<HashSet<&str>> {
    let dedup_size = candidates.len().saturating_add(path_count);
    (path_count > 1 && dedup_size > SMALL_SENTENCE_MERGE).then(|| {
        let mut already = HashSet::with_capacity(dedup_size);
        already.extend(candidates.iter().map(|item| item.word.as_str()));
        already
    })
}

fn owned_sentence_row(typed_pinyin: &str, path: SentencePath, source: CandidateSource) -> WordItem {
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

enum SentencePathIter<'a> {
    Owned(std::vec::IntoIter<SentencePath>),
    Borrowed(std::slice::Iter<'a, SentencePath>),
}

impl<'a> Iterator for SentencePathIter<'a> {
    type Item = Cow<'a, SentencePath>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Owned(paths) => paths.next().map(Cow::Owned),
            Self::Borrowed(paths) => paths.next().map(Cow::Borrowed),
        }
    }
}

fn sentence_path_iter(paths: Cow<'_, [SentencePath]>) -> SentencePathIter<'_> {
    match paths {
        Cow::Owned(paths) => SentencePathIter::Owned(paths.into_iter()),
        Cow::Borrowed(paths) => SentencePathIter::Borrowed(paths.iter()),
    }
}

fn sentence_seen_linear(candidates: &[WordItem], block: &[WordItem], sentence: &str) -> bool {
    candidates
        .iter()
        .chain(block)
        .any(|item| item.word == sentence)
}

/// 词格行的来源标记：语法模型开着时首行是解码最优路径（`Collocation` 的归属），
/// 其余行是词格自己的备选（`Generated`）。`include_lattice_best` 只在
/// `include_collocation_best` 同开时才让首行之后的备选继续上榜——那是旧行为里
/// 本就不看 `include_lattice_best` 的无重排通道。
fn lattice_row_source(block: &[WordItem], options: &LatticeOptions<'_>) -> CandidateSource {
    if block.is_empty() && options.include_collocation_best {
        CandidateSource::Collocation
    } else {
        CandidateSource::Generated
    }
}

fn generated_block_linear<'a>(
    candidates: &[WordItem],
    paths: impl Into<Cow<'a, [SentencePath]>>,
    typed_pinyin: &str,
    options: &LatticeOptions<'_>,
) -> Vec<WordItem> {
    let paths = paths.into();
    let mut block = Vec::new();
    let capacity = paths.len();
    for path in sentence_path_iter(paths) {
        if sentence_seen_linear(candidates, &block, &path.sentence) {
            continue;
        }
        // 只放搭配行时词格自己的备选不上榜；
        // 搭配开关没开时这个开关本来就被忽略（无重排基线语义）。
        if !block.is_empty() && options.include_collocation_best && !options.include_lattice_best {
            break;
        }
        // 全部重复时不分配；首条保留句子沿用原容量，避免非空块新增扩容。
        if block.is_empty() {
            block = Vec::with_capacity(capacity);
        }
        block.push(owned_sentence_row(
            typed_pinyin,
            path.into_owned(),
            lattice_row_source(&block, options),
        ));
    }
    block
}

fn take_sentence_index(
    paths: &[SentencePath],
    ranked: impl IntoIterator<Item = usize>,
    previous: Option<usize>,
    options: &LatticeOptions<'_>,
    is_listed: &impl Fn(&str) -> bool,
) -> Option<usize> {
    for index in ranked {
        let sentence = &paths[index].sentence;
        let seen = is_listed(sentence)
            || previous.is_some_and(|selected| paths[selected].sentence == *sentence);
        if !seen {
            return Some(index);
        }
        if !options.show_next_on_duplicate {
            return None;
        }
    }
    None
}

/// 两个来源先按旧规则选索引，再把字段交给输出槽；槽顺序始终是字面、键盘。
fn reranked_block<'a>(
    paths: impl Into<Cow<'a, [SentencePath]>>,
    lattice: impl IntoIterator<Item = usize>,
    keyboard: Option<impl IntoIterator<Item = usize>>,
    options: &LatticeOptions<'_>,
    typed_pinyin: &str,
    is_listed: impl Fn(&str) -> bool,
) -> Vec<WordItem> {
    let paths = paths.into();
    // 搭配模型开着时词格首选归属 Collocation；`include_lattice_best` 关掉时
    // 词格自己的行不出，但搭配行仍放。
    let literal = (options.include_lattice_best || options.include_collocation_best)
        .then(|| take_sentence_index(&paths, lattice, None, options, &is_listed))
        .flatten();
    let pick = keyboard
        .and_then(|keyboard| take_sentence_index(&paths, keyboard, literal, options, &is_listed));
    let mut rows = [None, None];
    for (index, path) in sentence_path_iter(paths).enumerate() {
        if literal == Some(index) {
            rows[0] = Some(owned_sentence_row(
                typed_pinyin,
                path.into_owned(),
                if options.include_collocation_best {
                    CandidateSource::Collocation
                } else {
                    CandidateSource::Generated
                },
            ));
        } else if pick == Some(index) {
            rows[1] = Some(owned_sentence_row(
                typed_pinyin,
                path.into_owned(),
                CandidateSource::NeuralKeyboard,
            ));
        }
    }
    let mut block = Vec::new();
    for row in rows.into_iter().flatten() {
        if block.is_empty() {
            block = Vec::with_capacity(2);
        }
        block.push(row);
    }
    block
}

/// The count of leading dictionary rows that answer the whole key: canonical key equal to the joined syllables, or covering every syllable (WL:625-641, WL:372-381). A prefix-range row with the same character count (滚球 for typed gun'qi) is not an exact hit and must not pin a correctly pronounced sentence behind it.
pub fn whole_sentence_insert_position(candidates: &[WordItem], syllables: &[String]) -> usize {
    if syllables.is_empty() {
        return 0;
    }
    let n = syllables.len();
    candidates
        .iter()
        .take_while(|item| {
            item.source.is_dictionary()
                && if item.canonical_pinyin.is_empty() {
                    covers_all_syllables(item, n)
                } else {
                    item.canonical_pinyin
                        .split('\'')
                        .eq(syllables.iter().map(|syllable| syllable.as_str()))
                }
        })
        .count()
}

/// WL:372-381.
fn covers_all_syllables(item: &WordItem, syllables: usize) -> bool {
    syllables > 0
        && (item.word.chars().count() == syllables
            || (!item.canonical_pinyin.is_empty()
                && item.canonical_pinyin.split('\'').count() == syllables))
}

#[cfg(test)]
mod tests {
    use super::super::decode::tests::{lookup, syllables, table, Table};
    use super::super::decode::TypoEdge;
    use super::super::ngram::tests::triple_table;
    use super::super::ngram::SENTENCE_START;
    use super::super::personal::tests::record_run;
    use super::super::personal::PersonalNgram;
    use super::*;
    use std::sync::Arc;

    fn merge(
        candidates: &mut Vec<WordItem>,
        text: &str,
        rows: &Table,
        typed: &str,
        options: &LatticeOptions<'_>,
    ) -> Option<TypoSentence> {
        merge_lattice_candidates(
            candidates,
            &syllables(text),
            &mut lookup(rows),
            typed,
            options,
            None,
            &mut [],
            "",
        )
    }

    fn dictionary_row(pinyin: &str, word: &str, weight: i64, canonical: &str) -> WordItem {
        WordItem::new(pinyin, word, weight, CandidateSource::Database, canonical)
    }

    /// test_pinyin.cpp:566-578.
    #[test]
    fn exact_full_key_row_stays_first() {
        let rows = table(&[
            ("gao", &[("高", 12000)]),
            ("tan", &[("碳", 4000)]),
            ("gang", &[("钢", 5000)]),
            ("tan'gang", &[("碳钢", 16000)]),
        ]);
        let mut candidates = vec![dictionary_row("gktjgh", "高碳钢", 50000, "gao'tan'gang")];
        merge(
            &mut candidates,
            "gao'tan'gang",
            &rows,
            "gktjgh",
            &LatticeOptions::default(),
        );
        assert_eq!(candidates[0].word, "高碳钢");
        assert_eq!(candidates[0].source, CandidateSource::Database);
    }

    /// test_pinyin.cpp:580-596.
    #[test]
    fn lattice_goes_ahead_of_a_fallback_row() {
        let rows = table(&[
            ("gao", &[("高", 12000)]),
            ("tan", &[("碳", 4000), ("谈", 9000)]),
            ("gang", &[("钢", 5000), ("刚", 9000)]),
            ("nie", &[("镊", 3000)]),
            ("zi", &[("子", 9000)]),
            ("tan'gang", &[("碳钢", 16000)]),
            ("nie'zi", &[("镊子", 18000)]),
        ]);
        let mut candidates = vec![WordItem::new(
            "gktjghnxzi",
            "高谈刚捏子",
            1,
            CandidateSource::Fallback,
            "",
        )];
        merge(
            &mut candidates,
            "gao'tan'gang'nie'zi",
            &rows,
            "gktjghnxzi",
            &LatticeOptions::default(),
        );
        assert_eq!(candidates[0].word, "高碳钢镊子");
        assert!(candidates.iter().any(|item| item.word == "高谈刚捏子"));
    }

    /// test_pinyin.cpp:614-626.
    #[test]
    fn exact_two_syllable_row_stays_ahead() {
        let rows = table(&[
            ("nie", &[("捏", 8000)]),
            ("zi", &[("子", 9000)]),
            ("nie'zi", &[("镊子", 18000)]),
        ]);
        let mut candidates = vec![dictionary_row("nxzi", "捏子", 12000, "nie'zi")];
        merge(
            &mut candidates,
            "nie'zi",
            &rows,
            "nxzi",
            &LatticeOptions::default(),
        );
        assert_eq!(candidates[0].word, "捏子");
        assert_eq!(candidates[0].source, CandidateSource::Database);
        let generated = candidates
            .iter()
            .find(|item| item.word == "镊子")
            .expect("镊子 is offered");
        assert_eq!(generated.source, CandidateSource::Generated);
        assert_eq!(generated.pinyin, "nxzi");
        assert_eq!(generated.canonical_pinyin, "nie'zi");
        assert!(generated.sentence_association);
        assert_eq!(generated.sentence_words, ["镊子"]);
    }

    /// test_pinyin.cpp:628-639.
    #[test]
    fn prefix_range_row_does_not_outrank_the_sentence() {
        let rows = table(&[("gun", &[("滚", 10000)]), ("qi", &[("起", 9000)])]);
        let mut candidates = vec![dictionary_row("gunqi", "滚球", 30000, "gun'qiu")];
        merge(
            &mut candidates,
            "gun'qi",
            &rows,
            "gunqi",
            &LatticeOptions::default(),
        );
        assert_eq!(candidates[0].word, "滚起");
        assert_eq!(candidates[0].source, CandidateSource::Generated);
        assert_eq!(candidates[1].word, "滚球");
    }

    /// test_pinyin.cpp:668-677.
    #[test]
    fn abbreviated_segments_produce_nothing() {
        let rows = table(&[
            ("g", &[("个", 100)]),
            ("k", &[("可", 100)]),
            ("t", &[("他", 100)]),
            ("g'k't", &[("个可他", 50)]),
        ]);
        let mut candidates = Vec::new();
        merge(
            &mut candidates,
            "g'k't",
            &rows,
            "gkt",
            &LatticeOptions::default(),
        );
        assert!(candidates.is_empty());
    }

    #[test]
    fn one_syllable_produces_nothing() {
        let rows = table(&[("ni", &[("你", 100)])]);
        let mut candidates = Vec::new();
        merge(
            &mut candidates,
            "ni",
            &rows,
            "ni",
            &LatticeOptions::default(),
        );
        assert!(candidates.is_empty());
    }

    #[test]
    fn existing_words_are_not_repeated() {
        let rows = table(&[
            ("ni", &[("你", 9000), ("泥", 100)]),
            ("hao", &[("好", 9000)]),
        ]);
        let mut candidates = vec![WordItem::new(
            "nihao",
            "你好",
            1,
            CandidateSource::CloudSuggestion,
            "",
        )];
        merge(
            &mut candidates,
            "ni'hao",
            &rows,
            "nihao",
            &LatticeOptions::default(),
        );
        assert_eq!(
            candidates[0].word, "泥好",
            "the insert position is 0 and 你好 is already listed"
        );
        assert_eq!(candidates[1].word, "你好");
        assert_eq!(candidates[1].source, CandidateSource::CloudSuggestion);
        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn linear_sentence_dedup_scans_existing_rows_without_allocation() {
        let candidates = vec![dictionary_row("nihao", "你好", 1, "ni'hao")];
        let block = vec![dictionary_row("nihao", "拟好", 1, "ni'hao")];
        let (found, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            sentence_seen_linear(&candidates, &block, "拟好")
        });

        assert!(found);
        assert_eq!(allocations, 0);
    }

    #[test]
    fn whole_sentence_insert_position_compares_syllables_without_allocating() {
        let candidates = [dictionary_row("nihao", "你好", 1, "ni'hao")];
        let syllables = syllables("ni'hao");
        let (position, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            whole_sentence_insert_position(&candidates, &syllables)
        });

        assert_eq!(position, 1);
        assert_eq!(allocations, 0, "整句插入位置拼接了临时读音键");
    }

    #[test]
    fn emit_caps_what_reaches_the_list() {
        let directory = tempfile::tempdir().unwrap();
        let rows = table(&[
            ("shu'ru", &[("输入", 20000)]),
            ("fa", &[("法", 800000), ("发", 900000)]),
        ]);
        let trigram = Arc::new(triple_table(
            directory.path(),
            "triple.bin",
            &[(SENTENCE_START, "输入", "法", 4.0)],
        ));
        let mut options = LatticeOptions {
            trigram: Some(trigram),
            nbest: 6,
            emit: 1,
            ..LatticeOptions::default()
        };
        let mut candidates = Vec::new();
        merge(&mut candidates, "shu'ru'fa", &rows, "shurufa", &options);
        assert_eq!(
            candidates.len(),
            1,
            "emit caps what reaches the candidate list"
        );
        assert_eq!(
            candidates[0].word, "输入法",
            "and what reaches it is the rescored winner"
        );

        options.emit = 0;
        let mut every = Vec::new();
        merge(&mut every, "shu'ru'fa", &rows, "shurufa", &options);
        assert!(every.len() > 1, "emit zero hands back the alternatives");
        assert_eq!(every[0].word, "输入法");
        assert!(every
            .iter()
            .all(|item| item.source == CandidateSource::Generated && item.sentence_association));
    }

    #[test]
    fn weight_is_truncated_log_prob() {
        let rows = table(&[("ni", &[("你", 500_000)]), ("hao", &[("好", 250_000)])]);
        let mut candidates = Vec::new();
        merge(
            &mut candidates,
            "ni'hao",
            &rows,
            "nihao",
            &LatticeOptions::default(),
        );
        let expected = ((0.5f64).ln() + (0.25f64).ln()) * 1000.0;
        assert_eq!(candidates[0].weight, expected as i64);
        assert_eq!(candidates[0].weight, -2079);
    }

    #[test]
    fn lattice_rows_carry_their_words() {
        let rows = table(&[
            ("wo", &[("我", 900000)]),
            ("xiang", &[("想", 450000), ("翔", 100000)]),
            ("qu", &[("去", 400000), ("区", 100000)]),
        ]);
        let mut model = PersonalNgram::default();
        record_run(&mut model, &["我", "想", "区"], 2);
        let options = LatticeOptions {
            personal: Some(&model),
            nbest: 16,
            ..LatticeOptions::default()
        };
        let paths = super::super::decode::decode_sentences(
            &syllables("wo'xiang'qu"),
            &mut lookup(&rows),
            &options,
        );
        let mut candidates = Vec::new();
        merge(
            &mut candidates,
            "wo'xiang'qu",
            &rows,
            "wo'xiang'qu",
            &options,
        );
        assert_eq!(candidates[0].word, paths[0].sentence);
        assert_eq!(candidates[0].sentence_words, paths[0].words);
        assert_eq!(candidates.len(), paths.len());
    }

    #[test]
    fn insert_position_counts_exact_leading_rows() {
        let keys = syllables("ni'hao");
        let rows = vec![
            dictionary_row("nihao", "你好", 10, "ni'hao"),
            WordItem::new("nihao", "拟好", 9, CandidateSource::UserDatabase, ""),
            dictionary_row("nihao", "你好吗", 8, "ni'hao'ma"),
            dictionary_row("nihao", "你", 7, "ni"),
        ];
        assert_eq!(whole_sentence_insert_position(&rows, &keys), 2);
        assert_eq!(whole_sentence_insert_position(&rows, &[]), 0);
        assert_eq!(whole_sentence_insert_position(&[], &keys), 0);
        let online = [WordItem::new(
            "nihao",
            "你好",
            1,
            CandidateSource::CloudSuggestion,
            "ni'hao",
        )];
        assert_eq!(
            whole_sentence_insert_position(&online, &keys),
            0,
            "only dictionary rows hold the seat"
        );
        let no_canonical_two_chars = [dictionary_row("nihao", "你号", 1, "")];
        assert_eq!(
            whole_sentence_insert_position(&no_canonical_two_chars, &keys),
            1
        );
        let no_canonical_three_chars = [dictionary_row("nihao", "你好啊", 1, "")];
        assert_eq!(
            whole_sentence_insert_position(&no_canonical_three_chars, &keys),
            0
        );
    }

    #[test]
    fn covers_all_syllables_reads_the_key_too() {
        let item = dictionary_row("xian", "西安", 1, "xi'an");
        assert!(covers_all_syllables(&item, 2));
        let item = dictionary_row("xian", "先", 1, "xi'an");
        assert!(
            covers_all_syllables(&item, 2),
            "the canonical key has two syllables"
        );
        assert!(!covers_all_syllables(&item, 0));
    }

    fn typo_rows() -> Table {
        table(&[
            ("ta", &[("他", 900000)]),
            ("shi", &[("是", 900000)]),
            ("jian", &[("见", 500000)]),
            ("shi'jian", &[("时间", 30000)]),
        ])
    }

    fn typo_edge() -> TypoEdge {
        TypoEdge {
            start: 1,
            end: 3,
            key: "shi'jian".into(),
            value: "事件".into(),
            weight: 90000,
            penalty: 0.5,
        }
    }

    #[test]
    fn typo_sentence_is_returned_not_merged() {
        let rows = typo_rows();
        let mut seen = Vec::new();
        let mut source = |best: &SentencePath| {
            seen.push(best.sentence.clone());
            vec![typo_edge()]
        };
        let mut candidates = Vec::new();
        let typo = merge_lattice_candidates(
            &mut candidates,
            &syllables("ta'shi'jian"),
            &mut lookup(&rows),
            "tashijian",
            &LatticeOptions::default(),
            Some(&mut source),
            &mut [],
            "",
        )
        .expect("a typo sentence");
        assert_eq!(
            seen,
            ["他时间"],
            "the source receives the literal best path"
        );
        assert_eq!(typo.sentence, "他事件");
        assert!(candidates.iter().all(|item| item.word != "他事件"));
        assert_eq!(candidates[0].word, "他时间");
    }

    #[test]
    fn typo_decode_needs_three_syllables() {
        let rows = table(&[("shi", &[("是", 900000)]), ("jian", &[("见", 500000)])]);
        let mut called = false;
        let mut source = |_: &SentencePath| {
            called = true;
            Vec::new()
        };
        let mut candidates = Vec::new();
        let typo = merge_lattice_candidates(
            &mut candidates,
            &syllables("shi'jian"),
            &mut lookup(&rows),
            "shijian",
            &LatticeOptions::default(),
            Some(&mut source),
            &mut [],
            "",
        );
        assert!(typo.is_none());
        assert!(!called);
        assert_eq!(candidates.len(), 1);
    }

    fn path(sentence: &str, log_prob: f64) -> SentencePath {
        SentencePath {
            sentence: sentence.into(),
            key: "a'b".into(),
            log_prob,
            words: vec![sentence.into()],
            typo_edges: 0,
        }
    }

    fn block(
        lattice: &[SentencePath],
        keyboard: Option<&[SentencePath]>,
        options: &LatticeOptions<'_>,
        listed: &[&str],
    ) -> Vec<(String, CandidateSource)> {
        let mut paths = lattice.to_vec();
        let first_keyboard = paths.len();
        if let Some(keyboard) = keyboard {
            paths.extend_from_slice(keyboard);
        }
        let last_keyboard = paths.len();
        reranked_block(
            paths,
            0..lattice.len(),
            keyboard.map(|_| first_keyboard..last_keyboard),
            options,
            "ab",
            |sentence| listed.contains(&sentence),
        )
        .into_iter()
        .map(|item| {
            assert!(item.sentence_association);
            assert_eq!(item.sentence_words, std::slice::from_ref(&item.word));
            (item.word.into_string(), item.source)
        })
        .collect()
    }

    fn words(rows: &[(String, CandidateSource)]) -> Vec<(&str, CandidateSource)> {
        rows.iter()
            .map(|(word, source)| (word.as_str(), *source))
            .collect()
    }

    use CandidateSource::{Generated, NeuralKeyboard as Key};

    #[test]
    fn neural_duplicate_first_pick_breaks_or_moves_on() {
        let lattice = [path("甲", -1.0), path("乙", -2.0)];
        let keyboard = [path("甲", -1.0), path("乙", -2.0)];
        let options = LatticeOptions::default();
        assert_eq!(
            words(&block(&lattice, Some(&keyboard), &options, &[])),
            [("甲", Generated)]
        );
        let next = LatticeOptions {
            show_next_on_duplicate: true,
            ..LatticeOptions::default()
        };
        assert_eq!(
            words(&block(&lattice, Some(&keyboard), &next, &[])),
            [("甲", Generated), ("乙", Key)]
        );
        // A word already in the candidate list counts as listed too.
        assert_eq!(
            words(&block(&lattice, Some(&keyboard), &options, &["甲"])),
            []
        );
    }

    #[test]
    fn neural_without_lattice_best() {
        let lattice = [path("甲", -1.0), path("乙", -2.0)];
        let keyboard = [path("乙", -2.0), path("甲", -1.0)];
        let options = LatticeOptions {
            include_lattice_best: false,
            ..LatticeOptions::default()
        };
        assert_eq!(
            words(&block(&lattice, Some(&keyboard), &options, &[])),
            [("乙", Key)]
        );
        let keyboard = [path("甲", -1.0), path("乙", -2.0)];
        assert_eq!(
            words(&block(&lattice, Some(&keyboard), &options, &[])),
            [("甲", Key)],
            "without the lattice row the keyboard may pick the lattice best"
        );
        assert_eq!(words(&block(&lattice, None, &options, &[])), []);
    }
}

#[cfg(test)]
#[path = "merge_block_frozen.rs"]
mod block_frozen;

#[cfg(test)]
#[path = "merge_block_buffer_tests.rs"]
mod block_buffer_tests;

#[cfg(test)]
#[path = "merge_borrowed_order_tests.rs"]
mod borrowed_order_tests;

#[cfg(test)]
#[path = "generated_ownership_tests.rs"]
mod generated_ownership_tests;

#[cfg(test)]
#[path = "neural_ownership_tests.rs"]
mod neural_ownership_tests;

#[cfg(test)]
#[path = "empty_source_dedup_tests.rs"]
mod empty_source_dedup_tests;

#[cfg(test)]
#[path = "merge_single_path_tests.rs"]
mod single_path_tests;

#[cfg(test)]
#[path = "merge_collocation_tests.rs"]
mod collocation_tests;

#[cfg(test)]
#[path = "decoded_projection_tests.rs"]
mod decoded_projection_tests;
