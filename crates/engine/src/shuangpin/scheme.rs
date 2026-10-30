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
use crate::touch::{KeyGeometry, KeyTouch};
use crate::types::{QueryRequest, SchemeKey, SchemeType};
use std::sync::Arc;

pub struct ShuangpinScheme {
    profile: &'static ShuangpinProfile,
    raw: String,
    /// 大写字母开哪种辅助码段（句中或末尾），由会话按触发方式开关设置；`'` 与 `|` 段不受它影响。
    helpcode_uppercase_triggers: UppercaseTriggers,
    /// 与 `raw` 逐字节对齐的触点：小写字母和 `;` 带上按下时的触点，`'`、大写字母和辅助码标记是 `None`。一个触点都没有时保持为空，不为全 `None` 占位，所以桌面的每次按键不多一次分配。
    touches: Vec<Option<KeyTouch>>,
    /// 下一个入串的按键的触点，由会话在 `handle_key` 之前交来，`handle_key` 用掉或丢掉。
    pending_touch: Option<KeyTouch>,
    /// 触点所在的几何，没有注册几何时为 `None`，请求就不带触点。
    key_geometry: Option<Arc<KeyGeometry>>,
}

impl ShuangpinScheme {
    pub fn new(profile: &'static ShuangpinProfile) -> Self {
        Self {
            profile,
            raw: String::new(),
            helpcode_uppercase_triggers: UppercaseTriggers::default(),
            touches: Vec::new(),
            pending_touch: None,
            key_geometry: None,
        }
    }

    /// 宿主注册或清除的键位几何（会话转交）；布局或代次变化后旧触点失效，原按键串保留。
    pub fn set_key_geometry(&mut self, geometry: Option<Arc<KeyGeometry>>) {
        if self.key_geometry != geometry {
            self.touches.clear();
            self.pending_touch = None;
        }
        self.key_geometry = geometry;
    }

    /// 下一个按键的触点，只对紧接着的那一次 `handle_key` 有效。
    pub fn set_pending_touch(&mut self, touch: Option<KeyTouch>) {
        self.pending_touch = touch;
    }

    /// 记下刚入串的那个字节的触点，保持与 `raw` 对齐。
    fn push_touch(&mut self, touch: Option<KeyTouch>) {
        if self.touches.is_empty() {
            if touch.is_none() {
                return;
            }
            self.touches.resize(self.raw.len() - 1, None);
        }
        self.touches.push(touch);
        debug_assert_eq!(self.touches.len(), self.raw.len());
    }

    /// `ImeSession::configure_shuangpin_scheme` 的对应物：开关层算好归句中还是末尾后下发。
    pub fn set_helpcode_uppercase_triggers(&mut self, triggers: UppercaseTriggers) {
        self.helpcode_uppercase_triggers = triggers;
    }

    pub fn reset(&mut self) {
        self.raw.clear();
        self.touches.clear();
        self.pending_touch = None;
    }

    pub fn handle_key(&mut self, key: SchemeKey) -> bool {
        // 待用触点只属于这一次按键：没入串（被拒的 `;`、第二个 `'`）也丢掉。
        let touch = self.pending_touch.take();
        let pushed = match key {
            SchemeKey::Backspace => {
                if self.raw.pop().is_none() {
                    return false;
                }
                self.touches.truncate(self.raw.len());
                return true;
            }
            // A second consecutive `'` is refused, so `''` never forms from keys; a leading one is kept (:66-74).
            SchemeKey::Apostrophe => {
                if !self.raw.ends_with('\'') {
                    self.raw.push('\'');
                    None
                } else {
                    return false;
                }
            }
            SchemeKey::Semicolon => {
                if self.accepts_ing_key() {
                    self.raw.push(';');
                    touch
                } else {
                    return false;
                }
            }
            // 辅助码段标记：收不收由调用方按触发规则判断，这里只负责记下（scheme.cpp:72-92）。
            SchemeKey::Symbol(MID_SENTENCE_MARKER) => {
                self.raw.push(MID_SENTENCE_MARKER as char);
                None
            }
            SchemeKey::Symbol(END_HELPCODE_MARKER) => {
                self.raw.push(END_HELPCODE_MARKER as char);
                None
            }
            SchemeKey::Symbol(DIRECT_HELPCODE_SLASH) => {
                self.raw.push(DIRECT_HELPCODE_SLASH as char);
                None
            }
            // 大写字母是辅助码，不是音节里的键，不带触点。
            SchemeKey::Letter(letter) if letter.is_ascii_alphabetic() => {
                self.raw.push(char::from(letter));
                touch.filter(|_| letter.is_ascii_lowercase())
            }
            SchemeKey::Letter(_) | SchemeKey::Minus | SchemeKey::Symbol(_) | SchemeKey::Requery => {
                return false;
            }
        };
        self.push_touch(pushed);
        true
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
        // 只有整串都能按位置对上时才带触点：手打的 `'` 让纠错整句不纠，句中辅助码把输入串改写成去段串。其余情况一律是空 Vec，桌面请求逐字节不变。
        request.key_touches.clear();
        request.key_geometry = None;
        let Some(geometry) = &self.key_geometry else {
            return;
        };
        if request.valid
            && self.touches.iter().any(Option::is_some)
            && request.raw_input_with_helpcode_marks.is_empty()
            && request.raw_input.len() == self.touches.len()
            && !self.raw.contains('\'')
        {
            request.key_touches.extend_from_slice(&self.touches);
            request.key_geometry = Some(Arc::clone(geometry));
        }
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
        realign_touches(&mut self.touches, self.raw.as_bytes(), source.as_bytes());
        self.raw.clear();
        self.raw.reserve(source.len());
        self.raw.push_str(source);
    }

    /// 与 `preedit` 逐字节对齐的触点；一个都没有时为空。
    #[cfg(test)]
    pub(crate) fn touches(&self) -> &[Option<KeyTouch>] {
        &self.touches
    }
}

/// 宿主改写了输入串（光标处插入或删除、部分选词吃掉前缀、交出光标前的部分、撤销时补回前缀）：新旧串的公共后缀和公共前缀保留原来的触点，中间新出现的字节没有触点。先认后缀再在剩下的部分里认前缀：吃掉或补回前缀是确定的改动，`xpxp`（谢谢）选走第一个字剩下的 `xp` 必须对上后两键的触点，先认前缀会错拿刚选走那两键的。`hh` 这类相同的相邻字母在光标处删改时分不清是哪一个，触点可能互换，影响很小。
fn realign_touches(touches: &mut Vec<Option<KeyTouch>>, old: &[u8], new: &[u8]) {
    if touches.is_empty() {
        return;
    }
    debug_assert_eq!(touches.len(), old.len());
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take_while(|(left, right)| left == right)
        .count();
    let prefix = old[..old.len() - suffix]
        .iter()
        .zip(&new[..new.len() - suffix])
        .take_while(|(left, right)| left == right)
        .count();
    touches.splice(
        prefix..old.len() - suffix,
        std::iter::repeat_n(None, new.len() - prefix - suffix),
    );
    if touches.iter().all(Option::is_none) {
        touches.clear();
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

    fn touch(dx: i8) -> Option<KeyTouch> {
        Some(KeyTouch { dx, dy: 0, copy: 0 })
    }

    fn geometry() -> Arc<KeyGeometry> {
        let mut keys = [[0.0f32; 4]; 26];
        for (index, key) in keys.iter_mut().enumerate() {
            *key = [index as f32 * 10.0, 0.0, 10.0, 10.0];
        }
        Arc::new(KeyGeometry::new(5, &keys, None, &[]).unwrap())
    }

    /// 按 `text` 逐键打，第 i 个键带 `touches[i]`。
    fn type_touched(scheme: &mut ShuangpinScheme, text: &str, touches: &[Option<KeyTouch>]) {
        for (byte, touch) in text.bytes().zip(touches) {
            scheme.set_pending_touch(*touch);
            scheme.handle_key(match byte {
                b'\'' => SchemeKey::Apostrophe,
                b';' => SchemeKey::Semicolon,
                letter => SchemeKey::Letter(letter),
            });
        }
    }

    #[test]
    fn changed_geometry_discards_old_and_pending_touches() {
        let original = geometry();
        let mut keys = [[0.0f32; 4]; 26];
        for (index, key) in keys.iter_mut().enumerate() {
            *key = [index as f32 * 10.0, 0.0, 10.0, 10.0];
        }
        let next_generation = Arc::new(KeyGeometry::new(6, &keys, None, &[]).unwrap());
        keys[0][0] += 5.0;
        let changed_layout = Arc::new(KeyGeometry::new(5, &keys, None, &[]).unwrap());
        for replacement in [Some(next_generation), Some(changed_layout), None] {
            let mut typed = scheme(ShuangpinProfileKind::Xiaohe);
            typed.set_key_geometry(Some(Arc::clone(&original)));
            type_touched(
                &mut typed,
                "nihc",
                &[touch(1), touch(2), touch(3), touch(4)],
            );
            typed.set_pending_touch(touch(5));
            typed.set_key_geometry(replacement);
            assert_eq!(typed.preedit(), "nihc");
            assert!(typed.build_request().key_touches.is_empty());
            assert!(typed.touches().is_empty());
            typed.handle_key(SchemeKey::Letter(b'm'));
            assert!(typed.touches().is_empty());
            typed.set_key_geometry(Some(Arc::clone(&original)));
            assert!(typed.build_request().key_touches.is_empty());
            type_touched(&mut typed, "a", &[touch(6)]);
            assert_eq!(
                typed.build_request().key_touches,
                [None, None, None, None, None, touch(6)]
            );
        }
    }

    #[test]
    fn repeated_geometry_preserves_recorded_and_pending_touches() {
        let mut typed = scheme(ShuangpinProfileKind::Xiaohe);
        typed.set_key_geometry(Some(geometry()));
        type_touched(&mut typed, "ni", &[touch(1), touch(2)]);
        typed.set_pending_touch(touch(3));
        // 独立分配但值相同的几何也属于重复下发。
        typed.set_key_geometry(Some(geometry()));
        typed.handle_key(SchemeKey::Letter(b'h'));
        assert_eq!(
            typed.build_request().key_touches,
            [touch(1), touch(2), touch(3)]
        );
    }

    #[test]
    fn touches_follow_the_raw_input_key_by_key() {
        let mut typed = scheme(ShuangpinProfileKind::Microsoft);
        typed.set_key_geometry(Some(geometry()));
        // 没有触点的按键不占地方。
        type_text(&mut typed, "ni");
        assert!(typed.touches().is_empty());
        // 第一个触点把前面补成 `None`；`;` 带触点，`'` 和大写字母（辅助码）不带。
        type_touched(
            &mut typed,
            "h;'zA",
            &[touch(1), touch(2), touch(3), touch(4), touch(5)],
        );
        assert_eq!(typed.preedit(), "nih;'zA");
        assert_eq!(
            typed.touches(),
            [None, None, touch(1), touch(2), None, touch(4), None]
        );
        // 退格删掉末尾那个；被拒的键（第二个 `'`）不留下触点，也不把它带给下一键。
        typed.handle_key(SchemeKey::Backspace);
        assert_eq!(
            typed.touches(),
            [None, None, touch(1), touch(2), None, touch(4)]
        );
        typed.handle_key(SchemeKey::Backspace);
        type_touched(&mut typed, "''", &[touch(6), touch(7)]);
        type_text(&mut typed, "c");
        assert_eq!(typed.preedit(), "nih;'c");
        assert_eq!(
            typed.touches(),
            [None, None, touch(1), touch(2), None, None]
        );
        typed.reset();
        assert!(typed.touches().is_empty());
        type_text(&mut typed, "n");
        assert!(typed.touches().is_empty());
    }

    #[test]
    fn host_edits_keep_the_touches_outside_the_edited_span() {
        let mut typed = scheme(ShuangpinProfileKind::Xiaohe);
        type_touched(
            &mut typed,
            "nihcma",
            &[touch(1), touch(2), touch(3), touch(4), touch(5), touch(6)],
        );
        // 光标处插入：插进来的键没有触点，两边的照旧。
        typed.set_raw_input("nihxcma", "");
        assert_eq!(
            typed.touches(),
            [
                touch(1),
                touch(2),
                touch(3),
                None,
                touch(4),
                touch(5),
                touch(6)
            ]
        );
        // 光标处删除。
        typed.set_raw_input("nihcma", "");
        assert_eq!(
            typed.touches(),
            [touch(1), touch(2), touch(3), touch(4), touch(5), touch(6)]
        );
        // 部分选词吃掉前缀，剩下的按后缀对上。
        typed.set_raw_input("ma", "");
        assert_eq!(typed.touches(), [touch(5), touch(6)]);
        // 叠词：选走 `xpxp`（谢谢）的第一个字，剩下的是后两键；撤销补回前缀时，补回的两键没有触点。
        let mut repeated = scheme(ShuangpinProfileKind::Xiaohe);
        type_touched(
            &mut repeated,
            "xpxp",
            &[touch(1), touch(2), touch(3), touch(4)],
        );
        repeated.set_raw_input("xp", "");
        assert_eq!(repeated.touches(), [touch(3), touch(4)]);
        repeated.set_raw_input("xpxp", "");
        assert_eq!(repeated.touches(), [None, None, touch(3), touch(4)]);
        // 换成完全不同的输入：一个触点都不剩，回到空。
        typed.set_raw_input("ui", "");
        assert!(typed.touches().is_empty());
        // 相同的相邻字母分不清删的是哪一个，长度和其余位置仍对得上。
        let mut doubled = scheme(ShuangpinProfileKind::Xiaohe);
        type_touched(&mut doubled, "hhc", &[touch(1), touch(2), touch(3)]);
        doubled.set_raw_input("hc", "");
        assert_eq!(doubled.touches().len(), 2);
        assert_eq!(doubled.touches()[1], touch(3));
    }

    #[test]
    fn requests_carry_touches_only_when_every_position_lines_up() {
        let geometry = geometry();
        let mut typed = scheme(ShuangpinProfileKind::Xiaohe);
        type_touched(&mut typed, "nihc", &[touch(1), None, touch(3), touch(4)]);
        // 没有几何：不带触点。
        let request = typed.build_request();
        assert!(request.key_touches.is_empty());
        assert!(request.key_geometry.is_none());
        typed.set_key_geometry(Some(Arc::clone(&geometry)));
        // 注册前记下的触点没有所属布局，不能配上新几何使用。
        assert!(typed.build_request().key_touches.is_empty());
        typed.reset();
        type_touched(&mut typed, "nihc", &[touch(1), None, touch(3), touch(4)]);
        let request = typed.build_request();
        assert_eq!(request.key_touches, [touch(1), None, touch(3), touch(4)]);
        assert_eq!(request.key_geometry.as_deref(), Some(&*geometry));
        // 复用同一份请求时，退回没有触点也清干净。
        let mut reused = request;
        typed.set_key_geometry(None);
        typed.build_request_into(&mut reused);
        assert!(reused.key_touches.is_empty());
        assert!(reused.key_geometry.is_none());
        typed.set_key_geometry(Some(Arc::clone(&geometry)));
        // 手打的 `'` 不纠，整串不带触点。
        type_text(&mut typed, "'m");
        assert!(typed.build_request().key_touches.is_empty());
        // 一个触点都没有的输入不带。
        let mut plain = scheme(ShuangpinProfileKind::Xiaohe);
        plain.set_key_geometry(Some(Arc::clone(&geometry)));
        type_text(&mut plain, "nihc");
        assert!(plain.build_request().key_touches.is_empty());
        // 句中辅助码把输入改写成去段串，位置对不上，也不带。
        let mut marked = scheme(ShuangpinProfileKind::Xiaohe);
        marked.set_key_geometry(Some(geometry));
        type_touched(
            &mut marked,
            "nihc",
            &[touch(1), touch(2), touch(3), touch(4)],
        );
        marked.handle_key(SchemeKey::Symbol(MID_SENTENCE_MARKER));
        type_text(&mut marked, "ab");
        let request = marked.build_request();
        assert!(!request.raw_input_with_helpcode_marks.is_empty());
        assert!(request.key_touches.is_empty());
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
