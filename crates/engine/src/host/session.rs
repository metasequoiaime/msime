//! The bridge's `Session` wrapper over the engine session (api-contract §1a, §2, §5): ASCII checks, command numbering with `CommitRawWithoutLearning`, the raw-commit learning policy, the flattened snapshot and the online query snapshot.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use super::options::{runtime_paths, session_options, shuangpin_profile, EngineOptions};
use crate::assets;
use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::helpcode::{compute_helpcodes, load_helpcode_keymap, HelpcodeKeymap, SharedKeymap};
use crate::local::database::LocalDatabaseLease;
use crate::pinyin::segment::is_complete_pinyin_input;
use crate::types::{
    CandidateEdge, CandidateSource, CommandTableEntry, CommandTranslationQuery, KeyResult,
    LocalInputMode, MentionEntry, OnlineQuery, QuickPhraseEntry, SchemeType, ShuangpinProfileKind,
};
use crate::user_dictionary::ngram_store::flush_journal;
use crate::user_dictionary::removal::learn_entered_english_word;

/// The weight an entered English word is learned at: the C++ default argument of `learn_entered_english_word` (user_dictionary_journal.h:136-137), which the bridge relied on.
const ENTERED_ENGLISH_WORD_WEIGHT: i64 = 10;

fn temporary_japanese_word(commit: &str) -> String {
    let mut word = String::with_capacity(1 + commit.len());
    word.push('R');
    word.push_str(commit);
    word
}

/// The host's command numbering. `CommitRawWithoutLearning` has no engine counterpart, and took 11 before the engine's `ConvertHanja` existed, so that one is 12 here and mapped by name rather than by ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Command {
    Backspace = 0,
    CommitCandidate = 1,
    CommitRaw = 2,
    Cancel = 3,
    MoveLeft = 4,
    MoveRight = 5,
    MoveHome = 6,
    MoveEnd = 7,
    DeleteForward = 8,
    CycleKanaVariant = 9,
    CommitReading = 10,
    /// Commit the letters as typed without learning them as an English word.
    CommitRawWithoutLearning = 11,
    /// Open or close the active scheme's candidate list (the Korean Hanja list, the Zhuyin conversion list); unhandled in a scheme without one. Hosts may call it `MSIME_OPEN_CANDIDATE_LIST`.
    ConvertHanja = 12,
}

/// Every `candidate_*` vector has `candidates.len()` elements; the runtime's reorderings require it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EngineSnapshot {
    pub local_mode: String,
    /// The non-letter characters `character` takes in this state (`SessionSnapshot::spelling_symbols`): send one of these as a character, never as punctuation.
    pub spelling_symbols: String,
    pub dedicated_english: bool,
    /// Mirrors `set_nine_key_enabled`; the engine snapshot has no such flag.
    pub nine_key: bool,
    pub nine_key_spellings: Vec<String>,
    pub microsoft_shuangpin: bool,
    pub shuangpin_profile: String,
    pub preedit: String,
    /// The kana reading in Japanese, the composed Hangul in Korean, the converted text plus the pending bopomofo in Zhuyin, the stroke glyphs (一丨丿丶乛＊) in Stroke, else empty. In Stroke each glyph stands for one ASCII letter of `editing_text`, so `caret_position` also counts glyphs.
    pub reading: String,
    pub editing_text: String,
    pub caret_position: usize,
    pub segment_raw_boundaries: Vec<u64>,
    pub candidates: Vec<String>,
    pub candidate_codes: Vec<String>,
    pub scheme: u8,
    pub answered_by_pinyin_fallback: bool,
    pub wubi_unique_four_code: bool,
    pub candidate_annotations: Vec<String>,
    pub candidate_sources: Vec<u8>,
    pub candidate_positions: Vec<u8>,
    pub candidate_corrected: Vec<bool>,
    pub candidate_answers_key: Vec<bool>,
    /// The scheme's openable candidate list is showing (the Korean Hanja list, the Zhuyin conversion list); candidates are its rows while it is.
    pub candidate_list_open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EngineResult {
    pub handled: bool,
    pub has_commit: bool,
    pub commit: String,
    pub diagnostic: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OnlineQuerySnapshot {
    pub available: bool,
    pub scheme: u8,
    pub generation: u64,
    pub identity: String,
    pub query_text: String,
    pub cache_key: String,
    pub pinyin_segments: Vec<String>,
    pub cloud_eligible: bool,
    pub ai_eligible: bool,
    pub session_id: u64,
}

/// Thread-confined like the bridge session it replaces.
pub struct Session {
    inner: crate::session::Session,
    options: EngineOptions,
    nine_key: bool,
    microsoft_shuangpin: bool,
    shuangpin_profile: String,
    helpcode_keymap: Option<SharedKeymap>,
    helpcode_enabled: bool,
    show_helpcode: bool,
    /// The generation and resource directories the local-mode queries read; dropped after `inner`, so the last session to go closes their cached connections.
    _local_databases: LocalDatabaseLease,
    _thread_confined: PhantomData<Rc<()>>,
}

impl Session {
    pub fn new(options: &EngineOptions) -> Result<Session> {
        // The C++ ran `options_for` a second time only to read the profile name back (bridge.cpp:430); that rerun recopied the same sidecar, so one mapping is enough.
        let inner = crate::session::Session::new(session_options(options)?)?;
        let profile = shuangpin_profile(options)?;
        // The engine loads its own copy for filtering; this one only annotates, and exists only while helpcode is on (bridge.cpp:431-435).
        let helpcode_keymap = if options.helpcode {
            match &options.helpcode_table {
                Some(table) => Some(table.clone()),
                None => Some(Arc::new(load_helpcode_keymap(
                    Path::new(&options.resources),
                    &options.helpcode_schema,
                )?)),
            }
        } else {
            None
        };
        Ok(Session {
            inner,
            options: options.clone(),
            nine_key: false,
            microsoft_shuangpin: options.scheme == SchemeType::Shuangpin as u8
                && profile == ShuangpinProfileKind::Microsoft,
            shuangpin_profile: profile.name().to_owned(),
            helpcode_keymap,
            helpcode_enabled: options.helpcode,
            show_helpcode: options.show_helpcode,
            _local_databases: LocalDatabaseLease::new([
                PathBuf::from(&options.dictionaries),
                PathBuf::from(&options.resources),
            ]),
            _thread_confined: PhantomData,
        })
    }

    /// With the annotation rule of bridge.cpp:914-929.
    pub fn snapshot(&self) -> Result<EngineSnapshot> {
        let value = self.inner.snapshot();
        let helpcode_scheme = value.scheme.helpcode();
        let uppercase_all = value.scheme == SchemeType::Quanpin;
        // Rows the engine generated in the expression, command and mention modes are not spelled by pinyin; their annotations are the engine's own.
        let keymap = self.helpcode_keymap.as_deref().filter(|_| {
            self.helpcode_enabled && helpcode_scheme && !value.local_mode.generates_text()
        });
        let count = value.candidates.len();
        let mut output = EngineSnapshot {
            local_mode: value.local_mode.name().to_owned(),
            spelling_symbols: value.spelling_symbols,
            dedicated_english: value.dedicated_english,
            nine_key: self.nine_key,
            nine_key_spellings: value.nine_key_spellings,
            microsoft_shuangpin: self.microsoft_shuangpin,
            shuangpin_profile: self.shuangpin_profile.clone(),
            preedit: value.preedit,
            reading: if value.scheme.draws_reading() {
                value.normalized_segmentation
            } else {
                String::new()
            },
            editing_text: value.editing_text,
            caret_position: value.caret_position,
            segment_raw_boundaries: self
                .inner
                .segment_raw_boundaries()
                .into_iter()
                .map(|boundary| boundary as u64)
                .collect(),
            candidates: Vec::with_capacity(count),
            candidate_codes: Vec::with_capacity(count),
            scheme: value.scheme as u8,
            answered_by_pinyin_fallback: value.answered_by_pinyin_fallback,
            wubi_unique_four_code: value.wubi_unique_four_code,
            candidate_annotations: Vec::with_capacity(count),
            candidate_sources: Vec::with_capacity(count),
            candidate_positions: Vec::with_capacity(count),
            candidate_corrected: Vec::with_capacity(count),
            candidate_answers_key: Vec::with_capacity(count),
            candidate_list_open: value.candidate_list_open,
        };
        for (index, candidate) in value.candidates.into_iter().enumerate() {
            let mut annotation = value
                .candidate_annotations
                .get(index)
                .cloned()
                .unwrap_or_else(|| candidate.corrected_from.clone());
            if let Some(keymap) = keymap {
                if !self.show_helpcode {
                    // Hidden helpcodes give their slot back to the correction hint the engine would otherwise have shown.
                    let helpcode = compute_helpcodes(&candidate.word, uppercase_all, keymap);
                    if !helpcode.is_empty() && annotation == helpcode {
                        annotation = candidate.corrected_from.clone();
                    }
                } else if annotation.is_empty() && candidate.source == CandidateSource::Generated {
                    // The engine annotates dictionary rows itself; a sentence it synthesised carries no helpcode until the host adds one.
                    annotation = compute_helpcodes(&candidate.word, uppercase_all, keymap);
                }
            }
            output.candidate_annotations.push(annotation);
            output.candidate_sources.push(candidate.source as u8);
            output
                .candidate_positions
                .push(candidate.fixed_position as u8);
            output
                .candidate_corrected
                .push(!candidate.corrected_from.is_empty());
            // A short vector would be an engine bug. False is the safe reading of one: a consumer that sees nothing answering the key declines to reorder (bridge.cpp:933-937).
            output.candidate_answers_key.push(
                value
                    .candidate_answers_key
                    .get(index)
                    .copied()
                    .unwrap_or(false),
            );
            output.candidate_codes.push(candidate.pinyin);
            output.candidates.push(candidate.word);
        }
        Ok(output)
    }

    /// `available = false` and defaults when there is no query.
    pub fn online_query(&self) -> Result<OnlineQuerySnapshot> {
        let Some(query) = self.inner.online_query() else {
            return Ok(OnlineQuerySnapshot::default());
        };
        Ok(OnlineQuerySnapshot {
            available: true,
            scheme: query.scheme as u8,
            generation: query.generation,
            identity: query.identity,
            query_text: query.query_text,
            cache_key: query.cache_key,
            pinyin_segments: query.pinyin_segments,
            cloud_eligible: query.cloud_eligible,
            ai_eligible: query.ai_eligible,
            session_id: query.session_id,
        })
    }

    pub fn reset_cache(&mut self) {
        self.inner.reset_cache();
    }

    pub fn set_caret(&mut self, caret: Option<usize>) {
        self.inner.set_caret(caret);
    }

    pub fn prefix_end(&self) -> usize {
        self.inner.prefix_end()
    }

    pub fn pending_suffix(&self) -> String {
        self.inner.pending_suffix()
    }

    pub fn reset_context(&mut self) {
        self.inner.reset_context();
    }

    /// Replace the `/` mode's command table live; `EngineOptions::command_table` is what a rebuilt session starts with.
    pub fn set_command_table(&mut self, table: &[CommandTableEntry]) -> Result<()> {
        match self.inner.set_command_table(table) {
            Some(diagnostic) => Err(EngineError::failed(&diagnostic)),
            None => Ok(()),
        }
    }

    /// 实时替换宿主给的辅助码表（辅助码表插件）；`None` 回到 `helpcode_schema` 对应的表。重建的会话从 `EngineOptions::helpcode_table` 开始。
    ///
    /// 从不失败：回退的表读不出来（典型是 `custom/<stem>` 的文件已被删掉）时装上空表并记一条日志。宿主在获得焦点时调用它，这里报错会让每一次聚焦都失败。
    pub fn set_helpcode_table(&mut self, table: Option<SharedKeymap>) -> Result<()> {
        let keymap = match &table {
            Some(table) => table.clone(),
            None => Arc::new(
                load_helpcode_keymap(
                    Path::new(&self.options.resources),
                    &self.options.helpcode_schema,
                )
                .unwrap_or_else(|error| {
                    eprintln!(
                        "msime: helpcode schema {} unavailable, using an empty table: {error}",
                        self.options.helpcode_schema
                    );
                    HelpcodeKeymap::default()
                }),
            ),
        };
        self.inner.set_helpcode_table(keymap.clone());
        if self.helpcode_enabled {
            self.helpcode_keymap = Some(keymap);
        }
        self.options.helpcode_table = table;
        Ok(())
    }

    /// 实时替换 K 模式的宿主短语表；重建的会话从 `EngineOptions::quick_phrase_table` 开始。
    pub fn set_quick_phrase_table(&mut self, table: &[QuickPhraseEntry]) -> Result<()> {
        match self.inner.set_quick_phrase_table(table) {
            Some(diagnostic) => Err(EngineError::failed(&diagnostic)),
            None => Ok(()),
        }
    }

    /// Replace the `@` mode's list live; `EngineOptions::mention_entries` is what a rebuilt session starts with.
    pub fn set_mention_entries(&mut self, entries: &[MentionEntry]) -> Result<()> {
        match self.inner.set_mention_entries(entries) {
            Some(diagnostic) => Err(EngineError::failed(&diagnostic)),
            None => Ok(()),
        }
    }

    /// Offer the Chinese administrative divisions in `@` mode after the user's list. Off in a new session, so a host that has the switch on sets it on every session it builds, as it does nine-key mode.
    pub fn set_mention_places(&mut self, enabled: bool) -> Result<()> {
        match self.inner.set_mention_places(enabled) {
            Some(diagnostic) => Err(EngineError::failed(&diagnostic)),
            None => Ok(()),
        }
    }

    /// Live update of the neural rescoring context without a rebuild.
    pub fn set_rescoring_context(&mut self, context: &str) {
        self.inner.set_rescoring_context(context);
    }

    /// Whether an apostrophe now is input (`3jin'g`, `/fyhello'world`) rather than punctuation that ends the composition.
    pub fn takes_local_separator(&self) -> bool {
        self.inner.takes_local_separator()
    }

    /// The `/fy` request for the user's translation service, the only one a local mode makes; `None` whenever nothing asks for one.
    pub fn command_translation_query(&self) -> Option<CommandTranslationQuery> {
        self.inner.command_translation_query()
    }

    /// Puts the translation first in the `/fy` list; false for a stale query or text a row cannot show.
    pub fn apply_command_translation(
        &mut self,
        query: &CommandTranslationQuery,
        translation: &str,
    ) -> bool {
        self.inner.apply_command_translation(query, translation)
    }

    /// False for an unavailable query or a source other than 0 (cloud) and 1 (AI).
    pub fn apply_online_candidate(
        &mut self,
        query: &OnlineQuerySnapshot,
        candidate: &str,
        source: u8,
    ) -> Result<bool> {
        let Some((request, source)) = online_request(query, source) else {
            return Ok(false);
        };
        Ok(self
            .inner
            .apply_online_candidate(&request, candidate, source))
    }

    pub fn apply_online_candidates(
        &mut self,
        query: &OnlineQuerySnapshot,
        candidates: &[String],
        source: u8,
    ) -> Result<bool> {
        let Some((request, source)) = online_request(query, source) else {
            return Ok(false);
        };
        Ok(self
            .inner
            .apply_online_candidates(&request, candidates, source))
    }

    /// `CHARACTER_MUST_BE_ASCII` above 127.
    pub fn character(&mut self, value: u8, shift: bool) -> Result<EngineResult> {
        require_ascii(value, diagnostics::CHARACTER_MUST_BE_ASCII)?;
        Ok(result_for(self.inner.character(value, shift)))
    }

    pub fn expand_initial_candidates(&mut self) -> Result<bool> {
        Ok(self.inner.expand_initial_candidates())
    }

    pub fn set_nine_key_enabled(&mut self, enabled: bool) -> Result<()> {
        self.inner.set_nine_key_enabled(enabled);
        self.nine_key = enabled;
        Ok(())
    }

    /// Out of range is unhandled.
    pub fn choose_nine_key_spelling(&mut self, index: usize) -> Result<EngineResult> {
        Ok(result_for(self.inner.choose_nine_key_spelling(index)))
    }

    /// `CommitRaw` goes through the learning policy of bridge.cpp:1332-1359 and `CommitRawWithoutLearning` through :1321-1331.
    pub fn command(&mut self, command: Command) -> Result<EngineResult> {
        match command {
            Command::CommitRaw => Ok(self.commit_raw_with_policy()),
            Command::CommitRawWithoutLearning => Ok(self.commit_raw_without_learning()),
            Command::ConvertHanja => Ok(result_for(
                self.inner.command(crate::types::Command::ConvertHanja),
            )),
            _ => {
                // The other host codes are the engine's ordinals one for one (bridge.cpp:1302-1319).
                let engine = crate::types::Command::from_u8(command as u8)
                    .ok_or_else(|| EngineError::invalid(diagnostics::UNSUPPORTED_INPUT_COMMAND))?;
                Ok(result_for(self.inner.command(engine)))
            }
        }
    }

    pub fn select(&mut self, index: usize) -> Result<EngineResult> {
        Ok(result_for(self.inner.select(index)))
    }

    pub fn pin_candidate(&mut self, index: usize) -> Result<EngineResult> {
        Ok(result_for(self.inner.pin(index)))
    }

    pub fn remove_candidate(&mut self, index: usize) -> Result<EngineResult> {
        Ok(result_for(self.inner.remove(index)))
    }

    /// `INVALID_CANDIDATE_POSITION` unless 1..=5.
    pub fn fix_candidate_position(&mut self, index: usize, position: u8) -> Result<EngineResult> {
        if !(1..=5).contains(&position) {
            return Err(EngineError::invalid(
                diagnostics::INVALID_CANDIDATE_POSITION,
            ));
        }
        Ok(result_for(
            self.inner.fix_position(index, i32::from(position)),
        ))
    }

    pub fn clear_candidate_position(&mut self, index: usize) -> Result<EngineResult> {
        Ok(result_for(self.inner.clear_position(index)))
    }

    pub fn select_edge(&mut self, index: usize, edge: CandidateEdge) -> Result<EngineResult> {
        Ok(result_for(self.inner.select_edge(index, edge)))
    }

    pub fn finish(&mut self, index: usize) -> Result<EngineResult> {
        Ok(result_for(self.inner.finish(index)))
    }

    /// `PUNCTUATION_MUST_BE_ASCII` above 127.
    pub fn punctuation(&mut self, value: u8) -> Result<EngineResult> {
        require_ascii(value, diagnostics::PUNCTUATION_MUST_BE_ASCII)?;
        Ok(result_for(self.inner.punctuation(value)))
    }

    pub fn balance_paired_punctuation_after_auto_close(&mut self, opening: u8) -> Result<()> {
        require_ascii(opening, diagnostics::PAIRED_OPENING_MUST_BE_ASCII)?;
        self.inner
            .balance_paired_punctuation_after_auto_close(opening);
        Ok(())
    }

    pub fn set_chinese_punctuation_enabled(&mut self, enabled: bool) -> Result<()> {
        self.inner.set_chinese_punctuation_enabled(enabled);
        Ok(())
    }

    pub fn set_punctuation_lock(&mut self, lock: u8) -> Result<()> {
        self.inner.set_punctuation_lock(i32::from(lock))
    }

    pub fn set_paired_punctuation_enabled(&mut self, enabled: bool) -> Result<()> {
        self.inner.set_paired_punctuation_enabled(enabled);
        Ok(())
    }

    pub fn set_dedicated_english(&mut self, enabled: bool) -> Result<()> {
        self.inner.set_dedicated_english(enabled);
        Ok(())
    }

    /// The letters as typed, learned as nothing. The engine's own raw commit learns the word itself in dedicated English, so that mode takes the preedit and cancels instead, stripping the trigger letter of a temporary mode the way `InputSession` does (bridge.cpp:1320-1331).
    fn commit_raw_without_learning(&mut self) -> EngineResult {
        let before = self.inner.snapshot();
        if !before.dedicated_english {
            return result_for(self.inner.command(crate::types::Command::CommitRaw));
        }
        let mut raw = before.preedit;
        if matches!(
            before.local_mode,
            LocalInputMode::TemporaryEnglish | LocalInputMode::TemporaryJapanese
        ) && !raw.is_empty()
        {
            raw.remove(0);
        }
        self.inner.command(crate::types::Command::Cancel);
        result_for(KeyResult::committed(raw))
    }

    /// Windows learns an entered word only on Enter: letters committed raw in dedicated English, a local mode, or pinyin that is not a complete syllable sequence are learned as an English word (bridge.cpp:1332-1359).
    fn commit_raw_with_policy(&mut self) -> EngineResult {
        let before = self.inner.snapshot();
        let chinese_scheme = before.scheme.learns_english_words();
        let complete_pure_pinyin = chinese_scheme && {
            let segmentation = if before.normalized_segmentation.is_empty() {
                &before.raw_segmentation
            } else {
                &before.normalized_segmentation
            };
            !segmentation.is_empty() && is_complete_pinyin_input(segmentation)
        };
        // 计算、指令和名单模式里是算式、触发词或键，网址也不是英文单词，都不是用户拼出的词，不进英文词库。
        let should_learn = !before.local_mode.generates_text()
            && before.local_mode != LocalInputMode::Url
            && (before.dedicated_english
                || before.local_mode != LocalInputMode::None
                || (chinese_scheme && !complete_pure_pinyin));
        let mut result = self.inner.command(crate::types::Command::CommitRaw);
        if let Some(commit) = result.commit.as_deref().filter(|_| should_learn) {
            if !commit.is_empty() {
                let word = if before.local_mode == LocalInputMode::TemporaryJapanese {
                    temporary_japanese_word(commit)
                } else {
                    commit.to_owned()
                };
                let paths = runtime_paths(&self.options);
                // The commit already happened; a word that could not be learned is reported beside it rather than undoing it.
                if learn_entered_english_word(
                    &paths.dictionary(assets::ENGLISH_DICTIONARY),
                    &paths.user(assets::USER_JOURNAL),
                    &word,
                    ENTERED_ENGLISH_WORD_WEIGHT,
                )
                .is_err()
                {
                    result.diagnostic = Some(diagnostics::ENGLISH_WORD_NOT_LEARNED.to_owned());
                }
            }
        }
        result_for(result)
    }
}

/// The C++ registered `PersonalNgramStore::flush_all` with `atexit` (personal_ngram_store.cpp:255), so context learned in the last ~2 s reached the journal when the host quit. Rust runs no destructors for statics and `atexit` needs unsafe, so the session writes its journal's queue when the host drops it, which hosts do on deactivation and shutdown. A host that can exit without dropping its sessions (macOS `[NSApp terminate:]` runs `exit()`) calls `flush_personal_learning`, through host-api's `msime_client_flush_all`, from its will-terminate hook instead.
impl Drop for Session {
    fn drop(&mut self) {
        let journal = runtime_paths(&self.options).user(assets::USER_JOURNAL);
        // Nobody is left to report to: a failed write stays queued and flagged in the store, and the next record reports it and schedules another try.
        let _ = flush_journal(&journal);
        // A host thread whose sessions are gone (a quiesced IME, a closed window) holds no journal handle.
        crate::user_dictionary::journal::release_thread_journal();
    }
}

/// Whether a commit made in `local_mode` (a `local_mode` name, as `EngineSnapshot` carries it) counts as typing. Text the expression, command and mention modes generated does not; an unknown name does, as every mode did before those.
pub fn local_mode_counts_as_typing(local_mode: &str) -> bool {
    LocalInputMode::from_name(local_mode).is_none_or(|mode| !mode.generates_text())
}

/// `result_for` (bridge.cpp:408-410): an empty commit with `has_commit = true` stays representable.
fn result_for(value: KeyResult) -> EngineResult {
    EngineResult {
        handled: value.handled,
        has_commit: value.commit.is_some(),
        commit: value.commit.unwrap_or_default(),
        diagnostic: value.diagnostic.unwrap_or_default(),
    }
}

fn require_ascii(value: u8, message: &str) -> Result<()> {
    if value > 127 {
        return Err(EngineError::invalid(message));
    }
    Ok(())
}

/// The query handed back field by field (bridge.cpp:977-1017). `None` for an unavailable query, a source other than cloud or AI, or a scheme ordinal no session could have issued, which is a stale query rather than an error (api-contract §5.3).
fn online_request(
    query: &OnlineQuerySnapshot,
    source: u8,
) -> Option<(OnlineQuery, CandidateSource)> {
    if !query.available {
        return None;
    }
    let source = match source {
        0 => CandidateSource::CloudSuggestion,
        1 => CandidateSource::AiSuggestion,
        _ => return None,
    };
    let request = OnlineQuery {
        scheme: SchemeType::from_u8(query.scheme)?,
        generation: query.generation,
        identity: query.identity.clone(),
        query_text: query.query_text.clone(),
        cache_key: query.cache_key.clone(),
        pinyin_segments: query.pinyin_segments.clone(),
        cloud_eligible: query.cloud_eligible,
        ai_eligible: query.ai_eligible,
        session_id: query.session_id,
    };
    Some((request, source))
}

#[cfg(test)]
mod tests {
    use super::temporary_japanese_word;

    #[test]
    fn temporary_japanese_word_allocates_only_result_bytes() {
        let word = temporary_japanese_word("かな");
        assert_eq!(word, "Rかな");
        assert_eq!(word.capacity(), word.len());
    }
}
