//! Selection and commit (core-session.md §5.7).

use super::chain::Pick;
use super::input::InputSession;
use crate::diagnostics;
use crate::error::Result;
use crate::pinyin::segment::{join_segments, split_segments};
use crate::pinyin::syllables::normalize_umlaut_aliases;
use crate::quanpin::QuanpinEngine;
use crate::text::{count_utf8_chars, first_han_char, last_han_char};
use crate::types::{
    CandidateEdge, KeyResult, LocalInputMode, PersonalDictionaryKind, SchemeType, WordItem,
};
use crate::user_dictionary::journal::is_user_inserted;

impl InputSession {
    pub(super) fn select_candidate(&mut self, index: usize) -> KeyResult {
        if let Some(result) = self.select_in_open_list(index) {
            return result;
        }
        if !self.has_composition() || index >= self.candidates().len() {
            return KeyResult::unhandled();
        }
        self.commit(index)
    }

    pub(super) fn select_candidate_edge(&mut self, index: usize, edge: CandidateEdge) -> KeyResult {
        // A row of an editor-owned list is a choice for the conversion, not text to commit, so its edge characters are not either.
        if let Some(result) = self.select_in_open_list(index) {
            return result;
        }
        if !self.has_composition() {
            return KeyResult::unhandled();
        }
        let Some(candidate) = self.candidates().get(index) else {
            return KeyResult::unhandled();
        };
        let character = match edge {
            CandidateEdge::FirstHan => first_han_char(&candidate.word),
            CandidateEdge::LastHan => last_han_char(&candidate.word),
        }
        .to_owned();
        if character.is_empty() {
            return KeyResult::unhandled();
        }
        // One character of a candidate is not what the composition spelled, so nothing is learned and the rest is dropped.
        self.reset_composition();
        self.chain.reset();
        KeyResult::committed(character)
    }

    /// Choosing a row of an openable list whose rows stay in the editor (Zhuyin): the row's text replaces its span of the conversion, the list closes and nothing is committed. Unhandled when there is no such row. `None` when no such list is open; the Korean Hanja list commits its row through `commit`.
    fn select_in_open_list(&mut self, index: usize) -> Option<KeyResult> {
        if !self.candidate_list_open() || !self.zhuyin_rules_apply() {
            return None;
        }
        let selected = self.engine.select_zhuyin(index);
        self.update_mixed_candidates();
        Some(match selected {
            Ok(true) => {
                self.online_requests.invalidate();
                KeyResult::handled()
            }
            Ok(false) => KeyResult::unhandled(),
            Err(error) => KeyResult::handled().with_diagnostic(Some(error.to_string())),
        })
    }

    /// Commit segment by segment until nothing is composing. An open Korean Hanja list is closed first, so finishing (punctuation, leaving the client, a host's finish key) commits the Hangul: only an explicit choice commits a Hanja.
    pub(super) fn finish_composition(&mut self, first_index: usize) -> KeyResult {
        self.close_korean_hanja();
        let mut result = KeyResult::unhandled();
        let mut index = first_index;
        while self.has_composition() {
            let part = self.commit(index);
            index = 0;
            result.handled = true;
            if let Some(text) = part.commit {
                result
                    .commit
                    .get_or_insert_with(String::new)
                    .push_str(&text);
            }
            if part.diagnostic.is_some() {
                result.diagnostic = part.diagnostic;
            }
        }
        // Hosts flush a composition when focus moves or they insert text themselves, so what follows is rarely the next word of the same phrase.
        self.chain.last_pick = None;
        result
    }

    /// input_session.cpp:828-956.
    pub(super) fn commit(&mut self, index: usize) -> KeyResult {
        // A selection made while a caret prefix is decoded leaves prefix mode: the rest of the composition decodes whole again.
        self.caret = None;
        // A Zhuyin commit is the converted text, whatever the list shows: its rows are choices inside the conversion, made through `select_in_open_list`. The pending syllable is dropped, nothing is learned, and a composition that converted nothing yet commits nothing.
        if self.zhuyin_rules_apply() {
            let text = self.engine.take_zhuyin_text();
            self.reset_composition();
            self.chain.reset();
            if text.is_empty() {
                return KeyResult::handled();
            }
            return KeyResult::committed(text);
        }
        let selected = self.candidates().get(index).cloned();
        // A Korean commit is the chosen Hanja or the Hangul itself, and nothing about it is learned: the rows are keyed by Dubeolsik letters, which every learning path below would read as pinyin. A Vietnamese commit is the displayed word, learned nowhere either.
        // A Cantonese commit is learned nowhere either. A row that covers only the leading syllables commits at once and the letters after it keep composing (`holds_phrase_progress` is false), so there is no phrase being built to hold.
        if self.cantonese_rules_apply() {
            let Some(selected) = selected else {
                let text = self.preedit();
                self.reset_composition();
                self.chain.reset();
                return KeyResult::committed(text);
            };
            if self.engine.select_cantonese(&selected) {
                self.update_mixed_candidates();
            } else {
                self.reset_composition();
            }
            self.chain.reset();
            return KeyResult::committed(selected.word);
        }
        // 笔画的提交不学习：候选来自只读的 `stroke.db`，键是笔画字母，任何学习路径都会把它当拼音写进用户词典。选中的字结束整个组合；没有候选时上屏键入的字母串，与 Enter 相同。
        if self.stroke_rules_apply() {
            let text =
                selected.map_or_else(|| self.engine.request().raw_input.clone(), |item| item.word);
            self.reset_composition();
            self.chain.reset();
            return KeyResult::committed(text);
        }
        // 藏文提交的是显示出来的藏文（不附加音节点），同样不学习。
        if self.korean_rules_apply() || self.vietnamese_rules_apply() || self.tibetan_rules_apply()
        {
            let text = selected.map_or_else(|| self.preedit(), |item| item.word);
            self.reset_composition();
            self.chain.reset();
            return KeyResult::committed(text);
        }
        let text = match &selected {
            Some(item) => Some(item.word.clone()),
            // The bare prefix letter of a temporary mode is a marker, not text.
            None if matches!(
                self.local_mode,
                LocalInputMode::TemporaryEnglish | LocalInputMode::TemporaryJapanese
            ) && self.local_preedit.len() == 1 =>
            {
                None
            }
            None => Some(self.preedit()),
        };
        let committed = |diagnostic: Option<String>| KeyResult {
            handled: true,
            commit: text.clone(),
            diagnostic,
        };

        let mut diagnostic = self
            .ranking_index(index)
            .and_then(|learn_index| self.learn_candidate(learn_index));

        let Some(selected) = selected else {
            self.record_context_into(&mut diagnostic, None, false, false);
            self.chain.last_pick = None;
            self.reset_composition();
            return committed(diagnostic);
        };
        let has_dictionary_reading = selected.source.is_dictionary();
        // Whole sentences from the lattice carry a canonical reading and join the phrase being composed like dictionary rows do.
        let has_generated_reading =
            selected.source.is_generated_or_fallback() && !selected.canonical_pinyin.is_empty();
        let follows_pinyin =
            self.candidates_follow_pinyin() || selected.scheme == SchemeType::Quanpin;
        if !((has_dictionary_reading || has_generated_reading)
            && self.local_mode == LocalInputMode::None
            && !self.dedicated_english
            && follows_pinyin)
        {
            self.record_context_into(&mut diagnostic, Some(&selected), false, false);
            self.chain.last_pick = None;
            self.reset_composition();
            return committed(diagnostic);
        }

        // A user row of three or more characters answering the whole composition is almost always a sentence stored by sentence learning; counting it as one word would teach the model a sentence as a word. User rows come back as Database like shipped ones, so the journal's insert record tells them apart; checked last because it reads a file.
        let learned_sentence_word = has_dictionary_reading
            && self.personal_context_applies()
            && !self.chain.same_composition
            && !selected.canonical_pinyin.is_empty()
            && count_utf8_chars(&selected.word) >= 3
            && self.selection_completes_composition(
                &selected.pinyin,
                &selected.word,
                selected.scheme,
            )
            && {
                let mut segments = split_segments(&selected.canonical_pinyin);
                normalize_umlaut_aliases(&mut segments);
                is_user_inserted(
                    &self.journal_path(),
                    PersonalDictionaryKind::Pinyin,
                    &join_segments(&segments),
                    &selected.word,
                )
            };
        let opens_composition = self.phrase_progress.word.is_empty();
        let transition = self.advance_composition_after_selection(
            &selected.pinyin,
            &selected.word,
            &selected.canonical_pinyin,
            selected.scheme,
        );
        self.record_context_into(
            &mut diagnostic,
            Some(&selected),
            learned_sentence_word,
            transition.continues_composition,
        );
        let pick = (has_dictionary_reading
            && !learned_sentence_word
            && !self.is_wubi()
            && !self.is_japanese()
            && self.learning_enabled
            && self.personal_context_enabled)
            .then(|| Pick {
                canonical_pinyin: transition.selected_canonical_pinyin.clone(),
                word: selected.word.clone(),
                committed_at: self.steady_now(),
            });
        // Recording the context has already ended the chain for a commit that does not follow the previous one.
        if opens_composition {
            if let (Some(current), Some(previous)) = (&pick, self.chain.last_pick.clone()) {
                if let Some(pair_diagnostic) = self.learn_pick_pair(&previous, current) {
                    diagnostic.get_or_insert(pair_diagnostic);
                }
            }
        }
        self.chain.last_pick = pick;
        // The rest of the input was re-queried before the chain moved on; order it against the word just committed.
        if transition.continues_composition && self.chain.previous.is_some() {
            self.update_mixed_candidates();
        }
        let progress = Self::update_creating_word_progress(
            &self.phrase_progress.pinyin,
            &self.phrase_progress.word,
            &selected.word,
            &transition,
        );
        if transition.continues_composition {
            self.phrase_progress = progress;
            self.discard_abandoned_phrase_progress();
            return committed(diagnostic);
        }
        if !self.phrase_progress.word.is_empty()
            && progress.can_store
            && self.learning_enabled
            && self
                .store_user_phrase_from_canonical_pinyin(&progress.pinyin, &progress.word)
                .is_err()
        {
            diagnostic = Some(diagnostics::PHRASE_NOT_PERSISTED.to_owned());
        }
        self.reset_composition();
        committed(diagnostic)
    }

    /// Chain bookkeeping for a nine-key commit: the grid bypasses this session, so the word after it must not read the word before it as its context. The committed text is not needed, since it only fed the dropped learning-undo ledger.
    pub(super) fn note_external_commit(&mut self) {
        self.reset_commit_context();
    }

    /// Store a phrase under a complete quanpin key. Quanpin and shuangpin share the canonical dictionary, so the key never goes back through a shuangpin profile (input_session_composition.cpp:522-536).
    pub(super) fn store_user_phrase_from_canonical_pinyin(
        &mut self,
        pinyin: &str,
        word: &str,
    ) -> Result<()> {
        self.canonical_phrase_engine()
            .create_word_from_canonical_pinyin(pinyin, word)
    }

    pub(super) fn canonical_phrase_engine(&mut self) -> &mut QuanpinEngine {
        let paths = &self.paths;
        self.canonical_phrase_engine
            .get_or_insert_with(|| QuanpinEngine::new(paths))
    }

    /// `record_personal_context`, with its diagnostic kept only when nothing earlier failed.
    fn record_context_into(
        &mut self,
        diagnostic: &mut Option<String>,
        selected: Option<&WordItem>,
        learned_sentence_word: bool,
        composition_left: bool,
    ) {
        if let Some(context_diagnostic) =
            self.record_personal_context(selected, learned_sentence_word, composition_left)
        {
            diagnostic.get_or_insert(context_diagnostic);
        }
    }
}
