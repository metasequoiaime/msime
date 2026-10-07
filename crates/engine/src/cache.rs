//! The C++ `CircularBuffer`: a bounded map that evicts the oldest *insertion*. Reads do not refresh an entry and re-inserting a key keeps its age, so this is FIFO, not LRU. The goldens depend on which entries survive (online rows inserted into the series cache in particular), so the semantics are kept exactly on top of `lru::LruCache` by never promoting.

use std::borrow::Borrow;
use std::hash::Hash;
use std::num::NonZeroUsize;

use lru::LruCache;

pub struct FifoCache<K: Hash + Eq, V> {
    entries: LruCache<K, V>,
}

impl<K: Hash + Eq, V: Clone> FifoCache<K, V> {
    /// A zero capacity is a programming error in a constant, not a runtime condition.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: LruCache::new(
                NonZeroUsize::new(capacity).expect("cache capacity is a nonzero constant"),
            ),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        self.entries.peek(key).cloned()
    }

    pub fn get_ref(&self, key: &K) -> Option<&V> {
        self.entries.peek(key)
    }

    pub fn get_ref_by<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.entries.peek(key)
    }

    pub fn contains(&self, key: &K) -> bool {
        self.entries.contains(key)
    }

    /// Updates in place without changing the entry's age; a new key evicts the oldest insertion when full.
    pub fn insert(&mut self, key: K, value: V) {
        if let Some(slot) = self.entries.peek_mut(&key) {
            *slot = value;
        } else {
            self.entries.push(key, value);
        }
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.entries.pop(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Keep entries whose values still contain useful rows. The cache is used for
    /// bounded candidate lists, so pruning a value can also remove its key without
    /// changing the insertion order of entries that remain.
    pub fn retain_mut<F>(&mut self, mut keep: F)
    where
        K: Clone,
        F: FnMut(&K, &mut V) -> bool,
    {
        let mut removed = Vec::new();
        for (key, value) in self.entries.iter_mut() {
            if !keep(key, value) {
                removed.push(key.clone());
            }
        }
        for key in removed {
            self.entries.pop(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_by_insertion_order_not_use() {
        let mut cache = FifoCache::new(2);
        cache.insert("a", 1);
        cache.insert("b", 2);
        assert_eq!(cache.get(&"a"), Some(1));
        cache.insert("a", 3);
        cache.insert("c", 4);
        assert!(!cache.contains(&"a"));
        assert_eq!(cache.get(&"b"), Some(2));
        assert_eq!(cache.get(&"c"), Some(4));
    }

    #[test]
    fn looks_up_owned_keys_by_borrowed_key() {
        let mut cache = FifoCache::new(2);
        cache.insert("a".to_owned(), 1);
        assert_eq!(cache.get_ref_by("a"), Some(&1));
    }
}
