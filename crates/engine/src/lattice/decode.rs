//! Graph construction, beam search and the typo sentence decode (quanpin.md §10.1-§10.5, Google parameters removed).
//!
//! A phrase graph and a Viterbi beam search over dictionary spans, after libpinyin's PinyinLookup2 (the unigram path score is a product of P(word), with a beam per syllable step) and sunpinyin's lattice columns. The unigram normaliser supplies the usual "fewer tokens win" bias.
//!
//! With a bigram table, each transition also earns `bigram_weight * max(0, ln(P(next|previous) / P(next)))`. Without it every path spelling the same syllables is judged on its words' frequencies alone, which is why 配置于权限 used to beat 配置与权限: 于 is the commoner character and nothing else had an opinion. The term is a bonus rather than a replacement: an absent pair leaves the path where the unigram score put it, and so does a pair the table scores below zero, since the table keeps only pairs seen often enough and an absent one is no evidence of being likelier than an observed, unlikely one. On sentences-neutral-v1 dropping the negative side raised top-1 0.608 -> 0.617 and top-5 0.833 -> 0.847 with the reranker, 0.480 -> 0.490 and 0.789 -> 0.803 without it (#7583).
//!
//! A trigram cannot be searched the same way without carrying two words of history in every beam entry, so it is applied after the search: the n best paths are rescored with what the third word adds over the second, then reordered. That is the reason to decode more paths than are shown.

use std::cell::RefCell;
use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::Arc;

#[cfg(test)]
use std::cell::Cell;

use super::cxx_sort;
use super::ngram::{NgramTable, SENTENCE_START};
use super::personal::PersonalNgram;
use super::LatticeLookup;
use crate::assets;
use crate::dictionary::DictRow;
use crate::paths::RuntimePaths;

/// A lattice span row: exact key, value, dictionary weight.
pub type LatticeLexeme = DictRow;

/// 字级搭配打分函数的签名：`scorer(上下文尾窗, 候选词, 是否句尾词) -> 主分同域的搭配分`。
pub type CollocationScorer<'a> = dyn Fn(&str, &str, bool) -> f64 + 'a;

/// The knobs of one decode.
#[derive(Clone)]
pub struct LatticeOptions<'a> {
    pub beam: usize,
    /// 从末列回溯的路径数。默认5条；产品的 `SentenceNgramTables::options` 搜索6条，神经重排启用时搜索12条。
    pub nbest: usize,
    /// Cap on rows per span. The DB lookup already applies the same cap.
    pub span_limit: usize,
    pub max_phrase_syllables: usize,
    /// Heuristic unigram normaliser against a phrase-length bonus: single-character msime-pinyin.db weights are corpus counts, phrase weights are on a smaller scale.
    pub unigram_z: f64,
    /// 多字词每音节奖励，并扣一份作为每词插入代价。默认值 20 沿用原校准：旧评分从 3 提到 20 时，句子首选率上升至平台期，词级结果不变；继续增大会损失手写句子集。
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
    /// 字级搭配打分（octagram `.gram`）：入参是路径已累计文本的尾窗、候选词、
    /// 是否句尾词，返回解码主分同域的搭配分；`None`（默认）= 关闭。
    /// 装配方把 `octagram::GramDb::query` 的自然对数域结果乘 `LOG10_E` 换算。
    pub collocation_scorer: Option<&'a CollocationScorer<'a>>,
    /// 搭配项的标定系数：scorer 存在且它大于 0 时，逐边叠加 `weight * scorer(...)` 到路径分。
    pub collocation_weight: f64,
    /// 语法模型激活时把解码最优路径以 `CandidateSource::Collocation` 行展示；
    /// 与 `include_lattice_best` 同开且指向同一句时归属 Collocation。
    pub include_collocation_best: bool,
}

/// 多字词每音节奖励，并扣一份作为每词插入代价。九宫格跨切分比较再扣音节奖励，保留已经计入的每词代价，见 `nine_key::comparable_weight`。
pub const PHRASE_LENGTH_BONUS: f64 = 20.0;

/// 句末语气词：整句最后一条边是这些单字之一、前面还有别的词时，加 `SENTENCE_FINAL_PARTICLE_BONUS`。n-gram 表只有句首记号、没有句尾，又是从维基百科统计的，几乎见不到口语句尾，「债券吗」「标注呀」只能按单字词频和同码或同音的「那」「亚」比，十有八九输掉。
pub const SENTENCE_FINAL_PARTICLES: &[char] = &[
    '啊', '呀', '吧', '呢', '吗', '啦', '嘛', '哦', '哇', '呗', '咯', '喽',
];

/// 句末语气词的加分（自然对数）。在 `resources/eval/sentences-particles-v1.tsv` 的一半上从 2 到 5 扫过，3 是现有评测集一项都不退步时最大的值：再大，九键长句集里原本找得到的金标准会被挤出列表。
pub const SENTENCE_FINAL_PARTICLE_BONUS: f64 = 3.0;

/// 叠字叹词（啊啊、呃呃）不是词组，不拿多音节的词组奖励，按单字口径计分。否则九宫格上 `22` 的「啊啊」凭奖励压过「吧」，「相关吧」变成「相关啊啊」。
const REDUPLICATED_INTERJECTIONS: &[char] = &[
    '啊', '呃', '哦', '嗯', '呀', '哇', '噢', '唉', '哎', '喔', '诶', '额', '谔',
];

fn is_sentence_final_particle(word: &str) -> bool {
    let mut chars = word.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if SENTENCE_FINAL_PARTICLES.contains(&c))
}

/// 计词组奖励时这个词算几个音节：叠字叹词只算一个。
fn bonus_syllables(word: &str, syllables: usize) -> usize {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return syllables;
    };
    let reduplicated =
        syllables > 1 && REDUPLICATED_INTERJECTIONS.contains(&first) && chars.all(|c| c == first);
    if reduplicated {
        1
    } else {
        syllables
    }
}

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
            collocation_scorer: None,
            collocation_weight: 0.0,
            include_collocation_best: false,
        }
    }
}

/// 活动词典持有构造时预热的 n 元表，按键查询不再构造路径或访问进程共享缓存。
#[derive(Default)]
pub(crate) struct SentenceNgramTables {
    bigram: Option<Arc<NgramTable>>,
    trigram: Option<Arc<NgramTable>>,
}

impl SentenceNgramTables {
    pub(crate) fn load(paths: &RuntimePaths) -> Self {
        Self {
            bigram: NgramTable::shared(&paths.dictionary(assets::BIGRAM_TABLE)),
            trigram: NgramTable::shared(&paths.dictionary(assets::TRIGRAM_TABLE)),
        }
    }

    pub(crate) fn options<'a>(&self, sentence_alternatives: bool) -> LatticeOptions<'a> {
        LatticeOptions {
            nbest: 6,
            emit: if sentence_alternatives { 0 } else { 1 },
            bigram: self.bigram.clone(),
            trigram: self.trigram.clone(),
            ..LatticeOptions::default()
        }
    }
}

/// 固定 `87593cb4791e16297798f3542f817aa80da8be48` 的旧路径型配置入口，仅移除注释供测试对照。
#[cfg(test)]
pub fn make_sentence_lattice_options<'a>(
    paths: &RuntimePaths,
    sentence_alternatives: bool,
) -> LatticeOptions<'a> {
    LatticeOptions {
        nbest: 6,
        emit: if sentence_alternatives { 0 } else { 1 },
        bigram: NgramTable::shared(&paths.dictionary(assets::BIGRAM_TABLE)),
        trigram: NgramTable::shared(&paths.dictionary(assets::TRIGRAM_TABLE)),
        ..LatticeOptions::default()
    }
}

/// 备忘最多存的二元分条数，满了整个清空重来，约 0.5 MB。
const BIGRAM_MEMO_CAPACITY: usize = 1 << 15;

#[cfg(test)]
thread_local! {
    static BIGRAM_MEMO_HASH_CALLS: Cell<usize> = const { Cell::new(0) };
}

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
        let full = self.scores.len() >= BIGRAM_MEMO_CAPACITY;
        match self.scores.entry(key) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) if full => {
                let key = entry.into_key();
                self.scores.clear();
                let score = table.score(key);
                self.scores.insert(key, score);
                score
            }
            Entry::Vacant(entry) => {
                let score = table.score(key);
                entry.insert(score);
                score
            }
        }
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
        #[cfg(test)]
        BIGRAM_MEMO_HASH_CALLS.with(|calls| calls.set(calls.get() + 1));
        self.0 = bytes.iter().fold(self.0, |state, &byte| {
            (state ^ u64::from(byte)).wrapping_mul(0x0100_0000_01B3)
        });
    }

    fn write_u64(&mut self, value: u64) {
        #[cfg(test)]
        BIGRAM_MEMO_HASH_CALLS.with(|calls| calls.set(calls.get() + 1));
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

/// 单字权重按语料计数归一；多字词按音节奖励，并为每次插入扣一份奖励，避免拆成更多词组额外赚取正的词频分。
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
        weight.ln() + options.phrase_length_bonus * (syllables - 1) as f64
    }
}

/// 建词格、束搜索并做三元重排。调用方负责音节准入，查询行必须携带完整读音。
/// 普通整句合并仍走 `merge_lattice_candidates`，为纠错复用同一张图。
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

/// 按音节位置约束词边后再解码，保证低频的辅助码匹配字不会在束搜索前被淘汰。
pub fn decode_constrained_sentences(
    syllables: &[String],
    lookup: &mut LatticeLookup<'_>,
    options: &LatticeOptions<'_>,
    accepts: impl Fn(usize, &str) -> bool,
) -> Vec<SentencePath> {
    if syllables.is_empty() {
        return Vec::new();
    }
    let mut unfiltered = options.clone();
    unfiltered.span_limit = usize::MAX;
    let mut graph = build_graph(syllables, lookup, &unfiltered);
    // 终点计数只在本次过滤中使用，跨起始列清零复用。
    let mut counts = vec![0; syllables.len() + 1];
    for (start, edges) in graph.iter_mut().enumerate() {
        counts.fill(0);
        edges.retain(|edge| {
            if !accepts(start, &edge.word) || counts[edge.end] >= options.span_limit {
                return false;
            }
            counts[edge.end] += 1;
            true
        });
    }
    decode_graph(&graph, options, None)
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
    // 重复跨度只查一次字典，缓存首个图边区间；首次查询行直接移交字段。
    struct CachedSpan {
        column: usize,
        edges: std::ops::Range<usize>,
        reserve: usize,
    }
    let span_capacity = (0..n)
        .map(|start| n.min(start.saturating_add(max_len)) - start)
        .sum();
    let mut span_cache: HashMap<&[String], CachedSpan> = HashMap::with_capacity(span_capacity);
    for start in 0..n {
        // 同一列的跨度长度各异，重复键必在更早的列；追加图边不改变区间索引。
        let (earlier, remaining) = graph.split_at_mut(start);
        let edges = &mut remaining[0];
        for end in start + 1..=n.min(start + max_len) {
            let span = &syllables[start..end];
            match span_cache.entry(span) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    let cached = entry.get();
                    edges.reserve(cached.reserve);
                    for edge in &earlier[cached.column][cached.edges.clone()] {
                        edges.push(Edge {
                            end,
                            word: edge.word.clone(),
                            key: edge.key.clone(),
                            log_prob: edge.log_prob,
                            typo: false,
                        });
                    }
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let rows = lookup(span);
                    let reserve = rows.len().min(options.span_limit);
                    edges.reserve(reserve);
                    let begin = edges.len();
                    let mut span_key = None;
                    for row in rows.into_iter().take(options.span_limit) {
                        if row.value.is_empty() {
                            continue;
                        }
                        let log_prob = edge_log_prob(
                            row.weight,
                            bonus_syllables(&row.value, end - start),
                            options,
                        );
                        edges.push(Edge {
                            end,
                            word: row.value,
                            key: if row.key.is_empty() {
                                span_key.get_or_insert_with(|| span.join("'")).clone()
                            } else {
                                row.key
                            },
                            log_prob,
                            typo: false,
                        });
                    }
                    entry.insert(CachedSpan {
                        column: start,
                        edges: begin..edges.len(),
                        reserve,
                    });
                }
            }
        }
    }
    graph
}

/// 与 `build_graph` 相同，但查询行只读借用调用方缓存；图边仍取得自己的字段所有权。
pub(super) fn build_graph_borrowed<'rows>(
    syllables: &[String],
    lookup: &mut impl FnMut(&[String]) -> &'rows [LatticeLexeme],
    options: &LatticeOptions<'_>,
) -> Graph {
    let n = syllables.len();
    let max_len = options.max_phrase_syllables.max(1);
    let mut graph: Graph = (0..n).map(|_| Vec::new()).collect();
    struct CachedSpan {
        column: usize,
        edges: std::ops::Range<usize>,
        reserve: usize,
    }
    let span_capacity = (0..n)
        .map(|start| n.min(start.saturating_add(max_len)) - start)
        .sum();
    let mut span_cache: HashMap<&[String], CachedSpan> = HashMap::with_capacity(span_capacity);
    for start in 0..n {
        let (earlier, remaining) = graph.split_at_mut(start);
        let edges = &mut remaining[0];
        for end in start + 1..=n.min(start + max_len) {
            let span = &syllables[start..end];
            match span_cache.entry(span) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    let cached = entry.get();
                    edges.reserve(cached.reserve);
                    for edge in &earlier[cached.column][cached.edges.clone()] {
                        edges.push(Edge {
                            end,
                            word: edge.word.clone(),
                            key: edge.key.clone(),
                            log_prob: edge.log_prob,
                            typo: false,
                        });
                    }
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let rows = lookup(span);
                    let reserve = rows.len().min(options.span_limit);
                    edges.reserve(reserve);
                    let begin = edges.len();
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
                            log_prob: edge_log_prob(
                                row.weight,
                                bonus_syllables(&row.value, end - start),
                                options,
                            ),
                            typo: false,
                        });
                    }
                    entry.insert(CachedSpan {
                        column: start,
                        edges: begin..edges.len(),
                        reserve,
                    });
                }
            }
        }
    }
    graph
}

struct BeamColumn<'g> {
    hyps: Vec<Hyp<'g>>,
    streamed: bool,
}

#[derive(Clone, Copy)]
struct Hyp<'g> {
    score: f64,
    /// 前驱假设的列号与行号；起点填 `(0, 0)`，只有 `edge` 非空时才可回溯。
    prev: (usize, usize),
    /// 到达本假设的图边；仅起点为 `None`，同时表示没有前驱。
    edge: Option<&'g Edge>,
    typo_edges: usize,
}

/// 搭配查询的上下文窗口：`.gram` 的查询键最多编入 8 个码点，尾窗留 8 个足够。
const COLLOCATION_TAIL_CODEPOINTS: usize = 8;

/// 定长内联的 UTF-8 尾窗，随 `Hyp` 按值复制，不进堆。
#[derive(Clone, Copy, Default)]
struct CollocationTail {
    bytes: [u8; COLLOCATION_TAIL_CODEPOINTS * 4],
    len: u8,
}

impl CollocationTail {
    fn as_str(&self) -> &str {
        #[cfg(all(test, debug_assertions))]
        collocation_tail_tests::record_tail_string();
        // 内容只由词边文本拼成，恒为合法 UTF-8；兜底空串不 panic。
        std::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("")
    }

    /// 尾窗推进一个词：留下 (tail + word) 的最后 `COLLOCATION_TAIL_CODEPOINTS` 个码点。
    fn push(&mut self, word: &str) {
        let word_cps = word.chars().count();
        if word_cps >= COLLOCATION_TAIL_CODEPOINTS {
            let start = word
                .char_indices()
                .nth(word_cps - COLLOCATION_TAIL_CODEPOINTS)
                .map(|(i, _)| i)
                .unwrap_or(0);
            let tail = &word[start..];
            self.bytes[..tail.len()].copy_from_slice(tail.as_bytes());
            self.len = tail.len() as u8;
            return;
        }
        let keep = COLLOCATION_TAIL_CODEPOINTS - word_cps;
        // 先算好位置结束对 self 的只读借用，再原地搬移字节。
        let (start, tail_len) = {
            let tail = self.as_str();
            let skip = tail.chars().count().saturating_sub(keep);
            (
                tail.char_indices()
                    .nth(skip)
                    .map(|(i, _)| i)
                    .unwrap_or(tail.len()),
                tail.len(),
            )
        };
        let keep_bytes = tail_len - start;
        self.bytes.copy_within(start..tail_len, 0);
        self.bytes[keep_bytes..keep_bytes + word.len()].copy_from_slice(word.as_bytes());
        self.len = (keep_bytes + word.len()) as u8;
    }
}

/// 沿 `prev` 链回溯，把路径已累计文本的最后 `COLLOCATION_TAIL_CODEPOINTS`
/// 个码点装进尾窗。只为 `collocation_scorer` 激活的假设回溯一次，
/// 未激活路径不占 `Hyp` 的内存。
fn collocation_tail(columns: &[BeamColumn<'_>], pos: usize, index: usize) -> CollocationTail {
    #[cfg(all(test, debug_assertions))]
    collocation_tail_tests::record_tail();
    let mut words = [None; COLLOCATION_TAIL_CODEPOINTS];
    let mut count = 0;
    let mut cps = 0usize;
    let (mut p, mut i) = (pos, index);
    while let Some(edge) = columns[p].hyps[i].edge {
        words[count] = Some(edge.word.as_str());
        count += 1;
        cps += edge.word.chars().count();
        if cps >= COLLOCATION_TAIL_CODEPOINTS || count >= words.len() {
            break;
        }
        let prev = columns[p].hyps[i].prev;
        p = prev.0;
        i = prev.1;
    }
    let mut tail = CollocationTail::default();
    for word in words[..count].iter().rev().flatten() {
        tail.push(word);
    }
    tail
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

/// 按前向图累计各列在剪枝前可能收到的假设上界；源列剪枝后最多保留 `beam` 条。
fn column_capacities(graph: &Graph, extra: Option<&Graph>, beam: usize) -> Vec<usize> {
    let mut capacities = vec![0usize; graph.len() + 1];
    capacities[0] = 1;
    for (start, edges) in graph.iter().enumerate() {
        let surviving = capacities[start].min(beam);
        for edge in edges
            .iter()
            .chain(extra.into_iter().flat_map(|extra| &extra[start]))
        {
            if edge.end <= graph.len() {
                capacities[edge.end] = capacities[edge.end].saturating_add(surviving);
            }
        }
    }
    capacities
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

// 小列只在线性借用槽中定位，超过上限后沿用原哈希索引。
const SMALL_BIGRAM_ROWS: usize = 8;

/// WL:385-485. `extra` holds more edges per column searched as if they were part of `graph`, so the typo decode adds edges without copying the literal graph.
pub(super) fn decode_graph(
    graph: &Graph,
    options: &LatticeOptions<'_>,
    extra: Option<&Graph>,
) -> Vec<SentencePath> {
    // 常见预算与非末列一致，单独实例化以保留展开循环的固定束宽。
    if options.nbest > options.beam {
        decode_graph_with_terminal::<true>(graph, options, extra)
    } else {
        decode_graph_with_terminal::<false>(graph, options, extra)
    }
}

fn decode_graph_with_terminal<const WIDE_TERMINAL: bool>(
    graph: &Graph,
    options: &LatticeOptions<'_>,
    extra: Option<&Graph>,
) -> Vec<SentencePath> {
    let n = graph.len();
    let personal = active_personal(options);
    let bigram_table = options.bigram.as_ref();
    let bigram = options.bigram.as_deref();
    // 搭配项是逐边加分：scorer 存在且标定系数为正才生效，退回现状与缺表完全一致。
    let collocation = options
        .collocation_scorer
        .filter(|_| options.collocation_weight > 0.0);
    let mut memo = options.bigram_memo.map(RefCell::borrow_mut);
    let capacities = column_capacities(graph, extra, options.beam);
    let terminal_limit = if WIDE_TERMINAL {
        options.beam.max(options.nbest)
    } else {
        options.beam
    };
    // 各列先按入列上界决定是否流式选择；末列使用最终结果预算。
    let mut columns: Vec<BeamColumn<'_>> = capacities
        .into_iter()
        .enumerate()
        .map(|(position, capacity)| {
            let limit = if WIDE_TERMINAL && position == n {
                terminal_limit
            } else {
                options.beam
            };
            let stream_capacity = limit.saturating_add(1);
            let streamed = limit != 0 && capacity > stream_capacity;
            BeamColumn {
                hyps: Vec::with_capacity(if streamed { stream_capacity } else { capacity }),
                streamed,
            }
        })
        .collect();
    columns[0].hyps.push(Hyp {
        score: 0.0,
        prev: (0, 0),
        edge: None,
        typo_edges: 0,
    });

    // 同一列里很多假设经由同一个词到达，只是更早的历史不同；它们对每条出边查到的二元分完全一样。按前一个词把这一列所有出边的二元分记下来，同一个词只查一遍表。
    let row_capacity = options.beam.max(options.nbest);
    let mut bigram_rows: Vec<Vec<f32>> = Vec::new();
    let mut bigram_row_indices: HashMap<&str, usize> = HashMap::new();
    let mut small_bigram_row_words = [""; SMALL_BIGRAM_ROWS];
    for pos in 0..n {
        if columns[pos].streamed {
            cxx_sort::finish_partial(&mut columns[pos].hyps, options.beam, &mut scores_higher);
        } else {
            keep_beam(&mut columns[pos].hyps, options.beam);
        }
        let (done, ahead) = columns.split_at_mut(pos + 1);
        let current = &done[pos].hyps;
        // 行存储跨列复用；前词索引和有效行数只属于当前列，评分必须重新填满。
        let mut used_bigram_rows = 0;
        bigram_row_indices.clear();
        // 只读原始切片，评分与展开都按主图、附加图的相同顺序重新遍历。
        let literal = &graph[pos];
        let extra_edges = extra.map_or(&[][..], |extra| extra[pos].as_slice());
        let outgoing_len = literal.len() + extra_edges.len();
        let outgoing = literal.iter().chain(extra_edges.iter());
        for (index, hyp) in current.iter().enumerate() {
            // Column 0 has no predecessor, so the start token carries what the corpus knows about how sentences open; every later column uses the word the hypothesis arrived on.
            let previous = hyp.edge.map_or(SENTENCE_START, |edge| edge.word.as_str());
            let bonuses = bigram_table.map(|table| {
                let cached_row = if used_bigram_rows <= SMALL_BIGRAM_ROWS {
                    small_bigram_row_words[..used_bigram_rows]
                        .iter()
                        .position(|word| *word == previous)
                } else {
                    bigram_row_indices.get(previous).copied()
                };
                let row = if let Some(row) = cached_row {
                    row
                } else {
                    let row = used_bigram_rows;
                    if row == bigram_rows.len() {
                        if bigram_rows.is_empty() {
                            bigram_rows.reserve_exact(row_capacity);
                        }
                        bigram_rows.push(Vec::new());
                    }
                    let scores = &mut bigram_rows[row];
                    scores.clear();
                    scores.reserve_exact(outgoing_len);
                    match memo.as_deref_mut() {
                        Some(memo) => scores.extend(
                            outgoing
                                .clone()
                                .map(|edge| memo.score(table, previous, &edge.word)),
                        ),
                        None => scores.extend(
                            outgoing
                                .clone()
                                .map(|edge| table.bigram(previous, &edge.word)),
                        ),
                    }
                    used_bigram_rows += 1;
                    if row < SMALL_BIGRAM_ROWS {
                        small_bigram_row_words[row] = previous;
                    } else {
                        if row == SMALL_BIGRAM_ROWS {
                            // 第九个不同前词才建立哈希索引，行号仍按首次出现的顺序。
                            bigram_row_indices.reserve(row_capacity);
                            bigram_row_indices.extend(
                                small_bigram_row_words
                                    .iter()
                                    .enumerate()
                                    .map(|(index, word)| (*word, index)),
                            );
                        }
                        bigram_row_indices.insert(previous, row);
                    }
                    row
                };
                &bigram_rows[row]
            });
            // 前驱列已经剪枝且不会再改变；前驱列为零时没有更早的词，起点也填零。
            let context = personal.map(|personal| {
                let earlier = match hyp.prev {
                    (column, at) if column > 0 => {
                        done[column].hyps[at].edge.map(|edge| edge.word.as_str())
                    }
                    _ => None,
                };
                personal.context(earlier, Some(previous))
            });
            // 同一假设的出边共享上下文；空列或搭配停用时不回溯。
            let prepared_tail = collocation
                .filter(|_| outgoing_len != 0)
                .map(|scorer| (scorer, collocation_tail(done, pos, index)));
            let collocation = prepared_tail
                .as_ref()
                .map(|(scorer, tail)| (*scorer, tail.as_str()));
            for (at, edge) in outgoing.clone().enumerate() {
                let mut score = hyp.score + edge.log_prob;
                if let Some((scorer, tail)) = collocation {
                    // 句尾词把 is_rear 置真：`.gram` 的「词+$」rear 项据此参与打分。
                    score += options.collocation_weight * scorer(tail, &edge.word, edge.end == n);
                }
                if edge.end == n && hyp.edge.is_some() && is_sentence_final_particle(&edge.word) {
                    score += SENTENCE_FINAL_PARTICLE_BONUS;
                }
                let static_bonus = bonuses.map_or(0.0, |bonuses| f64::from(bonuses[at]));
                if bigram.is_some() {
                    // Only a pair the table favours moves the path. A pair it calls less likely than chance counts the same as one it never kept: the table drops rare pairs, so an absent pair is usually rarer still, and penalising the observed one made a common word lose to a rarer one with no pair at all. 句首→一大 is -1.0 and 句首→益达 is absent, which put 益达拖 ahead of 一大拖 for yidatuo (#7583).
                    score += options.bigram_weight * static_bonus.max(0.0);
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
                let next = Hyp {
                    score,
                    prev: (pos, index),
                    edge: Some(edge),
                    typo_edges: hyp.typo_edges + usize::from(edge.typo),
                };
                let target = &mut ahead[edge.end - pos - 1];
                if target.streamed {
                    let limit = if WIDE_TERMINAL && edge.end == n {
                        terminal_limit
                    } else {
                        options.beam
                    };
                    cxx_sort::push_partial(&mut target.hyps, limit, next, &mut scores_higher);
                } else {
                    target.hyps.push(next);
                }
            }
        }
    }

    // 末列不再被后续路径回溯；先完成同一堆选择，再沿用原最终排序。
    let terminal = &mut columns[n];
    let mut last = std::mem::take(&mut terminal.hyps);
    if last.is_empty() {
        return Vec::new();
    }
    if terminal.streamed {
        cxx_sort::finish_partial(&mut last, terminal_limit, &mut scores_higher);
    } else {
        keep_beam(&mut last, terminal_limit);
    }
    cxx_sort::sort(&mut last, &mut scores_higher);
    let take = options.nbest.min(last.len());

    // 小末列沿用线性去重；收齐唯一预算后不再物化后续假设，大末列保留原哈希回退。
    let inline_unique = last.len() <= SMALL_SENTENCE_PATHS;
    let mut paths: Vec<SentencePath> = Vec::with_capacity(last.len());
    let mut trace = Vec::new();
    for hyp in &last {
        if inline_unique && paths.len() == take {
            break;
        }
        // 图在回溯期间不变；首条实际访问的路径才申请引用页，后续路径复用它。
        trace.clear();
        trace.reserve_exact(n);
        let mut cursor = Some(hyp);
        while let Some(step) = cursor {
            let Some(edge) = step.edge else { break };
            trace.push(edge);
            let (column, at) = step.prev;
            cursor = Some(&columns[column].hyps[at]);
        }
        trace.reverse();
        let mut sentence = String::with_capacity(trace.iter().map(|edge| edge.word.len()).sum());
        for edge in &trace {
            sentence.push_str(&edge.word);
        }
        if sentence.is_empty()
            || (inline_unique && paths.iter().any(|path| path.sentence == sentence))
        {
            continue;
        }
        // 保留的路径才复制拥有词；按边数连接读音，空读音也保留原分隔符。
        let mut words = Vec::with_capacity(n);
        let mut key = String::with_capacity(
            trace.iter().map(|edge| edge.key.len()).sum::<usize>() + trace.len().saturating_sub(1),
        );
        for (index, edge) in trace.iter().enumerate() {
            words.push(edge.word.clone());
            if index != 0 {
                key.push('\'');
            }
            key.push_str(&edge.key);
        }
        paths.push(SentencePath {
            sentence,
            key,
            log_prob: hyp.score,
            words,
            typo_edges: hyp.typo_edges,
        });
    }
    drop(trace);
    if !inline_unique {
        retain_unique_sentences(&mut paths, take);
    }
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
    decode_typo_on_graph_with_policy(graph, options, literal_best, edges, false)
}

/// 解码纠错整句；双拼即使纠错句分数低于当前字面整句，也要把它作为候选提供给用户。
pub(super) fn decode_typo_on_graph_with_policy(
    graph: &Graph,
    options: &LatticeOptions<'_>,
    literal_best: &SentencePath,
    edges: &[TypoEdge],
    allow_nonleading: bool,
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
        nbest: if allow_nonleading {
            options.nbest.max(64)
        } else {
            options.nbest
        },
        ..options.clone()
    };
    let paths = decode_graph(graph, &typo_options, Some(&extra));
    let best = if allow_nonleading {
        paths.into_iter().find(|path| path.typo_edges > 0)?
    } else {
        paths.into_iter().next()?
    };
    // The narrower beam can lose the literal best path, so beating it is checked rather than assumed.
    if best.typo_edges == 0 || best.sentence == literal_best.sentence {
        return None;
    }
    if !allow_nonleading && best.log_prob <= literal_best.log_prob {
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

/// 整句改字里的一段：音节 `start..end` 转换成 `word`，`key` 是它的读音（音节以 `'` 连接）。改字只处理一个汉字对应一个音节的句子，所以 `word` 的字数等于 `end - start`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedSpan {
    pub start: usize,
    pub end: usize,
    pub word: String,
    pub key: String,
}

/// 在 `pins` 的约束下重新转换 `syllables`：钉住的段原样出现，其余音节由词网格解出，不能跨过任何钉住的段。只用一个汉字对应一个音节的词，结果按音节切成首尾相接、覆盖全部音节的段。`pins` 须互不重叠且落在音节范围内；某个钉住的词不在词库里时按最低权重补一条边。解不出路径时为空。
pub fn decode_pinned(
    syllables: &[String],
    lookup: &mut LatticeLookup<'_>,
    options: &LatticeOptions<'_>,
    pins: &[PinnedSpan],
) -> Vec<PinnedSpan> {
    let n = syllables.len();
    if n == 0 || pins.iter().any(|pin| pin.start >= pin.end || pin.end > n) {
        return Vec::new();
    }
    let mut graph = build_graph(syllables, lookup, options);
    for (start, edges) in graph.iter_mut().enumerate() {
        edges.retain(|edge| {
            if edge.word.chars().count() != edge.end - start {
                return false;
            }
            pins.iter().all(|pin| {
                let overlaps = start < pin.end && pin.start < edge.end;
                !overlaps || (start == pin.start && edge.end == pin.end && edge.word == pin.word)
            })
        });
    }
    for pin in pins {
        let column = &mut graph[pin.start];
        if !column.iter().any(|edge| edge.end == pin.end) {
            column.push(Edge {
                end: pin.end,
                word: pin.word.clone(),
                key: pin.key.clone(),
                log_prob: edge_log_prob(1, pin.end - pin.start, options),
                typo: false,
            });
        }
    }
    let Some(best) = decode_graph(&graph, options, None).into_iter().next() else {
        return Vec::new();
    };
    let keys: Vec<&str> = best.key.split('\'').collect();
    if keys.len() != n {
        return Vec::new();
    }
    let mut spans = Vec::with_capacity(best.words.len());
    let mut start = 0;
    for word in best.words {
        let end = start + word.chars().count();
        spans.push(PinnedSpan {
            start,
            end,
            key: keys[start..end].join("'"),
            word,
        });
        start = end;
    }
    spans
}

#[cfg(test)]
#[path = "decode_bigrams_tests.rs"]
mod bigram_buffer_tests;

#[cfg(test)]
#[path = "decode_bigram_memo_entry_tests.rs"]
mod bigram_memo_entry_tests;

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
    fn a_complete_phrase_does_not_lose_to_extra_frequency_terms() {
        let rows = table(&[
            ("xing'he", &[("星禾", 50_000)]),
            ("mian'ban", &[("面板", 20_000)]),
            ("xing'he'mian'ban", &[("星河面板", 100)]),
        ]);
        assert_eq!(
            best("xing'he'mian'ban", &rows, &LatticeOptions::default()),
            "星河面板"
        );
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
        assert!((edge_log_prob(100, 2, &options) - (100f64.ln() + 20.0)).abs() < 1e-12);
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

        assert_eq!(column_capacities(&graph, Some(&extra), 4), [1, 1, 3, 0]);
    }

    #[test]
    fn decode_without_bigram_does_not_allocate_bigram_rows() {
        let graph = vec![Vec::new()];
        let options = LatticeOptions::default();
        let (paths, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            decode_graph(&graph, &options, None)
        });

        assert!(paths.is_empty());
        assert_eq!(allocations, 3);
    }

    #[test]
    fn bigram_row_allocations_do_not_grow_with_column_count() {
        use crate::ime::personal_rerank::allocations::measure;

        let directory = tempfile::tempdir().unwrap();
        let bigram = Arc::new(pair_table(
            directory.path(),
            "zero.bin",
            &[("甲", "甲", 0.0)],
        ));
        for length in [1, 2, 8, 32] {
            let graph = (0..length)
                .map(|position| {
                    vec![Edge {
                        end: position + 1,
                        word: "甲".to_owned(),
                        key: "jia".to_owned(),
                        log_prob: -1.0,
                        typo: false,
                    }]
                })
                .collect();
            let options = LatticeOptions {
                beam: 1,
                nbest: 1,
                ..LatticeOptions::default()
            };
            let (plain, without) = measure(|| decode_graph(&graph, &options, None));
            let options = LatticeOptions {
                bigram: Some(Arc::clone(&bigram)),
                ..options
            };
            let (scored, with) = measure(|| decode_graph(&graph, &options, None));
            assert_eq!(scored, plain);
            assert_eq!(with.remaining_bytes, without.remaining_bytes);
            assert_eq!(with.minimum_bytes, 0);
            assert!(with.peak_bytes <= without.peak_bytes + 256);
            assert!(
                with.allocations <= without.allocations + 3,
                "{length} 列：无表 {without:?}，有表 {with:?}"
            );
        }
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

    #[test]
    fn a_sentence_final_particle_beats_a_commoner_homophone() {
        let rows = table(&[
            ("zhai'quan", &[("债券", 5000)]),
            ("ma", &[("马", 9000), ("吗", 3000)]),
        ]);
        assert_eq!(
            best("zhai'quan'ma", &rows, &LatticeOptions::default()),
            "债券吗"
        );
    }

    #[test]
    fn the_particle_bonus_is_only_for_the_last_word() {
        let rows = table(&[
            ("ma", &[("马", 9000), ("吗", 3000)]),
            ("shang", &[("上", 9000)]),
        ]);
        assert_eq!(best("ma'shang", &rows, &LatticeOptions::default()), "马上");
    }

    #[test]
    fn reduplicated_interjections_take_no_phrase_bonus() {
        assert_eq!(bonus_syllables("啊啊", 2), 1);
        assert_eq!(bonus_syllables("呃呃呃", 3), 1);
        assert_eq!(bonus_syllables("爸爸", 2), 2);
        assert_eq!(bonus_syllables("哈哈", 2), 2);
        assert_eq!(bonus_syllables("啊", 1), 1);
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
    fn an_unlikely_pair_does_not_lose_to_an_unseen_one() {
        let directory = tempfile::tempdir().unwrap();
        let rows = table(&[
            ("yi'da", &[("一大", 301240), ("益达", 200000)]),
            ("tuo", &[("拖", 463823)]),
        ]);
        let text = "yi'da'tuo";
        assert_eq!(best(text, &rows, &LatticeOptions::default()), "一大拖");

        // The shipped table has 句首 -> 一大 at -1.0 and no 句首 -> 益达 at all (#7583).
        let bigram = Arc::new(pair_table(
            directory.path(),
            "start.bin",
            &[(SENTENCE_START, "一大", -1.0)],
        ));
        let options = LatticeOptions {
            bigram: Some(bigram),
            ..LatticeOptions::default()
        };
        assert_eq!(
            best(text, &rows, &options),
            "一大拖",
            "a pair scored below chance weighs the same as a pair the table never kept"
        );
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

    fn pinned(start: usize, end: usize, word: &str, key: &str) -> PinnedSpan {
        PinnedSpan {
            start,
            end,
            word: word.to_owned(),
            key: key.to_owned(),
        }
    }

    fn pinned_words(spans: &[PinnedSpan]) -> Vec<&str> {
        spans.iter().map(|span| span.word.as_str()).collect()
    }

    #[test]
    fn pinned_decode_keeps_the_pin_and_reconverts_around_it() {
        let rows = table(&[
            ("bei", &[("被", 9000), ("北", 5000), ("背", 4000)]),
            ("jing", &[("经", 9000), ("京", 3000), ("景", 2000)]),
            ("bei'jing", &[("背景", 30000), ("北京", 20000)]),
            ("qu", &[("去", 9000)]),
        ]);
        let options = LatticeOptions::default();
        let text = syllables("qu'bei'jing");
        let free = decode_pinned(&text, &mut lookup(&rows), &options, &[]);
        assert_eq!(pinned_words(&free), ["去", "背景"]);
        assert_eq!(free[1], pinned(1, 3, "背景", "bei'jing"));

        // 钉住一个两字词：它原样出现，左边照常解码。
        let word = decode_pinned(
            &text,
            &mut lookup(&rows),
            &options,
            &[pinned(1, 3, "北京", "bei'jing")],
        );
        assert_eq!(pinned_words(&word), ["去", "北京"]);

        // 钉住词中间的一个字：跨过它的词不能再用，左边那个音节单独解码。
        let character = decode_pinned(
            &text,
            &mut lookup(&rows),
            &options,
            &[pinned(2, 3, "京", "jing")],
        );
        assert_eq!(pinned_words(&character), ["去", "被", "京"]);
        assert_eq!(character[2], pinned(2, 3, "京", "jing"));
    }

    #[test]
    fn a_pin_missing_from_the_dictionary_still_decodes() {
        let rows = table(&[("qu", &[("去", 9000)]), ("bei", &[("被", 9000)])]);
        let spans = decode_pinned(
            &syllables("qu'bei"),
            &mut lookup(&rows),
            &LatticeOptions::default(),
            &[pinned(1, 2, "贝", "bei")],
        );
        assert_eq!(pinned_words(&spans), ["去", "贝"]);
        assert!(decode_pinned(
            &syllables("qu'bei"),
            &mut lookup(&rows),
            &LatticeOptions::default(),
            &[pinned(1, 3, "贝", "bei")],
        )
        .is_empty());
    }
}

#[cfg(test)]
#[path = "graph_ownership_tests.rs"]
mod graph_ownership_tests;

#[cfg(test)]
#[path = "decode_hyp_layout_tests.rs"]
mod hyp_layout_tests;

#[cfg(test)]
#[path = "decode_capacity_tests.rs"]
mod capacity_tests;

#[cfg(test)]
#[path = "decode_traceback_tests.rs"]
mod traceback_tests;

#[cfg(test)]
#[path = "decode_return_tests.rs"]
mod return_tests;

#[cfg(test)]
#[path = "decode_prefix_tests.rs"]
mod prefix_tests;

#[cfg(test)]
#[path = "decode_outgoing_tests.rs"]
mod outgoing_tests;

#[cfg(test)]
#[path = "decode_entry_tests.rs"]
mod entry_tests;

#[cfg(test)]
#[path = "decode_stream_tests.rs"]
mod stream_tests;

#[cfg(test)]
#[path = "decode_materialization_tests.rs"]
mod materialization_tests;

#[cfg(test)]
#[path = "decode_terminal_tests.rs"]
mod terminal_tests;

#[cfg(test)]
#[path = "decode_constrained_tests.rs"]
mod constrained_tests;

#[cfg(test)]
#[path = "decode_row_index_tests.rs"]
mod row_index_tests;

#[cfg(test)]
#[path = "decode_collocation_tests.rs"]
mod collocation_tests;

#[cfg(test)]
#[path = "decode_collocation_tail_tests.rs"]
mod collocation_tail_tests;
