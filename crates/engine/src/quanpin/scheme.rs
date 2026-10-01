//! Quanpin key handling and request building (`R/schemes/quanpin_scheme.cpp`, quanpin.md §3.5).

use crate::pinyin::active_helpcode::{detect_active_helpcode_length, strip_active_helpcodes};
use crate::pinyin::segment::{cut_pinyin_by_mode, join_segments, CutMode};
use crate::shuangpin::query::{apply_segmentation_cases, remove_manual_delimiters};
use crate::types::{QueryRequest, SchemeKey, SchemeType};

#[derive(Debug, Clone, Default)]
pub struct QuanpinScheme {
    raw: String,
}

impl QuanpinScheme {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.raw.clear();
    }

    /// Letters keep their case; `'` is appended unless the last character already is one; Backspace pops; Escape and Return reset.
    pub fn handle_key(&mut self, key: SchemeKey) {
        match key {
            SchemeKey::Letter(letter) if letter.is_ascii_alphabetic() => {
                self.raw.push(char::from(letter));
            }
            SchemeKey::Apostrophe => {
                if !self.raw.ends_with('\'') {
                    self.raw.push('\'');
                }
            }
            SchemeKey::Backspace => {
                self.raw.pop();
            }
            SchemeKey::Letter(_) | SchemeKey::Semicolon | SchemeKey::Minus | SchemeKey::Requery => {
            }
        }
    }

    /// The correction-mode request (QS:142-155).
    pub fn build_request(&self) -> QueryRequest {
        let mut request = QueryRequest {
            scheme: SchemeType::Quanpin,
            raw_input: self.raw.to_ascii_lowercase(),
            raw_input_with_cases: self.raw.clone(),
            ..QueryRequest::default()
        };
        Self::apply_segmentation(&mut request, CutMode::Correction);
        request
    }

    /// The raw letters, case kept.
    pub fn preedit(&self) -> String {
        self.raw.clone()
    }

    /// Host editing replaces the composition; `raw_with_cases` wins when non-empty.
    pub fn set_raw_input(&mut self, raw: &str, raw_with_cases: &str) {
        self.raw = if raw_with_cases.is_empty() {
            raw
        } else {
            raw_with_cases
        }
        .to_string();
    }

    /// Fill a request's segmentation fields from its raw input with the given cut (QS:82-139): active helpcodes stripped, cases re-applied, trailing `'` and helpcode letters re-appended.
    pub fn apply_segmentation(request: &mut QueryRequest, mode: CutMode) {
        // The helpcode letters are stripped whatever the request's helpcode switch says; the engine gates on the switch when it queries (QE:47-50), and the preedit keeps showing them apart either way.
        let helpcode_length =
            detect_active_helpcode_length(&request.raw_input, &request.raw_input_with_cases);
        let normalized_source =
            strip_active_helpcodes(&request.raw_input, &request.raw_input_with_cases);

        request.normalized_input = remove_manual_delimiters(&normalized_source);
        request.normalized_segmentation = if request.normalized_input.is_empty() {
            String::new()
        } else {
            cut_pinyin_by_mode(&normalized_source, mode)
                .first()
                .map(|segments| join_segments(segments))
                .unwrap_or_default()
        };
        if request.normalized_segmentation.is_empty() {
            request.normalized_segmentation = normalized_source;
        }

        let cased = &request.raw_input_with_cases;
        let cased_source = if helpcode_length > 0 && cased.len() >= helpcode_length {
            &cased[..cased.len() - helpcode_length]
        } else {
            cased.as_str()
        };
        let mut raw_segmentation =
            apply_segmentation_cases(&request.normalized_segmentation, cased_source);
        if cased_source.ends_with('\'') && !raw_segmentation.ends_with('\'') {
            raw_segmentation.push('\'');
        }
        if helpcode_length > 0 {
            raw_segmentation.push('\'');
            raw_segmentation.push_str(&cased[cased.len() - helpcode_length..]);
        }
        request.raw_segmentation = raw_segmentation;
        request.segmentation = request.normalized_segmentation.clone();
        request.valid = !request.normalized_input.is_empty();
    }

    /// Re-cut literally (greedy); what autocorrect suppression does to a request.
    pub fn apply_literal_segmentation(request: &mut QueryRequest) {
        Self::apply_segmentation(request, CutMode::Greedy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> QuanpinScheme {
        let mut scheme = QuanpinScheme::new();
        for byte in text.bytes() {
            scheme.handle_key(if byte == b'\'' {
                SchemeKey::Apostrophe
            } else {
                SchemeKey::Letter(byte)
            });
        }
        scheme
    }

    #[test]
    fn keys_edit_the_raw_letters() {
        let mut scheme = typed("Ni''");
        assert_eq!(scheme.preedit(), "Ni'", "a doubled apostrophe is dropped");
        scheme.handle_key(SchemeKey::Semicolon);
        scheme.handle_key(SchemeKey::Minus);
        scheme.handle_key(SchemeKey::Requery);
        scheme.handle_key(SchemeKey::Letter(b'1'));
        assert_eq!(scheme.preedit(), "Ni'");
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.preedit(), "Ni");
        scheme.reset();
        assert_eq!(scheme.preedit(), "");
        let mut scheme = typed("ab");
        scheme.reset();
        assert_eq!(scheme.preedit(), "");
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.preedit(), "");
    }

    #[test]
    fn host_raw_input_prefers_the_cased_form() {
        let mut scheme = QuanpinScheme::new();
        scheme.set_raw_input("nihao", "NiHao");
        assert_eq!(scheme.preedit(), "NiHao");
        scheme.set_raw_input("nihao", "");
        assert_eq!(scheme.preedit(), "nihao");
    }

    #[test]
    fn request_is_cut_in_correction_mode() {
        let request = typed("nihao").build_request();
        assert_eq!(request.scheme, SchemeType::Quanpin);
        assert_eq!(request.raw_input, "nihao");
        assert_eq!(request.normalized_input, "nihao");
        assert_eq!(request.normalized_segmentation, "ni'hao");
        assert_eq!(request.segmentation, "ni'hao");
        assert_eq!(request.raw_segmentation, "ni'hao");
        assert!(request.valid);

        let aliased = typed("sahng").build_request();
        assert_eq!(aliased.segmentation, "shang", "the alias layer always runs");
        let mut literal = aliased.clone();
        QuanpinScheme::apply_literal_segmentation(&mut literal);
        assert_ne!(literal.segmentation, "shang");
    }

    #[test]
    fn trailing_apostrophe_and_case_survive_in_the_raw_segmentation() {
        let request = typed("NiHao'").build_request();
        assert_eq!(request.raw_input, "nihao'");
        assert_eq!(request.segmentation, "ni'hao");
        assert_eq!(request.raw_segmentation, "Ni'Hao'");
    }

    #[test]
    fn active_helpcode_letters_are_split_off() {
        let request = typed("nihaoAB").build_request();
        assert_eq!(request.normalized_input, "nihao");
        assert_eq!(
            request.normalized_input.capacity(),
            request.normalized_input.len()
        );
        assert_eq!(request.segmentation, "ni'hao");
        assert_eq!(request.raw_segmentation, "ni'hao'AB");
    }

    #[test]
    fn empty_input_is_invalid() {
        let request = typed("'").build_request();
        assert!(!request.valid);
        assert!(!QuanpinScheme::new().build_request().valid);
    }
}
