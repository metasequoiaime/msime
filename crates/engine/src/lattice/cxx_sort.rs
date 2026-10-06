//! The two unstable sorts the C++ decoder orders its beam with, `std::partial_sort` (WL:105) and `std::sort` (WL:447), reproduced step for step from the libc++ the reference was built with (AppleClang 21, `__algorithm/partial_sort.h`, `sort.h`, `sift_down.h`, `pop_heap.h`, `push_heap.h`).
//!
//! Hypotheses with equal scores are common: every zero-weight character of a syllable (all 79 qia rows of the shipped msime-pinyin.db weigh 0) scores the same after the same prefix, and so do homophones the n-gram tables know nothing about. Which of them survive the beam and in what order they are listed is decided only by how the sort moves equal elements, so a stable sort shows different sentences (上恰 where the reference shows 上㓣) and swaps tied rows (王芳使我的同时 / 王芳使我的同事). Neither Rust's `sort_unstable` nor `select_nth_unstable` moves elements the way libc++ does, hence the port.
//!
//! Only the paths the comparator of a lambda over a non-arithmetic type takes are ported: `__assume_both_children` is false in the heap, and the introsort partitions with `__partition_with_equals_on_right`, never the bitset partition. `before(a, b)` is the C++ comparator, true when `a` goes first.

/// `std::partial_sort(v.begin(), v.begin() + middle, v.end(), before)`: afterwards `v[..middle]` holds the first `middle` elements in order and the rest is left in the heap selection's order.
pub(super) fn partial_sort<T: Copy>(
    v: &mut [T],
    middle: usize,
    before: &mut impl FnMut(&T, &T) -> bool,
) {
    if middle == 0 {
        return;
    }
    make_heap(&mut v[..middle], before);
    for index in middle..v.len() {
        if before(&v[index], &v[0]) {
            v.swap(index, 0);
            sift_down(&mut v[..middle], before, 0);
        }
    }
    sort_heap(&mut v[..middle], before);
}

/// `std::sort(v.begin(), v.end(), before)`.
pub(super) fn sort<T: Copy>(v: &mut [T], before: &mut impl FnMut(&T, &T) -> bool) {
    if v.is_empty() {
        return;
    }
    // `2 * std::__bit_log2(len)`.
    let depth = 2 * (usize::BITS - 1 - v.len().leading_zeros()) as usize;
    introsort(v, 0, v.len(), before, depth, true);
}

fn make_heap<T: Copy>(v: &mut [T], before: &mut impl FnMut(&T, &T) -> bool) {
    let n = v.len();
    if n > 1 {
        for start in (0..=(n - 2) / 2).rev() {
            sift_down(v, before, start);
        }
    }
}

/// `__sift_down<_, false>` over the whole of `v`.
fn sift_down<T: Copy>(v: &mut [T], before: &mut impl FnMut(&T, &T) -> bool, mut start: usize) {
    let len = v.len();
    if len < 2 || (len - 2) / 2 < start {
        return;
    }
    let mut child = 2 * start + 1;
    if child + 1 < len && before(&v[child], &v[child + 1]) {
        child += 1;
    }
    if before(&v[child], &v[start]) {
        return;
    }
    let top = v[start];
    loop {
        v[start] = v[child];
        start = child;
        if (len - 2) / 2 < child {
            break;
        }
        child = 2 * child + 1;
        if child + 1 < len && before(&v[child], &v[child + 1]) {
            child += 1;
        }
        if before(&v[child], &top) {
            break;
        }
    }
    v[start] = top;
}

/// `__floyd_sift_down`: walks the hole at the root down to a leaf along the larger children and returns where it ended.
fn floyd_sift_down<T: Copy>(v: &mut [T], before: &mut impl FnMut(&T, &T) -> bool) -> usize {
    let len = v.len();
    let mut hole = 0;
    let mut child = 0;
    loop {
        child = 2 * child + 1;
        if child + 1 < len && before(&v[child], &v[child + 1]) {
            child += 1;
        }
        v[hole] = v[child];
        hole = child;
        if child > (len - 2) / 2 {
            return hole;
        }
    }
}

/// `__sift_up` of the last element of `v`.
fn sift_up<T: Copy>(v: &mut [T], before: &mut impl FnMut(&T, &T) -> bool) {
    let len = v.len();
    if len <= 1 {
        return;
    }
    let mut parent = (len - 2) / 2;
    let mut last = len - 1;
    if !before(&v[parent], &v[last]) {
        return;
    }
    let moved = v[last];
    loop {
        v[last] = v[parent];
        last = parent;
        if parent == 0 {
            break;
        }
        parent = (parent - 1) / 2;
        if !before(&v[parent], &moved) {
            break;
        }
    }
    v[last] = moved;
}

/// `__pop_heap` of the whole of `v`: the root moves to the end.
fn pop_heap<T: Copy>(v: &mut [T], before: &mut impl FnMut(&T, &T) -> bool) {
    let len = v.len();
    if len <= 1 {
        return;
    }
    let top = v[0];
    let mut hole = floyd_sift_down(v, before);
    let last = len - 1;
    if hole == last {
        v[hole] = top;
    } else {
        v[hole] = v[last];
        hole += 1;
        v[last] = top;
        sift_up(&mut v[..hole], before);
    }
}

fn sort_heap<T: Copy>(v: &mut [T], before: &mut impl FnMut(&T, &T) -> bool) {
    for n in (2..=v.len()).rev() {
        pop_heap(&mut v[..n], before);
    }
}

/// The branching `__sort3`.
fn sort3<T: Copy>(
    v: &mut [T],
    x: usize,
    y: usize,
    z: usize,
    before: &mut impl FnMut(&T, &T) -> bool,
) {
    if !before(&v[y], &v[x]) {
        if !before(&v[z], &v[y]) {
            return;
        }
        v.swap(y, z);
        if before(&v[y], &v[x]) {
            v.swap(x, y);
        }
        return;
    }
    if before(&v[z], &v[y]) {
        v.swap(x, z);
        return;
    }
    v.swap(x, y);
    if before(&v[z], &v[y]) {
        v.swap(y, z);
    }
}

fn sort4<T: Copy>(v: &mut [T], x: [usize; 4], before: &mut impl FnMut(&T, &T) -> bool) {
    sort3(v, x[0], x[1], x[2], before);
    if before(&v[x[3]], &v[x[2]]) {
        v.swap(x[2], x[3]);
        if before(&v[x[2]], &v[x[1]]) {
            v.swap(x[1], x[2]);
            if before(&v[x[1]], &v[x[0]]) {
                v.swap(x[0], x[1]);
            }
        }
    }
}

fn sort5<T: Copy>(v: &mut [T], x: [usize; 5], before: &mut impl FnMut(&T, &T) -> bool) {
    sort4(v, [x[0], x[1], x[2], x[3]], before);
    if before(&v[x[4]], &v[x[3]]) {
        v.swap(x[3], x[4]);
        if before(&v[x[3]], &v[x[2]]) {
            v.swap(x[2], x[3]);
            if before(&v[x[2]], &v[x[1]]) {
                v.swap(x[1], x[2]);
                if before(&v[x[1]], &v[x[0]]) {
                    v.swap(x[0], x[1]);
                }
            }
        }
    }
}

/// `__insertion_sort` over `v[first..last]`.
fn insertion_sort<T: Copy>(
    v: &mut [T],
    first: usize,
    last: usize,
    before: &mut impl FnMut(&T, &T) -> bool,
) {
    if first == last {
        return;
    }
    for i in first + 1..last {
        if before(&v[i], &v[i - 1]) {
            let moved = v[i];
            let mut k = i - 1;
            let mut j = i;
            loop {
                v[j] = v[k];
                j = k;
                if j == first {
                    break;
                }
                k -= 1;
                if !before(&moved, &v[k]) {
                    break;
                }
            }
            v[j] = moved;
        }
    }
}

/// `__insertion_sort_unguarded`: `v[first - 1]` goes before or ties with everything in the range, so the inner loop needs no bound.
fn insertion_sort_unguarded<T: Copy>(
    v: &mut [T],
    first: usize,
    last: usize,
    before: &mut impl FnMut(&T, &T) -> bool,
) {
    if first == last {
        return;
    }
    for i in first + 1..last {
        if before(&v[i], &v[i - 1]) {
            let moved = v[i];
            let mut k = i - 1;
            let mut j = i;
            loop {
                v[j] = v[k];
                j = k;
                k -= 1;
                if !before(&moved, &v[k]) {
                    break;
                }
            }
            v[j] = moved;
        }
    }
}

/// `__insertion_sort_incomplete`: gives up after eight moves and reports whether `v[first..last]` ended up sorted.
fn insertion_sort_incomplete<T: Copy>(
    v: &mut [T],
    first: usize,
    last: usize,
    before: &mut impl FnMut(&T, &T) -> bool,
) -> bool {
    match last - first {
        0 | 1 => return true,
        2 => {
            if before(&v[last - 1], &v[first]) {
                v.swap(first, last - 1);
            }
            return true;
        }
        3 => {
            sort3(v, first, first + 1, last - 1, before);
            return true;
        }
        4 => {
            sort4(v, [first, first + 1, first + 2, last - 1], before);
            return true;
        }
        5 => {
            sort5(
                v,
                [first, first + 1, first + 2, first + 3, last - 1],
                before,
            );
            return true;
        }
        _ => {}
    }
    const LIMIT: usize = 8;
    let mut j = first + 2;
    sort3(v, first, first + 1, j, before);
    let mut count = 0;
    for i in j + 1..last {
        if before(&v[i], &v[j]) {
            let moved = v[i];
            let mut k = j;
            j = i;
            loop {
                v[j] = v[k];
                j = k;
                if j == first {
                    break;
                }
                k -= 1;
                if !before(&moved, &v[k]) {
                    break;
                }
            }
            v[j] = moved;
            count += 1;
            if count == LIMIT {
                return i + 1 == last;
            }
        }
        j = i;
    }
    true
}

/// `__partition_with_equals_on_right` of `v[first..last]` around `v[first]`: returns the pivot's final index and whether the range was already partitioned.
fn partition_with_equals_on_right<T: Copy>(
    v: &mut [T],
    first: usize,
    last: usize,
    before: &mut impl FnMut(&T, &T) -> bool,
) -> (usize, bool) {
    let begin = first;
    let pivot = v[first];
    let mut first = first;
    let mut last = last;
    loop {
        first += 1;
        if !before(&v[first], &pivot) {
            break;
        }
    }
    if begin == first - 1 {
        while first < last {
            last -= 1;
            if before(&v[last], &pivot) {
                break;
            }
        }
    } else {
        loop {
            last -= 1;
            if before(&v[last], &pivot) {
                break;
            }
        }
    }
    let already_partitioned = first >= last;
    while first < last {
        v.swap(first, last);
        loop {
            first += 1;
            if !before(&v[first], &pivot) {
                break;
            }
        }
        loop {
            last -= 1;
            if before(&v[last], &pivot) {
                break;
            }
        }
    }
    let pivot_at = first - 1;
    if begin != pivot_at {
        v[begin] = v[pivot_at];
    }
    v[pivot_at] = pivot;
    (pivot_at, already_partitioned)
}

/// `__partition_with_equals_on_left`: elements tying with the pivot stay left of it. Returns the index after the pivot.
fn partition_with_equals_on_left<T: Copy>(
    v: &mut [T],
    first: usize,
    last: usize,
    before: &mut impl FnMut(&T, &T) -> bool,
) -> usize {
    let begin = first;
    let end = last;
    let pivot = v[first];
    let mut first = first;
    let mut last = last;
    if before(&pivot, &v[end - 1]) {
        loop {
            first += 1;
            if before(&pivot, &v[first]) {
                break;
            }
        }
    } else {
        loop {
            first += 1;
            if !(first < end && !before(&pivot, &v[first])) {
                break;
            }
        }
    }
    if first < last {
        loop {
            last -= 1;
            if !before(&pivot, &v[last]) {
                break;
            }
        }
    }
    while first < last {
        v.swap(first, last);
        loop {
            first += 1;
            if before(&pivot, &v[first]) {
                break;
            }
        }
        loop {
            last -= 1;
            if !before(&pivot, &v[last]) {
                break;
            }
        }
    }
    let pivot_at = first - 1;
    if begin != pivot_at {
        v[begin] = v[pivot_at];
    }
    v[pivot_at] = pivot;
    first
}

/// `__introsort<_, _, _, false>` over `v[first..last]`.
fn introsort<T: Copy>(
    v: &mut [T],
    mut first: usize,
    mut last: usize,
    before: &mut impl FnMut(&T, &T) -> bool,
    mut depth: usize,
    mut leftmost: bool,
) {
    const INSERTION_LIMIT: usize = 24;
    const NINTHER_THRESHOLD: usize = 128;
    loop {
        let len = last - first;
        match len {
            0 | 1 => return,
            2 => {
                if before(&v[last - 1], &v[first]) {
                    v.swap(first, last - 1);
                }
                return;
            }
            3 => {
                sort3(v, first, first + 1, last - 1, before);
                return;
            }
            4 => {
                sort4(v, [first, first + 1, first + 2, last - 1], before);
                return;
            }
            5 => {
                sort5(
                    v,
                    [first, first + 1, first + 2, first + 3, last - 1],
                    before,
                );
                return;
            }
            _ => {}
        }
        if len < INSERTION_LIMIT {
            if leftmost {
                insertion_sort(v, first, last, before);
            } else {
                insertion_sort_unguarded(v, first, last, before);
            }
            return;
        }
        if depth == 0 {
            // Heap sort, as introsort falls back to: `__partial_sort(first, last, last)`.
            let range = &mut v[first..last];
            let whole = range.len();
            partial_sort(range, whole, before);
            return;
        }
        depth -= 1;
        let half = len / 2;
        if len > NINTHER_THRESHOLD {
            sort3(v, first, first + half, last - 1, before);
            sort3(v, first + 1, first + half - 1, last - 2, before);
            sort3(v, first + 2, first + half + 1, last - 3, before);
            sort3(v, first + half - 1, first + half, first + half + 1, before);
            v.swap(first, first + half);
        } else {
            sort3(v, first + half, first, last - 1, before);
        }
        if !leftmost && !before(&v[first - 1], &v[first]) {
            first = partition_with_equals_on_left(v, first, last, before);
            continue;
        }
        let (pivot, already_partitioned) = partition_with_equals_on_right(v, first, last, before);
        if already_partitioned {
            let left_sorted = insertion_sort_incomplete(v, first, pivot, before);
            if insertion_sort_incomplete(v, pivot + 1, last, before) {
                if left_sorted {
                    return;
                }
                last = pivot;
                continue;
            } else if left_sorted {
                first = pivot + 1;
                continue;
            }
        }
        introsort(v, first, pivot, before, depth, leftmost);
        leftmost = false;
        first = pivot + 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (score, insertion index): the index shows where a tie ended up.
    fn tied(scores: &[i32]) -> Vec<(i32, usize)> {
        scores.iter().copied().zip(0..).collect()
    }

    fn descending(a: &(i32, usize), b: &(i32, usize)) -> bool {
        a.0 > b.0
    }

    /// A small deterministic generator, so the property tests cover many shapes without a dependency.
    fn scores(seed: u64, len: usize, distinct: i32) -> Vec<i32> {
        let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        (0..len)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                ((state >> 33) % distinct as u64) as i32
            })
            .collect()
    }

    fn assert_permutation(sorted: &[(i32, usize)], len: usize) {
        let mut indices: Vec<usize> = sorted.iter().map(|entry| entry.1).collect();
        indices.sort_unstable();
        assert_eq!(indices, (0..len).collect::<Vec<_>>());
    }

    #[test]
    fn sort_orders_every_shape() {
        for len in 0..300 {
            for distinct in [1, 2, 3, 7, 1000] {
                let original = tied(&scores(len as u64 * 31 + distinct as u64, len, distinct));
                let mut v = original.clone();
                sort(&mut v, &mut descending);
                assert!(
                    v.windows(2).all(|pair| pair[0].0 >= pair[1].0),
                    "len {len} distinct {distinct}: {v:?}"
                );
                assert_permutation(&v, len);
            }
        }
    }

    #[test]
    fn partial_sort_selects_the_best_prefix() {
        for len in 0..200 {
            for middle in [0, 1, 5, 32, len / 2, len] {
                if middle > len {
                    continue;
                }
                let original = tied(&scores(len as u64 * 17 + middle as u64, len, 5));
                let mut v = original.clone();
                partial_sort(&mut v, middle, &mut descending);
                assert!(v[..middle].windows(2).all(|pair| pair[0].0 >= pair[1].0));
                if middle > 0 && middle < len {
                    let worst_kept = v[middle - 1].0;
                    assert!(v[middle..].iter().all(|entry| entry.0 <= worst_kept));
                }
                assert_permutation(&v, len);
            }
        }
    }

    /// The tie orders below were traced by hand through libc++'s code, the order the reference decoder produced.
    #[test]
    fn short_ranges_keep_insertion_order_on_ties() {
        // Under 24 elements `std::sort` is an insertion sort, which never moves an element past an equal one.
        let mut v = tied(&[1, 2, 1, 2, 1, 2, 1, 2, 1, 2]);
        sort(&mut v, &mut descending);
        let order: Vec<usize> = v.iter().map(|entry| entry.1).collect();
        assert_eq!(order, [1, 3, 5, 7, 9, 0, 2, 4, 6, 8]);
    }

    #[test]
    fn heap_selection_reorders_ties() {
        // make_heap([0, 1, 2]): the root's larger child is 1 (a tie never prefers the right child) and a tie does not stop the sift, so 1 and 0 trade places: [1, 0, 2]. Element 3 does not go before the root and is left out. sort_heap then pops 1 to the back (Floyd's sift moves 0 up, 2 fills the hole, nothing sifts up): [0, 2, 1], and pops 0: [2, 0, 1].
        let mut v = tied(&[5, 5, 5, 5]);
        partial_sort(&mut v, 3, &mut descending);
        let order: Vec<usize> = v.iter().map(|entry| entry.1).collect();
        assert_eq!(order, [2, 0, 1, 3]);
    }

    #[test]
    fn long_all_tied_range_is_reversed_around_the_pivot() {
        // 24 ties: median-of-three changes nothing, the right-hand partition finds no element before the pivot, so `first` stops at 1, `last` walks down to 1 and the range counts as already partitioned; the pivot stays at 0 and both insertion sorts see only ties.
        let mut v = tied(&[0; 24]);
        sort(&mut v, &mut descending);
        let order: Vec<usize> = v.iter().map(|entry| entry.1).collect();
        assert_eq!(order, (0..24).collect::<Vec<_>>());
    }
}
