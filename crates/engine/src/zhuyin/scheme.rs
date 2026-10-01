//! The Dachen bopomofo editor with libchewing's semantics: keys fill the pending syllable, a tone key completes it against the syllable inventory, and the completed syllables are reconverted after every change. Candidates come from a list the user opens; choosing one pins that span's text and never commits. Text leaves the editor only through Enter, Shift punctuation, auto-shift past `MAX_SYLLABLES` and `take_text`.

use std::collections::HashMap;

use super::conversion::{self, Span, MAX_SYLLABLES};
use super::layout::{self, DACHEN_SYMBOLS, IDLE_SYMBOLS, SHIFT_PUNCTUATION};
use super::syllable::PendingSyllable;
use crate::error::Result;
use crate::language_dictionary::{LanguageDictionary, LanguageEntry};
use crate::types::{QueryRequest, SchemeType};

/// The non-letter keys the editor still claims while the list is open: the phonetic keys that are not selection digits. Digits 1–9 and Space go to selection.
pub const LIST_OPEN_SYMBOLS: &str = "0,./;-";

/// A key as the editor sees it. The session maps host keys and command 16 (`Command::ConvertHanja`, "open the candidate list") onto these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZhuyinKey {
    /// A printable ASCII key: a Dachen key, a tone key, or a Shift punctuation key.
    Char(u8),
    /// Open the list, or close it when it is open.
    OpenList,
    Enter,
    Escape,
    Backspace,
}

/// One row of the candidate list: `text` for the syllables from `start` to the end of the composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListCandidate {
    pub text: String,
    pub start: usize,
}

/// A completed syllable and the keys that typed it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Syllable {
    toned: String,
    keys: String,
}

pub struct ZhuyinScheme {
    dictionary: LanguageDictionary,
    syllables: Vec<Syllable>,
    /// Spans whose text the user chose from the list; non-overlapping.
    pins: Vec<Span>,
    pending: PendingSyllable,
    /// The current conversion of `syllables`, recomputed after every change to them or to `pins`.
    conversion: Vec<Span>,
    list_open: bool,
    /// The list rows while `list_open`, empty otherwise.
    list: Vec<ListCandidate>,
    /// Text the last key committed, waiting for the session to hand it to the host.
    committed: String,
    /// The heaviest entry per key, for the composition in progress. The dictionary is read-only, so entries stay valid; the cache is dropped when the composition ends and on every auto-shift, so it holds at most the keys of `MAX_SYLLABLES` syllables.
    best: HashMap<String, Option<LanguageEntry>>,
}

impl ZhuyinScheme {
    /// An idle editor reading `dictionary` (`zhuyin.db`).
    pub fn new(dictionary: LanguageDictionary) -> Self {
        Self {
            dictionary,
            syllables: Vec::new(),
            pins: Vec::new(),
            pending: PendingSyllable::default(),
            conversion: Vec::new(),
            list_open: false,
            list: Vec::new(),
            committed: String::new(),
            best: HashMap::new(),
        }
    }

    /// The `zhuyin.db` connection, given back when the editor is replaced so the next one reuses it.
    pub fn into_dictionary(self) -> LanguageDictionary {
        self.dictionary
    }

    /// Drops the composition and any text not yet taken.
    pub fn reset(&mut self) {
        self.clear_composition();
        self.committed.clear();
    }

    pub fn is_composing(&self) -> bool {
        !self.syllables.is_empty() || !self.pending.is_empty()
    }

    /// Handles one key and returns whether the editor claimed it. An unclaimed key is left to the session and host (idle tone digits and Space type themselves; selection digits and Space while the list is open select through the session). Text the key committed is in `take_committed`.
    pub fn handle_key(&mut self, key: ZhuyinKey) -> Result<bool> {
        match key {
            ZhuyinKey::Char(byte) => self.handle_char(byte),
            _ if !self.is_composing() => Ok(false),
            ZhuyinKey::OpenList => {
                if self.list_open {
                    self.close_list();
                } else {
                    self.open_list()?;
                }
                Ok(true)
            }
            ZhuyinKey::Enter => {
                let text = self.take_text();
                self.committed.push_str(&text);
                Ok(true)
            }
            ZhuyinKey::Escape => {
                if self.list_open {
                    self.close_list();
                } else {
                    self.clear_composition();
                }
                Ok(true)
            }
            ZhuyinKey::Backspace => {
                if self.list_open {
                    self.close_list();
                } else if !self.pending.pop() {
                    let end = self.syllables.len();
                    self.syllables.pop();
                    self.pins.retain(|pin| pin.end != end);
                    self.reconvert()?;
                    if !self.is_composing() {
                        self.clear_composition();
                    }
                }
                Ok(true)
            }
        }
    }

    /// Pins the text of list row `index` over its span and closes the list. Nothing is committed. Returns false when the list is closed or has no such row.
    pub fn select(&mut self, index: usize) -> Result<bool> {
        if !self.list_open {
            return Ok(false);
        }
        let Some(candidate) = self.list.get(index).cloned() else {
            return Ok(false);
        };
        let pin = Span {
            start: candidate.start,
            end: self.syllables.len(),
            text: candidate.text,
        };
        self.pins
            .retain(|other| !other.overlaps(pin.start, pin.end));
        self.pins.push(pin);
        self.pins.sort_by_key(|pin| pin.start);
        self.close_list();
        self.reconvert()?;
        Ok(true)
    }

    /// The converted text, ending the composition; the pending syllable is dropped. This is what blur, a scheme switch and Enter commit.
    pub fn take_text(&mut self) -> String {
        let text = self.converted_text();
        self.clear_composition();
        text
    }

    /// The text the last key committed; empty when it committed none.
    pub fn take_committed(&mut self) -> String {
        std::mem::take(&mut self.committed)
    }

    /// The keys that typed the composition, in order, for the caret-locked editing text.
    pub fn editing_text(&self) -> String {
        build_editing_keys(&self.syllables, &self.pending)
    }

    /// The converted text followed by the pending bopomofo, e.g. `你好ㄇㄚ`.
    pub fn reading(&self) -> String {
        let mut reading = self.converted_text();
        reading.push_str(&self.pending.bopomofo());
        reading
    }

    /// What the session shows and tests for a composition: the reading, which is empty exactly when nothing is composing.
    pub fn preedit(&self) -> String {
        self.reading()
    }

    /// The request the session keeps for the composition. Nothing is queried with it, since the list rows come from the editor itself; `raw_input` is the typed keys the caret-locked editing text shows and `normalized_segmentation` the reading the snapshot draws.
    pub fn build_request(&self) -> QueryRequest {
        let keys = self.editing_text();
        QueryRequest {
            scheme: SchemeType::Zhuyin,
            raw_input: keys.clone(),
            raw_input_with_cases: keys.clone(),
            normalized_input: keys.clone(),
            raw_segmentation: keys,
            normalized_segmentation: self.reading(),
            valid: self.is_composing(),
            ..QueryRequest::default()
        }
    }

    pub fn converted_text(&self) -> String {
        build_converted_text(&self.conversion)
    }

    pub fn list_open(&self) -> bool {
        self.list_open
    }

    /// The list rows; empty unless the list is open.
    pub fn candidates(&self) -> &[ListCandidate] {
        &self.list
    }

    /// The non-letter keys the editor claims in its current state, in `DACHEN_SYMBOLS` order.
    pub fn spelling_symbols(&self) -> &'static str {
        if !self.is_composing() {
            IDLE_SYMBOLS
        } else if self.list_open {
            LIST_OPEN_SYMBOLS
        } else {
            DACHEN_SYMBOLS
        }
    }

    fn handle_char(&mut self, byte: u8) -> Result<bool> {
        if let Some((_, mark)) = SHIFT_PUNCTUATION.iter().find(|(key, _)| *key == byte) {
            let text = self.take_text();
            self.committed.push_str(&text);
            self.committed.push(*mark);
            return Ok(true);
        }
        if self.list_open {
            if !byte.is_ascii_lowercase() && !LIST_OPEN_SYMBOLS.as_bytes().contains(&byte) {
                return Ok(false);
            }
            self.close_list();
        }
        // Every phonetic key is a lowercase letter or one of IDLE_SYMBOLS, so each starts a composition from idle.
        if let Some((symbol, kind)) = layout::symbol(byte) {
            self.pending.insert(symbol, kind);
            return Ok(true);
        }
        let Some(mark) = layout::tone_mark(byte) else {
            return Ok(false);
        };
        if !self.is_composing() {
            return Ok(false);
        }
        let Some(toned) = self.pending.toned(mark) else {
            // A tone with nothing pending: Space opens the list, the tone digits are swallowed.
            if byte == b' ' {
                self.open_list()?;
            }
            return Ok(true);
        };
        // A syllable the inventory does not know stays pending; libchewing beeps here.
        if !self.dictionary.has_syllable(&toned)? {
            return Ok(true);
        }
        if self.syllables.len() == MAX_SYLLABLES {
            self.shift_leftmost_word();
        }
        let mut keys = self.pending.keys();
        keys.push(char::from(byte));
        self.syllables.push(Syllable { toned, keys });
        self.pending.clear();
        self.reconvert()?;
        Ok(true)
    }

    /// Commits the first converted word and drops its syllables, making room for one more.
    fn shift_leftmost_word(&mut self) {
        let Some(word) = self.conversion.first() else {
            return;
        };
        let end = word.end;
        self.committed.push_str(&word.text);
        self.syllables.drain(..end);
        self.best.clear();
        self.pins.retain(|pin| pin.start >= end);
        for pin in &mut self.pins {
            pin.start -= end;
            pin.end -= end;
        }
    }

    /// Lists every suffix span of the syllables, longest first, each span's entries by weight. Every entry is listed, since the list is the only way to choose a character; common syllables such as ㄧˋ have over 200. The list stays closed when it would be empty.
    fn open_list(&mut self) -> Result<()> {
        let count = self.syllables.len();
        let mut list = Vec::new();
        for start in 0..count {
            let key = self.key(start, count);
            let entries = self.dictionary.lookup(&key, usize::MAX)?;
            list.reserve(entries.len());
            for entry in entries {
                list.push(ListCandidate {
                    text: entry.text,
                    start,
                });
            }
        }
        self.list_open = !list.is_empty();
        self.list = list;
        Ok(())
    }

    fn close_list(&mut self) {
        self.list_open = false;
        self.list.clear();
    }

    fn clear_composition(&mut self) {
        self.syllables.clear();
        self.pins.clear();
        self.pending.clear();
        self.conversion.clear();
        self.close_list();
        self.best.clear();
    }

    fn key(&self, start: usize, end: usize) -> String {
        build_zhuyin_key(&self.syllables[start..end])
    }

    fn reconvert(&mut self) -> Result<()> {
        let syllables: Vec<&str> = self
            .syllables
            .iter()
            .map(|syllable| syllable.toned.as_str())
            .collect();
        let dictionary = &self.dictionary;
        let best = &mut self.best;
        self.conversion = conversion::convert(&syllables, &self.pins, |key| {
            if let Some(entry) = best.get(key) {
                return Ok(entry.clone());
            }
            let entry = dictionary.lookup(key, 1)?.into_iter().next();
            best.insert(key.to_owned(), entry.clone());
            Ok(entry)
        })?;
        Ok(())
    }
}

fn build_editing_keys(syllables: &[Syllable], pending: &PendingSyllable) -> String {
    let pending_capacity = usize::from(pending.initial.is_some())
        + usize::from(pending.medial.is_some())
        + usize::from(pending.rime.is_some());
    let capacity = syllables
        .iter()
        .map(|syllable| syllable.keys.len())
        .sum::<usize>()
        + pending_capacity;
    let mut keys = String::with_capacity(capacity);
    for syllable in syllables {
        keys.push_str(&syllable.keys);
    }
    pending.append_keys(&mut keys);
    keys
}

fn build_converted_text(spans: &[Span]) -> String {
    let capacity = spans.iter().map(|span| span.text.len()).sum();
    let mut text = String::with_capacity(capacity);
    for span in spans {
        text.push_str(&span.text);
    }
    text
}

fn build_zhuyin_key(syllables: &[Syllable]) -> String {
    let capacity = syllables
        .iter()
        .map(|syllable| syllable.toned.len())
        .sum::<usize>()
        .saturating_add(syllables.len().saturating_sub(1));
    let mut key = String::with_capacity(capacity);
    for (index, syllable) in syllables.iter().enumerate() {
        if index > 0 {
            key.push(' ');
        }
        key.push_str(&syllable.toned);
    }
    key
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::language_dictionary::{
        open_read_only, FORMAT_VERSION, METADATA_FORMAT_VERSION, SCHEMA,
    };

    const ENTRIES: [(&str, &str, i64); 15] = [
        ("ㄋㄧˇ", "你", 1000),
        ("ㄋㄧˇ", "妳", 300),
        ("ㄋㄧˇ", "擬", 50),
        ("ㄏㄠˇ", "好", 2000),
        ("ㄏㄠˇ", "郝", 10),
        ("ㄋㄧˇ ㄏㄠˇ", "你好", 500),
        ("ㄊㄞˊ", "台", 900),
        ("ㄊㄞˊ", "臺", 400),
        ("ㄨㄢ", "彎", 500),
        ("ㄨㄢ", "灣", 300),
        ("ㄊㄞˊ ㄨㄢ", "臺灣", 800),
        ("ㄊㄞˊ ㄨㄢ", "台灣", 600),
        ("ㄇㄚ˙", "嗎", 800),
        ("ㄇㄚ", "媽", 700),
        ("ㄢ", "安", 100),
    ];

    #[test]
    fn editing_keys_append_syllables_and_pending_keys_in_order() {
        let syllables = vec![
            Syllable {
                toned: "ㄋㄧˇ".to_owned(),
                keys: "su3".to_owned(),
            },
            Syllable {
                toned: "ㄏㄠˇ".to_owned(),
                keys: "lc3".to_owned(),
            },
        ];
        let pending = PendingSyllable {
            initial: Some('ㄇ'),
            medial: Some('ㄚ'),
            rime: None,
        };

        assert_eq!(build_editing_keys(&syllables, &pending), "su3lc3a8");
    }

    #[test]
    fn dictionary_key_joins_toned_syllables_in_order() {
        let syllables = vec![
            Syllable {
                toned: "ㄋㄧˇ".to_owned(),
                keys: "su3".to_owned(),
            },
            Syllable {
                toned: "ㄏㄠˇ".to_owned(),
                keys: "lc3".to_owned(),
            },
        ];

        assert_eq!(build_zhuyin_key(&syllables), "ㄋㄧˇ ㄏㄠˇ");
    }

    #[test]
    fn converted_text_appends_spans_in_order() {
        let spans = vec![
            Span {
                start: 0,
                end: 2,
                text: "你好".to_owned(),
            },
            Span {
                start: 2,
                end: 3,
                text: "嗎".to_owned(),
            },
        ];

        assert_eq!(build_converted_text(&spans), "你好嗎");
    }

    fn scheme() -> (tempfile::TempDir, ZhuyinScheme) {
        scheme_with(&ENTRIES)
    }

    fn scheme_with(entries: &[(&str, &str, i64)]) -> (tempfile::TempDir, ZhuyinScheme) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zhuyin.db");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(SCHEMA).unwrap();
        connection
            .execute(
                "INSERT INTO metadata VALUES (?1, ?2)",
                (METADATA_FORMAT_VERSION, FORMAT_VERSION.to_string()),
            )
            .unwrap();
        for &(key, text, weight) in entries {
            connection
                .execute(
                    "INSERT INTO entries VALUES (?1, ?2, ?3)",
                    (key, text, weight),
                )
                .unwrap();
            for syllable in key.split(' ') {
                connection
                    .execute("INSERT OR IGNORE INTO syllables VALUES (?1)", (syllable,))
                    .unwrap();
            }
        }
        drop(connection);
        let scheme = ZhuyinScheme::new(open_read_only(&path).unwrap());
        (dir, scheme)
    }

    /// Types `keys` as characters and returns what each claimed.
    fn type_keys(scheme: &mut ZhuyinScheme, keys: &str) -> Vec<bool> {
        keys.bytes()
            .map(|byte| scheme.handle_key(ZhuyinKey::Char(byte)).unwrap())
            .collect()
    }

    fn texts(scheme: &ZhuyinScheme) -> Vec<(&str, usize)> {
        scheme
            .candidates()
            .iter()
            .map(|candidate| (candidate.text.as_str(), candidate.start))
            .collect()
    }

    #[test]
    fn idle_claims_phonetic_keys_but_not_tone_keys() {
        let (_dir, mut scheme) = scheme();
        assert_eq!(scheme.spelling_symbols(), IDLE_SYMBOLS);
        for byte in *b"3467 " {
            assert!(!scheme.handle_key(ZhuyinKey::Char(byte)).unwrap());
            assert!(!scheme.is_composing());
        }
        for key in [
            ZhuyinKey::Enter,
            ZhuyinKey::Escape,
            ZhuyinKey::Backspace,
            ZhuyinKey::OpenList,
        ] {
            assert!(!scheme.handle_key(key).unwrap());
        }
        // Uppercase letters and other ASCII are not Dachen keys.
        assert_eq!(type_keys(&mut scheme, "A!'"), [false, false, false]);
        for start in ["1", ",", "-", "a"] {
            scheme.reset();
            assert_eq!(type_keys(&mut scheme, start), [true]);
            assert!(scheme.is_composing());
            assert_eq!(scheme.spelling_symbols(), DACHEN_SYMBOLS);
        }
        assert_eq!(scheme.reading(), "ㄇ");
        assert_eq!(scheme.editing_text(), "a");
    }

    #[test]
    fn slots_are_replaced_by_keys_of_the_same_kind() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, "sxuj");
        assert_eq!(scheme.reading(), "ㄌㄨ");
        assert_eq!(scheme.editing_text(), "xj");
        type_keys(&mut scheme, "sul3");
        assert_eq!(scheme.converted_text(), "");
        assert_eq!(scheme.reading(), "ㄋㄧㄠ");
        type_keys(&mut scheme, "c");
        type_keys(&mut scheme, "j");
        assert_eq!(scheme.reading(), "ㄏㄨㄠ");
    }

    #[test]
    fn tone_keys_complete_only_known_syllables() {
        let (_dir, mut scheme) = scheme();
        // ㄋˇ is not in the inventory: the key is consumed and ㄋ stays pending.
        assert_eq!(type_keys(&mut scheme, "s3"), [true, true]);
        assert_eq!(scheme.reading(), "ㄋ");
        assert_eq!(scheme.editing_text(), "s");
        type_keys(&mut scheme, "u3");
        assert_eq!(scheme.reading(), "你");
        assert_eq!(scheme.editing_text(), "su3");
        // A tone digit with nothing pending is swallowed.
        assert_eq!(type_keys(&mut scheme, "4"), [true]);
        assert_eq!(scheme.reading(), "你");
        // Space is tone 1 and 7 the neutral tone.
        type_keys(&mut scheme, "a8 a87");
        assert_eq!(scheme.reading(), "你媽嗎");
        assert_eq!(scheme.editing_text(), "su3a8 a87");
        assert_eq!(scheme.take_committed(), "");
    }

    #[test]
    fn conversion_prefers_words() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, "su3cl3");
        assert_eq!(scheme.converted_text(), "你好");
        type_keys(&mut scheme, "a8");
        assert_eq!(scheme.reading(), "你好ㄇㄚ");
        scheme.reset();
        type_keys(&mut scheme, "w96j0 ");
        assert_eq!(scheme.converted_text(), "臺灣");
    }

    #[test]
    fn the_list_holds_suffix_spans_longest_first() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, "w96j0 ");
        assert!(scheme.handle_key(ZhuyinKey::OpenList).unwrap());
        assert!(scheme.list_open());
        assert_eq!(scheme.spelling_symbols(), LIST_OPEN_SYMBOLS);
        assert_eq!(
            texts(&scheme),
            [("臺灣", 0), ("台灣", 0), ("彎", 1), ("灣", 1)]
        );
        // Command 16 again closes it.
        assert!(scheme.handle_key(ZhuyinKey::OpenList).unwrap());
        assert!(!scheme.list_open());
        assert!(scheme.candidates().is_empty());
        // Space with nothing pending opens it too.
        assert!(scheme.handle_key(ZhuyinKey::Char(b' ')).unwrap());
        assert!(scheme.list_open());
    }

    #[test]
    fn the_list_holds_every_entry_of_a_span() {
        // ㄧˋ has 215 single characters in the real data; the list must not cut off the light ones.
        let texts: Vec<String> = (0..215u32)
            .map(|index| char::from_u32(0x4E00 + index).unwrap().to_string())
            .collect();
        let entries: Vec<(&str, &str, i64)> = texts
            .iter()
            .zip((1..=215i64).rev())
            .map(|(text, weight)| ("ㄧˋ", text.as_str(), weight))
            .collect();
        let (_dir, mut scheme) = scheme_with(&entries);
        type_keys(&mut scheme, "u4");
        assert!(scheme.handle_key(ZhuyinKey::OpenList).unwrap());
        assert_eq!(scheme.candidates().len(), 215);
        assert_eq!(scheme.list.capacity(), scheme.list.len());
        assert_eq!(scheme.candidates()[214].text, texts[214]);
    }

    #[test]
    fn select_pins_without_committing_and_pins_survive_reconversion() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, "su3 ");
        assert_eq!(texts(&scheme), [("你", 0), ("妳", 0), ("擬", 0)]);
        assert!(scheme.select(1).unwrap());
        assert!(!scheme.list_open());
        assert_eq!(scheme.take_committed(), "");
        assert_eq!(scheme.reading(), "妳");
        // The pinned 妳 holds even though 你好 is the better word.
        type_keys(&mut scheme, "cl3");
        assert_eq!(scheme.converted_text(), "妳好");
        // Choosing the longer span replaces the pin it overlaps.
        type_keys(&mut scheme, " ");
        assert_eq!(texts(&scheme), [("你好", 0), ("好", 1), ("郝", 1)]);
        assert!(scheme.select(2).unwrap());
        assert_eq!(scheme.converted_text(), "妳郝");
        type_keys(&mut scheme, " ");
        assert!(scheme.select(0).unwrap());
        assert_eq!(scheme.converted_text(), "你好");
        type_keys(&mut scheme, "w96");
        assert_eq!(scheme.converted_text(), "你好台");
        assert!(!scheme.select(0).unwrap());
        assert!(!scheme.list_open());
        // Backspace drops the last syllable; the pin on the others stays.
        scheme.handle_key(ZhuyinKey::Backspace).unwrap();
        assert_eq!(scheme.converted_text(), "你好");
        assert_eq!(scheme.take_committed(), "");
    }

    #[test]
    fn list_keys_close_or_fall_through() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, "su3 ");
        assert!(scheme.list_open());
        // Selection digits and Space belong to the session while the list is open.
        assert_eq!(type_keys(&mut scheme, "1 3"), [false, false, false]);
        assert!(scheme.list_open());
        // 0 closes the list and types ㄢ.
        assert_eq!(type_keys(&mut scheme, "0"), [true]);
        assert!(!scheme.list_open());
        assert_eq!(scheme.reading(), "你ㄢ");
        type_keys(&mut scheme, " ");
        assert_eq!(scheme.reading(), "你安");
        // Esc and Backspace only close an open list.
        type_keys(&mut scheme, " ");
        assert!(scheme.handle_key(ZhuyinKey::Escape).unwrap());
        assert!(!scheme.list_open());
        assert_eq!(scheme.reading(), "你安");
        type_keys(&mut scheme, " ");
        assert!(scheme.handle_key(ZhuyinKey::Backspace).unwrap());
        assert_eq!(scheme.reading(), "你安");
        // A letter closes it and composes.
        type_keys(&mut scheme, " ");
        assert_eq!(type_keys(&mut scheme, "c"), [true]);
        assert!(!scheme.list_open());
        assert_eq!(scheme.reading(), "你安ㄏ");
    }

    #[test]
    fn enter_commits_the_conversion_and_esc_clears() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, "su3cl3a");
        assert!(scheme.handle_key(ZhuyinKey::Enter).unwrap());
        assert_eq!(scheme.take_committed(), "你好");
        assert!(!scheme.is_composing());

        type_keys(&mut scheme, "su3cl3");
        assert!(scheme.handle_key(ZhuyinKey::Escape).unwrap());
        assert!(!scheme.is_composing());
        assert_eq!(scheme.take_committed(), "");

        type_keys(&mut scheme, "su3");
        assert_eq!(scheme.take_text(), "你");
        assert!(!scheme.is_composing());
    }

    #[test]
    fn backspace_drops_a_symbol_then_a_syllable() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, "su3cl3ju");
        let mut shown = vec![scheme.reading()];
        while scheme.is_composing() {
            assert!(scheme.handle_key(ZhuyinKey::Backspace).unwrap());
            shown.push(scheme.reading());
        }
        assert_eq!(shown, ["你好ㄧ", "你好", "你", ""]);
        assert_eq!(scheme.spelling_symbols(), IDLE_SYMBOLS);
    }

    #[test]
    fn shift_punctuation_commits_then_inserts_the_mark() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, "su3cl3a");
        assert_eq!(type_keys(&mut scheme, "<"), [true]);
        assert_eq!(scheme.take_committed(), "你好，");
        assert!(!scheme.is_composing());
        assert_eq!(type_keys(&mut scheme, "["), [true]);
        assert_eq!(scheme.take_committed(), "「");
        type_keys(&mut scheme, "su3 ");
        assert_eq!(type_keys(&mut scheme, "?"), [true]);
        assert_eq!(scheme.take_committed(), "你？");
        assert!(!scheme.list_open());
    }

    #[test]
    fn the_21st_syllable_commits_the_leftmost_word() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, &"su3cl3".repeat(10));
        assert_eq!(scheme.converted_text(), "你好".repeat(10));
        assert_eq!(scheme.take_committed(), "");
        type_keys(&mut scheme, "su");
        assert_eq!(scheme.take_committed(), "");
        type_keys(&mut scheme, "3");
        assert_eq!(scheme.take_committed(), "你好");
        assert_eq!(scheme.converted_text(), format!("{}你", "你好".repeat(9)));
        assert_eq!(scheme.editing_text(), format!("{}su3", "su3cl3".repeat(9)));
        // The lookup cache keeps no key longer than the syllables that are left.
        let longest = scheme.best.keys().map(|key| key.split(' ').count()).max();
        assert_eq!(longest, Some(scheme.syllables.len()));

        // A pin to the right of the shifted word moves with its syllables.
        scheme.reset();
        type_keys(&mut scheme, &"su3cl3".repeat(10));
        type_keys(&mut scheme, " ");
        assert!(scheme.select(2).unwrap());
        assert_eq!(scheme.converted_text(), format!("{}你郝", "你好".repeat(9)));
        type_keys(&mut scheme, "su3");
        assert_eq!(scheme.take_committed(), "你好");
        assert_eq!(
            scheme.converted_text(),
            format!("{}你郝你", "你好".repeat(8))
        );
    }
}
