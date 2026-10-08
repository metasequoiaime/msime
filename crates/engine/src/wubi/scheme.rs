//! Wubi key handling (`R/schemes/wubi_scheme.*`): letters `a..=y`, `z` only while mixed pinyin is allowed, at most four letters or 32 while the pinyin fallback is extending the code.

use crate::types::{QueryRequest, SchemeKey, SchemeType};

pub const MAX_CODE_LENGTH: usize = 4;
/// Long enough for any pinyin spelling, short enough to stay a composition rather than a paragraph.
pub const MAX_MIXED_CODE_LENGTH: usize = 32;

#[derive(Debug, Clone, Default)]
pub struct WubiScheme {
    raw: String,
    /// No wubi code uses `z`, but mixed input has to accept it: dropping it would not refuse a pinyin spelling, it would silently turn `zhongguo` into `hongguo`. It follows the setting rather than the last query because the letter can open a composition.
    mixed_pinyin_allowed: bool,
    /// A fifth letter is refused unless mixed input found that the table cannot answer the code in hand, so `nihao` can be finished while a fluent wubi typist sees no change.
    extended_length_allowed: bool,
}

fn is_wubi_letter(lower: u8, mixed_pinyin_allowed: bool) -> bool {
    (b'a'..=b'y').contains(&lower) || (mixed_pinyin_allowed && lower == b'z')
}

impl WubiScheme {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears the code and the extended-length allowance.
    pub fn reset(&mut self) {
        self.raw.clear();
        self.extended_length_allowed = false;
    }

    /// Keys beyond the length limit or outside the alphabet are ignored, which the session reports as unhandled.
    pub fn handle_key(&mut self, key: SchemeKey) {
        match key {
            SchemeKey::Backspace => {
                self.raw.pop();
            }
            SchemeKey::Letter(letter) if letter.is_ascii_alphabetic() => {
                let lower = letter.to_ascii_lowercase();
                if is_wubi_letter(lower, self.mixed_pinyin_allowed)
                    && self.raw.len() < self.max_code_length()
                {
                    self.raw.push(char::from(lower));
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

    /// Every string field is the raw code; valid when non-empty.
    pub fn build_request(&self) -> QueryRequest {
        QueryRequest {
            scheme: SchemeType::Wubi,
            raw_input: self.raw.clone(),
            raw_input_with_cases: self.raw.clone(),
            normalized_input: self.raw.clone(),
            raw_segmentation: self.raw.clone(),
            normalized_segmentation: self.raw.clone(),
            segmentation: self.raw.clone(),
            valid: !self.raw.is_empty(),
            ..QueryRequest::default()
        }
    }

    pub fn preedit(&self) -> String {
        self.raw.clone()
    }

    /// Host editing: lowercase, filter to the alphabet and clip to the current limit. The C++ read `raw_input_with_cases` when it was non-empty; after lowercasing both spellings give the same code, so either may be passed.
    pub fn set_raw_input(&mut self, raw: &str) {
        let limit = self.max_code_length();
        self.raw.clear();
        self.raw.reserve(limit.min(raw.len()));
        self.raw.extend(
            raw.bytes()
                .map(|byte| byte.to_ascii_lowercase())
                .filter(|&lower| is_wubi_letter(lower, self.mixed_pinyin_allowed))
                .take(limit)
                .map(char::from),
        );
    }

    /// Exactly four letters. Longer mixed-pinyin spellings are fallback queries, not complete wubi codes.
    pub fn has_complete_code(&self) -> bool {
        self.raw.len() == MAX_CODE_LENGTH
    }

    pub fn set_mixed_pinyin_allowed(&mut self, allowed: bool) {
        self.mixed_pinyin_allowed = allowed;
    }

    pub fn set_extended_length_allowed(&mut self, allowed: bool) {
        self.extended_length_allowed = allowed;
    }

    fn max_code_length(&self) -> usize {
        if self.extended_length_allowed {
            MAX_MIXED_CODE_LENGTH
        } else {
            MAX_CODE_LENGTH
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(scheme: &mut WubiScheme, text: &str) {
        for byte in text.bytes() {
            let key = match byte {
                b'\'' => SchemeKey::Apostrophe,
                _ => SchemeKey::Letter(byte),
            };
            scheme.handle_key(key);
        }
    }

    // test_input_session.cpp:775-782: z, a fifth letter and an apostrophe are refused.
    #[test]
    fn refuses_z_a_fifth_letter_and_apostrophes() {
        let mut scheme = WubiScheme::new();
        typed(&mut scheme, "z");
        assert_eq!(scheme.preedit(), "");
        typed(&mut scheme, "abcd");
        typed(&mut scheme, "e");
        assert_eq!(scheme.preedit(), "abcd");
        typed(&mut scheme, "'");
        assert_eq!(scheme.preedit(), "abcd");
        assert!(scheme.has_complete_code());
    }

    #[test]
    fn symbol_keys_are_ignored() {
        let mut scheme = WubiScheme::new();
        typed(&mut scheme, "ab");
        scheme.handle_key(SchemeKey::Symbol(b';'));
        scheme.handle_key(SchemeKey::Symbol(b' '));
        assert_eq!(scheme.preedit(), "ab");
    }

    #[test]
    fn uppercase_letters_are_lowercased() {
        let mut scheme = WubiScheme::new();
        typed(&mut scheme, "WQ");
        assert_eq!(scheme.preedit(), "wq");
    }

    #[test]
    fn mixed_pinyin_accepts_z_and_extended_length_lifts_the_limit() {
        let mut scheme = WubiScheme::new();
        scheme.set_mixed_pinyin_allowed(true);
        typed(&mut scheme, "zhongguo");
        assert_eq!(scheme.preedit(), "zhon");
        assert!(scheme.has_complete_code());

        scheme.set_extended_length_allowed(true);
        typed(&mut scheme, "gguo");
        assert_eq!(scheme.preedit(), "zhongguo");
        assert!(!scheme.has_complete_code());

        // Reset drops the allowance but keeps the setting-driven z.
        scheme.reset();
        assert_eq!(scheme.preedit(), "");
        typed(&mut scheme, "zhongguo");
        assert_eq!(scheme.preedit(), "zhon");
    }

    #[test]
    fn backspace_and_return() {
        let mut scheme = WubiScheme::new();
        typed(&mut scheme, "wq");
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.preedit(), "w");
        scheme.handle_key(SchemeKey::Backspace);
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.preedit(), "");
        typed(&mut scheme, "ab");
        scheme.reset();
        assert_eq!(scheme.preedit(), "");
    }

    #[test]
    fn set_raw_input_normalises_and_clips() {
        let mut scheme = WubiScheme::new();
        scheme.set_raw_input("A'b-Z9cdef");
        assert_eq!(scheme.preedit(), "abcd");

        scheme.set_mixed_pinyin_allowed(true);
        scheme.set_extended_length_allowed(true);
        scheme.set_raw_input("Ni'Hao");
        assert_eq!(scheme.preedit(), "nihao");
    }

    #[test]
    fn set_raw_input_reuses_existing_storage() {
        let mut scheme = WubiScheme::new();
        scheme.set_mixed_pinyin_allowed(true);
        scheme.set_extended_length_allowed(true);
        scheme.set_raw_input("abcdefghijklmnopqrstuvwxyzabcdef");
        let capacity = scheme.raw.capacity();

        scheme.set_raw_input("ab");

        assert_eq!(scheme.preedit(), "ab");
        assert!(scheme.raw.capacity() >= capacity);
    }

    #[test]
    fn request_carries_the_code_in_every_field() {
        let mut scheme = WubiScheme::new();
        assert!(!scheme.build_request().valid);
        typed(&mut scheme, "wq");
        let request = scheme.build_request();
        assert_eq!(request.scheme, SchemeType::Wubi);
        assert!(request.valid);
        for field in [
            &request.raw_input,
            &request.raw_input_with_cases,
            &request.normalized_input,
            &request.raw_segmentation,
            &request.normalized_segmentation,
            &request.segmentation,
        ] {
            assert_eq!(field, "wq");
        }
    }
}
