//! 候选排序的唯一实现：句子模型重排、整句次选读法的后移，以及交给重排器的上文窗口。
//!
//! 这里只做决策，不碰候选数组：`rerank_pick` 返回应移到首位的下标，`runner_up_order` 返回一个排列（新座位 -> 旧座位）。`msime-input-runtime` 把结果同步应用到候选元数据和引擎座位映射上，网页引擎（`msime-engine-wasm`）也调用同一组函数，两处的排序因此不会各自漂移。
//!
//! 这是 msime 自己的产品胶水，不在引擎文档化的 API 里：第三方拿到的是引擎的原始顺序（核心 `Session` 的候选），要不要再排、怎么排由它自己决定。

use crate::SchemeType;

pub use chinese_ime_lm::{CandidateFacts, Reranker, SentenceModel, DICTIONARY_SOURCES};

/// 引擎拼整句时给句子前缀打分的模型：给上文和一批文字，返回各自的逐字平均自然对数概率，顺序同 `candidates`，打不了分时返回空表。可以从任何线程调用，实现自己加锁。
///
/// 和 [`CandidateScorer`] 的区别：那个在 runtime 里、最后从引擎给的整句里挑；这个交给引擎，在词网格的束搜索里每一列给假设打分，让模型看着像话的前缀留到后面，正确的整句不至于在半路被剪掉（`lattice::decode` 的 `BeamModel`）。只有挂上的逐键打分器给出它（[`CandidateScorer::sentence_scorer`]）时引擎才用。
pub trait SentenceScorer: Send + Sync {
    fn log_probabilities(&self, context: &str, candidates: &[&str]) -> Vec<f32>;
}

/// 交给引擎的整句打分器，随每个请求带着（`QueryRequest::sentence_scorer`）。按指针比较：同一个打分器才算没变。
#[derive(Clone, Default)]
pub struct SentenceScorerHandle(pub Option<std::sync::Arc<dyn SentenceScorer>>);

impl PartialEq for SentenceScorerHandle {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }
}

impl std::fmt::Debug for SentenceScorerHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(if self.0.is_some() {
            "SentenceScorerHandle(Some)"
        } else {
            "SentenceScorerHandle(None)"
        })
    }
}

/// 排序决策要的模型能力：按上文给候选打逐字平均自然对数概率，以及模型一次能读多少字。
///
/// `rerank_pick` 只经这个接口用模型，所以随包的纯 Rust 句子模型（[`Reranker`]）和需要原生运行库的模型后端走同一套规则：裁决点、词网格分混合、词典首选的豁免都在这里，后端只管算分。引擎本身仍是纯 Rust，原生后端放在引擎之外的 crate 里实现这个接口。
pub trait CandidateScorer: Send {
    /// 模型一次能读的字数，上文和候选合计；[`rerank_context`] 按它截上文。
    fn context_length(&self) -> usize;

    /// 每个候选在 `context` 之后的逐字平均自然对数概率，顺序同 `candidates`。
    fn log_probabilities(&mut self, context: &str, candidates: &[&str]) -> Vec<f32>;

    /// 在上屏的那一次派发里同步给一整池联想续写（[`prediction_order`]，最多 32 条）打分，是否仍在逐键 p95 ≤ 10 ms 的预算之内。上屏也是一次按键，守同一条线。是的话 runtime 才打开引擎的联想候选池、让它排上屏后联想；不是的话联想与没挂打分器时逐行相同。
    ///
    /// 默认 `false`，后端要在这台机器上量过才报 `true`。随包的 [`Reranker`]（`model-v1`，纯 Rust 参考实现）不报：Pixel 3 XL 上它让每次上屏多花 p50 约 50 ms、p95 约 100–120 ms。ONNX Runtime 后端只在大模型过了宿主的自测预算时报（见 `OnnxScorer::fits_keystroke_budget`）：Mac 上 8x448 多花 p95 约 5 ms；退到 6x256 的老手机不报，Pixel 3 XL 上 6x256 多花 p95 13–14 ms。见 `.agents/notes/implemented/feature/2026-11-02-next-word-prediction-rerank.md`。
    fn scores_prediction_pool_within_keystroke_budget(&self) -> bool {
        false
    }

    /// 挂着这个打分器时，引擎每条切分至少解出几条整句读法给它比较（`Session::set_sentence_pool`）；0 是引擎的默认条数（6）。
    ///
    /// 默认 0：多比较几条要多花模型的时间，后端要在真实运行时里量过才报。随包的 [`Reranker`]（`model-v1`）不报，它在 Pixel 3 XL 上本来就超出逐键预算。ONNX Runtime 后端报宿主按这台机器的速度定下的池（`OnnxScorer::set_sentence_pool`，默认 [`SENTENCE_POOL`]）。
    fn sentence_pool(&self) -> usize {
        0
    }

    /// 词网格整句之间比较时（[`rerank_pick`] 的静态证据分支），词网格逐字分在混合里的权重，模型逐字分占其余。默认 0.5，等权，是在随包 `model-v1` 上定的（`.agents/notes/implemented/bug-fix/2026-10-14-rerank-static-evidence.md`）。更准的模型可以让它少一些：ONNX Runtime 后端报 0.3，见 `.agents/notes/implemented/feature/2026-11-02-onnx-static-evidence-weight.md`。
    fn static_evidence_weight(&self) -> f64 {
        0.5
    }

    /// 上屏的文字变了（runtime 每次上屏、或宿主送来上文之后调），`context` 是此后按键时排序所依据的全部上文，尚未按 [`rerank_context`] 截短。按上文缓存模型状态的后端可以趁下一次按键之前在别的线程把它编码好，按键线程就不用等；默认什么都不做。这里不能同步做耗时的事：调用就在上屏那一次按键里。
    fn context_changed(&mut self, _context: &str) {}

    /// 挂着这个逐键打分器时，停顿后的落定重排（`Runtime::rerank_settled`）不再做：它逐键排出的顺序就是最终顺序。默认 `false`，随包的 [`Reranker`] 不报。ONNX Runtime 后端报 `true`：落定模型是随包的 `model-v1` 桌面版，比它弱，停顿后再排一遍只会把它选对的换掉——2026-11-03 在 typing-chat test 上，6x256 配 32 条首选 79.1%，加上落定重排降到 73.6%，与真实 macOS 输入法停顿后按空格量出的 73.2% 吻合（220 段里 192 段首选一字不差）。
    fn supersedes_settled_rerank(&self) -> bool {
        false
    }

    /// 交给引擎、在拼整句时给前缀打分的同一个模型（[`SentenceScorer`]）。默认 `None`：随包的 [`Reranker`] 不给，它跑不进束搜索每一列的预算。ONNX Runtime 后端给，共用同一个模型和前缀缓存。
    fn sentence_scorer(&self) -> Option<std::sync::Arc<dyn SentenceScorer>> {
        None
    }

    /// 模型认为应当提到首位的候选下标，`None` 为不动；规则同 [`Reranker::best_where`]：首行是可信的词典命中或首行不回答按键时不比较，只比较前 [`COMPARED_LIMIT`] 条回答按键的候选，少于两条不比较。
    fn best_where(
        &mut self,
        context: &str,
        texts: &[&str],
        facts: &dyn Fn(usize) -> CandidateFacts,
    ) -> Option<usize> {
        let leader = facts(0);
        if texts.is_empty() || leader.trusted_dictionary_hit || !leader.answers_key {
            return None;
        }
        let mut considered = [0; COMPARED_LIMIT];
        let mut subset = [""; COMPARED_LIMIT];
        let mut count = 0;
        for index in (0..texts.len()).filter(|&index| facts(index).answers_key) {
            if count == COMPARED_LIMIT {
                break;
            }
            considered[count] = index;
            subset[count] = texts[index];
            count += 1;
        }
        if count < 2 {
            return None;
        }
        let scores = self.log_probabilities(context, &subset[..count]);
        if scores.len() != count {
            return None;
        }
        let mut best = 0;
        for slot in 1..count {
            if scores[slot] > scores[best] {
                best = slot;
            }
        }
        (best != 0).then(|| considered[best])
    }
}

/// [`CandidateScorer::best_where`] 最多比较几条回答按键的候选，与 [`Reranker::best_where`] 相同。
pub const COMPARED_LIMIT: usize = 9;

impl CandidateScorer for Reranker {
    fn context_length(&self) -> usize {
        self.model().context_length()
    }

    fn log_probabilities(&mut self, context: &str, candidates: &[&str]) -> Vec<f32> {
        Reranker::log_probabilities(self, context, candidates)
    }

    // 随包模型沿用它自己的实现，行为与引入这个接口之前逐字相同。
    fn best_where(
        &mut self,
        context: &str,
        texts: &[&str],
        facts: &dyn Fn(usize) -> CandidateFacts,
    ) -> Option<usize> {
        Reranker::best_where(self, context, texts, facts)
    }
}

/// `CandidateSource::Generated`: a whole-sentence path the word lattice assembled. The one source
/// whose members really are alternative readings of the same key.
pub const LATTICE_SOURCE: u8 = 8;

/// `CandidateSource::QuickPhrase`, `Emoji` and `Kaomoji`: entries a keyword finds in a catalog, listed in the catalog's order. They are not readings of the key, so the reranker neither compares them nor promotes them: typing `kiss` in kaomoji mode had the model seat the catalog's 646th entry, `French Kiss!(*￣(￣　*)`, above its first.
const CATALOG_SOURCES: [u8; 3] = [5, 6, 7];

/// `CandidateSource::AiSuggestion` 和已经不再产生的占位行 `AiPending`：在线座位规则（`normalize_online_slots`）总把 AI 行放回第一页最后一格和列表末尾，模型提它们也没用；更要紧的是，若让 AI 候选参与比较，它一到就可能抢走本来要提到首位的本地候选，回答到达时本地候选的次序就变了（#6888）。
const AI_SEAT_SOURCES: [u8; 2] = [3, 12];

/// `CandidateSource::EnglishDictionary`。混输英文只在拼音拆不开、英文完全匹配（或用户固定在第一位）时才由引擎放到首位，那是引擎有意的安排；句子模型读的是中文，不能因为英文行不算词典命中就把中文行转到它前面。这道检查不分方案和模式：只有英文行的列表（比如专用英文模式、临时英文）首行也是英文，同样不重排，这是有意的：模型读的是中文，比较英文行没有意义。
const ENGLISH_SOURCE: u8 = 4;

/// `CandidateSource::MixedJapanese`：日文混输行。它领先时是引擎判定拼音给不出整段的词，和领先的英文一样不重排；它在别的位置时也不让模型拿它比较、把它提到首位，句子模型读的是中文。
const MIXED_JAPANESE_SOURCE: u8 = 13;

/// 候选列表通常不超过一页；短列表的文本引用放在栈上，较大列表继续走堆缓冲。
const SMALL_RERANK_TEXTS: usize = 128;

/// 静态证据分支只比较前九条真正回答按键的候选。
const STATIC_RERANK_LIMIT: usize = 9;

/// 报这个数的逐键打分器（[`CandidateScorer::sentence_pool`]）挂着时，26 键全拼每条切分解出的整句读法条数（`Session::set_sentence_pool`），正好填满模型一次比较的条数。不挂时引擎只解 6 条，长句的比较位常有三条空着，而对的那句往往就排在第 7 到第 9 条：ctx-v1 的 dev 分句上，金标准出现在候选里的比例从 72.1% 到 75.6%，6x256 的首选从 65.7% 到 67.9%。更多的读法还会更准（解 32 条、比较 32 条是 72.3%），但模型的耗时随比较的条数增长，Pixel 3 XL 上 6x256 已经用到逐键预算的九成，所以只填满现有的比较位。见 `.agents/notes/implemented/feature/2026-11-02-sentence-pool-for-reranker.md`。
pub const SENTENCE_POOL: usize = STATIC_RERANK_LIMIT;

/// 打分器报的整句池最多这么大：静态证据分支一次比较的条数取 [`SENTENCE_POOL`]（九条）与池中较大的那个，放在栈上的数组按它开。
pub const MAX_SENTENCE_POOL: usize = 32;

/// 候选行不在任何窄裁决点上。
pub const CONTEST_NONE: u8 = 0;
/// 九宫格里固定在第 1 位的最近选择，和没有它时会领先的那条词典行。
pub const CONTEST_RECENCY: u8 = 1;
/// 九宫格首页里拼满读法、字数相同、权重相近的前两条词典行：同一串数字的另一个读音（监狱/教育），或者同音的另一个词（他/她）。
pub const CONTEST_RUNNER_UP: u8 = 2;

/// 最近选择要让位，模型给另一行的逐字平均自然对数概率得比它高出这么多。最近选择是用户上一次在同一串键上的选择，是不弱的证据，margin 为 0 时 dev 上一词一块聊天集反而变差；dev 上按事先写下的规则在 0、1、2 里选了 1，见 `.agents/notes/implemented/feature/2026-10-29-nine-key-model-contests.md`。
const RECENCY_MARGIN: f32 = 1.0;

/// 拼满读法的第二名要越过词典首选，逐字平均自然对数概率得比它高出这么多。首选的词频是模型没有的证据，dev 上 margin 0 在聊天集上变差；按事先写下的规则在 0、1 里选了 1。
const RUNNER_UP_MARGIN: f32 = 1.0;

/// 每种裁决点上挑战者要比首行高出多少（逐字平均自然对数）才换到首位；`None` 是没有这种裁决点。
fn contest_margin(kind: u8) -> Option<f32> {
    match kind {
        CONTEST_RECENCY => Some(RECENCY_MARGIN),
        CONTEST_RUNNER_UP => Some(RUNNER_UP_MARGIN),
        _ => None,
    }
}

/// The traits of the scheme behind `scheme`, which the Engine reports as its `SchemeType` ordinal. Only the placeholder snapshot of a failed refresh carries an ordinal no scheme has; each caller decides what that placeholder means, the way the ordinal comparisons this replaces did.
fn scheme_type(scheme: u8) -> Option<SchemeType> {
    SchemeType::from_u8(scheme)
}

/// Whether the runtime may reorder the scheme's candidate list (the sentence model and the runner-up demotion). A scheme whose selection goes straight to the document instead of being held as phrase progress (the Korean Hanja list) lists its table in frequency order, not readings of one sentence the model can compare, and that order is the one to keep.
pub fn reorders_candidates(scheme: u8) -> bool {
    scheme_type(scheme).is_none_or(SchemeType::holds_phrase_progress)
}

/// 方案的纠错行能不能被重排器提到首位。双拼的纠错整句不由重排器提到首位（#6034，引擎把它排在整句块之后），所以重排时它既不参与比较，也不让整张表失去词典命中的豁免；双拼触点纠错能不能坐首选只由引擎按 `TouchTuning::lead_margin` 决定（#10185，默认无穷大，即不坐）。全拼整串读不通时，纠错读法就是主读法，重排器照旧允许它领先；输入读得通、引擎另给的纠错整句由引擎按 `typo_lead_margin` 定座位，重排器不提它（`typo_sentence_beside_literal`）。
fn corrections_may_lead(scheme: u8) -> bool {
    scheme_type(scheme) != Some(SchemeType::Shuangpin)
}

/// 原地应用排列，`order` 将每个新座位映射到旧座位。
///
/// 按循环交换候选所有权，不复制元素。记录已访问座位，让每条循环只遍历一次；
/// 短列表用栈缓冲，大列表才申请辅助缓冲，恒等排列直接返回。
pub fn apply_order<T>(items: &mut [T], order: &[usize]) {
    debug_assert_eq!(items.len(), order.len());
    apply_order_with(order, |left, right| items.swap(left, right));
}

/// 按排列的循环调用交换操作，让调用方同步移动多个并行数组。
///
/// `order` 必须是新座位到旧座位的合法排列；交换操作涉及的数组长度必须与它一致。
/// 每条循环只遍历一次，共用访问标记缓冲，恒等排列不调用交换操作。
pub fn apply_order_with(order: &[usize], mut swap: impl FnMut(usize, usize)) {
    if order.iter().enumerate().all(|(seat, row)| seat == *row) {
        return;
    }
    let mut stack_visited = [false; 128];
    let mut heap_visited;
    let visited = if order.len() <= stack_visited.len() {
        &mut stack_visited[..order.len()]
    } else {
        heap_visited = vec![false; order.len()];
        &mut heap_visited[..]
    };
    for start in 0..order.len() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut current = start;
        while order[current] != start {
            let next = order[current];
            swap(current, next);
            visited[next] = true;
            current = next;
        }
    }
}

/// Keep the Engine-index mapping empty while the cached candidates are still in Engine order.
/// Reordering paths call this immediately before their first permutation, so ordinary keystrokes
/// avoid rebuilding an identity vector on every snapshot.
pub fn ensure_engine_order(engine_order: &mut Vec<usize>, count: usize) {
    if engine_order.len() == count {
        return;
    }
    engine_order.clear();
    engine_order.extend(0..count);
}

pub fn rotate_to_front<T>(items: &mut [T], index: usize) {
    items[..=index].rotate_right(1);
}

/// How far the context handed to a reranker moves at a time once it no longer fits the model.
pub const RERANK_CONTEXT_STEP: usize = 16;

/// The tail of the committed text a reranker should see, trimmed so the window holds still while a candidate grows.
///
/// The reranker keeps the model state for its prefix across keystrokes, keyed on the prefix tokens, and that cache is what keeps a keystroke inside a frame. It trims the context itself to leave room for the longest candidate, so left to do that, a context longer than the window slides by one character every time a candidate gains one — which is most keystrokes that complete a syllable. Every slide is a different prefix, so it reran the prefix and dropped every resume point with it, and a keystroke cost 20-45ms instead of about 1ms. Nothing showed it in a short test: the context only outgrows the window after a few sentences in one application, which is when "typing falls behind" was reported.
///
/// Trimming here, in steps, keeps the prefix identical until the longest candidate crosses a step, and the reranker then finds nothing further to trim because everything handed over already fits. A context that fits whole is handed over unchanged, so short contexts rank exactly as before.
pub fn rerank_context(context: &str, window: usize, longest: usize) -> &str {
    let room = window.saturating_sub(longest + 1);
    let keep = room / RERANK_CONTEXT_STEP * RERANK_CONTEXT_STEP;
    let count = context.chars().count();
    if count <= room {
        return context;
    }
    let skip = count - keep;
    context
        .char_indices()
        .nth(skip)
        .map_or("", |(start, _)| &context[start..])
}

/// 上屏后联想里词库续写的顺序：按模型给的「这一行接在上文后面」的总对数概率从高到低，返回排列（新座位 -> 旧座位），同分保持原来的词频顺序；少于两行或打不了分时为 `None`。
///
/// `context` 是上屏之前的文字连同刚上屏的词，按 [`rerank_context`] 截到模型读得下。用总分（逐字平均 × 字数）而不是逐字平均：联想要猜的是用户接下来真会打的那一段，总分让更长的续写为多出的每个字付出概率，不会因为每个字都顺就排到前面。离线可行性评测里同样从 32 条里挑，总分比平均分的首位命中高（单字上屏 16.7% 对 18.8%，两字以内 8.6% 对 9.3%），见 `.agents/notes/implemented/feature/2026-11-02-next-word-prediction-rerank.md`。
pub fn prediction_order(
    scorer: &mut dyn CandidateScorer,
    context: &str,
    rows: &[&str],
) -> Option<Vec<usize>> {
    if rows.len() < 2 {
        return None;
    }
    let longest = rows
        .iter()
        .map(|row| row.chars().count())
        .max()
        .unwrap_or(0);
    let context = rerank_context(context, scorer.context_length(), longest);
    let scores = scorer.log_probabilities(context, rows);
    if scores.len() != rows.len() {
        return None;
    }
    let totals: Vec<f32> = scores
        .iter()
        .zip(rows)
        .map(|(mean, row)| {
            let total = mean * row.chars().count() as f32;
            if total.is_nan() {
                f32::NEG_INFINITY
            } else {
                total
            }
        })
        .collect();
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|&left, &right| totals[right].total_cmp(&totals[left]));
    Some(order)
}

/// 排序决策读取的一行候选：文字、来源序号（`CandidateSource`），是否回答整个按键串，是否是纠错后的读法。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderRow<'a> {
    pub text: &'a str,
    pub source: u8,
    pub answers_key: bool,
    pub corrected: bool,
}

/// 词网格的自然对数总分，去掉共同的音节奖励；保留整数毫 nat 与字数避免快照丢失精度。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticEvidence {
    pub log_prob_milli_nats: i64,
    pub characters: usize,
}

impl StaticEvidence {
    fn mean(self) -> Option<f64> {
        (self.characters > 0)
            .then(|| self.log_prob_milli_nats as f64 / 1000.0 / self.characters as f64)
    }
}

/// 排序决策读取的候选行来源。连续的 `OrderRow` 数组和快照里的并行数组都可以实现它。
pub trait OrderRowSource<'a> {
    fn len(&self) -> usize;
    fn get(&self, index: usize) -> OrderRow<'a>;

    /// 没有静态证据的调用方继续使用原模型决策，不从来源或行位置猜一个分数。
    fn static_evidence(&self, _index: usize) -> Option<StaticEvidence> {
        None
    }

    /// 这一行所在的窄裁决点（`CONTEST_*`）；不标记的调用方永远是 `CONTEST_NONE`。
    fn contest(&self, _index: usize) -> u8 {
        CONTEST_NONE
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<'a> OrderRowSource<'a> for [OrderRow<'a>] {
    fn len(&self) -> usize {
        <[OrderRow<'a>]>::len(self)
    }

    fn get(&self, index: usize) -> OrderRow<'a> {
        self[index]
    }
}

impl<'a> OrderRowSource<'a> for Vec<OrderRow<'a>> {
    fn len(&self) -> usize {
        self.as_slice().len()
    }

    fn get(&self, index: usize) -> OrderRow<'a> {
        self[index]
    }
}

impl<'a, const N: usize> OrderRowSource<'a> for [OrderRow<'a>; N] {
    fn len(&self) -> usize {
        N
    }

    fn get(&self, index: usize) -> OrderRow<'a> {
        self[index]
    }
}

/// 为已有候选行附上可选的静态证据；长度不一致时整份证据失效。
pub struct ScoredOrderRows<'s, R: ?Sized> {
    rows: &'s R,
    evidence: &'s [Option<StaticEvidence>],
}

impl<'s, R: ?Sized> ScoredOrderRows<'s, R> {
    pub fn new(rows: &'s R, evidence: &'s [Option<StaticEvidence>]) -> Self {
        Self { rows, evidence }
    }
}

impl<'a, R: OrderRowSource<'a> + ?Sized> OrderRowSource<'a> for ScoredOrderRows<'_, R> {
    fn len(&self) -> usize {
        self.rows.len()
    }
    fn get(&self, index: usize) -> OrderRow<'a> {
        self.rows.get(index)
    }
    fn static_evidence(&self, index: usize) -> Option<StaticEvidence> {
        if self.evidence.len() != self.rows.len() {
            return None;
        }
        self.evidence.get(index).copied().flatten()
    }
    fn contest(&self, index: usize) -> u8 {
        self.rows.contest(index)
    }
}

/// 借用快照里的并行候选数组，避免排序前复制一份行数组。
pub struct ParallelOrderRows<'a> {
    texts: &'a [String],
    sources: &'a [u8],
    answers_key: &'a [bool],
    corrected: &'a [bool],
    static_evidence: Option<&'a [Option<StaticEvidence>]>,
    contests: Option<&'a [u8]>,
}

/// 找出实际交给模型比较的候选中最长的一行。
///
/// `Reranker::best_where` 只比较前九条回答当前按键的候选；比较集合之后的长候选不能影响上下文窗口，否则会让模型每次无关的长行出现时重建前缀。
fn longest_rerank_candidate<'a, R, F>(rows: &R, answers_key: &F, limit: usize) -> usize
where
    R: OrderRowSource<'a> + ?Sized,
    F: Fn(OrderRow<'a>) -> bool,
{
    (0..rows.len())
        .map(|index| rows.get(index))
        .filter(|row| answers_key(*row))
        .take(limit)
        .map(|row| row.text.chars().count())
        .max()
        .unwrap_or(0)
}

impl<'a> ParallelOrderRows<'a> {
    pub fn new(
        texts: &'a [String],
        sources: &'a [u8],
        answers_key: &'a [bool],
        corrected: &'a [bool],
    ) -> Self {
        Self {
            texts,
            sources,
            answers_key,
            corrected,
            static_evidence: None,
            contests: None,
        }
    }

    pub fn with_static_evidence(mut self, evidence: &'a [Option<StaticEvidence>]) -> Self {
        self.static_evidence = (evidence.len() == self.texts.len()).then_some(evidence);
        self
    }

    /// 附上引擎标的裁决点；长度和候选不一致时当作没有标记。
    pub fn with_contests(mut self, contests: &'a [u8]) -> Self {
        self.contests = (contests.len() == self.texts.len()).then_some(contests);
        self
    }
}

impl<'a> OrderRowSource<'a> for ParallelOrderRows<'a> {
    fn len(&self) -> usize {
        self.texts.len()
    }

    fn static_evidence(&self, index: usize) -> Option<StaticEvidence> {
        self.static_evidence?.get(index).copied().flatten()
    }

    fn contest(&self, index: usize) -> u8 {
        self.contests
            .and_then(|contests| contests.get(index).copied())
            .unwrap_or(CONTEST_NONE)
    }

    fn get(&self, index: usize) -> OrderRow<'a> {
        OrderRow {
            text: &self.texts[index],
            source: self.sources[index],
            answers_key: self.answers_key[index],
            corrected: self.corrected[index],
        }
    }
}

fn row_answers_key(scheme: u8, row: OrderRow<'_>) -> bool {
    row.answers_key
        && !CATALOG_SOURCES.contains(&row.source)
        && !AI_SEAT_SOURCES.contains(&row.source)
        && row.source != MIXED_JAPANESE_SOURCE
        && (corrections_may_lead(scheme) || !row.corrected)
}

/// 输入读得通时（`typo_sentence_beside_literal`），纠错整句在重排器的混合分（逐字自然对数）上要比最好的字面读法高出这么多，才能被提到首位。模型常觉得改出来的句子更通顺：不设门槛时 convert_eval 10396 条原句首选 0.858 → 0.843（「我的班不大」成了「我的办不到」），完全不让重排器提纠错句时打错集（5285 条）首选掉到 0.026，比没有合法拼音纠错的 develop（0.067）还低，因为纠错句多半靠重排器才坐上首位。扫 0、0.3、0.6、1.0、1.5：原句首选 0.843、0.850、0.854、0.857、0.858，打错集首选 0.212、0.210、0.203、0.190、0.169。1.5 时原句相对 develop 丢 5 条、得 9 条，1.0 时丢 16 条、得 9 条。
pub(crate) const CORRECTION_RERANK_MARGIN: f64 = 1.5;

fn any_corrected_key<'a, R: OrderRowSource<'a> + ?Sized>(scheme: u8, rows: &'a R) -> bool {
    corrections_may_lead(scheme)
        && (0..rows.len()).any(|index| rows.get(index).corrected)
        && !typo_sentence_beside_literal(scheme, rows)
}

/// 表里既有字面整句读法、又有纠错整句：输入读得通，引擎另给了打错时的改正（`quanpin::typo_edges`）。纠错句坐第几位只由引擎按它胜过字面整句多少决定（`LatticeOptions::typo_lead_margin`、`quanpin::dictionary::TYPO_SECOND_SEAT_MARGIN`）；重排器只在字面读法之间比较，不把纠错句提到首位，纠错句也不撤销词典首选的豁免。放任重排器比较时，模型常觉得改出来的句子更通顺，打对的输入就被改了字：一万多条原句里有 175 条丢了首选（「他的工作非常忙」成了「非常慢」）。整串读不通、词网格读法全是纠错读法时不算，那时纠错读法就是主读法，照旧让重排器比较。
fn typo_sentence_beside_literal<'a, R: OrderRowSource<'a> + ?Sized>(
    scheme: u8,
    rows: &'a R,
) -> bool {
    if !corrections_may_lead(scheme) {
        return false;
    }
    let mut literal = false;
    let mut corrected = false;
    for index in 0..rows.len() {
        let row = rows.get(index);
        if row.source == LATTICE_SOURCE {
            if row.corrected {
                corrected = true;
            } else {
                literal = true;
            }
            if literal && corrected {
                return true;
            }
        }
    }
    false
}

/// 判断 `rerank_pick` 是否需要运行模型，提前拒绝时宿主无需把停顿模型移出会话线程。
///
/// 韩文汉字候选按单音节词频排表，不能用中文模型评分。五笔码表回答的列表先排表码行，
/// 再追加拼音纠错行；若让纠错规则撤销表码首选的豁免，就会把 `dyn` 的「态」挤到
/// `dynn` 的「太快」之后。只有完全由拼音兜底回答的五笔列表才按拼音重排。
pub fn rerank_needs_model<'a, R: OrderRowSource<'a> + ?Sized>(
    scheme: u8,
    answered_by_pinyin_fallback: bool,
    rows: &'a R,
) -> bool {
    if !reorders_candidates(scheme) {
        return false;
    }
    if scheme_type(scheme) == Some(SchemeType::Wubi) && !answered_by_pinyin_fallback {
        return false;
    }
    if rows.len() < 2 {
        return false;
    }
    let leader = rows.get(0);
    // 引擎让英文或日文混输行领先的列表原样保留，见 `ENGLISH_SOURCE`、`MIXED_JAPANESE_SOURCE`。
    if matches!(leader.source, ENGLISH_SOURCE | MIXED_JAPANESE_SOURCE) {
        return false;
    }
    // 首行不回答当前按键时，上游同样拒绝比较，无需准备模型输入。
    if !row_answers_key(scheme, leader) {
        return false;
    }
    // 引擎让纠错整句坐了首位（胜过字面整句很多），不再交给模型。
    if leader.corrected && typo_sentence_beside_literal(scheme, rows) {
        return false;
    }
    if contest_rows(scheme, rows, &mut [0; CONTEST_LIMIT]).is_some() {
        return true;
    }
    // 词典命中携带当前按键的词频；全拼出现任何纠错行时，这份依据不再可信，
    // 整表撤销豁免。双拼纠错行不许领先，也不撤销词典首选的豁免。
    let corrected_key = any_corrected_key(scheme, rows);
    let trusted_leader = DICTIONARY_SOURCES.contains(&leader.source) && !corrected_key;
    !trusted_leader
}

/// rerank() 的决策部分：返回应当移到首位的行下标；None 表示不动。
///
/// `scheme` 是 `SchemeType` 序号；不允许重排的方案（韩文汉字表）返回 None，五笔且 `!answered_by_pinyin_fallback` 时返回 None，行数 < 2 时返回 None，首行是英文时返回 None。打分器要续用跨按键的缓存，因此取 `&mut`。
pub fn rerank_pick<'a, R: OrderRowSource<'a> + ?Sized>(
    reranker: &mut dyn CandidateScorer,
    context: &str,
    scheme: u8,
    answered_by_pinyin_fallback: bool,
    rows: &'a R,
) -> Option<usize> {
    let pool = reranker.sentence_pool();
    rerank_pick_within(
        reranker,
        context,
        scheme,
        answered_by_pinyin_fallback,
        rows,
        pool,
    )
}

/// 同 [`rerank_pick`]，但静态证据分支比较几条由调用方给（`pool`，取它与九条中较大的那个，最多 [`MAX_SENTENCE_POOL`]），而不是打分器报的整句池。引擎只在 26 键全拼上多解读法（`Session::set_sentence_pool`）；九宫格和 14 键的列表来自几十条切分，比较整池只会多花时间、不在逐键耗时的估计里，runtime 在键位网格上传 0。
pub fn rerank_pick_within<'a, R: OrderRowSource<'a> + ?Sized>(
    reranker: &mut dyn CandidateScorer,
    context: &str,
    scheme: u8,
    answered_by_pinyin_fallback: bool,
    rows: &'a R,
    pool: usize,
) -> Option<usize> {
    if !rerank_needs_model(scheme, answered_by_pinyin_fallback, rows) {
        return None;
    }
    let mut contested = [0; CONTEST_LIMIT];
    if let Some((count, margin)) = contest_rows(scheme, rows, &mut contested) {
        return pick_contest(reranker, context, rows, &contested[..count], margin);
    }
    let beside_literal = typo_sentence_beside_literal(scheme, rows);
    let answers_key = |row: OrderRow<'a>| row_answers_key(scheme, row);
    let literal_answers_key =
        |row: OrderRow<'a>| row_answers_key(scheme, row) && !(beside_literal && row.corrected);
    let corrected_key = any_corrected_key(scheme, rows);
    let limit = pool.clamp(STATIC_RERANK_LIMIT, MAX_SENTENCE_POOL);
    // 只有参与比较的候选需要占用模型上下文窗口。
    let longest = longest_rerank_candidate(rows, &answers_key, limit);
    let context = rerank_context(context, reranker.context_length(), longest);
    if rows.static_evidence(0).is_some() {
        let mut considered = [0; MAX_SENTENCE_POOL];
        let mut statik = [0.0; MAX_SENTENCE_POOL];
        let mut subset = [""; MAX_SENTENCE_POOL];
        if let Some(count) = fill_static_rerank_inputs(
            rows,
            answers_key,
            &mut considered[..limit],
            &mut statik[..limit],
            &mut subset[..limit],
        ) {
            let weight = reranker.static_evidence_weight();
            let neural = reranker.log_probabilities(context, &subset[..count]);
            if neural.len() == count {
                // 词网格逐字分与模型逐字分按打分器给的权重混合，两侧均为自然对数。
                let combined = |index: usize| {
                    statik[index] * weight + f64::from(neural[index]) * (1.0 - weight)
                };
                let mut best = 0;
                for index in 1..count {
                    if combined(index) > combined(best) {
                        best = index;
                    }
                }
                // 输入读得通时，纠错整句要比最好的字面读法高出 CORRECTION_RERANK_MARGIN 才能被提上来，见常量说明。
                if beside_literal && rows.get(considered[best]).corrected {
                    let literal = (0..count)
                        .filter(|&index| !rows.get(considered[index]).corrected)
                        .max_by(|&a, &b| combined(a).total_cmp(&combined(b)));
                    if let Some(literal) = literal.filter(|&literal| {
                        combined(best) < combined(literal) + CORRECTION_RERANK_MARGIN
                    }) {
                        best = literal;
                    }
                }
                // Readings that differ from the pick only in 他/她/它 are decided by the model alone; see differs_only_in_third_person.
                let leader = subset[best];
                for index in 0..count {
                    if neural[index] > neural[best]
                        && differs_only_in_third_person(subset[index], leader)
                    {
                        best = index;
                    }
                }
                return (best != 0).then(|| considered[best]);
            }
        }
    }
    with_row_texts(rows, |texts| {
        reranker.best_where(context, texts, &|index| CandidateFacts {
            answers_key: literal_answers_key(rows.get(index)),
            trusted_dictionary_hit: DICTIONARY_SOURCES.contains(&rows.get(index).source)
                && !corrected_key,
        })
    })
}

/// 两条读法是否只在第三人称代词「他」「她」「它」上不同（字数相同，其余逐字相同，至少有一处不同）。
///
/// 这几个字同音，词网格分不出来，只能按词频排，而词频说的是语料里哪个更常见，不是这句话指的是谁；该用哪一个只有上文知道。整句池里两条读法只差代词时，混合分里那 0.3 的词网格分只会把模型的判断往高频的「他」那边拉：2026-11-04 的聊天集里，「你记得跟她说谢谢」「她说上次你修好的那个灯」模型都选对了，混合后分别以 0.002 和 0.004 输给了「他」。所以这种情况下只看模型分。在 366 段带上文、含代词的聊天句（合成聊天开发集加单人聊天集）上，6x256 从 203 到 216（改对 20、改错 7，p = 0.019），8x448 从 226 到 235（改对 13、改错 4，p = 0.049）；整套单人聊天集（330 段）6x256 改对 2、改错 0，8x448 改对 0、改错 1；sentences 四套 1,580 句两个模型都一句没变。
pub fn differs_only_in_third_person(a: &str, b: &str) -> bool {
    let pronoun = |c: char| matches!(c, '他' | '她' | '它');
    let mut differs = false;
    let mut left = a.chars();
    let mut right = b.chars();
    loop {
        match (left.next(), right.next()) {
            (None, None) => return differs,
            (Some(x), Some(y)) if x == y => {}
            (Some(x), Some(y)) if pronoun(x) && pronoun(y) => differs = true,
            _ => return false,
        }
    }
}

/// 一句话开头单打一个音节、首选却是句尾语气词（`ma` 的「吗」、`ba` 的「吧」）时，该提到首位的那一行；不是这种情形时为 `None`。
///
/// 「吗」的词频是「妈」的 51 倍、「吧」比「爸」高两成，可这些词频几乎全来自句尾，句首单打 `ma` 想要的不会是「吗」。聊天记录（typing-chat-v1）里每段会话开头的「妈」52 次全被打成「吗」，而逐键模型碰不到它：词典首选是可信的，[`rerank_needs_model`] 不交给模型比较。这里只在 `lattice::decode::opens_sentence` 为真时动，首选让给第一页里第一个不是语气词、同样回答按键的单字词典行；挂着打分器时由它在这些单字里挑，每个字后面接上 [`OPENING_PAUSE`] 再比。句中照常（「好 | 吧」先上屏「好」再打 `ba`，要的就是「吧」）。
pub fn opening_particle_pick<'a, R: OrderRowSource<'a> + ?Sized>(
    reranker: Option<&mut (dyn CandidateScorer + '_)>,
    context: &str,
    scheme: u8,
    rows: &'a R,
) -> Option<usize> {
    if !reorders_candidates(scheme) || scheme_type(scheme) == Some(SchemeType::Wubi) {
        return None;
    }
    if rows.len() < 2 || !crate::lattice::decode::opens_sentence(context) {
        return None;
    }
    let single = |text: &str| {
        let mut chars = text.chars();
        chars.next().filter(|_| chars.next().is_none())
    };
    let leader = rows.get(0);
    let particle = |c: char| crate::lattice::decode::SENTENCE_ONLY_FINAL_PARTICLES.contains(&c);
    if !DICTIONARY_SOURCES.contains(&leader.source)
        || !row_answers_key(scheme, leader)
        || !single(leader.text).is_some_and(particle)
    {
        return None;
    }
    let mut considered = [0; COMPARED_LIMIT];
    let mut texts = [""; COMPARED_LIMIT];
    let mut count = 0;
    for index in 1..rows.len().min(COMPARED_LIMIT + 1) {
        let row = rows.get(index);
        if count < COMPARED_LIMIT
            && DICTIONARY_SOURCES.contains(&row.source)
            && row_answers_key(scheme, row)
            && single(row.text).is_some_and(|c| !particle(c))
        {
            considered[count] = index;
            texts[count] = row.text;
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    let Some(reranker) = reranker else {
        return Some(considered[0]);
    };
    // Each character is scored followed by a pause (OPENING_PAUSE), as the word it is about to stand for; see OPENING_PAUSE for why.
    let mut buffers = [[0u8; 8]; COMPARED_LIMIT];
    let mut lengths = [0usize; COMPARED_LIMIT];
    for slot in 0..count {
        let text = texts[slot];
        buffers[slot][..text.len()].copy_from_slice(text.as_bytes());
        buffers[slot][text.len()..text.len() + OPENING_PAUSE.len()]
            .copy_from_slice(OPENING_PAUSE.as_bytes());
        lengths[slot] = text.len() + OPENING_PAUSE.len();
    }
    let mut paused = [""; COMPARED_LIMIT];
    for slot in 0..count {
        paused[slot] = std::str::from_utf8(&buffers[slot][..lengths[slot]]).unwrap_or(texts[slot]);
    }
    let context = rerank_context(
        context,
        reranker.context_length(),
        1 + OPENING_PAUSE.chars().count(),
    );
    let scores = reranker.log_probabilities(context, &paused[..count]);
    if scores.len() != count {
        return Some(considered[0]);
    }
    let best = (1..count).fold(0, |best, slot| {
        if scores[slot] > scores[best] {
            slot
        } else {
            best
        }
    });
    Some(considered[best])
}

/// 句首单字交给逐键模型比较时，接在每个字后面的停顿。
///
/// 逐键模型给一个字的分是「下一个字是它」的概率，这个字作为更长的词开头的情形也算在里面：消息开头「马上……」「麻烦你……」很常见，「马」「麻」就沾了这些词的光，可用户单打 `ma` 再按空格，要的是一个单独成词的字。字后面接一个逗号再比，比的就是「这个字单独说完、停一下」的概率，「妈，」远比「麻，」常见。2026-11-04 换上聊天上文训练的 6x256（chinese-ime-lm rerank-v2）后，Mac 自动打字验收里会话开头的「妈」7 次有 3 次成了「麻」或「马」（模型只差 0.09 到 0.44）；离线用同样的上文重放，接上逗号再比，新旧 6x256 都是 7 次全对。只比单字不加停顿时，旧模型也只是险胜（领先 0.12 到 2.01）。
pub const OPENING_PAUSE: &str = "，";

/// 一次窄裁决最多比较几行：首页放得下的行数。
const CONTEST_LIMIT: usize = 9;

/// 首行带裁决标记、这种裁决点开着、同种标记又回答按键的行至少两条时，把它们的下标写进 `out`（首行在最前），返回条数和 margin。
fn contest_rows<'a, R: OrderRowSource<'a> + ?Sized>(
    scheme: u8,
    rows: &'a R,
    out: &mut [usize; CONTEST_LIMIT],
) -> Option<(usize, f32)> {
    let kind = rows.contest(0);
    if kind == CONTEST_NONE {
        return None;
    }
    let margin = contest_margin(kind)?;
    let mut count = 0;
    for index in 0..rows.len() {
        if count == CONTEST_LIMIT {
            break;
        }
        if rows.contest(index) == kind && row_answers_key(scheme, rows.get(index)) {
            out[count] = index;
            count += 1;
        }
    }
    (count >= 2 && out[0] == 0).then_some((count, margin))
}

/// 只在被标记的几行之间让模型挑：挑战者的逐字平均对数概率比首行高出 `margin` 才提到首位。
fn pick_contest<'a, R: OrderRowSource<'a> + ?Sized>(
    reranker: &mut dyn CandidateScorer,
    context: &str,
    rows: &'a R,
    contested: &[usize],
    margin: f32,
) -> Option<usize> {
    let mut texts = [""; CONTEST_LIMIT];
    let mut longest = 0;
    for (slot, &index) in contested.iter().enumerate() {
        texts[slot] = rows.get(index).text;
        longest = longest.max(texts[slot].chars().count());
    }
    let texts = &texts[..contested.len()];
    let context = rerank_context(context, reranker.context_length(), longest);
    let scores = reranker.log_probabilities(context, texts);
    if scores.len() != texts.len() {
        return None;
    }
    let mut best = 0;
    for slot in 1..scores.len() {
        if scores[slot] > scores[best] {
            best = slot;
        }
    }
    (best != 0 && scores[best] - scores[0] > margin).then(|| contested[best])
}

fn fill_static_rerank_inputs<'a, R, F>(
    rows: &'a R,
    answers_key: F,
    considered: &mut [usize],
    statik: &mut [f64],
    subset: &mut [&'a str],
) -> Option<usize>
where
    R: OrderRowSource<'a> + ?Sized,
    F: Fn(OrderRow<'a>) -> bool,
{
    let mut count = 0;
    // 只和首选同样多字的读法比较：全拼整句一个字一个音节，字数不同就是同一串字母的另一种切法（`xian` 切成 `xi'an`）。那种读法要多出一个音节，词网格分已经说明它不像，模型却只看字，常把「要上线」换成「要上西安」。
    // 比较的窗口不变（前 `considered.len()` 条回答按键的行），只在窗口里跳过字数不同的：跳过的不能让窗口外更靠后的读法补进来。
    let mut width = None;
    let mut window = 0;
    for index in 0..rows.len() {
        let row = rows.get(index);
        if !answers_key(row) {
            continue;
        }
        if window == considered.len() {
            break;
        }
        window += 1;
        let characters = row.text.chars().count();
        if *width.get_or_insert(characters) != characters {
            continue;
        }
        considered[count] = index;
        statik[count] = rows.static_evidence(index)?.mean()?;
        subset[count] = row.text;
        count += 1;
    }
    (count >= 2).then_some(count)
}

fn with_row_texts<'a, R, T>(rows: &'a R, operation: impl FnOnce(&[&'a str]) -> T) -> T
where
    R: OrderRowSource<'a> + ?Sized,
{
    if rows.len() <= SMALL_RERANK_TEXTS {
        let mut texts = [""; SMALL_RERANK_TEXTS];
        for (index, text) in texts.iter_mut().enumerate().take(rows.len()) {
            *text = rows.get(index).text;
        }
        operation(&texts[..rows.len()])
    } else {
        let mut texts = Vec::with_capacity(rows.len());
        texts.extend((0..rows.len()).map(|index| rows.get(index).text));
        operation(&texts)
    }
}

/// demote_runner_up_readings() 的决策部分：返回 order（新座位 -> 旧座位）；恒等排列时返回 None。
///
/// Keep the leading sentence readings together near the top and move the rest of them behind the list.
///
/// The lattice searches several readings of the whole key so that something can choose between them. Leaving all of them at the front fills the candidate page with near-duplicate sentences and pushes the short candidates a user actually wants off it, which is why the search used to be pinned to a single path.
///
/// For a sentence of three or more characters the first page keeps three readings, seated together right after the first one, and only the rest are moved back. One reading was too few once the Google fallback stopped holding a second sentence seat: on sentences-neutral-v1 top5 fell to 0.615 and on sentences-v2 to 0.269 with the correct sentence sitting at reading two or three, and keeping three lifts them to 0.839 and 0.763 while quanpin-words-v1 top5 moves only from 0.940 to 0.938. Shorter readings still keep one, because two-syllable keys are where the runner-ups (倪好, 你号, 你毫 after 你好) push dictionary words off the page, and keeping three there costs words top5 two points.
///
/// They are moved rather than removed. Deleting them threw away the model's later choices, so a reading the model ranked fourth was unreachable even when it was right.
///
/// Only lattice readings are touched. An earlier version of this keyed on "any source that is not a dictionary", which is wrong twice over: a source number says which code produced a candidate, not that two candidates are spellings of one answer, and most of the other sources are plural by design — English words, emoji, kaomoji, quick phrases and AI suggestions all arrive as lists, and that version silently dropped all but one of each.
pub fn runner_up_order<'a, R: OrderRowSource<'a> + ?Sized>(
    scheme: u8,
    rows: &R,
) -> Option<Vec<usize>> {
    runner_up_order_with(scheme, rows, &[])
}

/// [`runner_up_order`]，`touch_supported[i]` 为真的纠错行不算整句读法、留在引擎给的座位上。
///
/// 26 键双拼带着触点查出的表里，落点支持的纠错整句是按落点计过价的改正，不是同一串按键的又一种读法；把它当次选读法挪到表尾，它就几乎从不出现在第一页（双拼触点评测里能找到原句的纠错整句有九成排在表尾）。按行而不是按整张表判断：同一张表里还可能有别的纠错行（模糊音替代读法按平面表查出的纠错整句），它们没过落点的闸，照旧沉底。`touch_supported` 与 `rows` 等长，或者为空表示没有这样的行；长度对不上时整列作废。没有触点的表（桌面、实体键盘）照旧，结果逐字节不变。
pub fn runner_up_order_with<'a, R: OrderRowSource<'a> + ?Sized>(
    scheme: u8,
    rows: &R,
    touch_supported: &[bool],
) -> Option<Vec<usize>> {
    runner_up_order_pooled(scheme, rows, touch_supported, 0)
}

/// [`runner_up_order_with`]，`pool` 是这张表解码时引擎拿到的整句池（`Session::set_sentence_pool`，runtime 按挂着的逐键打分器给，键位网格上是 0）。
///
/// 池超过默认条数时，全拼引擎把另一种切分的整句读法挪进主切分整句的座位，好让模型比较它们（`quanpin::series::defer_pooled_sentences`）；那种切分的音节数不同时（wenjian 的 wen'jian 和 wen'ji'an），读法字数也不同，只按第一条读法的字数认读法就一条也认不出来，模型比完以后它们原地留在词库词前面（#10998）。所以这时字数不同、同样读满整串的词网格行也算读法：排在最前的那一条坐到它后面最后一个读满整串的词库词之后，其余挪到表尾；第一条读法后面留的 `keep - 1` 条仍只从同字数的读法里取。没有池时引擎本来就把另一种切分的读法排在主切分的整串词之后，那里的第一条常常就是用户要的句子（chulia 的「处理啊」在第 6 位），这时照旧只按字数认，结果逐字节不变。取舍和评测见 `.agents/notes/implemented/bug-fix/2026-11-06-pooled-readings-of-other-segmentations.md`。
pub fn runner_up_order_pooled<'a, R: OrderRowSource<'a> + ?Sized>(
    scheme: u8,
    rows: &R,
    touch_supported: &[bool],
    pool: usize,
) -> Option<Vec<usize>> {
    runner_up_order_pinned(scheme, rows, touch_supported, pool, None)
}

/// 引擎紧挨着第一条整句读法安放的全拼纠错整句现在在哪一格：胜过字面整句很多的坐在它前面（首位，`lattice::decode::LatticeOptions::typo_lead_margin`），明显胜过的坐在它后面（`quanpin::dictionary::TYPO_SECOND_SEAT_MARGIN`），其余的排在整句块之后。`engine_seats[i]` 是当前第 i 格在引擎原表里的座位；重排器把别的读法提到首位以后，那一行就不再紧跟当前的第一条读法，只能按引擎座位认。双拼的纠错句由引擎排在整句块之后（#6034），没有这个座位。长度对不上时为 `None`。
pub fn engine_seated_correction<'a, R: OrderRowSource<'a> + ?Sized>(
    scheme: u8,
    rows: &R,
    engine_seats: &[usize],
) -> Option<usize> {
    let count = rows.len();
    if !corrections_may_lead(scheme) || engine_seats.len() != count {
        return None;
    }
    let first = (0..count)
        .filter(|&index| {
            let row = rows.get(index);
            row.source == LATTICE_SOURCE && !row.corrected
        })
        .map(|index| engine_seats[index])
        .min()?;
    (0..count).find(|&index| {
        let row = rows.get(index);
        row.source == LATTICE_SOURCE
            && row.corrected
            && (engine_seats[index] + 1 == first || engine_seats[index] == first + 1)
    })
}

/// [`runner_up_order_pooled`]，`pinned` 是 [`engine_seated_correction`] 找到的那一行：它不按读法降位，在第一条字面读法之前的坐首位、之后的坐第二位，并占掉一条留下的读法名额；读法按第一条字面读法的字数认。它不是同一串字母的又一种读法，而是对打错的改正；按读法降位会把它和其余读法一起挪走（字数不同时排到留下的读法后面，字数相同又超出名额时挪到表尾），手机候选栏上就看不到了。`pinned` 不是纠错的整句行时不起作用。
pub fn runner_up_order_pinned<'a, R: OrderRowSource<'a> + ?Sized>(
    scheme: u8,
    rows: &R,
    touch_supported: &[bool],
    pool: usize,
    pinned: Option<usize>,
) -> Option<Vec<usize>> {
    // The lattice runs from two syllables (a single syllable is never decoded), so a shorter candidate reached the list some other way and is not a reading of the same sentence. Japanese kana are the case that proves it: あ and ア are both Generated and both one character. Two rather than three because two-syllable keys are where the lattice's runner-up readings otherwise fill the first page ahead of dictionary words: on quanpin-words-v1 this moves two-syllable top5 from 0.883 to 0.924 with top1 unchanged.
    const SENTENCE_SYLLABLES: usize = 2;
    // From this many characters a reading is a sentence rather than a word, and the page keeps `SENTENCE_READINGS` of them.
    const LONG_SENTENCE_CHARACTERS: usize = 3;
    const SENTENCE_READINGS: usize = 3;

    // Korean Hanja rows are not lattice readings, and their table order is the one to keep.
    if !reorders_candidates(scheme) {
        return None;
    }
    let count = rows.len();
    if count < 2 {
        return None;
    }
    let touch_supported = if touch_supported.len() == count {
        touch_supported
    } else {
        &[]
    };
    let lattice = |index: usize, row: &OrderRow<'_>| {
        row.source == LATTICE_SOURCE
            && !(row.corrected && touch_supported.get(index).copied().unwrap_or(false))
    };
    let pinned = pinned.filter(|&index| {
        index < count && {
            let row = rows.get(index);
            row.source == LATTICE_SOURCE && row.corrected && lattice(index, &row)
        }
    });
    // 读法按第一条字面读法的字数认：坐首位的纠错句字数常和字面读法不同，按它认就一条字面读法也认不出来，整页都是不成句的读法。
    let width = (0..rows.len())
        .map(|index| (index, rows.get(index)))
        .find(|(index, row)| Some(*index) != pinned && lattice(*index, row))
        .map(|(_, row)| row.text.chars().count())?;
    if width < SENTENCE_SYLLABLES {
        return None;
    }
    let keep = if width >= LONG_SENTENCE_CHARACTERS {
        SENTENCE_READINGS
    } else {
        1
    };
    // Every lattice reading of the full key, in list order. The first stays where it is, the next `keep - 1` are seated right behind it, and the rest go to the back in their existing order. `keep` is at most three, so rescanning the rows costs less than allocating separate reading and membership arrays.
    let keep = if pinned.is_some() {
        (keep - 1).max(1)
    } else {
        keep
    };
    let seated = |index: usize| {
        let row = rows.get(index);
        Some(index) != pinned && lattice(index, &row) && row.text.chars().count() == width
    };
    // 整句池让引擎挪过座位时（见 [`runner_up_order_pooled`]），字数不同、同样读满整串的词网格行是另一种切分的读法。只有全拼和五笔的拼音回退走全拼引擎的整句池。
    let pooled = pool > crate::lattice::decode::SENTENCE_PATHS
        && matches!(
            scheme_type(scheme),
            Some(SchemeType::Quanpin | SchemeType::Wubi)
        );
    let other_reading = |index: usize| {
        let row = rows.get(index);
        let characters = row.text.chars().count();
        pooled
            && Some(index) != pinned
            && row.answers_key
            && lattice(index, &row)
            && characters >= SENTENCE_SYLLABLES
            && characters != width
    };
    // 另一种切分里排在最前的读法留一条，坐到它后面最后一个读满整串的词库词之后，也就是没有整句池时引擎给它的位置；其余的和多出来的读法一起挪到表尾。
    let first_other = (0..count).find(|&index| other_reading(index));
    let anchor = first_other.and_then(|first| {
        (first + 1..count).rev().find(|&index| {
            let row = rows.get(index);
            row.answers_key && !row.corrected && DICTIONARY_SOURCES.contains(&row.source)
        })
    });
    // 按新顺序逐个交出旧座位。先走一遍只判断是不是恒等排列：常见的整句候选已经连续时直接返回，不为最终排列分配缓冲。
    let walk = |emit: &mut dyn FnMut(usize)| {
        let mut seated_seen = 0;
        for index in 0..count {
            if seated(index) {
                if seated_seen == 0 {
                    // 纠错句在第一条读法之前就坐首位，之后就坐第二位。
                    match pinned {
                        Some(pinned) if pinned < index => {
                            emit(pinned);
                            emit(index);
                        }
                        Some(pinned) => {
                            emit(index);
                            emit(pinned);
                        }
                        None => emit(index),
                    }
                    let mut kept = 1;
                    for next in index + 1..count {
                        if kept == keep {
                            break;
                        }
                        if seated(next) {
                            emit(next);
                            kept += 1;
                        }
                    }
                }
                seated_seen += 1;
                continue;
            }
            if Some(index) == pinned {
                continue;
            }
            if Some(index) == first_other {
                if anchor.is_none() {
                    emit(index);
                }
                continue;
            }
            if other_reading(index) {
                continue;
            }
            emit(index);
            if Some(index) == anchor {
                emit(first_other.expect("an anchor follows the first other reading"));
            }
        }
        let mut seated_seen = 0;
        for index in 0..count {
            if seated(index) {
                if seated_seen >= keep {
                    emit(index);
                }
                seated_seen += 1;
            } else if other_reading(index) && Some(index) != first_other {
                emit(index);
            }
        }
    };
    let mut output = 0;
    let mut identity = true;
    walk(&mut |index| {
        identity &= output == index;
        output += 1;
    });
    debug_assert_eq!(output, count);
    if identity {
        return None;
    }
    let mut order = Vec::with_capacity(count);
    walk(&mut |index| order.push(index));
    debug_assert_eq!(order.len(), count);
    Some(order)
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_third_person_pronouns_may_differ() {
        assert!(super::differs_only_in_third_person(
            "你记得跟她说谢谢",
            "你记得跟他说谢谢"
        ));
        assert!(super::differs_only_in_third_person("它不让碰", "她不让碰"));
        // 同一条读法不算「只差代词」。
        assert!(!super::differs_only_in_third_person("他说", "他说"));
        // 别的字不同、字数不同都不算。
        assert!(!super::differs_only_in_third_person("他说谢谢", "她说射射"));
        assert!(!super::differs_only_in_third_person("他说", "他说了"));
        assert!(!super::differs_only_in_third_person("他们", "你们"));
    }

    use super::*;

    fn row(text: &str, source: u8) -> OrderRow<'_> {
        OrderRow {
            text,
            source,
            answers_key: true,
            corrected: false,
        }
    }

    /// 只给「目标」高分的打分器，报 `pool` 作整句池。
    struct Favouring {
        target: &'static str,
        pool: usize,
    }

    impl CandidateScorer for Favouring {
        fn context_length(&self) -> usize {
            64
        }

        fn log_probabilities(&mut self, _context: &str, candidates: &[&str]) -> Vec<f32> {
            candidates
                .iter()
                .map(|text| if *text == self.target { 0.0 } else { -9.0 })
                .collect()
        }

        fn sentence_pool(&self) -> usize {
            self.pool
        }
    }

    /// 静态证据分支比较几条跟着整句池走：池 0 或 9 只比较前九条，排在第 12 条的读法挑不上；池 12 时挑得上。`rerank_pick` 用打分器报的池，`rerank_pick_within` 用调用方给的（键位网格上传 0）。
    #[test]
    fn the_static_comparison_covers_the_sentence_pool() {
        const TEXTS: [&str; 12] = [
            "甲", "乙", "丙", "丁", "戊", "己", "庚", "辛", "壬", "癸", "子", "丑",
        ];
        let rows: Vec<OrderRow<'_>> = TEXTS.iter().map(|text| row(text, LATTICE_SOURCE)).collect();
        let evidence: Vec<Option<StaticEvidence>> = (0..TEXTS.len())
            .map(|index| {
                Some(StaticEvidence {
                    log_prob_milli_nats: -1000 - 100 * index as i64,
                    characters: 1,
                })
            })
            .collect();
        let scored = ScoredOrderRows::new(rows.as_slice(), &evidence);
        let scorer = |pool| Favouring {
            target: "丑", pool
        };
        assert_eq!(rerank_pick(&mut scorer(0), "", 0, false, &scored), None);
        assert_eq!(rerank_pick(&mut scorer(9), "", 0, false, &scored), None);
        assert_eq!(
            rerank_pick(&mut scorer(12), "", 0, false, &scored),
            Some(11)
        );
        assert_eq!(
            rerank_pick_within(&mut scorer(12), "", 0, false, &scored, 0),
            None
        );
        // 第九条以内的读法不论池多大都在比较里。
        let mut early = Favouring {
            target: "壬",
            pool: 0,
        };
        assert_eq!(rerank_pick(&mut early, "", 0, false, &scored), Some(8));
    }

    #[test]
    fn rerank_context_that_fits_is_handed_over_whole() {
        // Short contexts are what the sentence eval measured, so they must reach the model untouched.
        assert_eq!(rerank_context("你好世界", 64, 10), "你好世界");
        assert_eq!(rerank_context("", 64, 10), "");
    }

    #[test]
    fn rerank_context_holds_still_while_a_candidate_grows() {
        // A long context used to slide by one character per keystroke, which made the reranker rerun its prefix every time.
        let context: String = "今天天气很好我们一起去公园散步".repeat(8);
        let windows: Vec<&str> = (1..=40)
            .map(|longest| rerank_context(&context, 64, longest))
            .collect();
        let mut distinct = windows.clone();
        distinct.dedup();
        assert!(
            distinct.len() <= 64 / RERANK_CONTEXT_STEP + 1,
            "{}",
            distinct.len()
        );
        for (longest, window) in (1..=40).zip(&windows) {
            let count = window.chars().count();
            assert!(count + longest < 64, "longest {longest} kept {count}");
            assert_eq!(count % RERANK_CONTEXT_STEP, 0);
            // Always the most recent text, never the start of it.
            assert!(context.ends_with(window));
        }
    }

    #[test]
    fn rerank_context_cuts_on_a_character_boundary_and_can_empty() {
        let context = "a中b文".repeat(40);
        let window = rerank_context(&context, 64, 20);
        assert_eq!(window.chars().count(), 32);
        assert!(context.ends_with(window));
        // A candidate that fills the window leaves no room, and the answer is an empty context rather than a panic.
        assert_eq!(rerank_context(&context, 64, 70), "");
    }

    #[test]
    fn rerank_context_length_ignores_candidates_beyond_model_budget() {
        let rows = [
            row("甲", LATTICE_SOURCE),
            row("乙", LATTICE_SOURCE),
            row("丙", LATTICE_SOURCE),
            row("丁", LATTICE_SOURCE),
            row("戊", LATTICE_SOURCE),
            row("己", LATTICE_SOURCE),
            row("庚", LATTICE_SOURCE),
            row("辛", LATTICE_SOURCE),
            row("壬", LATTICE_SOURCE),
            row("这是没有进入模型比较集合的长候选", LATTICE_SOURCE),
        ];
        assert_eq!(
            longest_rerank_candidate(&rows, &|row| row.answers_key, STATIC_RERANK_LIMIT),
            1
        );
    }

    #[test]
    fn in_place_order_applies_candidate_permutations() {
        let mut values = vec!["zero", "one", "two", "three", "four"];
        apply_order(&mut values, &[2, 4, 1, 0, 3]);
        assert_eq!(values, vec!["two", "four", "one", "zero", "three"]);
    }

    #[test]
    fn in_place_order_preserves_ownership_and_storage_boundaries() {
        struct Value(usize);
        for count in [0, 1, 127, 128, 129, 256, 8192] {
            let mut order: Vec<usize> = (0..count).collect();
            if count > 1 {
                order[1..].rotate_left(1);
            }
            let mut values: Vec<Value> = (0..count).map(Value).collect();
            let pointer = values.as_ptr();
            let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                apply_order(&mut values, &order);
            });
            assert_eq!(values.as_ptr(), pointer);
            assert!(allocations <= usize::from(count > 128));
            assert_eq!(
                values.iter().map(|value| value.0).collect::<Vec<_>>(),
                order
            );
        }
        let mut values: Vec<usize> = (0..8192).collect();
        let order = values.clone();
        let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            apply_order(&mut values, &order);
        });
        assert_eq!(values, order);
        assert_eq!(allocations, 0);
    }

    #[test]
    fn in_place_order_matches_every_small_permutation() {
        fn visit(order: &mut [usize], start: usize) {
            if start == order.len() {
                let mut values: Vec<usize> = (0..order.len()).collect();
                apply_order(&mut values, order);
                assert_eq!(values, order);
                return;
            }
            for next in start..order.len() {
                order.swap(start, next);
                visit(order, start + 1);
                order.swap(start, next);
            }
        }
        for count in 0..=7 {
            visit(&mut (0..count).collect::<Vec<_>>(), 0);
        }
    }

    #[test]
    #[ignore = "长循环的配对计时，需在空闲主机上显式运行"]
    fn long_candidate_permutations_have_linear_scaling() {
        use std::hint::black_box;
        use std::time::Instant;

        fn elapsed(count: usize) -> u128 {
            let order: Vec<usize> = std::iter::once(0)
                .chain(2..count)
                .chain(std::iter::once(1))
                .collect();
            let mut values: Vec<usize> = (0..count).collect();
            apply_order(&mut values, &order);
            assert_eq!(values, order);
            let begin = Instant::now();
            for _ in 0..32768 / count {
                apply_order(black_box(&mut values), black_box(&order));
            }
            black_box(values);
            begin.elapsed().as_nanos()
        }
        // 两种长度处理相同总行数；交替先后顺序取中位数，预算留出主机噪声。
        let mut small = Vec::new();
        let mut large = Vec::new();
        for round in 0..5 {
            if round % 2 == 0 {
                small.push(elapsed(128));
                large.push(elapsed(8192));
            } else {
                large.push(elapsed(8192));
                small.push(elapsed(128));
            }
        }
        small.sort_unstable();
        large.sort_unstable();
        eprintln!(
            "permutation_equal_rows_small_ns={} large_ns={}",
            small[2], large[2]
        );
        assert!(
            large[2] <= small[2] * 16,
            "同等总行数的长排列成本过高: {large:?} / {small:?}"
        );
    }

    /// Only `CandidateSource::Generated` names alternative readings of one key. Every other source
    /// is plural by design — English words, emoji, kaomoji, quick phrases, AI suggestions — and an
    /// earlier version of this rule kept one of each and dropped the rest.
    #[test]
    fn one_lattice_row_needs_no_temporary_order_allocations() {
        let rows = [row("你好", LATTICE_SOURCE), row("你", 0)];
        let (order, allocations) =
            crate::ime::personal_rerank::allocations::count(|| runner_up_order(0, &rows));
        assert_eq!(order, None);
        assert_eq!(allocations, 0);
    }

    #[test]
    fn short_rerank_texts_need_no_temporary_heap_state() {
        let rows = [row("你好", LATTICE_SOURCE), row("倪好", LATTICE_SOURCE)];
        let (summary, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            with_row_texts(&rows, |texts| (texts.len(), texts[0], texts[1]))
        });
        assert_eq!(summary, (2, "你好", "倪好"));
        assert_eq!(allocations, 0);
    }

    #[test]
    fn static_rerank_inputs_need_no_temporary_heap_state() {
        let rows = [row("甲", LATTICE_SOURCE), row("乙", LATTICE_SOURCE)];
        let scores = [
            Some(StaticEvidence {
                log_prob_milli_nats: -1000,
                characters: 1,
            }),
            Some(StaticEvidence {
                log_prob_milli_nats: -2000,
                characters: 1,
            }),
        ];
        let scored = ScoredOrderRows::new(&rows, &scores);
        let (summary, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            let mut considered = [0; STATIC_RERANK_LIMIT];
            let mut statik = [0.0; STATIC_RERANK_LIMIT];
            let mut subset = [""; STATIC_RERANK_LIMIT];
            let count = fill_static_rerank_inputs(
                &scored,
                |row| row.answers_key,
                &mut considered,
                &mut statik,
                &mut subset,
            );
            (count, considered[0], considered[1], statik[0], subset[1])
        });
        assert_eq!(summary, (Some(2), 0, 1, -1.0, "乙"));
        assert_eq!(allocations, 0);
    }

    #[test]
    fn only_the_lattice_source_is_treated_as_alternative_readings() {
        assert_eq!(LATTICE_SOURCE, 8);
        for plural in [2u8, 3, 4, 5, 6, 7] {
            assert_ne!(LATTICE_SOURCE, plural);
        }
    }

    #[test]
    fn engine_order_is_built_only_when_missing() {
        let mut engine_order = Vec::new();
        ensure_engine_order(&mut engine_order, 3);
        assert_eq!(engine_order, vec![0, 1, 2]);
        // 已经是这一轮的映射时保持原样，不重置成恒等排列。
        engine_order.swap(0, 2);
        ensure_engine_order(&mut engine_order, 3);
        assert_eq!(engine_order, vec![2, 1, 0]);
    }

    #[test]
    fn rotate_to_front_keeps_the_rest_in_order() {
        let mut items = vec!["a", "b", "c", "d"];
        rotate_to_front(&mut items, 2);
        assert_eq!(items, vec!["c", "a", "b", "d"]);
    }

    #[test]
    fn only_korean_keeps_its_table_order() {
        let korean = SchemeType::Korean as u8;
        assert!(!reorders_candidates(korean));
        for ordinal in (0..=u8::MAX).filter(|&ordinal| ordinal != korean) {
            if let Some(scheme) = SchemeType::from_u8(ordinal) {
                assert_eq!(reorders_candidates(ordinal), scheme.holds_phrase_progress());
            }
        }
        // 刷新失败时的占位快照不对应任何方案，照旧允许重排。
        assert!(reorders_candidates(255));
    }

    #[test]
    fn only_shuangpin_corrections_stay_behind() {
        let shuangpin = SchemeType::Shuangpin as u8;
        assert!(!corrections_may_lead(shuangpin));
        for ordinal in (0..=u8::MAX).filter(|&ordinal| ordinal != shuangpin) {
            assert!(corrections_may_lead(ordinal), "{ordinal}");
        }
    }

    #[test]
    fn long_sentences_keep_three_readings_after_the_first() {
        // 第一条整句留在原位，其后两条紧跟着，其余整句挪到末尾；非整句来源保持相对顺序。
        let rows = [
            row("今天天气", LATTICE_SOURCE),
            row("今天", 0),
            row("金天天气", LATTICE_SOURCE),
            row("今天添气", LATTICE_SOURCE),
            row("今添天气", LATTICE_SOURCE),
            row("金", 0),
        ];
        assert_eq!(runner_up_order(0, &rows), Some(vec![0, 2, 3, 1, 5, 4]));
    }

    #[test]
    fn two_character_readings_keep_only_the_first() {
        let rows = [
            row("你好", LATTICE_SOURCE),
            row("倪好", LATTICE_SOURCE),
            row("你", 0),
        ];
        assert_eq!(runner_up_order(0, &rows), Some(vec![0, 2, 1]));
    }

    /// wenjian：首条读法是两字的 wen'jian，整句池把另一种切分 wen'ji'an 的三字读法放在主切分整句的座位上。降位后它们不再排在读满整串的词库词前面：排在最前的「问及按」坐到最后一个整串词库词「稳健」之后，其余挪到表尾。
    #[test]
    fn readings_of_another_segmentation_go_behind_the_words() {
        let partial = |text| OrderRow {
            answers_key: false,
            ..row(text, 0)
        };
        let rows = [
            row("文件", 0),
            row("问间", LATTICE_SOURCE),
            row("问及按", LATTICE_SOURCE),
            row("文集按", LATTICE_SOURCE),
            partial("文件夹"),
            row("稳健", 0),
            row("问见", LATTICE_SOURCE),
            partial("文"),
        ];
        assert_eq!(
            runner_up_order_pooled(0, &rows, &[], 32),
            Some(vec![0, 1, 4, 5, 2, 7, 3, 6])
        );
        // 五笔混输拼音走同一个全拼引擎，同一条规则。
        assert_eq!(
            runner_up_order_pooled(SchemeType::Wubi as u8, &rows, &[], SENTENCE_POOL),
            Some(vec![0, 1, 4, 5, 2, 7, 3, 6])
        );
        // 没有整句池时引擎没挪过座位，照旧只按字数认：三字读法留在原位。
        assert_eq!(
            runner_up_order(0, &rows),
            Some(vec![0, 1, 2, 3, 4, 5, 7, 6])
        );
        assert_eq!(
            runner_up_order_pooled(0, &rows, &[], 6),
            runner_up_order(0, &rows)
        );
        // 双拼不走全拼引擎的整句池。
        let shuangpin = SchemeType::Shuangpin as u8;
        assert_eq!(
            runner_up_order_pooled(shuangpin, &rows, &[], 32),
            runner_up_order(shuangpin, &rows)
        );
    }

    /// wuliji：首条读法三个字，后面留的两条只从同一种切分的读法里取。另一种切分排在最前的读法后面没有读满整串的词库词时留在原位，其余挪到表尾。
    #[test]
    fn kept_readings_come_from_the_leading_segmentation() {
        let partial = |text| OrderRow {
            answers_key: false,
            ..row(text, 0)
        };
        let rows = [
            row("乌力吉", 0),
            row("物理及", LATTICE_SOURCE),
            row("误立即及", LATTICE_SOURCE),
            row("物理机", LATTICE_SOURCE),
            row("物理即", LATTICE_SOURCE),
            row("物理急", LATTICE_SOURCE),
            partial("物理"),
            row("屋里及时", LATTICE_SOURCE),
        ];
        assert_eq!(
            runner_up_order_pooled(0, &rows, &[], 32),
            Some(vec![0, 1, 3, 4, 2, 6, 5, 7])
        );
    }

    /// 只回答按键前缀或补全到按键之后的生成行不是整串的读法，字数不同时留在原位。
    #[test]
    fn other_widths_need_a_whole_key_reading() {
        let completion = OrderRow {
            answers_key: false,
            ..row("问及按们", LATTICE_SOURCE)
        };
        let rows = [row("问间", LATTICE_SOURCE), completion, row("文件", 0)];
        assert_eq!(runner_up_order_pooled(0, &rows, &[], 32), None);
    }

    /// 引擎放在第一条读法正后方的全拼纠错整句留在第二位，占掉一条留下的读法名额；放在整句块之后的、双拼的照旧。
    #[test]
    fn a_quanpin_correction_seated_after_the_first_reading_keeps_its_seat() {
        let quanpin = SchemeType::Quanpin as u8;
        let shuangpin = SchemeType::Shuangpin as u8;
        let corrected = |text| OrderRow {
            corrected: true,
            ..row(text, LATTICE_SOURCE)
        };
        let order = |scheme, rows: &[OrderRow<'_>], seats: &[usize]| {
            let pinned = engine_seated_correction(scheme, rows, seats);
            runner_up_order_pinned(scheme, rows, &[], 0, pinned)
        };
        let identity = |count: usize| (0..count).collect::<Vec<_>>();
        let seated = [
            row("仙子啊我大哟段话", LATTICE_SOURCE),
            corrected("现在我打一段话"),
            row("仙子啊我打哟段话", LATTICE_SOURCE),
            row("仙子啊我打哟端华", LATTICE_SOURCE),
            row("仙子啊我大哟端华", LATTICE_SOURCE),
            row("仙子", 0),
            row("仙", 0),
        ];
        assert_eq!(
            engine_seated_correction(quanpin, &seated, &identity(7)),
            Some(1)
        );
        assert_eq!(
            order(quanpin, &seated, &identity(7)),
            Some(vec![0, 1, 2, 5, 6, 3, 4])
        );
        // 不给座位时照旧按读法降位。
        assert_eq!(
            runner_up_order(quanpin, &seated),
            Some(vec![0, 2, 3, 1, 5, 6, 4])
        );
        // 重排器把引擎的第三条读法提到首位后，纠错句被挤到第三格；按引擎座位仍认得出它，回到第二位。
        let rotated = [
            row("仙子啊我打哟段话", LATTICE_SOURCE),
            row("仙子啊我大哟段话", LATTICE_SOURCE),
            corrected("现在我打一段话"),
            row("仙子啊我打哟端华", LATTICE_SOURCE),
            row("仙子", 0),
        ];
        let rotated_seats = [2, 0, 1, 3, 4];
        assert_eq!(
            engine_seated_correction(quanpin, &rotated, &rotated_seats),
            Some(2)
        );
        assert_eq!(
            order(quanpin, &rotated, &rotated_seats),
            Some(vec![0, 2, 1, 4, 3])
        );
        // 坐首位的纠错句留在首位，读法按第一条字面读法的字数认：按纠错句的字数认，七字句后面的八字字面读法一条也不会降位。
        let leading = [
            corrected("现在我打一段话"),
            row("仙子啊我打哟段话", LATTICE_SOURCE),
            row("仙子啊我大哟段话", LATTICE_SOURCE),
            row("仙子啊我打哟端华", LATTICE_SOURCE),
            row("仙子啊我大哟端华", LATTICE_SOURCE),
            row("仙子", 0),
        ];
        assert_eq!(
            engine_seated_correction(quanpin, &leading, &identity(6)),
            Some(0)
        );
        assert_eq!(
            order(quanpin, &leading, &identity(6)),
            Some(vec![0, 1, 2, 5, 3, 4])
        );
        // 字数和读法相同也一样：它不按读法占座，也就不会因为超出名额挪到表尾。
        let same_width = [
            row("仙子啊我大哟段话", LATTICE_SOURCE),
            corrected("现在啊我打一段话"),
            row("仙子啊我打哟段话", LATTICE_SOURCE),
            row("仙子啊我打哟端华", LATTICE_SOURCE),
            row("仙子", 0),
        ];
        assert_eq!(
            order(quanpin, &same_width, &identity(5)),
            Some(vec![0, 1, 2, 4, 3])
        );
        // 双拼的纠错句不由这里保座。
        assert_eq!(
            engine_seated_correction(shuangpin, &seated, &identity(7)),
            None
        );
        // 引擎排在整句块之后的纠错句照旧：留下三条读法，它跟在后面。
        let behind = [
            row("仙子啊我大哟段话", LATTICE_SOURCE),
            row("仙子啊我打哟段话", LATTICE_SOURCE),
            row("仙子啊我打哟端华", LATTICE_SOURCE),
            row("仙子啊我大哟端华", LATTICE_SOURCE),
            corrected("现在我打一段话"),
            row("仙子", 0),
        ];
        assert_eq!(
            engine_seated_correction(quanpin, &behind, &identity(6)),
            None
        );
        assert_eq!(
            order(quanpin, &behind, &identity(6)),
            Some(vec![0, 1, 2, 4, 5, 3])
        );
        // 两字读法本来只留一条；纠错句保座后也不会把其余读法全留下。
        let short = [
            row("你好", LATTICE_SOURCE),
            corrected("您好"),
            row("倪好", LATTICE_SOURCE),
            row("你", 0),
        ];
        assert_eq!(order(quanpin, &short, &identity(4)), Some(vec![0, 1, 3, 2]));
        // 座位长度对不上时不保座。
        assert_eq!(engine_seated_correction(quanpin, &short, &[0, 1]), None);
    }

    /// 带触点查出的双拼表里，落点支持的纠错整句不算整句读法：前三条字面读法照旧留在前面、其余挪到表尾，纠错行留在引擎给的座位上。没有触点时它和别的读法一样被挪走。
    #[test]
    fn touch_priced_corrections_keep_their_engine_seat() {
        let shuangpin = SchemeType::Shuangpin as u8;
        let corrected = OrderRow {
            corrected: true,
            ..row("没关系", LATTICE_SOURCE)
        };
        let rows = [
            row("没跟系", LATTICE_SOURCE),
            row("美跟系", LATTICE_SOURCE),
            row("没根系", LATTICE_SOURCE),
            row("美根系", LATTICE_SOURCE),
            corrected,
            row("没跟", 0),
            row("没", 0),
        ];
        let supported = [false, false, false, false, true, false, false];
        assert_eq!(
            runner_up_order_with(shuangpin, &rows, &supported),
            Some(vec![0, 1, 2, 4, 5, 6, 3])
        );
        assert_eq!(
            runner_up_order_with(shuangpin, &rows, &[]),
            Some(vec![0, 1, 2, 5, 6, 3, 4])
        );
        assert_eq!(
            runner_up_order_with(shuangpin, &rows, &[]),
            runner_up_order(shuangpin, &rows)
        );
        // 长度对不上的列整列作废，不会错把别的行当成落点支持的。
        assert_eq!(
            runner_up_order_with(shuangpin, &rows, &supported[..6]),
            runner_up_order(shuangpin, &rows)
        );
        // 只有纠错行才看这一列：字面读法即使被标上也照旧算读法。
        let mut literal = supported;
        literal[3] = true;
        assert_eq!(
            runner_up_order_with(shuangpin, &rows, &literal),
            Some(vec![0, 1, 2, 4, 5, 6, 3])
        );
        // 纠错行挤在前三条读法里时，没有触点它占掉一个名额；有触点时让出名额，第三条字面读法坐到前两条后面，纠错行跟在它后面。
        let crowded = [
            row("没跟系", LATTICE_SOURCE),
            row("美跟系", LATTICE_SOURCE),
            corrected,
            row("没根系", LATTICE_SOURCE),
            row("没", 0),
        ];
        assert_eq!(
            runner_up_order_with(shuangpin, &crowded, &[]),
            Some(vec![0, 1, 2, 4, 3])
        );
        assert_eq!(
            runner_up_order_with(shuangpin, &crowded, &[false, false, true, false, false]),
            Some(vec![0, 1, 3, 2, 4])
        );
    }

    /// 守卫的另一半（#10018 PR3）：引擎按首选规则把双拼纠错整句放在第一位时（它同时是落点支持的那一行），重排器不跑模型、不把它换下去，次选读法的后移也不动它，字面整句留在第二位、前三条字面读法照旧在它后面。全拼同样不交给模型：字面整句读法在场时，纠错句坐首位是引擎按 `typo_lead_margin` 定的（`typo_sentence_beside_literal`）。
    #[test]
    fn a_shuangpin_correction_the_engine_put_first_stays_first() {
        let shuangpin = SchemeType::Shuangpin as u8;
        let rows = [
            OrderRow {
                corrected: true,
                ..row("没关系", LATTICE_SOURCE)
            },
            row("没跟系", LATTICE_SOURCE),
            row("美跟系", LATTICE_SOURCE),
            row("没根系", LATTICE_SOURCE),
            row("美根系", LATTICE_SOURCE),
            row("没跟", 0),
            row("没", 0),
        ];
        assert!(!rerank_needs_model(shuangpin, false, &rows));
        assert!(!rerank_needs_model(SchemeType::Quanpin as u8, false, &rows));
        let supported = [true, false, false, false, false, false, false];
        let order = runner_up_order_with(shuangpin, &rows, &supported).unwrap();
        assert_eq!(order, vec![0, 1, 2, 3, 5, 6, 4]);
        assert_eq!(order.iter().position(|&row| row == 1), Some(1));
    }

    /// 同一张表里有两条纠错整句（模糊音替代读法按平面表查出的那条没过落点的闸）：只有落点支持的那条留在座位上，另一条照旧沉底。
    #[test]
    fn only_the_touch_supported_correction_keeps_its_seat() {
        let shuangpin = SchemeType::Shuangpin as u8;
        let corrected = |text: &'static str| OrderRow {
            corrected: true,
            ..row(text, LATTICE_SOURCE)
        };
        let rows = [
            row("没跟系", LATTICE_SOURCE),
            row("美跟系", LATTICE_SOURCE),
            row("没根系", LATTICE_SOURCE),
            row("美根系", LATTICE_SOURCE),
            corrected("没关系"),
            corrected("美关系"),
            row("没跟", 0),
            row("没", 0),
        ];
        let supported = [false, false, false, false, true, false, false, false];
        assert_eq!(
            runner_up_order_with(shuangpin, &rows, &supported),
            Some(vec![0, 1, 2, 4, 6, 7, 3, 5])
        );
    }

    #[test]
    fn runner_up_order_leaves_lists_it_has_no_say_over() {
        let korean = SchemeType::Korean as u8;
        let rows = [
            row("你好", LATTICE_SOURCE),
            row("倪好", LATTICE_SOURCE),
            row("你", 0),
        ];
        assert_eq!(runner_up_order(korean, &rows), None);
        // 少于两行、没有整句读法、整句只有一个字（假名）、或排列不变时都返回 None。
        assert_eq!(runner_up_order(0, &rows[..1]), None);
        assert_eq!(runner_up_order(0, &[row("你", 0), row("尼", 0)]), None);
        assert_eq!(
            runner_up_order(0, &[row("あ", LATTICE_SOURCE), row("ア", LATTICE_SOURCE)]),
            None
        );
        assert_eq!(
            runner_up_order(0, &[row("你好", LATTICE_SOURCE), row("你", 0)]),
            None
        );
    }

    /// 输入读得通时，纠错整句要在混合分上比最好的字面读法高出 `CORRECTION_RERANK_MARGIN` 才被提上来；字面读法之间照常比较。词网格分相同，混合分之差是模型分之差的一半（权重 0.5）。
    #[test]
    fn a_correction_beside_literal_readings_needs_the_margin_to_lead() {
        let texts = ["我的班不大", "我的版不大", "我的办不到"].map(String::from);
        let sources = [LATTICE_SOURCE; 3];
        let answers = [true; 3];
        let corrected = [false, false, true];
        let evidence = [Some(StaticEvidence {
            log_prob_milli_nats: -5000,
            characters: 5,
        }); 3];
        let rows = ParallelOrderRows::new(&texts, &sources, &answers, &corrected)
            .with_static_evidence(&evidence);
        let quanpin = SchemeType::Quanpin as u8;
        let pick = |means: &[(&'static str, f32)]| {
            let mut scorer = Means {
                means: means.to_vec(),
                window: 64,
                context: String::new(),
            };
            rerank_pick_within(&mut scorer, "", quanpin, false, &rows, 0)
        };
        let below = 2.0 * CORRECTION_RERANK_MARGIN as f32 - 0.2;
        let above = 2.0 * CORRECTION_RERANK_MARGIN as f32 + 0.2;
        assert_eq!(
            pick(&[
                ("我的班不大", -4.0),
                ("我的版不大", -5.0),
                ("我的办不到", -4.0 + below)
            ]),
            None
        );
        assert_eq!(
            pick(&[
                ("我的班不大", -4.0),
                ("我的版不大", -3.5),
                ("我的办不到", -4.0 + below)
            ]),
            Some(1)
        );
        assert_eq!(
            pick(&[
                ("我的班不大", -4.0),
                ("我的版不大", -5.0),
                ("我的办不到", -4.0 + above)
            ]),
            Some(2)
        );
    }

    /// 逐字平均分照表给，表外 -5；记下读到的上文。
    struct Means {
        means: Vec<(&'static str, f32)>,
        window: usize,
        context: String,
    }

    impl CandidateScorer for Means {
        fn context_length(&self) -> usize {
            self.window
        }

        fn log_probabilities(&mut self, context: &str, candidates: &[&str]) -> Vec<f32> {
            self.context = context.to_owned();
            candidates
                .iter()
                .map(|text| {
                    self.means
                        .iter()
                        .find(|(row, _)| row == text)
                        .map_or(-5.0, |(_, mean)| *mean)
                })
                .collect()
        }
    }

    fn means(means: &[(&'static str, f32)], window: usize) -> Means {
        Means {
            means: means.to_vec(),
            window,
            context: String::new(),
        }
    }

    #[test]
    fn prediction_order_ranks_by_total_and_keeps_weight_order_on_ties() {
        // 们都 平均 -1 总分 -2，们 平均 -1.5 总分 -1.5；乙 丙 同分，保持原来的先后。NaN 排到最后。
        let mut scorer = means(&[("们都", -1.0), ("们", -1.5), ("怪", f32::NAN)], 64);
        let order = prediction_order(&mut scorer, "我", &["怪", "乙", "们都", "丙", "们"]).unwrap();
        assert_eq!(order, [4, 2, 1, 3, 0]);
        assert_eq!(scorer.context, "我");
    }

    #[test]
    fn prediction_order_declines_one_row_and_a_wrong_length() {
        let mut scorer = means(&[], 64);
        assert_eq!(prediction_order(&mut scorer, "", &["们"]), None);
        struct Short;
        impl CandidateScorer for Short {
            fn context_length(&self) -> usize {
                64
            }
            fn log_probabilities(&mut self, _: &str, _: &[&str]) -> Vec<f32> {
                Vec::new()
            }
        }
        assert_eq!(prediction_order(&mut Short, "", &["们", "的"]), None);
    }

    #[test]
    fn prediction_order_trims_the_context_like_the_keystroke_rerank() {
        let context = "今天天气很好我们一起去公园散步".repeat(8);
        let mut scorer = means(&[], 64);
        prediction_order(&mut scorer, &context, &["们", "一起"]).unwrap();
        assert_eq!(scorer.context, rerank_context(&context, 64, 2));
        assert!(scorer.context.chars().count() < context.chars().count());
    }
}
