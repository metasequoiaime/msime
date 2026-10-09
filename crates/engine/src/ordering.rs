//! 候选排序的唯一实现：句子模型重排、整句次选读法的后移，以及交给重排器的上文窗口。
//!
//! 这里只做决策，不碰候选数组：`rerank_pick` 返回应移到首位的下标，`runner_up_order` 返回一个排列（新座位 -> 旧座位）。`msime-input-runtime` 把结果同步应用到它的八个并行数组上，网页引擎（`msime-engine-wasm`）也调用同一组函数，两处的排序因此不会各自漂移。

use crate::SchemeType;

pub use chinese_ime_lm::{CandidateFacts, Reranker, SentenceModel, DICTIONARY_SOURCES};

/// `CandidateSource::Generated`: a whole-sentence path the word lattice assembled. The one source
/// whose members really are alternative readings of the same key.
pub const LATTICE_SOURCE: u8 = 8;

/// `CandidateSource::QuickPhrase`, `Emoji` and `Kaomoji`: entries a keyword finds in a catalog, listed in the catalog's order. They are not readings of the key, so the reranker neither compares them nor promotes them: typing `kiss` in kaomoji mode had the model seat the catalog's 646th entry, `French Kiss!(*￣(￣　*)`, above its first.
const CATALOG_SOURCES: [u8; 3] = [5, 6, 7];

/// 候选列表通常不超过一页；短列表的文本引用放在栈上，较大列表继续走堆缓冲。
const SMALL_RERANK_TEXTS: usize = 128;

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

/// Apply a permutation in place. `order` maps each new seat to its old seat.
///
/// The order is built as a permutation of the candidate seats, so each cycle can be rotated with
/// swaps. Keeping the operation in place matters here because the same order is applied to eight
/// parallel arrays, several of which contain candidate strings.
pub fn apply_order<T>(items: &mut [T], order: &[usize]) {
    debug_assert_eq!(items.len(), order.len());
    for start in 0..items.len() {
        // Process each cycle only from its smallest member, without allocating a visited bitmap.
        let mut current = order[start];
        let mut smallest = start;
        while current != start {
            smallest = smallest.min(current);
            current = order[current];
        }
        if smallest != start {
            continue;
        }
        current = start;
        while order[current] != start {
            let next = order[current];
            items.swap(current, next);
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

/// 排序决策读取的候选行来源。连续的 `OrderRow` 数组和快照里的并行数组都可以实现它。
pub trait OrderRowSource<'a> {
    fn len(&self) -> usize;
    fn get(&self, index: usize) -> OrderRow<'a>;

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

/// 借用快照里的并行候选数组，避免排序前复制一份行数组。
pub struct ParallelOrderRows<'a> {
    texts: &'a [String],
    sources: &'a [u8],
    answers_key: &'a [bool],
    corrected: &'a [bool],
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
        }
    }
}

impl<'a> OrderRowSource<'a> for ParallelOrderRows<'a> {
    fn len(&self) -> usize {
        self.texts.len()
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

/// rerank() 的决策部分：返回应当移到首位的行下标；None 表示不动。
///
/// `scheme` 是 `SchemeType` 序号；不允许重排的方案（韩文汉字表）返回 None，五笔且 `!answered_by_pinyin_fallback` 时返回 None，行数 < 2 时返回 None。`Reranker::best_where` 是 `&mut self`，因此这里取 `&mut`。
pub fn rerank_pick<'a, R: OrderRowSource<'a> + ?Sized>(
    reranker: &mut Reranker,
    context: &str,
    scheme: u8,
    answered_by_pinyin_fallback: bool,
    rows: &'a R,
) -> Option<usize> {
    // A Korean Hanja list is a table in frequency order for one syllable, not Chinese text the language model can read.
    if !reorders_candidates(scheme) {
        return None;
    }
    // A Wubi list the table answered is ranked by the table: the Engine seats the Wubi rows first (`merge_pinyin_fallback`) and appends the mixed-in pinyin rows after them. Those pinyin rows are corrections of the same letters (dyn read as dun), so the corrected-key rule below would strip the exact code hit (态 on dyn) of its dictionary exemption and let the model promote a longer code's row (太快 on dynn) over it. Only a list the pinyin fallback answered alone is pinyin, and that one is reranked like pinyin.
    if scheme_type(scheme) == Some(SchemeType::Wubi) && !answered_by_pinyin_fallback {
        return None;
    }
    if rows.len() < 2 {
        return None;
    }
    // A dictionary hit earns the model's deference because it carries corpus frequency for the
    // key the user typed. That premise fails the moment the engine offers a correction of that
    // key: the frequency then belongs to the letters that arrived rather than to the word they
    // were aiming at, and the list holds both readings. So the whole list loses the exemption,
    // not the corrected rows — the row that would wrongly win is the uncorrected one.
    //
    // With correction off, or with nothing corrected, this is exactly the previous behaviour,
    // which is what the 2052-case dictionary measurement was taken on.
    //
    // 不许领先的纠错行（双拼）只是附带的一个改正：它不算回答了按键，模型不拿它比较，也不因为它的存在撤掉词典命中的豁免，排序和没有纠错时完全一样。
    let corrections_lead = corrections_may_lead(scheme);
    let corrected_key = corrections_lead && (0..rows.len()).any(|index| rows.get(index).corrected);
    let answers_key = |row: OrderRow<'a>| {
        row.answers_key
            && !CATALOG_SOURCES.contains(&row.source)
            && (corrections_lead || !row.corrected)
    };
    // Only candidates that answer the key are scored, so they are the ones the window has to leave room for.
    let longest = (0..rows.len())
        .map(|index| rows.get(index))
        .filter(|row| answers_key(*row))
        .map(|row| row.text.chars().count())
        .max()
        .unwrap_or(0);
    let context = rerank_context(context, reranker.model().context_length(), longest);
    with_row_texts(rows, |texts| {
        reranker.best_where(context, texts, |index| CandidateFacts {
            answers_key: answers_key(rows.get(index)),
            trusted_dictionary_hit: DICTIONARY_SOURCES.contains(&rows.get(index).source)
                && !corrected_key,
        })
    })
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
    let width = (0..rows.len())
        .map(|index| rows.get(index))
        .find(|row| row.source == LATTICE_SOURCE)
        .map(|row| row.text.chars().count())?;
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
        row.source == LATTICE_SOURCE && row.text.chars().count() == width
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
    fn in_place_order_applies_candidate_permutations() {
        let mut values = vec!["zero", "one", "two", "three", "four"];
        apply_order(&mut values, &[2, 4, 1, 0, 3]);
        assert_eq!(values, vec!["two", "four", "one", "zero", "three"]);
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
