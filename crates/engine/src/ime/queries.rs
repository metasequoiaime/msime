//! `CandidateQueries` (core-session.md §7.2, §10; schemes-lang.md §4.5): local mode dispatch and the mixed English / emoji / kaomoji insertion into pinyin lists.

use std::borrow::Cow;

use crate::assets;
use crate::dictionary::english::EnglishDictionary;
use crate::local::chaizi::query_chaizi;
use crate::local::command::{
    command_title, plugin_command_source, query_command, translation_source, usable_command_table,
    usable_plugin_commands,
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
    merge_quick_phrases, query_exact_quick_phrases, query_quick_phrases, usable_quick_phrase_table,
};
use crate::local::symbol_candidates::{query_mixed_symbols, SymbolRow, MIXED_SYMBOL_LIMIT};
use crate::local::unicode::query_unicode;
use crate::local::LocalQueryResult;
use crate::paths::RuntimePaths;
use crate::shuangpin::ShuangpinProfile;
use crate::types::{
    CandidateSource, CommandTableEntry, EnglishInputOptions, LocalInputMode, MentionEntry,
    MixedExpressiveOptions, PluginCommand, QuickPhraseEntry, SchemeType, WordItem,
};

pub const MIXED_ENGLISH_LIMIT: usize = 5;
pub const MODE_ENGLISH_LIMIT: usize = 1_000;
const MIXED_DEDUP_CAPACITY: usize =
    MIXED_ENGLISH_LIMIT + MIXED_SYMBOL_LIMIT + MIXED_FETCH_LIMIT * 2;
/// 输入含糊（单音节名称派、度，两个字母的首字母 `ax`、`jf`）时符号最早排在第几位（下标）：这样的输入能指的词太多，排到第 4 位，前三个词不被挤动。
const SYMBOL_AMBIGUOUS_SLOT: usize = 3;
/// 名称那个词不在候选里时，符号排在第几位（下标）：默认每页 6 个，下标 5 是第一页最后一位，看得见又不挤掉前面的词。
const SYMBOL_UNANCHORED_SLOT: usize = 5;
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
    profile: &'static ShuangpinProfile,
    english: Option<EnglishDictionary>,
    /// The host's command table, usable rows only.
    command_table: Vec<CommandTableEntry>,
    /// 宿主交来的插件指令，原样保留：指令表一换就要按新表重新过滤。
    requested_plugin_commands: Vec<PluginCommand>,
    /// 生效的插件指令，经过 `usable_plugin_commands`。
    plugin_commands: Vec<PluginCommand>,
    /// 宿主的短语表（插件），只保留能用的行，按编码排序。
    quick_phrase_table: Vec<QuickPhraseEntry>,
    /// The host's mention list, usable entries only.
    mentions: Vec<MentionEntry>,
    /// Whether `@` mode offers the embedded places after the list.
    mention_places: bool,
    /// 宿主开着繁体输出：拆字行按转换后的字形去重。
    chaizi_traditional_output: bool,
}

impl CandidateQueries {
    pub fn new(paths: &RuntimePaths, profile: &'static ShuangpinProfile) -> Self {
        Self {
            paths: paths.clone(),
            profile,
            english: None,
            command_table: Vec::new(),
            requested_plugin_commands: Vec::new(),
            plugin_commands: Vec::new(),
            quick_phrase_table: Vec::new(),
            mentions: Vec::new(),
            mention_places: false,
            chaizi_traditional_output: false,
        }
    }

    /// Keeps the rows `/` mode can use and drops the rest.
    pub fn set_command_table(&mut self, table: &[CommandTableEntry]) {
        self.command_table = usable_command_table(table);
        // 用户的指令表永远赢：表变了，插件指令按新表重新过滤。
        self.plugin_commands =
            usable_plugin_commands(&self.command_table, &self.requested_plugin_commands);
    }

    /// 换上宿主算好的插件指令，只留下引擎能用的（见 `usable_plugin_commands`）。
    pub fn set_plugin_commands(&mut self, commands: &[PluginCommand]) {
        self.requested_plugin_commands = commands.to_vec();
        self.plugin_commands = usable_plugin_commands(&self.command_table, commands);
    }

    /// 生效的插件指令里 `trigger` 的标题；`trigger` 不属于任何生效的插件指令时为 `None`。宿主交来的表里触发词重复、或者标题不合规时，标题以过滤之后留下的那一条为准。
    pub fn plugin_command_title(&self, trigger: &str) -> Option<&str> {
        self.plugin_commands
            .iter()
            .find(|command| command.trigger == trigger)
            .map(|command| command.title.as_str())
    }

    /// `/` 之后的输入属于哪条生效的插件指令：（触发词，参数）。
    pub fn plugin_command_source<'a>(&self, code: &'a str) -> Option<(&str, &'a str)> {
        plugin_command_source(code, &self.plugin_commands)
    }

    /// 保留 K 模式能用的宿主短语行，丢弃其余。
    pub fn set_quick_phrase_table(&mut self, table: &[QuickPhraseEntry]) {
        self.quick_phrase_table = usable_quick_phrase_table(table);
    }

    /// 普通组字的编码恰好是某条快捷短语的编码时，把短语并进候选：`slot` 为 `None` 时追加在末尾（`quick_phrase_frequency` 关着或还没有学习记录的默认落位）；`Some(n)` 时插在第 n 个计位候选之后（`quick_phrase_slots` 学下来的位置）。只认整个编码，不按前缀匹配（见 [`query_exact_quick_phrases`]）。
    pub fn insert_quick_phrase_candidates(
        &self,
        code: &str,
        candidates: &mut Vec<WordItem>,
        slot: Option<usize>,
    ) {
        if code.is_empty() || !code.bytes().all(|byte| byte.is_ascii_lowercase()) {
            return;
        }
        let merged = query_exact_quick_phrases(
            code,
            &self.paths.dictionary(assets::MAIN_DICTIONARY),
            &self.quick_phrase_table,
        );
        let phrases: Vec<WordItem> = merged
            .candidates
            .into_iter()
            .filter(|candidate| {
                !candidates
                    .iter()
                    .any(|existing| existing.word == candidate.word)
            })
            .collect();
        let Some(slot) = slot else {
            candidates.extend(phrases);
            return;
        };
        // slot 是「组前面有几个计位候选」：0 放最前，n 放第 n 个之后，超出放回末尾。
        let index = if slot == 0 {
            0
        } else {
            candidates
                .iter()
                .enumerate()
                .filter(|(_, item)| counts_toward_quick_phrase_slot(item))
                .nth(slot - 1)
                .map(|(index, _)| index + 1)
                .unwrap_or(candidates.len())
        };
        candidates.splice(index..index, phrases);
    }

    /// Keeps the entries `@` mode can use and drops the rest.
    pub fn set_mentions(&mut self, entries: &[MentionEntry]) {
        self.mentions = usable_mentions(entries);
    }

    /// 联系人按姓名、姓名加手机号成对置顶；完整匹配先于前缀，同名词典行替换为联系人来源。
    pub fn append_contact_candidates(&self, prefix: &str, candidates: &mut Vec<WordItem>) {
        if self.mentions.is_empty()
            || prefix.is_empty()
            || !prefix
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'\'')
        {
            return;
        }
        let prefix = prefix.replace('\'', "");
        if prefix.is_empty() {
            return;
        }
        let mut contacts = Vec::new();
        // 每个联系人占两行，完整匹配优先，不让较早导入的长姓名抢占精确姓名。
        for exact in [true, false] {
            for entry in &self.mentions {
                if entry.phone.is_empty() || entry.key.is_empty() {
                    continue;
                }
                let compact = entry.key.replace('\'', "");
                let initials: String = entry
                    .key
                    .split('\'')
                    .filter_map(|syllable| syllable.chars().next())
                    .collect();
                let matches = compact.starts_with(&prefix) || initials.starts_with(&prefix);
                let is_exact = compact == prefix || initials == prefix;
                if !matches || is_exact != exact {
                    continue;
                }
                for text in [
                    entry.text.clone(),
                    format!("{} {}", entry.text, entry.phone),
                ] {
                    if contacts.iter().any(|item: &WordItem| item.word == text) {
                        continue;
                    }
                    contacts.push(WordItem::new(
                        entry.key.clone(),
                        text,
                        0,
                        CandidateSource::Contact,
                        entry.key.clone(),
                    ));
                }
                if contacts.len() >= 18 {
                    break;
                }
            }
            if contacts.len() >= 18 {
                break;
            }
        }
        candidates.retain(|item| !contacts.iter().any(|contact| contact.word == item.word));
        candidates.splice(0..0, contacts);
    }

    /// Turns the embedded places of `@` mode on or off.
    pub fn set_mention_places(&mut self, enabled: bool) {
        self.mention_places = enabled;
    }

    /// 宿主是否把候选转成繁体输出；开着时拆字行按繁体字形去重，否则简繁两种字形都列出。
    pub fn set_chaizi_traditional_output(&mut self, enabled: bool) {
        self.chaizi_traditional_output = enabled;
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

    /// The title of a `/` mode row, by the trigger its `pinyin` holds. 插件指令的标题不在这里查：它由 runtime 画在插件注释格里，不进引擎的注释。
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
        let shuangpin = self.profile;
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
            // 输入属于某条插件指令时，引擎不列任何行：插件的行由宿主问过插件之后写进会话的缓存，在那之前由兜底行顶着。
            LocalInputMode::Command if self.plugin_command_source(code).is_some() => {
                LocalQueryResult::default()
            }
            LocalInputMode::Command => rows(query_command(code, now, &self.command_table)),
            LocalInputMode::Mention => {
                rows(query_mentions(code, &self.mentions, self.mention_places))
            }
            // 网址模式不查任何候选，只留显示整段预编辑的兜底行。
            LocalInputMode::Url => LocalQueryResult::default(),
            LocalInputMode::Chaizi => rows(query_chaizi(
                code,
                scheme,
                shuangpin,
                self.chaizi_traditional_output,
            )),
            // 符号模式查两张编进引擎的表（分组码与符号名），不发诊断：表总在，输入只是没命中。
            LocalInputMode::Symbol => rows(crate::local::symbol_groups::query_symbol_mode(
                code,
                crate::local::symbol_groups::SYMBOL_MODE_LIMIT,
            )),
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

    /// 英文、emoji、颜文字、符号和日文混输行混入拼音列表，规则见 `insert_mixed_rows_with_japanese`。英文第一行默认进 candidate_queries.cpp:101-195 的优先位置（首选之后）；只有达到 `minimum_prefix`、`english_may_lead` 为真、输入拆不成完整音节（全拼 `is_complete_pinyin_input`，双拼 `is_complete_input`），且去重后剩下的第一行英文正好就是整段输入时，它才占首位。这条规则从不比较英文和拼音的权重，两边不在一个尺度上（#1040、#1063）。`japanese` 是调用方已经判定要混入的那一行日文（见 `InputSession::mixed_japanese_row`），这里只决定它的位置。符号行（打拼音出符号）另要 `symbols_allowed`：调用方在光标前缀解码和辅助码激活时传假，这两种情况下整段输入不是用户打的名称。`first_page_rows` 是宿主的每页候选数，英文组在首页最多占到这一行（`EnglishInputOptions::first_page_count`）。`mixed_enabled` 是候选混输总开关（`SessionOptions::mixed_candidates`），为假时各分成员不查，列表原样返回。不是全拼、双拼，在本地模式或专用英文里，或者什么都没开时原样返回。
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
        english_may_lead: bool,
        symbols_allowed: bool,
        japanese: Option<MixedJapaneseRow>,
        first_page_rows: usize,
        mixed_enabled: bool,
    ) -> Vec<WordItem> {
        let anything_enabled = mixed_enabled
            && (english.mixed_candidates
                || expressive.emoji_candidates
                || expressive.kaomoji_candidates
                || (expressive.symbol_candidates && symbols_allowed)
                || japanese.is_some());
        if !anything_enabled
            || dedicated_english
            || local_mode != LocalInputMode::None
            || !scheme.allows_english_emoji_mixing()
            || prefix.is_empty()
        {
            return candidates;
        }

        let english_rows =
            if english.mixed_candidates && prefix.bytes().all(|byte| byte.is_ascii_lowercase()) {
                if prefix.len() >= english.minimum_prefix {
                    self.english_dictionary()
                        .query_prefix(prefix, MIXED_ENGLISH_LIMIT)
                } else if prefix.len() >= 2 {
                    self.english_dictionary()
                        .query_exact(prefix)
                        .into_iter()
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };
        let shuangpin = self.profile;
        let long_enough = prefix.len() >= MIXED_EXPRESSIVE_MINIMUM_INPUT;
        let (emoji_rows, kaomoji_rows) =
            if long_enough && (expressive.emoji_candidates || expressive.kaomoji_candidates) {
                let others_db = self.paths.resource(assets::OTHER_DICTIONARY);
                let emoji_rows = if expressive.emoji_candidates {
                    query_mixed_emoji(prefix, scheme, &others_db, shuangpin)
                } else {
                    Vec::new()
                };
                let kaomoji_rows = if expressive.kaomoji_candidates {
                    query_mixed_kaomoji(prefix, scheme, &others_db, shuangpin)
                } else {
                    Vec::new()
                };
                (emoji_rows, kaomoji_rows)
            } else {
                (Vec::new(), Vec::new())
            };
        let symbol_rows = if expressive.symbol_candidates && symbols_allowed && long_enough {
            query_mixed_symbols(prefix, scheme, shuangpin)
        } else {
            Vec::new()
        };
        // 只有查到了英文才算音节完整性：双拼的判定每段要分配一次。
        let english_lead = (english_may_lead
            && prefix.len() >= english.minimum_prefix
            && !english_rows.is_empty()
            && !spells_whole_syllables(prefix, scheme, self.profile))
        .then_some(prefix);
        insert_mixed_rows_with_japanese(
            candidates,
            english_rows,
            emoji_rows,
            kaomoji_rows,
            symbol_rows,
            english_lead,
            japanese,
            english.first_page_count,
            first_page_rows,
        )
    }
}

/// 槽位计数的普通候选（`counts_toward_quick_phrase_slot`，quick_phrase_query.cpp:123-143）：固定位置的行、快捷短语自己和混输/在线/插件/联系人行都不算。
pub(crate) fn counts_toward_quick_phrase_slot(item: &WordItem) -> bool {
    if item.fixed_position > 0 {
        return false;
    }
    !matches!(
        item.source,
        CandidateSource::QuickPhrase
            | CandidateSource::CloudSuggestion
            | CandidateSource::CloudPending
            | CandidateSource::AiSuggestion
            | CandidateSource::AiPending
            | CandidateSource::EnglishDictionary
            | CandidateSource::Emoji
            | CandidateSource::Kaomoji
            | CandidateSource::MixedJapanese
            | CandidateSource::Plugin
            | CandidateSource::Contact
    )
}

/// 输入能不能整段切成完整音节。全拼看严格的完整音节（不认纠错和别名），双拼看是不是每两键一个音节；其余方案不让英文、日文领先，按完整算。
pub(crate) fn spells_whole_syllables(
    input: &str,
    scheme: SchemeType,
    profile: &ShuangpinProfile,
) -> bool {
    match scheme {
        SchemeType::Quanpin => crate::pinyin::segment::is_complete_pinyin_input(input),
        SchemeType::Shuangpin => crate::shuangpin::query::is_complete_input(input, profile),
        _ => true,
    }
}

/// 日文混输要混进列表的那一行（来源 `MixedJapanese`），以及它是否领先。
#[derive(Debug, Clone, PartialEq)]
pub struct MixedJapaneseRow {
    pub item: WordItem,
    /// 占首位，否则进优先位置；规则见 `InputSession::mixed_japanese_row`。
    pub lead: bool,
}

/// 每组先按文字与列表、与前面各组去重。英文的第一行放进优先位置（首选之后，有云候选、AI 候选时再往后），其余接在末尾；`english_lead` 是调用方判定英文可以领先的那段输入（拼音拆不开的整段），去重后剩下的第一行英文正好拼出它时占首位，有云候选、AI 候选时也一样，其余英文照旧接在末尾。完全匹配的那行因为和候选重复被去掉时不领先，不拿补全词顶上。emoji、颜文字接在它描绘的那个候选词后面，接不上的排在末尾，见 `anchor_expressive_rows`。
#[cfg(test)]
fn insert_mixed_rows(
    candidates: Vec<WordItem>,
    english: Vec<WordItem>,
    emoji: Vec<ExpressiveRow>,
    kaomoji: Vec<ExpressiveRow>,
    english_lead: Option<&str>,
) -> Vec<WordItem> {
    insert_mixed_rows_with_japanese(
        candidates,
        english,
        emoji,
        kaomoji,
        Vec::new(),
        english_lead,
        None,
        1,
        0,
    )
}

/// `insert_mixed_rows` 再加上日文混输的那一行和符号行。日文行在英文之后、emoji 和颜文字之前放进列表，文字和已有候选（含刚放进去的英文）重复时不加；领先时占首位，英文已经按 `english_lead` 占了首位时排在它后面（下标 1）；不领先时进和英文相同的优先位置（`priority_slot`），排在英文那一行前面。`first_page_count` 是首页最多展示的英文候选数（`EnglishInputOptions::first_page_count`，1–3）：第一行英文进位之后，其余英文紧跟它继续占首页的空位，`first_page_rows`（宿主每页候选数）放不下的照旧接在列表末尾，0 表示不展开。符号行在英文之后、emoji 之前去重，所以和它文字相同的 emoji 行被去掉；比较时 emoji 去掉 U+FE0F，留下不带变体选择符的符号。符号行最后放，见 `place_symbol_rows`。
#[allow(clippy::too_many_arguments)]
fn insert_mixed_rows_with_japanese(
    mut candidates: Vec<WordItem>,
    mut english: Vec<WordItem>,
    mut emoji: Vec<ExpressiveRow>,
    mut kaomoji: Vec<ExpressiveRow>,
    mut symbols: Vec<SymbolRow>,
    english_lead: Option<&str>,
    japanese: Option<MixedJapaneseRow>,
    first_page_count: usize,
    first_page_rows: usize,
) -> Vec<WordItem> {
    if english.is_empty()
        && emoji.is_empty()
        && kaomoji.is_empty()
        && symbols.is_empty()
        && japanese.is_none()
    {
        return candidates;
    }
    // 借用现有候选词并原地筛掉重复项；释放这些借用后再把候选行移入结果。
    let (english_unique, symbol_unique, emoji_unique, kaomoji_unique) = {
        let mut seen = [None; MIXED_DEDUP_CAPACITY];
        let mut seen_length = 0;
        let english_unique = unique_mask(
            english.iter().map(|item| item.word.as_str()),
            &candidates,
            &mut seen,
            &mut seen_length,
        );
        let symbol_unique = unique_mask(
            symbols.iter().map(|row| row.item.word.as_str()),
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
        (english_unique, symbol_unique, emoji_unique, kaomoji_unique)
    };
    retain_masked_rows(&mut english, english_unique);
    retain_masked_rows(&mut symbols, symbol_unique);
    retain_masked_rows(&mut emoji, emoji_unique);
    retain_masked_rows(&mut kaomoji, kaomoji_unique);
    if !symbols.is_empty() {
        // ✔️ 和符号 ✔ 是同一个字形的两种样式，只留符号行。
        emoji.retain(|row| {
            !symbols
                .iter()
                .any(|symbol| same_without_variation_selector(&row.item.word, &symbol.item.word))
        });
    }
    candidates.reserve(
        english.len()
            + emoji.len()
            + kaomoji.len()
            + symbols.len()
            + usize::from(japanese.is_some()),
    );

    let mut english = english.into_iter();
    let mut english_led = false;
    let mut english_seat = None;
    if let Some(first) = english.next() {
        english_led = english_lead.is_some_and(|input| first.pinyin == input);
        let slot = if english_led {
            0
        } else {
            priority_slot(&candidates)
        };
        candidates.insert(slot, first);
        english_seat = Some(slot);
    }
    if let Some(japanese) =
        japanese.filter(|row| !candidates.iter().any(|item| item.word == row.item.word))
    {
        let slot = if japanese.lead {
            usize::from(english_led)
        } else {
            priority_slot(&candidates)
        };
        if let Some(seat) = &mut english_seat {
            *seat += usize::from(slot <= *seat);
        }
        candidates.insert(slot, japanese.item);
    }
    // 首页英文上限（Windows `cn_en_mixed_input_first_page_count`）：第一行之后的英文紧跟它继续占首页的空位，
    // 首页放不下或只有一行时照旧接在列表末尾。第一行总能留位，所以 `first_page_count` 至少能放一个。
    if let Some(seat) = english_seat {
        let room = first_page_rows.saturating_sub(seat + 1);
        let extra = usize::min(first_page_count.saturating_sub(1), room);
        let extra: Vec<WordItem> = english.by_ref().take(extra).collect();
        candidates.splice(seat + 1..seat + 1, extra);
        candidates.extend(english);
    }
    anchor_expressive_rows(&mut candidates, emoji, kaomoji);
    place_symbol_rows(&mut candidates, symbols);
    candidates
}

/// 九宫格的混排：英文行已经按九宫格自己的规则插进了列表，emoji、颜文字和符号按与 26 键相同的规则去重、定位（`insert_mixed_rows_with_japanese`）。
pub(crate) fn insert_expressive_rows(
    candidates: Vec<WordItem>,
    emoji: Vec<ExpressiveRow>,
    kaomoji: Vec<ExpressiveRow>,
    symbols: Vec<SymbolRow>,
) -> Vec<WordItem> {
    insert_mixed_rows_with_japanese(
        candidates,
        Vec::new(),
        emoji,
        kaomoji,
        symbols,
        None,
        None,
        0,
        0,
    )
}

/// 两段文字去掉 U+FE0F（emoji 样式的变体选择符）后相同。
fn same_without_variation_selector(left: &str, right: &str) -> bool {
    left.chars()
        .filter(|&character| character != '\u{FE0F}')
        .eq(right.chars().filter(|&character| character != '\u{FE0F}'))
}

/// 符号行接在它名称那个词（锚定词）后面，同一个词后面先符号、再 emoji、再颜文字（emoji 已经接好，插在锚定词下标 + 1 就把它们推到后面）：
///
/// - 锚定词在候选里：紧跟在第一个锚定词后面，几个名称给出同一个符号时取其中排得最靠前的；输入含糊（单音节名称、两个字母的首字母）时最早排在 `SYMBOL_AMBIGUOUS_SLOT`。
/// - 锚定词不在候选里：全拼两个音节以上和拉丁名称排在 `SYMBOL_UNANCHORED_SLOT`，单音节和首字母匹配不出。
///
/// 所有位置都 ≥ 1，列表空时不加，所以混排时符号不占首位。混排之后名称词还可能被挪走（只出单字、固定位置），那一步由 `keep_expressive_rows_off_the_lead` 兜住。位置超过列表长度时接在末尾。
fn place_symbol_rows(list: &mut Vec<WordItem>, mut symbols: Vec<SymbolRow>) {
    if symbols.is_empty() || list.is_empty() {
        return;
    }
    debug_assert!(symbols.len() <= MIXED_SYMBOL_LIMIT);
    // （插在第几行，组内第几行），排好序就是插入的先后。
    let mut placed = [(0_usize, 0_usize); MIXED_SYMBOL_LIMIT];
    let mut placed_length = 0;
    let length = list.len();
    with_first_position(list, |first_position| {
        for (index, row) in symbols.iter().enumerate() {
            let anchor = row
                .anchors
                .iter()
                .filter_map(|anchor| first_position(anchor))
                .min();
            let slot = match anchor {
                Some(anchor) if row.ambiguous => (anchor + 1).max(SYMBOL_AMBIGUOUS_SLOT),
                Some(anchor) => anchor + 1,
                None if row.unanchored => SYMBOL_UNANCHORED_SLOT,
                None => continue,
            };
            placed[placed_length] = (slot.min(length), index);
            placed_length += 1;
        }
    });
    let placed = &mut placed[..placed_length];
    placed.sort_unstable();
    // 从后往前插：后面的插入不移动前面的位置；同一个位置倒着插，插完正好是查询的先后。
    for &(slot, index) in placed.iter().rev() {
        list.insert(slot, std::mem::take(&mut symbols[index].item));
    }
}

/// 混排的 emoji、颜文字和符号行（来源 `Emoji`、`Kaomoji`）总跟在一个词后面，混排时不占首位；但混排之后列表还会变：只出单字筛掉了多字的名称词，固定位置把名称词挪到了后面，接在它后面的行就顶到了首位，空格会直接上屏它（W#842）。这一步在列表定型后补救：首位是这些行时，把后面第一个能挪的词提到首位，它前面的这些行各往后挪一位，填上它空出来的位置。
///
/// 能挪的词是没有固定位置、也不坐在线座位（云候选、AI）的行，前面夹着的固定词和在线行原地不动；没有这样的词时退而取第一个词。整个列表都是这些行时不动。
pub(crate) fn keep_expressive_rows_off_the_lead(list: &mut [WordItem]) {
    let expressive = |item: &WordItem| {
        matches!(
            item.source,
            CandidateSource::Emoji | CandidateSource::Kaomoji
        )
    };
    if !list.first().is_some_and(expressive) {
        return;
    }
    let Some(word) = list
        .iter()
        .position(|item| {
            !expressive(item) && item.fixed_position == 0 && !item.source.takes_online_seat()
        })
        .or_else(|| list.iter().position(|item| !expressive(item)))
    else {
        return;
    };
    // 从词的位置往前，逐个和前面的表情、符号行交换：词换到首位，这些行按原来的先后各占下一个这样的位置，中间的词不动。
    let mut at = word;
    for earlier in (0..word).rev() {
        if expressive(&list[earlier]) {
            list.swap(earlier, at);
            at = earlier;
        }
    }
}

/// 把「某个词在列表里第一次出现的位置」交给 `use_positions`。短列表直接线性找，不分配堆内存；26 键的列表常有几百行（`ji` 七百多行），每个词都从头扫一遍太慢，先把候选词连同位置排好序再二分查找，同一个词出现几次时取最前面的位置。
fn with_first_position<R>(
    list: &[WordItem],
    use_positions: impl FnOnce(&dyn Fn(&str) -> Option<usize>) -> R,
) -> R {
    if list.len() <= EXPRESSIVE_ANCHOR_LINEAR_LIMIT {
        use_positions(&|word: &str| list.iter().position(|candidate| candidate.word == word))
    } else {
        let mut words: Vec<(&str, usize)> = list
            .iter()
            .enumerate()
            .map(|(at, candidate)| (candidate.word.as_str(), at))
            .collect();
        words.sort_unstable();
        use_positions(&|word: &str| {
            let at = words.partition_point(|(known, _)| *known < word);
            words
                .get(at)
                .filter(|(known, _)| *known == word)
                .map(|(_, position)| *position)
        })
    }
}

/// 英文第一行默认的位置：首选之后；有云候选时在它之后。AI 候选不算：引擎把 AI 行排在最后，第一条显示在第一页最后一格（input-runtime 的 `normalize_online_slots`），英文行的位置因此在回答到达前后不变（#6888）。拼音拆不开、英文完全匹配时不用它，英文直接占首位（见 `insert_mixed_rows`）。
fn priority_slot(candidates: &[WordItem]) -> usize {
    let has_cloud = candidates.iter().any(|item| {
        matches!(
            item.source,
            CandidateSource::CloudSuggestion | CandidateSource::CloudPending
        )
    });
    (1 + usize::from(has_cloud)).min(candidates.len())
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
        with_first_position(list, |first_position| add_anchors(first_position));
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
    use crate::shuangpin::profile::default_profile;

    #[test]
    fn lowercase_prefix_is_borrowed_without_allocating() {
        let (prefix, allocations) =
            crate::ime::personal_rerank::allocations::count(|| lowercase_prefix("hello"));
        assert_eq!(prefix, "hello");
        assert_eq!(allocations, 0);
    }

    #[test]
    fn mixed_skips_unused_expressive_database_path_allocations() {
        let paths = RuntimePaths {
            resources: "/synthetic/msime-unused-mixed-resources".into(),
            ..RuntimePaths::default()
        };
        let english = EnglishInputOptions {
            mixed_candidates: true,
            minimum_prefix: usize::MAX,
            first_page_count: 1,
        };
        for (prefix, english, expressive, with_japanese) in [
            ("ni", english, MixedExpressiveOptions::default(), false),
            (
                "n",
                EnglishInputOptions::default(),
                MixedExpressiveOptions {
                    emoji_candidates: true,
                    kaomoji_candidates: true,
                    symbol_candidates: true,
                },
                false,
            ),
            (
                "ni",
                EnglishInputOptions::default(),
                MixedExpressiveOptions::default(),
                true,
            ),
        ] {
            let mut queries = CandidateQueries::new(&paths, default_profile());
            // 短完整词也会查询英文；先初始化词库，只计混排阶段的无用表情路径分配。
            if english.mixed_candidates {
                queries.english_dictionary();
            }
            let mut candidates = Vec::with_capacity(3);
            candidates.extend(chinese());
            let expected = candidates.clone();
            let japanese = with_japanese.then(|| japanese("合成日文", false).unwrap());
            let (mixed, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                queries.mixed(
                    candidates,
                    prefix,
                    SchemeType::Quanpin,
                    english,
                    expressive,
                    false,
                    LocalInputMode::None,
                    true,
                    true,
                    japanese,
                    6,
                    true,
                )
            });

            assert_eq!(mixed.capacity(), 3);
            if with_japanese {
                assert_eq!(mixed[0], expected[0]);
                assert_eq!(mixed[1].word, "合成日文");
                assert_eq!(mixed[1].source, CandidateSource::MixedJapanese);
                assert_eq!(mixed[2], expected[1]);
            } else {
                assert_eq!(mixed, expected);
            }
            assert_eq!(allocations, 0, "无需表情词库时仍分配了路径：{allocations}");
        }
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
        let list = insert_mixed_rows(chinese(), english, Vec::new(), Vec::new(), None);
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
        let list = insert_mixed_rows(chinese(), english, emoji, kaomoji, None);
        assert_eq!(
            words(&list),
            vec!["你", "Ni", "倪", "Ninja", "😀", "😁", "(^_^)", "(T_T)"]
        );
        // 接不上的行每组只留前三行。
        let many = (0..5)
            .map(|index| plain(&format!("表情{index}"), CandidateSource::Emoji))
            .collect();
        let list = insert_mixed_rows(chinese(), Vec::new(), many, Vec::new(), None);
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
        let merged = insert_mixed_rows(list, Vec::new(), emoji, Vec::new(), None);
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
        let merged = insert_mixed_rows(list, Vec::new(), emoji, Vec::new(), None);
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
            None,
        );
        assert_eq!(words(&merged), vec!["美", "", "🇺🇸"]);
    }

    #[test]
    fn short_expressive_anchor_scan_does_not_allocate_word_index() {
        let mut candidates = Vec::with_capacity(3);
        candidates.extend(chinese());
        let emoji = vec![depicting("🐔", CandidateSource::Emoji, "倪 chicken")];

        let (list, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            insert_mixed_rows(candidates, Vec::new(), emoji, Vec::new(), None)
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
        let merged = insert_mixed_rows(list, Vec::new(), emoji, kaomoji, None);
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
            None,
        );
        assert_eq!(words(&merged), vec!["开心", "😄", "开心", "(^_^)"]);
    }

    #[test]
    fn anchored_rows_follow_their_word_behind_cloud_and_english_rows() {
        // 首选、云候选之后是英文首行，AI 行在引擎里排在最后；emoji 不看这些位置，只跟着它描绘的词，哪怕词就是云候选或英文词。
        let list = vec![
            row("你", CandidateSource::Database),
            row("美国", CandidateSource::CloudSuggestion),
            row("倪", CandidateSource::Database),
            row("智", CandidateSource::AiSuggestion),
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
            None,
        );
        assert_eq!(
            words(&merged),
            vec!["你", "(^_^)", "美国", "🇺🇸", "Ni", "倪", "智", "Ninja", "🥷", "😀"]
        );
    }

    #[test]
    fn a_cloud_row_pushes_the_priority_slot_back() {
        let mut list = chinese();
        list.insert(1, row("云", CandidateSource::CloudSuggestion));
        let with_cloud = insert_mixed_rows(
            list,
            vec![row("Ni", CandidateSource::EnglishDictionary)],
            Vec::new(),
            Vec::new(),
            None,
        );
        assert_eq!(words(&with_cloud), vec!["你", "云", "Ni", "倪"]);
    }

    /// AI 行不占英文的优先位置：它排在首选和云候选之后的哪里（引擎现在放在最后，旧规则放在第 2 位），英文行在其余候选中的位置都和没有 AI 行时一样（#6888）。
    #[test]
    fn an_ai_row_does_not_move_the_english_slot() {
        let english = || vec![row("Ni", CandidateSource::EnglishDictionary)];
        let others = |list: &[WordItem]| -> Vec<String> {
            list.iter()
                .filter(|item| !item.source.holds_ai_seat())
                .map(|item| item.word.to_string())
                .collect()
        };
        for cloud in [false, true] {
            let base = || {
                let mut list = chinese();
                if cloud {
                    list.insert(1, row("云", CandidateSource::CloudSuggestion));
                }
                list
            };
            let without = insert_mixed_rows(base(), english(), Vec::new(), Vec::new(), None);
            for at in [2, base().len()] {
                let mut list = base();
                list.insert(at, row("智", CandidateSource::AiSuggestion));
                let with_ai = insert_mixed_rows(list, english(), Vec::new(), Vec::new(), None);
                assert_eq!(
                    others(&with_ai),
                    others(&without),
                    "cloud {cloud}, AI at {at}"
                );
            }
        }
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
            None,
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
            insert_expressive_rows(list(), emoji(), kaomoji(), Vec::new()),
            insert_mixed_rows(list(), Vec::new(), emoji(), kaomoji(), None)
        );
        assert_eq!(
            words(&insert_expressive_rows(
                list(),
                emoji(),
                kaomoji(),
                Vec::new()
            )),
            vec!["里", "鸡", "🐔", "😁", "(^_^)"]
        );
        // 与列表里已有的行重复的不再出现。
        let duplicate = insert_expressive_rows(
            chinese(),
            vec![depicting("倪", CandidateSource::Emoji, "你")],
            Vec::new(),
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
            None,
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
            insert_mixed_rows(candidates, english, emoji, kaomoji, None)
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
            None,
        );
        assert_eq!(words(&list), vec!["你", "Ni", "倪", "😀"]);
        assert_eq!(list[3].source, CandidateSource::Emoji);
    }

    #[test]
    fn mixed_passes_through_when_it_does_not_apply() {
        let mut queries = CandidateQueries::new(&RuntimePaths::default(), default_profile());
        let on = EnglishInputOptions {
            mixed_candidates: true,
            minimum_prefix: 2,

            first_page_count: 1,
        };
        let expressive_on = MixedExpressiveOptions {
            emoji_candidates: true,
            kaomoji_candidates: true,
            symbol_candidates: true,
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
                true,
                true,
                None,
                6,
                true,
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
        let mut queries = CandidateQueries::new(&paths, default_profile());

        let candidates = queries.temporary_english("he");

        assert_eq!(candidates.len(), 11);
        assert_eq!(candidates.capacity(), 11);
    }

    fn japanese(word: &str, lead: bool) -> Option<MixedJapaneseRow> {
        let mut item = WordItem::new("kaishi", word, 0, CandidateSource::MixedJapanese, "");
        item.scheme = SchemeType::JapaneseRomaji;
        Some(MixedJapaneseRow { item, lead })
    }

    fn kaishi_chinese() -> Vec<WordItem> {
        vec![
            WordItem::new("kaishi", "开始", 1, CandidateSource::Database, "kai'shi"),
            WordItem::new("kai", "开", 1, CandidateSource::Database, "kai"),
        ]
    }

    /// 领先的日文行占首位；不领先时进优先位置，排在英文那一行前面；emoji 照旧接在它描绘的词后面。
    #[test]
    fn a_japanese_row_leads_or_takes_the_priority_slot_ahead_of_english() {
        let led = insert_mixed_rows_with_japanese(
            kaishi_chinese(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            japanese("開始", true),
            1,
            0,
        );
        assert_eq!(words(&led), vec!["開始", "开始", "开"]);

        let slotted = insert_mixed_rows_with_japanese(
            kaishi_chinese(),
            vec![english("kaishi", "kaishi")],
            vec![depicting("🎬", CandidateSource::Emoji, "开始")],
            Vec::new(),
            Vec::new(),
            None,
            japanese("開始", false),
            1,
            0,
        );
        assert_eq!(words(&slotted), vec!["开始", "🎬", "開始", "kaishi", "开"]);

        // 只有一行中文时，不领先的日文跟在它后面、英文前面。
        let short = insert_mixed_rows_with_japanese(
            vec![WordItem::new(
                "kaishi",
                "开始",
                1,
                CandidateSource::Database,
                "kai'shi",
            )],
            vec![english("kaishi", "kaishi")],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            japanese("開始", false),
            1,
            0,
        );
        assert_eq!(words(&short), vec!["开始", "開始", "kaishi"]);
    }

    /// 英文按拼音拆不开、完全匹配占了首位时，领先的日文排第二。
    #[test]
    fn a_leading_japanese_row_follows_a_leading_english_word() {
        let list = insert_mixed_rows_with_japanese(
            hello_chinese(),
            vec![english("hello", "hello")],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Some("hello"),
            japanese("へっぉ", true),
            1,
            0,
        );
        assert_eq!(words(&list), vec!["hello", "へっぉ", "何咯", "和"]);
        // 英文没有领先（只是补全）时日文占首位。
        let list = insert_mixed_rows_with_japanese(
            hello_chinese(),
            vec![english("hellos", "hellos")],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Some("hello"),
            japanese("へっぉ", true),
            1,
            0,
        );
        assert_eq!(words(&list), vec!["へっぉ", "何咯", "hellos", "和"]);
    }

    /// 有云候选时，不领先的日文和英文一样往后让，领先的仍在首位；AI 行（引擎排在最后）不让它再往后。
    #[test]
    fn japanese_rows_respect_the_cloud_seat() {
        let slot = 2;
        for ai in [None, Some(CandidateSource::AiSuggestion)] {
            let with_online = || {
                let mut list = kaishi_chinese();
                list.insert(1, row("云", CandidateSource::CloudSuggestion));
                if let Some(ai) = ai {
                    list.push(row("智", ai));
                }
                list
            };
            let slotted = insert_mixed_rows_with_japanese(
                with_online(),
                vec![english("kaishi", "kaishi")],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                None,
                japanese("開始", false),
                1,
                0,
            );
            assert_eq!(slotted[slot].word, "開始", "{ai:?} {:?}", words(&slotted));
            assert_eq!(slotted[slot + 1].word, "kaishi", "{ai:?}");
            let led = insert_mixed_rows_with_japanese(
                with_online(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                None,
                japanese("開始", true),
                1,
                0,
            );
            assert_eq!(led[0].word, "開始", "{ai:?}");
        }
    }

    /// 日文文字和已有候选（含混进来的英文）重复时不加。
    #[test]
    fn a_duplicate_japanese_row_is_dropped() {
        let mut list = kaishi_chinese();
        list.push(WordItem::new(
            "kaishi",
            "開始",
            1,
            CandidateSource::Database,
            "kai'shi",
        ));
        let merged = insert_mixed_rows_with_japanese(
            list,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            japanese("開始", true),
            1,
            0,
        );
        assert_eq!(words(&merged), vec!["开始", "开", "開始"]);
        assert!(merged
            .iter()
            .all(|item| item.source != CandidateSource::MixedJapanese));

        let merged = insert_mixed_rows_with_japanese(
            kaishi_chinese(),
            vec![english("kaishi", "OK")],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            japanese("OK", false),
            1,
            0,
        );
        assert_eq!(words(&merged), vec!["开始", "OK", "开"]);
        assert_eq!(merged[1].source, CandidateSource::EnglishDictionary);
    }

    /// 只开了日文混输（英文、emoji、颜文字都关）时 `mixed` 也要把日文行混进去。
    #[test]
    fn mixed_inserts_a_japanese_row_with_everything_else_off() {
        let mut queries = CandidateQueries::new(
            &RuntimePaths::default(),
            crate::shuangpin::profile::default_profile(),
        );
        let list = queries.mixed(
            kaishi_chinese(),
            "kaishi",
            SchemeType::Quanpin,
            EnglishInputOptions::default(),
            MixedExpressiveOptions::default(),
            false,
            LocalInputMode::None,
            true,
            true,
            japanese("開始", false),
            6,
            true,
        );
        assert_eq!(words(&list), vec!["开始", "開始", "开"]);
    }

    /// 候选混输总开关（`SessionOptions::mixed_candidates`，Windows `utility.mixed_candidates`）关掉时，
    /// 英文、emoji、颜文字、符号和日文行各分成员照常传进来也不查，列表原样返回。
    #[test]
    fn the_mixed_candidates_master_switch_gates_every_member() {
        let mut queries = CandidateQueries::new(
            &RuntimePaths::default(),
            crate::shuangpin::profile::default_profile(),
        );
        let candidates = kaishi_chinese();
        let expected = words(&candidates);
        let list = queries.mixed(
            candidates.clone(),
            "kaishi",
            SchemeType::Quanpin,
            EnglishInputOptions {
                mixed_candidates: true,
                ..EnglishInputOptions::default()
            },
            MixedExpressiveOptions {
                emoji_candidates: true,
                kaomoji_candidates: true,
                symbol_candidates: true,
            },
            false,
            LocalInputMode::None,
            true,
            true,
            japanese("開始", false),
            6,
            false,
        );
        assert_eq!(words(&list), expected);
    }

    /// 英文库里的一行：`key` 是小写词，`display` 是显示的写法。
    fn english(key: &str, display: &str) -> WordItem {
        WordItem::new(key, display, 1, CandidateSource::EnglishDictionary, "")
    }

    /// 真实词库里 `hello` 拆不开（he'l'lo），全拼给出的是拼起来的 何咯 和单字 和。
    fn hello_chinese() -> Vec<WordItem> {
        vec![
            WordItem::new("hello", "何咯", 1, CandidateSource::Generated, "he'lo"),
            WordItem::new("he", "和", 1, CandidateSource::Database, "he"),
        ]
    }

    /// 首页英文候选数（`mixed_input.first_page_count`，Windows `cn_en_mixed_input_first_page_count`）：
    /// 第一行英文进优先位置之后，其余英文紧跟它继续占首页的空位（最多到 `first_page_count` 个），
    /// 首页放不下或数量为 1 时照旧接在列表末尾——列表比页短时末尾仍在首页，与 Windows 去 `items.end()` 一致。
    #[test]
    fn first_page_count_limits_english_rows_on_the_first_page() {
        let rows = || {
            vec![
                english("hello", "hello"),
                english("hellos", "hellos"),
                english("helmet", "helmet"),
            ]
        };
        let merge = |count: usize, page: usize| {
            insert_mixed_rows_with_japanese(
                hello_chinese(),
                rows(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                None,
                None,
                count,
                page,
            )
        };
        assert_eq!(
            words(&merge(1, 6)),
            ["何咯", "hello", "和", "hellos", "helmet"]
        );
        assert_eq!(
            words(&merge(2, 6)),
            ["何咯", "hello", "hellos", "和", "helmet"]
        );
        assert_eq!(
            words(&merge(3, 6)),
            ["何咯", "hello", "hellos", "helmet", "和"]
        );
        // 首页只放得下第一行英文时，数量再大也不展开，其余接在列表末尾。
        assert_eq!(
            words(&merge(3, 2)),
            ["何咯", "hello", "和", "hellos", "helmet"]
        );
    }

    #[test]
    fn an_exact_english_word_leads_input_pinyin_cannot_spell() {
        let rows = || vec![english("hello", "hello"), english("hellos", "hellos")];
        let led = insert_mixed_rows(
            hello_chinese(),
            rows(),
            Vec::new(),
            Vec::new(),
            Some("hello"),
        );
        assert_eq!(words(&led), vec!["hello", "何咯", "和", "hellos"]);
        // 调用方没判定可以领先时仍是优先位置。
        let slotted = insert_mixed_rows(hello_chinese(), rows(), Vec::new(), Vec::new(), None);
        assert_eq!(words(&slotted), vec!["何咯", "hello", "和", "hellos"]);
        // 第一行只是补全（输入 `hell`）时不领先。
        let completion = insert_mixed_rows(
            hello_chinese(),
            rows(),
            Vec::new(),
            Vec::new(),
            Some("hell"),
        );
        assert_eq!(words(&completion), vec!["何咯", "hello", "和", "hellos"]);
    }

    #[test]
    fn only_the_first_exact_english_row_leads() {
        let list = insert_mixed_rows(
            hello_chinese(),
            vec![
                english("hello", "hello"),
                english("hello", "Hello"),
                english("hellos", "hellos"),
            ],
            Vec::new(),
            Vec::new(),
            Some("hello"),
        );
        assert_eq!(words(&list), vec!["hello", "何咯", "和", "Hello", "hellos"]);
    }

    #[test]
    fn a_leading_english_word_keeps_the_first_seat_ahead_of_cloud_and_ai_rows() {
        for ai in [CandidateSource::AiSuggestion, CandidateSource::AiPending] {
            let mut list = hello_chinese();
            list.insert(1, row("云", CandidateSource::CloudSuggestion));
            list.insert(2, row("智", ai));
            let merged = insert_mixed_rows(
                list,
                vec![english("hello", "hello")],
                Vec::new(),
                Vec::new(),
                Some("hello"),
            );
            assert_eq!(
                words(&merged),
                vec!["hello", "何咯", "云", "智", "和"],
                "{ai:?}"
            );
        }
    }

    #[test]
    fn a_deduplicated_exact_word_lets_no_completion_lead() {
        // 拼音词库里也有拉丁写法的词（`GitHub`），完全匹配的英文行和它重复时被去掉，剩下的补全词仍进优先位置。
        let mut list = hello_chinese();
        list.push(WordItem::new(
            "hello",
            "hello",
            1,
            CandidateSource::Database,
            "",
        ));
        let merged = insert_mixed_rows(
            list,
            vec![english("hello", "hello"), english("hellos", "hellos")],
            Vec::new(),
            Vec::new(),
            Some("hello"),
        );
        assert_eq!(words(&merged), vec!["何咯", "hellos", "和", "hello"]);
        assert_eq!(merged[3].source, CandidateSource::Database);
    }

    /// 用给定英文词建一个英文库，返回查询器；`TempDir` 要活到查询结束。
    fn queries_with_english(
        profile: &'static ShuangpinProfile,
        rows: &[(&str, &str, i64)],
    ) -> (tempfile::TempDir, CandidateQueries) {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join(assets::ENGLISH_DICTIONARY);
        crate::ensure_english_schema(&database).unwrap();
        let connection = Connection::open(&database).unwrap();
        for row in rows {
            connection
                .execute(
                    "INSERT INTO english_words(word,display,weight) VALUES(?1,?2,?3)",
                    *row,
                )
                .unwrap();
        }
        let paths = RuntimePaths {
            dictionaries: directory.path().to_owned(),
            ..RuntimePaths::default()
        };
        (directory, CandidateQueries::new(&paths, profile))
    }

    fn mix(
        queries: &mut CandidateQueries,
        candidates: Vec<WordItem>,
        prefix: &str,
        scheme: SchemeType,
        minimum_prefix: usize,
        english_may_lead: bool,
    ) -> Vec<WordItem> {
        queries.mixed(
            candidates,
            prefix,
            scheme,
            EnglishInputOptions {
                mixed_candidates: true,
                minimum_prefix,

                first_page_count: 1,
            },
            MixedExpressiveOptions::default(),
            false,
            LocalInputMode::None,
            english_may_lead,
            true,
            None,
            6,
            true,
        )
    }

    #[test]
    fn short_exact_english_is_available_without_completions_or_a_leading_seat() {
        let (_directory, mut queries) = queries_with_english(
            default_profile(),
            &[
                ("home", "home", 10),
                ("homeland", "homeland", 1000),
                ("cd", "CD", 100),
            ],
        );
        for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
            for _ in 0..2 {
                for (input, english) in [("home", "home"), ("cd", "CD")] {
                    let chinese = vec![WordItem::new(
                        input,
                        "合成词",
                        1,
                        CandidateSource::Database,
                        "",
                    )];
                    let list = mix(&mut queries, chinese, input, scheme, 5, true);
                    assert_eq!(words(&list), ["合成词", english]);
                    assert_eq!(list[1].pinyin, input);
                    assert_eq!(list[1].source, CandidateSource::EnglishDictionary);
                }
                assert!(mix(&mut queries, Vec::new(), "hom", scheme, 5, true).is_empty());
                assert!(mix(&mut queries, Vec::new(), "h", scheme, 5, true).is_empty());
            }
        }
    }

    #[test]
    fn quanpin_english_leads_only_input_that_is_not_whole_syllables() {
        let (_directory, mut queries) = queries_with_english(
            default_profile(),
            &[
                ("hello", "hello", 100),
                ("change", "change", 100),
                ("women", "women", 100),
                ("home", "home", 100),
            ],
        );
        let quanpin = SchemeType::Quanpin;
        let list = mix(&mut queries, hello_chinese(), "hello", quanpin, 5, true);
        assert_eq!(words(&list), vec!["hello", "何咯", "和"]);
        // 会话不让英文领先时（光标前缀、辅助码）仍在优先位置。
        let list = mix(&mut queries, hello_chinese(), "hello", quanpin, 5, false);
        assert_eq!(words(&list), vec!["何咯", "hello", "和"]);
        // chang'e、wo'men 是完整音节：英文完全匹配也只进优先位置。
        let chinese = |key: &str, word: &str| {
            vec![WordItem::new(key, word, 1, CandidateSource::Database, key)]
        };
        let list = mix(
            &mut queries,
            chinese("chang'e", "嫦娥"),
            "change",
            quanpin,
            5,
            true,
        );
        assert_eq!(words(&list), vec!["嫦娥", "change"]);
        let list = mix(
            &mut queries,
            chinese("wo'men", "我们"),
            "women",
            quanpin,
            5,
            true,
        );
        assert_eq!(words(&list), vec!["我们", "women"]);
        // 短完整词仍可选；`home`（ho'me 拆不开）在门槛 5 时不领先，放宽到 4 才领先。
        let list = mix(
            &mut queries,
            chinese("hou'me", "后面"),
            "home",
            quanpin,
            5,
            true,
        );
        assert_eq!(words(&list), vec!["后面", "home"]);
        let list = mix(
            &mut queries,
            chinese("hou'me", "后面"),
            "home",
            quanpin,
            4,
            true,
        );
        assert_eq!(words(&list), vec!["home", "后面"]);
    }

    #[test]
    fn shuangpin_english_leads_only_keys_that_are_not_whole_syllables() {
        // 小鹤：奇数长度的 `hello` 拆不成两键一个音节；`home` 是 ho（huo）me，两键都成音节。
        let (_directory, mut queries) = queries_with_english(
            default_profile(),
            &[("hello", "hello", 100), ("home", "home", 100)],
        );
        let shuangpin = SchemeType::Shuangpin;
        let chinese = |key: &str, word: &str| {
            vec![WordItem::new(key, word, 1, CandidateSource::Database, "")]
        };
        let list = mix(
            &mut queries,
            chinese("hello", "和"),
            "hello",
            shuangpin,
            4,
            true,
        );
        assert_eq!(words(&list), vec!["hello", "和"]);
        let list = mix(
            &mut queries,
            chinese("home", "火么"),
            "home",
            shuangpin,
            4,
            true,
        );
        assert_eq!(words(&list), vec!["火么", "home"]);
    }

    // ---- 打拼音出符号 ----

    /// 一行符号，`anchor` 是它名称那个词。
    fn symbol(word: &str, anchor: &'static str, ambiguous: bool, unanchored: bool) -> SymbolRow {
        SymbolRow {
            item: WordItem::new("x", word, 0, CandidateSource::Emoji, ""),
            anchors: vec![anchor],
            ambiguous,
            unanchored,
        }
    }

    fn database(words: &[&str]) -> Vec<WordItem> {
        words
            .iter()
            .map(|word| row(word, CandidateSource::Database))
            .collect()
    }

    #[test]
    fn contact_candidates_replace_dictionary_duplicates_with_name_and_phone() {
        let mut queries = CandidateQueries::new(&RuntimePaths::default(), default_profile());
        queries.set_mentions(&[MentionEntry {
            text: "拟好".into(),
            key: "ni'hao".into(),
            phone: "202-555-0101".into(),
        }]);
        for input in ["nihao", "ni'hao", "nh"] {
            let mut rows = database(&["你好", "拟好"]);
            queries.append_contact_candidates(input, &mut rows);
            assert_eq!(words(&rows), ["拟好", "拟好 202-555-0101", "你好"]);
            assert!(rows[..2]
                .iter()
                .all(|row| row.source == CandidateSource::Contact));
        }
    }

    #[test]
    fn contacts_prioritize_exact_names_and_ignore_plain_mentions() {
        let mut queries = CandidateQueries::new(&RuntimePaths::default(), default_profile());
        queries.set_mentions(&[
            MentionEntry {
                text: "合成甲乙".into(),
                key: "he'cheng'jia'yi".into(),
                phone: "202-555-0101".into(),
            },
            MentionEntry {
                text: "合成甲".into(),
                key: "he'cheng'jia".into(),
                phone: "202-555-0102".into(),
            },
            MentionEntry {
                text: "普通名单".into(),
                key: "he'cheng'jia".into(),
                phone: String::new(),
            },
        ]);
        let mut rows = database(&["合成甲", "其他词"]);
        queries.append_contact_candidates("hechengjia", &mut rows);
        assert_eq!(
            words(&rows),
            [
                "合成甲",
                "合成甲 202-555-0102",
                "合成甲乙",
                "合成甲乙 202-555-0101",
                "其他词"
            ]
        );
        for invalid in ["", "'", "he1", "HE", "其他"] {
            let mut rows = database(&["其他词"]);
            queries.append_contact_candidates(invalid, &mut rows);
            assert_eq!(words(&rows), ["其他词"]);
        }
    }

    fn with_symbols(list: Vec<WordItem>, symbols: Vec<SymbolRow>) -> Vec<WordItem> {
        insert_mixed_rows_with_japanese(
            list,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            symbols,
            None,
            None,
            1,
            0,
        )
    }

    #[test]
    fn symbols_follow_their_name_ahead_of_emoji_and_kaomoji() {
        let merged = insert_mixed_rows_with_japanese(
            database(&["正负", "政府", "正副"]),
            Vec::new(),
            vec![depicting("➕", CandidateSource::Emoji, "正负")],
            vec![depicting("(+_-)", CandidateSource::Kaomoji, "正负")],
            vec![symbol("±", "正负", false, true)],
            None,
            None,
            1,
            0,
        );
        assert_eq!(
            words(&merged),
            vec!["正负", "±", "➕", "(+_-)", "政府", "正副"]
        );
        // 名称不在首位时接在它后面。
        let merged = with_symbols(
            database(&["政府", "正负", "正副"]),
            vec![symbol("±", "正负", false, true)],
        );
        assert_eq!(words(&merged), vec!["政府", "正负", "±", "正副"]);
        // 同一个名称的几个符号按查询的先后排在一起。
        let merged = with_symbols(
            database(&["平方米", "平方", "平房"]),
            vec![
                symbol("㎡", "平方米", false, true),
                symbol("m²", "平方米", false, true),
            ],
        );
        assert_eq!(words(&merged), vec!["平方米", "㎡", "m²", "平方", "平房"]);
    }

    #[test]
    fn single_syllable_symbols_wait_until_the_fourth_place() {
        let merged = with_symbols(
            database(&["派", "排", "牌", "拍", "怕"]),
            vec![symbol("π", "派", true, false)],
        );
        assert_eq!(words(&merged), vec!["派", "排", "牌", "π", "拍", "怕"]);
        // 名称本来就排在后面时照样紧跟它。
        let merged = with_symbols(
            database(&["排", "牌", "拍", "怕", "派"]),
            vec![symbol("π", "派", true, false)],
        );
        assert_eq!(words(&merged), vec!["排", "牌", "拍", "怕", "派", "π"]);
        // 列表只有两行时接在末尾。
        let merged = with_symbols(
            database(&["派", "排"]),
            vec![symbol("π", "派", true, false)],
        );
        assert_eq!(words(&merged), vec!["派", "排", "π"]);
        // 名称不在候选里时不出。
        let merged = with_symbols(
            database(&["排", "牌"]),
            vec![symbol("π", "派", true, false)],
        );
        assert_eq!(words(&merged), vec!["排", "牌"]);
    }

    #[test]
    fn number_symbols_preserve_the_first_three_words_and_require_the_number_anchor() {
        let numbers = || {
            query_mixed_symbols(
                "yi",
                SchemeType::Quanpin,
                crate::shuangpin::profile::default_profile(),
            )
        };
        let merged = with_symbols(database(&["一", "已", "以", "易", "衣"]), numbers());
        assert_eq!(
            words(&merged),
            vec!["一", "已", "以", "①", "⑴", "⒈", "易", "衣"]
        );
        let merged = with_symbols(database(&["已", "以", "易"]), numbers());
        assert_eq!(words(&merged), vec!["已", "以", "易"]);
    }

    #[test]
    fn a_symbol_shared_by_several_names_follows_the_earliest_of_them() {
        let shared = || SymbolRow {
            item: WordItem::new("dg", "√", 0, CandidateSource::Emoji, ""),
            anchors: vec!["对钩", "打钩"],
            ambiguous: false,
            unanchored: false,
        };
        let merged = with_symbols(database(&["打钩", "大哥", "对钩"]), vec![shared()]);
        assert_eq!(words(&merged), vec!["打钩", "√", "大哥", "对钩"]);
        // 表里靠前的名称不在候选里时跟着在的那个，不因此消失。
        let merged = with_symbols(database(&["大哥", "打钩"]), vec![shared()]);
        assert_eq!(words(&merged), vec!["大哥", "打钩", "√"]);
    }

    #[test]
    fn unanchored_symbols_take_the_last_place_of_the_first_page_or_nothing() {
        let seven = || database(&["一", "二", "三", "四", "五", "六", "七"]);
        let merged = with_symbols(seven(), vec![symbol("◎", "同心圆", false, true)]);
        assert_eq!(words(&merged)[5], "◎");
        assert_eq!(merged.len(), 8);
        // 列表短时接在末尾。
        let merged = with_symbols(
            database(&["一", "二"]),
            vec![symbol("◎", "同心圆", false, true)],
        );
        assert_eq!(words(&merged), vec!["一", "二", "◎"]);
        // 首字母匹配和单音节在名称不在时不出。
        let merged = with_symbols(
            seven(),
            vec![
                symbol("℃", "摄氏度", false, false),
                symbol("π", "派", true, false),
            ],
        );
        assert_eq!(merged, seven());
        // 列表空时不加，空格键不会上屏符号。
        assert!(with_symbols(Vec::new(), vec![symbol("±", "正负", false, true)]).is_empty());
    }

    #[test]
    fn a_symbol_never_takes_the_first_place() {
        // 线性同余伪随机：列表长短不一、名称有时在首位、有时在中间、有时不在。
        let mut state = 0x2545_f491_u64;
        let mut next = |bound: usize| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            ((state >> 33) as usize) % bound
        };
        let names = ["正负", "派", "摄氏度", "同心圆"];
        for _ in 0..500 {
            let length = next(80);
            let mut list: Vec<WordItem> = (0..length)
                .map(|index| row(&format!("词{index}"), CandidateSource::Database))
                .collect();
            for name in names {
                if length > 0 && next(2) == 0 {
                    let at = next(length);
                    list[at] = row(name, CandidateSource::Database);
                }
            }
            let symbols = vec![
                symbol("±", "正负", false, true),
                symbol("π", "派", true, false),
                symbol("℃", "摄氏度", false, false),
                symbol("◎", "同心圆", false, true),
            ];
            let merged = with_symbols(list, symbols);
            if let Some(first) = merged.first() {
                assert_ne!(first.source, CandidateSource::Emoji, "{:?}", words(&merged));
            }
        }
    }

    #[test]
    fn leading_expressive_rows_move_behind_the_first_word() {
        let mut list = vec![
            row(">", CandidateSource::Emoji),
            row("(>_<)", CandidateSource::Kaomoji),
            row("大于", CandidateSource::Database),
            row("大", CandidateSource::Database),
        ];
        keep_expressive_rows_off_the_lead(&mut list);
        assert_eq!(words(&list), vec!["大于", ">", "(>_<)", "大"]);
        // 首位已经是词时不动；整个列表都是这些行时也不动。
        keep_expressive_rows_off_the_lead(&mut list);
        assert_eq!(words(&list), vec!["大于", ">", "(>_<)", "大"]);
        let mut only = vec![
            row("😀", CandidateSource::Emoji),
            row("(^_^)", CandidateSource::Kaomoji),
        ];
        keep_expressive_rows_off_the_lead(&mut only);
        assert_eq!(words(&only), vec!["😀", "(^_^)"]);
        keep_expressive_rows_off_the_lead(&mut []);

        // 固定在第 3 位的词和第 2 位的云候选原地不动，提到首位的是后面第一个普通词。
        let mut fixed = row("大于", CandidateSource::Database);
        fixed.fixed_position = 3;
        let mut list = vec![
            row(">", CandidateSource::Emoji),
            row("大鱼", CandidateSource::CloudSuggestion),
            fixed,
            row("＞", CandidateSource::Emoji),
            row("大", CandidateSource::Database),
            row("达", CandidateSource::Database),
        ];
        keep_expressive_rows_off_the_lead(&mut list);
        assert_eq!(words(&list), vec!["大", "大鱼", "大于", ">", "＞", "达"]);
        // 只剩固定词时退而取第一个词。
        let mut fixed = row("大于", CandidateSource::Database);
        fixed.fixed_position = 3;
        let mut list = vec![row(">", CandidateSource::Emoji), fixed];
        keep_expressive_rows_off_the_lead(&mut list);
        assert_eq!(words(&list), vec!["大于", ">"]);
    }

    #[test]
    fn symbols_are_deduplicated_and_win_over_emoji_presentation() {
        // 列表里已有同样文字时不再加。
        let merged = with_symbols(
            database(&["大于", ">"]),
            vec![
                symbol(">", "大于", false, true),
                symbol("＞", "大于", false, true),
            ],
        );
        assert_eq!(words(&merged), vec!["大于", "＞", ">"]);
        // ✔️ 与符号 ✔ 只差 U+FE0F：留符号行，去掉 emoji 行；文字完全相同的 emoji 行同样去掉。
        let merged = insert_mixed_rows_with_japanese(
            database(&["对号", "对好"]),
            Vec::new(),
            vec![
                depicting("✔\u{FE0F}", CandidateSource::Emoji, "对号"),
                depicting("√", CandidateSource::Emoji, "对号"),
                depicting("👍", CandidateSource::Emoji, "对号"),
            ],
            Vec::new(),
            vec![
                symbol("√", "对号", false, true),
                symbol("✔", "对号", false, true),
            ],
            None,
            None,
            1,
            0,
        );
        assert_eq!(words(&merged), vec!["对号", "√", "✔", "👍", "对好"]);
    }

    #[test]
    fn latin_symbols_follow_the_english_row_of_their_name() {
        let merged = insert_mixed_rows_with_japanese(
            database(&["嘎嘛", "噶"]),
            vec![english("gamma", "gamma")],
            Vec::new(),
            Vec::new(),
            vec![
                symbol("γ", "gamma", false, true),
                symbol("Γ", "gamma", false, true),
            ],
            Some("gamma"),
            None,
            1,
            0,
        );
        assert_eq!(words(&merged), vec!["gamma", "γ", "Γ", "嘎嘛", "噶"]);
        // 英文不领先时进优先位置，符号照样跟着它。
        let merged = insert_mixed_rows_with_japanese(
            database(&["嘎嘛", "噶"]),
            vec![english("gamma", "gamma")],
            Vec::new(),
            Vec::new(),
            vec![symbol("γ", "gamma", false, true)],
            None,
            None,
            1,
            0,
        );
        assert_eq!(words(&merged), vec!["嘎嘛", "gamma", "γ", "噶"]);
    }

    #[test]
    fn mixed_adds_symbols_only_when_switched_on_and_allowed() {
        let mut queries = CandidateQueries::new(&RuntimePaths::default(), default_profile());
        let list = || database(&["政府", "正负", "正副"]);
        let symbols_on = MixedExpressiveOptions {
            symbol_candidates: true,
            ..MixedExpressiveOptions::default()
        };
        let mix = |queries: &mut CandidateQueries, expressive, allowed, scheme| {
            queries.mixed(
                list(),
                "zhengfu",
                scheme,
                EnglishInputOptions::default(),
                expressive,
                false,
                LocalInputMode::None,
                true,
                allowed,
                None,
                6,
                true,
            )
        };
        assert_eq!(
            words(&mix(&mut queries, symbols_on, true, SchemeType::Quanpin)),
            vec!["政府", "正负", "±", "正副"]
        );
        // 开关关着、调用方不允许（光标前缀解码、辅助码激活）、五笔：列表原样。
        assert_eq!(
            mix(
                &mut queries,
                MixedExpressiveOptions::default(),
                true,
                SchemeType::Quanpin
            ),
            list()
        );
        assert_eq!(
            mix(&mut queries, symbols_on, false, SchemeType::Quanpin),
            list()
        );
        assert_eq!(
            mix(&mut queries, symbols_on, true, SchemeType::Wubi),
            list()
        );
        assert!(queries.english.is_none());
    }
}
