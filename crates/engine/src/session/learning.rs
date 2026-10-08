//! Learning on commit (core-session.md §7.3-§7.5, overlays.md §3.3, §9.2, §9.4): frequency adjustment, sentence and online-row phrases, accepted typos, rejected corrections, pick pairs and personal context. Every failure becomes the matching `diagnostics` string on the key result; learning never blocks the commit.
//!
//! A `Session` always configures frequency adjustment (session.cpp:21-23 rejects invalid options instead), so the reference's unconfigured compatibility path, which bumped a picked row to its key's maximum plus one, is unreachable and not ported.

use std::time::Duration;

use super::composition::{
    append_canonical_pinyin, fold_autocorrect_letters, normalize_canonical_pinyin_for_word,
};
use super::input::InputSession;
use crate::assets;
use crate::diagnostics;
use crate::lattice::personal::PersonalTransition;
use crate::local::jianpin::jianpin_ranking_context;
use crate::pinyin::autocorrect::autocorrect_suppression_key;
use crate::pinyin::segment::split_segments;
use crate::pinyin::syllables::normalize_umlaut_aliases;
use crate::pinyin::typos::syllable_typo_kind;
use crate::text::{count_utf8_chars, is_all_han, is_han};
use crate::types::{
    CandidateSource, FrequencyAdjustmentMode, FrequencyAdjustmentOptions, LocalInputMode,
    PersonalDictionaryKind, WordItem,
};
use crate::user_dictionary::journal::is_user_deleted;
use crate::user_dictionary::picks::{clear_pick_transition, record_pick_transition};
use crate::user_dictionary::ranking::{
    adjust_candidate_ranking, adjust_english_candidate_ranking, RankingRequest,
};
use crate::user_dictionary::typo_profile::is_storable_autocorrect_suppression;

/// Match the engine's own maximum stored phrase width. Longer generated sentences are useful for this commit only and would otherwise grow the user dictionary without bound (ISC:51-53).
pub const MAX_LEARNED_SENTENCE_SYLLABLES: usize = 7;
/// Two explicit picks from separate compositions become one word after the same pair has been seen this many times.
pub const PICK_PAIR_WORD_THRESHOLD: i64 = 3;
pub const MAX_PICK_PAIR_WORD_CHARS: usize = 4;
/// Hosts do not report everything between two compositions (their own English mode, modifier shortcuts, caret moves by mouse), so picks further apart than this are not treated as one phrase.
pub const PICK_PAIR_MAX_GAP_SECONDS: u64 = 10;

/// An explicit pick among alternatives says more than accepting a decoded sentence, so it counts twice (IS:1181).
const DICTIONARY_PICK_TIMES: u32 = 2;

/// Split and umlaut-normalised syllables of a reading.
fn normalized_syllables(pinyin: &str) -> Vec<String> {
    let mut segments = split_segments(pinyin);
    normalize_umlaut_aliases(&mut segments);
    segments
}

/// 只需判断句子长度时统计分隔符，避免为每个音节复制字符串。
fn segment_count(pinyin: &str) -> usize {
    if pinyin.is_empty() {
        0
    } else {
        pinyin.bytes().filter(|&byte| byte == b'\'').count() + 1
    }
}

fn join_words(first: &str, second: &str) -> String {
    let mut word = String::with_capacity(first.len() + second.len());
    word.push_str(first);
    word.push_str(second);
    word
}

fn english_context_key(context: &str) -> String {
    let mut key = String::with_capacity("english:".len() + context.len());
    key.push_str("english:");
    key.extend(
        context
            .chars()
            .map(|character| character.to_ascii_lowercase()),
    );
    key
}

/// 只复制实际参与排名的行，避免先复制完整排序列表再筛选一遍。
fn clone_matching_rows(
    ordered: &[WordItem],
    mut matches: impl FnMut(&WordItem) -> bool,
) -> Vec<WordItem> {
    ordered
        .iter()
        .filter(|item| matches(item))
        .cloned()
        .collect()
}

impl InputSession {
    /// input_session.cpp:1274-1319; `index` is in ranking order.
    pub(super) fn learn_candidate(&mut self, index: usize) -> Option<String> {
        if !self.learning_enabled {
            return None;
        }
        let selected_source = self.ranking_list().get(index)?.source;
        let temporary_english = self.local_mode == LocalInputMode::TemporaryEnglish;
        if (self.dedicated_english || temporary_english)
            && selected_source == CandidateSource::EnglishDictionary
            && self.frequency.mode != FrequencyAdjustmentMode::Disabled
            && index != 0
        {
            return self.adjust_candidate_frequency(index, self.frequency, false);
        }
        // Generated/Fallback and injected online sentences are not dictionary rows, so frequency adjustment has nowhere to persist them. Store the selected sentence as a user phrase instead. This applies even at index zero and is independent of the frequency-adjustment mode.
        if selected_source.is_sentence_learning() {
            let selected = self.ranking_list().get(index)?.clone();
            return self.learn_sentence_candidate(&selected);
        }
        if self.frequency.mode == FrequencyAdjustmentMode::Disabled || index == 0 {
            return None;
        }
        if !selected_source.is_dictionary()
            || !self
                .engine
                .current_scheme_type()
                .learns_into_main_dictionary()
        {
            return None;
        }
        self.adjust_candidate_frequency(index, self.frequency, false)
    }

    /// input_session.cpp:1352-1412, with the ranking set filtered to the selected row's producer in mixed wubi.
    pub(super) fn adjust_candidate_frequency(
        &mut self,
        index: usize,
        options: FrequencyAdjustmentOptions,
        force_top: bool,
    ) -> Option<String> {
        // Ranks are read from the order before personal context reordering.
        let ordered = self.ranking_list();
        let selected = ordered.get(index)?.clone();
        let user_db = self.journal_path();
        if selected.source == CandidateSource::EnglishDictionary {
            let context = if self.dedicated_english {
                self.dedicated_english_preedit.clone()
            } else if self.local_mode == LocalInputMode::TemporaryEnglish {
                self.local_preedit.get(1..).unwrap_or_default().to_owned()
            } else {
                self.engine.request().raw_input.clone()
            };
            let context_key = english_context_key(&context);
            let english_rows = clone_matching_rows(ordered, |item| {
                item.source == CandidateSource::EnglishDictionary
            });
            let english_db = self.paths.dictionary(assets::ENGLISH_DICTIONARY);
            let adjusted = adjust_english_candidate_ranking(&RankingRequest {
                main_db: &english_db,
                user_db,
                context_key: &context_key,
                ordered: &english_rows,
                entry_key: &selected.pinyin,
                value: &selected.word,
                mode: options.mode,
                linear_step: options.linear_step,
                trigger_count: options.trigger_count,
                force_top,
                kind: PersonalDictionaryKind::English,
            });
            return adjusted
                .err()
                .map(|_| diagnostics::ENGLISH_FREQUENCY_NOT_PERSISTED.to_owned());
        }

        let super_jianpin = self.local_mode == LocalInputMode::SuperJianpin;
        // In a mixed wubi list the selected row's producer decides: a quanpin row is ranked, keyed and stored as pinyin, and only a row the wubi table answered is ranked under the code itself (overlays.md §3.3). The pinyin fallback reuses the context the fixed positions are written under, otherwise a pinned candidate would not be recognised here.
        let wubi = Self::is_wubi_native_candidate(&selected);
        let pinyin_fallback = self.is_wubi() && !wubi;
        let request = self.engine.request();
        let mut context_key = if super_jianpin {
            jianpin_ranking_context(
                self.local_preedit.get(1..).unwrap_or_default(),
                self.scheme(),
                self.shuangpin_profile(),
            )
        } else if wubi {
            request.raw_input.clone()
        } else if pinyin_fallback {
            self.position_context(false, false).into_owned()
        } else {
            request.normalized_segmentation.clone()
        };
        if !super_jianpin && context_key.is_empty() {
            context_key = self.engine.request().segmentation.clone();
        }
        let wubi_row = wubi && !super_jianpin;
        let entry_key = if wubi_row {
            selected.pinyin.clone()
        } else if selected.canonical_pinyin.is_empty() {
            context_key.clone()
        } else {
            selected.canonical_pinyin.clone()
        };
        let mixed_wubi = self.is_wubi();
        let ranked = clone_matching_rows(ordered, |item| {
            !mixed_wubi || item.scheme == selected.scheme
        });
        let main_db = self.paths.dictionary(assets::MAIN_DICTIONARY);
        let adjusted = adjust_candidate_ranking(&RankingRequest {
            main_db: &main_db,
            user_db,
            context_key: &context_key,
            ordered: &ranked,
            entry_key: &entry_key,
            value: &selected.word,
            mode: options.mode,
            linear_step: options.linear_step,
            trigger_count: options.trigger_count,
            force_top,
            kind: if wubi_row {
                self.engine.wubi_input_options().profile.dictionary_kind()
            } else {
                PersonalDictionaryKind::Pinyin
            },
        });
        match adjusted {
            Ok(changed) => {
                if changed {
                    self.engine.reset_cache();
                }
                None
            }
            Err(_) => Some(diagnostics::FREQUENCY_NOT_PERSISTED.to_owned()),
        }
    }

    /// input_session_composition.cpp:562-600.
    pub(super) fn learn_sentence_candidate(&mut self, selected: &WordItem) -> Option<String> {
        // Local shortcuts and English/Japanese modes also use Generated candidates, but they are not pinyin sentences and must never enter the pinyin user dictionary. A native wubi row is not one either, while a pinyin row beside it in a mixed list is (overlays.md §3.3).
        if self.local_mode != LocalInputMode::None
            || self.dedicated_english
            || !self
                .engine
                .current_scheme_type()
                .learns_into_main_dictionary()
            || Self::is_wubi_native_candidate(selected)
        {
            return None;
        }
        let typo_diagnostic = self.learn_accepted_typos(selected);
        // Both quanpin and shuangpin sentences carry canonical quanpin; require a complete reading with one syllable per Han character before creating the row. Online cloud/AI rows are injected after the local query and carry no canonical reading: for a complete full-pinyin query the session's explicit segmentation is the canonical key, because the committed letters without apostrophes would let correction re-segment a reading such as qi'e'huan before it is stored.
        let online = selected.source.is_online();
        let selected_canonical =
            if selected.canonical_pinyin.is_empty() && online && self.is_all_complete_pure_pinyin()
            {
                self.pinyin_segmentation()
            } else {
                selected.canonical_pinyin.clone()
            };
        let canonical = normalize_canonical_pinyin_for_word(&selected_canonical, &selected.word);
        if canonical.is_empty() || segment_count(&canonical) > MAX_LEARNED_SENTENCE_SYLLABLES {
            return typo_diagnostic;
        }
        if online && !self.online_word_matches_reading(&canonical, &selected.word) {
            return typo_diagnostic;
        }
        if self
            .store_user_phrase_from_canonical_pinyin(&canonical, &selected.word)
            .is_err()
        {
            return Some(diagnostics::SENTENCE_NOT_PERSISTED.to_owned());
        }
        typo_diagnostic
    }

    /// Providers return bare words, so the only reading to check an online row against is the typed one. A character absent from every single-character table cannot be checked and is accepted so rare words stay learnable (input_session_composition.cpp:538-560).
    fn online_word_matches_reading(&mut self, canonical: &str, word: &str) -> bool {
        let syllables = normalized_syllables(canonical);
        let mut characters = word.chars();
        let engine = self.canonical_phrase_engine();
        for syllable in &syllables {
            let Some(character) = characters.next() else {
                return false;
            };
            if !is_han(character) {
                return false;
            }
            let han = character.to_string();
            if engine.find_candidate(syllable, &han).is_none() && engine.knows_han_char(&han) {
                return false;
            }
        }
        characters.next().is_none()
    }

    /// input_session_composition.cpp:602-627.
    pub(super) fn learn_accepted_typos(&mut self, selected: &WordItem) -> Option<String> {
        if selected.source != CandidateSource::Generated
            || !selected.sentence_association
            || selected.corrected_from.is_empty()
            || !self.scheme().supports_autocorrect()
        {
            return None;
        }
        let typed = normalized_syllables(&selected.pinyin.to_ascii_lowercase());
        let intended = normalized_syllables(&selected.canonical_pinyin);
        if typed.is_empty() || typed.len() != intended.len() {
            return None;
        }
        let pairs: Vec<(String, String)> = typed
            .into_iter()
            .zip(intended)
            .filter(|(typed, intended)| {
                typed != intended && syllable_typo_kind(typed, intended).is_some()
            })
            .collect();
        if pairs.is_empty() {
            return None;
        }
        self.typo_profile
            .record_accepted(&pairs)
            .err()
            .map(|_| diagnostics::TYPO_NOT_PERSISTED.to_owned())
    }

    /// On CommitRaw while a correction is offered (input_session_composition.cpp:629-656).
    pub(super) fn learn_rejected_correction(&mut self) -> Option<String> {
        if !self.learning_enabled
            || !self.scheme().supports_autocorrect()
            || self.local_mode != LocalInputMode::None
            || self.dedicated_english
            || self.autocorrect_types == 0
            || !self.has_composition()
        {
            return None;
        }
        let request = self.engine.request();
        let key = autocorrect_suppression_key(&request.raw_input, &request.raw_input_with_cases);
        // An input the store cannot hold (longer than its limit) is not a write failure to report on every Enter.
        if !is_storable_autocorrect_suppression(&key) {
            return None;
        }
        // A row labelled as corrected whose reading is the typed letters after all (a v/u spelling) offered nothing to reject.
        let typed_letters = fold_autocorrect_letters(&key);
        let correction_offered = self.candidates().iter().any(|item| {
            if item.corrected_from.is_empty() {
                return false;
            }
            let reading = if item.canonical_pinyin.is_empty() {
                &item.pinyin
            } else {
                &item.canonical_pinyin
            };
            fold_autocorrect_letters(reading) != typed_letters
        });
        if !correction_offered {
            return None;
        }
        self.typo_profile
            .record_suppression(&key)
            .err()
            .map(|_| diagnostics::AUTOCORRECT_PREFERENCE_NOT_PERSISTED.to_owned())
    }

    /// input_session_composition.cpp:658-691.
    pub(super) fn learn_pick_pair(
        &mut self,
        previous: &super::chain::Pick,
        current: &super::chain::Pick,
    ) -> Option<String> {
        if current
            .committed_at
            .saturating_duration_since(previous.committed_at)
            > Duration::from_secs(PICK_PAIR_MAX_GAP_SECONDS)
        {
            return None;
        }
        let word = join_words(&previous.word, &current.word);
        if count_utf8_chars(&word) > MAX_PICK_PAIR_WORD_CHARS || !is_all_han(&word) {
            return None;
        }
        let previous_key =
            normalize_canonical_pinyin_for_word(&previous.canonical_pinyin, &previous.word);
        let current_key =
            normalize_canonical_pinyin_for_word(&current.canonical_pinyin, &current.word);
        if previous_key.is_empty() || current_key.is_empty() {
            return None;
        }
        let key = append_canonical_pinyin(&previous_key, &current_key);
        let Ok(count) = record_pick_transition(
            self.journal_path(),
            &previous_key,
            &previous.word,
            &current_key,
            &current.word,
        ) else {
            return Some(diagnostics::PICK_TRANSITION_NOT_PERSISTED.to_owned());
        };
        if count < PICK_PAIR_WORD_THRESHOLD {
            return None;
        }
        // A word the user deleted must not come back from the same habit that created it.
        if !is_user_deleted(
            self.journal_path(),
            PersonalDictionaryKind::Pinyin,
            &key,
            &word,
        ) {
            // The counter stays so that the next occurrence retries the insert.
            if self
                .store_user_phrase_from_canonical_pinyin(&key, &word)
                .is_err()
            {
                return Some(diagnostics::PHRASE_NOT_PERSISTED.to_owned());
            }
            // The phrase engine only resets its own cache; the typing engine must see the new word on its next query.
            self.engine.reset_cache();
        }
        // The reference discards this result (ISC:686): a counter left behind only makes the next occurrence of the pair try the insert again, which finds the word and changes nothing.
        let _ = clear_pick_transition(
            self.journal_path(),
            &previous_key,
            &previous.word,
            &current_key,
            &current.word,
        );
        None
    }

    /// input_session.cpp:1158-1200.
    pub(super) fn record_personal_context(
        &mut self,
        selected: Option<&WordItem>,
        learned_sentence_word: bool,
        composition_left: bool,
    ) -> Option<String> {
        // Wubi never reaches here with a context: personal_context_applies() admits only quanpin and shuangpin.
        let Some(selected) = selected else {
            self.chain.reset();
            return None;
        };
        if !self.personal_context_applies() || learned_sentence_word {
            self.chain.reset();
            return None;
        }
        let words: Vec<(&str, u32)> = if selected.source.is_dictionary() {
            vec![(selected.word.as_str(), DICTIONARY_PICK_TIMES)]
        } else if selected.source.is_generated_or_fallback() && !selected.sentence_words.is_empty()
        {
            let continues = self.chain.same_composition && self.chain.previous.is_some();
            selected
                .sentence_words
                .iter()
                .enumerate()
                .map(|(index, word)| (word.as_str(), if index == 0 && continues { 2 } else { 1 }))
                .collect()
        } else {
            self.chain.reset();
            return None;
        };
        let now = self.steady_now();
        let mut transitions = Vec::with_capacity(words.len());
        for (word, times) in words {
            transitions.push(PersonalTransition {
                earlier: self.chain.earlier.clone().unwrap_or_default(),
                previous: self.chain.previous.clone().unwrap_or_default(),
                word: word.to_owned(),
                times,
            });
            self.chain.advance(word, composition_left, now);
        }
        self.personal_context
            .record(&transitions)
            .err()
            .map(|_| diagnostics::PERSONAL_CONTEXT_NOT_PERSISTED.to_owned())
    }

    /// learning, personal context, engine view, quanpin or shuangpin.
    pub(super) fn personal_context_applies(&self) -> bool {
        self.learning_enabled
            && self.personal_context_enabled
            && self.local_mode == LocalInputMode::None
            && !self.dedicated_english
            && self.scheme().reranks_with_sentence_model()
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    use rusqlite::Connection;

    use super::*;
    use crate::paths::RuntimePaths;
    use crate::session::{Clock, Session, SessionOptions};
    use crate::types::{autocorrect_type, SchemeType, ShuangpinProfileKind};
    use crate::user_dictionary::ngram_store::flush_all;

    #[test]
    fn sentence_segment_count_avoids_temporary_strings() {
        let (count, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            super::segment_count("yi'er'san'si")
        });

        assert_eq!(count, 4);
        assert_eq!(allocations, 0);
        assert_eq!(super::segment_count(""), 0);
        assert_eq!(super::segment_count("yi"), 1);
        for input in ["'", "yi''er'", "'yi"] {
            assert_eq!(
                super::segment_count(input),
                crate::pinyin::segment::split_segments(input).len()
            );
        }
    }

    struct Fixture {
        _root: tempfile::TempDir,
        paths: RuntimePaths,
    }

    impl Fixture {
        /// Pinyin rows `(key, word, weight)` and wubi rows `(code, word, weight)` in the shipped schemas, plus the lantian helpcode table the default options load.
        fn new(pinyin: &[(&str, &str, i64)], wubi: &[(&str, &str, i64)]) -> Self {
            let root = tempfile::tempdir().unwrap();
            let paths = RuntimePaths {
                resources: root.path().join("resources"),
                user_data: root.path().join("user"),
                cache: root.path().join("cache"),
                dictionaries: root.path().join("dictionaries"),
            };
            for directory in [
                &paths.resources,
                &paths.user_data,
                &paths.cache,
                &paths.dictionaries,
            ] {
                std::fs::create_dir_all(directory).unwrap();
            }
            std::fs::create_dir_all(paths.resources.join("helpcodes")).unwrap();
            std::fs::write(paths.resources.join("helpcodes/helpcode.txt"), "你=ab\n").unwrap();
            let main = Connection::open(paths.dictionary(assets::MAIN_DICTIONARY)).unwrap();
            for (key, word, weight) in pinyin {
                let table = crate::user_dictionary::journal::pinyin_table(key).unwrap();
                main.execute_batch(&format!(
                    "CREATE TABLE IF NOT EXISTS \"{table}\"(key TEXT, jp TEXT, value TEXT, weight INTEGER);"
                ))
                .unwrap();
                let jp: String = key
                    .split('\'')
                    .filter_map(|syllable| syllable.chars().next())
                    .collect();
                main.execute(
                    &format!("INSERT INTO \"{table}\" VALUES(?1,?2,?3,?4)"),
                    (key, jp, word, weight),
                )
                .unwrap();
            }
            main.execute_batch(
                "CREATE TABLE IF NOT EXISTS wubi86(key TEXT, value TEXT, weight INTEGER);",
            )
            .unwrap();
            for row in wubi {
                main.execute("INSERT INTO wubi86 VALUES(?1,?2,?3)", *row)
                    .unwrap();
            }
            crate::dictionary::english::ensure_english_schema(
                &paths.dictionary(assets::ENGLISH_DICTIONARY),
            )
            .unwrap();
            Self { _root: root, paths }
        }

        fn journal(&self) -> PathBuf {
            self.paths.user(assets::USER_JOURNAL)
        }

        fn session(&self, adjust: impl FnOnce(&mut SessionOptions)) -> Session {
            let mut options = SessionOptions::new(self.paths.clone());
            adjust(&mut options);
            Session::new(options).unwrap()
        }

        fn query(&self, path: &Path, sql: &str) -> i64 {
            flush_all();
            Connection::open(path)
                .unwrap()
                .query_row(sql, [], |row| row.get(0))
                .unwrap_or(0)
        }

        fn journal_count(&self, sql: &str) -> i64 {
            self.query(&self.journal(), sql)
        }

        fn main_count(&self, sql: &str) -> i64 {
            self.query(&self.paths.dictionary(assets::MAIN_DICTIONARY), sql)
        }
    }

    fn type_text(session: &mut Session, text: &str) {
        for byte in text.bytes() {
            session.character(byte, false);
        }
    }

    fn words(session: &Session) -> Vec<String> {
        session
            .snapshot()
            .candidates
            .into_iter()
            .map(|item| item.word)
            .collect()
    }

    fn select_word(session: &mut Session, word: &str) -> crate::types::KeyResult {
        let index = words(session)
            .iter()
            .position(|candidate| candidate == word)
            .unwrap_or_else(|| panic!("{word} is not offered: {:?}", words(session)));
        session.select(index)
    }

    fn cancel(session: &mut Session) {
        session.command(crate::types::Command::Cancel);
    }

    const FQ: [(&str, &str, i64); 6] = [
        ("ni", "甲", 100),
        ("ni", "乙", 90),
        ("ni", "丙", 80),
        ("ni", "丁", 70),
        ("ni", "戊", 60),
        ("ni", "己", 50),
    ];

    fn frequency(
        mode: FrequencyAdjustmentMode,
        trigger_count: i32,
        linear_step: i32,
    ) -> FrequencyAdjustmentOptions {
        FrequencyAdjustmentOptions {
            mode,
            trigger_count,
            linear_step,
        }
    }

    #[test]
    fn joined_words_allocates_only_result_bytes() {
        let word = join_words("你好", "世界");
        assert_eq!(word, "你好世界");
        assert_eq!(word.capacity(), word.len());
    }

    #[test]
    fn english_context_key_allocates_only_result_bytes() {
        let key = english_context_key("HeLLo");
        assert_eq!(key, "english:hello");
        assert_eq!(key.capacity(), key.len());
    }

    #[test]
    fn copying_rows_for_ranking_clones_only_matching_rows() {
        let rows = [
            WordItem::new("ni", "甲", 3, CandidateSource::Database, "ni"),
            WordItem::new("ni", "乙", 2, CandidateSource::EnglishDictionary, "ni"),
            WordItem::new("ni", "丙", 1, CandidateSource::Database, "ni"),
        ];

        let copied = clone_matching_rows(&rows, |item| item.source == CandidateSource::Database);

        assert_eq!(
            copied
                .iter()
                .map(|item| item.word.as_str())
                .collect::<Vec<_>>(),
            ["甲", "丙"]
        );
    }

    /// F1 (test_input_session.cpp:1091-1119): the pick's place in a new session, per mode.
    #[test]
    fn frequency_modes_reorder_the_next_session() {
        for (mode, step, expected) in [
            (FrequencyAdjustmentMode::Disabled, 1, 5),
            (FrequencyAdjustmentMode::Pin, 1, 0),
            (FrequencyAdjustmentMode::Halve, 1, 2),
            (FrequencyAdjustmentMode::Linear, 2, 3),
            (FrequencyAdjustmentMode::Promote, 1, 4),
        ] {
            let fixture = Fixture::new(&FQ, &[]);
            let options = frequency(mode, 1, step);
            {
                let mut session = fixture.session(|o| o.frequency = options);
                type_text(&mut session, "ni");
                let result = select_word(&mut session, "己");
                assert_eq!(result.commit.as_deref(), Some("己"));
                assert_eq!(result.diagnostic, None, "{mode:?}");
            }
            let mut session = fixture.session(|o| o.frequency = options);
            type_text(&mut session, "ni");
            assert_eq!(
                words(&session).iter().position(|word| word == "己"),
                Some(expected),
                "{mode:?}"
            );
            let state = fixture.journal_count("SELECT count(*) FROM user_dictionary_operations")
                + fixture.journal_count("SELECT count(*) FROM candidate_selection_state");
            assert_eq!(
                state > 0,
                mode != FrequencyAdjustmentMode::Disabled,
                "{mode:?}"
            );
        }
    }

    /// F2 and F3 (test_input_session.cpp:1122-1151).
    #[test]
    fn the_trigger_count_and_the_leader() {
        let fixture = Fixture::new(&FQ, &[]);
        let options = frequency(FrequencyAdjustmentMode::Pin, 2, 1);
        let mut session = fixture.session(|o| o.frequency = options);
        type_text(&mut session, "ni");
        select_word(&mut session, "己");
        type_text(&mut session, "ni");
        assert_eq!(words(&session)[5], "己");
        select_word(&mut session, "己");
        type_text(&mut session, "ni");
        assert_eq!(words(&session)[0], "己");
        cancel(&mut session);

        let fixture = Fixture::new(&FQ, &[]);
        let mut session =
            fixture.session(|o| o.frequency = frequency(FrequencyAdjustmentMode::Promote, 1, 1));
        type_text(&mut session, "ni");
        session.select(0);
        assert_eq!(
            fixture.journal_count("SELECT count(*) FROM user_dictionary_operations"),
            0
        );
        assert_eq!(
            fixture.journal_count("SELECT count(*) FROM candidate_selection_state"),
            0
        );
    }

    /// F5 and F6 (test_input_session.cpp:1174-1202): shuangpin learns under the canonical quanpin key, wubi under its code.
    #[test]
    fn shuangpin_and_wubi_persist_their_picks() {
        let fixture = Fixture::new(
            &[("ni'hao", "你好", 200), ("ni'hao", "拟好", 100)],
            &[("aaaa", "工", 200), ("aaaa", "或", 100)],
        );
        let options = frequency(FrequencyAdjustmentMode::Pin, 1, 1);
        {
            let mut session = fixture.session(|o| {
                o.frequency = options;
                o.scheme = SchemeType::Shuangpin;
                o.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
            });
            type_text(&mut session, "nihc");
            select_word(&mut session, "拟好");
            session.switch_scheme(SchemeType::Wubi).unwrap();
            type_text(&mut session, "aaaa");
            select_word(&mut session, "或");
        }
        assert_eq!(
            fixture.journal_count("SELECT count(*) FROM user_dictionary_operations WHERE dictionary='pinyin' AND key='ni''hao' AND value='拟好'"),
            1
        );
        assert_eq!(
            fixture.journal_count("SELECT count(*) FROM user_dictionary_operations WHERE dictionary='wubi' AND key='aaaa' AND value='或'"),
            1
        );
        let mut session = fixture.session(|o| {
            o.frequency = options;
            o.scheme = SchemeType::Shuangpin;
        });
        type_text(&mut session, "nihc");
        assert_eq!(words(&session)[0], "拟好");
        session.switch_scheme(SchemeType::Wubi).unwrap();
        type_text(&mut session, "aaaa");
        assert_eq!(words(&session)[0], "或");
    }

    /// 发布布局：`msime-pinyin.db` 没有五笔表，码表单独在只读的 `msime-wubi.db` 里。准备出的代次把码表并回工作主词库，五笔选词学到的权重写得进去、下一次会话读得到，换一个代次（升级）时日志里的五笔行也能回放。
    #[test]
    fn wubi_learning_survives_the_split_dictionary_layout() {
        let root = tempfile::tempdir().unwrap();
        let resources = root.path().join("resources");
        std::fs::create_dir_all(resources.join("helpcodes")).unwrap();
        std::fs::write(resources.join("helpcodes/helpcode.txt"), "你=ab\n").unwrap();
        Connection::open(resources.join(assets::MAIN_DICTIONARY))
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_1_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);
                 INSERT INTO tbl_1_n VALUES('ni','n','你',100);",
            )
            .unwrap();
        Connection::open(resources.join(assets::WUBI_DICTIONARY))
            .unwrap()
            .execute_batch(
                "CREATE TABLE wubi86(key TEXT NOT NULL, value TEXT NOT NULL, weight INTEGER NOT NULL DEFAULT 0, UNIQUE(key, value));
                 CREATE TABLE wubi98(key TEXT NOT NULL, value TEXT NOT NULL, weight INTEGER NOT NULL DEFAULT 0, UNIQUE(key, value));
                 INSERT INTO wubi86 VALUES('aaaa','工',200),('aaaa','或',100);",
            )
            .unwrap();
        crate::dictionary::english::ensure_english_schema(
            &resources.join(assets::ENGLISH_DICTIONARY),
        )
        .unwrap();
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        let open = |content_id: &str| {
            let paths = crate::user_dictionary::generation::prepare_runtime_paths(
                &resources, &user, &cache, content_id,
            )
            .unwrap();
            let mut options = SessionOptions::new(paths);
            options.frequency = frequency(FrequencyAdjustmentMode::Pin, 1, 1);
            options.scheme = SchemeType::Wubi;
            Session::new(options).unwrap()
        };
        {
            let mut session = open("v1");
            type_text(&mut session, "aaaa");
            assert_eq!(words(&session)[..2], ["工", "或"]);
            assert_eq!(select_word(&mut session, "或").diagnostic, None);
        }
        flush_all();
        for content_id in ["v1", "v2"] {
            let mut session = open(content_id);
            type_text(&mut session, "aaaa");
            assert_eq!(words(&session)[0], "或", "{content_id}");
        }
    }

    /// F7 (test_input_session.cpp:1204-1217): the commit survives, and the diagnostic carries no input text.
    #[test]
    fn a_failed_write_keeps_the_commit_with_a_diagnostic() {
        let fixture = Fixture::new(&FQ, &[]);
        std::fs::create_dir(fixture.journal()).unwrap();
        let mut session =
            fixture.session(|o| o.frequency = frequency(FrequencyAdjustmentMode::Promote, 1, 1));
        type_text(&mut session, "ni");
        let result = select_word(&mut session, "己");
        assert_eq!(result.commit.as_deref(), Some("己"));
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::FREQUENCY_NOT_PERSISTED)
        );
        assert_eq!(
            fixture.main_count("SELECT weight FROM tbl_1_n WHERE value='己'"),
            50
        );
    }

    /// test_input_session.cpp:790-803.
    #[test]
    fn learning_off_writes_nothing() {
        let fixture = Fixture::new(&[("bu'hao", "不好", 200), ("bu'hao", "补好", 100)], &[]);
        let mut session = fixture.session(|o| {
            o.learning = false;
            o.frequency = frequency(FrequencyAdjustmentMode::Pin, 1, 1);
        });
        type_text(&mut session, "buhao");
        select_word(&mut session, "补好");
        assert_eq!(
            fixture.journal_count("SELECT count(*) FROM user_dictionary_operations"),
            0
        );
        assert_eq!(
            fixture.journal_count("SELECT count(*) FROM personal_bigram"),
            0
        );
        let mut session = fixture.session(|_| {});
        type_text(&mut session, "buhao");
        assert_eq!(words(&session)[0], "不好");
    }

    const PC: [(&str, &str, i64); 6] = [
        ("ni", "甲", 300),
        ("ni", "乙", 200),
        ("ni", "丙", 100),
        ("hao", "子", 300),
        ("hao", "丑", 200),
        ("hao", "寅", 100),
    ];

    /// test_personal_context_input_session.cpp:545-589, 681-762: each pick is counted twice after the word before it, a pause or a break starts a new chain, and the switch or learning off records nothing.
    #[test]
    fn picks_record_their_context_and_breaks_reset_it() {
        let fixture = Fixture::new(&PC, &[]);
        let now = Arc::new(Mutex::new(Instant::now()));
        let clock_now = Arc::clone(&now);
        let mut session = fixture.session(|_| {});
        session.set_clock(Clock {
            steady: Box::new(move || *clock_now.lock().unwrap()),
            ..Clock::default()
        });
        type_text(&mut session, "ni");
        select_word(&mut session, "丙");
        type_text(&mut session, "hao");
        select_word(&mut session, "寅");
        assert_eq!(
            fixture.journal_count(
                "SELECT count FROM personal_bigram WHERE previous=char(1) AND word='丙'"
            ),
            2
        );
        assert_eq!(
            fixture.journal_count(
                "SELECT count FROM personal_bigram WHERE previous='丙' AND word='寅'"
            ),
            2
        );
        assert_eq!(fixture.journal_count("SELECT count FROM personal_trigram WHERE earlier=char(1) AND previous='丙' AND word='寅'"), 2);

        // Punctuation ends the run: the next pick starts a chain.
        session.punctuation(b',');
        type_text(&mut session, "hao");
        select_word(&mut session, "丑");
        assert_eq!(
            fixture.journal_count(
                "SELECT count FROM personal_bigram WHERE previous=char(1) AND word='丑'"
            ),
            2
        );

        // A pause longer than the chain window starts one too.
        *now.lock().unwrap() += Duration::from_secs(30);
        type_text(&mut session, "ni");
        select_word(&mut session, "乙");
        assert_eq!(
            fixture.journal_count("SELECT count(*) FROM personal_bigram WHERE previous='丑'"),
            0
        );

        // Personal context off: nothing further is recorded.
        let before = fixture.journal_count("SELECT sum(count) FROM personal_bigram");
        session.set_personal_context_enabled(false);
        type_text(&mut session, "ni");
        select_word(&mut session, "甲");
        assert_eq!(
            fixture.journal_count("SELECT sum(count) FROM personal_bigram"),
            before
        );
    }

    const PP: [(&str, &str, i64); 5] = [
        ("shan", "闪", 200),
        ("shan", "山", 100),
        ("shui", "睡", 200),
        ("shui", "水", 100),
        // Gives the two-syllable table the learned word goes into.
        ("shan'shan", "闪闪", 10),
    ];

    /// test_pick_pair_input_session.cpp:168-197, 300-330: a pair picked three times becomes a word; a removed word does not come back; picks too far apart do not count.
    #[test]
    fn a_pair_picked_three_times_becomes_a_word() {
        let fixture = Fixture::new(&PP, &[]);
        let mut session = fixture.session(|_| {});
        for round in 1..=3 {
            type_text(&mut session, "shan");
            assert_eq!(select_word(&mut session, "山").diagnostic, None);
            type_text(&mut session, "shui");
            assert_eq!(select_word(&mut session, "水").diagnostic, None);
            // A space the host inserts breaks the run, so every round starts afresh.
            session.character(b' ', false);
            let stored = fixture
                .main_count("SELECT count(*) FROM tbl_2_s WHERE key='shan''shui' AND value='山水'");
            assert_eq!(stored, i64::from(round == 3), "round {round}");
        }
        assert_eq!(
            fixture.journal_count(
                "SELECT count(*) FROM pick_transitions WHERE previous_value='山' AND value='水'"
            ),
            0,
            "the counter outlived the word it created"
        );
        assert_eq!(
            fixture.journal_count("SELECT user_inserted FROM user_dictionary_operations WHERE key='shan''shui' AND value='山水'"),
            1
        );

        // Deleted, the habit does not bring it back.
        type_text(&mut session, "shanshui");
        let index = words(&session)
            .iter()
            .position(|word| word == "山水")
            .unwrap();
        assert_eq!(session.remove(index).diagnostic, None);
        cancel(&mut session);
        for _ in 0..3 {
            type_text(&mut session, "shan");
            select_word(&mut session, "山");
            type_text(&mut session, "shui");
            select_word(&mut session, "水");
            session.character(b' ', false);
        }
        assert_eq!(
            fixture.main_count("SELECT count(*) FROM tbl_2_s WHERE value='山水'"),
            0
        );
    }

    /// test_pick_pair_input_session.cpp:236-250: picks with a long pause between them are not one phrase.
    #[test]
    fn picks_after_a_long_pause_are_not_a_pair() {
        let fixture = Fixture::new(&PP, &[]);
        let now = Arc::new(Mutex::new(Instant::now()));
        let clock_now = Arc::clone(&now);
        let mut session = fixture.session(|_| {});
        session.set_clock(Clock {
            steady: Box::new(move || *clock_now.lock().unwrap()),
            ..Clock::default()
        });
        type_text(&mut session, "shan");
        select_word(&mut session, "山");
        *now.lock().unwrap() += Duration::from_secs(PICK_PAIR_MAX_GAP_SECONDS + 1);
        type_text(&mut session, "shui");
        select_word(&mut session, "水");
        assert_eq!(
            fixture.journal_count("SELECT count(*) FROM pick_transitions"),
            0
        );
    }

    /// test_typo_correction_input_session.cpp:400-440: committing the raw letters while a correction is offered records the input once, and then that input is answered as typed.
    #[test]
    fn committing_raw_letters_suppresses_the_correction() {
        let fixture = Fixture::new(&[("shang", "上", 100), ("sha", "沙", 50)], &[]);
        let types = autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR;
        let mut session = fixture.session(|o| o.autocorrect_types = types);
        type_text(&mut session, "shabg");
        let corrected = session
            .snapshot()
            .candidates
            .iter()
            .any(|item| !item.corrected_from.is_empty());
        assert!(
            corrected,
            "the neighbor correction was not offered: {:?}",
            words(&session)
        );
        let raw = session.command(crate::types::Command::CommitRaw);
        assert_eq!(raw.commit.as_deref(), Some("shabg"));
        assert_eq!(raw.diagnostic, None);
        assert_eq!(
            fixture.journal_count(
                "SELECT commits FROM pinyin_autocorrect_suppressions WHERE input='shabg'"
            ),
            1
        );
        type_text(&mut session, "shabg");
        assert!(session
            .snapshot()
            .candidates
            .iter()
            .all(|item| item.corrected_from.is_empty()));
        cancel(&mut session);

        // With learning off nothing is recorded.
        let quiet = Fixture::new(&[("shang", "上", 100)], &[]);
        let mut session = quiet.session(|o| {
            o.autocorrect_types = types;
            o.learning = false;
        });
        type_text(&mut session, "shabg");
        session.command(crate::types::Command::CommitRaw);
        assert_eq!(
            quiet.journal_count("SELECT count(*) FROM pinyin_autocorrect_suppressions"),
            0
        );
    }

    /// 904bd0976 / 8d046e269: mixed Wubi asks quanpin for the same letters, and that request honours the refusal too, so a correction the user turned down by committing raw does not come back through the Wubi pinyin fallback.
    #[test]
    fn a_refused_correction_stays_refused_in_mixed_wubi() {
        let fixture = Fixture::new(&[("shang", "上", 100), ("sha", "沙", 50)], &[]);
        let types = autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR;
        let mixed_wubi = |o: &mut SessionOptions| {
            o.scheme = SchemeType::Wubi;
            o.wubi.mixed_pinyin = true;
            o.autocorrect_types = types;
        };
        let offers_correction = |session: &Session| {
            session
                .snapshot()
                .candidates
                .iter()
                .any(|item| !item.corrected_from.is_empty())
        };

        // Without a refusal the fallback offers the neighbor correction, otherwise the check below proves nothing.
        let mut wubi = fixture.session(mixed_wubi);
        type_text(&mut wubi, "shabg");
        assert_eq!(wubi.snapshot().editing_text, "shabg");
        assert!(
            offers_correction(&wubi),
            "mixed wubi did not offer the correction: {:?}",
            words(&wubi)
        );
        cancel(&mut wubi);
        drop(wubi);

        // The refusal is learned where it can be: in quanpin, by committing the raw letters.
        let mut quanpin = fixture.session(|o| o.autocorrect_types = types);
        type_text(&mut quanpin, "shabg");
        let raw = quanpin.command(crate::types::Command::CommitRaw);
        assert_eq!(raw.commit.as_deref(), Some("shabg"));
        assert_eq!(
            fixture.journal_count(
                "SELECT commits FROM pinyin_autocorrect_suppressions WHERE input='shabg'"
            ),
            1
        );
        drop(quanpin);

        let mut wubi = fixture.session(mixed_wubi);
        type_text(&mut wubi, "shabg");
        assert_eq!(wubi.snapshot().editing_text, "shabg");
        assert!(
            !offers_correction(&wubi),
            "the refused correction came back through mixed wubi: {:?}",
            words(&wubi)
        );
    }

    /// 904bd0976: a sentence a neural model picked has no dictionary row to re-rank, so selecting it stores the sentence as a user phrase like a lattice sentence. Needs the keyboard model from `MSIME_EVAL_RESOURCES`.
    #[test]
    fn a_neural_sentence_is_learned_as_a_sentence() {
        let model = match crate::lattice::neural::test_model_path(assets::NEURAL_MODEL_KEYBOARD) {
            Ok(model) => model,
            Err(reason) => {
                eprintln!("skipping a_neural_sentence_is_learned_as_a_sentence: {reason}");
                return;
            }
        };
        let fixture = Fixture::new(
            &[
                ("shu'ru", "输入", 20000),
                ("fa", "法", 800000),
                ("fa", "发", 900000),
                ("fa", "罚", 100000),
            ],
            &[],
        );
        std::fs::copy(
            &model,
            fixture.paths.resource(assets::NEURAL_MODEL_KEYBOARD),
        )
        .unwrap();
        Connection::open(fixture.paths.dictionary(assets::MAIN_DICTIONARY))
            .unwrap()
            .execute_batch("CREATE TABLE IF NOT EXISTS tbl_3_s(key TEXT, jp TEXT, value TEXT, weight INTEGER);")
            .unwrap();
        let mut session = fixture.session(|o| {
            o.sentence_association.neural_keyboard = true;
            o.sentence_association.show_next_on_duplicate = true;
        });
        type_text(&mut session, "shurufa");
        let neural = session
            .snapshot()
            .candidates
            .into_iter()
            .find(|item| item.source == CandidateSource::NeuralKeyboard)
            .unwrap_or_else(|| panic!("no neural row: {:?}", words(&session)));
        let result = select_word(&mut session, &neural.word);
        assert_eq!(result.commit.as_deref(), Some(neural.word.as_str()));
        assert_eq!(result.diagnostic, None);
        assert_eq!(
            fixture.main_count(&format!(
                "SELECT count(*) FROM tbl_3_s WHERE key='shu''ru''fa' AND value='{}'",
                neural.word
            )),
            1
        );
        assert_eq!(
            fixture.journal_count(&format!(
                "SELECT user_inserted FROM user_dictionary_operations WHERE key='shu''ru''fa' AND value='{}'",
                neural.word
            )),
            1
        );
    }

    /// test_online_input_session.cpp (personal-learning overlay): a selected cloud word is stored under the typed reading when every character can have it; a character known under other readings only is refused, one no table knows is accepted.
    #[test]
    fn online_words_are_learned_only_under_a_reading_they_can_have() {
        let fixture = Fixture::new(
            &[
                ("ni", "你", 100),
                ("hao", "好", 100),
                ("ma", "马", 100),
                // Gives the two-syllable table the learned word goes into.
                ("ni'ni", "妮妮", 1),
            ],
            &[],
        );
        let mut session = fixture.session(|_| {});
        for (word, stored) in [("拟好", true), ("你马", false)] {
            type_text(&mut session, "nihao");
            let query = session
                .online_query()
                .expect("a complete pinyin input asks online");
            assert!(session.apply_online_candidates(
                &query,
                &[word.to_owned()],
                CandidateSource::CloudSuggestion
            ));
            let result = select_word(&mut session, word);
            assert_eq!(result.commit.as_deref(), Some(word));
            assert_eq!(result.diagnostic, None);
            let rows = fixture.main_count(&format!(
                "SELECT count(*) FROM tbl_2_n WHERE key='ni''hao' AND value='{word}'"
            ));
            assert_eq!(rows, i64::from(stored), "{word}");
        }
        assert_eq!(
            fixture.journal_count("SELECT user_inserted FROM user_dictionary_operations WHERE key='ni''hao' AND value='拟好'"),
            1
        );
    }

    /// Fixture F_TC (test_typo_correction_input_session.cpp:120-142).
    fn typo_fixture() -> Fixture {
        let fixture = Fixture::new(
            &[
                ("mei", "没", 1000),
                ("mei", "美", 900),
                ("gan", "干", 1000),
                ("guan", "关", 500),
                ("xi", "系", 1000),
                ("guan'xi", "关系", 100_000),
            ],
            &[],
        );
        Connection::open(fixture.paths.dictionary(assets::MAIN_DICTIONARY))
            .unwrap()
            .execute_batch("CREATE TABLE IF NOT EXISTS tbl_3_m(key TEXT, jp TEXT, value TEXT, weight INTEGER);")
            .unwrap();
        fixture
    }

    /// test_typo_correction_input_session.cpp:314-335: selecting the typo sentence counts the accepted syllable typo and stores the sentence under its corrected reading; with learning off nothing is counted.
    #[test]
    fn an_accepted_typo_sentence_is_counted_and_stored() {
        let all_types = autocorrect_type::TRANSPOSITION
            | autocorrect_type::NEIGHBOR
            | autocorrect_type::MISSING_OR_EXTRA;
        for learning in [true, false] {
            let fixture = typo_fixture();
            let mut session = fixture.session(|o| {
                o.autocorrect_types = all_types;
                o.learning = learning;
            });
            type_text(&mut session, "meiganxi");
            let result = select_word(&mut session, "没关系");
            assert_eq!(result.commit.as_deref(), Some("没关系"));
            assert_eq!(result.diagnostic, None);
            let typos = fixture.journal_count(
                "SELECT accepted FROM pinyin_typo_counts WHERE typed='gan' AND intended='guan'",
            );
            assert_eq!(typos, i64::from(learning));
            let stored = fixture.main_count(
                "SELECT count(*) FROM tbl_3_m WHERE key='mei''guan''xi' AND value='没关系'",
            );
            assert_eq!(stored, i64::from(learning));
            if learning {
                assert_eq!(
                    crate::user_dictionary::typo_profile::PersonalTypoProfile::shared(
                        &fixture.journal()
                    )
                    .accepted("gan", "guan"),
                    1
                );
            }
        }
    }

    /// The first row the session would store as a sentence, if any.
    fn sentence_row(session: &Session) -> Option<(usize, String)> {
        session
            .snapshot()
            .candidates
            .iter()
            .position(|item| item.source.is_sentence_learning())
            .map(|index| (index, session.snapshot().candidates[index].word.clone()))
    }

    /// ISC:51-53: a generated sentence longer than `MAX_LEARNED_SENTENCE_SYLLABLES` is committed but never stored; one at the cap is.
    #[test]
    fn a_sentence_longer_than_the_cap_is_committed_but_not_stored() {
        let fixture = Fixture::new(&[("yi", "一", 100), ("er", "二", 100)], &[]);
        // Both tables exist, so only the cap can keep the longer sentence out.
        Connection::open(fixture.paths.dictionary(assets::MAIN_DICTIONARY))
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_7_y(key TEXT, jp TEXT, value TEXT, weight INTEGER);\
                 CREATE TABLE tbl_others_y(key TEXT, jp TEXT, value TEXT, weight INTEGER);",
            )
            .unwrap();
        for syllables in [
            MAX_LEARNED_SENTENCE_SYLLABLES + 1,
            MAX_LEARNED_SENTENCE_SYLLABLES,
        ] {
            let mut session = fixture.session(|_| {});
            type_text(&mut session, &"yi".repeat(syllables));
            let (index, word) = sentence_row(&session).unwrap_or_else(|| {
                panic!(
                    "no generated sentence for {syllables} syllables: {:?}",
                    words(&session)
                )
            });
            assert_eq!(word.chars().count(), syllables);
            let result = session.select(index);
            assert_eq!(result.commit.as_deref(), Some(word.as_str()));
            assert_eq!(result.diagnostic, None);
            // The SQL literal of yi'yi'...: each apostrophe doubled.
            let key = vec!["yi"; syllables].join("''");
            let stored = fixture.journal_count(&format!(
                "SELECT count(*) FROM user_dictionary_operations WHERE key='{key}' AND value='{word}' AND user_inserted=1"
            ));
            let table = crate::format::quanpin_table(syllables, b'y').unwrap();
            let in_main = fixture.main_count(&format!(
                "SELECT count(*) FROM {table} WHERE value='{word}'"
            ));
            if syllables > MAX_LEARNED_SENTENCE_SYLLABLES {
                assert_eq!(
                    fixture.journal_count("SELECT count(*) FROM user_dictionary_operations"),
                    0
                );
                assert_eq!(in_main, 0);
            } else {
                assert_eq!(stored, 1);
                assert_eq!(in_main, 1);
            }
        }
    }

    /// input_session_composition.cpp:562-600: the English and Japanese modes offer Generated and online rows too, but none of them is a pinyin sentence, so selecting one stores nothing. Today these rows also carry no canonical pinyin, so the empty-reading check rejects them as well; the test pins the outcome, whichever check makes it.
    #[test]
    fn generated_rows_outside_pinyin_composition_are_not_stored() {
        let fixture = Fixture::new(&[("ni", "你", 100), ("hao", "好", 100)], &[]);
        for mode in [
            "temporary English",
            "dedicated English",
            "temporary Japanese",
        ] {
            let mut session = fixture.session(|_| {});
            match mode {
                "temporary English" => {
                    assert!(session.character(b'Y', true).handled);
                    type_text(&mut session, "zzq");
                }
                "dedicated English" => {
                    session.set_dedicated_english(true);
                    type_text(&mut session, "zzq");
                }
                _ => {
                    assert!(session.character(b'R', true).handled);
                    type_text(&mut session, "ka");
                }
            }
            let (index, word) = sentence_row(&session)
                .unwrap_or_else(|| panic!("{mode} offers no generated row: {:?}", words(&session)));
            let result = session.select(index);
            assert_eq!(result.commit.as_deref(), Some(word.as_str()), "{mode}");
            assert_eq!(
                fixture.journal_count("SELECT count(*) FROM user_dictionary_operations"),
                0,
                "{mode} stored {word}"
            );
        }
        assert_eq!(fixture.main_count("SELECT count(*) FROM tbl_1_n"), 1);

        // A cloud answer to Japanese `ka` is an online row, which the session otherwise stores under the typed reading.
        let mut session = fixture.session(|o| o.scheme = SchemeType::JapaneseRomaji);
        type_text(&mut session, "ka");
        let query = session.online_query().expect("a Japanese cloud query");
        assert!(session.apply_online_candidates(
            &query,
            &["蚊".to_owned()],
            CandidateSource::CloudSuggestion
        ));
        let result = select_word(&mut session, "蚊");
        assert_eq!(result.commit.as_deref(), Some("蚊"));
        assert_eq!(
            fixture.journal_count("SELECT count(*) FROM user_dictionary_operations"),
            0
        );
    }

    /// test_runtime_isolation.cpp:618-642: a pin the journal refuses is reported, and the list keeps its order.
    #[test]
    fn a_failed_pin_reports_and_keeps_the_order() {
        let fixture = Fixture::new(&FQ, &[]);
        std::fs::create_dir(fixture.journal()).unwrap();
        let mut session = fixture.session(|o| o.learning = false);
        type_text(&mut session, "ni");
        let before = session.snapshot();
        let index = words(&session)
            .iter()
            .position(|word| word == "己")
            .unwrap();
        let result = session.pin(index);
        assert!(result.handled);
        assert_eq!(result.commit, None);
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::FREQUENCY_NOT_PERSISTED)
        );
        let after = session.snapshot();
        assert_eq!(after.preedit, before.preedit);
        assert_eq!(after.candidates[0].word, before.candidates[0].word);
        assert_eq!(
            fixture.main_count("SELECT weight FROM tbl_1_n WHERE value='己'"),
            50
        );
    }
}
