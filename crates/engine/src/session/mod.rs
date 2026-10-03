//! The public `Session` (`include/metasequoia/session.h`, core-session.md §4): an input session plus the nine-key session, with the routing rules between them. One host serialises calls to its session; distinct sessions may run concurrently.
//!
//! The setters the product only sets at construction (`switch_scheme`, `set_helpcode_schema`, `set_helpcode_enabled`, `set_wubi_mixed_pinyin`, `set_personal_context_enabled`) and `candidate_key` stay public because the golden scenarios drive them. Learning undo (`set_learning_undo_enabled`, `forget_recent_commits`) is dropped.

mod candidates;
mod chain;
mod clock;
mod commit;
mod composition;
mod editing;
mod input;
mod learning;
mod online;
pub mod options;
#[cfg(test)]
mod tests;

use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::nine_key::NineKeySession;
use crate::types::{
    CandidateEdge, CandidateSource, Command, CommandTableEntry, CommandTranslationQuery, KeyResult,
    LocalInputMode, MentionEntry, OnlineQuery, QuickPhraseEntry, SchemeType,
};

pub use clock::Clock;
use input::InputSession;
pub use options::{SessionOptions, SessionSnapshot};

pub struct Session {
    input: InputSession,
    nine_key: NineKeySession,
    nine_key_enabled: bool,
    shuangpin_preedit_uses_raw: bool,
}

impl Session {
    /// Validates the paths and every option (`INVALID_SESSION_OPTIONS`, `UNKNOWN_HELPCODE_SCHEMA`), loads the helpcode table and applies the options in the reference order (session.cpp:11-30, 57-66).
    pub fn new(options: SessionOptions) -> Result<Session> {
        options.paths.validate()?;
        let input = InputSession::new(&options)?;
        let nine_key = NineKeySession::new(
            &options.paths,
            options.learning,
            options.frequency,
            options.fuzzy_pinyin,
            options.english,
        );
        Ok(Session {
            input,
            nine_key,
            nine_key_enabled: false,
            shuangpin_preedit_uses_raw: options.shuangpin_preedit_uses_raw,
        })
    }

    /// Replace the steady and local clocks, so tests can cross the 3 s / 8 s / 10 s personal-learning windows and the date/time mode can be pinned.
    pub fn set_clock(&mut self, clock: Clock) {
        self.input.clock = clock;
    }

    /// One ASCII character; `shift_only` is a bare Shift+letter (local mode entry). Digits 2-9 go to the nine-key session while it is enabled and nothing else is composing.
    pub fn character(&mut self, value: u8, shift_only: bool) -> KeyResult {
        // English is a mode rather than a scheme, so the grid stays available in it: the same digits spell words instead of syllables. A local mode still takes the keys, and the scheme underneath must be quanpin, the only one whose syllables the grid knows.
        if self.nine_key_enabled
            && self.input.scheme().nine_key()
            && self.input.local_mode == LocalInputMode::None
            && self.input.preedit().is_empty()
            && (b'2'..=b'9').contains(&value)
        {
            let result = self.nine_key.character(value);
            return self.after_nine_key(result);
        }
        if self.nine_key.active() {
            return KeyResult::unhandled();
        }
        self.input.handle_character(value, shift_only)
    }

    /// Cancels any nine-key digits first. Call with nothing composing when the keyboard layout changes.
    pub fn set_nine_key_enabled(&mut self, enabled: bool) {
        self.nine_key.command(Command::Cancel);
        self.nine_key_enabled = enabled;
        self.nine_key
            .set_english_only(enabled && self.input.dedicated_english);
    }

    pub fn choose_nine_key_spelling(&mut self, index: usize) -> KeyResult {
        let result = self.nine_key.choose_spelling(index);
        self.after_nine_key(result)
    }

    pub fn command(&mut self, command: Command) -> KeyResult {
        if self.nine_key.active() {
            let result = self.nine_key.command(command);
            return self.after_nine_key(result);
        }
        self.input.handle_command(command)
    }

    /// `1`..`9` select the first nine candidates.
    pub fn candidate_key(&mut self, value: u8) -> KeyResult {
        if self.nine_key.active() {
            if !(b'1'..=b'9').contains(&value) {
                return KeyResult::unhandled();
            }
            let result = self.nine_key.select(usize::from(value - b'1'));
            return self.after_nine_key(result);
        }
        self.input.handle_candidate_key(value)
    }

    /// Finishes the composition and appends the Chinese mark; unhandled when Chinese punctuation is off or locked to ASCII, or the key has no mark.
    pub fn punctuation(&mut self, value: u8) -> KeyResult {
        if !self.nine_key.active() {
            return self.input.handle_punctuation(value);
        }
        let mut result = self.input.handle_punctuation(value);
        if !result.handled {
            return result;
        }
        // The grid's text goes before the mark the input session translated.
        let composition = self.nine_key.finish(0);
        if composition.commit.is_some() {
            self.input.reset_commit_context();
        }
        let mut text = composition.commit.unwrap_or_default();
        text.push_str(result.commit.as_deref().unwrap_or_default());
        result.commit = Some(text);
        result
    }

    /// Live; keeps the composition, caret and pairing state.
    pub fn set_chinese_punctuation_enabled(&mut self, enabled: bool) {
        self.input.chinese_punctuation_enabled = enabled;
    }

    /// `INVALID_PUNCTUATION_LOCK` outside 0..=2.
    pub fn set_punctuation_lock(&mut self, lock: i32) -> Result<()> {
        if !(0..=2).contains(&lock) {
            return Err(EngineError::invalid(diagnostics::INVALID_PUNCTUATION_LOCK));
        }
        self.input.punctuation_lock = lock;
        Ok(())
    }

    pub fn set_paired_punctuation_enabled(&mut self, enabled: bool) {
        self.input.punctuation.set_paired_enabled(enabled);
    }

    /// The host emitted the closing half of a paired mark itself.
    pub fn balance_paired_punctuation_after_auto_close(&mut self, opening: u8) {
        self.input.punctuation.balance_after_auto_close(opening);
    }

    pub fn select(&mut self, index: usize) -> KeyResult {
        if self.nine_key.active() {
            let result = self.nine_key.select(index);
            return self.after_nine_key(result);
        }
        self.input.select_candidate(index)
    }

    /// Commit the first or last Han character of a candidate and drop the rest of the composition.
    pub fn select_edge(&mut self, index: usize, edge: CandidateEdge) -> KeyResult {
        if self.nine_key.active() {
            return KeyResult::unhandled();
        }
        self.input.select_candidate_edge(index, edge)
    }

    /// Promote a dictionary candidate without committing; unsupported rows are unhandled, persistence failures carry a diagnostic.
    pub fn pin(&mut self, index: usize) -> KeyResult {
        if self.nine_key.active() {
            return self.nine_key.pin(index);
        }
        self.input.pin_candidate(index)
    }

    /// Remove a dictionary phrase without committing; single-character non-English rows are protected.
    pub fn remove(&mut self, index: usize) -> KeyResult {
        if self.nine_key.active() {
            return self.nine_key.remove(index);
        }
        self.input.remove_candidate(index)
    }

    /// Fix a dictionary candidate to slot 1..=5 in this input context; unhandled for any other slot.
    pub fn fix_position(&mut self, index: usize, position: i32) -> KeyResult {
        if !(1..=5).contains(&position) {
            return KeyResult::unhandled();
        }
        if self.nine_key.active() {
            return self.nine_key.set_position(index, position);
        }
        self.input.set_candidate_position(index, position)
    }

    pub fn clear_position(&mut self, index: usize) -> KeyResult {
        if self.nine_key.active() {
            return self.nine_key.set_position(index, 0);
        }
        self.input.set_candidate_position(index, 0)
    }

    /// Finish the whole composition starting with the host-highlighted candidate (0 when none is highlighted); remaining segments use their leading candidate, and an invalid index commits the raw input.
    pub fn finish(&mut self, first_index: usize) -> KeyResult {
        if self.nine_key.active() {
            let result = self.nine_key.finish(first_index);
            return self.after_nine_key(result);
        }
        self.input.finish_composition(first_index)
    }

    /// Discards the composition. Fails, staying in the current scheme with the composition untouched, when the new scheme's dictionary cannot be opened (Cantonese without a usable `cantonese.db`, Stroke without a usable `stroke.db`: `LANGUAGE_DICTIONARY_UNAVAILABLE`, `LANGUAGE_DICTIONARY_VERSION_UNSUPPORTED`).
    pub fn switch_scheme(&mut self, scheme: SchemeType) -> Result<()> {
        self.input.switch_scheme(scheme)?;
        self.nine_key.command(Command::Cancel);
        Ok(())
    }

    pub fn is_supported_helpcode_schema(schema: &str) -> bool {
        crate::helpcode::is_supported_helpcode_schema(schema)
    }

    /// False for an unsupported schema or a table that cannot be loaded.
    pub fn set_helpcode_schema(&mut self, schema: &str) -> bool {
        self.input.set_helpcode_schema(schema)
    }

    /// 换上宿主给的辅助码表，替换当前的表；全拼和双拼都用它。
    pub fn set_helpcode_table(&mut self, table: crate::helpcode::SharedKeymap) {
        self.input.set_helpcode_table(table);
    }

    /// Quanpin and shuangpin together, as `SessionOptions::helpcode` does.
    pub fn set_helpcode_enabled(&mut self, enabled: bool) {
        self.input.set_helpcode_enabled(enabled);
    }

    /// Flipping it resets the composition.
    pub fn set_dedicated_english(&mut self, enabled: bool) {
        self.nine_key.command(Command::Cancel);
        self.input.set_dedicated_english_mode(enabled);
        // The grid's digits mean letters in English and syllables outside it, so whichever of the two switches moves last has to tell it.
        self.nine_key
            .set_english_only(self.nine_key_enabled && enabled);
    }

    pub fn set_wubi_mixed_pinyin(&mut self, enabled: bool) {
        self.input.set_wubi_mixed_pinyin(enabled);
    }

    pub fn reset_cache(&mut self) {
        self.input.reset_cache();
    }

    /// Replace the `/` mode's command table; rows it cannot use are dropped. A diagnostic only if the open command list could not be refreshed.
    pub fn set_command_table(&mut self, table: &[CommandTableEntry]) -> Option<String> {
        self.input.set_command_table(table)
    }

    /// 替换 K 模式的宿主短语表；用不了的行被丢弃。只有打开的 K 模式列表刷新失败时才返回诊断。
    pub fn set_quick_phrase_table(&mut self, table: &[QuickPhraseEntry]) -> Option<String> {
        self.input.set_quick_phrase_table(table)
    }

    /// Replace the `@` mode's list; entries it cannot use are dropped.
    pub fn set_mention_entries(&mut self, entries: &[MentionEntry]) -> Option<String> {
        self.input.set_mention_entries(entries)
    }

    /// Turn the embedded places of `@` mode on or off.
    pub fn set_mention_places(&mut self, enabled: bool) -> Option<String> {
        self.input.set_mention_places(enabled)
    }

    /// Forget the committed-word context (focus or application change, host-inserted text).
    pub fn reset_context(&mut self) {
        self.input.reset_commit_context();
    }

    /// Disabling also ends the current context.
    pub fn set_personal_context_enabled(&mut self, enabled: bool) {
        self.input.set_personal_context_enabled(enabled);
    }

    /// The committed text the neural sentence models condition on, updated without rebuilding the session. Only the last 64 characters matter.
    pub fn set_rescoring_context(&mut self, context: &str) {
        self.input.set_rescoring_context(context);
    }

    /// Hand over the candidates withheld from a single-letter query; whether the list grew.
    pub fn expand_initial_candidates(&mut self) -> bool {
        // Nine-key rows come from the spelling session rather than a dictionary query, so nothing is withheld there.
        if self.nine_key.active() {
            return false;
        }
        self.input.expand_initial_candidates()
    }

    /// Composition caret for prefix decoding (overlays.md §7.6); `None` returns to the end. A no-op while nine-key digits are active.
    pub fn set_caret(&mut self, caret: Option<usize>) {
        if self.nine_key.active() {
            return;
        }
        self.input.set_caret(caret);
    }

    /// End of the complete pinyin units before the caret in `editing_text`; 0 while nine-key digits are active.
    pub fn prefix_end(&self) -> usize {
        if self.nine_key.active() {
            return 0;
        }
        self.input.prefix_end()
    }

    /// The raw text after `prefix_end`, case kept; empty while nine-key digits are active.
    pub fn pending_suffix(&self) -> String {
        if self.nine_key.active() {
            return String::new();
        }
        self.input.pending_suffix()
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        if self.nine_key.active() {
            return self.nine_key.snapshot();
        }
        let input = &self.input;
        let candidates = input.candidates().to_vec();
        let mut preedit = input.preedit();
        if !self.shuangpin_preedit_uses_raw
            && input.scheme() == SchemeType::Shuangpin
            && input.local_mode == LocalInputMode::None
            && !input.dedicated_english
        {
            preedit = input.pinyin_segmentation_with_cases();
        }
        SessionSnapshot {
            scheme: input.scheme(),
            local_mode: input.local_mode,
            spelling_symbols: input.spelling_symbols(),
            preedit,
            raw_segmentation: input.raw_segmentation(),
            normalized_segmentation: input.normalized_segmentation(),
            dedicated_english: input.dedicated_english,
            editing_text: input.editing_text(),
            caret_position: input.caret_position(),
            nine_key_spellings: Vec::new(),
            answered_by_pinyin_fallback: input.answered_by_pinyin_fallback(),
            wubi_unique_four_code: input.wubi_unique_four_code(),
            shuangpin_profile: input.profile.name().to_owned(),
            candidate_sources: candidates.iter().map(|item| item.source).collect(),
            candidate_annotations: input.candidate_annotations(),
            // `pinyin` rather than `canonical_pinyin`: the former is what composition advancement consumes, which is the question being asked.
            candidate_answers_key: candidates
                .iter()
                .map(|item| {
                    input.selection_completes_composition(&item.pinyin, &item.word, item.scheme)
                })
                .collect(),
            candidate_list_open: input.candidate_list_open(),
            candidates,
        }
    }

    /// Byte offsets into `editing_text` for pinyin-unit editing; empty when idle, in local modes, for non-pinyin schemes and during nine-key input. Read-only.
    pub fn segment_raw_boundaries(&self) -> Vec<usize> {
        if self.nine_key.active() {
            return Vec::new();
        }
        self.input.segment_raw_boundaries()
    }

    pub fn online_query(&self) -> Option<OnlineQuery> {
        if self.nine_key.active() {
            return None;
        }
        self.input.online_query()
    }

    /// Whether an apostrophe now separates a unit from its target or one `/fy` word from the next, so a host or runtime that would finish the composition on punctuation hands it over as a character instead.
    pub fn takes_local_separator(&self) -> bool {
        !self.nine_key.active() && self.input.takes_local_separator()
    }

    /// The `/fy` translation request; see `InputSession::command_translation_query`.
    pub fn command_translation_query(&self) -> Option<CommandTranslationQuery> {
        if self.nine_key.active() {
            return None;
        }
        self.input.command_translation_query()
    }

    /// Whether the translation now shows first; false (not an error) for a stale query.
    pub fn apply_command_translation(
        &mut self,
        query: &CommandTranslationQuery,
        translation: &str,
    ) -> bool {
        if self.nine_key.active() {
            return false;
        }
        self.input.apply_command_translation(query, translation)
    }

    /// Whether the rows now show; false (not an error) for a stale query.
    pub fn apply_online_candidates(
        &mut self,
        query: &OnlineQuery,
        words: &[String],
        source: CandidateSource,
    ) -> bool {
        if self.nine_key.active() {
            return false;
        }
        self.input.apply_online_candidates(query, words, source)
    }

    pub fn apply_online_candidate(
        &mut self,
        query: &OnlineQuery,
        word: &str,
        source: CandidateSource,
    ) -> bool {
        if self.nine_key.active() {
            return false;
        }
        self.input.apply_online_candidate(query, word, source)
    }

    /// Grid commits bypass the input session, so the word after one must not read the word before it as its context (session.cpp:37-45).
    fn after_nine_key(&mut self, result: KeyResult) -> KeyResult {
        if result.commit.is_some() {
            self.input.note_external_commit();
        }
        result
    }
}
