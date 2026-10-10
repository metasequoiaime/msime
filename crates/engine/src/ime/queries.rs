//! `CandidateQueries` (core-session.md §7.2, §10; schemes-lang.md §4.5): local mode dispatch and the mixed English / emoji / kaomoji insertion into pinyin lists.

use std::borrow::Cow;

use crate::assets;
use crate::dictionary::english::EnglishDictionary;
use crate::local::command::{
    command_title, query_command, translation_source, usable_command_table,
};
use crate::local::date_time::{query_date_time, LocalDateTime};
use crate::local::emoji::{
    query_emoji, query_kaomoji, query_mixed_emoji, query_mixed_kaomoji, ExpressiveRow,
    MIXED_FETCH_LIMIT, MIXED_RESULT_LIMIT, MODE_RESULT_LIMIT,
};
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
const MIXED_DEDUP_CAPACITY: usize = MIXED_ENGLISH_LIMIT + MIXED_FETCH_LIMIT * 2;
/// 混入 emoji 和颜文字所需的最短输入：它们按拼音查，一个字母能匹配的太多。九宫格按数字个数算，同样是 2。
pub(crate) const MIXED_EXPRESSIVE_MINIMUM_INPUT: usize = 2;
/// 短候选列表直接线性找锚点，避免为索引表分配堆内存。
const EXPRESSIVE_ANCHOR_LINEAR_LIMIT: usize = 64;

pub(crate) fn lowercase_prefix(raw: &str) -> Cow<'_, str> {
    if raw.bytes().all(|byte| byte.is_ascii_lowercase()) {
        Cow::Borrowed(raw)
    } else {
        Cow::Owned(raw.to_ascii_lowercase())
    }
}

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
    pub fn mention_annotation(&self, text: &str, key: &str) -> &'static str {
        if !self.mention_places {
            return "";
        }
        mention_annotation(text, key, &self.mentions)
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
            .query_prefix(&lowercase_prefix(raw), MODE_ENGLISH_LIMIT);
        let mut candidates = Vec::with_capacity(completions.len().saturating_add(1));
        candidates.push(WordItem::new("", raw, 0, CandidateSource::Generated, ""));
        candidates.extend(
            completions
                .into_iter()
                .filter(|candidate| !candidate.word.eq_ignore_ascii_case(raw)),
        );
        candidates
    }

    /// 英文、emoji、颜文字混入拼音列表，规则见 `insert_mixed_rows`（英文沿用 candidate_queries.cpp:101-195 的优先位置）。不是全拼、双拼，在本地模式或专用英文里，或者什么都没开时原样返回。
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
        let others_db = self.paths.resource(assets::OTHER_DICTIONARY);
        let shuangpin = profile(self.profile);
        let long_enough = prefix.len() >= MIXED_EXPRESSIVE_MINIMUM_INPUT;
        let emoji_rows = if expressive.emoji_candidates && long_enough {
            query_mixed_emoji(prefix, scheme, &others_db, shuangpin)
        } else {
            Vec::new()
        };
        let kaomoji_rows = if expressive.kaomoji_candidates && long_enough {
            query_mixed_kaomoji(prefix, scheme, &others_db, shuangpin)
        } else {
            Vec::new()
        };
        insert_mixed_rows(candidates, english_rows, emoji_rows, kaomoji_rows)
    }
}

/// 每组先按文字与列表、与前面各组去重。英文的第一行放进优先位置（首选之后，有云候选、AI 候选时再往后），其余接在末尾。emoji、颜文字接在它描绘的那个候选词后面，接不上的排在末尾，见 `anchor_expressive_rows`。
fn insert_mixed_rows(
    mut candidates: Vec<WordItem>,
    mut english: Vec<WordItem>,
    mut emoji: Vec<ExpressiveRow>,
    mut kaomoji: Vec<ExpressiveRow>,
) -> Vec<WordItem> {
    if english.is_empty() && emoji.is_empty() && kaomoji.is_empty() {
        return candidates;
    }
    // 借用现有候选词并原地筛掉重复项；释放这些借用后再把候选行移入结果。
    let (english_unique, emoji_unique, kaomoji_unique) = {
        let mut seen = [None; MIXED_DEDUP_CAPACITY];
        let mut seen_length = 0;
        let english_unique = unique_mask(
            english.iter().map(|item| item.word.as_str()),
            &candidates,
            &mut seen,
            &mut seen_length,
        );
        let emoji_unique = unique_mask(
            emoji.iter().map(|row| row.item.word.as_str()),
            &candidates,
            &mut seen,
            &mut seen_length,
        );
        let kaomoji_unique = unique_mask(
            kaomoji.iter().map(|row| row.item.word.as_str()),
            &candidates,
            &mut seen,
            &mut seen_length,
        );
        (english_unique, emoji_unique, kaomoji_unique)
    };
    retain_masked_rows(&mut english, english_unique);
    retain_masked_rows(&mut emoji, emoji_unique);
    retain_masked_rows(&mut kaomoji, kaomoji_unique);
    candidates.reserve(english.len() + emoji.len() + kaomoji.len());

    let mut english = english.into_iter();
    if let Some(first) = english.next() {
        let slot = priority_slot(&candidates);
        candidates.insert(slot, first);
        candidates.extend(english);
    }
    anchor_expressive_rows(&mut candidates, emoji, kaomoji);
    candidates
}

/// 九宫格的混排：英文行已经按九宫格自己的规则插进了列表，emoji 和颜文字按与 26 键相同的规则去重、定位（`insert_mixed_rows`）。
pub(crate) fn insert_expressive_rows(
    candidates: Vec<WordItem>,
    emoji: Vec<ExpressiveRow>,
    kaomoji: Vec<ExpressiveRow>,
) -> Vec<WordItem> {
    insert_mixed_rows(candidates, Vec::new(), emoji, kaomoji)
}

/// 首选之后；有云候选时在它之后，有 AI 候选时再往后一位。
fn priority_slot(candidates: &[WordItem]) -> usize {
    let (has_ai, has_cloud) = candidates.iter().fold((false, false), |(ai, cloud), item| {
        (
            ai || item.source == CandidateSource::AiSuggestion,
            cloud || item.source == CandidateSource::CloudSuggestion,
        )
    });
    let slot = if has_ai {
        3
    } else if has_cloud {
        2
    } else {
        1
    };
    slot.min(candidates.len())
}

/// 一行 emoji 或颜文字的某个关键词正好是某个候选词时，紧接在第一个这样的候选后面；同一个词后面的几行 emoji 在颜文字前，各自按查询的先后。它总跟在一个词后面，所以不会占首选。接不上任何候选的行每组只留前 `MIXED_RESULT_LIMIT` 行，emoji 在前，排在列表末尾。只对插入前的行找词，混入的行不互相接。容量由调用方预留；有关键词时为候选词的有序索引分配一次。
fn anchor_expressive_rows(
    list: &mut Vec<WordItem>,
    mut emoji: Vec<ExpressiveRow>,
    mut kaomoji: Vec<ExpressiveRow>,
) {
    // （接在第几行后面，组，组内第几行），按字典序排好就是插入的先后。
    let mut anchored = [(0_usize, 0_usize, 0_usize); MIXED_FETCH_LIMIT * 2];
    let mut anchored_length = 0;
    let mut placed = [0_u64; 2];
    let any_keywords = emoji
        .iter()
        .chain(kaomoji.iter())
        .any(|row| !row.keywords.is_empty());
    if any_keywords {
        let mut add_anchors = |first_position: &dyn Fn(&str) -> Option<usize>| {
            for (group, rows) in [&emoji, &kaomoji].into_iter().enumerate() {
                debug_assert!(rows.len() <= MIXED_FETCH_LIMIT);
                for (index, row) in rows.iter().enumerate() {
                    let anchor = row
                        .keywords
                        .split_whitespace()
                        .filter_map(first_position)
                        .min();
                    if let Some(anchor) = anchor {
                        anchored[anchored_length] = (anchor, group, index);
                        anchored_length += 1;
                        placed[group] |= 1 << index;
                    }
                }
            }
        };
        if list.len() <= EXPRESSIVE_ANCHOR_LINEAR_LIMIT {
            let first_position =
                |keyword: &str| list.iter().position(|candidate| candidate.word == keyword);
            add_anchors(&first_position);
        } else {
            // 26 键的列表常有几百行（`ji` 七百多行），每行 emoji 的每个关键词都从头扫一遍列表太慢：先把候选词连同位置排好序，关键词二分查找；同一个词出现几次时取最前面的位置。
            let mut words: Vec<(&str, usize)> = list
                .iter()
                .enumerate()
                .map(|(at, candidate)| (candidate.word.as_str(), at))
                .collect();
            words.sort_unstable();
            let first_position = |keyword: &str| {
                let at = words.partition_point(|(word, _)| *word < keyword);
                words
                    .get(at)
                    .filter(|(word, _)| *word == keyword)
                    .map(|(_, position)| *position)
            };
            add_anchors(&first_position);
        }
    }
    let anchored = &mut anchored[..anchored_length];
    anchored.sort_unstable();
    // 从后往前插：后面的插入不移动前面的锚点；同一个锚点倒着插，插完正好是要的先后。
    for &(anchor, group, index) in anchored.iter().rev() {
        let rows = if group == 0 { &mut emoji } else { &mut kaomoji };
        list.insert(anchor + 1, std::mem::take(&mut rows[index].item));
    }
    for (group, rows) in [emoji, kaomoji].into_iter().enumerate() {
        list.extend(
            rows.into_iter()
                .enumerate()
                .filter(|(index, _)| placed[group] & (1 << index) == 0)
                .take(MIXED_RESULT_LIMIT)
                .map(|(_, row)| row.item),
        );
    }
}

/// 用位掩码原地移除重复行，保留首次出现的顺序和原列表容量。
fn retain_masked_rows<T>(rows: &mut Vec<T>, mut mask: u64) {
    let mut index = 0;
    rows.retain(|_| {
        let keep = mask & 1 != 0;
        mask >>= 1;
        index += 1;
        debug_assert!(index <= u64::BITS as usize);
        keep
    });
}

/// 用位掩码记录每组候选的首次出现，避免为受协议限制的短列表分配布尔数组。
fn unique_mask<'a>(
    words: impl Iterator<Item = &'a str>,
    existing: &[WordItem],
    seen: &mut [Option<&'a str>; MIXED_DEDUP_CAPACITY],
    seen_length: &mut usize,
) -> u64 {
    words.enumerate().fold(0, |mask, (index, word)| {
        debug_assert!(index < u64::BITS as usize);
        let duplicate = existing
            .iter()
            .any(|candidate| candidate.word.as_str() == word)
            || seen[..*seen_length]
                .iter()
                .flatten()
                .any(|seen| *seen == word);
        if duplicate {
            return mask;
        }
        seen[*seen_length] = Some(word);
        *seen_length += 1;
        mask | (1_u64 << index)
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;

    #[test]
    fn lowercase_prefix_is_borrowed_without_allocating() {
        let (prefix, allocations) =
            crate::ime::personal_rerank::allocations::count(|| lowercase_prefix("hello"));
        assert_eq!(prefix, "hello");
        assert_eq!(allocations, 0);
    }

    fn row(word: &str, source: CandidateSource) -> WordItem {
        WordItem::new("ni", word, 1, source, "")
    }

    fn words(list: &[WordItem]) -> Vec<&str> {
        list.iter().map(|item| item.word.as_str()).collect()
    }

    /// 一行 emoji 或颜文字，`keywords` 是它在目录里的关键词。
    fn depicting(word: &str, source: CandidateSource, keywords: &str) -> ExpressiveRow {
        ExpressiveRow {
            item: row(word, source),
            keywords: keywords.to_owned(),
        }
    }

    /// 接不上任何候选词的一行。
    fn plain(word: &str, source: CandidateSource) -> ExpressiveRow {
        depicting(word, source, "")
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
    fn extra_rows_are_deduplicated_in_place_in_first_seen_order() {
        let mut rows = vec![
            row("Ni", CandidateSource::EnglishDictionary),
            row("倪", CandidateSource::EnglishDictionary),
            row("Ni", CandidateSource::EnglishDictionary),
        ];
        let mut seen = [None; MIXED_DEDUP_CAPACITY];
        let mut seen_length = 0;

        let mask = unique_mask(
            rows.iter().map(|item| item.word.as_str()),
            &[],
            &mut seen,
            &mut seen_length,
        );
        retain_masked_rows(&mut rows, mask);

        assert_eq!(words(&rows), vec!["Ni", "倪"]);
        assert_eq!(rows.capacity(), 3);
    }

    #[test]
    fn unanchored_emoji_and_kaomoji_go_to_the_end() {
        // test_mixed_expressive_input_session.cpp:159-176 的输入。参考实现把 emoji、颜文字的第一行放进优先位置；现在接不上候选词的行排在末尾，英文仍占优先位置（#5667、#5907）。
        let english = vec![
            row("Ni", CandidateSource::EnglishDictionary),
            row("Ninja", CandidateSource::EnglishDictionary),
        ];
        let emoji = vec![
            plain("😀", CandidateSource::Emoji),
            plain("😁", CandidateSource::Emoji),
        ];
        let kaomoji = vec![
            plain("(^_^)", CandidateSource::Kaomoji),
            plain("(T_T)", CandidateSource::Kaomoji),
        ];
        let list = insert_mixed_rows(chinese(), english, emoji, kaomoji);
        assert_eq!(
            words(&list),
            vec!["你", "Ni", "倪", "Ninja", "😀", "😁", "(^_^)", "(T_T)"]
        );
        // 接不上的行每组只留前三行。
        let many = (0..5)
            .map(|index| plain(&format!("表情{index}"), CandidateSource::Emoji))
            .collect();
        let list = insert_mixed_rows(chinese(), Vec::new(), many, Vec::new());
        assert_eq!(words(&list), vec!["你", "倪", "表情0", "表情1", "表情2"]);
    }

    #[test]
    fn expressive_rows_follow_the_word_they_depict() {
        // #5667 的截图：54（li、ji）是 里 李 鸡 🐔 几。
        let list = vec![
            row("里", CandidateSource::Database),
            row("李", CandidateSource::Database),
            row("鸡", CandidateSource::Database),
            row("几", CandidateSource::Database),
        ];
        let emoji = vec![
            depicting("🐔", CandidateSource::Emoji, "鸡 鸡头 chicken"),
            plain("🎁", CandidateSource::Emoji),
        ];
        let merged = insert_mixed_rows(list, Vec::new(), emoji, Vec::new());
        assert_eq!(words(&merged), vec!["里", "李", "鸡", "🐔", "几", "🎁"]);

        // 🇺🇲 只是编码以 meiguo 开头，接不上 美国，排到末尾；🇺🇸 紧跟 美国（#5907）。
        let list = vec![
            row("美国", CandidateSource::Database),
            row("没过", CandidateSource::Database),
        ];
        let emoji = vec![
            depicting("🇺🇲", CandidateSource::Emoji, "美国本土外小岛屿 flag:"),
            depicting("🇺🇸", CandidateSource::Emoji, "美国 美利坚 星条旗"),
        ];
        let merged = insert_mixed_rows(list, Vec::new(), emoji, Vec::new());
        assert_eq!(words(&merged), vec!["美国", "🇺🇸", "没过", "🇺🇲"]);
        // 关键词要整个相等，「美」不算。
        let merged = insert_mixed_rows(
            vec![
                row("美", CandidateSource::Database),
                row("", CandidateSource::Database),
            ],
            Vec::new(),
            vec![depicting("🇺🇸", CandidateSource::Emoji, "美国 星条旗")],
            Vec::new(),
        );
        assert_eq!(words(&merged), vec!["美", "", "🇺🇸"]);
    }

    #[test]
    fn short_expressive_anchor_scan_does_not_allocate_word_index() {
        let mut candidates = Vec::with_capacity(3);
        candidates.extend(chinese());
        let emoji = vec![depicting("🐔", CandidateSource::Emoji, "倪 chicken")];

        let (list, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            insert_mixed_rows(candidates, Vec::new(), emoji, Vec::new())
        });

        assert_eq!(words(&list), vec!["你", "倪", "🐔"]);
        assert_eq!(
            allocations, 0,
            "短表情锚定不应为候选词索引分配临时缓冲：{allocations}"
        );
    }

    #[test]
    fn several_rows_on_one_word_keep_emoji_before_kaomoji_in_query_order() {
        let list = vec![
            row("开心", CandidateSource::Database),
            row("凯", CandidateSource::Database),
            row("害羞", CandidateSource::Database),
        ];
        let emoji = vec![
            depicting("😄", CandidateSource::Emoji, "开心 高兴"),
            depicting("😊", CandidateSource::Emoji, "害羞"),
            depicting("😆", CandidateSource::Emoji, "高兴 开心"),
        ];
        let kaomoji = vec![
            depicting("(^_^)", CandidateSource::Kaomoji, "kai xin 开心"),
            depicting("(*/ω＼*)", CandidateSource::Kaomoji, "hai xiu 害羞"),
            plain("(T_T)", CandidateSource::Kaomoji),
        ];
        let merged = insert_mixed_rows(list, Vec::new(), emoji, kaomoji);
        assert_eq!(
            words(&merged),
            vec![
                "开心",
                "😄",
                "😆",
                "(^_^)",
                "凯",
                "害羞",
                "😊",
                "(*/ω＼*)",
                "(T_T)"
            ]
        );
        // 词在列表里出现两次时接在第一次后面；混入的行不互相接。
        let list = vec![
            row("开心", CandidateSource::Database),
            row("开心", CandidateSource::CloudSuggestion),
        ];
        let merged = insert_mixed_rows(
            list,
            Vec::new(),
            vec![depicting("😄", CandidateSource::Emoji, "开心")],
            vec![depicting("(^_^)", CandidateSource::Kaomoji, "😄")],
        );
        assert_eq!(words(&merged), vec!["开心", "😄", "开心", "(^_^)"]);
    }

    #[test]
    fn anchored_rows_follow_their_word_behind_cloud_ai_and_english_rows() {
        // 首选、云候选、AI 候选之后是英文首行；emoji 不看这些位置，只跟着它描绘的词，哪怕词就是云候选或英文词。
        let list = vec![
            row("你", CandidateSource::Database),
            row("美国", CandidateSource::CloudSuggestion),
            row("智", CandidateSource::AiSuggestion),
            row("倪", CandidateSource::Database),
        ];
        let merged = insert_mixed_rows(
            list,
            vec![
                row("Ni", CandidateSource::EnglishDictionary),
                row("Ninja", CandidateSource::EnglishDictionary),
            ],
            vec![
                depicting("🇺🇸", CandidateSource::Emoji, "美国"),
                depicting("🥷", CandidateSource::Emoji, "忍者 Ninja"),
                plain("😀", CandidateSource::Emoji),
            ],
            vec![depicting("(^_^)", CandidateSource::Kaomoji, "你")],
        );
        assert_eq!(
            words(&merged),
            vec!["你", "(^_^)", "美国", "🇺🇸", "智", "Ni", "倪", "Ninja", "🥷", "😀"]
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
            vec![plain("😀", CandidateSource::Emoji)],
            Vec::new(),
        );
        assert_eq!(words(&list), vec!["Ni", "Ninja", "😀"]);
    }

    #[test]
    fn nine_key_expressive_rows_use_the_same_rules() {
        let emoji = || {
            vec![
                depicting("🐔", CandidateSource::Emoji, "鸡"),
                plain("😁", CandidateSource::Emoji),
            ]
        };
        let kaomoji = || vec![plain("(^_^)", CandidateSource::Kaomoji)];
        let list = || {
            vec![
                row("里", CandidateSource::Database),
                row("鸡", CandidateSource::Database),
            ]
        };
        assert_eq!(
            insert_expressive_rows(list(), emoji(), kaomoji()),
            insert_mixed_rows(list(), Vec::new(), emoji(), kaomoji())
        );
        assert_eq!(
            words(&insert_expressive_rows(list(), emoji(), kaomoji())),
            vec!["里", "鸡", "🐔", "😁", "(^_^)"]
        );
        // 与列表里已有的行重复的不再出现。
        let duplicate = insert_expressive_rows(
            chinese(),
            vec![depicting("倪", CandidateSource::Emoji, "你")],
            Vec::new(),
        );
        assert_eq!(words(&duplicate), vec!["你", "倪"]);
    }

    #[test]
    fn mixed_rows_reserve_the_extra_candidate_capacity() {
        let list = insert_mixed_rows(
            vec![row("你", CandidateSource::Database)],
            (0..5)
                .map(|index| row(&format!("en{index}"), CandidateSource::EnglishDictionary))
                .collect(),
            (0..3)
                .map(|index| plain(&format!("😀{index}"), CandidateSource::Emoji))
                .collect(),
            (0..3)
                .map(|index| depicting(&format!("ka{index}"), CandidateSource::Kaomoji, "你"))
                .collect(),
        );

        assert_eq!(list.len(), 12);
        assert_eq!(list.capacity(), 12);
    }

    #[test]
    fn mixed_merge_keeps_bounded_dedup_state_off_the_heap() {
        let mut candidates = Vec::with_capacity(128 + 5 + MIXED_FETCH_LIMIT * 2);
        for index in 0..128 {
            candidates.push(row(&format!("候选{index}"), CandidateSource::Database));
        }
        let english = (0..5)
            .map(|index| row(&format!("英文{index}"), CandidateSource::EnglishDictionary))
            .collect();
        // 每组取满 `MIXED_FETCH_LIMIT` 行，一半接得上候选词。
        let emoji = (0..MIXED_FETCH_LIMIT)
            .map(|index| {
                depicting(
                    &format!("表情{index}"),
                    CandidateSource::Emoji,
                    &format!("候选{}", index * 2),
                )
            })
            .collect();
        let kaomoji = (0..MIXED_FETCH_LIMIT)
            .map(|index| {
                depicting(
                    &format!("颜文字{index}"),
                    CandidateSource::Kaomoji,
                    &format!("候选{}", index * 2 + 1),
                )
            })
            .collect();

        let (merged, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            insert_mixed_rows(candidates, english, emoji, kaomoji)
        });

        assert_eq!(merged.len(), 128 + 5 + MIXED_FETCH_LIMIT * 2);
        assert!(
            allocations <= 1,
            "mixed merge allocated {allocations} times"
        );
    }

    #[test]
    fn the_extra_lists_are_deduplicated_against_each_other() {
        let list = insert_mixed_rows(
            chinese(),
            vec![row("Ni", CandidateSource::EnglishDictionary)],
            vec![
                plain("Ni", CandidateSource::Emoji),
                plain("😀", CandidateSource::Emoji),
                plain("😀", CandidateSource::Emoji),
            ],
            vec![plain("😀", CandidateSource::Kaomoji)],
        );
        assert_eq!(words(&list), vec!["你", "Ni", "倪", "😀"]);
        assert_eq!(list[3].source, CandidateSource::Emoji);
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
