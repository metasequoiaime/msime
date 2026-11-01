//! 首组同音候选的个人上下文重排（core-session.md §7.2，`R/core/personal_context_rerank.cpp`）。

use crate::lattice::personal::{PersonalContext, PersonalNgram};
use crate::types::{SchemeType, WordItem};

pub const MAX_GROUP: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PersonalContextRerankOrder {
    group_len: usize,
    /// 组内各行在候选列表中的位置；首位之后的落点支持纠错行不在组内，重排时留在引擎给的位置。
    seats: [usize; MAX_GROUP],
    indices: [usize; MAX_GROUP],
}

impl PersonalContextRerankOrder {
    /// 按排序索引原地重排组内各行，不复制行内字符串；组外的行不动。
    pub(crate) fn reorder<T>(&self, rows: &mut [T]) {
        debug_assert!(self.group_len == 0 || rows.len() > self.seats[self.group_len - 1]);
        let mut positions = [0; MAX_GROUP];
        for (index, position) in positions.iter_mut().take(self.group_len).enumerate() {
            *position = index;
        }
        for seat in 0..self.group_len {
            let source = self.indices[seat];
            let current = positions[..self.group_len]
                .iter()
                .position(|&index| index == source)
                .expect("排序索引必须对应组内行");
            if current != seat {
                rows.swap(self.seats[seat], self.seats[current]);
                positions.swap(seat, current);
            }
        }
    }
}

/// 返回首组同长度字典候选的栈上排序索引；排序未变时返回 `None`，固定首选保持原位。
pub(crate) fn personal_context_rerank_order(
    candidates: &[WordItem],
    model: &PersonalNgram,
    earlier: Option<&str>,
    previous: &str,
    scheme: SchemeType,
    is_pinned: &mut dyn FnMut(&str) -> bool,
) -> Option<PersonalContextRerankOrder> {
    let first = candidates.first()?;
    if candidates.len() < 2 || !first.source.is_dictionary() {
        return None;
    }
    let context = model.context(earlier, Some(previous));
    let mu = context.confidence;
    if mu <= 0.0 {
        return None;
    }

    let (seats, group_len) = leading_group(candidates, scheme);
    if group_len < 2 {
        return None;
    }
    let mut rows = [first; MAX_GROUP];
    for (row, &seat) in rows.iter_mut().zip(&seats[..group_len]) {
        *row = &candidates[seat];
    }
    let group = &rows[..group_len];
    let personal = personal_probabilities(group, model, &context);
    let indices = blended_order_indices(group, &personal[..group_len], mu, is_pinned)?;
    Some(PersonalContextRerankOrder {
        group_len,
        seats,
        indices,
    })
}

fn personal_probabilities(
    group: &[&WordItem],
    model: &PersonalNgram,
    context: &PersonalContext,
) -> [f64; MAX_GROUP] {
    let mut personal = [0.0; MAX_GROUP];
    for (index, item) in group.iter().enumerate() {
        personal[index] = model.probability(context, &item.word);
    }
    personal
}

/// 匹配编码消耗的输入字母数，音节分隔符不计。
fn consumed_letters(pinyin: &str) -> usize {
    pinyin.bytes().filter(|&byte| byte != b'\'').count()
}

/// 首组与第一行字数、消耗字母数相同的连续字典候选（跳过首位之后的落点支持纠错行），最多 16 行；完整读音首选不与其它读音混排。返回各行在候选列表中的位置和组内行数。
///
/// A touch-supported correction row after the leader stays out of the group but does not end it: it keeps its raw dictionary weight while the engine seated it by the touch-penalized rank, so blending that weight here could lift a word the user did not type over the literal leader. Skipping it keeps the literal homophones seated behind it in the group, and `reorder` leaves the correction row where the engine put it.
fn leading_group(candidates: &[WordItem], scheme: SchemeType) -> ([usize; MAX_GROUP], usize) {
    let first = &candidates[0];
    let characters = first.word.chars().count();
    let letters = consumed_letters(&first.pinyin);
    // 只有全拼的匹配编码是拼写出的读音；简拼与双拼的编码可以指向多个读音，双拼编码与读音字面相同（如单键 `a`）也只是巧合，都不收紧到某一个读音。
    let exact_reading = scheme == SchemeType::Quanpin
        && !first.pinyin.is_empty()
        && first.pinyin == first.canonical_pinyin;
    let mut seats = [0; MAX_GROUP];
    let mut len = 0;
    for (index, item) in candidates.iter().enumerate() {
        if len == MAX_GROUP {
            break;
        }
        if index > 0 && item.touch_supported && !item.corrected_from.is_empty() {
            continue;
        }
        let joins = item.source.is_dictionary()
            && item.word.chars().count() == characters
            && consumed_letters(&item.pinyin) == letters
            && (!exact_reading
                || item.canonical_pinyin == first.canonical_pinyin
                // 同一串字母的另一种完整切分（fan'gan 与 fang'an）。
                || (!item.pinyin.is_empty() && item.pinyin == item.canonical_pinyin));
        if !joins {
            break;
        }
        seats[len] = index;
        len += 1;
    }
    (seats, len)
}

#[cfg(test)]
fn leading_group_len(candidates: &[WordItem], scheme: SchemeType) -> usize {
    leading_group(candidates, scheme).1
}

#[cfg(test)]
fn blended_order(
    group: &[WordItem],
    personal: &[f64],
    mu: f64,
    is_pinned: &mut dyn FnMut(&str) -> bool,
) -> Option<Vec<usize>> {
    let rows: Vec<&WordItem> = group.iter().collect();
    let indices = blended_order_indices(&rows, personal, mu, is_pinned)?;
    Some(indices[..group.len()].to_vec())
}

/// 按混合概率对首组评分，返回组内各位置该放的组内行号；模型未知或排序未变时不返回索引。
fn blended_order_indices(
    group: &[&WordItem],
    personal: &[f64],
    mu: f64,
    is_pinned: &mut dyn FnMut(&str) -> bool,
) -> Option<[usize; MAX_GROUP]> {
    let personal_sum: f64 = personal.iter().sum();
    if personal_sum <= 0.0 {
        return None;
    }
    let weight = |item: &WordItem| item.weight.max(1) as f64;
    let weight_sum: f64 = group.iter().copied().map(weight).sum();

    let mut scored = [(0.0, 0usize); MAX_GROUP];
    for (index, (item, probability)) in group.iter().copied().zip(personal).enumerate() {
        let share = weight(item) / weight_sum;
        scored[index] = (
            ((1.0 - mu) * share + mu * probability / personal_sum).ln(),
            index,
        );
    }
    let scored = &mut scored[..group.len()];
    // 稳定排序使同分候选保持字典顺序。
    scored.sort_by(|left, right| right.0.total_cmp(&left.0));

    // 固定首选查询会读取日志，仅在首选即将失位时调用。
    if scored[0].1 != 0 && is_pinned(&group[0].word) {
        let leader = scored
            .iter()
            .position(|&(_, index)| index == 0)
            .expect("the leader is scored");
        scored[..=leader].rotate_right(1);
    }

    if scored
        .iter()
        .enumerate()
        .all(|(seat, &(_, index))| seat == index)
    {
        return None;
    }
    let mut indices = [0; MAX_GROUP];
    for (seat, &(_, index)) in scored.iter().enumerate() {
        indices[seat] = index;
    }
    Some(indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CandidateSource;

    fn row(word: &str, weight: i64) -> WordItem {
        WordItem::new("ni", word, weight, CandidateSource::Database, "ni")
    }

    fn never_pinned() -> impl FnMut(&str) -> bool {
        |_: &str| false
    }

    #[test]
    fn a_strong_context_moves_the_follower_up() {
        let group = [row("甲", 300), row("乙", 200), row("丙", 100)];
        let order = blended_order(&group, &[0.0, 0.0, 1.0], 0.5, &mut never_pinned());
        assert_eq!(order, Some(vec![2, 0, 1]));
    }

    #[test]
    fn unchanged_personal_order_needs_no_allocations_at_the_group_limit() {
        let mut model = PersonalNgram::default();
        model.add_pair("甲", "乙", 32);
        let mut candidates = vec![row("丙", 1); MAX_GROUP];
        candidates[0] = row("乙", 100);

        let (ordered, allocations) = allocations::count(|| {
            personal_context_rerank_order(
                &candidates,
                &model,
                None,
                "甲",
                SchemeType::Quanpin,
                &mut never_pinned(),
            )
        });

        assert_eq!(ordered, None);
        assert_eq!(allocations, 0);
    }

    #[test]
    fn changed_personal_order_preserves_rows_outside_the_group_without_allocating() {
        let mut model = PersonalNgram::default();
        model.add_pair("甲", "乙", 32);
        let mut candidates = vec![row("丙", 1); MAX_GROUP];
        candidates[MAX_GROUP - 1] = row("乙", 1);
        candidates.push(row("丁", 1));
        let mut reordered = candidates.clone();

        let (ordered, allocations) = allocations::count(|| {
            personal_context_rerank_order(
                &candidates,
                &model,
                None,
                "甲",
                SchemeType::Quanpin,
                &mut never_pinned(),
            )
        });

        let order = ordered.expect("个人上下文应把最后一个组内候选提到首位");
        assert_eq!(allocations, 0);
        let ((), allocations) = allocations::count(|| order.reorder(&mut reordered));
        assert_eq!(reordered[0], candidates[MAX_GROUP - 1]);
        assert_eq!(&reordered[1..MAX_GROUP], &candidates[..MAX_GROUP - 1]);
        assert_eq!(reordered[MAX_GROUP], candidates[MAX_GROUP]);
        assert_eq!(allocations, 0);
    }

    #[test]
    fn changed_personal_order_returns_stack_indices_without_allocating_rows() {
        let mut model = PersonalNgram::default();
        model.add_pair("甲", "乙", 32);
        let mut candidates = vec![row("丙", 1); MAX_GROUP];
        candidates[MAX_GROUP - 1] = row("乙", 1);

        let (order, allocations) = allocations::count(|| {
            personal_context_rerank_order(
                &candidates,
                &model,
                None,
                "甲",
                SchemeType::Quanpin,
                &mut never_pinned(),
            )
        });

        let order = order.expect("个人上下文应返回组内排序");
        assert_eq!(order.group_len, MAX_GROUP);
        assert_eq!(order.indices[0], MAX_GROUP - 1);
        assert_eq!(allocations, 0);

        order.reorder(&mut candidates);
        assert_eq!(candidates[0].word, "乙");
        assert_eq!(candidates[MAX_GROUP - 1].word, "丙");
    }

    #[test]
    fn a_weak_context_keeps_the_dictionary_order() {
        // A context word the dictionary already ranks second cannot unseat a far heavier leader.
        let group = [row("甲", 1_000_000), row("乙", 10), row("丙", 1)];
        assert_eq!(
            blended_order(&group, &[0.0, 1.0, 0.0], 0.01, &mut never_pinned()),
            None
        );
        // The same weak context still lifts an unweighted follower past a row it knows nothing about.
        assert_eq!(
            blended_order(&group, &[0.0, 0.0, 1.0], 0.01, &mut never_pinned()),
            Some(vec![0, 2, 1])
        );
    }

    #[test]
    fn an_unknown_context_changes_nothing() {
        let group = [row("甲", 3), row("乙", 2)];
        assert_eq!(
            blended_order(&group, &[0.0, 0.0], 0.5, &mut never_pinned()),
            None
        );
    }

    #[test]
    fn weights_below_one_count_as_one() {
        // With equal shares the personal term alone decides.
        let group = [row("甲", 0), row("乙", -5)];
        assert_eq!(
            blended_order(&group, &[0.2, 0.8], 0.5, &mut never_pinned()),
            Some(vec![1, 0])
        );
    }

    #[test]
    fn ties_keep_the_dictionary_order() {
        let group = [row("甲", 1), row("乙", 1), row("丙", 1)];
        assert_eq!(
            blended_order(&group, &[0.0, 0.5, 0.5], 0.5, &mut never_pinned()),
            Some(vec![1, 2, 0])
        );
    }

    #[test]
    fn a_pinned_leader_keeps_its_seat_and_is_asked_only_when_it_would_move() {
        let group = [row("甲", 300), row("乙", 200), row("丙", 100)];
        let mut asked = Vec::new();
        let order = blended_order(&group, &[0.0, 0.3, 0.7], 0.9, &mut |word: &str| {
            asked.push(word.to_string());
            true
        });
        assert_eq!(order, Some(vec![0, 2, 1]));
        assert_eq!(asked, vec!["甲".to_string()]);

        let mut asked = 0;
        let order = blended_order(&group, &[0.9, 0.0, 0.1], 0.9, &mut |_: &str| {
            asked += 1;
            true
        });
        assert_eq!(order, Some(vec![0, 2, 1]));
        assert_eq!(asked, 0);
    }

    #[test]
    fn the_group_is_the_leading_same_length_dictionary_run() {
        let mut list = vec![row("甲", 3), row("乙", 2), row("丙丁", 1), row("戊", 1)];
        assert_eq!(leading_group_len(&list, SchemeType::Quanpin), 2);
        list[1].source = CandidateSource::CloudSuggestion;
        assert_eq!(leading_group_len(&list, SchemeType::Quanpin), 1);
        list[1].source = CandidateSource::UserDatabase;
        list[2] = row("丙", 1);
        assert_eq!(leading_group_len(&list, SchemeType::Quanpin), 4);

        let long: Vec<WordItem> = (0..20).map(|index| row("字", index)).collect();
        assert_eq!(leading_group_len(&long, SchemeType::Quanpin), MAX_GROUP);
    }

    #[test]
    fn personal_context_does_not_cross_matched_code_or_reading_boundaries() {
        let mut model = PersonalNgram::default();
        model.add_pair("子", "乙", 32);
        for (matched, canonical) in [("ge", "ge"), ("gei", "ge"), ("ge", "gei")] {
            let rows = [
                WordItem::new("gei", "甲", 100, CandidateSource::Database, "gei"),
                WordItem::new(matched, "乙", 1000, CandidateSource::Database, canonical),
            ];
            assert_eq!(leading_group_len(&rows, SchemeType::Quanpin), 1);
            assert_eq!(
                personal_context_rerank_order(
                    &rows,
                    &model,
                    None,
                    "子",
                    SchemeType::Quanpin,
                    &mut never_pinned(),
                ),
                None
            );
        }
    }

    #[test]
    fn an_abbreviated_key_keeps_personal_ranking_across_its_readings() {
        let mut model = PersonalNgram::default();
        model.add_pair("子", "乙", 32);
        let rows = [
            WordItem::new("l", "甲", 100, CandidateSource::Database, "lai"),
            WordItem::new("l", "乙", 90, CandidateSource::Database, "le"),
        ];
        let order = personal_context_rerank_order(
            &rows,
            &model,
            None,
            "子",
            SchemeType::Shuangpin,
            &mut never_pinned(),
        )
        .unwrap();
        assert_eq!(order.group_len, 2);
        assert_eq!(order.indices[0], 1);
    }

    #[test]
    fn a_shuangpin_key_that_spells_a_reading_keeps_personal_ranking_across_readings() {
        let mut model = PersonalNgram::default();
        model.add_pair("子", "乙", 32);
        let rows = [
            WordItem::new("a", "甲", 100, CandidateSource::Database, "a"),
            WordItem::new("a", "乙", 90, CandidateSource::Database, "ai"),
        ];
        let order = personal_context_rerank_order(
            &rows,
            &model,
            None,
            "子",
            SchemeType::Shuangpin,
            &mut never_pinned(),
        )
        .unwrap();
        assert_eq!(order.group_len, 2);
        assert_eq!(order.indices[0], 1);

        // 全拼输入 `a` 已拼满组首读音，`ai` 的候选不进同组。
        assert_eq!(leading_group_len(&rows, SchemeType::Quanpin), 1);
        assert_eq!(
            personal_context_rerank_order(
                &rows,
                &model,
                None,
                "子",
                SchemeType::Quanpin,
                &mut never_pinned(),
            ),
            None
        );
    }

    #[test]
    fn complete_alternative_segmentations_share_one_personal_group() {
        let mut model = PersonalNgram::default();
        model.add_pair("很", "反感", 32);
        let rows = [
            WordItem::new(
                "fang'an",
                "方案",
                1000,
                CandidateSource::Database,
                "fang'an",
            ),
            WordItem::new("fan'gan", "反感", 100, CandidateSource::Database, "fan'gan"),
        ];
        assert_eq!(leading_group_len(&rows, SchemeType::Quanpin), 2);
        let order = personal_context_rerank_order(
            &rows,
            &model,
            None,
            "很",
            SchemeType::Quanpin,
            &mut never_pinned(),
        )
        .unwrap();
        assert_eq!(order.group_len, 2);
        assert_eq!(order.indices[0], 1);
    }

    #[test]
    fn a_touch_correction_row_keeps_its_seat_outside_the_group() {
        // Shuangpin short-word corrections carry their raw dictionary weight; inside the group that weight alone would outrank the literal leader the touch penalty kept first. The literal homophones seated behind the correction row still join the group.
        let mut corrected = row("关", 2_000_000);
        corrected.corrected_from = "gf".into();
        corrected.touch_supported = true;
        let mut list = vec![row("跟", 1000), corrected, row("根", 500)];
        let (seats, len) = leading_group(&list, SchemeType::Shuangpin);
        assert_eq!(&seats[..len], &[0, 2]);

        let mut model = PersonalNgram::default();
        model.add_pair("没", "关", 32);
        assert_eq!(
            personal_context_rerank_order(
                &list,
                &model,
                None,
                "没",
                SchemeType::Shuangpin,
                &mut never_pinned()
            ),
            None
        );

        let mut model = PersonalNgram::default();
        model.add_pair("没", "根", 32);
        let order = personal_context_rerank_order(
            &list,
            &model,
            None,
            "没",
            SchemeType::Shuangpin,
            &mut never_pinned(),
        )
        .expect("个人上下文应把纠错行之后的字面同音字提到首位");
        let mut reordered = list.clone();
        order.reorder(&mut reordered);
        let words: Vec<&str> = reordered.iter().map(|row| row.word.as_str()).collect();
        assert_eq!(words, ["根", "关", "跟"]);

        // Other autocorrected rows (Quanpin corrected cuts) keep their place in the group, and a corrected leader still opens it.
        list[1].touch_supported = false;
        assert_eq!(leading_group_len(&list, SchemeType::Shuangpin), 3);
        list[0].corrected_from = "gf".into();
        list[0].touch_supported = true;
        list[1].touch_supported = true;
        let (seats, len) = leading_group(&list, SchemeType::Shuangpin);
        assert_eq!(&seats[..len], &[0, 2]);
    }
}

#[cfg(test)]
#[allow(unsafe_code)]
pub(crate) mod allocations {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
        static HEAP: Cell<Option<HeapUsage>> = const { Cell::new(None) };
    }

    #[derive(Clone, Copy, Default)]
    struct HeapUsage {
        live_bytes: i128,
        peak_bytes: usize,
        minimum_bytes: i128,
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct Measurement {
        pub allocations: usize,
        pub peak_bytes: usize,
        pub remaining_bytes: i128,
        pub minimum_bytes: i128,
    }

    struct CountingAllocator;

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    fn record() {
        let _ = COUNT.try_with(|count| {
            if let Some(value) = count.get() {
                count.set(Some(value + 1));
            }
        });
    }

    fn record_bytes(change: i128) {
        let _ = HEAP.try_with(|heap| {
            if let Some(mut value) = heap.get() {
                value.live_bytes += change;
                value.minimum_bytes = value.minimum_bytes.min(value.live_bytes);
                if value.live_bytes > 0 {
                    value.peak_bytes = value.peak_bytes.max(value.live_bytes as usize);
                }
                heap.set(Some(value));
            }
        });
    }

    // 只观测当前测试线程，布局、指针和内存管理全部委托给 System。
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            record();
            // 安全：原样传递调用方提供的合法布局。
            let pointer = unsafe { System.alloc(layout) };
            if !pointer.is_null() {
                record_bytes(layout.size() as i128);
            }
            pointer
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            record_bytes(-(layout.size() as i128));
            // 安全：原样传递 System 分配的指针及其原始布局。
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            record();
            // 安全：原样传递 System 分配的指针、原始布局及调用方要求的大小。
            let pointer = unsafe { System.realloc(ptr, layout, new_size) };
            if !pointer.is_null() {
                record_bytes(new_size as i128 - layout.size() as i128);
            }
            pointer
        }
    }

    struct Scope;

    impl Drop for Scope {
        fn drop(&mut self) {
            COUNT.with(|count| count.set(None));
            HEAP.with(|heap| heap.set(None));
        }
    }

    pub(crate) fn count<T>(operation: impl FnOnce() -> T) -> (T, usize) {
        let (result, measurement) = measure_inner(operation, false);
        (result, measurement.allocations)
    }

    /// 量化区间内的逻辑请求字节；峰值不含 System 内部暂存，不等于进程 RSS。
    /// 精确比较时，闭包不得释放进入区间前已存在的堆对象。
    pub(crate) fn measure<T>(operation: impl FnOnce() -> T) -> (T, Measurement) {
        measure_inner(operation, true)
    }

    fn measure_inner<T>(operation: impl FnOnce() -> T, track_heap: bool) -> (T, Measurement) {
        COUNT.with(|count| {
            assert!(count.get().is_none());
            count.set(Some(0));
        });
        let scope = Scope;
        if track_heap {
            HEAP.with(|heap| heap.set(Some(HeapUsage::default())));
        }
        let result = operation();
        let allocations = COUNT.with(|count| count.get().unwrap());
        let heap = HEAP.with(|heap| heap.get().unwrap_or_default());
        drop(scope);
        (
            result,
            Measurement {
                allocations,
                peak_bytes: heap.peak_bytes,
                remaining_bytes: heap.live_bytes,
                minimum_bytes: heap.minimum_bytes,
            },
        )
    }

    /// 逐字段重建出不与 `rows` 共享存储的期望值。`clone` 出来的期望值与原行共用同一个 `Arc`，热查询改写这些共享字段时只能另建存储，量到的就不再是原地复用时查询本身的分配。
    pub(crate) fn unshared_rows(rows: &[crate::types::WordItem]) -> Vec<crate::types::WordItem> {
        rows.iter()
            .map(|row| crate::types::WordItem {
                pinyin: row.pinyin.as_str().into(),
                canonical_pinyin: row.canonical_pinyin.as_str().into(),
                word: row.word.as_str().into(),
                corrected_from: row.corrected_from.as_str().into(),
                ..row.clone()
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "personal_rerank/heap_measurement_tests.rs"]
mod heap_measurement_tests;
