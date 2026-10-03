//! `CandidateQueries` (core-session.md §7.2, §10; schemes-lang.md §4.5): local mode dispatch and the mixed English / emoji / kaomoji insertion into pinyin lists.

use std::collections::HashSet;

use crate::assets;
use crate::dictionary::english::EnglishDictionary;
use crate::local::command::{
    command_title, query_command, translation_source, usable_command_table,
};
use crate::local::date_time::{query_date_time, LocalDateTime};
use crate::local::emoji::{query_emoji, query_kaomoji, MIXED_RESULT_LIMIT, MODE_RESULT_LIMIT};
use crate::local::expression::query_expression;
use crate::local::jianpin::{query_jianpin, result_limit};
use crate::local::mention::{mention_annotation, query_mentions, usable_mentions};
use crate::local::quick_phrase::{
    merge_quick_phrases, query_quick_phrases, usable_quick_phrase_table,
};
use crate::local::unicode::query_unicode;
use crate::local::LocalQueryResult;
use crate::paths::RuntimePaths;
use crate::shuangpin::profile::profile;
use crate::types::{
    CandidateSource, CommandTableEntry, EnglishInputOptions, LocalInputMode, MentionEntry,
    MixedExpressiveOptions, QuickPhraseEntry, SchemeType, ShuangpinProfileKind, WordItem,
};

pub const MIXED_ENGLISH_LIMIT: usize = 5;
pub const MODE_ENGLISH_LIMIT: usize = 1_000;

pub struct CandidateQueries {
    paths: RuntimePaths,
    profile: ShuangpinProfileKind,
    english: Option<EnglishDictionary>,
    /// The host's command table, usable rows only.
    command_table: Vec<CommandTableEntry>,
    /// 宿主的短语表（插件），只保留能用的行，按编码排序。
    quick_phrase_table: Vec<QuickPhraseEntry>,
    /// The host's mention list, usable entries only.
    mentions: Vec<MentionEntry>,
    /// Whether `@` mode offers the embedded places after the list.
    mention_places: bool,
}

impl CandidateQueries {
    pub fn new(paths: &RuntimePaths, profile: ShuangpinProfileKind) -> Self {
        Self {
            paths: paths.clone(),
            profile,
            english: None,
            command_table: Vec::new(),
            quick_phrase_table: Vec::new(),
            mentions: Vec::new(),
            mention_places: false,
        }
    }

    /// Keeps the rows `/` mode can use and drops the rest.
    pub fn set_command_table(&mut self, table: &[CommandTableEntry]) {
        self.command_table = usable_command_table(table);
    }

    /// 保留 K 模式能用的宿主短语行，丢弃其余。
    pub fn set_quick_phrase_table(&mut self, table: &[QuickPhraseEntry]) {
        self.quick_phrase_table = usable_quick_phrase_table(table);
    }

    /// Keeps the entries `@` mode can use and drops the rest.
    pub fn set_mentions(&mut self, entries: &[MentionEntry]) {
        self.mentions = usable_mentions(entries);
    }

    /// Turns the embedded places of `@` mode on or off.
    pub fn set_mention_places(&mut self, enabled: bool) {
        self.mention_places = enabled;
    }

    /// The annotation of an `@` row: a place's parent division, empty for the user's own entries.
    pub fn mention_annotation(&self, text: &str) -> &'static str {
        if !self.mention_places {
            return "";
        }
        mention_annotation(text, &self.mentions)
    }

    /// The translate command's trigger and English for the letters after `/`, against the live command table.
    pub fn translation_source(&self, code: &str) -> Option<(&'static str, String)> {
        translation_source(code, &self.command_table)
    }

    /// The title of a `/` mode row, by the trigger its `pinyin` holds.
    pub fn command_title(&self, trigger: &str) -> Option<&str> {
        command_title(trigger, &self.command_table)
    }

    /// The English dictionary, opened on first use from the generation copy with the resource translations sidecar and the learned-gloss store (candidate_queries.cpp:197-207). The store is a user file because the generation copy is replaced on every new generation and would lose what is written into it; it is `translation-glosses.db` rather than the contract's `gloss_cache.db` because that is the file the host writes and users have (data-formats.md §1.4, §11).
    pub fn english_dictionary(&mut self) -> &EnglishDictionary {
        let paths = &self.paths;
        self.english.get_or_insert_with(|| {
            EnglishDictionary::open(
                &paths.dictionary(assets::ENGLISH_DICTIONARY),
                Some(&paths.resource(assets::TRANSLATIONS)),
                Some(&paths.user(assets::LEARNED_GLOSSES)),
            )
        })
    }

    /// The rows of a local mode for its preedit (prefix letter included); `engine_candidates` are the Japanese provider rows for temporary Japanese.
    pub fn local(
        &mut self,
        mode: LocalInputMode,
        preedit: &str,
        scheme: SchemeType,
        now: &LocalDateTime,
        engine_candidates: &[WordItem],
    ) -> LocalQueryResult {
        // Every local preedit starts with its ASCII prefix letter (candidate_queries.cpp:15-99 strips it with `substr(1)`).
        let code = preedit.get(1..).unwrap_or_default();
        let shuangpin = profile(self.profile);
        let rows = |candidates| LocalQueryResult {
            candidates,
            diagnostic: None,
        };
        match mode {
            LocalInputMode::None => LocalQueryResult::default(),
            LocalInputMode::Unicode => rows(query_unicode(code)),
            LocalInputMode::DateTime => rows(query_date_time(code, now)),
            LocalInputMode::QuickPhrase => merge_quick_phrases(
                code,
                query_quick_phrases(code, &self.paths.dictionary(assets::MAIN_DICTIONARY)),
                &self.quick_phrase_table,
            ),
            LocalInputMode::Emoji => query_emoji(
                code,
                scheme,
                &self.paths.resource(assets::OTHER_DICTIONARY),
                MODE_RESULT_LIMIT,
                shuangpin,
            ),
            LocalInputMode::Kaomoji => query_kaomoji(
                code,
                scheme,
                &self.paths.resource(assets::OTHER_DICTIONARY),
                MODE_RESULT_LIMIT,
                shuangpin,
            ),
            LocalInputMode::SuperJianpin => query_jianpin(
                code,
                scheme,
                &self.paths.dictionary(assets::MAIN_DICTIONARY),
                result_limit(code),
                shuangpin,
            ),
            LocalInputMode::TemporaryEnglish => rows(self.temporary_english(code)),
            LocalInputMode::TemporaryJapanese => rows(engine_candidates.to_vec()),
            LocalInputMode::Expression => rows(query_expression(code)),
            LocalInputMode::Command => rows(query_command(code, now, &self.command_table)),
            LocalInputMode::Mention => {
                rows(query_mentions(code, &self.mentions, self.mention_places))
            }
            // 网址模式不查任何候选，只留显示整段预编辑的兜底行。
            LocalInputMode::Url => LocalQueryResult::default(),
        }
    }

    /// The typed text as a Generated row, then the completions of its lowercase form other than the typed word itself (candidate_queries.cpp:62-90).
    fn temporary_english(&mut self, raw: &str) -> Vec<WordItem> {
        if raw.is_empty() {
            return Vec::new();
        }
        let completions = self
            .english_dictionary()
            .query_prefix(&raw.to_ascii_lowercase(), MODE_ENGLISH_LIMIT);
        let mut candidates = Vec::with_capacity(completions.len().saturating_add(1));
        candidates.push(WordItem::new("", raw, 0, CandidateSource::Generated, ""));
        candidates.extend(
            completions
                .into_iter()
                .filter(|candidate| !candidate.word.eq_ignore_ascii_case(raw)),
        );
        candidates
    }

    /// Insert the first English, emoji and kaomoji rows at the priority slot and append the rest (candidate_queries.cpp:101-195). Pass-through outside quanpin and shuangpin, in local or dedicated modes, or with nothing enabled.
    #[allow(clippy::too_many_arguments)]
    pub fn mixed(
        &mut self,
        candidates: Vec<WordItem>,
        prefix: &str,
        scheme: SchemeType,
        english: EnglishInputOptions,
        expressive: MixedExpressiveOptions,
        dedicated_english: bool,
        local_mode: LocalInputMode,
    ) -> Vec<WordItem> {
        let anything_enabled = english.mixed_candidates
            || expressive.emoji_candidates
            || expressive.kaomoji_candidates;
        if !anything_enabled
            || dedicated_english
            || local_mode != LocalInputMode::None
            || !scheme.allows_english_emoji_mixing()
            || prefix.is_empty()
        {
            return candidates;
        }

        let english_rows = if english.mixed_candidates
            && prefix.len() >= english.minimum_prefix
            && prefix.bytes().all(|byte| byte.is_ascii_lowercase())
        {
            self.english_dictionary()
                .query_prefix(prefix, MIXED_ENGLISH_LIMIT)
        } else {
            Vec::new()
        };
        // Emoji and kaomoji are keyed by pinyin, so a single letter would match far too much.
        let others_db = self.paths.resource(assets::OTHER_DICTIONARY);
        let shuangpin = profile(self.profile);
        let emoji_rows = if expressive.emoji_candidates && prefix.len() >= 2 {
            query_emoji(prefix, scheme, &others_db, MIXED_RESULT_LIMIT, shuangpin).candidates
        } else {
            Vec::new()
        };
        let kaomoji_rows = if expressive.kaomoji_candidates && prefix.len() >= 2 {
            query_kaomoji(prefix, scheme, &others_db, MIXED_RESULT_LIMIT, shuangpin).candidates
        } else {
            Vec::new()
        };
        insert_mixed_rows(candidates, english_rows, emoji_rows, kaomoji_rows)
    }
}

/// Each extra list is deduplicated by word against the list and the lists before it; the first row of each goes to the priority slot (after the leading row, and after the cloud and AI rows when present), the rest to the end.
fn insert_mixed_rows(
    mut candidates: Vec<WordItem>,
    english: Vec<WordItem>,
    emoji: Vec<WordItem>,
    kaomoji: Vec<WordItem>,
) -> Vec<WordItem> {
    // Borrow the existing words while filtering; release those borrows before moving rows into the result groups.
    let mut seen: HashSet<&str> = candidates.iter().map(|item| item.word.as_str()).collect();
    let english_unique = unique_mask(&english, &mut seen);
    let emoji_unique = unique_mask(&emoji, &mut seen);
    let kaomoji_unique = unique_mask(&kaomoji, &mut seen);
    drop(seen);
    let groups: [Vec<WordItem>; 3] = [
        english
            .into_iter()
            .zip(english_unique)
            .filter_map(|(item, unique)| unique.then_some(item))
            .collect(),
        emoji
            .into_iter()
            .zip(emoji_unique)
            .filter_map(|(item, unique)| unique.then_some(item))
            .collect(),
        kaomoji
            .into_iter()
            .zip(kaomoji_unique)
            .filter_map(|(item, unique)| unique.then_some(item))
            .collect(),
    ];
    let extra = groups.iter().map(Vec::len).sum();
    candidates.reserve(extra);

    let has_source = |source| candidates.iter().any(|item| item.source == source);
    let mut slot = if has_source(CandidateSource::AiSuggestion) {
        3
    } else if has_source(CandidateSource::CloudSuggestion) {
        2
    } else {
        1
    }
    .min(candidates.len());

    let mut tails = Vec::with_capacity(groups.len());
    for group in groups {
        let mut rows = group.into_iter();
        if let Some(first) = rows.next() {
            candidates.insert(slot, first);
            slot += 1;
        }
        tails.push(rows);
    }
    for rows in tails {
        candidates.extend(rows);
    }
    candidates
}

fn unique_mask<'a>(rows: &'a [WordItem], seen: &mut HashSet<&'a str>) -> Vec<bool> {
    rows.iter()
        .map(|item| seen.insert(item.word.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;

    fn row(word: &str, source: CandidateSource) -> WordItem {
        WordItem::new("ni", word, 1, source, "")
    }

    fn words(list: &[WordItem]) -> Vec<&str> {
        list.iter().map(|item| item.word.as_str()).collect()
    }

    fn chinese() -> Vec<WordItem> {
        vec![
            row("你", CandidateSource::Database),
            row("倪", CandidateSource::Database),
        ]
    }

    #[test]
    fn english_rows_follow_the_leading_chinese_row() {
        // test_english_input_session.cpp:143-148: the English 倪 row duplicates the Chinese one and is dropped.
        let english = vec![
            row("Ni", CandidateSource::EnglishDictionary),
            row("Ninja", CandidateSource::EnglishDictionary),
            row("Nimbus", CandidateSource::EnglishDictionary),
            row("倪", CandidateSource::EnglishDictionary),
        ];
        let list = insert_mixed_rows(chinese(), english, Vec::new(), Vec::new());
        assert_eq!(words(&list), vec!["你", "Ni", "倪", "Ninja", "Nimbus"]);
        assert_eq!(list[2].source, CandidateSource::Database);
    }

    #[test]
    fn english_emoji_and_kaomoji_take_consecutive_slots() {
        // test_mixed_expressive_input_session.cpp:159-176.
        let english = vec![
            row("Ni", CandidateSource::EnglishDictionary),
            row("Ninja", CandidateSource::EnglishDictionary),
        ];
        let emoji = vec![
            row("😀", CandidateSource::Emoji),
            row("😁", CandidateSource::Emoji),
        ];
        let kaomoji = vec![
            row("(^_^)", CandidateSource::Kaomoji),
            row("(T_T)", CandidateSource::Kaomoji),
        ];
        let list = insert_mixed_rows(chinese(), english, emoji, kaomoji);
        assert_eq!(
            words(&list),
            vec!["你", "Ni", "😀", "(^_^)", "倪", "Ninja", "😁", "(T_T)"]
        );
    }

    #[test]
    fn online_rows_push_the_priority_slot_back() {
        let mut list = chinese();
        list.insert(1, row("云", CandidateSource::CloudSuggestion));
        let with_cloud = insert_mixed_rows(
            list.clone(),
            vec![row("Ni", CandidateSource::EnglishDictionary)],
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(words(&with_cloud), vec!["你", "云", "Ni", "倪"]);

        list.insert(2, row("智", CandidateSource::AiSuggestion));
        let with_ai = insert_mixed_rows(
            list,
            vec![row("Ni", CandidateSource::EnglishDictionary)],
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(words(&with_ai), vec!["你", "云", "智", "Ni", "倪"]);
    }

    #[test]
    fn an_empty_list_starts_with_the_extra_rows() {
        let list = insert_mixed_rows(
            Vec::new(),
            vec![
                row("Ni", CandidateSource::EnglishDictionary),
                row("Ninja", CandidateSource::EnglishDictionary),
            ],
            vec![row("😀", CandidateSource::Emoji)],
            Vec::new(),
        );
        assert_eq!(words(&list), vec!["Ni", "😀", "Ninja"]);
    }

    #[test]
    fn mixed_rows_reserve_the_extra_candidate_capacity() {
        let list = insert_mixed_rows(
            vec![row("你", CandidateSource::Database)],
            (0..5)
                .map(|index| row(&format!("en{index}"), CandidateSource::EnglishDictionary))
                .collect(),
            (0..3)
                .map(|index| row(&format!("😀{index}"), CandidateSource::Emoji))
                .collect(),
            (0..3)
                .map(|index| row(&format!("ka{index}"), CandidateSource::Kaomoji))
                .collect(),
        );

        assert_eq!(list.len(), 12);
        assert_eq!(list.capacity(), 12);
    }

    #[test]
    fn the_extra_lists_are_deduplicated_against_each_other() {
        let list = insert_mixed_rows(
            chinese(),
            vec![row("Ni", CandidateSource::EnglishDictionary)],
            vec![
                row("Ni", CandidateSource::Emoji),
                row("😀", CandidateSource::Emoji),
                row("😀", CandidateSource::Emoji),
            ],
            vec![row("😀", CandidateSource::Kaomoji)],
        );
        assert_eq!(words(&list), vec!["你", "Ni", "😀", "倪"]);
        assert_eq!(list[2].source, CandidateSource::Emoji);
    }

    #[test]
    fn mixed_passes_through_when_it_does_not_apply() {
        let mut queries =
            CandidateQueries::new(&RuntimePaths::default(), ShuangpinProfileKind::Xiaohe);
        let on = EnglishInputOptions {
            mixed_candidates: true,
            minimum_prefix: 2,
        };
        let expressive_on = MixedExpressiveOptions {
            emoji_candidates: true,
            kaomoji_candidates: true,
        };
        let cases = [
            (
                SchemeType::Quanpin,
                EnglishInputOptions::default(),
                MixedExpressiveOptions::default(),
                false,
                LocalInputMode::None,
                "ni",
            ),
            (
                SchemeType::Wubi,
                on,
                expressive_on,
                false,
                LocalInputMode::None,
                "ni",
            ),
            (
                SchemeType::JapaneseRomaji,
                on,
                expressive_on,
                false,
                LocalInputMode::None,
                "ni",
            ),
            (
                SchemeType::Korean,
                on,
                expressive_on,
                false,
                LocalInputMode::None,
                "ni",
            ),
            (
                SchemeType::Quanpin,
                on,
                expressive_on,
                true,
                LocalInputMode::None,
                "ni",
            ),
            (
                SchemeType::Shuangpin,
                on,
                expressive_on,
                false,
                LocalInputMode::Emoji,
                "ni",
            ),
            (
                SchemeType::Quanpin,
                on,
                expressive_on,
                false,
                LocalInputMode::None,
                "",
            ),
        ];
        for (scheme, english, expressive, dedicated, local, prefix) in cases {
            let list = queries.mixed(
                chinese(),
                prefix,
                scheme,
                english,
                expressive,
                dedicated,
                local,
            );
            assert_eq!(
                list,
                chinese(),
                "{scheme:?} {dedicated} {local:?} {prefix:?}"
            );
        }
        assert!(
            queries.english.is_none(),
            "no pass-through opens the English dictionary"
        );
    }

    #[test]
    fn temporary_english_reserves_the_generated_row_capacity() {
        let directory = tempfile::tempdir().unwrap();
        let words = [
            ("he00", "he00", 0),
            ("he01", "he01", 0),
            ("he02", "he02", 0),
            ("he03", "he03", 0),
            ("he04", "he04", 0),
            ("he05", "he05", 0),
            ("he06", "he06", 0),
            ("he07", "he07", 0),
            ("he08", "he08", 0),
            ("he09", "he09", 0),
        ];
        let database = directory.path().join(assets::ENGLISH_DICTIONARY);
        crate::ensure_english_schema(&database).unwrap();
        let connection = Connection::open(&database).unwrap();
        for word in words {
            connection
                .execute(
                    "INSERT INTO english_words(word,display,weight) VALUES(?1,?2,?3)",
                    word,
                )
                .unwrap();
        }
        let paths = RuntimePaths {
            dictionaries: directory.path().to_owned(),
            ..RuntimePaths::default()
        };
        let mut queries = CandidateQueries::new(&paths, ShuangpinProfileKind::Xiaohe);

        let candidates = queries.temporary_english("he");

        assert_eq!(candidates.len(), 11);
        assert_eq!(candidates.capacity(), 11);
    }
}
