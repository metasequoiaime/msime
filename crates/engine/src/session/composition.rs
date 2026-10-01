//! Selection advancement, segmentation getters and preedit display (core-session.md §5.8, §5.10-§5.11), including the wubi mixed routing rules that key advancement on the selected row's producer (overlays.md §3.3).

use std::borrow::Cow;

use super::input::{CreatingWordProgress, InputSession};
use crate::helpcode::compute_helpcodes;
use crate::japanese::romaji::convert_romaji;
use crate::pinyin::active_helpcode::{
    detect_active_helpcode_length, strip_active_helpcodes, strip_active_helpcodes_with_cases,
};
use crate::pinyin::autocorrect::{
    autocorrect_cut_detail, looks_like_syllable_with_jianpin_tail, AutocorrectCutSegment,
};
use crate::pinyin::segment::{is_complete_pinyin_input, join_segments, split_segments};
use crate::shuangpin::query::{
    detect_active_double_helpcode_length, effective_input_length, is_complete_input,
    raw_length_for_effective_prefix, remove_manual_delimiters,
};
use crate::shuangpin::ShuangpinProfile;
use crate::text::count_han_chars;
use crate::types::{
    request_autocorrect_mask, CandidateSource, LocalInputMode, QueryRequest, SchemeKey, SchemeType,
};

/// What a selection did to the composition.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct SelectionTransition {
    pub continues_composition: bool,
    pub full_pure_pinyin: String,
    pub current_segmentation: String,
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
    if !request.enable_shuangpin_helpcode || base.effective_raw_input.is_empty() {
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
    segmented.chars().filter(|c| *c != '\'').collect()
}

/// A consumed prefix can leave the remainder starting with the separator that followed it.
fn remove_consumed_leading_separators(raw: &str) -> &str {
    raw.trim_start_matches('\'')
}

/// Detect an active shuangpin helpcode without constructing the owned composition base. The
/// candidate refresh path only needs this length to decide whether dynamic rows can move.
fn active_shuangpin_helpcode_length(request: &QueryRequest, profile: &ShuangpinProfile) -> usize {
    if !request.enable_shuangpin_helpcode {
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

/// The canonical reading of a selected word, if it has one complete syllable per character (input_session_composition.cpp:76-96).
pub(super) fn normalize_canonical_pinyin_for_word(pinyin: &str, word: &str) -> String {
    if pinyin.is_empty() {
        return String::new();
    }
    let segments = split_segments(pinyin);
    if segments.is_empty() || segments.len() != count_han_chars(word) {
        return String::new();
    }
    if segments
        .iter()
        .any(|segment| segment.is_empty() || !is_complete_pinyin_input(segment))
    {
        return String::new();
    }
    join_segments(&segments)
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
pub(super) fn build_quanpin_autocorrect_display(request: &QueryRequest) -> String {
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
        return base.clone();
    }
    let types = request_autocorrect_mask(
        request.enable_quanpin_autocorrect_transposition,
        request.enable_quanpin_autocorrect_neighbor,
    );
    let letters_rewritten = !folded_letters_equal(base, cased);
    // The scheme kept the typed letters and no correction can apply, so there are no other separators to draw.
    if !letters_rewritten && (types == 0 || is_complete_pinyin_input(&request.raw_input)) {
        return base.clone();
    }
    // A legal syllable plus one trailing letter is jianpin, never a typo, as in the dictionary's own guard; without this the deletion table would re-separate `zheg`.
    if looks_like_syllable_with_jianpin_tail(&request.raw_input) {
        return base.clone();
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
            return display;
        }
    }
    if letters_rewritten {
        remove_delimiters(cased)
    } else {
        base.clone()
    }
}

impl InputSession {
    /// input_session_composition.cpp:714-748, with the selected row's scheme deciding the native-wubi branch.
    pub(super) fn selection_completes_composition(
        &self,
        pinyin: &str,
        word: &str,
        selected_scheme: SchemeType,
    ) -> bool {
        // Japanese, Korean and native wubi selections always finish: their advancement never continues.
        if self.is_japanese() || self.is_korean() || selected_scheme == SchemeType::Wubi {
            return true;
        }
        let request = self.engine.request();
        if self.is_shuangpin() {
            let base = resolve_shuangpin_composition_base(request, self.shuangpin_profile());
            let word_length = count_han_chars(word) * 2;
            let total = base.effective_raw_input.len();
            if base.helpcode_length > 0 {
                let required = word_length + base.helpcode_length;
                return !(required < total && word_length < total);
            }
            // With no helpcode the pure pinyin is the whole effective input.
            let mut consumed = remove_delimiters(pinyin).len();
            if consumed == 0 || consumed > total {
                consumed = word_length.min(total);
            }
            return consumed >= total;
        }
        let selected = remove_delimiters(pinyin);
        let raw_without_helpcodes =
            strip_active_helpcodes(&request.raw_input, &request.raw_input_with_cases);
        let cased_without_helpcodes =
            strip_active_helpcodes_with_cases(&request.raw_input, &request.raw_input_with_cases);
        let consumed_raw =
            raw_length_for_effective_prefix(&cased_without_helpcodes, selected.len());
        !(!selected.is_empty()
            && selected.len() < request.normalized_input.len()
            && consumed_raw < raw_without_helpcodes.len())
    }

    /// input_session_composition.cpp:799-911.
    pub(super) fn advance_composition_after_selection(
        &mut self,
        pinyin: &str,
        word: &str,
        canonical: &str,
        selected_scheme: SchemeType,
    ) -> SelectionTransition {
        let mut transition = SelectionTransition {
            selected_canonical_pinyin: canonical.to_owned(),
            wubi_native: selected_scheme == SchemeType::Wubi,
            ..SelectionTransition::default()
        };
        let request = self.engine.request().clone();
        if self.is_japanese() {
            transition.full_pure_pinyin = request.raw_input;
            transition.current_segmentation = request.segmentation;
            transition.current_segmentation_with_cases = request.raw_input_with_cases;
            return transition;
        }
        if transition.wubi_native {
            transition.full_pure_pinyin = request.normalized_input.clone();
            transition.current_segmentation = request.normalized_input;
            transition.current_segmentation_with_cases = request.raw_input;
            return transition;
        }
        transition.continues_composition =
            !self.selection_completes_composition(pinyin, word, selected_scheme);
        if self.is_shuangpin() {
            let base = resolve_shuangpin_composition_base(&request, self.shuangpin_profile());
            let word_length = count_han_chars(word) * 2;
            let total = base.effective_raw_input.len();
            transition.full_pure_pinyin =
                if base.helpcode_length > 0 && total >= base.helpcode_length {
                    base.effective_raw_input[..total - base.helpcode_length].to_owned()
                } else {
                    base.effective_raw_input.clone().into_owned()
                };
            if transition.continues_composition {
                let (start, end) = if base.helpcode_length > 0 {
                    // The helpcode chose this word; the rest drops it.
                    (
                        raw_length_for_effective_prefix(&base.raw_input_with_cases, word_length),
                        raw_length_for_effective_prefix(
                            &base.raw_input_with_cases,
                            total - base.helpcode_length,
                        ),
                    )
                } else {
                    let mut consumed = remove_delimiters(pinyin).len();
                    if consumed == 0 || consumed > total {
                        consumed = word_length.min(total);
                    }
                    (
                        raw_length_for_effective_prefix(&base.raw_input_with_cases, consumed),
                        base.raw_input.len(),
                    )
                };
                let rest = remove_consumed_leading_separators(&base.raw_input[start..end]);
                let rest_with_cases =
                    remove_consumed_leading_separators(&base.raw_input_with_cases[start..end]);
                self.engine.replace_active_raw_input(rest, rest_with_cases);
                self.online_requests.invalidate();
                self.update_mixed_candidates();
            }
            transition.current_segmentation = self.pinyin_segmentation();
            transition.current_segmentation_with_cases = self.pinyin_segmentation_with_cases();
            return transition;
        }

        // Quanpin, and the quanpin rows of a wubi composition, which shorten the wubi code the same way.
        transition.full_pure_pinyin = request.normalized_input.clone();
        let selected = remove_delimiters(pinyin);
        let raw_without_helpcodes =
            strip_active_helpcodes(&request.raw_input, &request.raw_input_with_cases);
        let cased_without_helpcodes =
            strip_active_helpcodes_with_cases(&request.raw_input, &request.raw_input_with_cases);
        if transition.continues_composition {
            let consumed =
                raw_length_for_effective_prefix(&cased_without_helpcodes, selected.len());
            let rest = remove_consumed_leading_separators(&raw_without_helpcodes[consumed..]);
            let rest_with_cases =
                remove_consumed_leading_separators(&cased_without_helpcodes[consumed..]);
            if self.is_wubi() {
                // The rest of a spelling the user is still in the middle of stays pinyin; the wubi table answering it would swap schemes underneath them.
                self.engine.keep_pinyin_tail();
            }
            self.engine.replace_active_raw_input(rest, rest_with_cases);
            self.online_requests.invalidate();
            self.update_mixed_candidates();
            transition.current_segmentation = self.pinyin_segmentation();
            transition.current_segmentation_with_cases = self.pinyin_segmentation_with_cases();
            return transition;
        }
        transition.current_segmentation = if request.normalized_segmentation.is_empty() {
            request.segmentation.clone()
        } else {
            request.normalized_segmentation.clone()
        };
        transition.current_segmentation_with_cases = self.pinyin_segmentation_with_cases();
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
        let selected_canonical = normalize_canonical_pinyin_for_word(
            &transition.selected_canonical_pinyin,
            selected_word,
        );
        let prior_parts_storable = current_word.is_empty() || !current_pinyin.is_empty();
        let pinyin = if prior_parts_storable && !selected_canonical.is_empty() {
            append_canonical_pinyin(current_pinyin, &selected_canonical)
        } else {
            String::new()
        };
        let completed = !transition.continues_composition;
        let can_store =
            completed && !normalize_canonical_pinyin_for_word(&pinyin, &word).is_empty();
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
    pub(super) fn pinyin_segmentation(&self) -> String {
        let request = self.engine.request();
        if request.normalized_segmentation.is_empty() {
            request.segmentation.clone()
        } else {
            request.normalized_segmentation.clone()
        }
    }

    /// input_session_composition.cpp:417-449.
    pub(super) fn pinyin_segmentation_with_cases(&self) -> String {
        let request = self.engine.request();
        let with_trailing_separator = |mut preedit: String| {
            if request.raw_input_with_cases.ends_with('\'') && !preedit.ends_with('\'') {
                preedit.push('\'');
            }
            preedit
        };
        match self.engine.current_scheme_type() {
            SchemeType::Wubi => request.raw_input.clone(),
            SchemeType::JapaneseRomaji | SchemeType::Korean => self.raw_with_cases().to_owned(),
            SchemeType::Shuangpin if self.shuangpin_preedit_uses_raw => {
                with_trailing_separator(if request.raw_segmentation.is_empty() {
                    request.raw_input.clone()
                } else {
                    request.raw_segmentation.clone()
                })
            }
            SchemeType::Quanpin => build_quanpin_autocorrect_display(request),
            SchemeType::Shuangpin => with_trailing_separator(self.pinyin_segmentation()),
        }
    }

    /// input_session_composition.cpp:456-481.
    pub(super) fn is_all_complete_pure_pinyin(&self) -> bool {
        let request = self.engine.request();
        match self.engine.current_scheme_type() {
            SchemeType::Wubi => request.valid,
            SchemeType::JapaneseRomaji => convert_romaji(&request.raw_input).complete,
            // Hangul is not pinyin.
            SchemeType::Korean => false,
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
                !segmentation.is_empty() && is_complete_pinyin_input(&segmentation)
            }
        }
    }

    /// Four native letters whose whole list is one wubi row, the rule the shipped engine applied (input_session_composition.cpp:483-490).
    pub(super) fn wubi_unique_four_code(&self) -> bool {
        if self.dedicated_english || self.local_mode != LocalInputMode::None {
            return false;
        }
        // The whole list counts, not only the wubi rows. In mixed Wubi four letters are often pinyin as well — `jixu` is the wubi code of 曳光弹 and the pinyin of 继续 — and committing the one wubi row on the fourth key takes the pinyin away from someone typing it. The wubi_mixed_routing overlay counted wubi rows only; it never applied to the shipped engine, and that rule is what auto-committed 曳光弹 for jixu.
        self.wubi_candidates_are_native()
            && self.engine.wubi_has_complete_code()
            && self.candidates().len() == 1
    }

    pub(super) fn has_active_helpcode(&self) -> bool {
        let request = self.engine.request();
        match self.engine.current_scheme_type() {
            SchemeType::Wubi | SchemeType::JapaneseRomaji | SchemeType::Korean => false,
            SchemeType::Shuangpin => {
                active_shuangpin_helpcode_length(request, self.shuangpin_profile()) > 0
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
    pub(super) fn candidate_annotations(&self) -> Vec<String> {
        // Rows the engine generated are not spelled by pinyin, so a helpcode would say nothing about them; a command row shows its command's title instead, and a place offered in `@` mode the division it belongs to.
        if self.local_mode.generates_text() {
            return self
                .candidates()
                .iter()
                .map(|item| match self.local_mode {
                    LocalInputMode::Command => self
                        .queries
                        .command_title(&item.pinyin)
                        .unwrap_or_default()
                        .to_owned(),
                    LocalInputMode::Mention => {
                        self.queries.mention_annotation(&item.word).to_owned()
                    }
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
                        if enabled && item.source != CandidateSource::EnglishDictionary =>
                    {
                        compute_helpcodes(&item.word, uppercase_all, keymap)
                    }
                    _ => String::new(),
                };
                if helpcode.is_empty() {
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
        self.engine.current_scheme_type().is_pinyin() || !self.wubi_candidates_are_native()
    }
}

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
        build_quanpin_autocorrect_display(&request)
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
            crate::shuangpin::profile::profile(crate::types::ShuangpinProfileKind::Xiaohe);
        for (raw, raw_with_cases, enabled) in [
            ("nihcAB", "nihcAB", true),
            ("uiu", "uiu", true),
            ("ui'u", "ui'u", true),
            ("uiu", "uiu", false),
        ] {
            let request = QueryRequest {
                raw_input: raw.to_owned(),
                raw_input_with_cases: raw_with_cases.to_owned(),
                enable_shuangpin_helpcode: enabled,
                ..QueryRequest::default()
            };
            assert_eq!(
                active_shuangpin_helpcode_length(&request, profile),
                resolve_shuangpin_composition_base(&request, profile).helpcode_length,
                "{raw}/{raw_with_cases}/{enabled}"
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
            crate::shuangpin::profile::profile(crate::types::ShuangpinProfileKind::Xiaohe);
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
