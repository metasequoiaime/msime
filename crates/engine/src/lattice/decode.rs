//! Graph construction, beam search and the typo sentence decode (quanpin.md §10.1-§10.5, Google parameters removed).
//!
//! A phrase graph and a Viterbi beam search over dictionary spans, after libpinyin's PinyinLookup2 (the unigram path score is a product of P(word), with a beam per syllable step) and sunpinyin's lattice columns. The unigram normaliser supplies the usual "fewer tokens win" bias.
//!
//! With a bigram table, each transition also earns `bigram_weight * ln(P(next|previous) / P(next))`. Without it every path spelling the same syllables is judged on its words' frequencies alone, which is why 配置于权限 used to beat 配置与权限: 于 is the commoner character and nothing else had an opinion. The term is a bonus rather than a replacement, so an absent pair leaves the path where the unigram score put it.
//!
//! A trigram cannot be searched the same way without carrying two words of history in every beam entry, so it is applied after the search: the n best paths are rescored with what the third word adds over the second, then reordered. That is the reason to decode more paths than are shown.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::Arc;

use super::cxx_sort;
use super::ngram::{NgramTable, SENTENCE_START};
use super::personal::PersonalNgram;
use super::LatticeLookup;
use crate::assets;
use crate::dictionary::DictRow;
use crate::paths::RuntimePaths;

/// A lattice span row: exact key, value, dictionary weight.
pub type LatticeLexeme = DictRow;

/// The knobs of one decode.
#[derive(Clone)]
pub struct LatticeOptions<'a> {
    pub beam: usize,
    /// Paths traced back from the final column. The default is 5; the product searches 6 (`make_sentence_lattice_options`) and 12 with neural rerankers.
    pub nbest: usize,
    /// Cap on rows per span. The DB lookup already applies the same cap.
    pub span_limit: usize,
    pub max_phrase_syllables: usize,
    /// Heuristic unigram normaliser against a phrase-length bonus: single-character msime-pinyin.db weights are corpus counts, phrase weights are on a smaller scale.
    pub unigram_z: f64,
    /// The bonus was 3.0, which was never measured against the eval sets: a plausible number for a term whose only job was to stop the decoder spelling a sentence out character by character. Swept through the client's convert_eval with the n-gram tables present, whole-sentence top-1 rises monotonically up to 20 and stops moving after it: sentences-v1 0.850 -> 0.900, sentences-v2 0.125 -> 0.189, quanpin-words-v1 unchanged at 0.768. Raising it further trades sentences-v1 away for nothing.
    pub phrase_length_bonus: f64,
    /// How much each context term may move a path (tests/src/eval_sentences.cpp and the sweep above): at bonus 20 the bigram term is worth doubling, sentences-v2 top-1 0.173 -> 0.189 and MRR 0.393 -> 0.401 with sentences-v1 unchanged, while 3.0 starts trading page coverage for it.
    pub bigram_weight: f64,
    /// The trigram weight moved nothing at any value tried, which is what a term that only fires on a third word of history looks like on sets this short; it stays at 1.0 rather than being tuned to a number the measurement cannot support.
    pub trigram_weight: f64,
    /// A transition v -> w earns `personal_weight * min(personal_max_bonus, ln((1 - mu) + mu * P_personal / P_static))`, where mu is the model's confidence in v (0 for a context never seen, capped at 0.5) and P_static stands for the static conditional probability, `personal_reference_probability * exp(bigram bonus)`. That is the log of blending the two probabilities, expressed as an increment over the static score the path already carries, so a context the user never typed after contributes nothing and one typed often shifts paths by a bounded amount. Within a composition the first word's context is the sentence start; words committed before it do not enter the lattice.
    ///
    /// Calibrated with test_personal_context_input_session.cpp against a 1.5-nat static gap between two homophones: one accepted sentence (every word counted once) must not flip it, one explicit pick (counted twice) must, and a trigram seen after one earlier word must pick a different continuation than after another. With the reference probability at 0.05 the first two cases move a path by 1.20 and 1.76 nats, and the most any transition can gain, at mu = 0.5, is about 3 nats. The cap only binds where the static tables call a pair unlikely, which is exactly where a single mistaken pick would otherwise outweigh them.
    pub personal_weight: f64,
    pub personal_reference_probability: f64,
    pub personal_max_bonus: f64,
    /// Paths to emit; 0 emits all (`sentence_alternatives`).
    pub emit: usize,
    /// How far, per typo edge, the corrected sentence has to beat the literal one before it takes the leading seat. Infinite keeps it behind the whole-sentence rows, where it only adds a candidate. Swept with eval_sentences --autocorrect-types 7: margins of 8 and 4 moved clean top-1 at every base cost from 8 to 28 (sentences-v2 46.5% -> 45.2%, sentences-neutral-v1 19.5% -> 18.4..18.9%).
    pub typo_lead_margin: f64,
    /// Beam of the typo pass, which only needs its best path. At 8 and at 4 the typo and clean eval numbers were the same as at the full beam, and the pass cost dropped from about 0.45 ms to 0.1-0.2 ms per query.
    pub typo_beam: usize,
    /// Shared by every session and outliving them.
    pub bigram: Option<Arc<NgramTable>>,
    pub trigram: Option<Arc<NgramTable>>,
    /// Borrowed while the caller holds the owning store's read lock. `None`, or a model with nothing recorded, scores every path exactly as without it.
    pub personal: Option<&'a PersonalNgram>,
    /// 二元分的备忘，只影响速度不影响结果；`None` 时每次直接查表。
    pub bigram_memo: Option<&'a RefCell<BigramMemo>>,
    /// With neural rerankers: whether the unreranked best still gets a Generated row.
    pub include_lattice_best: bool,
    /// With neural rerankers: take a source's next distinct path when its first pick is already listed.
    pub show_next_on_duplicate: bool,
}

/// 产品解码用的 `phrase_length_bonus`。词组边每覆盖一个音节就加这么多，所以整句分只在同一串音节里可比；九宫格要比较不同音节串解出的整句，得先把它扣掉，见 `nine_key::comparable_weight`。
pub const PHRASE_LENGTH_BONUS: f64 = 20.0;

impl Default for LatticeOptions<'_> {
    fn default() -> Self {
        Self {
            beam: 32,
            nbest: 5,
            span_limit: 32,
            max_phrase_syllables: 7,
            unigram_z: 1e6,
            phrase_length_bonus: PHRASE_LENGTH_BONUS,
            bigram_weight: 2.0,
            trigram_weight: 1.0,
            personal_weight: 1.0,
            personal_reference_probability: 0.05,
            personal_max_bonus: 2.5,
            emit: 0,
            typo_lead_margin: f64::INFINITY,
            typo_beam: 8,
            bigram: None,
            trigram: None,
            personal: None,
            bigram_memo: None,
            include_lattice_best: true,
            show_next_on_duplicate: false,
        }
    }
}

/// The single configuration point quanpin and shuangpin share (WL:643-653): `nbest = 6`, `emit = 0` with alternatives else 1, and the generation's n-gram tables. The two paths used to build this block each themselves, which is how a ranking fix reached one and missed the other for weeks (engine PR #156).
pub fn make_sentence_lattice_options<'a>(
    paths: &RuntimePaths,
    sentence_alternatives: bool,
) -> LatticeOptions<'a> {
    LatticeOptions {
        // Six are searched and rescored either way; `emit` only decides how many of them are handed back.
        nbest: 6,
        emit: if sentence_alternatives { 0 } else { 1 },
        bigram: NgramTable::shared(&paths.dictionary(assets::BIGRAM_TABLE)),
        trigram: NgramTable::shared(&paths.dictionary(assets::TRIGRAM_TABLE)),
        ..LatticeOptions::default()
    }
}

/// 备忘最多存的二元分条数，满了整个清空重来，约 0.5 MB。
const BIGRAM_MEMO_CAPACITY: usize = 1 << 15;

/// 二元分的备忘。九键一次刷新要对几十条切分各解一次词网格，它们的前后词对大量重复，而每次查表要在 12 MB 的映射里跳几次。值就是表里查到的分数（查不到为 0），只要表不变就永远正确；表换了（按 `Arc` 的地址判断）就清空。
#[derive(Default)]
pub struct BigramMemo {
    table: usize,
    scores: HashMap<u64, f32, BuildHasherDefault<KeyHasher>>,
}

impl BigramMemo {
    fn score(&mut self, table: &Arc<NgramTable>, previous: &str, next: &str) -> f32 {
        let Some(key) = table.bigram_key(previous, next) else {
            return 0.0;
        };
        let address = Arc::as_ptr(table) as usize;
        if self.table != address {
            self.scores.clear();
            self.table = address;
        }
        if let Some(&score) = self.scores.get(&key) {
            return score;
        }
        if self.scores.len() >= BIGRAM_MEMO_CAPACITY {
            self.scores.clear();
        }
        let score = table.score(key);
        self.scores.insert(key, score);
        score
    }
}

/// 备忘的键本身就是 FNV-1a 散列，直接拿来当散列值，不再过一遍 SipHash。
#[derive(Default)]
pub struct KeyHasher(u64);

impl Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0 = bytes.iter().fold(self.0, |state, &byte| {
            (state ^ u64::from(byte)).wrapping_mul(0x0100_0000_01B3)
        });
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = value;
    }
}

/// One decoded reading.
#[derive(Debug, Clone, PartialEq)]
pub struct SentencePath {
    pub sentence: String,
    /// The syllable keys of the words, joined with `'`.
    pub key: String,
    pub log_prob: f64,
    pub words: Vec<String>,
    /// Words on this path that came from a typo edge rather than from the typed syllables.
    pub typo_edges: usize,
}

/// A typo-substituted span the typo decode may use in place of the literal syllables.
#[derive(Debug, Clone, PartialEq)]
pub struct TypoEdge {
    pub start: usize,
    pub end: usize,
    /// The word's own reading, so a path through it carries the corrected pronunciation.
    pub key: String,
    pub value: String,
    pub weight: i64,
    /// Subtracted from the edge's usual log probability.
    pub penalty: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypoSentence {
    pub sentence: String,
    /// The corrected reading.
    pub key: String,
    pub score: f64,
    pub words: Vec<String>,
    pub edges: usize,
    pub literal_score: f64,
}

impl TypoSentence {
    /// `score > literal + margin * edges`; with the default infinite margin never, so the typo sentence lands right after the sentence block.
    pub fn leads(&self, margin: f64) -> bool {
        self.edges > 0 && self.score > self.literal_score + margin * self.edges as f64
    }
}

/// WL:56-68. Single-character rows in msime-pinyin.db are raw corpus counts (often 1e6+) while multi-syllable rows are phrase weights on a much smaller scale; libpinyin stores comparable log probabilities, approximated here by down-projecting unigrams and giving dictionary phrases a length bonus.
pub fn edge_log_prob(weight: i64, syllables: usize, options: &LatticeOptions<'_>) -> f64 {
    let weight = if weight > 0 { weight as f64 } else { 1.0 };
    let z = if options.unigram_z > 1.0 {
        options.unigram_z
    } else {
        1e6
    };
    if syllables <= 1 {
        weight.ln() - z.ln()
    } else {
        weight.ln() + options.phrase_length_bonus * syllables as f64
    }
}

/// Build the span graph over `syllables` (each span looked up once) and beam-search it, then rescore with the trigram table (WL:110-186, WL:385-485). The product decodes through `merge_lattice_candidates`, which keeps the graph for the typo pass; this is the C++ `decode_word_lattice` the reference tests drive.
#[cfg(test)]
pub fn decode_sentences(
    syllables: &[String],
    lookup: &mut LatticeLookup<'_>,
    options: &LatticeOptions<'_>,
) -> Vec<SentencePath> {
    if syllables.is_empty() {
        return Vec::new();
    }
    decode_graph(&build_graph(syllables, lookup, options), options, None)
}

/// Decode again with the typo edges added at `typo_beam`, and accept the best path only if it uses a typo edge, differs from the literal best and outscores it (WL:490-525). The product reaches `decode_typo_on_graph` through `merge_lattice_candidates`.
#[cfg(test)]
pub fn decode_typo_sentence(
    syllables: &[String],
    lookup: &mut LatticeLookup<'_>,
    options: &LatticeOptions<'_>,
    literal_best: &SentencePath,
    edges: &[TypoEdge],
) -> Option<TypoSentence> {
    if syllables.is_empty() || edges.is_empty() {
        return None;
    }
    decode_typo_on_graph(
        &build_graph(syllables, lookup, options),
        options,
        literal_best,
        edges,
    )
}

pub(super) struct Edge {
    end: usize,
    word: String,
    key: String,
    log_prob: f64,
    typo: bool,
}

/// Edges by start column.
pub(super) type Graph = Vec<Vec<Edge>>;

/// WL:110-148.
pub(super) fn build_graph(
    syllables: &[String],
    lookup: &mut LatticeLookup<'_>,
    options: &LatticeOptions<'_>,
) -> Graph {
    let n = syllables.len();
    let max_len = options.max_phrase_syllables.max(1);
    let mut graph: Graph = (0..n).map(|_| Vec::new()).collect();
    // A key repeated in the input (ma'ma'ma) is looked up once.
    let span_capacity = (0..n)
        .map(|start| n.min(start.saturating_add(max_len)) - start)
        .sum();
    let mut span_cache: HashMap<&[String], Vec<LatticeLexeme>> =
        HashMap::with_capacity(span_capacity);
    for (start, edges) in graph.iter_mut().enumerate() {
        for end in start + 1..=n.min(start + max_len) {
            let span = &syllables[start..end];
            let rows = span_cache.entry(span).or_insert_with(|| lookup(span));
            edges.reserve(rows.len().min(options.span_limit));
            let mut span_key = None;
            for row in rows.iter().take(options.span_limit) {
                if row.value.is_empty() {
                    continue;
                }
                edges.push(Edge {
                    end,
                    word: row.value.clone(),
                    key: if row.key.is_empty() {
                        span_key.get_or_insert_with(|| span.join("'")).clone()
                    } else {
                        row.key.clone()
                    },
                    log_prob: edge_log_prob(row.weight, end - start, options),
                    typo: false,
                });
            }
        }
    }
    graph
}

#[derive(Clone, Copy)]
struct Hyp<'g> {
    score: f64,
    /// Column and index of the hypothesis this one extends; `None` for the start.
    prev: Option<(usize, usize)>,
    /// The edge it arrived on; `None` only for the start.
    edge: Option<&'g Edge>,
    typo_edges: usize,
}

/// The C++ comparator of both beam sorts (WL:106, WL:447).
fn scores_higher(a: &Hyp<'_>, b: &Hyp<'_>) -> bool {
    a.score > b.score
}

/// Keeps the best `beam` hypotheses with libc++'s `partial_sort`, whose heap selection decides which of several tied hypotheses survive and in what order (WL:101-108).
fn keep_beam(column: &mut Vec<Hyp<'_>>, beam: usize) {
    if column.len() <= beam {
        return;
    }
    cxx_sort::partial_sort(column, beam, &mut scores_higher);
    column.truncate(beam);
}

/// The maximum number of hypotheses a column can receive before it is pruned: one per incoming edge for each surviving hypothesis at its source.
fn column_capacities(
    graph: &Graph,
    extra: Option<&Graph>,
    beam: usize,
    nbest: usize,
) -> Vec<usize> {
    let floor = beam.max(nbest);
    let mut incoming_edges = vec![0usize; graph.len() + 1];
    for edges in graph {
        for edge in edges {
            if edge.end <= graph.len() {
                incoming_edges[edge.end] = incoming_edges[edge.end].saturating_add(1);
            }
        }
    }
    if let Some(extra) = extra {
        for edges in extra {
            for edge in edges {
                if edge.end <= graph.len() {
                    incoming_edges[edge.end] = incoming_edges[edge.end].saturating_add(1);
                }
            }
        }
    }
    incoming_edges
        .into_iter()
        .map(|count| floor.max(count.saturating_mul(beam)))
        .collect()
}

/// Null when the options carry no personal data worth consulting, so the decode keeps its exact prior arithmetic (WL:71-76).
fn active_personal<'a>(options: &LatticeOptions<'a>) -> Option<&'a PersonalNgram> {
    options
        .personal
        .filter(|personal| !personal.is_empty() && options.personal_weight != 0.0)
}

/// WL:80-89. `static_bonus` is the unweighted static bigram increment of the same transition, 0 without a table.
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

/// WL:385-485. `extra` holds more edges per column searched as if they were part of `graph`, so the typo decode adds edges without copying the literal graph.
pub(super) fn decode_graph(
    graph: &Graph,
    options: &LatticeOptions<'_>,
    extra: Option<&Graph>,
) -> Vec<SentencePath> {
    let n = graph.len();
    let personal = active_personal(options);
    let bigram_table = options.bigram.as_ref();
    let bigram = options.bigram.as_deref();
    let mut memo = options.bigram_memo.map(RefCell::borrow_mut);
    let capacities = column_capacities(graph, extra, options.beam, options.nbest);
    let mut columns: Vec<Vec<Hyp<'_>>> = capacities.into_iter().map(Vec::with_capacity).collect();
    columns[0].push(Hyp {
        score: 0.0,
        prev: None,
        edge: None,
        typo_edges: 0,
    });

    // 同一列里很多假设经由同一个词到达，只是更早的历史不同；它们对每条出边查到的二元分完全一样。按前一个词把这一列所有出边的二元分记下来，同一个词只查一遍表。
    let row_capacity = options.beam.max(options.nbest);
    let mut bigram_rows: Vec<(&str, Vec<f32>)> = Vec::with_capacity(row_capacity);
    let mut bigram_row_indices: HashMap<&str, usize> = HashMap::with_capacity(row_capacity);
    // 这一列的出边，按原来的顺序（先图里的，再 `extra` 的）收集一次，每个假设都按这个顺序展开。
    let mut outgoing: Vec<&Edge> = Vec::new();
    for pos in 0..n {
        keep_beam(&mut columns[pos], options.beam);
        let (done, ahead) = columns.split_at_mut(pos + 1);
        let current = &done[pos];
        bigram_rows.clear();
        bigram_row_indices.clear();
        outgoing.clear();
        outgoing.extend(
            graph[pos]
                .iter()
                .chain(extra.into_iter().flat_map(|extra| extra[pos].iter())),
        );
        for (index, hyp) in current.iter().enumerate() {
            // Column 0 has no predecessor, so the start token carries what the corpus knows about how sentences open; every later column uses the word the hypothesis arrived on.
            let previous = hyp.edge.map_or(SENTENCE_START, |edge| edge.word.as_str());
            let bonuses = bigram_table.map(|table| {
                let row = if let Some(&row) = bigram_row_indices.get(previous) {
                    row
                } else {
                    let scores = match memo.as_deref_mut() {
                        Some(memo) => outgoing
                            .iter()
                            .map(|edge| memo.score(table, previous, &edge.word))
                            .collect(),
                        None => outgoing
                            .iter()
                            .map(|edge| table.bigram(previous, &edge.word))
                            .collect(),
                    };
                    bigram_rows.push((previous, scores));
                    let row = bigram_rows.len() - 1;
                    bigram_row_indices.insert(previous, row);
                    row
                };
                &bigram_rows[row].1
            });
            // The column a hypothesis points back into was pruned before this one was expanded and is never touched again, so the word two back is stable here.
            let context = personal.map(|personal| {
                let earlier = match hyp.prev {
                    Some((column, at)) if column > 0 => {
                        done[column][at].edge.map(|edge| edge.word.as_str())
                    }
                    _ => None,
                };
                personal.context(earlier, Some(previous))
            });
            for (at, &edge) in outgoing.iter().enumerate() {
                let mut score = hyp.score + edge.log_prob;
                let static_bonus = bonuses.map_or(0.0, |bonuses| f64::from(bonuses[at]));
                if bigram.is_some() {
                    score += options.bigram_weight * static_bonus;
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
                ahead[edge.end - pos - 1].push(Hyp {
                    score,
                    prev: Some((pos, index)),
                    edge: Some(edge),
                    typo_edges: hyp.typo_edges + usize::from(edge.typo),
                });
            }
        }
    }

    // The loop only prunes columns [0, n), so the terminal column still holds every hypothesis that reached the end. Nothing traces back through it, so truncating it cannot break traceback.
    let mut last = std::mem::take(&mut columns[n]);
    if last.is_empty() {
        return Vec::new();
    }
    keep_beam(&mut last, options.beam.max(options.nbest));
    cxx_sort::sort(&mut last, &mut scores_higher);
    let take = options.nbest.min(last.len());

    let mut paths = Vec::with_capacity(last.len());
    for hyp in &last {
        let mut words = Vec::with_capacity(n);
        let mut keys = Vec::with_capacity(n);
        let mut cursor = Some(hyp);
        while let Some(step) = cursor {
            let Some(edge) = step.edge else { break };
            words.push(edge.word.clone());
            keys.push(edge.key.as_str());
            cursor = step.prev.map(|(column, at)| &columns[column][at]);
        }
        words.reverse();
        keys.reverse();
        let sentence = words.concat();
        if sentence.is_empty() {
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
    retain_unique_sentences(&mut paths, take);
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

#[cfg(test)]
fn contains_sentence(paths: &[SentencePath], sentence: &str) -> bool {
    paths.iter().any(|path| path.sentence == sentence)
}

/// WL:168-186. The beam carries one word of history, so a third word of context cannot be searched without widening every hypothesis into (position, last two words); the survivors are rescored instead. Each entry holds what the third word adds over the second, so summing it onto a path that already carries its bigram score is the whole model, not a second opinion.
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

/// WL:490-525. Typo edges keep the literal edge scale minus their penalty, so the comparison with the literal sentence is on the same terms as everything else in the decode.
pub(super) fn decode_typo_on_graph(
    graph: &Graph,
    options: &LatticeOptions<'_>,
    literal_best: &SentencePath,
    edges: &[TypoEdge],
) -> Option<TypoSentence> {
    let n = graph.len();
    let mut capacities = vec![0usize; n];
    for typo in edges {
        if typo.start < typo.end && typo.end <= n && !typo.value.is_empty() {
            capacities[typo.start] = capacities[typo.start].saturating_add(1);
        }
    }
    let mut extra: Graph = capacities.into_iter().map(Vec::with_capacity).collect();
    let mut any = false;
    for typo in edges {
        if typo.start >= typo.end || typo.end > n || typo.value.is_empty() {
            continue;
        }
        extra[typo.start].push(Edge {
            end: typo.end,
            word: typo.value.clone(),
            key: typo.key.clone(),
            log_prob: edge_log_prob(typo.weight, typo.end - typo.start, options) - typo.penalty,
            typo: true,
        });
        any = true;
    }
    if !any {
        return None;
    }
    let typo_options = LatticeOptions {
        beam: options.typo_beam,
        ..options.clone()
    };
    let paths = decode_graph(graph, &typo_options, Some(&extra));
    let best = paths.into_iter().next()?;
    // The narrower beam can lose the literal best path, so beating it is checked rather than assumed.
    if best.typo_edges == 0
        || best.sentence == literal_best.sentence
        || best.log_prob <= literal_best.log_prob
    {
        return None;
    }
    Some(TypoSentence {
        sentence: best.sentence,
        key: best.key,
        score: best.log_prob,
        words: best.words,
        edges: best.typo_edges,
        literal_score: literal_best.log_prob,
    })
}

#[cfg(test)]
pub(super) mod tests {
    use std::collections::HashSet;

    use super::super::ngram::tests::{pair_table, triple_table};
    use super::super::personal::tests::record_run;
    use super::*;

    pub(crate) type Table = HashMap<String, Vec<LatticeLexeme>>;

    pub(crate) fn row(key: &str, value: &str, weight: i64) -> LatticeLexeme {
        LatticeLexeme {
            key: key.to_owned(),
            value: value.to_owned(),
            weight,
        }
    }

    /// Rows keyed by the `'`-joined span, the C++ `make_table_lattice_lookup`.
    pub(crate) fn table(entries: &[(&str, &[(&str, i64)])]) -> Table {
        entries
            .iter()
            .map(|(key, rows)| {
                (
                    (*key).to_owned(),
                    rows.iter()
                        .map(|(value, weight)| row(key, value, *weight))
                        .collect(),
                )
            })
            .collect()
    }

    pub(crate) fn lookup(table: &Table) -> impl FnMut(&[String]) -> Vec<LatticeLexeme> + '_ {
        move |span: &[String]| table.get(&span.join("'")).cloned().unwrap_or_default()
    }

    pub(crate) fn syllables(text: &str) -> Vec<String> {
        text.split('\'').map(str::to_owned).collect()
    }

    fn decode(text: &str, rows: &Table, options: &LatticeOptions<'_>) -> Vec<SentencePath> {
        decode_sentences(&syllables(text), &mut lookup(rows), options)
    }

    fn best(text: &str, rows: &Table, options: &LatticeOptions<'_>) -> String {
        let paths = decode(text, rows, options);
        assert!(!paths.is_empty(), "the lattice decoded nothing for {text}");
        paths[0].sentence.clone()
    }

    #[test]
    fn sentence_lookup_scans_existing_paths() {
        let paths = vec![SentencePath {
            sentence: "你好".to_owned(),
            key: "ni'hao".to_owned(),
            log_prob: 0.0,
            words: vec!["你".to_owned(), "好".to_owned()],
            typo_edges: 0,
        }];
        assert!(contains_sentence(&paths, "你好"));
        assert!(!contains_sentence(&paths, "泥好"));
    }

    #[test]
    fn edge_log_prob_projects_unigrams_and_rewards_phrases() {
        let options = LatticeOptions::default();
        assert!((edge_log_prob(1_000_000, 1, &options) - 0.0).abs() < 1e-12);
        assert_eq!(edge_log_prob(0, 1, &options), -(1e6f64).ln());
        assert_eq!(
            edge_log_prob(-5, 1, &options),
            edge_log_prob(1, 1, &options)
        );
        assert!((edge_log_prob(100, 2, &options) - (100f64.ln() + 40.0)).abs() < 1e-12);
        let odd = LatticeOptions {
            unigram_z: 0.5,
            ..LatticeOptions::default()
        };
        assert_eq!(edge_log_prob(10, 1, &odd), edge_log_prob(10, 1, &options));
    }

    #[test]
    fn columns_reserve_their_incoming_beam_fanout() {
        let edge = |end| Edge {
            end,
            word: "词".to_owned(),
            key: "ci".to_owned(),
            log_prob: 0.0,
            typo: false,
        };
        let graph = vec![vec![edge(1), edge(2)], vec![edge(2)], vec![]];
        let extra = vec![vec![], vec![edge(2)], vec![]];

        assert_eq!(column_capacities(&graph, Some(&extra), 4, 2), [4, 4, 12, 4]);
    }

    /// test_pinyin.cpp:540-548, like the other fake-lookup lattice cases of `test_word_lattice` (:534-677) ported here and in merge.rs; the SQLite lookup case (:641-666) is in dictionary/pinyin.rs.
    #[test]
    fn nie_zi_prefers_the_phrase() {
        let rows = table(&[
            ("nie", &[("捏", 8000), ("聂", 4000), ("镊", 3000)]),
            ("zi", &[("子", 9000)]),
            ("nie'zi", &[("镊子", 18000), ("孽子", 2000)]),
        ]);
        assert_eq!(best("nie'zi", &rows, &LatticeOptions::default()), "镊子");
    }

    /// test_pinyin.cpp:550-564.
    #[test]
    fn five_syllables_join_the_best_phrases() {
        let rows = table(&[
            ("gao", &[("高", 12000), ("搞", 11000)]),
            ("tan", &[("谈", 9000), ("碳", 4000), ("摊", 3500)]),
            ("gang", &[("刚", 9000), ("钢", 5000), ("岗", 4000)]),
            ("nie", &[("捏", 8000), ("镊", 3000)]),
            ("zi", &[("子", 9000)]),
            ("gao'tan", &[("高谈", 20000)]),
            ("tan'gang", &[("碳钢", 16000)]),
            ("nie'zi", &[("镊子", 18000)]),
        ]);
        let paths = decode("gao'tan'gang'nie'zi", &rows, &LatticeOptions::default());
        assert_eq!(paths[0].sentence, "高碳钢镊子");
        assert_eq!(paths[0].key, "gao'tan'gang'nie'zi");
        assert_eq!(paths[0].words, ["高", "碳钢", "镊子"]);
        assert_eq!(paths[0].typo_edges, 0);
    }

    /// test_pinyin.cpp:598-612.
    #[test]
    fn hen_la_ji_beats_the_rarer_homophone() {
        let rows = table(&[
            ("xing", &[("性", 8000)]),
            ("neng", &[("能", 8000)]),
            ("hen", &[("很", 20000), ("狠", 3000)]),
            ("la", &[("拉", 5000)]),
            ("ji", &[("圾", 4000)]),
            ("xing'neng", &[("性能", 15000)]),
            ("la'ji", &[("垃圾", 14000)]),
        ]);
        let sentence = best("xing'neng'hen'la'ji", &rows, &LatticeOptions::default());
        assert!(
            sentence.contains("很垃圾") && !sentence.contains("狠垃圾"),
            "{sentence}"
        );
    }

    #[test]
    fn paths_are_distinct_sorted_and_capped() {
        let rows = table(&[
            ("shu", &[("书", 9000), ("输", 8000), ("数", 7000)]),
            ("ru", &[("入", 9000), ("如", 8000)]),
            ("shu'ru", &[("输入", 20000)]),
        ]);
        let options = LatticeOptions {
            nbest: 3,
            ..LatticeOptions::default()
        };
        let paths = decode("shu'ru", &rows, &options);
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0].sentence, "输入");
        assert!(paths
            .windows(2)
            .all(|pair| pair[0].log_prob >= pair[1].log_prob));
        let sentences: HashSet<_> = paths.iter().map(|path| path.sentence.as_str()).collect();
        assert_eq!(sentences.len(), paths.len());
        assert!(decode_sentences(&[], &mut lookup(&rows), &options).is_empty());
        assert!(
            decode("shu'xx", &rows, &options).is_empty(),
            "an uncovered syllable leaves no path"
        );
    }

    #[test]
    fn same_sentence_from_two_segmentations_is_listed_once() {
        let rows = table(&[("ma", &[("马", 9000)]), ("ma'ma", &[("马马", 100)])]);
        let paths = decode("ma'ma", &rows, &LatticeOptions::default());
        assert_eq!(paths.len(), 1);
        assert_eq!(
            paths[0].words,
            ["马马"],
            "the higher-scoring spelling survives"
        );
    }

    #[test]
    fn sentence_path_dedup_uses_no_temporary_heap_state_for_small_beams() {
        let mut paths = (0..32)
            .map(|index| SentencePath {
                sentence: format!("句{}", index % 16),
                key: "a".to_owned(),
                log_prob: index as f64,
                words: vec!["句".to_owned()],
                typo_edges: 0,
            })
            .collect::<Vec<_>>();

        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            retain_unique_sentences(&mut paths, 12);
        });

        assert_eq!(allocations, 0);
        assert_eq!(paths.len(), 12);
    }

    #[test]
    fn repeated_span_cache_keys_do_not_allocate_joined_strings() {
        let syllables = vec!["a".to_owned(); 8];
        let options = LatticeOptions {
            max_phrase_syllables: 3,
            ..LatticeOptions::default()
        };
        let (graph, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            let mut lookup = |_: &[String]| Vec::new();
            build_graph(&syllables, &mut lookup, &options)
        });

        assert!(graph.iter().all(Vec::is_empty));
        assert!(
            allocations <= 8,
            "span cache should avoid joined key allocations: {allocations}"
        );
    }

    #[test]
    fn span_limit_and_empty_values_are_respected() {
        let rows = table(&[("a", &[("", 900_000), ("甲", 10), ("乙", 1_000_000)])]);
        let options = LatticeOptions {
            span_limit: 2,
            ..LatticeOptions::default()
        };
        let paths = decode("a", &rows, &options);
        assert_eq!(paths.len(), 1, "the third row is past the span limit");
        assert_eq!(paths[0].sentence, "甲");
        let mut calls = 0;
        let mut counting = |span: &[String]| {
            calls += 1;
            rows.get(&span.join("'")).cloned().unwrap_or_default()
        };
        decode_sentences(&syllables("a'a'a"), &mut counting, &options);
        assert_eq!(calls, 3, "a, a'a and a'a'a are each looked up once");
    }

    #[test]
    fn row_key_falls_back_to_the_span_key() {
        let mut rows = table(&[("ni", &[("你", 9000)]), ("hao", &[("好", 9000)])]);
        rows.insert("ni'hao".into(), vec![row("", "你好", 30000)]);
        let paths = decode("ni'hao", &rows, &LatticeOptions::default());
        assert_eq!(paths[0].sentence, "你好");
        assert_eq!(paths[0].key, "ni'hao");
    }

    #[test]
    fn bigram_flips_the_transition() {
        let directory = tempfile::tempdir().unwrap();
        let rows = table(&[
            ("pei'zhi", &[("配置", 20000)]),
            ("yu", &[("于", 900000), ("与", 300000)]),
            ("quan'xian", &[("权限", 18000)]),
        ]);
        let text = "pei'zhi'yu'quan'xian";
        assert_eq!(best(text, &rows, &LatticeOptions::default()), "配置于权限");

        let bigram = Arc::new(pair_table(
            directory.path(),
            "transition.bin",
            &[("配置", "与", 3.0)],
        ));
        let mut options = LatticeOptions {
            bigram: Some(bigram),
            ..LatticeOptions::default()
        };
        let with = decode(text, &rows, &options);
        assert_eq!(
            with[0].sentence, "配置与权限",
            "one known pair flips the path"
        );
        // A pair the corpus never saw is not penalised into last place, which is what makes the table safe to ship against a partial corpus.
        assert!(with.len() > 1, "the alternative path is still generated");

        // The weight has to be able to turn the term off again.
        options.bigram_weight = 0.0;
        assert_eq!(best(text, &rows, &options), "配置于权限");
    }

    #[test]
    fn trigram_reorders_the_finished_paths() {
        let directory = tempfile::tempdir().unwrap();
        let rows = table(&[
            ("shu'ru", &[("输入", 20000)]),
            ("fa", &[("法", 800000), ("发", 900000)]),
        ]);
        let text = "shu'ru'fa";
        let plain = decode(text, &rows, &LatticeOptions::default());
        assert!(plain.len() >= 2);
        assert_eq!(plain[0].sentence, "输入发");

        let trigram = Arc::new(triple_table(
            directory.path(),
            "triple.bin",
            &[(SENTENCE_START, "输入", "法", 4.0)],
        ));
        let mut options = LatticeOptions {
            trigram: Some(trigram),
            ..LatticeOptions::default()
        };
        assert_eq!(best(text, &rows, &options), "输入法");
        options.trigram_weight = 0.0;
        assert_eq!(
            best(text, &rows, &options),
            "输入发",
            "weight zero leaves the search order alone"
        );
    }

    #[test]
    fn sentence_options_search_six() {
        let paths = RuntimePaths::default();
        let shown = make_sentence_lattice_options(&paths, false);
        assert_eq!(
            (shown.nbest, shown.emit),
            (6, 1),
            "the default searches six and shows one"
        );
        let all = make_sentence_lattice_options(&paths, true);
        assert_eq!(
            (all.nbest, all.emit),
            (6, 0),
            "alternatives search the same six and show them"
        );
        assert!(all.bigram.is_none() && all.trigram.is_none());
        assert_eq!(all.beam, 32);
        assert!(all.include_lattice_best && !all.show_next_on_duplicate);
    }

    #[test]
    fn sentence_options_load_the_generation_tables() {
        let directory = tempfile::tempdir().unwrap();
        pair_table(
            directory.path(),
            assets::BIGRAM_TABLE,
            &[("配置", "与", 3.0)],
        );
        let paths = RuntimePaths {
            dictionaries: directory.path().to_path_buf(),
            ..RuntimePaths::default()
        };
        let options = make_sentence_lattice_options(&paths, true);
        assert_eq!(options.bigram.expect("bigram loads").len(), 1);
        assert!(options.trigram.is_none());
    }

    fn personal_lexicon() -> Table {
        // 想 leads 翔 by ln(4.5) = 1.5 nats of static evidence.
        table(&[
            ("wo", &[("我", 900000)]),
            ("ni", &[("你", 900000)]),
            ("xiang", &[("想", 450000), ("翔", 100000)]),
            ("qu", &[("去", 400000), ("区", 100000)]),
        ])
    }

    fn with_personal(personal: &PersonalNgram) -> LatticeOptions<'_> {
        LatticeOptions {
            personal: Some(personal),
            ..LatticeOptions::default()
        }
    }

    #[test]
    fn empty_personal_model_changes_nothing() {
        let rows = personal_lexicon();
        let empty = PersonalNgram::default();
        let plain = decode("wo'xiang'qu", &rows, &LatticeOptions::default());
        let with_empty = decode("wo'xiang'qu", &rows, &with_personal(&empty));
        assert_eq!(plain, with_empty);
        assert_eq!(plain[0].sentence, "我想去");
    }

    #[test]
    fn one_pick_flips_a_path_one_acceptance_does_not() {
        let rows = personal_lexicon();
        let mut accepted = PersonalNgram::default();
        record_run(&mut accepted, &["我", "翔"], 1);
        assert_eq!(best("wo'xiang", &rows, &with_personal(&accepted)), "我想");
        let mut picked = PersonalNgram::default();
        record_run(&mut picked, &["我", "翔"], 2);
        assert_eq!(best("wo'xiang", &rows, &with_personal(&picked)), "我翔");

        let silenced = LatticeOptions {
            personal_weight: 0.0,
            ..with_personal(&picked)
        };
        assert_eq!(best("wo'xiang", &rows, &silenced), "我想");
    }

    #[test]
    fn personal_trigram_tells_contexts_apart() {
        // After 我想 the user picked 区, after 你想 去. The bigram 想 -> 区/去 is a tie; only the trigram can tell them apart, and it has to beat 去's static lead to do it.
        let rows = personal_lexicon();
        let mut model = PersonalNgram::default();
        for _ in 0..4 {
            record_run(&mut model, &["我", "想", "区"], 1);
            record_run(&mut model, &["你", "想", "去"], 1);
        }
        assert_eq!(best("wo'xiang'qu", &rows, &with_personal(&model)), "我想区");
        assert_eq!(best("ni'xiang'qu", &rows, &with_personal(&model)), "你想去");
    }

    #[test]
    fn typo_sentence_needs_a_typo_edge_and_a_better_score() {
        let rows = table(&[
            ("shi", &[("是", 900000)]),
            ("jina", &[]),
            ("jian", &[("见", 500000)]),
            ("shi'jian", &[("时间", 30000)]),
            ("ta", &[("他", 900000)]),
        ]);
        let text = "ta'shi'jian";
        let mut rows_lookup = lookup(&rows);
        let options = LatticeOptions::default();
        let literal = decode_sentences(&syllables(text), &mut rows_lookup, &options);
        assert_eq!(literal[0].sentence, "他时间");

        let edge = |value: &str, weight: i64, penalty: f64| TypoEdge {
            start: 1,
            end: 3,
            key: "shi'jian".into(),
            value: value.into(),
            weight,
            penalty,
        };
        // A typo phrase worth more than the literal one wins and carries its own key.
        let typo = decode_typo_sentence(
            &syllables(text),
            &mut rows_lookup,
            &options,
            &literal[0],
            &[edge("事件", 90000, 0.5)],
        )
        .expect("the typo sentence outscores the literal one");
        assert_eq!(typo.sentence, "他事件");
        assert_eq!(typo.key, "ta'shi'jian");
        assert_eq!(typo.words, ["他", "事件"]);
        assert_eq!(typo.edges, 1);
        assert_eq!(typo.literal_score, literal[0].log_prob);
        assert!(typo.score > typo.literal_score);
        assert!(!typo.leads(f64::INFINITY));
        assert!(typo.leads(0.0));
        assert!(!typo.leads(typo.score - typo.literal_score));

        // Too expensive: the literal path stays best, so there is no typo sentence.
        assert!(decode_typo_sentence(
            &syllables(text),
            &mut rows_lookup,
            &options,
            &literal[0],
            &[edge("事件", 90000, 40.0)],
        )
        .is_none());
        // The same sentence as the literal best is not a correction.
        assert!(decode_typo_sentence(
            &syllables(text),
            &mut rows_lookup,
            &options,
            &literal[0],
            &[edge("时间", 900000, 0.0)],
        )
        .is_none());
        // Edges outside the input or without a value are ignored.
        let invalid = [
            TypoEdge {
                start: 2,
                end: 4,
                ..edge("事件", 90000, 0.0)
            },
            TypoEdge {
                start: 2,
                end: 2,
                ..edge("事件", 90000, 0.0)
            },
            edge("", 90000, 0.0),
        ];
        assert!(decode_typo_sentence(
            &syllables(text),
            &mut rows_lookup,
            &options,
            &literal[0],
            &invalid
        )
        .is_none());
        assert!(decode_typo_sentence(
            &syllables(text),
            &mut rows_lookup,
            &options,
            &literal[0],
            &[]
        )
        .is_none());
    }

    #[test]
    fn typo_leads_ignores_zero_edges() {
        let typo = TypoSentence {
            sentence: "甲".into(),
            key: "jia".into(),
            score: 10.0,
            words: vec!["甲".into()],
            edges: 0,
            literal_score: 0.0,
        };
        assert!(!typo.leads(0.0));
    }
}
