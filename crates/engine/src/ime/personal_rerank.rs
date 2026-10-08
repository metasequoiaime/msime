//! 首组同音候选的个人上下文重排（core-session.md §7.2，`R/core/personal_context_rerank.cpp`）。

use crate::lattice::personal::{PersonalContext, PersonalNgram};
use crate::types::WordItem;

pub const MAX_GROUP: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PersonalContextRerankOrder {
    group_len: usize,
    indices: [usize; MAX_GROUP],
}

impl PersonalContextRerankOrder {
    /// 按排序索引原地重排候选行，不复制行内字符串。
    pub(crate) fn reorder<T>(&self, rows: &mut [T]) {
        debug_assert!(rows.len() >= self.group_len);
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
                rows.swap(seat, current);
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

    let group = leading_group_len(candidates);
    if group < 2 {
        return None;
    }
    let personal = personal_probabilities(&candidates[..group], model, &context);
    blended_order_indices(&candidates[..group], &personal[..group], mu, is_pinned)
}

fn personal_probabilities(
    group: &[WordItem],
    model: &PersonalNgram,
    context: &PersonalContext,
) -> [f64; MAX_GROUP] {
    let mut personal = [0.0; MAX_GROUP];
    for (index, item) in group.iter().enumerate() {
        personal[index] = model.probability(context, &item.word);
    }
    personal
}

/// 首组与第一行字数相同的连续字典候选，最多 16 行，按 Unicode 码点计数。
fn leading_group_len(candidates: &[WordItem]) -> usize {
    let characters = candidates[0].word.chars().count();
    candidates
        .iter()
        .take(MAX_GROUP)
        .take_while(|item| item.source.is_dictionary() && item.word.chars().count() == characters)
        .count()
}

#[cfg(test)]
fn blended_order(
    group: &[WordItem],
    personal: &[f64],
    mu: f64,
    is_pinned: &mut dyn FnMut(&str) -> bool,
) -> Option<Vec<usize>> {
    let order = blended_order_indices(group, personal, mu, is_pinned)?;
    Some(order.indices[..order.group_len].to_vec())
}

/// 按混合概率对首组评分；模型未知或排序未变时不返回索引。
fn blended_order_indices(
    group: &[WordItem],
    personal: &[f64],
    mu: f64,
    is_pinned: &mut dyn FnMut(&str) -> bool,
) -> Option<PersonalContextRerankOrder> {
    let personal_sum: f64 = personal.iter().sum();
    if personal_sum <= 0.0 {
        return None;
    }
    let weight = |item: &WordItem| item.weight.max(1) as f64;
    let weight_sum: f64 = group.iter().map(weight).sum();

    let mut scored = [(0.0, 0usize); MAX_GROUP];
    for (index, (item, probability)) in group.iter().zip(personal).enumerate() {
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
    Some(PersonalContextRerankOrder {
        group_len: group.len(),
        indices,
    })
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
            personal_context_rerank_order(&candidates, &model, None, "甲", &mut never_pinned())
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
            personal_context_rerank_order(&candidates, &model, None, "甲", &mut never_pinned())
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
            personal_context_rerank_order(&candidates, &model, None, "甲", &mut never_pinned())
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
        assert_eq!(leading_group_len(&list), 2);
        list[1].source = CandidateSource::CloudSuggestion;
        assert_eq!(leading_group_len(&list), 1);
        list[1].source = CandidateSource::UserDatabase;
        list[2] = row("丙", 1);
        assert_eq!(leading_group_len(&list), 4);

        let long: Vec<WordItem> = (0..20).map(|index| row("字", index)).collect();
        assert_eq!(leading_group_len(&long), MAX_GROUP);
    }
}

#[cfg(test)]
#[allow(unsafe_code)]
pub(crate) mod allocations {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
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

    // 只记录当前测试线程的分配次数，布局、指针和内存管理全部委托给 System。
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            record();
            // 安全：原样传递调用方提供的合法布局。
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // 安全：原样传递 System 分配的指针及其原始布局。
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            record();
            // 安全：原样传递 System 分配的指针、原始布局及调用方要求的大小。
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }

    struct Scope;

    impl Drop for Scope {
        fn drop(&mut self) {
            COUNT.with(|count| count.set(None));
        }
    }

    pub(crate) fn count<T>(operation: impl FnOnce() -> T) -> (T, usize) {
        COUNT.with(|count| {
            assert!(count.get().is_none());
            count.set(Some(0));
        });
        let scope = Scope;
        let result = operation();
        let allocations = COUNT.with(|count| count.get().unwrap());
        drop(scope);
        (result, allocations)
    }
}
