//! Quanpin key handling and request building (`R/schemes/quanpin_scheme.cpp`, quanpin.md §3.5).

use crate::pinyin::active_helpcode::{detect_active_helpcode_length, strip_active_helpcodes};
use crate::pinyin::segment::{cut_pinyin_by_mode, join_segments, CutMode};
use crate::shuangpin::query::{apply_segmentation_cases_into, remove_manual_delimiters};
use crate::types::{QueryRequest, SchemeKey, SchemeType};

fn replace_string(target: &mut String, source: String) {
    if target.capacity() == 0 {
        *target = source;
    } else {
        target.clone_from(&source);
    }
}

/// 全拼部件辅助段的引导键，记进输入串（#11804）。
pub(crate) const COMPONENT_FILTER_MARKER: u8 = b'`';

/// 把输入串在第一个反引号处分成拼音和部件辅助段（不含反引号）；没有反引号时为 `None`。
pub(crate) fn split_component_filter(raw: &str) -> Option<(&str, &str)> {
    raw.split_once(char::from(COMPONENT_FILTER_MARKER))
}

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
    pub fn handle_key(&mut self, key: SchemeKey) -> bool {
        match key {
            SchemeKey::Letter(letter) if letter.is_ascii_alphabetic() => {
                self.raw.push(char::from(letter));
                true
            }
            SchemeKey::Apostrophe => {
                if !self.raw.ends_with('\'') {
                    self.raw.push('\'');
                    true
                } else {
                    false
                }
            }
            SchemeKey::Backspace => self.raw.pop().is_some(),
            // 部件辅助段的反引号：会话只在全拼完整、光标在末尾、选项打开时送来（`InputSession::component_filter_backtick_ready`）；一段组字里只有一个。
            SchemeKey::Symbol(COMPONENT_FILTER_MARKER)
                if !self.raw.is_empty()
                    && !self.raw.contains(char::from(COMPONENT_FILTER_MARKER)) =>
            {
                self.raw.push(char::from(COMPONENT_FILTER_MARKER));
                true
            }
            SchemeKey::Letter(_)
            | SchemeKey::Semicolon
            | SchemeKey::Minus
            | SchemeKey::Symbol(_)
            | SchemeKey::Requery => false,
        }
    }

    /// The correction-mode request (QS:142-155).
    pub fn build_request(&self) -> QueryRequest {
        let mut request = QueryRequest::default();
        self.build_request_into(&mut request);
        request
    }

    /// 将全拼请求写入已有存储，避免逐键刷新重复分配按键和切分字符串。
    pub fn build_request_into(&self, request: &mut QueryRequest) {
        Self::build_request_from_raw_into(&self.raw, &self.raw, request);
    }

    /// 从原文直接写入查询请求，光标前缀查询不必创建临时方案。
    pub fn build_request_from_raw_into(
        raw: &str,
        raw_with_cases: &str,
        request: &mut QueryRequest,
    ) {
        request.scheme = SchemeType::Quanpin;
        let raw_with_cases = if raw_with_cases.is_empty() {
            raw
        } else {
            raw_with_cases
        };
        // 部件辅助段从反引号起到末尾：查询、切分、学词和云候选只看反引号前的拼音，原串留给预编辑和编辑光标。
        let (pinyin, pinyin_with_cases) = match split_component_filter(raw_with_cases) {
            Some((pinyin_with_cases, parts)) => {
                request.raw_input_with_helpcode_marks.clear();
                request
                    .raw_input_with_helpcode_marks
                    .push_str(raw_with_cases);
                request.component_filter_parts.clear();
                request.component_filter_parts.extend(
                    parts
                        .bytes()
                        .map(|byte| char::from(byte.to_ascii_lowercase())),
                );
                (
                    split_component_filter(raw).map_or(raw, |(pinyin, _)| pinyin),
                    pinyin_with_cases,
                )
            }
            None => {
                request.raw_input_with_helpcode_marks.clear();
                request.component_filter_parts.clear();
                (raw, raw_with_cases)
            }
        };
        request.raw_input.clear();
        request.raw_input.extend(
            pinyin
                .bytes()
                .map(|byte| char::from(byte.to_ascii_lowercase())),
        );
        request.raw_input_with_cases.clear();
        request.raw_input_with_cases.push_str(pinyin_with_cases);
        Self::apply_segmentation(request, CutMode::Correction);
    }

    /// The raw letters, case kept.
    pub fn preedit(&self) -> String {
        self.raw.clone()
    }

    /// Host editing replaces the composition; `raw_with_cases` wins when non-empty.
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

    /// Fill a request's segmentation fields from its raw input with the given cut (QS:82-139): active helpcodes stripped, cases re-applied, trailing `'` and helpcode letters re-appended.
    pub fn apply_segmentation(request: &mut QueryRequest, mode: CutMode) {
        // 关闭辅助码时，大写字母仍属于拼音原文，不能从切分中剥离。
        let helpcode_length = if request.enable_quanpin_helpcode {
            detect_active_helpcode_length(&request.raw_input, &request.raw_input_with_cases)
        } else {
            0
        };
        let normalized_source = if helpcode_length > 0 {
            strip_active_helpcodes(&request.raw_input, &request.raw_input_with_cases)
        } else {
            request.raw_input.clone()
        };

        let normalized_input = remove_manual_delimiters(&normalized_source);
        let normalized_input_is_empty = normalized_input.is_empty();
        replace_string(&mut request.normalized_input, normalized_input);
        let normalized_segmentation = if normalized_input_is_empty {
            String::new()
        } else {
            cut_pinyin_by_mode(&normalized_source, mode)
                .first()
                .map(|segments| join_segments(segments))
                .unwrap_or_default()
        };
        if normalized_segmentation.is_empty() {
            replace_string(&mut request.normalized_segmentation, normalized_source);
        } else {
            replace_string(
                &mut request.normalized_segmentation,
                normalized_segmentation,
            );
        }

        let cased = &request.raw_input_with_cases;
        let cased_source = if helpcode_length > 0 && cased.len() >= helpcode_length {
            &cased[..cased.len() - helpcode_length]
        } else {
            cased.as_str()
        };
        apply_segmentation_cases_into(
            &request.normalized_segmentation,
            cased_source,
            &mut request.raw_segmentation,
        );
        let raw_segmentation = &mut request.raw_segmentation;
        if cased_source.ends_with('\'') && !raw_segmentation.ends_with('\'') {
            raw_segmentation.push('\'');
        }
        if helpcode_length > 0 {
            raw_segmentation.push('\'');
            raw_segmentation.push_str(&cased[cased.len() - helpcode_length..]);
        }
        // 部件辅助段原样接在切分后面显示，和大写辅助码一样不参与切分。
        if let Some((_, parts)) = split_component_filter(&request.raw_input_with_helpcode_marks) {
            raw_segmentation.push(char::from(COMPONENT_FILTER_MARKER));
            raw_segmentation.push_str(parts);
        }
        request
            .segmentation
            .clone_from(&request.normalized_segmentation);
        request.valid = !normalized_input_is_empty;
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
    fn refreshing_a_request_reuses_the_case_output_buffer() {
        let scheme = typed("nihaoAB");
        let mut request = QueryRequest {
            enable_quanpin_helpcode: true,
            ..QueryRequest::default()
        };
        scheme.build_request_into(&mut request);
        let expected = request.clone();
        let pointer = request.raw_segmentation.as_ptr();
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            scheme.build_request_into(&mut request);
        });
        assert_eq!(request, expected);
        assert_eq!(request.raw_segmentation, "ni'hao'AB");
        assert_eq!(request.raw_segmentation.as_ptr(), pointer);
        assert_eq!(allocations, 16);
    }

    #[test]
    fn keys_edit_the_raw_letters() {
        let mut scheme = typed("Ni''");
        assert_eq!(scheme.preedit(), "Ni'", "a doubled apostrophe is dropped");
        scheme.handle_key(SchemeKey::Semicolon);
        scheme.handle_key(SchemeKey::Minus);
        scheme.handle_key(SchemeKey::Requery);
        scheme.handle_key(SchemeKey::Letter(b'1'));
        scheme.handle_key(SchemeKey::Symbol(b'-'));
        scheme.handle_key(SchemeKey::Symbol(b' '));
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
    fn host_raw_input_reuses_existing_storage() {
        let mut scheme = QuanpinScheme::new();
        scheme.set_raw_input("abcdefghijklmnopqrstuvwxyz", "");
        let capacity = scheme.raw.capacity();

        scheme.set_raw_input("ni", "");

        assert_eq!(scheme.preedit(), "ni");
        assert!(scheme.raw.capacity() >= capacity);
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
        let mut request = QueryRequest {
            enable_quanpin_helpcode: true,
            ..QueryRequest::default()
        };
        typed("nihaoAB").build_request_into(&mut request);
        assert_eq!(request.normalized_input, "nihao");
        assert_eq!(
            request.normalized_input.capacity(),
            request.normalized_input.len()
        );
        assert_eq!(request.segmentation, "ni'hao");
        assert_eq!(request.raw_segmentation, "ni'hao'AB");
    }

    #[test]
    fn disabled_helpcode_keeps_uppercase_letters_in_pinyin_segmentation() {
        let request = typed("maO").build_request();
        assert_eq!(request.raw_input, "mao");
        assert_eq!(request.normalized_input, "mao");
        assert_eq!(request.raw_segmentation, "maO");
    }

    #[test]
    fn empty_input_is_invalid() {
        let request = typed("'").build_request();
        assert!(!request.valid);
        assert!(!QuanpinScheme::new().build_request().valid);
    }

    #[test]
    fn the_component_segment_stays_out_of_the_request_spelling() {
        let mut scheme = typed("cheng");
        assert!(scheme.handle_key(SchemeKey::Symbol(COMPONENT_FILTER_MARKER)));
        // 一段组字只有一个反引号。
        assert!(!scheme.handle_key(SchemeKey::Symbol(COMPONENT_FILTER_MARKER)));
        for letter in *b"chong" {
            assert!(scheme.handle_key(SchemeKey::Letter(letter)));
        }
        let request = scheme.build_request();
        assert_eq!(scheme.preedit(), "cheng`chong");
        assert_eq!(request.raw_input, "cheng");
        assert_eq!(request.raw_input_with_cases, "cheng");
        assert_eq!(request.normalized_input, "cheng");
        assert_eq!(request.segmentation, "cheng");
        assert_eq!(request.raw_input_with_helpcode_marks, "cheng`chong");
        assert_eq!(request.component_filter_parts, "chong");
        assert_eq!(request.raw_segmentation, "cheng`chong");
        assert!(request.valid);

        // 反引号刚打下时段是空的；没有反引号的请求把上一次的段清掉。
        let mut reused = request.clone();
        QuanpinScheme::build_request_from_raw_into("lu`", "lu`", &mut reused);
        assert_eq!(reused.raw_input, "lu");
        assert_eq!(reused.component_filter_parts, "");
        assert_eq!(reused.raw_input_with_helpcode_marks, "lu`");
        QuanpinScheme::build_request_from_raw_into("lu", "lu", &mut reused);
        assert!(reused.raw_input_with_helpcode_marks.is_empty());
        assert!(reused.component_filter_parts.is_empty());

        // 空的组字不收反引号。
        assert!(!QuanpinScheme::new().handle_key(SchemeKey::Symbol(COMPONENT_FILTER_MARKER)));
    }
}
