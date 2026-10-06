//! The MSJPDT1 lemma dictionary (schemes-lang.md §5.7-§5.8, data-formats.md §8): tokens sorted by reading, a connection matrix and a string blob, all little-endian. Loaded once per path for the process.
//!
//! The file is mapped read-only, as japanese_sentence_decoder.cpp:101-125 did, so its 66 MB are clean, file-backed pages the system can evict under memory pressure (the iOS keyboard extension's limit) rather than dirty heap read on the first Japanese query. The mapping rests on the resource contract: `msime-japanese.dat` ships read-only in the resource bundle and a replacement arrives by rename, never by an in-place write, so a mapped inode keeps its bytes for as long as the dictionary lives (`replacing_a_model_file_never_alters_a_loaded_dictionary`). Access is by offset with unaligned little-endian loads, as the C++ `memcpy` did.

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, Weak};

use memmap2::Mmap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JapaneseLemma {
    pub reading: String,
    pub surface: String,
    pub left_id: u16,
    pub right_id: u16,
    pub word_cost: i32,
    pub token_id: u32,
}

const MAGIC: &[u8; 8] = b"MSJPDT1\0";
const HEADER_SIZE: usize = 56;
const TOKEN_SIZE: usize = 20;
const MAX_TOKEN_COUNT: u32 = 2_000_000;
const MAX_STRING_SIZE: u64 = 1 << 32;
const MAX_CONNECTION_COUNT: u64 = 20_000_000;
/// Out-of-range ids cost this much, so a corrupt id can never look like a cheap transition.
const INVALID_CONNECTION_COST: i32 = 10_000;
/// Pending romaji such as `k` expands into several one-kana prefix queries, by far the widest ranges; the best this many of each first-kana group are kept at load.
const SHORT_PREFIX_CANDIDATE_COUNT: usize = 64;

/// One 20-byte packed token record (`<IHIHHHi`).
#[derive(Debug, Clone, Copy)]
struct Token {
    reading_offset: u32,
    reading_length: u16,
    surface_offset: u32,
    surface_length: u16,
    left_id: u16,
    right_id: u16,
    word_cost: i32,
}

pub struct JapaneseDictionary {
    bytes: Mmap,
    token_offset: usize,
    token_count: usize,
    connection_offset: usize,
    connection_size: usize,
    string_offset: usize,
    /// First code point of a reading -> the best token ids of that group, sorted by (cost, id).
    short_prefix_index: HashMap<String, Vec<u32>>,
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("four bytes"))
}

fn i32_at(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("four bytes"))
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("eight bytes"))
}

fn collect_query_ids<I>(ids: I, capacity: usize) -> Vec<u32>
where
    I: IntoIterator<Item = u32>,
{
    let mut collected = Vec::with_capacity(capacity);
    collected.extend(ids);
    collected
}

/// Keeps the `limit` cheapest ids by (cost, id), cheapest first: the C++ bounded max-heap, whose result is the same set in the same order because (cost, id) is a total order.
fn best_ids(mut ids: Vec<u32>, limit: usize, cost: impl Fn(u32) -> i32) -> Vec<u32> {
    let key = |id: &u32| (cost(*id), *id);
    if limit > 0 && ids.len() > limit {
        ids.select_nth_unstable_by_key(limit - 1, key);
    }
    ids.truncate(limit);
    ids.sort_unstable_by_key(key);
    ids
}

impl JapaneseDictionary {
    /// `None` when the file is missing or fails any header, bounds or ordering check.
    #[allow(unsafe_code)]
    pub fn load(path: &Path) -> Option<JapaneseDictionary> {
        if !std::fs::symlink_metadata(path).ok()?.file_type().is_file() {
            return None;
        }
        let file = File::open(path).ok()?;
        let metadata = file.metadata().ok()?;
        // A directory or a file too short for the header is refused before anything is mapped.
        if !metadata.is_file() || metadata.len() < HEADER_SIZE as u64 {
            return None;
        }
        // SAFETY: a mapping is only sound while nothing changes the file underneath it. The model ships read-only with the resources and is replaced by rename, never written in place (module doc), so the mapped inode keeps its bytes for as long as the map lives.
        let bytes = unsafe { Mmap::map(&file) }.ok()?;
        Self::parse(bytes)
    }

    fn parse(bytes: Mmap) -> Option<JapaneseDictionary> {
        if bytes.len() < HEADER_SIZE || &bytes[..8] != MAGIC {
            return None;
        }
        let version = u32_at(&bytes, 8);
        let token_count = u32_at(&bytes, 12);
        let connection_size = u32_at(&bytes, 16);
        let token_offset = u64_at(&bytes, 24);
        let connection_offset = u64_at(&bytes, 32);
        let string_offset = u64_at(&bytes, 40);
        let string_size = u64_at(&bytes, 48);
        if version != 1
            || connection_size == 0
            || token_count > MAX_TOKEN_COUNT
            || string_size > MAX_STRING_SIZE
        {
            return None;
        }
        let size = bytes.len() as u64;
        let contains = |offset: u64, length: u64| {
            offset >= HEADER_SIZE as u64 && offset <= size && length <= size - offset
        };
        let connection_count = u64::from(connection_size) * u64::from(connection_size);
        if connection_count > MAX_CONNECTION_COUNT
            || !contains(token_offset, u64::from(token_count) * TOKEN_SIZE as u64)
            || !contains(connection_offset, connection_count * 2)
            || !contains(string_offset, string_size)
        {
            return None;
        }
        let mut dictionary = JapaneseDictionary {
            bytes,
            token_offset: token_offset as usize,
            token_count: token_count as usize,
            connection_offset: connection_offset as usize,
            connection_size: connection_size as usize,
            string_offset: string_offset as usize,
            short_prefix_index: HashMap::new(),
        };
        // Lemma text is handed out as `&str`; the builder only writes UTF-8, so a blob that is not is corrupt.
        let strings = dictionary
            .bytes
            .get(dictionary.string_offset..dictionary.string_offset + string_size as usize)?;
        let strings = std::str::from_utf8(strings).ok()?;
        for index in 0..dictionary.token_count {
            let token = dictionary.token_at(index);
            let reading_end = u64::from(token.reading_offset) + u64::from(token.reading_length);
            let surface_end = u64::from(token.surface_offset) + u64::from(token.surface_length);
            if reading_end > string_size
                || surface_end > string_size
                || usize::from(token.left_id) >= dictionary.connection_size
                || usize::from(token.right_id) >= dictionary.connection_size
            {
                return None;
            }
            for (start, end) in [
                (token.reading_offset as usize, reading_end as usize),
                (token.surface_offset as usize, surface_end as usize),
            ] {
                if !strings.is_char_boundary(start) || !strings.is_char_boundary(end) {
                    return None;
                }
            }
            // The file order is the search index; unsorted input would make every lookup silently wrong.
            if index > 0
                && dictionary.reading(&token) < dictionary.reading(&dictionary.token_at(index - 1))
            {
                return None;
            }
        }
        if dictionary.token_count == 0 {
            return None;
        }
        dictionary.short_prefix_index = dictionary.build_short_prefix_index()?;
        Some(dictionary)
    }

    /// Readings are sorted, so each first-code-point group is contiguous and one linear pass indexes them all. `None` for an empty reading.
    fn build_short_prefix_index(&self) -> Option<HashMap<String, Vec<u32>>> {
        let mut index = HashMap::new();
        let mut group_start = 0;
        while group_start < self.token_count {
            let first = self.reading(&self.token_at(group_start)).chars().next()?;
            let mut group_end = group_start;
            while group_end < self.token_count
                && self.reading(&self.token_at(group_end)).starts_with(first)
            {
                group_end += 1;
            }
            let ids = collect_query_ids(
                group_start as u32..group_end as u32,
                group_end - group_start,
            );
            let best = best_ids(ids, SHORT_PREFIX_CANDIDATE_COUNT, |id| self.cost_of(id));
            index.entry(first.to_string()).or_insert(best);
            group_start = group_end;
        }
        Some(index)
    }

    /// One dictionary per path for the process; only a loaded one is kept.
    pub fn shared(path: &Path) -> Option<Arc<JapaneseDictionary>> {
        static MODELS: LazyLock<Mutex<HashMap<PathBuf, Weak<JapaneseDictionary>>>> =
            LazyLock::new(|| Mutex::new(HashMap::new()));
        let mut models = MODELS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        models.retain(|_, model| model.strong_count() > 0);
        if let Some(existing) = models.get(path).and_then(Weak::upgrade) {
            return Some(existing);
        }
        let model = Arc::new(Self::load(path)?);
        models.insert(path.to_path_buf(), Arc::downgrade(&model));
        Some(model)
    }

    /// Tokens whose reading equals `reading`, the `limit` cheapest by (cost, id).
    pub fn exact_lemmas(&self, reading: &str, limit: usize) -> Vec<JapaneseLemma> {
        if reading.is_empty() || limit == 0 {
            return Vec::new();
        }
        let start = self.lower_bound(reading);
        let matches = collect_query_ids(
            (start..self.token_count)
                .take_while(|&index| self.reading(&self.token_at(index)) == reading)
                .map(|index| index as u32),
            limit.min(self.token_count.saturating_sub(start)),
        );
        self.best_lemmas(matches, limit)
    }

    /// Tokens whose reading starts with `prefix`, the `limit` cheapest; single-code-point prefixes come from a precomputed index.
    pub fn prefix_lemmas(&self, prefix: &str, limit: usize) -> Vec<JapaneseLemma> {
        if prefix.is_empty() || limit == 0 {
            return Vec::new();
        }
        if let Some(cached) = self.short_prefix_index.get(prefix) {
            if limit <= SHORT_PREFIX_CANDIDATE_COUNT {
                return cached
                    .iter()
                    .take(limit)
                    .map(|&id| self.lemma(id))
                    .collect();
            }
        }
        let start = self.lower_bound(prefix);
        let matches = collect_query_ids(
            (start..self.token_count)
                .take_while(|&index| self.reading(&self.token_at(index)).starts_with(prefix))
                .map(|index| index as u32),
            limit.min(self.token_count.saturating_sub(start)),
        );
        self.best_lemmas(matches, limit)
    }

    /// Token ids whose reading starts with `prefix` followed by one of `next_kana`.
    /// Each suffix is a contiguous sorted range, so querying those ranges avoids
    /// scanning unrelated readings in the whole `prefix` group. Overlapping
    /// suffixes are deduplicated before ranking.
    fn continuing_candidate_ids(&self, prefix: &str, next_kana: &[&str]) -> Vec<u32> {
        let mut matches = Vec::new();
        for kana in next_kana {
            if kana.is_empty() {
                for index in self.lower_bound(prefix)..self.token_count {
                    let reading = self.reading(&self.token_at(index));
                    let Some(remaining) = reading.strip_prefix(prefix) else {
                        break;
                    };
                    if !remaining.is_empty() {
                        matches.push(index as u32);
                    }
                }
                continue;
            }
            let mut query = String::with_capacity(prefix.len() + kana.len());
            query.push_str(prefix);
            query.push_str(kana);
            let start = self.lower_bound(&query);
            for index in start..self.token_count {
                if !self.reading(&self.token_at(index)).starts_with(&query) {
                    break;
                }
                matches.push(index as u32);
            }
        }
        matches.sort_unstable();
        matches.dedup();
        matches
    }

    /// Tokens strictly longer than `prefix` whose remainder starts with one of `next_kana`.
    ///
    /// The reference ended its scan at the first reading that was not longer than the prefix, and a reading equal to the prefix sorts first, so any prefix that was itself a word returned nothing. Equal readings are skipped instead, as the contract says.
    pub fn prefix_lemmas_continuing(
        &self,
        prefix: &str,
        next_kana: &[&str],
        limit: usize,
    ) -> Vec<JapaneseLemma> {
        if prefix.is_empty() || next_kana.is_empty() || limit == 0 {
            return Vec::new();
        }
        let matches = self.continuing_candidate_ids(prefix, next_kana);
        self.best_lemmas(matches, limit)
    }

    /// 10000 for an out-of-range id.
    pub fn connection_cost(&self, right_id: u16, left_id: u16) -> i32 {
        let (right, left) = (usize::from(right_id), usize::from(left_id));
        if right >= self.connection_size || left >= self.connection_size {
            return INVALID_CONNECTION_COST;
        }
        let index = right * self.connection_size + left;
        i32::from(u16_at(&self.bytes, self.connection_offset + index * 2) as i16)
    }

    fn token_at(&self, index: usize) -> Token {
        let offset = self.token_offset + index * TOKEN_SIZE;
        let bytes = &self.bytes;
        Token {
            reading_offset: u32_at(bytes, offset),
            reading_length: u16_at(bytes, offset + 4),
            surface_offset: u32_at(bytes, offset + 6),
            surface_length: u16_at(bytes, offset + 10),
            left_id: u16_at(bytes, offset + 12),
            right_id: u16_at(bytes, offset + 14),
            word_cost: i32_at(bytes, offset + 16),
        }
    }

    /// Load checked that every token range lies inside the blob on character boundaries of valid UTF-8, so the conversion cannot fail.
    fn text(&self, offset: u32, length: u16) -> &str {
        let start = self.string_offset + offset as usize;
        std::str::from_utf8(&self.bytes[start..start + usize::from(length)])
            .expect("token strings are validated at load")
    }

    fn reading(&self, token: &Token) -> &str {
        self.text(token.reading_offset, token.reading_length)
    }

    fn cost_of(&self, id: u32) -> i32 {
        self.token_at(id as usize).word_cost
    }

    /// First token whose reading is not less than `reading`, bytewise as the C++ `string_view` compare.
    fn lower_bound(&self, reading: &str) -> usize {
        let (mut first, mut last) = (0, self.token_count);
        while first < last {
            let middle = first + (last - first) / 2;
            if self.reading(&self.token_at(middle)) < reading {
                first = middle + 1;
            } else {
                last = middle;
            }
        }
        first
    }

    fn best_lemmas(&self, ids: Vec<u32>, limit: usize) -> Vec<JapaneseLemma> {
        best_ids(ids, limit, |id| self.cost_of(id))
            .into_iter()
            .map(|id| self.lemma(id))
            .collect()
    }

    fn lemma(&self, id: u32) -> JapaneseLemma {
        let token = self.token_at(id as usize);
        JapaneseLemma {
            reading: self.reading(&token).to_owned(),
            surface: self
                .text(token.surface_offset, token.surface_length)
                .to_owned(),
            left_id: token.left_id,
            right_id: token.right_id,
            word_cost: token.word_cost,
            token_id: id,
        }
    }
}

/// Builds MSJPDT1 files for the tests of this module and its callers, the way `build_sentence_model.py` lays them out.
#[cfg(test)]
pub(crate) mod test_model {
    /// `(reading, surface, left_id, right_id, cost)`; the caller keeps readings in byte order, as the loader requires.
    pub type Entry<'a> = (&'a str, &'a str, u16, u16, i32);

    pub fn bytes(entries: &[Entry<'_>], connection_size: u32, connection: &[i16]) -> Vec<u8> {
        let mut strings = Vec::<u8>::new();
        let mut interned = std::collections::HashMap::<String, u32>::new();
        let mut intern = |text: &str, strings: &mut Vec<u8>| -> (u32, u16) {
            let offset = *interned.entry(text.to_owned()).or_insert_with(|| {
                let offset = strings.len() as u32;
                strings.extend_from_slice(text.as_bytes());
                offset
            });
            (offset, text.len() as u16)
        };
        let mut tokens = Vec::new();
        for &(reading, surface, left, right, cost) in entries {
            let reading = intern(reading, &mut strings);
            let surface = intern(surface, &mut strings);
            tokens.extend_from_slice(&reading.0.to_le_bytes());
            tokens.extend_from_slice(&reading.1.to_le_bytes());
            tokens.extend_from_slice(&surface.0.to_le_bytes());
            tokens.extend_from_slice(&surface.1.to_le_bytes());
            tokens.extend_from_slice(&left.to_le_bytes());
            tokens.extend_from_slice(&right.to_le_bytes());
            tokens.extend_from_slice(&cost.to_le_bytes());
        }
        let token_offset = 56u64;
        let connection_offset = token_offset + tokens.len() as u64;
        let string_offset = connection_offset + connection.len() as u64 * 2;
        let mut file = Vec::new();
        file.extend_from_slice(b"MSJPDT1\0");
        file.extend_from_slice(&1u32.to_le_bytes());
        file.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        file.extend_from_slice(&connection_size.to_le_bytes());
        file.extend_from_slice(&0u32.to_le_bytes());
        for value in [
            token_offset,
            connection_offset,
            string_offset,
            strings.len() as u64,
        ] {
            file.extend_from_slice(&value.to_le_bytes());
        }
        file.extend_from_slice(&tokens);
        for cost in connection {
            file.extend_from_slice(&cost.to_le_bytes());
        }
        file.extend_from_slice(&strings);
        file
    }

    /// `test_engine_smoke.cpp:159-198`: two lemmas and a one-id matrix, enough to drive the provider's prefix-lemma branch.
    pub fn smoke() -> Vec<u8> {
        bytes(
            &[("かんじ", "漢字", 0, 0, 1000), ("しし", "四肢", 0, 0, 1200)],
            1,
            &[0],
        )
    }

    /// `test_runtime_isolation.cpp` `make_japanese_model`: one かな lemma with the given surface.
    pub fn single(surface: &str) -> Vec<u8> {
        bytes(&[("かな", surface, 0, 0, 100)], 1, &[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same read-only mapping type `load` produces, over anonymous memory, so the checks run on in-memory bytes.
    fn mapped(bytes: &[u8]) -> Mmap {
        let mut map = memmap2::MmapMut::map_anon(bytes.len()).expect("anonymous map");
        map.copy_from_slice(bytes);
        map.make_read_only().expect("read-only map")
    }

    fn parse(bytes: impl AsRef<[u8]>) -> Option<JapaneseDictionary> {
        JapaneseDictionary::parse(mapped(bytes.as_ref()))
    }

    fn parsed(bytes: Vec<u8>) -> JapaneseDictionary {
        parse(bytes).expect("valid model")
    }

    fn surfaces(lemmas: &[JapaneseLemma]) -> Vec<&str> {
        lemmas.iter().map(|lemma| lemma.surface.as_str()).collect()
    }

    #[test]
    fn scanned_query_ids_reserve_the_lookup_limit() {
        let ids = collect_query_ids([1_u32, 2, 3], 8);
        assert_eq!(ids, [1, 2, 3]);
        assert!(ids.capacity() >= 8);
    }

    // test_runtime_isolation.cpp:105-139.
    #[test]
    fn replacing_a_model_file_never_alters_a_loaded_dictionary() {
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("mapped-japanese.dat");
        std::fs::write(&path, test_model::single("甲")).expect("write model");
        let original = JapaneseDictionary::load(&path).expect("original loads");
        assert_eq!(original.exact_lemmas("かな", 8)[0].surface, "甲");

        let replacement = root.path().join("replacement-japanese.dat");
        std::fs::write(&replacement, test_model::single("乙")).expect("write replacement");
        std::fs::remove_file(&path).expect("remove");
        std::fs::rename(&replacement, &path).expect("rename");
        let updated = JapaneseDictionary::load(&path).expect("updated loads");
        assert_eq!(updated.exact_lemmas("かな", 8)[0].surface, "乙");
        assert_eq!(original.exact_lemmas("かな", 8)[0].surface, "甲");
    }

    #[test]
    fn truncated_or_corrupt_models_are_refused() {
        let valid = test_model::single("甲");
        for length in [0, 55, 77, valid.len() - 1] {
            assert!(parse(&valid[..length]).is_none(), "{length}");
        }
        let mut invalid_offset = valid.clone();
        invalid_offset[24..32].fill(0xff);
        assert!(parse(invalid_offset).is_none());
        let mut invalid_reading = valid.clone();
        invalid_reading[60] = 0xff;
        invalid_reading[61] = 0xff;
        assert!(parse(invalid_reading).is_none());

        let root = tempfile::tempdir().expect("temporary directory");
        assert!(JapaneseDictionary::load(&root.path().join("absent.dat")).is_none());
        assert!(JapaneseDictionary::load(root.path()).is_none());
        // Files shorter than the header, including an empty one, are refused before mapping.
        for length in [0, HEADER_SIZE - 1] {
            let short = root.path().join(format!("short-{length}.dat"));
            std::fs::write(&short, &valid[..length]).expect("write short model");
            assert!(JapaneseDictionary::load(&short).is_none(), "{length}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_valid_model() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let external = root.path().join("external.dat");
        std::fs::write(&external, test_model::single("甲")).unwrap();
        let linked = root.path().join("linked.dat");
        symlink(&external, &linked).unwrap();

        assert!(JapaneseDictionary::load(&external).is_some());
        assert!(JapaneseDictionary::load(&linked).is_none());
    }

    /// `load` maps the file (the `bytes` field is a read-only `Mmap`, not an owned buffer) and every lookup, the matrix search included, reads through that mapping; unlinking the file does not disturb a loaded dictionary.
    #[test]
    fn a_mapped_model_answers_lookups_and_sentence_search() {
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("msime-japanese.dat");
        let file = test_model::bytes(
            &[("かな", "仮名", 0, 0, 900), ("し", "詩", 0, 0, 400)],
            1,
            &[10],
        );
        std::fs::write(&path, &file).expect("write model");
        let dictionary = JapaneseDictionary::load(&path).expect("model loads");
        let _: &Mmap = &dictionary.bytes;
        assert_eq!(dictionary.bytes.len(), file.len());
        assert_eq!(&dictionary.bytes[..], &file[..]);
        std::fs::remove_file(&path).expect("remove");

        assert_eq!(surfaces(&dictionary.exact_lemmas("かな", 8)), vec!["仮名"]);
        assert_eq!(surfaces(&dictionary.prefix_lemmas("か", 8)), vec!["仮名"]);
        assert_eq!(dictionary.connection_cost(0, 0), 10);
        let sentence = super::super::matrix::search_converted(
            &dictionary,
            &super::super::romaji::convert_romaji("kanasi"),
            4,
        );
        // Sentence start, 仮名, 詩 and sentence end are each joined by the one 10-cost transition: 900 + 400 + 3 * 10.
        assert_eq!(sentence[0].text, "仮名詩");
        assert_eq!(sentence[0].cost, 1_330);
    }

    #[test]
    fn header_and_ordering_checks() {
        let mut bad_magic = test_model::single("甲");
        bad_magic[0] = b'X';
        assert!(parse(bad_magic).is_none());
        let mut bad_version = test_model::single("甲");
        bad_version[8] = 2;
        assert!(parse(bad_version).is_none());

        let unsorted = test_model::bytes(&[("し", "市", 0, 0, 1), ("か", "蚊", 0, 0, 1)], 1, &[0]);
        assert!(parse(unsorted).is_none());
        let empty_reading = test_model::bytes(&[("", "空", 0, 0, 1)], 1, &[0]);
        assert!(parse(empty_reading).is_none());
        let bad_id = test_model::bytes(&[("か", "蚊", 1, 0, 1)], 1, &[0]);
        assert!(parse(bad_id).is_none());
        let no_tokens = test_model::bytes(&[], 1, &[0]);
        assert!(parse(no_tokens).is_none());
        // A string range that ends inside a character.
        let mut split = test_model::single("甲");
        split[60] = 5;
        assert!(parse(split).is_none());
    }

    #[test]
    fn lemma_lookups_order_by_cost_then_id() {
        let dictionary = parsed(test_model::bytes(
            &[
                ("か", "蚊", 0, 1, 300),
                ("か", "化", 1, 0, 100),
                ("か", "可", 0, 0, 100),
                ("かな", "仮名", 0, 0, 50),
                ("かんじ", "漢字", 1, 1, 10),
                ("き", "木", 0, 0, 1),
            ],
            2,
            &[5, -7, 300, 40],
        ));
        let exact = dictionary.exact_lemmas("か", 8);
        assert_eq!(surfaces(&exact), vec!["化", "可", "蚊"]);
        assert_eq!(exact[0].token_id, 1);
        assert_eq!(
            (exact[0].left_id, exact[0].right_id, exact[0].word_cost),
            (1, 0, 100)
        );
        assert_eq!(exact[0].reading, "か");
        assert_eq!(
            surfaces(&dictionary.exact_lemmas("か", 2)),
            vec!["化", "可"]
        );
        assert!(dictionary.exact_lemmas("く", 8).is_empty());
        assert!(dictionary.exact_lemmas("か", 0).is_empty());

        // One code point comes from the index, longer prefixes from a scan, with the same order.
        assert_eq!(
            surfaces(&dictionary.prefix_lemmas("か", 3)),
            vec!["漢字", "仮名", "化"]
        );
        assert_eq!(
            surfaces(&dictionary.prefix_lemmas("か", 100)),
            vec!["漢字", "仮名", "化", "可", "蚊"]
        );
        assert_eq!(surfaces(&dictionary.prefix_lemmas("かん", 8)), vec!["漢字"]);
        assert!(dictionary.prefix_lemmas("", 8).is_empty());

        assert_eq!(
            surfaces(&dictionary.prefix_lemmas_continuing("か", &["な", "ん"], 8)),
            vec!["漢字", "仮名"]
        );
        assert_eq!(
            surfaces(&dictionary.prefix_lemmas_continuing("か", &["ん"], 8)),
            vec!["漢字"]
        );
        assert!(dictionary
            .prefix_lemmas_continuing("かな", &["な"], 8)
            .is_empty());
        assert!(dictionary.prefix_lemmas_continuing("か", &[], 8).is_empty());

        assert_eq!(dictionary.connection_cost(0, 0), 5);
        assert_eq!(dictionary.connection_cost(0, 1), -7);
        assert_eq!(dictionary.connection_cost(1, 0), 300);
        assert_eq!(dictionary.connection_cost(1, 1), 40);
        assert_eq!(dictionary.connection_cost(2, 0), 10_000);
        assert_eq!(dictionary.connection_cost(0, 2), 10_000);
    }

    #[test]
    fn continuing_candidate_ranges_deduplicate_overlapping_kana() {
        let dictionary = parsed(test_model::bytes(
            &[
                ("かな", "仮名", 0, 0, 10),
                ("かない", "家内", 0, 0, 20),
                ("かに", "蟹", 0, 0, 40),
                ("かん", "漢", 0, 0, 30),
            ],
            1,
            &[0],
        ));

        let ids = dictionary.continuing_candidate_ids("か", &["な", "ない", "ん"]);
        assert_eq!(ids, vec![0, 1, 3]);
    }

    #[test]
    fn short_prefix_index_keeps_the_best_sixty_four() {
        let surfaces_owned: Vec<String> = (0..70).map(|index| format!("語{index:02}")).collect();
        let entries: Vec<test_model::Entry<'_>> = surfaces_owned
            .iter()
            .enumerate()
            .map(|(index, surface)| ("か", surface.as_str(), 0, 0, 1_000 - index as i32))
            .collect();
        let dictionary = parsed(test_model::bytes(&entries, 1, &[0]));
        let cached = dictionary.prefix_lemmas("か", 64);
        assert_eq!(cached.len(), 64);
        assert_eq!(cached[0].surface, "語69");
        assert_eq!(cached[63].surface, "語06");
        // Past the cached count the full range is scanned.
        assert_eq!(dictionary.prefix_lemmas("か", 65).len(), 65);
    }

    #[test]
    fn shared_returns_one_dictionary_per_path_while_it_lives() {
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("msime-japanese.dat");
        std::fs::write(&path, test_model::single("甲")).expect("write model");
        let first = JapaneseDictionary::shared(&path).expect("loads");
        let second = JapaneseDictionary::shared(&path).expect("shared");
        assert!(Arc::ptr_eq(&first, &second));
        assert!(JapaneseDictionary::shared(&root.path().join("absent.dat")).is_none());
    }

    /// The shipped `dict-v2.0.1` model (data-formats.md §8): header values and a few lookups.
    #[test]
    fn real_model_loads() {
        let Some(resources) = std::env::var_os("MSIME_EVAL_RESOURCES") else {
            eprintln!(
                "skipped: MSIME_EVAL_RESOURCES is not set to the dict-v2.0.1 resource directory"
            );
            return;
        };
        let path = Path::new(&resources).join(crate::assets::JAPANESE_MODEL);
        let dictionary = JapaneseDictionary::load(&path).expect("the shipped model loads");
        assert_eq!(dictionary.token_count, 1_284_987);
        assert_eq!(dictionary.connection_size, 2_672);
        assert!(surfaces(&dictionary.exact_lemmas("かんじ", 16)).contains(&"漢字"));
        assert!(!dictionary.prefix_lemmas("か", 24).is_empty());
    }
}
