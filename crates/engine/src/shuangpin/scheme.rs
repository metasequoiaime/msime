//! 双拼的按键处理（`R/schemes/shuangpin_scheme.cpp`，schemes-lang.md §1.8）。方案用得到 `;` 键时（微软双拼把 ing 放在 `;` 上，自定义方案可以把韵母或零声母编码的第二个键放在 `;` 上，见 `ShuangpinProfile::uses_semicolon_key`），自上一个 `'` 以来的片段长度为奇数时把 `;` 当作按键接受。

use super::query::{
    apply_segmentation_cases, effective_input_length, normalize_input, segment_input,
    to_quanpin_segmentation,
};
use super::ShuangpinProfile;
use crate::types::{QueryRequest, SchemeKey, SchemeType};

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

    pub fn handle_key(&mut self, key: SchemeKey) -> bool {
        match key {
            SchemeKey::Backspace => self.raw.pop().is_some(),
            // A second consecutive `'` is refused, so `''` never forms from keys; a leading one is kept (:66-74).
            SchemeKey::Apostrophe => {
                if !self.raw.ends_with('\'') {
                    self.raw.push('\'');
                    true
                } else {
                    false
                }
            }
            SchemeKey::Semicolon => {
                if self.accepts_ing_key() {
                    self.raw.push(';');
                    true
                } else {
                    false
                }
            }
            SchemeKey::Letter(letter) if letter.is_ascii_alphabetic() => {
                self.raw.push(char::from(letter));
                true
            }
            SchemeKey::Letter(_) | SchemeKey::Minus | SchemeKey::Symbol(_) | SchemeKey::Requery => {
                false
            }
        }
    }

    /// `;` 只能是一个音节的第二个键，即接在奇数长度的片段之后（:12-22）。
    fn accepts_ing_key(&self) -> bool {
        if !self.profile.uses_semicolon_key() {
            return false;
        }
        let chunk = self.raw.rsplit('\'').next().unwrap_or_default();
        chunk.len() % 2 == 1
    }

    /// :97-121.
    pub fn build_request(&self) -> QueryRequest {
        let mut request = QueryRequest::default();
        self.build_request_into(&mut request);
        request
    }

    /// 将双拼请求写入已有存储，避免逐键刷新重复构建按键和切分字符串。
    pub fn build_request_into(&self, request: &mut QueryRequest) {
        Self::build_request_from_raw_into(self.profile, &self.raw, &self.raw, request);
    }

    /// 从原文直接写入查询请求，光标前缀查询不必创建临时方案。
    pub fn build_request_from_raw_into(
        profile: &'static ShuangpinProfile,
        raw: &str,
        raw_with_cases: &str,
        request: &mut QueryRequest,
    ) {
        request.scheme = SchemeType::Shuangpin;
        request.raw_input.clear();
        request.raw_input.extend(
            raw.bytes()
                .map(|byte| char::from(byte.to_ascii_lowercase())),
        );
        let raw_with_cases = if raw_with_cases.is_empty() {
            raw
        } else {
            raw_with_cases
        };
        request.raw_input_with_cases.clear();
        request.raw_input_with_cases.push_str(raw_with_cases);
        request.valid = effective_input_length(&request.raw_input) > 0;
        if request.valid {
            let segmentation = segment_input(&request.raw_input, profile);
            let raw_segmentation =
                apply_segmentation_cases(&segmentation, &request.raw_input_with_cases);
            request.raw_segmentation.clone_from(&raw_segmentation);
            let normalized_segmentation = to_quanpin_segmentation(&segmentation, profile);
            request
                .normalized_segmentation
                .clone_from(&normalized_segmentation);
            request
                .segmentation
                .clone_from(&request.normalized_segmentation);
            let normalized_input = normalize_input(&request.raw_input, profile);
            request.normalized_input.clone_from(&normalized_input);
        } else {
            request.raw_segmentation.clear();
            request.normalized_segmentation.clear();
            request.segmentation.clear();
            request.normalized_input.clear();
        }
    }

    pub fn preedit(&self) -> String {
        self.raw.clone()
    }

    pub fn set_raw_input(&mut self, raw: &str, raw_with_cases: &str) {
        let source = if raw_with_cases.is_empty() {
            raw
        } else {
            raw_with_cases
        };
        self.raw.clear();
        self.raw.reserve(source.len());
        self.raw.push_str(source);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shuangpin::profile::profile;
    use crate::types::ShuangpinProfileKind;

    fn scheme(kind: ShuangpinProfileKind) -> ShuangpinScheme {
        ShuangpinScheme::new(profile(kind).unwrap())
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
    fn set_raw_input_reuses_existing_storage() {
        for kind in [
            ShuangpinProfileKind::Xiaohe,
            ShuangpinProfileKind::Ziranma,
            ShuangpinProfileKind::Shoudao,
            ShuangpinProfileKind::Microsoft,
        ] {
            let mut typed = scheme(kind);
            typed.set_raw_input("ni'hcAB", "Ni'HcAB");
            let pointer = typed.raw.as_ptr();
            let capacity = typed.raw.capacity();

            let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
                typed.set_raw_input("nihc", "NiHc");
                typed.set_raw_input("ni", "");
                typed.set_raw_input("", "");
            });

            assert_eq!(allocations, 0, "{kind:?}");
            assert_eq!(typed.raw.as_ptr(), pointer, "{kind:?}");
            assert_eq!(typed.raw.capacity(), capacity, "{kind:?}");
            assert!(typed.raw.is_empty());
            typed.set_raw_input("nihc", "NiHc");
            assert_eq!(typed.preedit(), "NiHc");
            typed.set_raw_input("ni", "");
            assert_eq!(typed.preedit(), "ni");
        }
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
