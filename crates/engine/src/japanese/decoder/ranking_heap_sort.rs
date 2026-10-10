use super::*;

// 单键堆排序的备选完整核心，仅供本地 release 对照。
pub(super) fn best_ids_from_iter<I>(ids: I, limit: usize, cost: impl Fn(u32) -> i32) -> Vec<u32>
where
    I: IntoIterator<Item = u32>,
{
    if limit == 0 {
        return Vec::new();
    }
    let mut ids = ids.into_iter();
    let Some(first) = ids.next() else {
        return Vec::new();
    };
    let mut best: BinaryHeap<(i32, u32)> = BinaryHeap::with_capacity(limit);
    for id in std::iter::once(first).chain(ids) {
        let key = (cost(id), id);
        if best.len() < limit {
            best.push(key);
        } else if key < *best.peek().expect("non-empty bounded heap") {
            best.pop();
            best.push(key);
        }
    }
    // 成本已随 ID 保存，直接按堆键升序输出，不再次读取模型成本。
    best.into_sorted_vec()
        .into_iter()
        .map(|(_, id)| id)
        .collect()
}

impl JapaneseDictionary {
    pub(super) fn ranked_heap_sort_prefix_lemmas_continuing_with<T>(
        &self,
        prefix: &str,
        next_kana: &[&str],
        limit: usize,
        build: impl FnMut(u32) -> T,
    ) -> Vec<T> {
        if prefix.is_empty() || next_kana.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut best: BinaryHeap<(i32, u32)> = BinaryHeap::new();
        let mut consider = |id: u32| {
            let key = (self.cost_of(id), id);
            if best.len() < limit {
                if best.is_empty() {
                    best.reserve_exact(limit);
                }
                best.push(key);
            } else if key < *best.peek().expect("non-empty bounded heap") {
                best.pop();
                best.push(key);
            }
        };
        let mut query = String::new();
        for kana in next_kana {
            if kana.is_empty() {
                for index in self.lower_bound(prefix)..self.token_count {
                    let reading = self.reading(&self.token_at(index));
                    let Some(remaining) = reading.strip_prefix(prefix) else {
                        break;
                    };
                    if !remaining.is_empty() {
                        consider(index as u32);
                    }
                }
                continue;
            }
            if query.is_empty() {
                // 只在首个非空后缀预留最长键容量，公共前缀在整个查询中保留。
                let suffix_capacity = if next_kana.len() == 1 {
                    kana.len()
                } else {
                    next_kana
                        .iter()
                        .map(|suffix| suffix.len())
                        .max()
                        .unwrap_or(0)
                };
                query.reserve_exact(prefix.len() + suffix_capacity);
                query.push_str(prefix);
            }
            query.truncate(prefix.len());
            query.push_str(kana);
            let start = self.lower_bound(&query);
            for index in start..self.token_count {
                if !self.reading(&self.token_at(index)).starts_with(&query) {
                    break;
                }
                consider(index as u32);
            }
        }
        // 已保存的键同时决定筛选和输出顺序；保留重叠后缀产生的重复 ID。
        best.into_sorted_vec()
            .into_iter()
            .map(|(_, id)| id)
            .map(build)
            .collect()
    }
    pub(super) fn ranked_heap_sort_exact_lemmas_with<T>(
        &self,
        reading: &str,
        limit: usize,
        build: impl FnMut(u32) -> T,
    ) -> Vec<T> {
        if reading.is_empty() || limit == 0 {
            return Vec::new();
        }
        let start = self.lower_bound(reading);
        let ids = (start..self.token_count)
            .take_while(|&index| self.reading(&self.token_at(index)) == reading)
            .map(|index| index as u32);
        best_ids_from_iter(ids, limit, |id| self.cost_of(id))
            .into_iter()
            .map(build)
            .collect()
    }
    pub(super) fn ranked_heap_sort_prefix_lemmas_with<T>(
        &self,
        prefix: &str,
        limit: usize,
        mut build: impl FnMut(u32) -> T,
    ) -> Vec<T> {
        if prefix.is_empty() || limit == 0 {
            return Vec::new();
        }
        if let Some(cached) = self.short_prefix_index.get(prefix) {
            if limit <= SHORT_PREFIX_CANDIDATE_COUNT {
                return cached.iter().take(limit).map(|&id| build(id)).collect();
            }
        }
        let start = self.lower_bound(prefix);
        let ids = (start..self.token_count)
            .take_while(|&index| self.reading(&self.token_at(index)).starts_with(prefix))
            .map(|index| index as u32);
        best_ids_from_iter(ids, limit, |id| self.cost_of(id))
            .into_iter()
            .map(build)
            .collect()
    }
}
