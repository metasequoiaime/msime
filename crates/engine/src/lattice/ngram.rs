//! `bigram.bin` / `trigram.bin` (MSNG v1, quanpin.md §12.1, data-formats.md §7). The files are copied into a generation through `<name>.incoming` and renamed into place, never written in place, so a table read once stays the table of that generation.
//!
//! The table is mapped read-only, as ngram_table.cpp:108 did (MapViewOfFile on Windows, :131-133), so its pages are clean and file-backed: the system can evict them under memory pressure, which the iOS keyboard extension's limit needs, instead of holding two 12 MB tables of dirty heap per generation. The `unsafe` map (decisions.md: memmap2 for bigram.bin/trigram.bin; `japanese::decoder` and `handwriting::recognizer` map their packaged models the same way) rests on the generation contract above. Like the reference, a table stays mapped for the life of the process once loaded.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use memmap2::{Mmap, MmapOptions};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

/// The sentence-start token in n-gram keys. Not a character any dictionary value can contain, so it cannot be confused with a real first word (NG:61-66).
pub const SENTENCE_START: &str = "\u{1}";
/// An entry count past any plausible table: a truncated or foreign file that happens to carry the magic must not turn into a nonsense allocation (NG:29-31).
pub const MAX_ENTRIES: usize = 40_000_000;

const MAGIC: &[u8; 4] = b"MSNG";
const VERSION: u32 = 1;
/// Magic, version, count, reserved.
const HEADER_BYTES: usize = 16;
const ENTRY_BYTES: usize = 8 + 4;

const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01B3;

pub struct NgramTable {
    /// The header and the entries, exactly `HEADER_BYTES + count * ENTRY_BYTES` bytes.
    bytes: Mmap,
    count: usize,
}

impl NgramTable {
    /// `None` for a missing, short, wrong-magic, wrong-version, oversized or unsorted file; the decoder then runs without the table.
    #[allow(unsafe_code)]
    pub fn load(path: &Path) -> Option<NgramTable> {
        if !std::fs::symlink_metadata(path).ok()?.file_type().is_file() {
            return None;
        }
        let mut file = File::open(path).ok()?;
        let size = file.metadata().ok()?.len();
        let mut header = [0u8; HEADER_BYTES];
        file.read_exact(&mut header).ok()?;
        if &header[..4] != MAGIC || read_u32(&header, 4) != VERSION {
            return None;
        }
        let count = read_u32(&header, 8) as usize;
        let needed = HEADER_BYTES + count * ENTRY_BYTES;
        if count > MAX_ENTRIES || needed as u64 > size {
            return None;
        }
        // Only `needed` bytes are mapped: trailing bytes are legal (NG:124-126), and the header alone bounds the mapping, so a foreign file cannot make it larger than the largest valid table.
        // SAFETY: a mapping is only sound while nothing changes the file underneath it. Tables reach a generation as `<name>.incoming` and are renamed into place, never written in place (module doc, assets.rs), so the mapped inode keeps its bytes for as long as the map lives; `needed <= size` was checked above.
        let bytes = unsafe { MmapOptions::new().len(needed).map(&file) }.ok()?;
        if bytes.len() != needed {
            return None;
        }
        let table = NgramTable { bytes, count };
        // Binary search returns wrong answers rather than misses on an unsorted file, so a file built by a different tool is refused here (NG:131-134).
        if (1..count).any(|index| table.key_at(index - 1) > table.key_at(index)) {
            return None;
        }
        Some(table)
    }

    /// One table per path for the process, remembering a missing file too, so a generation without tables is not probed on every query (NG:138-147).
    pub fn shared(path: &Path) -> Option<Arc<NgramTable>> {
        static LOADED: OnceLock<Mutex<HashMap<PathBuf, Option<Arc<NgramTable>>>>> = OnceLock::new();
        let mut loaded = LOADED
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        loaded
            .entry(path.to_path_buf())
            .or_insert_with(|| NgramTable::load(path).map(Arc::new))
            .clone()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.count
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// `ln(P(next | previous) / P(next))`, 0 on a miss or an empty `next`.
    pub fn bigram(&self, previous: &str, next: &str) -> f32 {
        if self.count == 0 || next.is_empty() {
            return 0.0;
        }
        self.lookup(fnv1a_words(&[previous, next]))
    }

    /// `ln(P(next | before, previous) / P(next | previous))`, 0 on a miss.
    pub fn trigram(&self, before: &str, previous: &str, next: &str) -> f32 {
        if self.count == 0 || next.is_empty() {
            return 0.0;
        }
        self.lookup(fnv1a_words(&[before, previous, next]))
    }

    fn key_at(&self, index: usize) -> u64 {
        let at = HEADER_BYTES + index * 8;
        u64::from_le_bytes(self.bytes[at..at + 8].try_into().expect("eight key bytes"))
    }

    fn value_at(&self, index: usize) -> f32 {
        let at = HEADER_BYTES + self.count * 8 + index * 4;
        f32::from_le_bytes(self.bytes[at..at + 4].try_into().expect("four value bytes"))
    }

    /// `std::lower_bound` over the key array, then an equality check (NG:149-155).
    fn lookup(&self, key: u64) -> f32 {
        let (mut low, mut high) = (0, self.count);
        while low < high {
            let middle = low + (high - low) / 2;
            if self.key_at(middle) < key {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        if low < self.count && self.key_at(low) == key {
            self.value_at(low)
        } else {
            0.0
        }
    }
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four header bytes"))
}

/// Mixes `text`'s bytes into an FNV-1a state.
pub(crate) fn fnv1a_mix(mut state: u64, text: &str) -> u64 {
    for byte in text.bytes() {
        state ^= u64::from(byte);
        state = state.wrapping_mul(FNV_PRIME);
    }
    state
}

/// The 0x00 byte between two words; xor with zero is a no-op, so only the multiply remains.
pub(crate) fn fnv1a_separate(state: u64) -> u64 {
    state.wrapping_mul(FNV_PRIME)
}

/// 64-bit FNV-1a over the words joined by one 0x00 byte (NG:40-57); the personal model derives its keys from the same hash.
pub fn fnv1a_words(words: &[&str]) -> u64 {
    let mut state = FNV_OFFSET;
    for (index, word) in words.iter().enumerate() {
        if index > 0 {
            state = fnv1a_separate(state);
        }
        state = fnv1a_mix(state, word);
    }
    state
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// Recomputed here rather than borrowed from the implementation: the table is written by `build_ngram.py`, so the format is a contract between two programs, and a test that reused the engine's own hash would keep passing if both sides drifted together.
    fn hash_joined(joined: &[u8]) -> u64 {
        let mut digest: u64 = 0xCBF29CE484222325;
        for &byte in joined {
            digest ^= u64::from(byte);
            digest = digest.wrapping_mul(0x100000001B3);
        }
        digest
    }

    fn hash_pair(previous: &str, next: &str) -> u64 {
        hash_joined(format!("{previous}\0{next}").as_bytes())
    }

    fn hash_triple(before: &str, previous: &str, next: &str) -> u64 {
        hash_joined(format!("{before}\0{previous}\0{next}").as_bytes())
    }

    pub(crate) fn write_table(path: &Path, entries: &[(u64, f32)], magic: &[u8; 4], version: u32) {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(magic);
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        for (key, _) in entries {
            bytes.extend_from_slice(&key.to_le_bytes());
        }
        for (_, value) in entries {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }

    pub(crate) fn sorted(mut entries: Vec<(u64, f32)>) -> Vec<(u64, f32)> {
        entries.sort_by_key(|entry| entry.0);
        entries
    }

    /// A valid table holding the given pairs, written into `directory`.
    pub(crate) fn pair_table(
        directory: &Path,
        name: &str,
        pairs: &[(&str, &str, f32)],
    ) -> NgramTable {
        let path = directory.join(name);
        let entries = pairs
            .iter()
            .map(|(previous, next, value)| (hash_pair(previous, next), *value))
            .collect();
        write_table(&path, &sorted(entries), MAGIC, VERSION);
        NgramTable::load(&path).expect("the fixture table loads")
    }

    pub(crate) fn triple_table(
        directory: &Path,
        name: &str,
        triples: &[(&str, &str, &str, f32)],
    ) -> NgramTable {
        let path = directory.join(name);
        let entries = triples
            .iter()
            .map(|(before, previous, next, value)| (hash_triple(before, previous, next), *value))
            .collect();
        write_table(&path, &sorted(entries), MAGIC, VERSION);
        NgramTable::load(&path).expect("the fixture table loads")
    }

    #[test]
    fn hash_matches_the_builder() {
        assert_eq!(fnv1a_words(&["配置", "与"]), hash_pair("配置", "与"));
        assert_eq!(fnv1a_words(&["a", "b", "c"]), hash_triple("a", "b", "c"));
        assert_eq!(fnv1a_words(&[]), 0xCBF29CE484222325);
    }

    #[test]
    fn lookup_round_trips() {
        let directory = tempfile::tempdir().unwrap();
        let table = pair_table(
            directory.path(),
            "lookup.bin",
            &[("配置", "与", 2.5), (SENTENCE_START, "本仓", -1.25)],
        );
        assert_eq!(table.len(), 2);
        assert!(!table.is_empty());
        assert!((table.bigram("配置", "与") - 2.5).abs() < 0.01);
        assert!(table.bigram(SENTENCE_START, "本仓") < -1.24);
        assert_eq!(
            table.bigram("配置", "于"),
            0.0,
            "an absent pair scores zero rather than a penalty"
        );
        assert_eq!(
            table.bigram("与", "配置"),
            0.0,
            "the pair is ordered, not a set"
        );
        assert_eq!(
            table.bigram("配置", ""),
            0.0,
            "an empty second word scores zero"
        );
    }

    /// The generation contract that makes the mapping sound: a new table arrives as `<name>.incoming` and is renamed over the old one, so a table already mapped keeps answering from the file it mapped.
    #[test]
    fn a_mapped_table_survives_a_rename_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let table = pair_table(directory.path(), "bigram.bin", &[("配置", "与", 2.5)]);
        let path = directory.path().join("bigram.bin");
        let incoming = directory.path().join("bigram.bin.incoming");
        write_table(
            &incoming,
            &sorted(vec![(hash_pair("配置", "与"), -2.0)]),
            MAGIC,
            VERSION,
        );
        std::fs::rename(&incoming, &path).unwrap();
        assert!((table.bigram("配置", "与") - 2.5).abs() < 0.01);
        let replaced = NgramTable::load(&path).expect("the new table loads");
        assert!((replaced.bigram("配置", "与") + 2.0).abs() < 0.01);
    }

    #[test]
    fn trigram_lookup_uses_three_words() {
        let directory = tempfile::tempdir().unwrap();
        let table = triple_table(
            directory.path(),
            "triple.bin",
            &[(SENTENCE_START, "输入", "法", 4.0)],
        );
        assert_eq!(table.trigram(SENTENCE_START, "输入", "法"), 4.0);
        assert_eq!(table.trigram("输入", SENTENCE_START, "法"), 0.0);
        assert_eq!(table.bigram("输入", "法"), 0.0);
        assert_eq!(table.trigram(SENTENCE_START, "输入", ""), 0.0);
    }

    #[test]
    fn rejects_bad_files() {
        let directory = tempfile::tempdir().unwrap();
        let dir = directory.path();
        assert!(
            NgramTable::load(&dir.join("absent.bin")).is_none(),
            "a missing file yields no table"
        );

        let wrong_magic = dir.join("magic.bin");
        write_table(&wrong_magic, &[(1, 1.0)], b"XXXX", 1);
        assert!(
            NgramTable::load(&wrong_magic).is_none(),
            "a foreign file is rejected"
        );

        let wrong_version = dir.join("version.bin");
        write_table(&wrong_version, &[(1, 1.0)], MAGIC, 99);
        assert!(
            NgramTable::load(&wrong_version).is_none(),
            "a future version is rejected"
        );

        let unsorted = dir.join("unsorted.bin");
        write_table(&unsorted, &[(9, 1.0), (2, 2.0)], MAGIC, 1);
        assert!(
            NgramTable::load(&unsorted).is_none(),
            "unsorted keys are rejected"
        );

        let duplicate = dir.join("duplicate.bin");
        write_table(&duplicate, &[(2, 1.0), (2, 2.0)], MAGIC, 1);
        assert!(
            NgramTable::load(&duplicate).is_some(),
            "non-decreasing keys pass std::is_sorted"
        );

        let truncated = dir.join("truncated.bin");
        write_table(&truncated, &[(1, 1.0), (2, 2.0)], MAGIC, 1);
        let bytes = std::fs::read(&truncated).unwrap();
        std::fs::write(&truncated, &bytes[..bytes.len() - 6]).unwrap();
        assert!(
            NgramTable::load(&truncated).is_none(),
            "a truncated file is rejected"
        );

        let short = dir.join("short.bin");
        std::fs::write(&short, b"MSNG\x01\0\0").unwrap();
        assert!(
            NgramTable::load(&short).is_none(),
            "a file shorter than the header is rejected"
        );

        let oversized = dir.join("oversized.bin");
        let mut header = Vec::new();
        header.extend_from_slice(MAGIC);
        header.extend_from_slice(&1u32.to_le_bytes());
        header.extend_from_slice(&(MAX_ENTRIES as u32 + 1).to_le_bytes());
        header.extend_from_slice(&0u32.to_le_bytes());
        std::fs::write(&oversized, header).unwrap();
        assert!(
            NgramTable::load(&oversized).is_none(),
            "a count past the cap is rejected"
        );

        let trailing = dir.join("trailing.bin");
        write_table(&trailing, &[(1, 1.0)], MAGIC, 1);
        let mut bytes = std::fs::read(&trailing).unwrap();
        bytes.extend_from_slice(&[0; 5]);
        std::fs::write(&trailing, bytes).unwrap();
        assert!(
            NgramTable::load(&trailing).is_some(),
            "only needed <= size is checked"
        );

        let empty = dir.join("empty.bin");
        write_table(&empty, &[], MAGIC, 1);
        let table = NgramTable::load(&empty).expect("an empty but valid table loads");
        assert!(table.is_empty());
        assert_eq!(
            table.bigram("配置", "与"),
            0.0,
            "an empty table scores everything zero"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_valid_table() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let external = directory.path().join("external.bin");
        write_table(&external, &[(1, 1.0)], MAGIC, VERSION);
        let linked = directory.path().join("linked.bin");
        symlink(&external, &linked).unwrap();

        assert!(NgramTable::load(&external).is_some());
        assert!(NgramTable::load(&linked).is_none());
    }

    #[test]
    fn shared_remembers_the_table_and_a_missing_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("shared.bin");
        assert!(NgramTable::shared(&path).is_none());
        write_table(&path, &[(1, 1.0)], MAGIC, 1);
        assert!(
            NgramTable::shared(&path).is_none(),
            "a remembered miss is not re-probed"
        );

        let present = directory.path().join("present.bin");
        write_table(&present, &[(1, 1.0)], MAGIC, 1);
        let first = NgramTable::shared(&present).expect("loads");
        let second = NgramTable::shared(&present).expect("cached");
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn reads_the_shipped_tables() {
        let Some(resources) = std::env::var_os("MSIME_EVAL_RESOURCES") else {
            eprintln!("skipping reads_the_shipped_tables: MSIME_EVAL_RESOURCES is not set");
            return;
        };
        let resources = PathBuf::from(resources);
        for name in [crate::assets::BIGRAM_TABLE, crate::assets::TRIGRAM_TABLE] {
            let table = NgramTable::load(&resources.join(name))
                .unwrap_or_else(|| panic!("{name} in MSIME_EVAL_RESOURCES loads"));
            assert_eq!(table.len(), 1_000_000, "{name}");
            assert_eq!(table.bigram(SENTENCE_START, ""), 0.0);
            // data-formats.md §7 measured every value in [-3, 3].
            assert!((0..table.len()).all(|index| table.value_at(index).abs() <= 3.0));
        }
    }
}
