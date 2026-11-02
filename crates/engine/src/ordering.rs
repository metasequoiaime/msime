//! 候选排序的唯一实现：句子模型重排、整句次选读法的后移，以及交给重排器的上文窗口。
//!
//! 这里只做决策，不碰候选数组：`rerank_pick` 返回应移到首位的下标，`runner_up_order` 返回一个排列（新座位 -> 旧座位）。`msime-input-runtime` 把结果同步应用到候选元数据和引擎座位映射上，网页引擎（`msime-engine-wasm`）也调用同一组函数，两处的排序因此不会各自漂移。
//!
//! 这是 msime 自己的产品胶水，不在引擎文档化的 API 里：第三方拿到的是引擎的原始顺序（核心 `Session` 的候选），要不要再排、怎么排由它自己决定。

use crate::SchemeType;

pub use chinese_ime_lm::{CandidateFacts, Reranker, SentenceModel, DICTIONARY_SOURCES};

/// 排序决策要的模型能力：按上文给候选打逐字平均自然对数概率，以及模型一次能读多少字。
///
/// `rerank_pick` 只经这个接口用模型，所以随包的纯 Rust 句子模型（[`Reranker`]）和需要原生运行库的模型后端走同一套规则：裁决点、词网格分混合、词典首选的豁免都在这里，后端只管算分。引擎本身仍是纯 Rust，原生后端放在引擎之外的 crate 里实现这个接口。
pub trait CandidateScorer: Send {
    /// 模型一次能读的字数，上文和候选合计；[`rerank_context`] 按它截上文。
    fn context_length(&self) -> usize;

    /// 每个候选在 `context` 之后的逐字平均自然对数概率，顺序同 `candidates`。
    fn log_probabilities(&mut self, context: &str, candidates: &[&str]) -> Vec<f32>;

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

/// 方案的纠错行能不能被重排器提到首位。双拼的纠错整句按产品取舍一律不抢首选（#6034，引擎把它排在整句块之后），所以重排时它既不参与比较，也不让整张表失去词典命中的豁免；全拼的纠错行本来就允许领先（`typo_lead_margin`），照旧。
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
fn longest_rerank_candidate<'a, R, F>(rows: &R, answers_key: &F) -> usize
where
    R: OrderRowSource<'a> + ?Sized,
    F: Fn(OrderRow<'a>) -> bool,
{
    (0..rows.len())
        .map(|index| rows.get(index))
        .filter(|row| answers_key(*row))
        .take(STATIC_RERANK_LIMIT)
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

fn any_corrected_key<'a, R: OrderRowSource<'a> + ?Sized>(scheme: u8, rows: &'a R) -> bool {
    corrections_may_lead(scheme) && (0..rows.len()).any(|index| rows.get(index).corrected)
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
    if !rerank_needs_model(scheme, answered_by_pinyin_fallback, rows) {
        return None;
    }
    let mut contested = [0; CONTEST_LIMIT];
    if let Some((count, margin)) = contest_rows(scheme, rows, &mut contested) {
        return pick_contest(reranker, context, rows, &contested[..count], margin);
    }
    let answers_key = |row: OrderRow<'a>| row_answers_key(scheme, row);
    let corrected_key = any_corrected_key(scheme, rows);
    // 只有参与比较的候选需要占用模型上下文窗口。
    let longest = longest_rerank_candidate(rows, &answers_key);
    let context = rerank_context(context, reranker.context_length(), longest);
    if rows.static_evidence(0).is_some() {
        let mut considered = [0; STATIC_RERANK_LIMIT];
        let mut statik = [0.0; STATIC_RERANK_LIMIT];
        let mut subset = [""; STATIC_RERANK_LIMIT];
        if let Some(count) =
            fill_static_rerank_inputs(rows, answers_key, &mut considered, &mut statik, &mut subset)
        {
            let neural = reranker.log_probabilities(context, &subset[..count]);
            if neural.len() == count {
                // 模型逐字分与词网格逐字分等权混合，两侧均为自然对数。
                let combined = |index: usize| (statik[index] + f64::from(neural[index])) * 0.5;
                let mut best = 0;
                for index in 1..count {
                    if combined(index) > combined(best) {
                        best = index;
                    }
                }
                return (best != 0).then(|| considered[best]);
            }
        }
    }
    with_row_texts(rows, |texts| {
        reranker.best_where(context, texts, &|index| CandidateFacts {
            answers_key: answers_key(rows.get(index)),
            trusted_dictionary_hit: DICTIONARY_SOURCES.contains(&rows.get(index).source)
                && !corrected_key,
        })
    })
}

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
    considered: &mut [usize; STATIC_RERANK_LIMIT],
    statik: &mut [f64; STATIC_RERANK_LIMIT],
    subset: &mut [&'a str; STATIC_RERANK_LIMIT],
) -> Option<usize>
where
    R: OrderRowSource<'a> + ?Sized,
    F: Fn(OrderRow<'a>) -> bool,
{
    let mut count = 0;
    for index in 0..rows.len() {
        let row = rows.get(index);
        if !answers_key(row) {
            continue;
        }
        if count == STATIC_RERANK_LIMIT {
            break;
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
    let width = (0..rows.len())
        .map(|index| (index, rows.get(index)))
        .find(|(index, row)| lattice(*index, row))
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
    let is_reading = |index: usize| {
        let row = rows.get(index);
        lattice(index, &row) && row.text.chars().count() == width
    };
    // 先只扫描输出顺序，常见的整句候选已经连续时直接返回，避免为最终排列分配缓冲。
    let mut output = 0;
    let mut identity = true;
    let mut reading_seen = 0;
    for index in 0..count {
        if !is_reading(index) {
            identity &= output == index;
            output += 1;
            continue;
        }
        if reading_seen == 0 {
            identity &= output == index;
            output += 1;
            let mut kept = 1;
            for next in index + 1..count {
                if is_reading(next) {
                    if kept == keep {
                        break;
                    }
                    identity &= output == next;
                    output += 1;
                    kept += 1;
                }
            }
        }
        reading_seen += 1;
    }
    reading_seen = 0;
    for index in 0..count {
        if is_reading(index) {
            if reading_seen >= keep {
                identity &= output == index;
                output += 1;
            }
            reading_seen += 1;
        }
    }
    debug_assert_eq!(output, count);
    if identity {
        return None;
    }

    let mut order = Vec::with_capacity(count);
    let mut reading_seen = 0;
    for index in 0..count {
        if !is_reading(index) {
            order.push(index);
            continue;
        }
        if reading_seen == 0 {
            order.push(index);
            let mut kept = 1;
            for next in index + 1..count {
                if is_reading(next) {
                    if kept == keep {
                        break;
                    }
                    order.push(next);
                    kept += 1;
                }
            }
        }
        reading_seen += 1;
    }
    reading_seen = 0;
    for index in 0..count {
        if is_reading(index) {
            if reading_seen >= keep {
                order.push(index);
            }
            reading_seen += 1;
        }
    }
    debug_assert_eq!(order.len(), count);
    if order.iter().enumerate().all(|(seat, index)| seat == *index) {
        return None;
    }
    Some(order)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(text: &str, source: u8) -> OrderRow<'_> {
        OrderRow {
            text,
            source,
            answers_key: true,
            corrected: false,
        }
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
        assert_eq!(longest_rerank_candidate(&rows, &|row| row.answers_key), 1);
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

    /// 守卫的另一半（#10018 PR3）：引擎按首选规则把双拼纠错整句放在第一位时（它同时是落点支持的那一行），重排器不跑模型、不把它换下去，次选读法的后移也不动它，字面整句留在第二位、前三条字面读法照旧在它后面。全拼的纠错行本来就允许领先，同样一张表照常交给模型比较。
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
        assert!(rerank_needs_model(SchemeType::Quanpin as u8, false, &rows));
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
}
