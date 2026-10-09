//! The Jyutping composition: typed letters and `'` boundaries, read as syllables against the inventory, and the candidates `msime-cantonese.db` has for them. A candidate may cover only the leading syllables; selecting it takes those letters out of the composition and leaves the rest composing, with nothing held back as phrase progress.

use std::collections::HashSet;
use std::sync::Arc;

use super::syllable::{self, Inventory, Segmentation};
use crate::error::Result;
use crate::language_dictionary::{LanguageDictionary, LanguageEntry};
use crate::types::{QueryRequest, SchemeKey, SchemeType};

/// Entries read for one span of complete syllables.
pub const SPAN_LIMIT: usize = 200;
/// Entries read for a reading whose last syllable is still a prefix.
pub const COMPLETION_LIMIT: usize = 50;
const SMALL_CANDIDATE_DEDUP: usize = 64;

/// One candidate row, with what selecting it consumes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CantoneseCandidate {
    pub text: String,
    pub weight: i64,
    /// The `entries.key` the row is stored under; for a completed prefix it spells out the last syllable in full.
    pub key: String,
    /// How many syllables of the reading it covers, counted from the first.
    pub syllables: usize,
    /// The byte length of the typed input it covers, apostrophes inside it included.
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct CantoneseScheme {
    /// Lowercase letters and single `'` boundaries, never starting with one.
    input: String,
    inventory: Arc<Inventory>,
}

/// 会话查询复用的字典行和跨度键；不属于方案的组字状态。
#[derive(Default)]
pub(crate) struct CantoneseQueryBuffer {
    entries: Vec<LanguageEntry>,
    completions: Vec<(String, LanguageEntry)>,
    key: String,
}

impl CantoneseScheme {
    pub fn new(inventory: Arc<Inventory>) -> Self {
        Self {
            input: String::new(),
            inventory,
        }
    }

    pub fn reset(&mut self) {
        self.input.clear();
    }

    /// Lowercase letters append and Backspace removes the last key. `'` appends only between letters: a boundary before the first syllable or right after another boundary marks nothing. Uppercase letters are not part of Jyutping and are ignored, like every other key.
    pub fn handle_key(&mut self, key: SchemeKey) -> bool {
        match key {
            SchemeKey::Letter(letter) if letter.is_ascii_lowercase() => {
                self.input.push(char::from(letter));
                true
            }
            SchemeKey::Apostrophe => {
                if !self.input.is_empty() && !self.input.ends_with('\'') {
                    self.input.push('\'');
                    true
                } else {
                    false
                }
            }
            SchemeKey::Backspace => self.input.pop().is_some(),
            SchemeKey::Letter(_)
            | SchemeKey::Semicolon
            | SchemeKey::Minus
            | SchemeKey::Symbol(_)
            | SchemeKey::Requery => false,
        }
    }

    /// Replaces the composition with a host edit: letters are lowercased, `'` and the spaces `editing_text` shows at syllable boundaries are both boundaries, anything else is dropped, and boundaries are normalized as typing would leave them. Reading a space as a boundary keeps the syllables the user saw when an edit changes the letters around them (`ngo oi` edited to `ngo i` stays two syllables rather than becoming `ngoi`).
    pub fn set_raw_input(&mut self, raw: &str) {
        self.input.clear();
        self.input.reserve(raw.len());
        for character in raw.chars() {
            if character.is_ascii_alphabetic() {
                self.input.push(character.to_ascii_lowercase());
            } else if character == '\'' || character == ' ' {
                self.handle_key(SchemeKey::Apostrophe);
            }
        }
    }

    /// The typed letters and boundaries.
    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn is_empty(&self) -> bool {
        self.input.is_empty()
    }

    /// The reading the composition shows and is looked up by (`syllable::best`).
    pub fn segmentation(&self) -> Segmentation {
        syllable::best(&self.input, &self.inventory)
    }

    /// The typed letters with a space at each syllable boundary (`nei hou`). Letters no syllable reads follow after a space as typed, and a trailing `'` stays visible so the key shows an effect.
    pub fn editing_text(&self) -> String {
        let reading = self.segmentation();
        let mut text = String::new();
        self.editing_text_into(&reading, &mut text);
        text
    }

    fn editing_text_into(&self, reading: &Segmentation, text: &mut String) {
        let prefix_capacity = reading
            .syllables
            .iter()
            .map(|syllable| syllable.end - syllable.start)
            .sum::<usize>()
            .saturating_add(reading.syllables.len().saturating_sub(1));
        let rest = self.input[reading.end()..].trim_start_matches('\'');
        let suffix_capacity = if rest.is_empty() {
            usize::from(self.input.ends_with('\''))
        } else {
            rest.len().saturating_add(usize::from(prefix_capacity > 0))
        };
        text.clear();
        let capacity = prefix_capacity.saturating_add(suffix_capacity);
        if text.capacity() == 0 {
            text.reserve_exact(capacity);
        } else {
            text.reserve(capacity);
        }
        for (index, syllable) in reading.syllables.iter().enumerate() {
            if index > 0 {
                text.push(' ');
            }
            text.push_str(&self.input[syllable.start..syllable.end]);
        }
        if !rest.is_empty() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(rest);
        } else if self.input.ends_with('\'') {
            text.push('\'');
        }
    }

    /// The request the session refreshes with. `raw_input` is the typed letters and boundaries, which the provider reads again through the same inventory; `segmentation` is the dictionary key of the reading and `normalized_segmentation` the text the composition shows.
    pub fn build_request(&self) -> QueryRequest {
        let mut request = QueryRequest::default();
        self.build_request_into(&mut request);
        request
    }

    pub fn build_request_into(&self, request: &mut QueryRequest) {
        let reading = self.segmentation();
        request.scheme = SchemeType::Cantonese;
        request.raw_input.clone_from(&self.input);
        request.raw_input_with_cases.clone_from(&self.input);
        request.normalized_input.clear();
        request.normalized_input.reserve(self.input.len());
        request
            .normalized_input
            .extend(self.input.chars().filter(|&character| character != '\''));
        request.raw_segmentation.clone_from(&self.input);
        self.editing_text_into(&reading, &mut request.normalized_segmentation);
        reading.key_into(
            &self.input,
            reading.syllables.len(),
            &mut request.segmentation,
        );
        request.valid = !self.input.is_empty();
    }

    /// The letters and boundaries as typed, which is what Enter commits; the spaced form the composition shows is `editing_text`.
    pub fn preedit(&self) -> String {
        self.input.clone()
    }

    /// The candidates for the composition, each text listed once per span length:
    /// 1. When the reading covers every letter, the entries of the whole reading, or when its last syllable is still a prefix, the entries that complete it. A last syllable that is complete but also starts a longer one (`ho` of `hou`) lists its exact entries and then the entries that complete it, so a word does not drop out of the list while its last syllable is half typed.
    /// 2. Then each leading span of complete syllables, longest first, each span's entries heaviest first.
    pub fn candidates(&self, dictionary: &LanguageDictionary) -> Result<Vec<CantoneseCandidate>> {
        let mut candidates = Vec::new();
        self.candidates_into(
            dictionary,
            &mut CantoneseQueryBuffer::default(),
            &mut candidates,
        )?;
        candidates.shrink_to_fit();
        Ok(candidates)
    }

    /// 将候选写入已有缓冲，并复用字典行和候选行中的字符串容量。
    pub(crate) fn candidates_into(
        &self,
        dictionary: &LanguageDictionary,
        buffer: &mut CantoneseQueryBuffer,
        candidates: &mut Vec<CantoneseCandidate>,
    ) -> Result<()> {
        let input = self.input.as_str();
        let reading = self.segmentation();
        let count = reading.syllables.len();
        let full = !reading.syllables.is_empty()
            && input[reading.end()..].bytes().all(|byte| byte == b'\'');
        let mut length = 0;
        let mut spans = count;
        if full {
            write_key(&reading, input, count, &mut buffer.key);
            if reading.ends_in_prefix() {
                dictionary.lookup_completions_into(
                    &buffer.key,
                    COMPLETION_LIMIT,
                    &mut buffer.completions,
                )?;
                reserve_candidate_batch(candidates, length, buffer.completions.len());
                for (key, entry) in &buffer.completions {
                    push_candidate(
                        candidates,
                        &mut length,
                        key,
                        &entry.text,
                        entry.weight,
                        count,
                        &reading,
                    );
                }
            } else {
                dictionary.lookup_into(&buffer.key, SPAN_LIMIT, &mut buffer.entries)?;
                reserve_candidate_batch(candidates, length, buffer.entries.len());
                for entry in &buffer.entries {
                    push_candidate(
                        candidates,
                        &mut length,
                        &buffer.key,
                        &entry.text,
                        entry.weight,
                        count,
                        &reading,
                    );
                }
                let last = reading.texts(input).last().unwrap_or_default();
                if self.inventory.is_prefix(last) {
                    dictionary.lookup_completions_into(
                        &buffer.key,
                        COMPLETION_LIMIT,
                        &mut buffer.completions,
                    )?;
                    reserve_candidate_batch(candidates, length, buffer.completions.len());
                    for (key, entry) in &buffer.completions {
                        push_candidate(
                            candidates,
                            &mut length,
                            key,
                            &entry.text,
                            entry.weight,
                            count,
                            &reading,
                        );
                    }
                }
            }
            spans = count - 1;
        }
        for span_length in (1..=spans).rev() {
            write_key(&reading, input, span_length, &mut buffer.key);
            dictionary.lookup_into(&buffer.key, SPAN_LIMIT, &mut buffer.entries)?;
            reserve_candidate_batch(candidates, length, buffer.entries.len());
            for entry in &buffer.entries {
                push_candidate(
                    candidates,
                    &mut length,
                    &buffer.key,
                    &entry.text,
                    entry.weight,
                    span_length,
                    &reading,
                );
            }
        }
        candidates.truncate(length);
        deduplicate_candidates(candidates);
        Ok(())
    }

    /// Takes the letters `candidate` covers out of the composition, with the boundary after them. Returns whether letters are left composing; the caller commits the candidate's text either way.
    pub fn select(&mut self, candidate: &CantoneseCandidate) -> bool {
        self.select_end(candidate.end)
    }

    /// 按候选覆盖的字节数删掉组字前缀，避免提交路径复制整行候选。
    pub fn select_end(&mut self, end: usize) -> bool {
        self.input.drain(..end);
        let boundaries = self.input.len() - self.input.trim_start_matches('\'').len();
        self.input.drain(..boundaries);
        !self.input.is_empty()
    }
}

fn reserve_candidate_batch(candidates: &mut Vec<CantoneseCandidate>, length: usize, batch: usize) {
    let required = length + batch;
    if required > candidates.capacity() {
        candidates.reserve_exact(required - candidates.len());
    }
}

fn push_candidate(
    candidates: &mut Vec<CantoneseCandidate>,
    length: &mut usize,
    key: &str,
    text: &str,
    weight: i64,
    syllables: usize,
    reading: &Segmentation,
) {
    let end = reading.syllables[syllables - 1].end;
    if let Some(candidate) = candidates.get_mut(*length) {
        candidate.text.clear();
        candidate.text.push_str(text);
        candidate.key.clear();
        candidate.key.push_str(key);
        candidate.weight = weight;
        candidate.syllables = syllables;
        candidate.end = end;
    } else {
        candidates.push(CantoneseCandidate {
            text: text.to_owned(),
            weight,
            key: key.to_owned(),
            syllables,
            end,
        });
    }
    *length += 1;
}

fn write_key(reading: &Segmentation, input: &str, count: usize, destination: &mut String) {
    destination.clear();
    for (index, syllable) in reading.syllables.iter().take(count).enumerate() {
        if index > 0 {
            destination.push(' ');
        }
        destination.push_str(&input[syllable.start..syllable.end]);
    }
}

fn deduplicate_candidates(candidates: &mut Vec<CantoneseCandidate>) {
    if candidates.len() <= SMALL_CANDIDATE_DEDUP {
        let mut write = 0;
        for read in 0..candidates.len() {
            if candidates[..write].iter().any(|existing| {
                existing.text == candidates[read].text
                    && existing.syllables == candidates[read].syllables
            }) {
                continue;
            }
            if write != read {
                candidates.swap(write, read);
            }
            write += 1;
        }
        candidates.truncate(write);
        return;
    }
    let mut seen = HashSet::with_capacity(candidates.len());
    let duplicates = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            (!seen.insert((candidate.text.as_str(), candidate.syllables))).then_some(index)
        })
        .collect::<Vec<_>>();
    drop(seen);
    let mut duplicates = duplicates.into_iter().peekable();
    let mut write = 0;
    for read in 0..candidates.len() {
        if duplicates.peek() == Some(&read) {
            duplicates.next();
            continue;
        }
        if write != read {
            candidates.swap(write, read);
        }
        write += 1;
    }
    candidates.truncate(write);
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use rusqlite::Connection;

    use super::*;
    use crate::cantonese::syllable::tests::{inventory, SYLLABLES};
    use crate::language_dictionary::{
        open_read_only, FORMAT_VERSION, METADATA_FORMAT_VERSION, METADATA_LICENSE,
        METADATA_SOURCE_COMMIT, SCHEMA,
    };

    const ENTRIES: [(&str, &str, i64); 17] = [
        ("nei hou", "你好", 900),
        ("nei hou", "妳好", 40),
        ("nei hoeng", "你向", 3),
        ("nei", "你", 5000),
        ("nei", "妳", 300),
        ("hou", "好", 4000),
        ("hou", "號", 500),
        ("gwong dung waa", "廣東話", 800),
        ("gwong dung", "廣東", 1200),
        ("gwong", "光", 2000),
        ("gwong", "廣", 900),
        ("dung", "東", 1500),
        ("waa", "話", 3000),
        ("ngo", "我", 6000),
        ("oi", "愛", 2500),
        ("ngoi", "外", 1000),
        // The same text under a different span is listed again, so 光 stays selectable for `gwong` alone.
        ("gwong dung waa", "光", 1),
    ];

    fn build(path: &Path) {
        let connection = Connection::open(path).unwrap();
        connection.execute_batch(SCHEMA).unwrap();
        for (name, value) in [
            (METADATA_FORMAT_VERSION, FORMAT_VERSION.to_string()),
            (
                METADATA_SOURCE_COMMIT,
                "259f0e48bba840c3a2e0d117539e96937f3d89bc".to_owned(),
            ),
            (METADATA_LICENSE, "CC-BY-4.0".to_owned()),
        ] {
            connection
                .execute("INSERT INTO metadata VALUES (?1, ?2)", (name, value))
                .unwrap();
        }
        for syllable in SYLLABLES {
            connection
                .execute("INSERT INTO syllables VALUES (?1)", (syllable,))
                .unwrap();
        }
        for (key, text, weight) in ENTRIES {
            connection
                .execute(
                    "INSERT INTO entries VALUES (?1, ?2, ?3)",
                    (key, text, weight),
                )
                .unwrap();
        }
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        dictionary: LanguageDictionary,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("msime-cantonese.db");
        build(&path);
        Fixture {
            dictionary: open_read_only(&path).unwrap(),
            _dir: dir,
        }
    }

    fn typed(keys: &str) -> CantoneseScheme {
        let mut scheme = CantoneseScheme::new(Arc::new(inventory()));
        for byte in keys.bytes() {
            scheme.handle_key(if byte == b'\'' {
                SchemeKey::Apostrophe
            } else {
                SchemeKey::Letter(byte)
            });
        }
        scheme
    }

    /// Each candidate as `text/syllables`.
    fn listed(scheme: &CantoneseScheme, dictionary: &LanguageDictionary) -> Vec<String> {
        scheme
            .candidates(dictionary)
            .unwrap()
            .iter()
            .map(|candidate| format!("{}/{}", candidate.text, candidate.syllables))
            .collect()
    }

    fn candidate(text: &str, syllables: usize) -> CantoneseCandidate {
        CantoneseCandidate {
            text: text.to_owned(),
            weight: 1,
            key: "nei".to_owned(),
            syllables,
            end: 3,
        }
    }

    #[test]
    fn editing_text_growing_buffer_allocates_once() {
        for suffix in ["", "zz", "'"] {
            let input = format!("{}{suffix}", "nei".repeat(20));
            let scheme = typed(&input);
            let reading = scheme.segmentation();
            let expected_suffix = if suffix == "zz" { " zz" } else { suffix };
            let expected = format!("{}{expected_suffix}", vec!["nei"; 20].join(" "));
            for capacity in [16, 0, 128] {
                let mut text = String::with_capacity(capacity);
                if capacity > 0 {
                    text.push_str("舊值");
                }
                let old_capacity = text.capacity();
                let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
                    scheme.editing_text_into(&reading, &mut text);
                });
                assert_eq!(text, expected);
                assert_eq!(allocations, usize::from(old_capacity < expected.len()));
                let capacity = text.capacity();
                let empty = typed("");
                let reading = empty.segmentation();
                let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
                    empty.editing_text_into(&reading, &mut text);
                });
                assert!(text.is_empty());
                assert_eq!(allocations, 0);
                assert_eq!(text.capacity(), capacity);
            }
        }
    }

    #[test]
    fn editing_text_suffix_cold_buffer_allocates_once() {
        for suffix in ["zz", "'"] {
            let scheme = typed(&format!("{}{suffix}", "nei".repeat(20)));
            let reading = scheme.segmentation();
            let mut text = String::new();
            let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
                scheme.editing_text_into(&reading, &mut text);
            });
            let suffix = if suffix == "zz" { " zz" } else { suffix };
            assert_eq!(text, format!("{}{suffix}", vec!["nei"; 20].join(" ")));
            assert_eq!(allocations, 1, "尾部与分隔符应纳入首次容量预留");
        }
    }

    #[test]
    fn short_candidate_dedup_keeps_first_rows_without_temporary_heap_state() {
        let mut candidates = vec![
            candidate("你", 1),
            candidate("妳", 1),
            candidate("你", 1),
            candidate("你", 2),
        ];
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            deduplicate_candidates(&mut candidates);
        });
        assert_eq!(allocations, 0);
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| (candidate.text.as_str(), candidate.syllables))
                .collect::<Vec<_>>(),
            [("你", 1), ("妳", 1), ("你", 2)]
        );

        let mut large = (0..=SMALL_CANDIDATE_DEDUP)
            .map(|index| candidate(&format!("字{index}"), 1))
            .collect::<Vec<_>>();
        large.push(candidate("字0", 1));
        deduplicate_candidates(&mut large);
        assert_eq!(large.len(), SMALL_CANDIDATE_DEDUP + 1);
        assert_eq!(large[0].text, "字0");
    }

    #[test]
    fn the_whole_reading_comes_first_then_leading_spans() {
        let fixture = fixture();
        let scheme = typed("neihou");
        assert_eq!(scheme.editing_text(), "nei hou");
        assert_eq!(
            listed(&scheme, &fixture.dictionary),
            ["你好/2", "妳好/2", "你/1", "妳/1"]
        );
        let scheme = typed("gwongdungwaa");
        assert_eq!(scheme.editing_text(), "gwong dung waa");
        assert_eq!(
            listed(&scheme, &fixture.dictionary),
            ["廣東話/3", "光/3", "廣東/2", "光/1", "廣/1"]
        );
    }

    #[test]
    fn candidates_reserve_each_dictionary_batch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("msime-cantonese.db");
        build(&path);
        let connection = Connection::open(&path).unwrap();
        connection.execute("DELETE FROM entries", []).unwrap();
        for index in 0..23 {
            connection
                .execute(
                    "INSERT INTO entries VALUES (?1, ?2, ?3)",
                    ("nei haa", format!("字{index:02}"), index),
                )
                .unwrap();
        }
        drop(connection);
        let dictionary = open_read_only(&path).unwrap();

        let candidates = typed("neih").candidates(&dictionary).unwrap();
        assert_eq!(candidates.len(), 23);
        assert_eq!(candidates.capacity(), candidates.len());
    }

    #[test]
    fn candidates_into_reuses_candidate_rows_on_requery() {
        let fixture = fixture();
        let scheme = typed("neihou");
        let mut buffer = CantoneseQueryBuffer::default();
        let mut candidates = Vec::new();
        scheme
            .candidates_into(&fixture.dictionary, &mut buffer, &mut candidates)
            .unwrap();
        let pointers = candidates
            .iter()
            .map(|candidate| (candidate.text.as_ptr(), candidate.key.as_ptr()))
            .collect::<Vec<_>>();

        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            scheme
                .candidates_into(&fixture.dictionary, &mut buffer, &mut candidates)
                .unwrap();
        });
        assert!(
            allocations <= 8,
            "重复查询只应保留分段所需的少量分配：{allocations}"
        );
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| (candidate.text.as_ptr(), candidate.key.as_ptr()))
                .collect::<Vec<_>>(),
            pointers
        );
    }

    #[test]
    fn ime_scheme_exposes_the_active_cantonese_state() {
        let scheme = crate::ime::scheme::Scheme::Cantonese(typed("neih"));
        assert_eq!(scheme.as_cantonese().unwrap().input(), "neih");
    }

    #[test]
    fn a_trailing_prefix_is_completed() {
        let fixture = fixture();
        let scheme = typed("neih");
        assert_eq!(scheme.editing_text(), "nei h");
        let candidates = scheme.candidates(&fixture.dictionary).unwrap();
        let rows: Vec<_> = candidates
            .iter()
            .map(|candidate| (candidate.text.as_str(), candidate.key.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("你好", "nei hou"),
                ("妳好", "nei hou"),
                ("你向", "nei hoeng"),
                ("你", "nei"),
                ("妳", "nei")
            ]
        );
        assert_eq!(candidates[0].end, 4);
        assert_eq!(candidates[3].end, 3);
        // A prefix as the only syllable completes against single-syllable keys only.
        assert_eq!(listed(&typed("gw"), &fixture.dictionary), ["光/1", "廣/1"]);
    }

    #[test]
    fn a_complete_trailing_syllable_that_starts_a_longer_one_is_completed_too() {
        let fixture = fixture();
        // `ho` is a syllable with no `nei ho` entry; 你好 must stay listed between `neih` and `neihou`.
        let syllables = Inventory::new(SYLLABLES.iter().copied().chain(["ho"]));
        let mut scheme = CantoneseScheme::new(Arc::new(syllables));
        scheme.set_raw_input("neiho");
        assert_eq!(scheme.editing_text(), "nei ho");
        assert_eq!(
            listed(&scheme, &fixture.dictionary),
            ["你好/2", "妳好/2", "你向/2", "你/1", "妳/1"]
        );
        // Exact entries of the whole reading come before its completions: 我 for `ngo`, then 外 for `ngoi`.
        let scheme = typed("ngo");
        assert_eq!(listed(&scheme, &fixture.dictionary), ["我/1", "外/1"]);
    }

    #[test]
    fn the_apostrophe_and_the_longer_syllable() {
        let fixture = fixture();
        let scheme = typed("ngo'oi");
        assert_eq!(scheme.editing_text(), "ngo oi");
        assert_eq!(listed(&scheme, &fixture.dictionary), ["我/1"]);
        let scheme = typed("ngoi");
        assert_eq!(scheme.editing_text(), "ngoi");
        assert_eq!(listed(&scheme, &fixture.dictionary), ["外/1"]);
        let scheme = typed("nei'");
        assert_eq!(scheme.editing_text(), "nei'");
        assert_eq!(listed(&scheme, &fixture.dictionary), ["你/1", "妳/1"]);
    }

    #[test]
    fn unread_letters_leave_the_leading_spans() {
        let fixture = fixture();
        let scheme = typed("neihoux");
        assert_eq!(scheme.editing_text(), "nei hou x");
        assert_eq!(
            listed(&scheme, &fixture.dictionary),
            ["你好/2", "妳好/2", "你/1", "妳/1"]
        );
        let scheme = typed("xx");
        assert_eq!(scheme.editing_text(), "xx");
        assert!(listed(&scheme, &fixture.dictionary).is_empty());
        let scheme = typed("nei'qq");
        assert_eq!(scheme.editing_text(), "nei qq");
    }

    #[test]
    fn selecting_a_partial_span_keeps_the_rest_composing() {
        let fixture = fixture();
        let mut scheme = typed("nei'hou");
        let candidates = scheme.candidates(&fixture.dictionary).unwrap();
        let you = candidates.iter().find(|c| c.text == "你").unwrap();
        assert_eq!(you.syllables, 1);
        assert!(scheme.select(you));
        // The boundary after the selected letters goes with them.
        assert_eq!(scheme.input(), "hou");
        assert_eq!(listed(&scheme, &fixture.dictionary), ["好/1", "號/1"]);
        let good = scheme.candidates(&fixture.dictionary).unwrap().remove(0);
        assert!(!scheme.select(&good));
        assert!(scheme.is_empty());

        let mut scheme = typed("neihou");
        let whole = scheme.candidates(&fixture.dictionary).unwrap().remove(0);
        assert_eq!(whole.text, "你好");
        assert!(!scheme.select(&whole));
        assert!(scheme.is_empty());

        // Letters no syllable reads stay composing after a selection.
        let mut scheme = typed("neihoux");
        let first = scheme.candidates(&fixture.dictionary).unwrap().remove(0);
        assert!(scheme.select(&first));
        assert_eq!(scheme.input(), "x");
    }

    #[test]
    fn keys_and_host_edits() {
        let mut scheme = typed("'nei''");
        assert_eq!(scheme.input(), "nei'");
        for key in [
            SchemeKey::Letter(b'N'),
            SchemeKey::Letter(b'1'),
            SchemeKey::Semicolon,
            SchemeKey::Minus,
            SchemeKey::Symbol(b' '),
            SchemeKey::Requery,
        ] {
            scheme.handle_key(key);
        }
        assert_eq!(scheme.input(), "nei'");
        scheme.handle_key(SchemeKey::Backspace);
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.input(), "ne");

        scheme.set_raw_input("'Nei'' hou1");
        assert_eq!(scheme.input(), "nei'hou");
        let editing = scheme.editing_text();
        assert_eq!(editing, "nei hou");
        assert_eq!(editing.capacity(), editing.len());
        scheme.reset();
        assert!(scheme.is_empty());
        assert_eq!(scheme.editing_text(), "");
        let fixture = fixture();
        assert!(scheme.candidates(&fixture.dictionary).unwrap().is_empty());
    }

    #[test]
    fn set_raw_input_reserves_source_capacity() {
        let source: String = (0..100)
            .map(|index| if index % 5 == 3 { ' ' } else { 'a' })
            .collect();
        let mut scheme = CantoneseScheme::new(Arc::new(inventory()));
        scheme.set_raw_input(&source);
        assert_eq!(scheme.input.len(), source.len());
        assert_eq!(scheme.input.capacity(), source.len());
    }

    #[test]
    fn build_request_reuses_request_strings() {
        let mut scheme = typed("nei'hou");
        let mut request = scheme.build_request();
        let pointers = [
            request.raw_input.as_ptr(),
            request.raw_input_with_cases.as_ptr(),
            request.normalized_input.as_ptr(),
            request.raw_segmentation.as_ptr(),
            request.normalized_segmentation.as_ptr(),
            request.segmentation.as_ptr(),
        ];

        scheme.handle_key(SchemeKey::Requery);
        scheme.build_request_into(&mut request);

        assert_eq!(
            [
                request.raw_input.as_ptr(),
                request.raw_input_with_cases.as_ptr(),
                request.normalized_input.as_ptr(),
                request.raw_segmentation.as_ptr(),
                request.normalized_segmentation.as_ptr(),
                request.segmentation.as_ptr(),
            ],
            pointers
        );
    }

    #[test]
    fn an_edit_of_the_spaced_text_keeps_the_shown_boundaries() {
        // `ngo'oi` shows `ngo oi`; deleting its `o` must leave `ngo` and `i`, not the single syllable `ngoi`.
        let mut scheme = typed("ngo'oi");
        let mut text = scheme.editing_text();
        text.remove(4);
        assert_eq!(text, "ngo i");
        scheme.set_raw_input(&text);
        assert_eq!(scheme.input(), "ngo'i");
        assert_eq!(scheme.editing_text(), "ngo i");
        // A spaced reading read back is the same reading, with the boundaries made explicit.
        let mut scheme = typed("neihou");
        let text = scheme.editing_text();
        scheme.set_raw_input(&text);
        assert_eq!(scheme.input(), "nei'hou");
        assert_eq!(scheme.editing_text(), "nei hou");
    }

    #[test]
    fn the_dictionary_opens_with_its_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("msime-cantonese.db");
        build(&path);
        let opened = crate::cantonese::CantoneseDictionary::open(&path).unwrap();
        let mut scheme = CantoneseScheme::new(opened.inventory());
        scheme.set_raw_input("gwongdungwaa");
        assert_eq!(
            listed(&scheme, opened.dictionary()),
            ["廣東話/3", "光/3", "廣東/2", "光/1", "廣/1"]
        );
        assert!(
            crate::cantonese::CantoneseDictionary::open(&dir.path().join("missing.db")).is_err()
        );
    }
}
