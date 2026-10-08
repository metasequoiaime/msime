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

// 双拼模糊候选的短合并直接扫描已有词，避免临时哈希表和重复索引分配。
const SMALL_FUZZY_DEDUP: usize = 64;

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
            let count = segment_count(&item.pinyin);
            if count <= typed.len() {
                item.pinyin = join_segments(&typed[..count]);
            }
        }
        let fuzzy = match help_codes.len() {
            2 => filter_candidates_with_double_helpcodes(fuzzy, help_codes, keymap),
            1 => reorder_candidates_with_single_helpcode(fuzzy, help_codes, keymap),
            _ => fuzzy,
        };
        if fuzzy.is_empty() {
            exact.sort_by_key(|item| std::cmp::Reverse(item.pinyin.len()));
            return exact;
        }
        append_fuzzy_rows(&mut exact, fuzzy);
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

    pub fn clear_online_candidates(&mut self, source: CandidateSource) {
        self.dictionary.clear_online_candidates(source);
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

/// 只需段数时直接统计分隔符，避免为每个模糊候选复制音节字符串。
fn segment_count(segmentation: &str) -> usize {
    if segmentation.is_empty() {
        0
    } else {
        segmentation.bytes().filter(|&byte| byte == b'\'').count() + 1
    }
}

fn append_fuzzy_rows(exact: &mut Vec<WordItem>, fuzzy: Vec<WordItem>) {
    if exact.len().saturating_add(fuzzy.len()) <= SMALL_FUZZY_DEDUP {
        exact.reserve(fuzzy.len());
        for item in fuzzy {
            if exact.iter().any(|existing| existing.word == item.word) {
                continue;
            }
            exact.push(item);
        }
        return;
    }
    // 借用词面计算重复项，释放集合后再移动整行，避免移动时仍持有借用。
    let mut seen: HashSet<&str> = exact.iter().map(|item| item.word.as_str()).collect();
    let duplicates = fuzzy
        .iter()
        .enumerate()
        .filter_map(|(index, item)| (!seen.insert(item.word.as_str())).then_some(index))
        .collect::<Vec<_>>();
    drop(seen);
    let unique_count = fuzzy.len() - duplicates.len();
    exact.reserve(unique_count);
    let mut duplicates = duplicates.into_iter().peekable();
    exact.extend(fuzzy.into_iter().enumerate().filter_map(|(index, item)| {
        if duplicates.peek() == Some(&index) {
            duplicates.next();
            None
        } else {
            Some(item)
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::{append_fuzzy_rows, segment_count};
    use crate::types::{CandidateSource, WordItem};

    fn row(word: &str) -> WordItem {
        WordItem::new("ni", word, 1, CandidateSource::Database, "ni")
    }

    #[test]
    fn segment_count_does_not_allocate_for_fuzzy_row_relabeling() {
        let (count, allocations) =
            crate::ime::personal_rerank::allocations::count(|| segment_count("ni'hao'jie"));

        assert_eq!(count, 3);
        assert_eq!(allocations, 0);
        assert_eq!(segment_count(""), 0);
        assert_eq!(segment_count("ni"), 1);
        for input in ["'", "ni''hao'", "'ni", "你'好", "a'a'a'a'a"] {
            assert_eq!(
                segment_count(input),
                crate::pinyin::segment::split_segments(input).len()
            );
        }
    }

    #[test]
    fn short_fuzzy_rows_are_appended_without_temporary_heap_state() {
        let mut exact = Vec::with_capacity(8);
        exact.push(row("你"));
        let fuzzy = vec![row("你"), row("好"), row("好"), row("吗")];
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            append_fuzzy_rows(&mut exact, fuzzy);
        });

        assert_eq!(allocations, 0);
        assert_eq!(
            exact
                .iter()
                .map(|item| item.word.as_str())
                .collect::<Vec<_>>(),
            ["你", "好", "吗"]
        );
    }

    #[test]
    fn large_fuzzy_rows_keep_first_occurrence_order() {
        let mut exact = Vec::with_capacity(70);
        exact.push(row("已有"));
        let fuzzy = (0..65)
            .map(|index| row(if index == 0 { "已有" } else { "模糊" }))
            .collect();

        append_fuzzy_rows(&mut exact, fuzzy);

        assert_eq!(
            exact
                .iter()
                .map(|item| item.word.as_str())
                .collect::<Vec<_>>(),
            ["已有", "模糊"]
        );
    }
}
