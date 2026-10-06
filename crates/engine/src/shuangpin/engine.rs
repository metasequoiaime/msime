//! `ShuangpinEngine` (schemes-lang.md §1.6): double and single helpcode queries, the normal series query, and the fuzzy tail borrowed from quanpin.

use std::collections::HashSet;

use super::dictionary::ShuangpinDictionary;
use super::query::{
    detect_active_double_helpcode_length, raw_length_for_effective_prefix,
    remove_manual_delimiters, segment_input, to_quanpin_segmentation,
    trim_trailing_letters_preserve_delimiters,
};
use super::utils::{get_full_help_codes, is_all_complete_pinyin};
use super::ShuangpinProfile;
use crate::helpcode::{
    filter_candidates_with_double_helpcodes, reorder_candidates_with_single_helpcode,
    HelpcodeKeymap,
};
use crate::paths::RuntimePaths;
use crate::pinyin::segment::{join_segments, split_segments};
use crate::quanpin::QuanpinDictionary;
use crate::types::{CandidateSource, FuzzyPinyinOptions, QueryRequest, SchemeType, WordItem};

/// The base the dictionary queries and the codes that filter or reorder it.
struct HelpcodeQuery {
    base_pure: String,
    base_segmentation: String,
    help_codes: String,
}

/// engine.cpp:59-74.
fn full_helpcode_query(
    raw: &str,
    raw_with_cases: &str,
    profile: &ShuangpinProfile,
) -> Option<HelpcodeQuery> {
    if detect_active_double_helpcode_length(raw, raw_with_cases, profile) != 2 {
        return None;
    }
    let base_raw = trim_trailing_letters_preserve_delimiters(raw, 2);
    Some(HelpcodeQuery {
        base_pure: remove_manual_delimiters(&base_raw),
        base_segmentation: segment_input(&base_raw, profile),
        help_codes: get_full_help_codes(&remove_manual_delimiters(raw_with_cases)),
    })
}

/// engine.cpp:76-109.
fn single_helpcode_query(
    raw: &str,
    pure_with_cases: &str,
    profile: &ShuangpinProfile,
) -> Option<HelpcodeQuery> {
    let length = pure_with_cases.len();
    if length <= 1 || length.is_multiple_of(2) {
        return None;
    }
    // A `'` right before the last letter makes that letter a user-delimited pinyin segment, not a helpcode; the composition layer reads `ui'u` the same way.
    let raw_prefix = raw_length_for_effective_prefix(raw, length - 1);
    if raw.as_bytes().get(raw_prefix) == Some(&b'\'') {
        return None;
    }
    let base_raw = trim_trailing_letters_preserve_delimiters(raw, 1);
    let base_pure = remove_manual_delimiters(&base_raw);
    let base_segmentation = segment_input(&base_raw, profile);
    if raw.contains('\'') && !base_segmentation.split('\'').all(|part| part.len() == 2) {
        return None;
    }
    if !is_all_complete_pinyin(&base_pure, &base_segmentation) {
        return None;
    }
    Some(HelpcodeQuery {
        base_pure,
        base_segmentation,
        // The case is kept: uppercase means "prefer the last character".
        help_codes: pure_with_cases[length - 1..].to_string(),
    })
}

/// The active helpcode reading of a request, double first; `None` reads the input as plain pinyin (`cls` is `c'ls`, engine.cpp:162).
fn active_helpcode_query(
    request: &QueryRequest,
    profile: &ShuangpinProfile,
) -> Option<HelpcodeQuery> {
    if !request.enable_shuangpin_helpcode {
        return None;
    }
    let raw = &request.raw_input;
    let raw_with_cases = if request.raw_input_with_cases.is_empty() {
        raw
    } else {
        &request.raw_input_with_cases
    };
    full_helpcode_query(raw, raw_with_cases, profile)
        .or_else(|| single_helpcode_query(raw, &remove_manual_delimiters(raw_with_cases), profile))
}

pub struct ShuangpinEngine {
    profile: &'static ShuangpinProfile,
    dictionary: ShuangpinDictionary,
    paths: RuntimePaths,
    /// Opened on the first fuzzy query only.
    fuzzy_dictionary: Option<QuanpinDictionary>,
}

impl ShuangpinEngine {
    pub fn new(profile: &'static ShuangpinProfile, paths: &RuntimePaths) -> Self {
        Self {
            profile,
            dictionary: ShuangpinDictionary::new(profile, paths),
            paths: paths.clone(),
            fuzzy_dictionary: None,
        }
    }

    /// :122-167.
    pub fn query(
        &mut self,
        request: &QueryRequest,
        keymap: Option<&HelpcodeKeymap>,
    ) -> Vec<WordItem> {
        if !request.valid {
            return Vec::new();
        }
        self.dictionary
            .set_sentence_alternatives(request.sentence_alternatives);
        self.dictionary
            .set_sentence_association(request.sentence_association);
        self.dictionary
            .set_rescoring_context(&request.rescoring_context);

        let raw = &request.raw_input;
        if remove_manual_delimiters(raw).is_empty() {
            return Vec::new();
        }
        // Without a loaded table nothing matches a helpcode, which is what the reference's null keymap did.
        let empty_keymap = HelpcodeKeymap::default();
        let keymap = keymap.unwrap_or(&empty_keymap);

        if let Some(helpcode) = active_helpcode_query(request, self.profile) {
            let exact = self.dictionary.generate_with_helpcodes(
                &helpcode.base_pure,
                &helpcode.base_segmentation,
                raw,
                &helpcode.help_codes,
                keymap,
            );
            return self.append_fuzzy(
                exact,
                &helpcode.base_segmentation,
                request.fuzzy_pinyin,
                &helpcode.help_codes,
                keymap,
            );
        }

        let segmentation = segment_input(raw, self.profile);
        let exact =
            self.dictionary
                .generate_series(&remove_manual_delimiters(raw), &segmentation, raw);
        self.append_fuzzy(exact, &segmentation, request.fuzzy_pinyin, "", keymap)
    }

    /// Quanpin fuzzy rows for the decoded segmentation, labelled with the shuangpin keys they cover, filtered or reordered by the helpcodes, appended uniquely by word, and the whole list stable-sorted by typed coverage (engine.cpp:227-258).
    fn append_fuzzy(
        &mut self,
        mut exact: Vec<WordItem>,
        raw_segmentation: &str,
        options: FuzzyPinyinOptions,
        help_codes: &str,
        keymap: &HelpcodeKeymap,
    ) -> Vec<WordItem> {
        if options.rules == 0 {
            return exact;
        }
        let paths = &self.paths;
        let fuzzy_dictionary = self
            .fuzzy_dictionary
            .get_or_insert_with(|| QuanpinDictionary::new(paths));
        let typed = split_segments(raw_segmentation);
        let mut fuzzy = fuzzy_dictionary.fuzzy_candidates(
            &to_quanpin_segmentation(raw_segmentation, self.profile),
            options,
        );
        for item in &mut fuzzy {
            let count = split_segments(&item.pinyin).len();
            if count <= typed.len() {
                item.pinyin = join_segments(&typed[..count]);
            }
        }
        let fuzzy = match help_codes.len() {
            2 => filter_candidates_with_double_helpcodes(fuzzy, help_codes, keymap),
            1 => reorder_candidates_with_single_helpcode(fuzzy, help_codes, keymap),
            _ => fuzzy,
        };
        // Keep deduplication keys borrowed until fuzzy rows are ready to move into the exact list.
        let mut seen: HashSet<&str> = exact.iter().map(|item| item.word.as_str()).collect();
        let unique = fuzzy
            .iter()
            .map(|item| seen.insert(item.word.as_str()))
            .collect::<Vec<_>>();
        drop(seen);
        let unique_count = unique.iter().filter(|&&is_unique| is_unique).count();
        exact.reserve(unique_count);
        exact.extend(
            fuzzy
                .into_iter()
                .zip(unique)
                .filter_map(|(item, unique)| unique.then_some(item)),
        );
        exact.sort_by_key(|item| std::cmp::Reverse(item.pinyin.len()));
        exact
    }

    /// Only when the first segment is one letter (:169-185).
    pub fn expand_initial_candidates(
        &mut self,
        request: &QueryRequest,
        candidates: &mut Vec<WordItem>,
    ) -> bool {
        if request.scheme != SchemeType::Shuangpin {
            return false;
        }
        let segmentation = segment_input(&request.raw_input, self.profile);
        let initial = segmentation.split('\'').next().unwrap_or_default();
        if initial.len() != 1 {
            return false;
        }
        self.dictionary
            .expand_initial_candidates(initial, candidates, &request.raw_input)
    }

    /// Routes online rows to the cache the request's own query read: the double-helpcode entry for its codes, the single-helpcode entries, or the series entry (pinyin_candidate_provider.cpp:186-210). The provider re-derived the mode with looser rules than the query and, for a batch, dropped the double codes, so a batch could land in an entry no query reads; this uses the query's decision (overlays.md §5.1).
    pub fn insert_online_words(
        &mut self,
        request: &QueryRequest,
        words: &[String],
        source: CandidateSource,
    ) -> bool {
        if !request.valid {
            return false;
        }
        // With a reranker loaded the context is part of the cache key, so the rows must go under the context this request was queried with.
        self.dictionary
            .set_rescoring_context(&request.rescoring_context);
        match active_helpcode_query(request, self.profile) {
            Some(helpcode) => {
                let double = if helpcode.help_codes.len() == 2 {
                    helpcode.help_codes.as_str()
                } else {
                    ""
                };
                self.dictionary.insert_word_to_active_helpcode_cache(
                    &request.raw_input,
                    words,
                    source,
                    double,
                )
            }
            None => self
                .dictionary
                .insert_word_to_series_cache(&request.raw_input, words, source),
        }
    }

    pub fn find_candidate(&self, key: &str, value: &str) -> Option<WordItem> {
        self.dictionary.find_candidate(key, value)
    }

    pub fn reset_cache(&mut self) {
        self.dictionary.reset_cache();
        if let Some(fuzzy_dictionary) = &mut self.fuzzy_dictionary {
            fuzzy_dictionary.reset_cache();
        }
    }
}
