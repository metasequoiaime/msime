//! Korean Hangul key handling on the Dubeolsik layout. The composition is the key letters of the open syllable; a key that starts the next syllable moves the finished one into `committed`, which the session hands to the host with the key's result. While the Hanja list is open the request asks for the syllable's Hanja; any edit of the composition closes the list.

use super::dubeolsik::{compose, compose_into, split_finished};
use crate::types::{QueryRequest, SchemeKey, SchemeType};

#[derive(Debug, Clone, Default)]
pub struct KoreanScheme {
    /// Key letters of the text still composing, case kept: Shift selects the double consonants and ㅒ ㅖ.
    raw: String,
    /// Syllables the last key finished, waiting for the session to commit them.
    committed: String,
    /// The Hanja list of the composing syllable is open.
    hanja: bool,
}

impl KoreanScheme {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.raw.clear();
        self.committed.clear();
        self.hanja = false;
    }

    /// Letters feed the automaton and Backspace removes the last jamo keystroke; every other key is ignored. A letter that opens a new syllable moves the finished ones to `committed`. Both edit the syllable the Hanja list was for, so both close it.
    pub fn handle_key(&mut self, key: SchemeKey) {
        match key {
            SchemeKey::Backspace => {
                self.hanja = false;
                self.raw.pop();
            }
            SchemeKey::Letter(letter) if letter.is_ascii_alphabetic() => {
                self.hanja = false;
                self.raw.push(char::from(letter));
                let (finished, open) = split_finished(&self.raw);
                if !finished.is_empty() {
                    self.committed.push_str(&finished);
                    let open_start = self.raw.len() - open.len();
                    self.raw.drain(..open_start);
                }
            }
            SchemeKey::Letter(_)
            | SchemeKey::Apostrophe
            | SchemeKey::Semicolon
            | SchemeKey::Minus
            | SchemeKey::Symbol(_)
            | SchemeKey::Requery => {}
        }
    }

    /// Only the composing text is described: `normalized_segmentation` carries the Hangul the way the Japanese scheme carries its kana reading, and is the syllable the Hanja table is read with while the list is open.
    pub fn build_request(&self) -> QueryRequest {
        let mut request = QueryRequest::default();
        self.build_request_into(&mut request);
        request
    }

    /// 将韩文请求写入已有存储，避免逐键刷新重复分配相同的按键字符串。
    pub fn build_request_into(&self, request: &mut QueryRequest) {
        request.scheme = SchemeType::Korean;
        request.raw_input.clear();
        request.raw_input.extend(
            self.raw
                .bytes()
                .map(|byte| char::from(byte.to_ascii_lowercase())),
        );
        request.raw_input_with_cases.clone_from(&self.raw);
        request.normalized_input.clone_from(&request.raw_input);
        request.raw_segmentation.clone_from(&self.raw);
        compose_into(&self.raw, &mut request.normalized_segmentation);
        request
            .segmentation
            .clone_from(&request.normalized_segmentation);
        request.korean_hanja = self.hanja;
        request.valid = !self.raw.is_empty();
    }

    /// The composed Hangul, never the key letters.
    pub fn preedit(&self) -> String {
        compose(&self.raw)
    }

    /// Keeps letters only; nothing is committed, so a host edit that spells several syllables keeps them all composing. The cased spelling wins when the host sent one, because case selects jamo here.
    pub fn set_raw_input(&mut self, raw: &str, raw_with_cases: &str) {
        let source = if raw_with_cases.is_empty() {
            raw
        } else {
            raw_with_cases
        };
        self.raw.clear();
        self.raw.reserve(source.len());
        self.raw
            .extend(source.chars().filter(char::is_ascii_alphabetic));
        self.committed.clear();
        self.hanja = false;
    }

    /// Asks for the Hanja of the composing text; the session keeps the list open only if the table has some.
    pub fn open_hanja(&mut self) {
        self.hanja = !self.raw.is_empty();
    }

    pub fn close_hanja(&mut self) {
        self.hanja = false;
    }

    pub fn hanja_open(&self) -> bool {
        self.hanja
    }

    /// The syllables the last key finished; empty when it finished none.
    pub fn take_committed(&mut self) -> String {
        std::mem::take(&mut self.committed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Types `keys` and collects what each key committed.
    fn typed(keys: &str) -> (KoreanScheme, String) {
        let mut scheme = KoreanScheme::new();
        let mut committed = String::new();
        for byte in keys.bytes() {
            scheme.handle_key(SchemeKey::Letter(byte));
            committed.push_str(&scheme.take_committed());
        }
        (scheme, committed)
    }

    #[test]
    fn finished_syllables_leave_the_composition() {
        let cases = [
            ("rk", "", "가"),
            ("rks", "", "간"),
            ("rksk", "가", "나"),
            ("dkssud", "안", "녕"),
            ("dkssudgktpdy", "안녕하세", "요"),
            ("ekfr", "", "닭"),
            ("ekfrk", "달", "가"),
            ("rr", "ㄱ", "ㄱ"),
            ("kk", "ㅏ", "ㅏ"),
            ("rhk", "", "과"),
            ("RkTkEk", "까싸", "따"),
        ];
        for (keys, committed, preedit) in cases {
            let (scheme, text) = typed(keys);
            assert_eq!(text, committed, "{keys}");
            assert_eq!(scheme.preedit(), preedit, "{keys}");
        }
    }

    #[test]
    fn the_composition_keeps_only_the_open_syllable_keys() {
        let (scheme, _) = typed("ekfrk");
        assert_eq!(scheme.build_request().raw_input_with_cases, "rk");
        let (scheme, _) = typed("dkssud");
        assert_eq!(scheme.build_request().raw_input_with_cases, "sud");
    }

    #[test]
    fn set_raw_input_reuses_existing_storage() {
        let mut scheme = KoreanScheme::new();
        scheme.set_raw_input("abcdefghijklmnopqrstuvwxyz", "");
        let capacity = scheme.raw.capacity();

        scheme.set_raw_input("rk", "");

        assert_eq!(scheme.build_request().raw_input_with_cases, "rk");
        assert!(scheme.raw.capacity() >= capacity);
    }

    #[test]
    fn moving_a_final_reuses_the_open_key_storage() {
        let mut scheme = KoreanScheme::new();
        scheme.set_raw_input("abcdefghijklmnop", "");
        scheme.reset();
        scheme.committed.reserve(16);
        for key in b"rks" {
            scheme.handle_key(SchemeKey::Letter(*key));
        }
        let pointer = scheme.raw.as_ptr();

        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            scheme.handle_key(SchemeKey::Letter(b'k'));
        });

        assert_eq!(allocations, 2);
        assert_eq!(scheme.raw.as_ptr(), pointer);
        assert_eq!(scheme.preedit(), "나");
        assert_eq!(scheme.take_committed(), "가");
    }

    #[test]
    fn backspace_removes_one_jamo_at_a_time() {
        let (mut scheme, _) = typed("ekfr");
        let mut shown = vec![scheme.preedit()];
        while !scheme.preedit().is_empty() {
            scheme.handle_key(SchemeKey::Backspace);
            shown.push(scheme.preedit());
        }
        assert_eq!(shown, ["닭", "달", "다", "ㄷ", ""]);

        let (mut scheme, _) = typed("rhkd");
        let mut shown = vec![scheme.preedit()];
        while !scheme.preedit().is_empty() {
            scheme.handle_key(SchemeKey::Backspace);
            shown.push(scheme.preedit());
        }
        assert_eq!(shown, ["광", "과", "고", "ㄱ", ""]);

        // After a final moved on, Backspace edits the new syllable only; the committed one is gone.
        let (mut scheme, committed) = typed("rksk");
        assert_eq!(committed, "가");
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.preedit(), "ㄴ");
        assert_eq!(scheme.take_committed(), "");
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.preedit(), "");
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.preedit(), "");
    }

    #[test]
    fn other_keys_are_ignored() {
        let (mut scheme, _) = typed("rk");
        for key in [
            SchemeKey::Apostrophe,
            SchemeKey::Semicolon,
            SchemeKey::Minus,
            SchemeKey::Requery,
            SchemeKey::Letter(b'1'),
            SchemeKey::Symbol(b';'),
            SchemeKey::Symbol(b' '),
        ] {
            scheme.handle_key(key);
        }
        assert_eq!(scheme.preedit(), "가");
        assert_eq!(scheme.take_committed(), "");
    }

    #[test]
    fn set_raw_input_round_trips_and_prefers_the_cased_spelling() {
        let (scheme, _) = typed("Rks");
        let request = scheme.build_request();
        let mut restored = KoreanScheme::new();
        restored.set_raw_input(&request.raw_input, &request.raw_input_with_cases);
        assert_eq!(restored.preedit(), "깐");
        assert_eq!(restored.build_request(), request);

        // Without the cased spelling the lowercase letters are all there is.
        restored.set_raw_input("rks", "");
        assert_eq!(restored.preedit(), "간");

        // Several syllables stay composing, and non-letters are dropped.
        restored.set_raw_input("dkssud", "dks'sud 1");
        assert_eq!(restored.preedit(), "안녕");
        assert_eq!(restored.take_committed(), "");
        // The next key finishes everything before the open syllable.
        restored.handle_key(SchemeKey::Letter(b'g'));
        assert_eq!(restored.take_committed(), "안녕");
        assert_eq!(restored.preedit(), "ㅎ");
    }

    #[test]
    fn set_raw_input_reserves_source_capacity() {
        let source: String = (0..100)
            .map(|index| if index % 5 == 0 { '1' } else { 'a' })
            .collect();
        let mut scheme = KoreanScheme::new();
        scheme.set_raw_input(&source, "");
        assert_eq!(scheme.raw.len(), 80);
        assert_eq!(scheme.raw.capacity(), source.len());
    }

    #[test]
    fn request_fields() {
        let (scheme, _) = typed("gkS");
        let request = scheme.build_request();
        assert_eq!(request.scheme, SchemeType::Korean);
        assert_eq!(request.raw_input, "gks");
        assert_eq!(request.normalized_input, "gks");
        assert_eq!(request.raw_input_with_cases, "gkS");
        assert_eq!(request.raw_segmentation, "gkS");
        assert_eq!(request.normalized_segmentation, "한");
        assert_eq!(request.segmentation, "한");
        assert!(request.valid);
        assert!(!KoreanScheme::new().build_request().valid);
    }

    #[test]
    fn reset_drops_the_composition_and_pending_commits() {
        let mut scheme = KoreanScheme::new();
        for byte in b"rkr" {
            scheme.handle_key(SchemeKey::Letter(*byte));
        }
        scheme.handle_key(SchemeKey::Letter(b'r'));
        scheme.reset();
        assert_eq!(scheme.preedit(), "");
        assert_eq!(scheme.take_committed(), "");
    }
}
