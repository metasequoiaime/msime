//! Personal context reorder of the leading homophone group (core-session.md §7.2, `R/core/personal_context_rerank.cpp`).

use crate::lattice::personal::{PersonalContext, PersonalNgram};
use crate::types::WordItem;

pub const MAX_GROUP: usize = 16;

/// The reordered list when the model moves something in the leading same-length dictionary group, else `None`. A pinned leader (`is_pinned(word)`) keeps its seat.
pub fn personal_context_rerank(
    candidates: &[WordItem],
    model: &PersonalNgram,
    earlier: Option<&str>,
    previous: &str,
    is_pinned: &mut dyn FnMut(&str) -> bool,
) -> Option<Vec<WordItem>> {
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
    let order = blended_order(&candidates[..group], &personal[..group], mu, is_pinned)?;

    let mut ordered = Vec::with_capacity(candidates.len());
    ordered.extend(order.into_iter().map(|index| candidates[index].clone()));
    ordered.extend_from_slice(&candidates[group..]);
    Some(ordered)
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

/// The leading run, at most 16 rows, of dictionary rows with as many characters as the first (code points, like `count_utf8_chars`).
fn leading_group_len(candidates: &[WordItem]) -> usize {
    let characters = candidates[0].word.chars().count();
    candidates
        .iter()
        .take(MAX_GROUP)
        .take_while(|item| item.source.is_dictionary() && item.word.chars().count() == characters)
        .count()
}

/// Indices of `group` ordered by `ln((1 - mu) * share + mu * personal / sum(personal))`, where `share` is the row's weight (at least 1) over the group's; `None` when the model knows none of the words or the order would not change.
fn blended_order(
    group: &[WordItem],
    personal: &[f64],
    mu: f64,
    is_pinned: &mut dyn FnMut(&str) -> bool,
) -> Option<Vec<usize>> {
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
    // Stable, so equal scores keep the dictionary order.
    scored.sort_by(|left, right| right.0.total_cmp(&left.0));

    // The pin lookup reads the journal, so it is only asked when the leader would actually lose its seat.
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
    let order: Vec<usize> = scored.iter().map(|&(_, index)| index).collect();
    Some(order)
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
            personal_context_rerank(&candidates, &model, None, "甲", &mut never_pinned())
        });

        assert_eq!(ordered, None);
        assert_eq!(allocations, 0);
    }

    #[test]
    fn changed_personal_order_only_allocates_the_result_and_indices() {
        let mut model = PersonalNgram::default();
        model.add_pair("甲", "乙", 32);
        let mut candidates = vec![row("丙", 1); MAX_GROUP];
        candidates[MAX_GROUP - 1] = row("乙", 1);
        candidates.push(row("丁", 1));
        let (cloned, result_allocations) = allocations::count(|| candidates.clone());
        drop(cloned);

        let (ordered, allocations) = allocations::count(|| {
            personal_context_rerank(&candidates, &model, None, "甲", &mut never_pinned())
        });

        let ordered = ordered.expect("个人上下文应把最后一个组内候选提到首位");
        assert_eq!(ordered[0].word, "乙");
        assert_eq!(&ordered[1..MAX_GROUP], &candidates[..MAX_GROUP - 1]);
        assert_eq!(ordered[MAX_GROUP], candidates[MAX_GROUP]);
        assert_eq!(allocations, result_allocations + 1);
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
mod allocations {
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

    pub(super) fn count<T>(operation: impl FnOnce() -> T) -> (T, usize) {
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
