//! `ImeSession` (core-session.md §6): owns the active scheme and the providers, turns every key into a fresh request and candidate list, and runs the wubi mixed-pinyin fallback merge. Also the list-level helpers the input session builds on: local and mixed candidate queries, the online batch rule and the personal context rerank.

pub mod online_batch;
pub mod personal_rerank;
pub mod queries;
pub mod registry;
pub mod scheme;

use std::collections::HashSet;
use std::sync::Arc;

use crate::assets;
use crate::helpcode::SharedKeymap;
use crate::paths::RuntimePaths;
use crate::pinyin::autocorrect::autocorrect_suppression_key;
use crate::quanpin::QuanpinScheme;
use crate::shuangpin::profile::profile;
use crate::shuangpin::query::{
    apply_segmentation_cases, detect_active_double_helpcode_length, remove_manual_delimiters,
    segment_input, to_quanpin_segmentation, trim_trailing_letters_preserve_delimiters,
};
use crate::shuangpin::ShuangpinProfile;
use crate::types::{
    autocorrect_type, CandidateSource, FuzzyPinyinOptions, QueryRequest, SchemeKey, SchemeType,
    SentenceAssociationOptions, ShuangpinProfileKind, WordItem, WubiInputOptions,
};
use crate::user_dictionary::typo_profile::PersonalTypoProfile;

use registry::ProviderRegistry;
use scheme::Scheme;

/// What the providers answered for the current composition.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompositionState {
    pub preedit: String,
    pub request: QueryRequest,
    pub candidates: Vec<WordItem>,
}

/// One decode of a request, before it is stored as the live state.
struct Decoded {
    candidates: Vec<WordItem>,
    /// Some row matches the whole wubi code; prefix rows do not count.
    wubi_table_answered: bool,
}

pub struct ImeSession {
    scheme: Scheme,
    registry: ProviderRegistry,
    state: CompositionState,
    profile: ShuangpinProfileKind,
    wubi_options: WubiInputOptions,
    /// Mixed wubi only: a pinyin candidate was picked out of this composition, so the rest decodes as quanpin until the composition ends (product decision 2026-09-30, the intent of test_wubi_mixed_input_session.cpp:194-212). Cleared by `reset`, `switch_scheme`, turning mixed input off, and an emptied composition.
    pinyin_tail: bool,
    autocorrect_types: u32,
    quanpin_helpcode: bool,
    shuangpin_helpcode: bool,
    fuzzy: FuzzyPinyinOptions,
    sentence_alternatives: bool,
    sentence_association: SentenceAssociationOptions,
    rescoring_context: String,
    typo_profile: Arc<PersonalTypoProfile>,
}

impl ImeSession {
    /// ime_session.cpp:42-49.
    pub fn new(scheme: SchemeType, profile: ShuangpinProfileKind, paths: &RuntimePaths) -> Self {
        let mut session = Self {
            scheme: Scheme::new(scheme, profile),
            registry: ProviderRegistry::new(profile, paths),
            state: CompositionState::default(),
            profile,
            wubi_options: WubiInputOptions::default(),
            pinyin_tail: false,
            autocorrect_types: 0,
            quanpin_helpcode: false,
            shuangpin_helpcode: false,
            fuzzy: FuzzyPinyinOptions::default(),
            sentence_alternatives: false,
            sentence_association: SentenceAssociationOptions::default(),
            rescoring_context: String::new(),
            typo_profile: PersonalTypoProfile::shared(&paths.user(assets::USER_JOURNAL)),
        };
        session.bind_wubi_scheme();
        session
    }

    pub fn candidates(&self) -> &[WordItem] {
        &self.state.candidates
    }

    pub fn request(&self) -> &QueryRequest {
        &self.state.request
    }

    pub fn preedit(&self) -> &str {
        &self.state.preedit
    }

    pub fn current_scheme_type(&self) -> SchemeType {
        self.scheme.scheme_type()
    }

    /// `scheme.handle_key` then `refresh_candidates`; `SchemeKey::Requery` only refreshes.
    pub fn handle_key(&mut self, key: SchemeKey) {
        if key != SchemeKey::Requery {
            self.scheme.handle_key(key);
        }
        self.refresh_candidates();
    }

    /// A new scheme and an empty state.
    pub fn switch_scheme(&mut self, scheme: SchemeType) {
        self.scheme = Scheme::new(scheme, self.profile);
        self.bind_wubi_scheme();
        self.state = CompositionState::default();
        self.pinyin_tail = false;
    }

    pub fn reset(&mut self) {
        self.scheme.reset();
        self.state = CompositionState::default();
        self.pinyin_tail = false;
    }

    /// A pinyin candidate was picked out of a mixed wubi composition: what is left keeps decoding as quanpin, never as wubi codes, until the composition ends. Nothing for any other scheme or with mixed input off.
    pub fn keep_pinyin_tail(&mut self) {
        self.pinyin_tail =
            self.current_scheme_type() == SchemeType::Wubi && self.wubi_options.mixed_pinyin;
    }

    /// Host editing for whichever scheme is active; wubi also re-derives its length allowance from the mixed-pinyin option (ime_session.cpp:103-157, 199-238).
    pub fn replace_active_raw_input(&mut self, raw: &str, raw_with_cases: &str) {
        // Host editing rewrites text the composition already holds, so its clip length comes from the setting, not from what the last query answered; otherwise moving the caret through a mixed composition drops everything past the fourth letter. `refresh_candidates` restores the query-derived value.
        let mixed_pinyin = self.wubi_options.mixed_pinyin;
        if let Some(wubi) = self.scheme.as_wubi_mut() {
            wubi.set_extended_length_allowed(mixed_pinyin);
        }
        self.scheme.set_raw_input(raw, raw_with_cases);
        self.refresh_candidates();
    }

    pub fn cycle_japanese_kana_variant(&mut self) -> bool {
        let Scheme::Japanese(japanese) = &mut self.scheme else {
            return false;
        };
        if !japanese.cycle_last_kana_variant() {
            return false;
        }
        self.refresh_candidates();
        true
    }

    /// Opens the Hanja list of the composing Korean syllable. False, with the list left closed, for every other scheme and for a composition the table has no Hanja for (a lone jamo).
    pub fn open_korean_hanja(&mut self) -> bool {
        let Scheme::Korean(korean) = &mut self.scheme else {
            return false;
        };
        korean.open_hanja();
        self.refresh_candidates();
        if !self.state.candidates.is_empty() {
            return true;
        }
        self.close_korean_hanja();
        false
    }

    /// Closes the Hanja list, so the composition answers nothing again; false when no list was open.
    pub fn close_korean_hanja(&mut self) -> bool {
        let Scheme::Korean(korean) = &mut self.scheme else {
            return false;
        };
        if !korean.hanja_open() {
            return false;
        }
        korean.close_hanja();
        self.refresh_candidates();
        true
    }

    pub fn korean_hanja_open(&self) -> bool {
        matches!(&self.scheme, Scheme::Korean(korean) if korean.hanja_open())
    }

    /// The Korean syllables the last key finished, which leave the composition as a commit; empty for every other scheme.
    pub fn take_korean_commit(&mut self) -> String {
        let Scheme::Korean(korean) = &mut self.scheme else {
            return String::new();
        };
        korean.take_committed()
    }

    /// Whether the active scheme is wubi and its code is exactly four letters.
    pub fn wubi_has_complete_code(&self) -> bool {
        self.scheme
            .as_wubi()
            .is_some_and(|wubi| wubi.has_complete_code())
    }

    /// Candidates for a raw prefix through a scratch scheme of the current type, leaving the live composition alone (caret-prefix decoding, overlays.md §7.6).
    pub fn query_raw_candidates(&mut self, raw: &str, raw_with_cases: &str) -> Vec<WordItem> {
        let request = self.raw_request(raw, raw_with_cases);
        if !request.valid {
            return Vec::new();
        }
        self.decode(&request).candidates
    }

    /// `expand_initial_candidates` for a list `query_raw_candidates` returned for the same raw prefix.
    pub fn expand_raw_initial_candidates(
        &mut self,
        raw: &str,
        raw_with_cases: &str,
        candidates: &mut Vec<WordItem>,
    ) -> bool {
        let request = self.raw_request(raw, raw_with_cases);
        request.valid
            && self
                .registry
                .expand_initial_candidates(&request, candidates)
    }

    /// The request a scratch scheme of the current type builds for `raw`, with the session's switches applied.
    fn raw_request(&self, raw: &str, raw_with_cases: &str) -> QueryRequest {
        let mut scratch = Scheme::new(self.current_scheme_type(), self.profile);
        if let Some(wubi) = scratch.as_wubi_mut() {
            wubi.set_mixed_pinyin_allowed(self.wubi_options.mixed_pinyin);
            wubi.set_extended_length_allowed(self.wubi_options.mixed_pinyin);
        }
        scratch.set_raw_input(raw, raw_with_cases);
        self.prepare_request(&scratch)
    }

    pub fn set_helpcode_keymap(&mut self, keymap: Option<SharedKeymap>) {
        self.registry.set_helpcode_keymap(keymap);
        self.refresh_candidates();
    }

    pub fn set_wubi_input_options(&mut self, options: WubiInputOptions) {
        self.wubi_options = options;
        self.pinyin_tail &= options.mixed_pinyin;
        self.bind_wubi_scheme();
    }

    pub fn set_autocorrect_types(&mut self, types: u32) {
        self.autocorrect_types = types;
    }

    pub fn set_quanpin_helpcode_enabled(&mut self, enabled: bool) {
        self.quanpin_helpcode = enabled;
    }

    pub fn set_shuangpin_helpcode_enabled(&mut self, enabled: bool) {
        self.shuangpin_helpcode = enabled;
    }

    pub fn set_fuzzy_pinyin_options(&mut self, options: FuzzyPinyinOptions) {
        self.fuzzy = options;
    }

    pub fn set_sentence_alternatives(&mut self, enabled: bool) {
        self.sentence_alternatives = enabled;
    }

    /// Carried on every request; the engines reset their caches when it changes (overlays.md §1.6.1).
    pub fn set_sentence_association(&mut self, options: SentenceAssociationOptions) {
        self.sentence_association = options;
    }

    pub fn set_rescoring_context(&mut self, context: &str) {
        context.clone_into(&mut self.rescoring_context);
    }

    /// Routed by the producing row's scheme in mixed wubi (overlays.md §3.3). The provider layer only reads: the session writes pins, removals and frequency learning into user_dictionary, keyed by the selected row's scheme, and learned phrases through its own canonical-pinyin `QuanpinEngine`; `reset_cache` then makes the providers see them.
    pub fn find_candidate(&self, scheme: SchemeType, key: &str, value: &str) -> Option<WordItem> {
        self.registry.find_candidate(scheme, key, value)
    }

    /// Resets the current scheme's provider, and quanpin's too while mixed wubi is on.
    pub fn reset_cache(&mut self) {
        let scheme = self.current_scheme_type();
        self.registry.reset_cache(scheme);
        // A mixed list holds rows of both dictionaries, and a pin or removal of a quanpin row has to show on the next refresh.
        if scheme == SchemeType::Wubi && self.wubi_options.mixed_pinyin {
            self.registry.reset_cache(SchemeType::Quanpin);
        }
        self.refresh_candidates();
    }

    pub fn expand_initial_candidates(&mut self) -> bool {
        self.registry
            .expand_initial_candidates(&self.state.request, &mut self.state.candidates)
    }

    /// Insert online rows for the current request and refresh; false when the provider could not take them.
    pub fn apply_dynamic_candidates(&mut self, words: &[String], source: CandidateSource) -> bool {
        if !self
            .registry
            .cache_dynamic_candidates_for_request(&self.state.request, words, source)
        {
            return false;
        }
        self.refresh_candidates();
        true
    }

    /// ime_session.cpp:299-369.
    fn refresh_candidates(&mut self) {
        self.state.preedit = self.scheme.preedit();
        let request = self.prepare_request(&self.scheme);
        if !request.valid {
            // An emptied composition is an invalid request, and Backspace never goes through `reset`: the next code must be answered by the wubi table again.
            self.pinyin_tail = false;
            self.state.request = request;
            self.state.candidates.clear();
            return;
        }

        let decoded = self.decode(&request);
        // A fifth letter is only allowed once the table has failed the code typed so far.
        let extended = self.wubi_options.mixed_pinyin && !decoded.wubi_table_answered;
        if let Some(wubi) = self.scheme.as_wubi_mut() {
            wubi.set_extended_length_allowed(extended);
        }
        self.state.request = request;
        self.state.candidates = decoded.candidates;
    }

    /// The scheme's request with the session's switches, autocorrect suppression and the shuangpin double-helpcode segmentation applied.
    fn prepare_request(&self, scheme: &Scheme) -> QueryRequest {
        let mut request = scheme.build_request();
        self.apply_request_options(&mut request);
        self.apply_autocorrect_suppression(&mut request);
        apply_shuangpin_helpcode_segmentation(&mut request, profile(self.profile));
        request
    }

    fn apply_request_options(&self, request: &mut QueryRequest) {
        request.enable_shuangpin_helpcode = self.shuangpin_helpcode;
        request.enable_quanpin_helpcode = self.quanpin_helpcode;
        request.sentence_alternatives = self.sentence_alternatives;
        request.enable_quanpin_autocorrect_transposition =
            self.autocorrect_types & autocorrect_type::TRANSPOSITION != 0;
        request.enable_quanpin_autocorrect_neighbor =
            self.autocorrect_types & autocorrect_type::NEIGHBOR != 0;
        request.fuzzy_pinyin = self.fuzzy;
        request.sentence_association = self.sentence_association;
        request
            .rescoring_context
            .clone_from(&self.rescoring_context);
    }

    /// An input the user committed raw while a correction was offered is answered as typed: both autocorrect switches off and a literal re-cut, because the correction aliases apply whatever the mask says (ime_session.cpp:285-297). Only consulted while some autocorrect type is on.
    fn apply_autocorrect_suppression(&self, request: &mut QueryRequest) {
        if request.scheme != SchemeType::Quanpin || self.autocorrect_types == 0 {
            return;
        }
        self.typo_profile.refresh_if_changed();
        let key = autocorrect_suppression_key(&request.raw_input, &request.raw_input_with_cases);
        if !self.typo_profile.suppressed(&key) {
            return;
        }
        request.enable_quanpin_autocorrect_transposition = false;
        request.enable_quanpin_autocorrect_neighbor = false;
        QuanpinScheme::apply_literal_segmentation(request);
    }

    /// Query the request's provider and, for wubi with mixed pinyin, append the quanpin rows for the same letters (ime_session.cpp:333-368). The quanpin request gets the session switches and the autocorrect suppression, so a correction the user refused by committing raw stays refused when the same letters arrive through mixed Wubi. A pinyin tail skips the wubi table: those letters are the rest of a spelling, not a code.
    fn decode(&mut self, request: &QueryRequest) -> Decoded {
        let pinyin_tail = request.scheme == SchemeType::Wubi
            && self.wubi_options.mixed_pinyin
            && self.pinyin_tail;
        let candidates = if pinyin_tail {
            Vec::new()
        } else {
            self.registry.query(request)
        };
        if request.scheme != SchemeType::Wubi {
            return Decoded {
                candidates,
                wubi_table_answered: false,
            };
        }
        let wubi_table_answered = wubi_table_answered(&candidates, &request.normalized_input);
        if !self.wubi_options.mixed_pinyin {
            return Decoded {
                candidates,
                wubi_table_answered,
            };
        }

        let mut pinyin = QuanpinScheme::new();
        pinyin.set_raw_input(&request.raw_input, &request.raw_input_with_cases);
        let mut mixed = pinyin.build_request();
        self.apply_request_options(&mut mixed);
        self.apply_autocorrect_suppression(&mut mixed);
        if !mixed.valid {
            return Decoded {
                candidates,
                wubi_table_answered,
            };
        }
        let pinyin_rows = self.registry.query(&mixed);
        Decoded {
            candidates: merge_pinyin_fallback(candidates, pinyin_rows),
            wubi_table_answered,
        }
    }

    fn bind_wubi_scheme(&mut self) {
        let mixed_pinyin = self.wubi_options.mixed_pinyin;
        if let Some(wubi) = self.scheme.as_wubi_mut() {
            wubi.set_mixed_pinyin_allowed(mixed_pinyin);
        }
    }
}

/// A row for the whole code answers it; the prefix rows the wubi query also returns do not, so they must not suppress the pinyin fallback.
fn wubi_table_answered(candidates: &[WordItem], code: &str) -> bool {
    candidates.iter().any(|item| item.pinyin == code)
}

/// Wubi rows first, so wubi ranking and fixed positions keep precedence, then the quanpin rows whose word is not shown yet, in quanpin order. Every row keeps its producer's scheme: the session reads "answered by the pinyin fallback" and routes pins, removals and learning from those tags (overlays.md §3.3), so no list-level flag is kept here.
fn merge_pinyin_fallback(
    mut candidates: Vec<WordItem>,
    pinyin_rows: Vec<WordItem>,
) -> Vec<WordItem> {
    // Keep deduplication keys borrowed until the pinyin rows are ready to move into the result.
    let mut seen: HashSet<&str> = candidates.iter().map(|item| item.word.as_str()).collect();
    let unique = pinyin_rows
        .iter()
        .map(|item| seen.insert(item.word.as_str()))
        .collect::<Vec<_>>();
    drop(seen);
    candidates.extend(
        pinyin_rows
            .into_iter()
            .zip(unique)
            .filter_map(|(item, unique)| unique.then_some(item)),
    );
    candidates
}

/// With a double helpcode after a complete shuangpin base, the segmentations become the base's plus `'` and the two help letters (ime_session.cpp:15-39). The detector counts in delimiter-free space, so the split is made there too; slicing raw bytes would push a pinyin letter into the base and a manual `'` into the help codes.
fn apply_shuangpin_helpcode_segmentation(request: &mut QueryRequest, profile: &ShuangpinProfile) {
    const HELPCODE_LENGTH: usize = 2;
    if request.scheme != SchemeType::Shuangpin
        || !request.enable_shuangpin_helpcode
        || detect_active_double_helpcode_length(
            &request.raw_input,
            &request.raw_input_with_cases,
            profile,
        ) != HELPCODE_LENGTH
    {
        return;
    }
    let base_raw = trim_trailing_letters_preserve_delimiters(&request.raw_input, HELPCODE_LENGTH);
    let base_raw_with_cases =
        trim_trailing_letters_preserve_delimiters(&request.raw_input_with_cases, HELPCODE_LENGTH);
    let base_segmentation = segment_input(&base_raw, profile);
    let effective_with_cases = remove_manual_delimiters(&request.raw_input_with_cases);
    let help_codes = &effective_with_cases[effective_with_cases.len() - HELPCODE_LENGTH..];

    request.raw_segmentation = format!(
        "{}'{help_codes}",
        apply_segmentation_cases(&base_segmentation, &base_raw_with_cases)
    );
    request.normalized_segmentation = format!(
        "{}'{help_codes}",
        to_quanpin_segmentation(&base_segmentation, profile)
    );
    request
        .segmentation
        .clone_from(&request.normalized_segmentation);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wubi(code: &str, word: &str) -> WordItem {
        let mut item = WordItem::new(code, word, 10, CandidateSource::Database, "");
        item.scheme = SchemeType::Wubi;
        item
    }

    fn quanpin(pinyin: &str, word: &str) -> WordItem {
        WordItem::new(pinyin, word, 10, CandidateSource::Database, pinyin)
    }

    /// The session's `answered_by_pinyin_fallback` predicate over the merged rows.
    fn only_pinyin_rows(list: &[WordItem]) -> bool {
        !list.is_empty() && list.iter().all(|item| item.scheme != SchemeType::Wubi)
    }

    fn words(list: &[WordItem]) -> Vec<&str> {
        list.iter().map(|item| item.word.as_str()).collect()
    }

    #[test]
    fn only_a_whole_code_row_answers_the_table() {
        // wubi86 prefix rows: `wq` also returns wqb 爷 and wqbb 父子.
        let rows = [wubi("wq", "你"), wubi("wqb", "爷"), wubi("wqbb", "父子")];
        assert!(wubi_table_answered(&rows, "wq"));
        assert!(!wubi_table_answered(&rows[1..], "wq"));
        assert!(!wubi_table_answered(&[], "nihao"));
    }

    #[test]
    fn a_matched_code_keeps_its_wubi_rows_first() {
        // test_wubi_mixed_input_session.cpp:121-128 fixture: wubi wq 你好; ni'hao 你好 / 拟好.
        let list = merge_pinyin_fallback(
            vec![wubi("wq", "你好"), wubi("wqaa", "众人")],
            vec![quanpin("ni'hao", "你好"), quanpin("ni'hao", "拟好")],
        );
        assert_eq!(words(&list), vec!["你好", "众人", "拟好"]);
        assert_eq!(list[0].scheme, SchemeType::Wubi);
        assert_eq!(list[2].scheme, SchemeType::Quanpin);
        assert!(!only_pinyin_rows(&list));
    }

    #[test]
    fn an_unmatched_code_is_answered_by_quanpin_alone() {
        // test_wubi_mixed_input_session.cpp:106-116: `nihao` has no wubi row, so the list equals the quanpin list.
        let pinyin = vec![quanpin("ni'hao", "你好"), quanpin("ni'hao", "拟好")];
        let list = merge_pinyin_fallback(Vec::new(), pinyin.clone());
        assert_eq!(list, pinyin);
        assert!(only_pinyin_rows(&list));
    }

    #[test]
    fn nothing_at_all_is_not_a_fallback_answer() {
        let list = merge_pinyin_fallback(Vec::new(), Vec::new());
        assert!(list.is_empty());
        assert!(!only_pinyin_rows(&list));
    }

    #[test]
    fn quanpin_duplicates_among_themselves_collapse_to_the_first() {
        let list = merge_pinyin_fallback(
            vec![wubi("nihao", "妳")],
            vec![
                quanpin("ni'hao", "你好"),
                quanpin("ni'hao", "妳"),
                quanpin("ni'ha'o", "你好"),
            ],
        );
        assert_eq!(words(&list), vec!["妳", "你好"]);
        assert_eq!(list[1].pinyin, "ni'hao");
        assert!(!only_pinyin_rows(&list));
    }
}
