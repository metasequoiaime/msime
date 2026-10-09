//! Japanese romaji key handling (schemes-lang.md §5.4).

use super::romaji::{convert_romaji, hiragana_to_romaji, next_kana_variant, romaji_reading_into};
use crate::types::{QueryRequest, SchemeKey, SchemeType};

#[derive(Debug, Clone, Default)]
pub struct JapaneseRomajiScheme {
    raw: String,
}

impl JapaneseRomajiScheme {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.raw.clear();
    }

    pub fn handle_key(&mut self, key: SchemeKey) -> bool {
        match key {
            SchemeKey::Backspace => self.raw.pop().is_some(),
            SchemeKey::Apostrophe => {
                self.raw.push('\'');
                true
            }
            // The physical minus key spells the long-vowel mark, so it is input here rather than a symbol.
            SchemeKey::Minus => {
                self.raw.push('-');
                true
            }
            // Case is kept for `raw_input_with_cases`; conversion lowercases.
            SchemeKey::Letter(letter) if letter.is_ascii_alphabetic() => {
                self.raw.push(char::from(letter));
                true
            }
            SchemeKey::Letter(_)
            | SchemeKey::Semicolon
            | SchemeKey::Symbol(_)
            | SchemeKey::Requery => false,
        }
    }

    /// `normalized_segmentation` is the hiragana plus pending letters.
    pub fn build_request(&self) -> QueryRequest {
        let mut request = QueryRequest::default();
        self.build_request_into(&mut request);
        request
    }

    /// 将日文请求写入已有存储，避免逐键刷新重复构建罗马字和假名字符串。
    pub fn build_request_into(&self, request: &mut QueryRequest) {
        request.scheme = SchemeType::JapaneseRomaji;
        request.raw_input.clear();
        request.raw_input.extend(
            self.raw
                .bytes()
                .map(|byte| char::from(byte.to_ascii_lowercase())),
        );
        request.normalized_input.clone_from(&request.raw_input);
        request.raw_input_with_cases.clone_from(&self.raw);
        request.raw_segmentation.clone_from(&self.raw);
        romaji_reading_into(&request.raw_input, &mut request.normalized_segmentation);
        request
            .segmentation
            .clone_from(&request.normalized_segmentation);
        request.valid = !self.raw.is_empty();
    }

    pub fn preedit(&self) -> String {
        self.raw.clone()
    }

    /// Keeps letters, `'` and `-`. The cased spelling wins when the host sent one.
    pub fn set_raw_input(&mut self, raw: &str, raw_with_cases: &str) {
        let source = if raw_with_cases.is_empty() {
            raw
        } else {
            raw_with_cases
        };
        self.raw.clear();
        self.raw.reserve(source.len());
        self.raw.extend(source.chars().filter(|&character| {
            character.is_ascii_alphabetic() || character == '\'' || character == '-'
        }));
    }

    /// Replace the last kana with its next variant and re-romanise; false when nothing changed. A half-typed romaji tail is not a kana yet, so there is nothing to modify.
    pub fn cycle_last_kana_variant(&mut self) -> bool {
        let conversion = convert_romaji(&self.raw);
        if !conversion.pending.is_empty() {
            return false;
        }
        let Some((start, _)) = conversion.hiragana.char_indices().next_back() else {
            return false;
        };
        let last = &conversion.hiragana[start..];
        let next = next_kana_variant(last);
        if next == last {
            return false;
        }
        let romaji = hiragana_to_romaji(&format!("{}{next}", &conversion.hiragana[..start]));
        if romaji.is_empty() {
            return false;
        }
        self.raw = romaji;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> JapaneseRomajiScheme {
        let mut scheme = JapaneseRomajiScheme::new();
        for byte in text.bytes() {
            scheme.handle_key(match byte {
                b'\'' => SchemeKey::Apostrophe,
                b'-' => SchemeKey::Minus,
                _ => SchemeKey::Letter(byte),
            });
        }
        scheme
    }

    fn reading(scheme: &JapaneseRomajiScheme) -> String {
        scheme.build_request().normalized_segmentation
    }

    // test_input_session.cpp:700-724: romaji gates.
    #[test]
    fn letters_apostrophe_and_long_vowel_are_input() {
        assert_eq!(typed("k").preedit(), "k");
        let long_vowel = typed("ra-menn");
        assert_eq!(long_vowel.preedit(), "ra-menn");
        assert_eq!(reading(&long_vowel), "らーめん");
        assert_eq!(typed("-").preedit(), "-");
        assert_eq!(typed("n'a").preedit(), "n'a");

        let mut scheme = typed("ka");
        scheme.handle_key(SchemeKey::Semicolon);
        scheme.handle_key(SchemeKey::Letter(b'1'));
        scheme.handle_key(SchemeKey::Symbol(b'-'));
        scheme.handle_key(SchemeKey::Symbol(b';'));
        assert_eq!(scheme.preedit(), "ka");
    }

    #[test]
    fn set_raw_input_keeps_long_vowels_and_prefers_the_cased_spelling() {
        let mut scheme = JapaneseRomajiScheme::new();
        scheme.set_raw_input("ko-hi-", "ko-hi-");
        assert_eq!(scheme.preedit(), "ko-hi-");
        scheme.set_raw_input("tokyo", "ToKyo 1!");
        assert_eq!(scheme.preedit(), "ToKyo");
        scheme.set_raw_input("n'a", "");
        assert_eq!(scheme.preedit(), "n'a");
    }

    #[test]
    fn set_raw_input_reuses_existing_storage() {
        let mut scheme = JapaneseRomajiScheme::new();
        scheme.set_raw_input("abcdefghijklmnopqrstuvwxyz", "");
        let capacity = scheme.raw.capacity();

        scheme.set_raw_input("ka", "");

        assert_eq!(scheme.preedit(), "ka");
        assert!(scheme.raw.capacity() >= capacity);
    }

    #[test]
    fn request_fields() {
        let scheme = typed("NihonG");
        let request = scheme.build_request();
        assert_eq!(request.scheme, SchemeType::JapaneseRomaji);
        assert_eq!(request.raw_input, "nihong");
        assert_eq!(request.normalized_input, "nihong");
        assert_eq!(request.raw_input_with_cases, "NihonG");
        assert_eq!(request.raw_segmentation, "NihonG");
        assert_eq!(request.normalized_segmentation, "にほんg");
        assert_eq!(request.segmentation, "にほんg");
        assert!(request.valid);
        assert!(!JapaneseRomajiScheme::new().build_request().valid);
    }

    fn assert_warm_request_has_no_temporary_conversion(raw: &str, reading: &str) {
        let scheme = typed(raw);
        let mut request = scheme.build_request();
        assert_eq!(request.normalized_segmentation, reading);
        let expected = request.clone();
        let pointer = request.normalized_segmentation.as_ptr();
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            scheme.build_request_into(&mut request);
        });
        assert_eq!(request, expected);
        assert_eq!(request.normalized_segmentation.as_ptr(), pointer);
        eprintln!("日文请求 {raw} 热构造分配：{allocations}");
        assert_eq!(allocations, 0, "已有请求缓冲不应构造临时转换字符串");
    }

    #[test]
    fn warm_complete_requests_do_not_allocate_conversion_strings() {
        for (raw, reading) in [
            ("nihongo", "にほんご"),
            ("Sinnyou", "しんよう"),
            ("n'a", "んあ"),
            ("xtsu", "っ"),
            ("matcha", "まっちゃ"),
            ("ko-hi-", "こーひー"),
            ("", ""),
        ] {
            assert_warm_request_has_no_temporary_conversion(raw, reading);
        }
    }

    #[test]
    fn warm_pending_requests_do_not_allocate_conversion_strings() {
        for (raw, reading) in [("NiHoNg", "にほんg"), ("kak", "かk"), ("k", "k")] {
            assert_warm_request_has_no_temporary_conversion(raw, reading);
        }
    }

    #[test]
    fn backspace_escape_and_return() {
        let mut scheme = typed("kan");
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.preedit(), "ka");
        scheme.reset();
        assert_eq!(scheme.preedit(), "");
        let mut scheme = typed("ka");
        scheme.reset();
        assert_eq!(scheme.preedit(), "");
    }

    // test_input_session.cpp:729-759: the variant key modifies the last kana instead of inserting one.
    #[test]
    fn kana_variant_cycling() {
        let mut scheme = typed("ka");
        assert_eq!(reading(&scheme), "か");
        assert!(scheme.cycle_last_kana_variant());
        assert_eq!(reading(&scheme), "が");
        assert!(scheme.cycle_last_kana_variant());
        assert_eq!(reading(&scheme), "か");

        let mut scheme = typed("kaha");
        assert!(scheme.cycle_last_kana_variant());
        assert_eq!(reading(&scheme), "かば");
        assert!(scheme.cycle_last_kana_variant());
        assert_eq!(reading(&scheme), "かぱ");
        assert!(scheme.cycle_last_kana_variant());
        assert_eq!(reading(&scheme), "かは");

        let mut scheme = typed("tsu");
        assert!(scheme.cycle_last_kana_variant());
        assert_eq!(reading(&scheme), "っ");

        let mut pending = typed("k");
        assert!(!pending.cycle_last_kana_variant());
        assert_eq!(pending.preedit(), "k");

        let mut without_ring = typed("nn");
        assert!(!without_ring.cycle_last_kana_variant());
        assert!(!JapaneseRomajiScheme::new().cycle_last_kana_variant());
    }
}
