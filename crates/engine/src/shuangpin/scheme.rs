//! 双拼的按键处理（`R/schemes/shuangpin_scheme.cpp`，schemes-lang.md §1.8）。方案用得到 `;` 键时（微软双拼把 ing 放在 `;` 上，自定义方案可以把韵母或零声母编码的第二个键放在 `;` 上，见 `ShuangpinProfile::uses_semicolon_key`），自上一个 `'` 以来的片段长度为奇数时把 `;` 当作按键接受。

use super::mid_sentence::{
    accepts_semicolon_final_at, has_mid_sentence_helpcode, parse_mid_sentence_helpcodes,
    UppercaseTriggers, DIRECT_HELPCODE_SLASH, END_HELPCODE_MARKER, MID_SENTENCE_MARKER,
};
use super::query::{
    apply_segmentation_cases_into, effective_input_length, segment_input,
    to_quanpin_segmentation_into,
};
use super::ShuangpinProfile;
use crate::types::{QueryRequest, SchemeKey, SchemeType};

pub struct ShuangpinScheme {
    profile: &'static ShuangpinProfile,
    raw: String,
    /// 大写字母开哪种辅助码段（句中或末尾），由会话按触发方式开关设置；`'` 与 `|` 段不受它影响。
    helpcode_uppercase_triggers: UppercaseTriggers,
}

impl ShuangpinScheme {
    pub fn new(profile: &'static ShuangpinProfile) -> Self {
        Self {
            profile,
            raw: String::new(),
            helpcode_uppercase_triggers: UppercaseTriggers::default(),
        }
    }

    /// `ImeSession::configure_shuangpin_scheme` 的对应物：开关层算好归句中还是末尾后下发。
    pub fn set_helpcode_uppercase_triggers(&mut self, triggers: UppercaseTriggers) {
        self.helpcode_uppercase_triggers = triggers;
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
            // 辅助码段标记：收不收由调用方按触发规则判断，这里只负责记下（scheme.cpp:72-92）。
            SchemeKey::Symbol(MID_SENTENCE_MARKER) => {
                self.raw.push(MID_SENTENCE_MARKER as char);
                true
            }
            SchemeKey::Symbol(END_HELPCODE_MARKER) => {
                self.raw.push(END_HELPCODE_MARKER as char);
                true
            }
            SchemeKey::Symbol(DIRECT_HELPCODE_SLASH) => {
                self.raw.push(DIRECT_HELPCODE_SLASH as char);
                true
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

    /// `;` 只能是一个音节的第二个键，即接在奇数长度的片段之后；大写段与末尾段不算这一节的键，
    /// 与 `contracts/mid_sentence_helpcode.h` 的 `AcceptsSemicolonFinalAt` 同一条规则。
    fn accepts_ing_key(&self) -> bool {
        if !self.profile.uses_semicolon_key() {
            return false;
        }
        accepts_semicolon_final_at(
            self.raw.as_bytes(),
            self.raw.len(),
            self.helpcode_uppercase_triggers,
        )
    }

    /// :97-121.
    pub fn build_request(&self) -> QueryRequest {
        let mut request = QueryRequest::default();
        self.build_request_into(&mut request);
        request
    }

    /// 将双拼请求写入已有存储，避免逐键刷新重复构建按键和切分字符串。
    pub fn build_request_into(&self, request: &mut QueryRequest) {
        Self::build_request_from_raw_into(
            self.profile,
            &self.raw,
            &self.raw,
            self.helpcode_uppercase_triggers,
            request,
        );
    }

    /// 从原文直接写入查询请求，光标前缀查询不必创建临时方案。`triggers` 与敲键时的设置一致，
    /// 否则前缀查询与正式查询切出来的大写段不一样。
    pub fn build_request_from_raw_into(
        profile: &'static ShuangpinProfile,
        raw: &str,
        raw_with_cases: &str,
        triggers: UppercaseTriggers,
        request: &mut QueryRequest,
    ) {
        request.scheme = SchemeType::Shuangpin;
        let raw_with_cases = if raw_with_cases.is_empty() {
            raw
        } else {
            raw_with_cases
        };
        // 下游只认 `'` 分隔的双拼：每段（反引号、末尾或大写段）换成分隔符，约束与装饰单独带着，
        // 原串留在 `raw_input_with_helpcode_marks` 给宿主回读和显示（scheme.cpp:124-144）。
        let parse = has_mid_sentence_helpcode(raw_with_cases.as_bytes(), triggers)
            .then(|| parse_mid_sentence_helpcodes(raw_with_cases, profile, triggers));
        let input = parse
            .as_ref()
            .map_or(raw_with_cases, |parsed| parsed.input.as_str());
        request.raw_input.clear();
        request.raw_input.extend(
            input
                .bytes()
                .map(|byte| char::from(byte.to_ascii_lowercase())),
        );
        request.raw_input_with_cases.clear();
        request.raw_input_with_cases.push_str(input);
        match parse {
            Some(parsed) => {
                request.raw_input_with_helpcode_marks.clear();
                request
                    .raw_input_with_helpcode_marks
                    .push_str(raw_with_cases);
                request.syllable_helpcodes = parsed.helpcodes;
                request.end_helpcodes = parsed.end_helpcodes;
                request.helpcode_source_index = parsed.source_index;
                request.helpcode_decorations = parsed.decorations;
            }
            None => {
                request.raw_input_with_helpcode_marks.clear();
                request.syllable_helpcodes.clear();
                request.end_helpcodes.clear();
                request.helpcode_source_index.clear();
                request.helpcode_decorations.clear();
            }
        }
        request.valid = effective_input_length(&request.raw_input) > 0;
        if request.valid {
            let segmentation = segment_input(&request.raw_input, profile);
            apply_segmentation_cases_into(
                &segmentation,
                &request.raw_input_with_cases,
                &mut request.raw_segmentation,
            );
            to_quanpin_segmentation_into(
                &segmentation,
                profile,
                &mut request.normalized_segmentation,
            );
            request
                .segmentation
                .clone_from(&request.normalized_segmentation);
            // 查询键直接使用本轮已经转换的切分，避免再次切分和转全拼。
            request.normalized_input.clear();
            request
                .normalized_input
                .reserve(effective_input_length(&request.normalized_segmentation));
            for part in request.normalized_segmentation.split('\'') {
                request.normalized_input.push_str(part);
            }
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
    fn refreshing_a_request_reuses_the_case_output_buffer() {
        let typed = {
            let mut typed = scheme(ShuangpinProfileKind::Xiaohe);
            type_text(&mut typed, "NiHc");
            typed
        };
        let mut request = typed.build_request();
        let expected = request.clone();
        let pointer = request.raw_segmentation.as_ptr();
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            typed.build_request_into(&mut request);
        });
        assert_eq!(request, expected);
        assert_eq!(request.raw_segmentation, "Ni'Hc");
        assert_eq!(request.raw_segmentation.as_ptr(), pointer);
        assert_eq!(allocations, 2);
    }

    #[test]
    fn request_normalization_reuses_the_converted_segmentation() {
        for (kind, raw, expected, budget) in [
            (ShuangpinProfileKind::Xiaohe, "NiHc", "nihao", 2),
            (ShuangpinProfileKind::Xiaohe, "Ni'Hc", "nihao", 3),
            (ShuangpinProfileKind::Xiaohe, "n", "n", 2),
            (ShuangpinProfileKind::Microsoft, "J;gK", "jinggao", 2),
        ] {
            let mut typed = scheme(kind);
            type_text(&mut typed, raw);
            let mut request = typed.build_request();
            let before = request.clone();
            let pointer = request.normalized_input.as_ptr();
            let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
                typed.build_request_into(&mut request);
            });
            assert_eq!(request, before, "{kind:?}, raw={raw}");
            assert_eq!(request.normalized_input, expected, "{kind:?}, raw={raw}");
            assert_eq!(request.normalized_input.as_ptr(), pointer);
            assert_eq!(allocations, budget, "{kind:?}, raw={raw}");
        }
    }

    #[test]
    fn request_normalization_does_not_allocate_a_temporary_segmentation() {
        let mut typed = scheme(ShuangpinProfileKind::Xiaohe);
        type_text(&mut typed, "NiHc");
        let mut request = typed.build_request();

        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            typed.build_request_into(&mut request);
        });

        assert_eq!(request.normalized_segmentation, "ni'hao");
        assert_eq!(
            allocations, 2,
            "规范切分写入已有缓冲仍申请了 {allocations} 次"
        );
    }

    #[test]
    fn request_normalization_matches_the_standalone_query_path() {
        use crate::shuangpin::custom::{builtin_table, custom_profile};
        use crate::shuangpin::query::normalize_input;

        let mut profiles = [
            ShuangpinProfileKind::Xiaohe,
            ShuangpinProfileKind::Ziranma,
            ShuangpinProfileKind::Shoudao,
            ShuangpinProfileKind::Microsoft,
            ShuangpinProfileKind::SmartAbc,
        ]
        .map(|kind| profile(kind).unwrap())
        .to_vec();
        let mut custom = builtin_table(ShuangpinProfileKind::Xiaohe).unwrap();
        for (initial, key) in &mut custom.initials {
            match initial.as_str() {
                "zh" => *key = "i".to_owned(),
                "ch" => *key = "v".to_owned(),
                _ => {}
            }
        }
        profiles.push(custom_profile(&custom).unwrap());
        profiles.push(
            custom_profile(&builtin_table(ShuangpinProfileKind::Microsoft).unwrap()).unwrap(),
        );

        let mut corpus = vec![String::new()];
        for &first in b"abcdefghijklmnopqrstuvwxyz;'" {
            corpus.push(char::from(first).to_string());
            for &second in b"abcdefghijklmnopqrstuvwxyz;'" {
                corpus.push(String::from_utf8(vec![first, second]).unwrap());
            }
        }
        corpus.extend(
            [
                "NiHc", "Ni'Hc", "NiHcAB", "'ni''hc'", "uj'uv", "UjUV'", "j;gk", "J;'gK", ";;;",
                "ni''''", "ni'hcx", "''''", "AABBCC", "n'h'c",
            ]
            .map(str::to_owned),
        );
        corpus.push("nihc".repeat(128));
        corpus.push("ni'hc'".repeat(128));

        for profile in profiles {
            let mut reused = QueryRequest::default();
            ShuangpinScheme::build_request_from_raw_into(
                profile,
                "nihc",
                "NiHc",
                UppercaseTriggers::default(),
                &mut reused,
            );
            for raw in &corpus {
                let uppercased = raw.to_ascii_uppercase();
                for cased in ["", uppercased.as_str(), "Different"] {
                    let mut fresh = QueryRequest::default();
                    ShuangpinScheme::build_request_from_raw_into(
                        profile,
                        raw,
                        cased,
                        UppercaseTriggers::default(),
                        &mut fresh,
                    );
                    assert_eq!(
                        fresh.normalized_input,
                        normalize_input(&fresh.raw_input, profile),
                        "profile={:?}, raw={raw:?}, cased={cased:?}",
                        profile.kind
                    );
                    ShuangpinScheme::build_request_from_raw_into(
                        profile,
                        raw,
                        cased,
                        UppercaseTriggers::default(),
                        &mut reused,
                    );
                    assert_eq!(
                        reused, fresh,
                        "profile={:?}, raw={raw:?}, cased={cased:?}",
                        profile.kind
                    );
                }
            }
        }
    }

    #[test]
    fn only_microsoft_takes_semicolon_after_an_odd_chunk() {
        for kind in [
            ShuangpinProfileKind::Xiaohe,
            ShuangpinProfileKind::Ziranma,
            ShuangpinProfileKind::Shoudao,
            ShuangpinProfileKind::Microsoft,
            ShuangpinProfileKind::SmartAbc,
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
            ShuangpinProfileKind::SmartAbc,
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
