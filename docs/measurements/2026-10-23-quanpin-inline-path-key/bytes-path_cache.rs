use std::borrow::Borrow;
use std::hash::{Hash, Hasher};

use crate::cache::FifoCache;
use crate::types::WordItem;

const INLINE_BYTES: usize = 31;

enum PathKey {
    Inline { bytes: [u8; INLINE_BYTES], len: u8 },
    Heap(String),
}

impl PathKey {
    fn new(value: &str) -> Self {
        if value.len() <= INLINE_BYTES {
            let mut bytes = [0; INLINE_BYTES];
            bytes[..value.len()].copy_from_slice(value.as_bytes());
            Self::Inline {
                bytes,
                len: value.len() as u8,
            }
        } else {
            Self::Heap(value.to_owned())
        }
    }

    fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Inline { bytes, len } => &bytes[..usize::from(*len)],
            Self::Heap(value) => value.as_bytes(),
        }
    }
}

// 存储与查找统一借用完整字节，不需要将内联键反复转换为字符串。
impl Borrow<[u8]> for PathKey {
    fn borrow(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl PartialEq for PathKey {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for PathKey {}

impl Hash for PathKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state);
    }
}

pub(super) struct PathCache {
    entries: FifoCache<PathKey, Vec<WordItem>>,
}

impl PathCache {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            entries: FifoCache::new(capacity),
        }
    }

    pub(super) fn get_ref_by(&self, key: &str) -> Option<&Vec<WordItem>> {
        self.entries.get_ref_by(key.as_bytes())
    }

    pub(super) fn insert_ref(&mut self, key: &str, rows: Vec<WordItem>) {
        self.entries.insert(PathKey::new(key), rows);
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(super) fn insert(&mut self, key: String, rows: Vec<WordItem>) {
        let key = if key.len() <= INLINE_BYTES {
            PathKey::new(&key)
        } else {
            PathKey::Heap(key)
        };
        self.entries.insert(key, rows);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ime::personal_rerank::allocations::count;
    use std::collections::hash_map::DefaultHasher;
    use std::mem::size_of;

    fn hash(value: &impl Hash) -> u64 {
        let mut state = DefaultHasher::new();
        value.hash(&mut state);
        state.finish()
    }

    #[test]
    fn owned_and_borrowed_keys_have_identical_hash_and_equality() {
        let values = [
            "".into(),
            "a".into(),
            "a\0b".into(),
            "合成😀".into(),
            "x".repeat(30),
            "x".repeat(31),
            "x".repeat(32),
            "合成😀".repeat(8),
        ];
        for value in &values {
            let (key, allocations) = count(|| PathKey::new(value));
            assert_eq!(allocations, usize::from(value.len() > INLINE_BYTES));
            assert_eq!(key.as_bytes(), value.as_bytes());
            assert_eq!(hash(&key), hash(&value.as_bytes()));
            let heap = PathKey::Heap(value.clone());
            assert!(key == heap);
            assert_eq!(hash(&key), hash(&heap));
            for other in &values {
                assert_eq!(key == PathKey::new(other), value == other);
            }
        }
        println!(
            "path-key size={} string={} slots=128",
            size_of::<PathKey>(),
            size_of::<String>()
        );
    }

    #[test]
    fn short_long_and_unicode_keys_keep_fifo_and_prefix_boundaries() {
        let keys = [
            "a".into(),
            "ab".into(),
            "a\0b".into(),
            "x".repeat(31),
            "x".repeat(32),
            "合成😀".into(),
            "合成😀".repeat(8),
        ];
        let mut cache = PathCache::new(keys.len());
        for (index, key) in keys.iter().enumerate() {
            cache.insert_ref(
                key,
                vec![WordItem::new(
                    "ni",
                    format!("合成值{index}"),
                    index as i64,
                    crate::types::CandidateSource::Database,
                    "ni",
                )],
            );
        }
        for (index, key) in keys.iter().enumerate() {
            assert_eq!(cache.get_ref_by(key).unwrap()[0].weight, index as i64);
            assert!(cache.get_ref_by(&format!("{key}z")).is_none());
        }
        cache.insert_ref(&keys[0], Vec::new());
        cache.insert_ref("合成新键", Vec::new());
        assert!(cache.get_ref_by(&keys[0]).is_none());
        for key in &keys[1..] {
            assert!(cache.get_ref_by(key).is_some());
        }
        cache.clear();
        for key in &keys {
            assert!(cache.get_ref_by(key).is_none());
        }
    }
}
