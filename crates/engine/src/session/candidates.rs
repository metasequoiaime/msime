//! The candidate merge pipeline and candidate actions (core-session.md §7.2-§7.3): personal rerank, mixed rows, fixed positions (split per producer in mixed wubi), pin, removal and position fixing.

use std::borrow::Cow;

use super::input::InputSession;
use crate::assets;
use crate::diagnostics;
use crate::ime::personal_rerank::personal_context_rerank;
use crate::ime::queries::MODE_ENGLISH_LIMIT;
use crate::local::date_time::LocalDateTime;
use crate::local::jianpin::jianpin_ranking_context;
use crate::pinyin::segment::{cut_pinyin_by_mode, join_segments, CutMode};
use crate::text::count_utf8_chars;
use crate::types::{
    CandidateSource, FrequencyAdjustmentMode, FrequencyAdjustmentOptions, KeyResult,
    LocalInputMode, PersonalDictionaryKind, SchemeType, WordItem,
};
use crate::user_dictionary::positions::{
    apply_fixed_positions, clear_fixed_position, is_pinned_candidate, record_pinned_candidate,
    set_fixed_position,
};
use crate::user_dictionary::removal::delete_dictionary_candidate;

fn english_position_context(input: &str) -> String {
    let mut context = String::with_capacity("english:".len() + input.len());
    context.push_str("english:");
    context.extend(
        input
            .chars()
            .map(|character| character.to_ascii_lowercase()),
    );
    context
}

impl InputSession {
    /// Prefix or engine rows, then personal context rerank, mixed English / emoji / kaomoji, fixed positions (input_session.cpp:1057-1078).
    pub(super) fn update_mixed_candidates(&mut self) {
        self.refresh_prefix_candidates();
        let decoded = if self.prefix_active {
            self.prefix_candidates.clone()
        } else {
            self.engine.candidates().to_vec()
        };
        self.personal_reranked = false;
        self.ranking_candidates = None;
        // At the chain start the preference is context-free, which is the frequency setting's business, not this one's.
        let reordered = match self.chain.previous.as_deref() {
            Some(previous) if self.personal_context_applies() => {
                let journal = self.journal_path();
                let context = self.pinyin_ranking_context();
                let model = self.personal_context.model();
                personal_context_rerank(
                    &decoded,
                    &model,
                    self.chain.earlier.as_deref(),
                    previous,
                    &mut |leader: &str| is_pinned_candidate(&journal, &context, leader),
                )
            }
            _ => None,
        };
        match reordered {
            Some(reordered) => {
                self.mixed_candidates = self.mixed_from(reordered);
                // Learning ranks against the order the dictionary gave, so the unreordered list is kept beside the shown one.
                self.ranking_candidates = Some(self.mixed_from(decoded));
                self.personal_reranked = true;
            }
            None => self.mixed_candidates = self.mixed_from(decoded),
        }
    }

    /// The candidates learning ranks against: the list before the personal rerank.
    pub(super) fn ranking_list(&self) -> &[WordItem] {
        self.ranking_candidates
            .as_deref()
            .unwrap_or_else(|| self.candidates())
    }

    pub(super) fn update_local_candidates(&mut self) -> Option<String> {
        // Only the date/time and command modes read the wall clock; the other modes would pay a local-offset lookup per key for nothing.
        let now = if matches!(
            self.local_mode,
            LocalInputMode::DateTime | LocalInputMode::Command
        ) {
            self.local_now()
        } else {
            LocalDateTime::default()
        };
        let scheme = self.scheme();
        let result = self.queries.local(
            self.local_mode,
            &self.local_preedit,
            scheme,
            &now,
            self.engine.candidates(),
        );
        let mut candidates = result.candidates;
        self.apply_candidate_positions(&mut candidates);
        self.local_candidates = candidates;
        // After the positions, so the synthetic row is never moved.
        self.add_local_fallback_candidate();
        result.diagnostic
    }

    pub(super) fn add_local_fallback_candidate(&mut self) {
        // An incomplete or unmatched special-mode input shows its raw text, prefix letter included, so Space commits it through the ordinary selection path (overlays.md §8.1).
        if self.local_candidates.is_empty() && !self.local_preedit.is_empty() {
            self.local_candidates.push(WordItem::new(
                self.local_preedit.clone(),
                self.local_preedit.clone(),
                1,
                CandidateSource::Fallback,
                "",
            ));
        }
    }

    pub(super) fn update_dedicated_english_candidates(&mut self) {
        self.dedicated_english_candidates.clear();
        if self.dedicated_english_preedit.is_empty() {
            return;
        }
        let prefix = self.dedicated_english_preedit.to_ascii_lowercase();
        let mut candidates = self
            .queries
            .english_dictionary()
            .query_prefix(&prefix, MODE_ENGLISH_LIMIT);
        self.apply_candidate_positions(&mut candidates);
        if candidates.is_empty() {
            // A word the dictionary does not know is still what the user is typing.
            candidates.push(WordItem::new(
                "",
                self.dedicated_english_preedit.clone(),
                0,
                CandidateSource::Generated,
                "",
            ));
        }
        self.dedicated_english_candidates = candidates;
    }

    /// input_session_candidates.cpp:19-61 with the per-producer split of overlays.md §3.3.
    pub(super) fn apply_candidate_positions(&mut self, items: &mut Vec<WordItem>) {
        if items.is_empty() {
            return;
        }
        let journal = self.journal_path();
        let regular = self.local_mode == LocalInputMode::None
            && !self.dedicated_english
            && !matches!(
                self.scheme(),
                SchemeType::JapaneseRomaji | SchemeType::Korean
            );
        let include_missing = self.engine.request().raw_input.len() == 1;
        let keep_dynamic = self.has_active_helpcode();
        let engine = &self.engine;
        if regular && self.is_wubi() {
            // Each producer's rows are fixed within their own group and under their own context, and the groups keep the wubi-first order.
            let (mut wubi_items, mut pinyin_items): (Vec<WordItem>, Vec<WordItem>) =
                items.drain(..).partition(Self::is_wubi_native_candidate);
            if !wubi_items.is_empty() {
                let context = self.position_context(false, true);
                let mut finder =
                    |key: &str, word: &str| engine.find_candidate(SchemeType::Wubi, key, word);
                apply_fixed_positions(
                    &journal,
                    context.as_ref(),
                    &mut wubi_items,
                    include_missing,
                    Some(&mut finder),
                    keep_dynamic,
                );
            }
            if !pinyin_items.is_empty() {
                let context = self.position_context(false, false);
                let mut finder =
                    |key: &str, word: &str| engine.find_candidate(SchemeType::Quanpin, key, word);
                apply_fixed_positions(
                    &journal,
                    context.as_ref(),
                    &mut pinyin_items,
                    include_missing,
                    Some(&mut finder),
                    keep_dynamic,
                );
            }
            items.append(&mut wubi_items);
            items.append(&mut pinyin_items);
        } else if regular {
            let context = self.position_context(false, false);
            let scheme = self.scheme();
            let mut finder = |key: &str, word: &str| engine.find_candidate(scheme, key, word);
            apply_fixed_positions(
                &journal,
                context.as_ref(),
                items,
                include_missing,
                Some(&mut finder),
                keep_dynamic,
            );
        } else if self.local_mode == LocalInputMode::SuperJianpin {
            let context = self.position_context(false, false);
            apply_fixed_positions(&journal, context.as_ref(), items, false, None, false);
        }
        if items
            .iter()
            .any(|item| item.source == CandidateSource::EnglishDictionary)
        {
            let context = self.position_context(true, false);
            apply_fixed_positions(&journal, context.as_ref(), items, false, None, true);
        }
    }

    /// input_session_candidates.cpp:21-44.
    pub(super) fn position_context(&self, english: bool, wubi: bool) -> Cow<'_, str> {
        let request = self.engine.request();
        if english {
            let input = if self.dedicated_english {
                &self.dedicated_english_preedit
            } else if self.local_mode == LocalInputMode::TemporaryEnglish {
                &self.local_preedit[1..]
            } else {
                &request.raw_input_with_cases
            };
            return Cow::Owned(english_position_context(input));
        }
        if self.local_mode == LocalInputMode::SuperJianpin {
            return Cow::Owned(jianpin_ranking_context(
                &self.local_preedit[1..],
                self.scheme(),
                self.shuangpin_profile(),
            ));
        }
        if wubi {
            return Cow::Borrowed(&request.raw_input);
        }
        let context: Cow<'_, str> = if request.normalized_input.is_empty() {
            Cow::Owned(self.pinyin_segmentation())
        } else {
            Cow::Borrowed(&request.normalized_input)
        };
        if request.raw_input.len() == 1 {
            return context;
        }
        // Positions are stored under one canonical cut, so the same letters typed with or without apostrophes share them.
        let plain = if context.contains('\'') {
            Cow::Owned(context.chars().filter(|c| *c != '\'').collect())
        } else {
            Cow::Borrowed(context.as_ref())
        };
        match cut_pinyin_by_mode(&plain, CutMode::Correction).first() {
            Some(cut) => Cow::Owned(join_segments(cut)),
            None => context,
        }
    }

    /// The pre-rerank index of a displayed candidate.
    pub(super) fn ranking_index(&self, displayed: usize) -> Option<usize> {
        let shown = self.candidates().get(displayed)?;
        let Some(ordered) = self.ranking_candidates.as_deref() else {
            return Some(displayed);
        };
        let found = ordered.iter().position(|item| {
            item.word == shown.word
                && item.source == shown.source
                && item.pinyin == shown.pinyin
                && item.canonical_pinyin == shown.canonical_pinyin
        });
        Some(found.unwrap_or(displayed))
    }

    /// 0 clears.
    pub(super) fn set_candidate_position(&mut self, index: usize, position: i32) -> KeyResult {
        if !(0..=5).contains(&position) {
            return KeyResult::unhandled();
        }
        let Some(selected) = self.candidates().get(index).cloned() else {
            return KeyResult::unhandled();
        };
        if !self.is_editable_source(&selected) {
            return KeyResult::unhandled();
        }
        let english = selected.source == CandidateSource::EnglishDictionary;
        let wubi = Self::is_wubi_native_candidate(&selected)
            && self.local_mode != LocalInputMode::SuperJianpin;
        let context = self.position_context(english, wubi);
        let key = if english || wubi || selected.canonical_pinyin.is_empty() {
            &selected.pinyin
        } else {
            &selected.canonical_pinyin
        };
        if context.is_empty() || key.is_empty() {
            return KeyResult::unhandled();
        }
        let journal = self.journal_path();
        let written = if position == 0 {
            clear_fixed_position(&journal, context.as_ref(), key, &selected.word)
        } else {
            set_fixed_position(&journal, context.as_ref(), key, &selected.word, position)
        };
        if written.is_err() {
            return KeyResult::handled()
                .with_diagnostic(Some(diagnostics::POSITION_NOT_PERSISTED.to_owned()));
        }
        KeyResult::handled().with_diagnostic(self.refresh_after_candidate_change())
    }

    pub(super) fn pin_candidate(&mut self, index: usize) -> KeyResult {
        let Some(index) = self.ranking_index(index) else {
            return KeyResult::unhandled();
        };
        let selected = self.ranking_list()[index].clone();
        if !self.is_editable_source(&selected) {
            return KeyResult::unhandled();
        }
        // Manual pinning is independent of the automatic learning preferences and never selects text.
        let pin = FrequencyAdjustmentOptions {
            mode: FrequencyAdjustmentMode::Pin,
            trigger_count: 1,
            linear_step: 1,
        };
        if let Some(diagnostic) = self.adjust_candidate_frequency(index, pin, true) {
            return KeyResult::handled().with_diagnostic(Some(diagnostic));
        }
        // Remembered so that the personal context rerank does not move the pinned word off the top it was pinned to.
        if selected.source != CandidateSource::EnglishDictionary
            && self.local_mode == LocalInputMode::None
            && !self.dedicated_english
            && self.scheme().is_pinyin()
            && record_pinned_candidate(
                &self.journal_path(),
                &self.pinyin_ranking_context(),
                &selected.word,
            )
            .is_err()
        {
            return KeyResult::handled()
                .with_diagnostic(Some(diagnostics::PIN_NOT_PERSISTED.to_owned()));
        }
        KeyResult::handled().with_diagnostic(self.refresh_after_candidate_change())
    }

    pub(super) fn remove_candidate(&mut self, index: usize) -> KeyResult {
        let Some(selected) = self.candidates().get(index).cloned() else {
            return KeyResult::unhandled();
        };
        let english = selected.source == CandidateSource::EnglishDictionary;
        // A single character is protected: removing it would leave its reading unanswerable.
        if !self.is_editable_source(&selected)
            || (!english && count_utf8_chars(&selected.word) <= 1)
        {
            return KeyResult::unhandled();
        }
        let wubi = Self::is_wubi_native_candidate(&selected)
            && self.local_mode != LocalInputMode::SuperJianpin;
        let kind = if english {
            PersonalDictionaryKind::English
        } else if wubi {
            PersonalDictionaryKind::Wubi
        } else {
            PersonalDictionaryKind::Pinyin
        };
        // A displayed pinyin row already carries its exact dictionary key; re-segmenting it could delete another pronunciation of the same word.
        let key = if english || wubi {
            &selected.pinyin
        } else {
            &selected.canonical_pinyin
        };
        if key.is_empty() {
            return KeyResult::unhandled();
        }
        let dictionary = self.paths.dictionary(if english {
            assets::ENGLISH_DICTIONARY
        } else {
            assets::MAIN_DICTIONARY
        });
        if delete_dictionary_candidate(&dictionary, &self.journal_path(), kind, key, &selected.word)
            .is_err()
        {
            return KeyResult::handled()
                .with_diagnostic(Some(diagnostics::REMOVAL_NOT_PERSISTED.to_owned()));
        }
        KeyResult::handled().with_diagnostic(self.refresh_after_candidate_change())
    }

    /// Caches reset and the active view rebuilt after a dictionary edit (input_session.cpp:1221-1235).
    fn refresh_after_candidate_change(&mut self) -> Option<String> {
        self.reset_cache();
        if self.dedicated_english {
            self.update_dedicated_english_candidates();
            return None;
        }
        if self.local_mode != LocalInputMode::None {
            return self.update_local_candidates();
        }
        self.recompute_candidates();
        None
    }

    /// Mixed rows and fixed positions over one decoded list.
    fn mixed_from(&mut self, decoded: Vec<WordItem>) -> Vec<WordItem> {
        let association_input = if self.prefix_active {
            self.prefix_query_input.clone()
        } else {
            self.engine.request().raw_input.clone()
        };
        let scheme = self.scheme();
        let mut mixed = self.queries.mixed(
            decoded,
            &association_input,
            scheme,
            self.english_options,
            self.expressive_options,
            self.dedicated_english,
            self.local_mode,
        );
        self.apply_candidate_positions(&mut mixed);
        mixed
    }
}

#[cfg(test)]
mod tests {
    use super::english_position_context;

    #[test]
    fn english_position_context_preserves_non_ascii_while_lowercasing_ascii() {
        assert_eq!(english_position_context("HeLLo 世界"), "english:hello 世界");
    }
}
