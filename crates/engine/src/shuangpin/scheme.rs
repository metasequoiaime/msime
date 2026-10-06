//! Shuangpin key handling (`R/schemes/shuangpin_scheme.cpp`, schemes-lang.md §1.8). Microsoft accepts `;` as a key when the chunk since the last `'` has odd length.

use super::query::{
    apply_segmentation_cases, effective_input_length, normalize_input, segment_input,
    to_quanpin_segmentation,
};
use super::ShuangpinProfile;
use crate::types::{QueryRequest, SchemeKey, SchemeType, ShuangpinProfileKind};

pub struct ShuangpinScheme {
    profile: &'static ShuangpinProfile,
    raw: String,
}

impl ShuangpinScheme {
    pub fn new(profile: &'static ShuangpinProfile) -> Self {
        Self {
            profile,
            raw: String::new(),
        }
    }

    pub fn reset(&mut self) {
        self.raw.clear();
    }

    pub fn handle_key(&mut self, key: SchemeKey) {
        match key {
            SchemeKey::Backspace => {
                self.raw.pop();
            }
            // A second consecutive `'` is refused, so `''` never forms from keys; a leading one is kept (:66-74).
            SchemeKey::Apostrophe => {
                if !self.raw.ends_with('\'') {
                    self.raw.push('\'');
                }
            }
            SchemeKey::Semicolon => {
                if self.accepts_ing_key() {
                    self.raw.push(';');
                }
            }
            SchemeKey::Letter(letter) if letter.is_ascii_alphabetic() => {
                self.raw.push(char::from(letter));
            }
            SchemeKey::Letter(_) | SchemeKey::Minus | SchemeKey::Symbol(_) | SchemeKey::Requery => {
            }
        }
    }

    /// Microsoft's `ing` key can only be the second key of a syllable, i.e. follow an odd-length chunk (:12-22).
    fn accepts_ing_key(&self) -> bool {
        if self.profile.kind != ShuangpinProfileKind::Microsoft {
            return false;
        }
        let chunk = self.raw.rsplit('\'').next().unwrap_or_default();
        chunk.len() % 2 == 1
    }

    /// :97-121.
    pub fn build_request(&self) -> QueryRequest {
        let raw_input = self.raw.to_ascii_lowercase();
        let mut request = QueryRequest {
            scheme: SchemeType::Shuangpin,
            raw_input_with_cases: self.raw.clone(),
            valid: effective_input_length(&raw_input) > 0,
            ..QueryRequest::default()
        };
        if request.valid {
            let segmentation = segment_input(&raw_input, self.profile);
            request.raw_segmentation =
                apply_segmentation_cases(&segmentation, &request.raw_input_with_cases);
            request.normalized_segmentation = to_quanpin_segmentation(&segmentation, self.profile);
            request.segmentation = request.normalized_segmentation.clone();
            request.normalized_input = normalize_input(&raw_input, self.profile);
        }
        request.raw_input = raw_input;
        request
    }

    pub fn preedit(&self) -> String {
        self.raw.clone()
    }

    pub fn set_raw_input(&mut self, raw: &str, raw_with_cases: &str) {
        self.raw = if raw_with_cases.is_empty() {
            raw
        } else {
            raw_with_cases
        }
        .to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shuangpin::profile::profile;

    fn scheme(kind: ShuangpinProfileKind) -> ShuangpinScheme {
        ShuangpinScheme::new(profile(kind))
    }

    fn type_text(scheme: &mut ShuangpinScheme, text: &str) {
        for byte in text.bytes() {
            scheme.handle_key(match byte {
                b'\'' => SchemeKey::Apostrophe,
                b';' => SchemeKey::Semicolon,
                letter => SchemeKey::Letter(letter),
            });
        }
    }

    #[test]
    fn only_microsoft_takes_semicolon_after_an_odd_chunk() {
        for kind in [
            ShuangpinProfileKind::Xiaohe,
            ShuangpinProfileKind::Ziranma,
            ShuangpinProfileKind::Shoudao,
            ShuangpinProfileKind::Microsoft,
        ] {
            let mut typed = scheme(kind);
            type_text(&mut typed, "b;");
            let expected = if kind == ShuangpinProfileKind::Microsoft {
                "b;"
            } else {
                "b"
            };
            assert_eq!(typed.preedit(), expected, "{kind:?}");
        }
        let mut microsoft = scheme(ShuangpinProfileKind::Microsoft);
        type_text(&mut microsoft, ";ni;");
        assert_eq!(microsoft.preedit(), "ni");
        type_text(&mut microsoft, "'b;");
        assert_eq!(microsoft.preedit(), "ni'b;");
    }

    #[test]
    fn editing_keys() {
        let mut typed = scheme(ShuangpinProfileKind::Xiaohe);
        type_text(&mut typed, "'ni''Hc");
        assert_eq!(typed.preedit(), "'ni'Hc");
        typed.handle_key(SchemeKey::Minus);
        typed.handle_key(SchemeKey::Letter(b'1'));
        typed.handle_key(SchemeKey::Requery);
        typed.handle_key(SchemeKey::Symbol(b';'));
        typed.handle_key(SchemeKey::Symbol(b' '));
        assert_eq!(typed.preedit(), "'ni'Hc");
        typed.handle_key(SchemeKey::Backspace);
        assert_eq!(typed.preedit(), "'ni'H");
        typed.reset();
        assert_eq!(typed.preedit(), "");
        typed.handle_key(SchemeKey::Backspace);
        type_text(&mut typed, "ni");
        typed.reset();
        assert_eq!(typed.preedit(), "");
        typed.set_raw_input("nihc", "NiHc");
        assert_eq!(typed.preedit(), "NiHc");
        typed.set_raw_input("nihc", "");
        assert_eq!(typed.preedit(), "nihc");
    }

    #[test]
    fn requests_carry_both_segmentations() {
        let mut typed = scheme(ShuangpinProfileKind::Xiaohe);
        type_text(&mut typed, "nihcC");
        let request = typed.build_request();
        assert!(request.valid);
        assert_eq!(request.scheme, SchemeType::Shuangpin);
        assert_eq!(request.raw_input, "nihcc");
        assert_eq!(request.raw_input_with_cases, "nihcC");
        assert_eq!(request.raw_segmentation, "ni'hc'C");
        assert_eq!(request.normalized_segmentation, "ni'hao'c");
        assert_eq!(request.segmentation, "ni'hao'c");
        assert_eq!(request.normalized_input, "nihaoc");

        let mut delimiter_only = scheme(ShuangpinProfileKind::Xiaohe);
        delimiter_only.handle_key(SchemeKey::Apostrophe);
        let request = delimiter_only.build_request();
        assert!(!request.valid);
        assert_eq!(request.raw_input, "'");
        assert!(request.raw_segmentation.is_empty());

        let mut microsoft = scheme(ShuangpinProfileKind::Microsoft);
        type_text(&mut microsoft, "n;");
        assert_eq!(microsoft.build_request().normalized_input, "ning");
    }
}
