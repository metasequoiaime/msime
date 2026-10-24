//! Selection advancement, segmentation getters and preedit display (core-session.md §5.8, §5.10-§5.11), including the wubi mixed routing rules that key advancement on the selected row's producer (overlays.md §3.3).

use std::borrow::Cow;

use super::input::{CreatingWordProgress, InputSession};
use crate::helpcode::compute_helpcodes;
use crate::japanese::romaji::is_romaji_complete;
use crate::pinyin::active_helpcode::{
    detect_active_helpcode_length, strip_active_helpcodes, strip_active_helpcodes_with_cases,
};
use crate::pinyin::autocorrect::{
    autocorrect_cut_detail, looks_like_syllable_with_jianpin_tail, AutocorrectCutSegment,
};
use crate::pinyin::segment::is_complete_pinyin_input;
use crate::shuangpin::query::{
    detect_active_double_helpcode_length, effective_input_length, is_complete_input,
    raw_length_for_effective_prefix, remove_manual_delimiters,
};
use crate::shuangpin::ShuangpinProfile;
use crate::text::count_han_chars;
use crate::types::{
    request_autocorrect_mask, CandidateSource, LocalInputMode, QueryRequest, SchemeKey, SchemeType,
    WordItem,
};
use crate::wubi::scheme::PINYIN_LOOKUP_PREFIX;

/// What a selection did to the composition.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct SelectionTransition {
    pub continues_composition: bool,
    pub full_pure_pinyin: String,
    pub current_segmentation_with_cases: String,
    pub selected_canonical_pinyin: String,
    pub wubi_native: bool,
}

/// A shuangpin composition split into the pinyin keys and a trailing helpcode (input_session_composition.cpp:111-163).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct ShuangpinCompositionBase<'a> {
    pub raw_input: Cow<'a, str>,
    pub raw_input_with_cases: Cow<'a, str>,
    /// Manual delimiters removed.
    pub effective_raw_input: Cow<'a, str>,
    pub effective_raw_input_with_cases: Cow<'a, str>,
    pub helpcode_length: usize,
}

pub(super) fn resolve_shuangpin_composition_base<'a>(
    request: &'a QueryRequest,
    profile: &ShuangpinProfile,
) -> ShuangpinCompositionBase<'a> {
    let raw_input = Cow::Borrowed(request.raw_input.as_str());
    let raw_input_with_cases = if request.raw_input_with_cases.is_empty() {
        Cow::Borrowed(request.raw_input.as_str())
    } else {
        Cow::Borrowed(request.raw_input_with_cases.as_str())
    };
    let effective_raw_input = remove_manual_delimiters_cow(request.raw_input.as_str());
    let raw_input_with_cases_source = if request.raw_input_with_cases.is_empty() {
        request.raw_input.as_str()
    } else {
        request.raw_input_with_cases.as_str()
    };
    let effective_raw_input_with_cases = remove_manual_delimiters_cow(raw_input_with_cases_source);
    let mut base = ShuangpinCompositionBase {
        effective_raw_input,
        effective_raw_input_with_cases,
        raw_input,
        raw_input_with_cases,
        helpcode_length: 0,
    };
    if !request.enable_shuangpin_helpcode
        || !request.shuangpin_helpcode_modes.direct
        || base.effective_raw_input.is_empty()
    {
        return base;
    }
    if detect_active_double_helpcode_length(&base.raw_input, &base.raw_input_with_cases, profile)
        == 2
    {
        base.helpcode_length = 2;
        return base;
    }
    let length = base.effective_raw_input.len();
    if length % 2 == 1 && length > 1 {
        let raw_prefix_length = raw_length_for_effective_prefix(&base.raw_input, length - 1);
        // An apostrophe right before the odd letter makes it a pinyin segment the user started, not an auxiliary code.
        let separated = base.raw_input.as_bytes().get(raw_prefix_length) == Some(&b'\'');
        if !separated && is_complete_input(&base.raw_input[..raw_prefix_length], profile) {
            base.helpcode_length = 1;
        }
    }
    base
}

fn remove_manual_delimiters_cow(raw: &str) -> Cow<'_, str> {
    if raw.contains('\'') {
        Cow::Owned(remove_manual_delimiters(raw))
    } else {
        Cow::Borrowed(raw)
    }
}

fn remove_delimiters(segmented: &str) -> String {
    let mut result = String::with_capacity(segmented.len());
    result.extend(segmented.chars().filter(|c| *c != '\''));
    result
}

fn with_trailing_separator<'a>(preedit: Cow<'a, str>, request: &QueryRequest) -> Cow<'a, str> {
    if request.raw_input_with_cases.ends_with('\'') && !preedit.ends_with('\'') {
        let mut preedit = preedit.into_owned();
        preedit.push('\'');
        Cow::Owned(preedit)
    } else {
        preedit
    }
}

/// A consumed prefix can leave the remainder starting with the separator that followed it.
fn remove_consumed_leading_separators(raw: &str) -> &str {
    raw.trim_start_matches('\'')
}

/// Detect an active shuangpin helpcode without constructing the owned composition base. The
/// candidate refresh path only needs this length to decide whether dynamic rows can move.
fn active_shuangpin_helpcode_length(request: &QueryRequest, profile: &ShuangpinProfile) -> usize {
    if !request.enable_shuangpin_helpcode || !request.shuangpin_helpcode_modes.direct {
        return 0;
    }
    let raw = &request.raw_input;
    let raw_with_cases = if request.raw_input_with_cases.is_empty() {
        raw
    } else {
        &request.raw_input_with_cases
    };
    if effective_input_length(raw) == 0 {
        return 0;
    }
    if detect_active_double_helpcode_length(raw, raw_with_cases, profile) == 2 {
        return 2;
    }
    let length = effective_input_length(raw);
    if length % 2 == 1 && length > 1 {
        let raw_prefix_length = raw_length_for_effective_prefix(raw, length - 1);
        let separated = raw.as_bytes().get(raw_prefix_length) == Some(&b'\'');
        if !separated && is_complete_input(&raw[..raw_prefix_length], profile) {
            return 1;
        }
    }
    0
}

/// 所选词的规范读音：撇号段数等于字符数，且每段可完整切分（input_session_composition.cpp:76-96）。
pub(super) fn normalize_canonical_pinyin_for_word(pinyin: &str, word: &str) -> String {
    if canonical_pinyin_is_valid_for_word(pinyin, word) {
        pinyin.to_owned()
    } else {
        String::new()
    }
}

// 只读调用直接复用校验结论；需要保存读音的调用仍由拥有型接口复制结果。
fn canonical_pinyin_is_valid_for_word(pinyin: &str, word: &str) -> bool {
    if pinyin.is_empty() {
        return false;
    }
    let segment_count = pinyin.bytes().filter(|&byte| byte == b'\'').count() + 1;
    if segment_count != count_han_chars(word) {
        return false;
    }
    !pinyin
        .split('\'')
        .any(|segment| segment.is_empty() || !is_complete_pinyin_input(segment))
}

/// An unknown reading anywhere makes the whole phrase unstorable, so an empty suffix empties the result.
pub(super) fn append_canonical_pinyin(prefix: &str, suffix: &str) -> String {
    if prefix.is_empty() {
        return suffix.to_owned();
    }
    if suffix.is_empty() {
        return String::new();
    }
    let mut result = String::with_capacity(prefix.len() + 1 + suffix.len());
    result.push_str(prefix);
    result.push('\'');
    result.push_str(suffix);
    result
}

/// Lowercased letters without delimiters, with the `v` spelling of ü folded onto `u`, so that length-preserving aliases (jv -> ju, nue -> nve) count as explained by the cut. It only compares two derived strings; the dictionary's `corrected_from` treats v and u as distinct on purpose.
pub(super) fn fold_autocorrect_letters(text: &str) -> String {
    text.bytes()
        .filter(|byte| *byte != b'\'')
        .map(|byte| match byte.to_ascii_lowercase() {
            b'v' => 'u',
            lower => lower as char,
        })
        .collect()
}

fn folded_autocorrect_byte(byte: u8) -> Option<u8> {
    if byte == b'\'' {
        None
    } else {
        Some(match byte.to_ascii_lowercase() {
            b'v' => b'u',
            lower => lower,
        })
    }
}

fn folded_letters_equal(left: &str, right: &str) -> bool {
    left.bytes()
        .filter_map(folded_autocorrect_byte)
        .eq(right.bytes().filter_map(folded_autocorrect_byte))
}

fn folded_segments_equal(segments: &[AutocorrectCutSegment], text: &str) -> bool {
    segments
        .iter()
        .flat_map(|segment| segment.syllable.bytes().filter_map(folded_autocorrect_byte))
        .eq(text.bytes().filter_map(folded_autocorrect_byte))
}

/// The preedit must always show the letters the user typed. Two layers can rewrite them into canonical pinyin: the scheme's alias table (sahng -> shang, baked into raw_segmentation) and the dictionary's correction search (shabg -> shang, which only re-separates). Both are redrawn here from the raw letters with separators at the cut positions; when the search cannot explain a rewrite (length-changing aliases such as mihng -> ming) the raw letters are shown without separators (input_session_composition.cpp:286-333).
pub(super) fn build_quanpin_autocorrect_display<'a>(request: &'a QueryRequest) -> Cow<'a, str> {
    let cased = if request.raw_input_with_cases.is_empty() {
        &request.raw_input
    } else {
        &request.raw_input_with_cases
    };
    let base = if request.raw_segmentation.is_empty() {
        cased
    } else {
        &request.raw_segmentation
    };
    if request.raw_input.is_empty() || cased.is_empty() {
        return Cow::Borrowed(base);
    }
    let types = request_autocorrect_mask(
        request.enable_quanpin_autocorrect_transposition,
        request.enable_quanpin_autocorrect_neighbor,
    );
    let letters_rewritten = !folded_letters_equal(base, cased);
    // The scheme kept the typed letters and no correction can apply, so there are no other separators to draw.
    if !letters_rewritten && (types == 0 || is_complete_pinyin_input(&request.raw_input)) {
        return Cow::Borrowed(base);
    }
    // A legal syllable plus one trailing letter is jianpin, never a typo, as in the dictionary's own guard; without this the deletion table would re-separate `zheg`.
    if looks_like_syllable_with_jianpin_tail(&request.raw_input) {
        return Cow::Borrowed(base);
    }
    let folded_input = fold_autocorrect_letters(cased);
    let cut = autocorrect_cut_detail(&folded_input, types).filter(|cut| !cut.is_empty());
    if let Some(cut) = cut {
        // When the scheme rewrote the letters, the query went through the alias reading, so separators may only come from the cut when both layers read the letters the same way (sahnghao -> shang'hao).
        if !letters_rewritten || folded_segments_equal(&cut.segments, base) {
            let boundary_count = cut.segments.len() - 1;
            let mut display = String::with_capacity(cased.len() + cut.segments.len());
            let mut letter_index = 0;
            let mut boundary_index = 0;
            for byte in cased.bytes().filter(|byte| *byte != b'\'') {
                display.push(byte as char);
                letter_index += 1;
                if boundary_index < boundary_count {
                    let segment = &cut.segments[boundary_index];
                    if letter_index == segment.start + segment.raw_text.len() {
                        display.push('\'');
                        boundary_index += 1;
                    }
                }
            }
            return Cow::Owned(display);
        }
    }
    if letters_rewritten {
        Cow::Owned(remove_delimiters(cased))
    } else {
        Cow::Borrowed(base)
    }
}

impl InputSession {
    /// 直接辅助码行的 `pinyin` 包含音节内辅助码，长度超过规范读音的两键编码。
    fn direct_shuangpin_consumed(&self, selected: &WordItem) -> Option<usize> {
        let request = self.engine.request();
        let consumed = selected.pinyin.len();
        (request.enable_shuangpin_helpcode
            && selected.corrected_from.is_empty()
            && !selected.canonical_pinyin.is_empty()
            && !selected.pinyin.contains('\'')
            && consumed > selected.canonical_pinyin.split('\'').count() * 2
            && request.raw_input.starts_with(&selected.pinyin))
        .then_some(consumed)
    }

    /// 纠错候选的读音可能比实际按键短或长；`corrected_from` 记录它实际回答的原文长度。
    fn selected_quanpin_length(selected: &WordItem) -> usize {
        if selected.corrected_from.is_empty() {
            remove_delimiters(&selected.pinyin).len()
        } else {
            selected.corrected_from.len()
        }
    }

    /// input_session_composition.cpp:714-748, with the selected row's scheme deciding the native-wubi branch.
    pub(super) fn selection_completes_composition(&self, selected: &WordItem) -> bool {
        if let Some(consumed) = self.literal_candidate_consumed(selected) {
            return consumed == self.raw_with_cases().len();
        }
        // Japanese, Korean and native wubi selections always finish: their advancement never continues.
        if self.engine.current_scheme_type().selection_completes()
            || selected.scheme == SchemeType::Wubi
        {
            return true;
        }
        let request = self.engine.request();
        if self.is_shuangpin() {
            if let Some(consumed) = self.direct_shuangpin_consumed(selected) {
                return consumed >= request.raw_input.len();
            }
            let base = resolve_shuangpin_composition_base(request, self.shuangpin_profile());
            let word_length = count_han_chars(&selected.word) * 2;
            let total = base.effective_raw_input.len();
            if base.helpcode_length > 0 {
                let required = word_length + base.helpcode_length;
                return !(required < total && word_length < total);
            }
            // With no helpcode the pure pinyin is the whole effective input.
            let mut consumed = remove_delimiters(&selected.pinyin).len();
            if consumed == 0 || consumed > total {
                consumed = word_length.min(total);
            }
            return consumed >= total;
        }
        let selected_length = Self::selected_quanpin_length(selected);
        // 五笔临时拼音反查的引导键 `z` 不是拼音，选词只在它后面那段拼音里计算。
        let (raw, raw_with_cases) = self.pinyin_spelling(request);
        let (raw_without_helpcodes, cased_without_helpcodes) = if request.enable_quanpin_helpcode {
            (
                strip_active_helpcodes(raw, raw_with_cases),
                strip_active_helpcodes_with_cases(raw, raw_with_cases),
            )
        } else {
            (raw.to_owned(), raw_with_cases.to_owned())
        };
        let consumed_raw =
            raw_length_for_effective_prefix(&cased_without_helpcodes, selected_length);
        let normalized_length = request.normalized_input.len() - self.pinyin_lookup_prefix_len();
        selected.pinyin.is_empty()
            || selected_length >= normalized_length
            || consumed_raw >= raw_without_helpcodes.len()
    }

    /// 五笔临时拼音反查时引导键 `z` 占的字节数，其余时候为 0。
    pub(super) fn pinyin_lookup_prefix_len(&self) -> usize {
        usize::from(self.engine.wubi_pinyin_lookup_active())
    }

    /// 请求里真正是拼音的那段原文（不带大小写的和带大小写的）：五笔临时拼音反查时去掉引导键 `z`，其余时候是整段原文。
    fn pinyin_spelling<'a>(&self, request: &'a QueryRequest) -> (&'a str, &'a str) {
        let prefix = self.pinyin_lookup_prefix_len();
        let raw = request.raw_input.get(prefix..).unwrap_or_default();
        let raw_with_cases = request
            .raw_input_with_cases
            .get(prefix..)
            .filter(|cases| !cases.is_empty())
            .unwrap_or(raw);
        (raw, raw_with_cases)
    }

    /// input_session_composition.cpp:799-911.
    pub(super) fn advance_composition_after_selection(
        &mut self,
        selected: &WordItem,
    ) -> SelectionTransition {
        let mut transition = SelectionTransition {
            selected_canonical_pinyin: selected.canonical_pinyin.clone(),
            wubi_native: selected.scheme == SchemeType::Wubi,
            ..SelectionTransition::default()
        };
        if self.is_japanese() {
            let request = self.engine.request();
            transition.full_pure_pinyin = request.raw_input.clone();
            transition.current_segmentation_with_cases = request.raw_input_with_cases.clone();
            return transition;
        }
        if transition.wubi_native {
            let request = self.engine.request();
            transition.full_pure_pinyin = request.normalized_input.clone();
            transition.current_segmentation_with_cases = request.raw_input.clone();
            return transition;
        }
        transition.continues_composition = !self.selection_completes_composition(selected);
        if self.is_shuangpin() {
            let (full_pure_pinyin, rest) = {
                let request = self.engine.request();
                let base = resolve_shuangpin_composition_base(request, self.shuangpin_profile());
                let word_length = count_han_chars(&selected.word) * 2;
                let total = base.effective_raw_input.len();
                let full_pure_pinyin = if base.helpcode_length > 0 && total >= base.helpcode_length
                {
                    base.effective_raw_input[..total - base.helpcode_length].to_owned()
                } else {
                    base.effective_raw_input.clone().into_owned()
                };
                let rest = if transition.continues_composition {
                    let (start, end) = if let Some(consumed) =
                        self.direct_shuangpin_consumed(selected)
                    {
                        // 内嵌辅助码跟随选中字一起消耗，剩余音节保留原始大小写。
                        (consumed, base.raw_input.len())
                    } else if base.helpcode_length > 0 {
                        // 辅助码选中了这个词，剩余输入不再保留该辅助码。
                        (
                            raw_length_for_effective_prefix(
                                &base.raw_input_with_cases,
                                word_length,
                            ),
                            raw_length_for_effective_prefix(
                                &base.raw_input_with_cases,
                                total - base.helpcode_length,
                            ),
                        )
                    } else {
                        let mut consumed = remove_delimiters(&selected.pinyin).len();
                        if consumed == 0 || consumed > total {
                            consumed = word_length.min(total);
                        }
                        (
                            raw_length_for_effective_prefix(&base.raw_input_with_cases, consumed),
                            base.raw_input.len(),
                        )
                    };
                    Some(if request.raw_input_with_helpcode_marks.is_empty() {
                        (
                            remove_consumed_leading_separators(&base.raw_input[start..end])
                                .to_owned(),
                            remove_consumed_leading_separators(
                                &base.raw_input_with_cases[start..end],
                            )
                            .to_owned(),
                        )
                    } else {
                        // 带辅助码段时按 source_index 把去段串的剩余范围映回原串
                        // （`RemapMidSentenceRest`）：段随已消耗的音节一起走。
                        let marks = request.raw_input_with_helpcode_marks.as_str();
                        let map = &request.helpcode_source_index;
                        let skip = base.raw_input_with_cases[start..end]
                            .bytes()
                            .take_while(|&byte| byte == b'\'')
                            .count();
                        let full_begin = map.get(start + skip).copied().unwrap_or(marks.len());
                        let full_end = map.get(end).copied().unwrap_or(marks.len());
                        let rest = &marks[full_begin..full_end];
                        (rest.to_lowercase(), rest.to_owned())
                    })
                } else {
                    None
                };
                (full_pure_pinyin, rest)
            };
            transition.full_pure_pinyin = full_pure_pinyin;
            if let Some((rest, rest_with_cases)) = rest {
                self.engine
                    .replace_active_raw_input(&rest, &rest_with_cases);
                self.online_requests.invalidate();
                self.update_mixed_candidates();
            }
            transition.current_segmentation_with_cases =
                self.pinyin_segmentation_with_cases().into_owned();
            return transition;
        }

        // Quanpin, and the quanpin rows of a wubi composition, which shorten the wubi code the same way.
        if transition.continues_composition {
            let lookup_prefix = self.pinyin_lookup_prefix_len();
            let (full_pure_pinyin, raw_without_helpcodes, cased_without_helpcodes) = {
                let request = self.engine.request();
                let (raw, raw_with_cases) = self.pinyin_spelling(request);
                if request.enable_quanpin_helpcode {
                    (
                        request.normalized_input[lookup_prefix..].to_owned(),
                        strip_active_helpcodes(raw, raw_with_cases),
                        strip_active_helpcodes_with_cases(raw, raw_with_cases),
                    )
                } else {
                    (
                        request.normalized_input[lookup_prefix..].to_owned(),
                        raw.to_owned(),
                        raw_with_cases.to_owned(),
                    )
                }
            };
            transition.full_pure_pinyin = full_pure_pinyin;
            let consumed = raw_length_for_effective_prefix(
                &cased_without_helpcodes,
                Self::selected_quanpin_length(selected),
            );
            let rest = remove_consumed_leading_separators(&raw_without_helpcodes[consumed..]);
            let rest_with_cases =
                remove_consumed_leading_separators(&cased_without_helpcodes[consumed..]);
            if self.is_wubi() {
                // The rest of a spelling the user is still in the middle of stays pinyin; the wubi table answering it would swap schemes underneath them.
                self.engine.keep_pinyin_tail();
            }
            if lookup_prefix > 0 {
                // 反查里选走一段之后，剩下的拼音仍在反查里：把引导键 `z` 接回去，否则剩下的字母会被当成五笔码。
                let prefix = char::from(PINYIN_LOOKUP_PREFIX);
                let rest = format!("{prefix}{rest}");
                let rest_with_cases = format!("{prefix}{rest_with_cases}");
                self.engine
                    .replace_active_raw_input(&rest, &rest_with_cases);
            } else {
                self.engine.replace_active_raw_input(rest, rest_with_cases);
            }
            self.online_requests.invalidate();
            self.update_mixed_candidates();
            transition.current_segmentation_with_cases =
                self.pinyin_segmentation_with_cases().into_owned();
            return transition;
        }
        let request = self.engine.request();
        transition.full_pure_pinyin = request.normalized_input.clone();
        transition.current_segmentation_with_cases =
            self.pinyin_segmentation_with_cases().into_owned();
        transition
    }

    /// input_session_composition.cpp:976-1004.
    pub(super) fn update_creating_word_progress(
        current_pinyin: &str,
        current_word: &str,
        selected_word: &str,
        transition: &SelectionTransition,
    ) -> CreatingWordProgress {
        let mut word = String::with_capacity(current_word.len() + selected_word.len());
        word.push_str(current_word);
        word.push_str(selected_word);
        if transition.wubi_native {
            // Wubi phrases are not composed from partial selections: a wubi code is one word.
            return CreatingWordProgress {
                pinyin: if current_pinyin.is_empty() {
                    transition.full_pure_pinyin.clone()
                } else {
                    current_pinyin.to_owned()
                },
                preedit: word.clone(),
                word,
                completed: true,
                can_store: false,
            };
        }
        let selected_storable = canonical_pinyin_is_valid_for_word(
            &transition.selected_canonical_pinyin,
            selected_word,
        );
        let prior_parts_storable = current_word.is_empty() || !current_pinyin.is_empty();
        let pinyin = if prior_parts_storable && selected_storable {
            append_canonical_pinyin(current_pinyin, &transition.selected_canonical_pinyin)
        } else {
            String::new()
        };
        let completed = !transition.continues_composition;
        let can_store = completed && canonical_pinyin_is_valid_for_word(&pinyin, &word);
        let mut preedit =
            String::with_capacity(word.len() + transition.current_segmentation_with_cases.len());
        preedit.push_str(&word);
        preedit.push_str(&transition.current_segmentation_with_cases);
        CreatingWordProgress {
            preedit,
            pinyin,
            word,
            completed,
            can_store,
        }
    }

    /// The segmentation the dictionary was queried with.
    pub(super) fn pinyin_segmentation(&self) -> Cow<'_, str> {
        let request = self.engine.request();
        if request.normalized_segmentation.is_empty() {
            Cow::Borrowed(&request.segmentation)
        } else {
            Cow::Borrowed(&request.normalized_segmentation)
        }
    }

    /// input_session_composition.cpp:417-449.
    pub(super) fn pinyin_segmentation_with_cases(&self) -> Cow<'_, str> {
        if self.has_literal_input() {
            return Cow::Borrowed(self.raw_with_cases());
        }
        let request = self.engine.request();
        match self.engine.current_scheme_type() {
            SchemeType::Wubi => Cow::Borrowed(&request.raw_input),
            SchemeType::JapaneseRomaji
            | SchemeType::Korean
            | SchemeType::Cantonese
            | SchemeType::Zhuyin
            | SchemeType::Vietnamese
            | SchemeType::Tibetan
            | SchemeType::Stroke => Cow::Borrowed(self.raw_with_cases()),
            SchemeType::Shuangpin if self.shuangpin_preedit_uses_raw => with_trailing_separator(
                if request.raw_segmentation.is_empty() {
                    Cow::Borrowed(&request.raw_input)
                } else {
                    // 辅助码段按原样接回对应音节的后面（`decorate_mid_sentence_segmentation`）。
                    Cow::Owned(crate::shuangpin::mid_sentence::decorate_segmentation(
                        &request.raw_segmentation,
                        &request.helpcode_decorations,
                    ))
                },
                request,
            ),
            SchemeType::Quanpin => build_quanpin_autocorrect_display(request),
            SchemeType::Shuangpin => with_trailing_separator(
                Cow::Owned(crate::shuangpin::mid_sentence::decorate_segmentation(
                    &self.pinyin_segmentation(),
                    &request.helpcode_decorations,
                )),
                request,
            ),
        }
    }

    /// input_session_composition.cpp:456-481.
    pub(super) fn is_all_complete_pure_pinyin(&self) -> bool {
        let request = self.engine.request();
        match self.engine.current_scheme_type() {
            SchemeType::Wubi => request.valid,
            SchemeType::JapaneseRomaji => is_romaji_complete(&request.raw_input),
            // 韩文、粤拼、注音、越南文、藏文和笔画都不是拼音。
            SchemeType::Korean
            | SchemeType::Cantonese
            | SchemeType::Zhuyin
            | SchemeType::Vietnamese
            | SchemeType::Tibetan
            | SchemeType::Stroke => false,
            SchemeType::Shuangpin => {
                let profile = self.shuangpin_profile();
                let base = resolve_shuangpin_composition_base(request, profile);
                if base.helpcode_length > 0
                    && base.effective_raw_input.len() >= base.helpcode_length
                {
                    let base_length = base.effective_raw_input.len() - base.helpcode_length;
                    let prefix = raw_length_for_effective_prefix(&base.raw_input, base_length);
                    return is_complete_input(&base.raw_input[..prefix], profile);
                }
                is_complete_input(&base.raw_input, profile)
            }
            SchemeType::Quanpin => {
                let segmentation = self.pinyin_segmentation();
                !segmentation.is_empty() && is_complete_pinyin_input(segmentation.as_ref())
            }
        }
    }

    /// Four native letters whose whole list is one wubi row, the rule the shipped engine applied (input_session_composition.cpp:483-490).
    pub(super) fn wubi_unique_four_code(&self) -> bool {
        if self.dedicated_english || self.local_mode != LocalInputMode::None {
            return false;
        }
        // 带 `z` 的码是通配猜码，不是用户确定下来的编码：它后面那个「唯一」候选只跟若干个码差一个字母，
        // 直接上屏就把「看见命中了什么」这件事拿走了——那正是这个键存在的理由。来源的
        // `wubi_unique_four_code` 用同一条 `raw_input.find('z')` 守卫。
        if self.engine.request().raw_input.contains('z') {
            return false;
        }
        // The whole list counts, not only the wubi rows. In mixed Wubi four letters are often pinyin as well — `jixu` is the wubi code of 曳光弹 and the pinyin of 继续 — and committing the one wubi row on the fourth key takes the pinyin away from someone typing it. The wubi_mixed_routing overlay counted wubi rows only; it never applied to the shipped engine, and that rule is what auto-committed 曳光弹 for jixu.
        // 追加在末尾的快捷短语不算：它们只是编码恰好相同，而这条规则问的是五笔码本身唯一；算上它们，`addr`（莽原）这类码就因为词库里有同编码的短语而不再自动上屏。宿主提交的是第一格，所以第一格也得是那一行。
        let candidates = self.candidates();
        self.wubi_candidates_are_native()
            && self.engine.wubi_has_complete_code()
            && candidates
                .first()
                .is_some_and(|item| item.source != CandidateSource::QuickPhrase)
            && candidates
                .iter()
                .filter(|item| item.source != CandidateSource::QuickPhrase)
                .count()
                == 1
    }

    pub(super) fn has_active_helpcode(&self) -> bool {
        let request = self.engine.request();
        match self.engine.current_scheme_type() {
            SchemeType::Wubi
            | SchemeType::JapaneseRomaji
            | SchemeType::Korean
            | SchemeType::Cantonese
            | SchemeType::Zhuyin
            | SchemeType::Vietnamese
            | SchemeType::Tibetan
            | SchemeType::Stroke => false,
            SchemeType::Shuangpin => {
                !request.end_helpcodes.is_empty()
                    || active_shuangpin_helpcode_length(request, self.shuangpin_profile()) > 0
            }
            SchemeType::Quanpin => {
                request.enable_quanpin_helpcode
                    && detect_active_helpcode_length(
                        &request.raw_input,
                        &request.raw_input_with_cases,
                    ) > 0
            }
        }
    }

    /// input_session.cpp:775-793.
    pub(super) fn candidate_annotations(&self, hide_helpcode: bool) -> Vec<String> {
        // Rows the engine generated are not spelled by pinyin, so a helpcode would say nothing about them; a command row shows its command's title instead, and a place offered in `@` mode the division it belongs to. 拆字行注释这个字的读音：用户就是不会读才来查的。
        if self.local_mode.annotates_rows() {
            return self
                .candidates()
                .iter()
                .map(|item| match self.local_mode {
                    // 插件行的标题和插件自己的注释由 runtime 画在插件注释格里；引擎注释格只放引擎的内容，留空。
                    LocalInputMode::Command if item.source == CandidateSource::Plugin => {
                        String::new()
                    }
                    LocalInputMode::Command => self
                        .queries
                        .command_title(&item.pinyin)
                        .unwrap_or_default()
                        .to_owned(),
                    LocalInputMode::Mention => self
                        .queries
                        .mention_annotation(&item.word, &item.pinyin)
                        .to_owned(),
                    LocalInputMode::Chaizi => crate::local::chaizi::reading(&item.word).to_owned(),
                    _ => String::new(),
                })
                .collect();
        }
        // A Hanja row shows its 훈음 (나라 이름 한), the reading a Korean user picks a Hanja by.
        if self.korean_rules_apply() {
            let syllable = &self.engine.request().normalized_segmentation;
            return self
                .candidates()
                .iter()
                .map(|item| crate::korean::hanja::gloss(syllable, &item.word).to_owned())
                .collect();
        }
        let enabled = self.helpcode_enabled();
        let uppercase_all = self.scheme() == SchemeType::Quanpin;
        let keymap = self.helpcode_keymap.as_deref();
        self.candidates()
            .iter()
            .map(|item| {
                let helpcode = match keymap {
                    Some(keymap)
                        if enabled
                            && !hide_helpcode
                            && !matches!(
                                item.source,
                                CandidateSource::EnglishDictionary | CandidateSource::MixedJapanese
                            ) =>
                    {
                        compute_helpcodes(&item.word, uppercase_all, keymap)
                    }
                    _ => String::new(),
                };
                if helpcode.is_empty() || hide_helpcode {
                    item.corrected_from.clone()
                } else {
                    helpcode
                }
            })
            .collect()
    }

    /// Staged host editing applied to the scheme (input_session_composition.cpp:1037-1070).
    pub(super) fn apply_pending_sequence(&mut self) {
        self.caret = None;
        let raw = self
            .pending_sequence
            .take()
            .unwrap_or_else(|| self.engine.request().raw_input.clone());
        let raw_with_cases = self
            .pending_sequence_with_cases
            .take()
            .unwrap_or_else(|| raw.clone());
        self.engine.replace_active_raw_input(&raw, &raw_with_cases);
        self.online_requests.invalidate();
        self.update_mixed_candidates();
    }

    /// Re-query without changing the composition.
    pub(super) fn recompute_candidates(&mut self) {
        if self.pending_sequence.is_some() || self.pending_sequence_with_cases.is_some() {
            self.apply_pending_sequence();
            return;
        }
        self.engine.handle_key(SchemeKey::Requery);
        self.update_mixed_candidates();
    }

    /// The code was answered only by quanpin rows (overlays.md §3.3: read from the rows, not the list as a whole).
    pub(super) fn answered_by_pinyin_fallback(&self) -> bool {
        self.is_wubi()
            && !self.candidates().is_empty()
            && self
                .candidates()
                .iter()
                .all(|item| !Self::is_wubi_native_candidate(item))
    }

    pub(super) fn wubi_candidates_are_native(&self) -> bool {
        self.is_wubi() && self.candidates().iter().any(Self::is_wubi_native_candidate)
    }

    /// Whether the list reads the composition as pinyin, so selections advance and learn as pinyin.
    pub(super) fn candidates_follow_pinyin(&self) -> bool {
        self.engine
            .current_scheme_type()
            .follows_pinyin_candidates()
            || !self.wubi_candidates_are_native()
    }
}

#[cfg(test)]
#[path = "composition/canonical_reading_tests.rs"]
mod canonical_reading_tests;

#[cfg(test)]
#[path = "composition/creating_word_reading_tests.rs"]
mod creating_word_reading_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_readings_need_one_complete_syllable_per_character() {
        assert_eq!(
            normalize_canonical_pinyin_for_word("ni'hao", "你好"),
            "ni'hao"
        );
        assert_eq!(normalize_canonical_pinyin_for_word("ni'hao", "你"), "");
        assert_eq!(normalize_canonical_pinyin_for_word("ni'h", "你好"), "");
        assert_eq!(normalize_canonical_pinyin_for_word("", "你"), "");
    }

    #[test]
    fn canonical_readings_append_and_empty_on_a_gap() {
        assert_eq!(append_canonical_pinyin("", "ni"), "ni");
        assert_eq!(append_canonical_pinyin("ni", "hao"), "ni'hao");
        let result = append_canonical_pinyin("shan", "shui");
        assert_eq!(result.capacity(), result.len());
        assert_eq!(append_canonical_pinyin("ni", ""), "");
        assert_eq!(result, "shan'shui");
    }

    /// test_input_session.cpp:600-609: a phrase is storable only when every picked piece had a canonical reading.
    #[test]
    fn an_unknown_earlier_reading_makes_the_phrase_unstorable() {
        let last = SelectionTransition {
            selected_canonical_pinyin: "te'le".into(),
            ..SelectionTransition::default()
        };
        let known = InputSession::update_creating_word_progress("xi", "西", "特乐", &last);
        assert!(known.completed && known.can_store);
        assert_eq!(known.pinyin, "xi'te'le");
        assert_eq!(known.word, "西特乐");
        assert_eq!(known.word.capacity(), known.word.len());
        assert_eq!(known.preedit.capacity(), known.preedit.len());
        let with_tail = SelectionTransition {
            selected_canonical_pinyin: "te'le".into(),
            current_segmentation_with_cases: "hao".into(),
            ..SelectionTransition::default()
        };
        let with_tail = InputSession::update_creating_word_progress("xi", "西", "特乐", &with_tail);
        assert_eq!(with_tail.preedit, "西特乐hao");
        assert_eq!(with_tail.preedit.capacity(), with_tail.preedit.len());

        let unknown = InputSession::update_creating_word_progress("", "西", "特乐", &last);
        assert!(unknown.completed);
        assert!(!unknown.can_store);
        assert!(unknown.pinyin.is_empty());
        assert_eq!(unknown.word, "西特乐");
        // Without the guard a piece with no Han character and no reading would pass the one-syllable-per-character check.
        let unknown = InputSession::update_creating_word_progress("", "abc", "特乐", &last);
        assert!(!unknown.can_store);
        assert!(unknown.pinyin.is_empty());
    }

    #[test]
    fn consumed_separators_are_dropped_from_the_rest() {
        assert_eq!(remove_consumed_leading_separators("''hao"), "hao");
        assert_eq!(remove_consumed_leading_separators("hao'"), "hao'");
        assert_eq!(remove_delimiters("ni'hao'"), "nihao");
    }

    #[test]
    fn folding_ignores_case_delimiters_and_the_v_spelling() {
        assert_eq!(fold_autocorrect_letters("Nv'E"), "nue");
        assert_eq!(fold_autocorrect_letters("sa'Hng"), "sahng");
        assert_eq!(fold_autocorrect_letters("Nv'e"), "nue");
    }

    #[test]
    fn folded_letter_comparison_matches_owned_folding() {
        for (left, right, equal) in [
            ("Nv'E", "nue", true),
            ("sa'Hng", "sahng", true),
            ("sahng", "shang", false),
            ("", "'", true),
        ] {
            assert_eq!(folded_letters_equal(left, right), equal, "{left}/{right}");
            assert_eq!(
                fold_autocorrect_letters(left) == fold_autocorrect_letters(right),
                equal,
                "owned {left}/{right}"
            );
        }
    }

    #[test]
    fn folded_cut_comparison_reads_segment_syllables_directly() {
        let segments = vec![crate::pinyin::autocorrect::AutocorrectCutSegment {
            syllable: "NvE".to_owned(),
            ..Default::default()
        }];
        assert!(folded_segments_equal(&segments, "nue"));
    }

    /// The request a quanpin session builds for `typed` under the two user switches (test_pinyin.cpp P38).
    fn display(typed: &str, transposition: bool, neighbor: bool) -> String {
        let mut scheme = crate::quanpin::QuanpinScheme::new();
        for byte in typed.bytes() {
            scheme.handle_key(if byte == b'\'' {
                SchemeKey::Apostrophe
            } else {
                SchemeKey::Letter(byte)
            });
        }
        let mut request = scheme.build_request();
        request.enable_quanpin_autocorrect_transposition = transposition;
        request.enable_quanpin_autocorrect_neighbor = neighbor;
        build_quanpin_autocorrect_display(&request).into_owned()
    }

    #[test]
    fn the_preedit_shows_the_typed_letters() {
        // Both switches on; deletion and insertion ride along.
        for (typed, shown) in [
            ("sahng", "sahng"),
            ("sahnghao", "sahng'hao"),
            ("shabg", "shabg"),
            ("iandu", "ian'du"),
            ("keneng", "ke'neng"),
            ("xi'an", "xi'an"),
            ("zheg", "zhe'g"),
            ("wj", "w'j"),
            ("nv", "nv"),
            ("shng", "shng"),
            ("sshang", "sshang"),
            ("zher", "zh'er"),
        ] {
            assert_eq!(display(typed, true, true), shown, "{typed}");
        }
        assert_eq!(display("saHng", true, true), "saHng");
        assert_eq!(display("shabg", false, true), "shabg");
        // With both switches off the scheme's greedy separators stay, but an alias rewrite is still undone.
        assert_eq!(display("shabg", false, false), "sha'b'g");
        assert_eq!(display("sahng", false, false), "sahng");
    }

    #[test]
    fn display_of_an_empty_request_is_its_segmentation() {
        let request = QueryRequest {
            raw_segmentation: "x".to_owned(),
            ..QueryRequest::default()
        };
        assert_eq!(build_quanpin_autocorrect_display(&request), "x");
    }

    #[test]
    fn helpcode_length_matches_the_composition_base() {
        let profile =
            crate::shuangpin::profile::profile(crate::types::ShuangpinProfileKind::Xiaohe).unwrap();
        for (raw, raw_with_cases, enabled, direct) in [
            ("nihcAB", "nihcAB", true, true),
            ("uiu", "uiu", true, true),
            ("ui'u", "ui'u", true, true),
            ("uiu", "uiu", false, true),
            // 末尾直接关闭后末尾裸字母不再算辅助码。
            ("uiu", "uiu", true, false),
            ("nihcAB", "nihcAB", true, false),
        ] {
            let request = QueryRequest {
                raw_input: raw.to_owned(),
                raw_input_with_cases: raw_with_cases.to_owned(),
                enable_shuangpin_helpcode: enabled,
                shuangpin_helpcode_modes: crate::types::ShuangpinHelpcodeModes {
                    direct,
                    ..crate::types::ShuangpinHelpcodeModes::default()
                },
                ..QueryRequest::default()
            };
            let expected = match (enabled, direct, raw) {
                (true, true, "nihcAB") => 2,
                (true, true, "uiu") => 1,
                _ => 0,
            };
            assert_eq!(
                active_shuangpin_helpcode_length(&request, profile),
                expected,
                "length {raw}/{raw_with_cases}/{enabled}/{direct}"
            );
            assert_eq!(
                resolve_shuangpin_composition_base(&request, profile).helpcode_length,
                expected,
                "base {raw}/{raw_with_cases}/{enabled}/{direct}"
            );
        }
    }

    #[test]
    fn shuangpin_composition_base_borrows_unseparated_input() {
        let request = QueryRequest {
            raw_input: "nihc".to_owned(),
            raw_input_with_cases: "nihc".to_owned(),
            ..QueryRequest::default()
        };
        let profile =
            crate::shuangpin::profile::profile(crate::types::ShuangpinProfileKind::Xiaohe).unwrap();
        let base = resolve_shuangpin_composition_base(&request, profile);
        assert!(matches!(base.raw_input, std::borrow::Cow::Borrowed(_)));
        assert!(matches!(
            base.raw_input_with_cases,
            std::borrow::Cow::Borrowed(_)
        ));
        assert!(matches!(
            base.effective_raw_input,
            std::borrow::Cow::Borrowed(_)
        ));
        assert!(matches!(
            base.effective_raw_input_with_cases,
            std::borrow::Cow::Borrowed(_)
        ));
    }
}
