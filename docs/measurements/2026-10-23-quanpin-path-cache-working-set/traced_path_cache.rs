use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::size_of;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::cache::FifoCache;
use crate::types::WordItem;

static NEXT_CACHE: AtomicU64 = AtomicU64::new(0);

/// 仅用于独立诊断构建；原缓存拥有值、查找和 FIFO 操作全部转发。
pub(super) struct TracedPathCache {
    inner: FifoCache<String, Vec<WordItem>>,
    ids: RefCell<HashMap<String, u64>>,
    id: u64,
}

impl TracedPathCache {
    pub(super) fn new(capacity: usize) -> Self {
        let id = NEXT_CACHE.fetch_add(1, Ordering::Relaxed);
        eprintln!("FIFO_TRACE N {id} {capacity} {}", size_of::<WordItem>());
        Self {
            inner: FifoCache::new(capacity),
            ids: RefCell::new(HashMap::new()),
            id,
        }
    }

    fn key_id(&self, key: &str) -> u64 {
        let mut ids = self.ids.borrow_mut();
        if let Some(id) = ids.get(key) {
            return *id;
        }
        let id = ids.len() as u64;
        ids.insert(key.to_owned(), id);
        id
    }

    pub(super) fn get_ref_by(&self, key: &str) -> Option<&Vec<WordItem>> {
        let rows = self.inner.get_ref_by(key);
        eprintln!(
            "FIFO_TRACE G {} {} {}",
            self.id,
            self.key_id(key),
            usize::from(rows.is_some())
        );
        rows
    }

    /// 前缀去重的可选探测没有查库义务，须与真正查询区分。
    pub(super) fn peek_ref_by(&self, key: &str) -> Option<&Vec<WordItem>> {
        let rows = self.inner.get_ref_by(key);
        eprintln!(
            "FIFO_TRACE P {} {} {}",
            self.id,
            self.key_id(key),
            usize::from(rows.is_some())
        );
        rows
    }

    pub(super) fn insert(&mut self, key: String, rows: Vec<WordItem>) {
        let mut bytes = rows.capacity() * size_of::<WordItem>();
        let mut allocations = usize::from(rows.capacity() != 0);
        let mut digest = Sha256::new();
        for row in &rows {
            for field in [
                &row.pinyin,
                &row.canonical_pinyin,
                &row.word,
                &row.corrected_from,
            ] {
                bytes += field.capacity();
                allocations += usize::from(field.capacity() != 0);
                digest.update((field.len() as u64).to_le_bytes());
                digest.update(field.as_bytes());
            }
            digest.update(row.weight.to_le_bytes());
            digest.update([row.source as u8, row.scheme as u8]);
            digest.update(row.fixed_position.to_le_bytes());
            digest.update([u8::from(row.fuzzy), u8::from(row.sentence_association)]);
            bytes += row.sentence_words.capacity() * size_of::<String>();
            allocations += usize::from(row.sentence_words.capacity() != 0);
            digest.update((row.sentence_words.len() as u64).to_le_bytes());
            for word in &row.sentence_words {
                bytes += word.capacity();
                allocations += usize::from(word.capacity() != 0);
                digest.update((word.len() as u64).to_le_bytes());
                digest.update(word.as_bytes());
            }
        }
        eprintln!(
            "FIFO_TRACE S {} {} {} {} {} {} {}",
            self.id,
            self.key_id(&key),
            rows.len(),
            key.capacity(),
            bytes,
            allocations,
            hex::encode(digest.finalize())
        );
        self.inner.insert(key, rows);
    }

    pub(super) fn clear(&mut self) {
        eprintln!("FIFO_TRACE C {}", self.id);
        self.inner.clear();
    }
}

impl Drop for TracedPathCache {
    fn drop(&mut self) {
        eprintln!("FIFO_TRACE D {}", self.id);
    }
}
