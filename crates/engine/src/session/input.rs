//! `InputSession` (core-session.md §5): the platform-neutral composition and commit policy. Three mutually exclusive views drive every getter: dedicated English, a local mode, or the scheme composition. Its behaviour is split over this module's sibling files by concern; this file holds the state and the key, command and punctuation dispatch.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::candidates::clone_candidate_rows;
use super::chain::CommitChain;
use super::clock::Clock;
use super::conversion::ConversionEdit;
use super::online::OnlineRequestGuard;
use super::options::SessionOptions;
use crate::assets;
use crate::cantonese;
use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::helpcode::{
    is_supported_helpcode_schema, load_helpcode_keymap_with_fallback, SharedKeymap,
};
use crate::ime::queries::CandidateQueries;
use crate::ime::ImeSession;
use crate::local::date_time::LocalDateTime;
use crate::local::url;
use crate::local::GENERATED_MODE_INPUT_LIMIT;
use crate::paths::RuntimePaths;
use crate::punctuation::PunctuationPolicy;
use crate::quanpin::QuanpinEngine;
use crate::shuangpin::custom::session_profile;
use crate::shuangpin::ShuangpinProfile;
use crate::stroke;
use crate::tibetan::{SHAD, TSHEG};
use crate::time::Instant;
use crate::types::{
    CandidateSource, Command, CommandTableEntry, EnglishInputOptions, FirstPageRows,
    FrequencyAdjustmentOptions, KeyResult, LocalInputMode, LocalModeOptions, MentionEntry,
    MixedExpressiveOptions, PluginCommand, QuickPhraseEntry, SchemeKey, SchemeType,
    ShuangpinHelpcodeModes, WordItem, WubiInputOptions,
};
use crate::user_dictionary::ngram_store::PersonalNgramStore;
use crate::user_dictionary::removal::learn_entered_english_word;
use crate::user_dictionary::typo_profile::PersonalTypoProfile;
use crate::zhuyin;
use crate::zhuyin::scheme::ZhuyinKey;

/// The weight an English word typed out and committed raw enters `msime-english.db` with (user_dictionary_journal.h:137).
const ENTERED_ENGLISH_WORD_WEIGHT: i64 = 10;

/// A phrase being composed from consecutive partial selections.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct CreatingWordProgress {
    pub pinyin: String,
    pub word: String,
    pub preedit: String,
    pub completed: bool,
    pub can_store: bool,
}

pub(super) struct InputSession {
    pub paths: RuntimePaths,
    pub journal_path: PathBuf,
    pub engine: ImeSession,
    pub queries: CandidateQueries,
    pub quick_phrase_candidates: bool,
    /// 快捷短语参与调频（`SessionOptions::quick_phrase_frequency`）：追加进普通候选的整组短语有槽位，越过它选普通候选让整组后退、选中组内短语让整组前移。
    pub quick_phrase_frequency: bool,
    /// 宿主的每页候选数（`SessionOptions::first_page_rows`），0 归一成 1；快捷短语组调频的后退封顶（首页末位）用它算。
    pub first_page_rows: usize,
    pub clock: Clock,
    pub profile: &'static ShuangpinProfile,
    pub caret: Option<usize>,
    pub phrase_progress: CreatingWordProgress,
    pub pending_sequence: Option<String>,
    pub pending_sequence_with_cases: Option<String>,
    pub autocorrect_types: u32,
    pub quanpin_helpcode_enabled: bool,
    pub shuangpin_helpcode_enabled: bool,
    pub helpcode_trigger_key: Option<u8>,
    /// 前导键之后、辅助码之前的原文长度。
    pub helpcode_entry: Option<usize>,
    pub helpcode_keymap: Option<SharedKeymap>,
    pub helpcode_fallback_resources: Option<PathBuf>,
    pub chinese_punctuation_enabled: bool,
    pub punctuation_lock: i32,
    pub learning_enabled: bool,
    /// 回车上屏英文拼写时是否学进英文词库（`SessionOptions::learn_english_on_enter`）。
    pub learn_english_on_enter: bool,
    pub personal_context_enabled: bool,
    pub punctuation: PunctuationPolicy,
    pub frequency: FrequencyAdjustmentOptions,
    pub local_mode_options: LocalModeOptions,
    pub english_options: EnglishInputOptions,
    pub expressive_options: MixedExpressiveOptions,
    /// 日文混输开着（`SessionOptions::japanese_mixing`）。
    pub japanese_mixing: bool,
    pub dedicated_english: bool,
    pub dedicated_english_preedit: String,
    pub dedicated_english_candidates: Vec<WordItem>,
    pub english_online_candidates_enabled: bool,
    pub english_online_rows: Vec<WordItem>,
    pub mixed_candidates: Vec<WordItem>,
    /// 候选混输总开关（`SessionOptions::mixed_candidates`）：关着时英文、emoji、颜文字、符号、日文混输行、行内日期时间和快捷短语都不混进普通候选，各分成员原值保留。
    pub mixed_candidates_enabled: bool,
    pub local_mode: LocalInputMode,
    /// The scheme to return to after temporary Japanese.
    pub temporary_original_scheme: Option<SchemeType>,
    pub local_preedit: String,
    pub local_candidates: Vec<WordItem>,
    /// 上一枚按键是交给宿主的大写字母（没有组字时 Shift+字母就走这条路）。连续按住 Shift 打大写时它让这一串开了头：后面的字母不再进快捷模式，第一枚仍照旧（#7616）。
    pub uppercase_passthrough: bool,
    /// 插件指令行的缓存：（问的是哪一份本地预编辑串，插件给的行）。只有一槽，预编辑串一变就清掉，所以 `/uuid` 退成 `/uui` 再打回 `/uuid` 会重新问插件；预编辑串不变时（方向键、翻页、列表重建）行一直在，不再问。0 行也缓存成空表。
    pub plugin_rows: Option<(String, Vec<WordItem>)>,
    pub online_requests: OnlineRequestGuard,
    pub cloud_candidates_enabled: bool,
    pub chain: CommitChain,
    pub personal_context: Arc<PersonalNgramStore>,
    pub typo_profile: Arc<PersonalTypoProfile>,
    pub personal_reranked: bool,
    /// The list before the personal rerank; learning, pinning and index-0 checks use this order.
    pub ranking_candidates: Option<Vec<WordItem>>,
    /// Caret-prefix decoding (overlays.md §7.6).
    pub prefix_active: bool,
    pub prefix_query_input: String,
    pub prefix_candidates: Vec<WordItem>,
    pub shuangpin_preedit_uses_raw: bool,
    pub single_character_only: bool,
    /// Writes phrases under complete quanpin keys whatever the active scheme is, so a shuangpin session never feeds a canonical key back through its profile; opened on first use.
    pub canonical_phrase_engine: Option<QuanpinEngine>,
    /// 整句改字（`conversion`）进行中时的状态；改字期间候选是光标处那一段的字和词。
    pub conversion: Option<ConversionEdit>,
    /// 「📅日期」页：选中混输里的入口后，候选窗换成这一组的全部格式（`EnterDateTimeCandidatePage`）。组字和 `mixed_candidates` 原样留着，任何非选词键把 `candidates()` 视图还回去。
    pub date_time_menu_page: Option<Vec<WordItem>>,
}

impl InputSession {
    /// Everything `Session::new` applies, in the reference order.
    pub fn new(options: &SessionOptions) -> Result<Self> {
        if options
            .helpcode_trigger_key
            .is_some_and(|key| !b";/\\`=".contains(&key))
        {
            return Err(EngineError::invalid(diagnostics::INVALID_SESSION_OPTIONS));
        }
        let paths = options.paths.clone();
        let journal = paths.user(assets::USER_JOURNAL);
        let profile = session_profile(
            options.shuangpin_profile,
            options.shuangpin_custom_profile.as_ref(),
            options.enabled_schemes.contains(SchemeType::Shuangpin),
        )?;
        let mut engine = ImeSession::new(
            options.scheme,
            options.enabled_schemes,
            profile,
            &paths,
            options.cantonese_dictionary.clone(),
            options.zhuyin_dictionary.clone(),
            options.stroke_dictionary.clone(),
            options.japanese_dictionary.clone(),
        )?;
        engine.set_autocorrect_types(0);
        engine.set_quanpin_helpcode_enabled(true);
        engine.set_shuangpin_helpcode_enabled(true);
        engine.set_korean_layout(options.korean_layout);
        engine.set_vietnamese_options(
            options.vietnamese_input_method,
            options.vietnamese_tone_style,
        );
        let mut session = Self {
            queries: CandidateQueries::new(&paths, profile),
            quick_phrase_candidates: options.quick_phrase_candidates,
            quick_phrase_frequency: options.quick_phrase_frequency,
            first_page_rows: FirstPageRows::new(options.first_page_rows).get(),
            engine,
            clock: Clock::default(),
            profile,
            caret: None,
            phrase_progress: CreatingWordProgress::default(),
            pending_sequence: None,
            pending_sequence_with_cases: None,
            autocorrect_types: 0,
            quanpin_helpcode_enabled: true,
            shuangpin_helpcode_enabled: true,
            helpcode_trigger_key: options.helpcode_trigger_key,
            helpcode_entry: None,
            helpcode_keymap: None,
            helpcode_fallback_resources: options.helpcode_fallback_resources.clone(),
            chinese_punctuation_enabled: true,
            punctuation_lock: 0,
            learning_enabled: true,
            learn_english_on_enter: options.learn_english_on_enter,
            personal_context_enabled: true,
            punctuation: PunctuationPolicy::default(),
            frequency: FrequencyAdjustmentOptions::default(),
            local_mode_options: LocalModeOptions::default(),
            english_options: EnglishInputOptions::default(),
            expressive_options: MixedExpressiveOptions::default(),
            japanese_mixing: false,
            dedicated_english: false,
            dedicated_english_preedit: String::new(),
            dedicated_english_candidates: Vec::new(),
            english_online_candidates_enabled: false,
            english_online_rows: Vec::new(),
            mixed_candidates: Vec::new(),
            mixed_candidates_enabled: true,
            local_mode: LocalInputMode::None,
            temporary_original_scheme: None,
            local_preedit: String::new(),
            local_candidates: Vec::new(),
            uppercase_passthrough: false,
            plugin_rows: None,
            date_time_menu_page: None,
            online_requests: OnlineRequestGuard::new(),
            // 宿主创建会话后显式打开；测试和嵌入式引擎默认保持离线。
            cloud_candidates_enabled: false,
            chain: CommitChain::default(),
            personal_context: PersonalNgramStore::for_journal(&journal),
            typo_profile: PersonalTypoProfile::shared(&journal),
            personal_reranked: false,
            ranking_candidates: None,
            prefix_active: false,
            prefix_query_input: String::new(),
            prefix_candidates: Vec::new(),
            shuangpin_preedit_uses_raw: true,
            single_character_only: options.single_character_only,
            canonical_phrase_engine: None,
            conversion: None,
            journal_path: journal,
            paths,
        };

        // session.cpp:15-29.
        session.shuangpin_preedit_uses_raw = options.shuangpin_preedit_uses_raw;
        session.set_autocorrect_types(options.autocorrect_types);
        session
            .engine
            .set_fuzzy_pinyin_options(options.fuzzy_pinyin);
        session.set_quanpin_helpcode_enabled(options.helpcode);
        session.set_shuangpin_helpcode_enabled(options.helpcode);
        session.set_shuangpin_helpcode_modes(options.shuangpin_helpcode_modes);
        if !is_supported_helpcode_schema(&options.helpcode_schema)
            || !frequency_options_valid(options.frequency)
            || !english_options_valid(options.english)
        {
            return Err(EngineError::invalid(diagnostics::INVALID_SESSION_OPTIONS));
        }
        // A missing custom table passes the name check; its load error (`UNKNOWN_HELPCODE_SCHEMA`) is what the caller sees.
        match &options.helpcode_table {
            // 宿主给了表（辅助码表插件）就直接用它，不读 schema 对应的文件。
            Some(table) => session.install_helpcode_table(table.clone()),
            None => session.install_helpcode_keymap(&options.helpcode_schema)?,
        }
        session.frequency = options.frequency;
        session.mixed_candidates_enabled = options.mixed_candidates;
        session.english_options = options.english;
        session.set_local_mode_options(options.local_modes);
        session.queries.set_command_table(&options.command_table);
        session
            .queries
            .set_plugin_commands(&options.plugin_commands);
        session.queries.set_mentions(&options.mention_entries);
        session
            .queries
            .set_quick_phrase_table(&options.quick_phrase_table);
        session
            .queries
            .set_chaizi_traditional_output(options.chaizi_traditional_output);
        session.expressive_options = options.expressive;
        session.japanese_mixing = options.japanese_mixing;
        if options.japanese_mixing {
            session.engine.warm_up_japanese();
        }
        session.set_wubi_input_options(options.wubi);
        session.set_personal_context_enabled(options.personal_context);

        // session.cpp:61-65, plus the neural switches the patch threaded through the same place.
        session.chinese_punctuation_enabled = options.chinese_punctuation;
        session
            .punctuation
            .set_paired_enabled(options.paired_punctuation);
        session.punctuation.set_marks(options.punctuation_marks);
        session.punctuation_lock = options.punctuation_lock;
        session.learning_enabled = options.learning;
        session
            .engine
            .set_sentence_alternatives(options.sentence_alternatives);
        session
            .engine
            .set_sentence_association(options.sentence_association.clone());
        session
            .engine
            .set_first_page_rows(FirstPageRows::new(options.first_page_rows));
        session
            .engine
            .set_rescoring_context(&options.rescoring_context);
        session.update_mixed_candidates();
        Ok(session)
    }

    /// input_session.cpp:88-245: caret insertion, dedicated English, local modes and their Shift entries, the acceptance gate, then the scheme. Handled iff the preedit changed.
    pub fn handle_character(&mut self, value: u8, shift_only: bool) -> KeyResult {
        self.exit_date_time_menu();
        let result = self
            .helpcode_character(value)
            .unwrap_or_else(|| self.dispatch_character(value, shift_only));
        // #7616：交还给宿主的大写字母让这一串开了头，直到下一枚不是大写字母的键为止。
        self.uppercase_passthrough = value.is_ascii_uppercase() && !result.handled;
        result
    }

    /// 这一串大写到此为止：不是「交给宿主的大写字母」的按键都算结束，下一枚字母重新算这一串的第一枚（#7616）。
    pub(super) fn end_uppercase_run(&mut self) {
        self.uppercase_passthrough = false;
    }

    pub(super) fn dispatch_character(&mut self, value: u8, shift_only: bool) -> KeyResult {
        // 改字时能写进组字的键先退出改字再照常键入；数字和空格等留给候选选择，宿主或 runtime 再按选择处理。
        if self.conversion.is_some() {
            if !(value.is_ascii_alphabetic() || value == b'\'' || value == b';') {
                return KeyResult::unhandled();
            }
            self.leave_conversion();
        }
        if self.takes_literal_symbol(value) {
            return self.insert_literal_symbol(value);
        }
        if self
            .caret
            .is_some_and(|caret| caret < self.editing_text_len())
        {
            return self.insert_at_caret(value);
        }
        self.caret = None;
        if self.dedicated_english {
            // Every other key is swallowed, even with nothing composed, so the English mode never leaks Chinese punctuation or digits into the host (core-session.md §15.2).
            if value.is_ascii_alphabetic() {
                self.invalidate_online_requests();
                self.dedicated_english_preedit.push(value as char);
                self.update_dedicated_english_candidates();
            }
            return KeyResult::handled();
        }
        if self.local_mode != LocalInputMode::None {
            return self.handle_local_character(value);
        }
        if self.is_korean() {
            return self.handle_korean_character(value);
        }
        if self.is_vietnamese() {
            return self.handle_vietnamese_character(value);
        }
        if self.is_tibetan() {
            return self.handle_tibetan_character(value);
        }
        if self.is_zhuyin() {
            return self.handle_zhuyin_character(value);
        }
        if self.is_stroke() {
            return self.handle_stroke_character(value);
        }
        if !self.has_composition() {
            // 连续大写里只有第一枚字母能进快捷模式，第二枚起留给宿主当大写字母。
            let entry = if shift_only && !self.uppercase_passthrough {
                self.local_mode_for_entry(value)
            } else {
                None
            }
            .or_else(|| self.local_mode_for_symbol(value));
            if let Some(mode) = entry {
                return KeyResult::handled().with_diagnostic(self.enter_local_mode(mode, value));
            }
        }
        // 组字原文是 `www`、`http` 等时，`.` `:`（五笔还有 `http` 后的 `s`）把组字转成网址；原文由 ASCII 字母、数字组成时，`@` 把它转成邮箱。必须在下面的接受过滤之前判断。
        if let Some(text) = self.url_entry(value) {
            self.enter_url_mode(text);
            return KeyResult::handled();
        }

        let scheme = self.scheme();
        let lowercase_letter = value.is_ascii_lowercase();
        // 双拼辅助码的间接触发键（`event_listener_keys.cpp` 的对应物）：`;` 先让给 `ing` 韵母，
        // 再看它当前归句中还是末尾；反引号同理。返回写进输入串的标记字符。
        let helpcode_marker = self.shuangpin_helpcode_marker(value);
        let microsoft_final = value == b';'
            && scheme == SchemeType::Shuangpin
            && self.profile.uses_semicolon_key()
            && helpcode_marker.is_none();
        let japanese_long_vowel = value == b'-' && scheme == SchemeType::JapaneseRomaji;
        // 已启用的快捷入口在上面处理；被关闭或资源不可用的入口按普通大写字母继续。
        let starts_pinyin_with_shift =
            shift_only && self.local_mode_options.shift_letter_entry && !self.uppercase_passthrough;
        let composing_pinyin_uppercase = value.is_ascii_uppercase()
            && (self.has_composition() || starts_pinyin_with_shift)
            && matches!(scheme, SchemeType::Quanpin | SchemeType::Shuangpin);
        if !lowercase_letter
            && !composing_pinyin_uppercase
            && value != b'\''
            && !microsoft_final
            && !japanese_long_vowel
            && helpcode_marker.is_none()
        {
            // With nothing composed the host inserts the key itself (a digit, a space, a capital), so the next word no longer follows the last one.
            if !self.has_composition() {
                self.reset_commit_context();
            }
            return KeyResult::unhandled();
        }
        if value == b'\'' && !self.has_composition() {
            self.reset_commit_context();
            return KeyResult::unhandled();
        }
        // A long pause before a new composition usually means the user moved to another field or application, which a host that never calls reset_context() does not report.
        if !self.has_composition()
            && self.chain.previous.is_some()
            && self.chain.paused(self.steady_now())
        {
            self.chain.reset();
        }

        let key = if value == b'\'' {
            SchemeKey::Apostrophe
        } else if let Some(marker) = helpcode_marker {
            // 触发键在输入串里一律记成标记字符（反引号或 '|'），与用户敲的键无关。
            SchemeKey::Symbol(marker)
        } else if microsoft_final {
            SchemeKey::Semicolon
        } else if japanese_long_vowel {
            SchemeKey::Minus
        } else if value.is_ascii_alphabetic() {
            SchemeKey::Letter(value)
        } else {
            // Only a key the scheme claims gets past the filter above; none of the current schemes claims one here.
            SchemeKey::Symbol(value)
        };
        let changed = self.engine.handle_key(key);
        self.update_mixed_candidates();
        // A key the scheme ignores (a fifth wubi letter, a second apostrophe) leaves the preedit alone and goes back to the host.
        if !changed {
            return KeyResult::unhandled();
        }
        self.invalidate_online_requests();
        KeyResult::handled()
    }

    /// input_session.cpp:958-1036.
    pub fn handle_local_character(&mut self, value: u8) -> KeyResult {
        let letter = value.is_ascii_alphabetic();
        let accepted = match self.local_mode {
            // 临时模式不接受的键交还宿主，同时保留当前预编辑；其他模式在下方先提交原文再退出。
            LocalInputMode::TemporaryEnglish => {
                if !letter {
                    return KeyResult::unhandled();
                }
                true
            }
            LocalInputMode::TemporaryJapanese => {
                if !letter && !matches!(value, b'\'' | b'-') {
                    return KeyResult::unhandled();
                }
                let key = match value {
                    b'\'' => SchemeKey::Apostrophe,
                    b'-' => SchemeKey::Minus,
                    _ => SchemeKey::Letter(value),
                };
                self.engine.handle_key(key);
                self.refresh_temporary_japanese();
                return KeyResult::handled();
            }
            // 空 J 入口后的大写属于英文串；交给下方拒绝大写分支转回普通组字。
            LocalInputMode::SuperJianpin => {
                letter
                    && !(value.is_ascii_uppercase()
                        && self.local_preedit.len() == 1
                        && matches!(self.scheme(), SchemeType::Quanpin | SchemeType::Shuangpin))
            }
            LocalInputMode::Emoji | LocalInputMode::Kaomoji => letter || value == b'\'',
            // 符号模式只收字母：分组码与符号名都是字母，撇号、数字和 `;` 交还宿主。
            LocalInputMode::Symbol => letter,
            LocalInputMode::QuickPhrase | LocalInputMode::DateTime => value.is_ascii_lowercase(),
            LocalInputMode::Unicode => {
                value.is_ascii_hexdigit() || (value == b'+' && self.local_preedit == "U")
            }
            LocalInputMode::Expression => {
                // 单位字母只在数字后接收；唯一的撇号分隔单位与目标，必须紧跟单位字母。宿主的拼写符号仍只含数字与运算符。
                let unit_letter =
                    self.expression_takes_unit_letter(value, self.local_preedit.len());
                let unit_separator = value == b'\''
                    && self.expression_takes_unit_separator(self.local_preedit.len());
                self.local_preedit.len() < GENERATED_MODE_INPUT_LIMIT
                    && (unit_letter
                        || unit_separator
                        || self
                            .local_mode
                            .spelling_symbols()
                            .as_bytes()
                            .contains(&value))
            }
            LocalInputMode::Command | LocalInputMode::Mention => {
                if self.local_preedit.len() == 1 && value.is_ascii_punctuation() {
                    return self.commit_bare_prefix(value);
                }
                self.local_preedit.len() < GENERATED_MODE_INPUT_LIMIT
                    && (value.is_ascii_lowercase()
                        || (value == b'\''
                            && self.command_takes_word_separator(self.local_preedit.len())))
            }
            LocalInputMode::Url => {
                // 网址不收的符号（`"` `<` `|` 等）和空格结束网址：交还 runtime，由它先上屏网址再处理这个键。
                if !url::accepts(value) {
                    return KeyResult::unhandled();
                }
                self.local_preedit.len() < url::INPUT_LIMIT
            }
            // 拆字收字母（大小写照原样写进预编辑，有大写时不查表），`'` 只收在部件之后，用来分隔部件；微软双拼的 `;` 是韵母 ing。
            LocalInputMode::Chaizi => {
                self.local_preedit.len() < GENERATED_MODE_INPUT_LIMIT
                    && (letter || self.chaizi_takes_symbol(value, self.local_preedit.len()))
            }
            LocalInputMode::None => return KeyResult::unhandled(),
        };
        if !accepted {
            // 空入口还没有模式内容，拒绝的第二枚大写留在同一组字，避免把 UV 拆开上屏。
            if value.is_ascii_uppercase()
                && self.local_preedit.len() == 1
                && self.local_preedit.as_bytes()[0].is_ascii_uppercase()
                && matches!(self.scheme(), SchemeType::Quanpin | SchemeType::Shuangpin)
            {
                let mut raw = self.local_preedit.clone();
                raw.push(value as char);
                self.reset_composition();
                self.engine.replace_active_raw_input(&raw, &raw);
                self.update_mixed_candidates();
                return KeyResult::handled();
            }
            // 不吞掉模式无法拼写的键；先提交所见原文，再由宿主继续处理当前按键。
            let mut result = self.commit_raw();
            result.handled = false;
            return result;
        }
        self.invalidate_online_requests();
        self.local_preedit.push(value as char);
        KeyResult::handled().with_diagnostic(self.update_local_candidates())
    }

    /// Korean keys: every letter is a jamo and always handled, carrying the syllables it finished as a commit; a letter typed into an open Hanja list closes it and keeps composing. Any other key leaves the scheme alone. Punctuation stays unhandled without touching the composition, because the punctuation route commits the open syllable ahead of the mark; a digit 1-9 with the Hanja list open stays unhandled with nothing committed, so the runtime's page selection picks the Hanja; any other digit, a space or another symbol commits the open syllable and stays unhandled, so the host inserts the key after the commit.
    fn handle_korean_character(&mut self, value: u8) -> KeyResult {
        if !value.is_ascii_alphabetic() {
            if !self.has_composition() {
                self.reset_commit_context();
                return KeyResult::unhandled();
            }
            if value.is_ascii_punctuation()
                || ((b'1'..=b'9').contains(&value) && self.engine.korean_hanja_open())
            {
                return KeyResult::unhandled();
            }
            return self.commit_korean_composition();
        }
        self.engine.handle_key(SchemeKey::Letter(value));
        let finished = self.engine.take_korean_commit();
        self.update_mixed_candidates();
        self.invalidate_online_requests();
        if finished.is_empty() {
            return KeyResult::handled();
        }
        self.chain.reset();
        KeyResult::committed(finished)
    }

    /// The open Korean syllable goes to the host and the key that ended it does not: the result is unhandled, so Space, Enter, a caret key or a digit still does its own work after the commit, as in every Korean input method.
    fn commit_korean_composition(&mut self) -> KeyResult {
        let text = self.preedit();
        self.reset_composition();
        self.chain.reset();
        KeyResult {
            handled: false,
            commit: Some(text),
            diagnostic: None,
        }
    }

    /// Vietnamese keys: a letter always spells, and a VNI digit spells while a word is composing; both are handled. With nothing composing any other key goes to the host. With a word composing, punctuation stays unhandled without touching it, because the punctuation route commits the word ahead of the mark; any other key commits the word and stays unhandled, so the host inserts the key after the commit.
    fn handle_vietnamese_character(&mut self, value: u8) -> KeyResult {
        let spells = value.is_ascii_alphabetic()
            || self
                .engine
                .vietnamese_spelling_symbols()
                .as_bytes()
                .contains(&value);
        if !spells {
            if !self.has_composition() {
                self.reset_commit_context();
                return KeyResult::unhandled();
            }
            if value.is_ascii_punctuation() {
                return KeyResult::unhandled();
            }
            return self.commit_vietnamese_composition();
        }
        // A long pause before a new word usually means the user moved to another field or application.
        if !self.has_composition()
            && self.chain.previous.is_some()
            && self.chain.paused(self.steady_now())
        {
            self.chain.reset();
        }
        let key = if value.is_ascii_alphabetic() {
            SchemeKey::Letter(value)
        } else {
            SchemeKey::Symbol(value)
        };
        self.engine.handle_key(key);
        self.update_mixed_candidates();
        self.invalidate_online_requests();
        KeyResult::handled()
    }

    /// 笔画键：`hspnz` 追加一笔，组合中 `x` 追加通配。空组合时其他键（含 `x`）交还宿主；组合中其他字母被吞掉，组合不变，非字母键（数字、空格、标点）照常不处理，由选词和标点路径接手。
    fn handle_stroke_character(&mut self, value: u8) -> KeyResult {
        if !self.has_composition() {
            if !stroke::is_stroke(value) {
                // The host inserts the key itself, so the next word no longer follows the last one.
                self.reset_commit_context();
                return KeyResult::unhandled();
            }
            // A long pause before a new composition usually means the user moved to another field or application.
            if self.chain.previous.is_some() && self.chain.paused(self.steady_now()) {
                self.chain.reset();
            }
        } else if !value.is_ascii_alphabetic() {
            return KeyResult::unhandled();
        }
        let previous = self.engine.request().raw_input.len();
        self.engine.handle_key(SchemeKey::Letter(value));
        self.update_mixed_candidates();
        if self.engine.request().raw_input.len() != previous {
            self.invalidate_online_requests();
        }
        KeyResult::handled()
    }

    /// The Vietnamese word goes to the host as displayed, and the key that ended it does not: the result is unhandled, so Space, Enter, a caret key or Tab still does its own work after the commit. Nothing is learned.
    fn commit_vietnamese_composition(&mut self) -> KeyResult {
        let text = self.preedit();
        self.reset_composition();
        self.chain.reset();
        KeyResult {
            handled: false,
            commit: Some(text),
            diagnostic: None,
        }
    }

    /// 藏文按键：威利能拼写的字母和当前状态的拼写符号（威利读不了的字母见 `commit_unspelled_tibetan_letter`）（`'` 随时，`+` `.` `-` 在组字时）进入威利原文，都算已处理。空格带音节点、`/` 带垂符上屏音节串（以 ང 结尾时垂符前补音节点），并且已处理；没有组字时 `/` 单独输出垂符，其他按键交给宿主。有组字时标点保持未处理且不碰组字，因为标点路由会先上屏音节串再输出标点；其他按键上屏音节串并保持未处理，由宿主在上屏后插入该键。
    fn handle_tibetan_character(&mut self, value: u8) -> KeyResult {
        if value == b'/' {
            return self.end_tibetan_syllables(SHAD, value);
        }
        if value == b' ' && self.has_composition() {
            return self.end_tibetan_syllables(TSHEG, value);
        }
        if value.is_ascii_alphabetic() && !self.engine.tibetan_claims_letter(value) {
            return self.commit_unspelled_tibetan_letter(value);
        }
        let spells = self.engine.tibetan_claims_letter(value)
            || self
                .engine
                .tibetan_spelling_symbols()
                .as_bytes()
                .contains(&value);
        if !spells {
            if !self.has_composition() {
                self.reset_commit_context();
                return KeyResult::unhandled();
            }
            if value.is_ascii_punctuation() {
                return KeyResult::unhandled();
            }
            return self.commit_tibetan_composition(false);
        }
        // 新音节串前的长时间停顿通常意味着用户换到了别的输入框或应用。
        if !self.has_composition()
            && self.chain.previous.is_some()
            && self.chain.paused(self.steady_now())
        {
            self.chain.reset();
        }
        let key = if value.is_ascii_alphabetic() {
            SchemeKey::Letter(value)
        } else {
            SchemeKey::Symbol(value)
        };
        self.engine.handle_key(key);
        self.update_mixed_candidates();
        self.invalidate_online_requests();
        KeyResult::handled()
    }

    /// 威利读不了的字母（大写锁定或误按 Shift 打出的 `B` `O`，以及 `q` `x`）不进原文：先上屏转换出的藏文（如果有组字），再把这个字母原样跟在后面，按键已处理。由会话自己写出字母，宿主不需要区分哪些字母被接收，拉丁字母也不会混进转换结果。不学习任何东西。
    fn commit_unspelled_tibetan_letter(&mut self, letter: u8) -> KeyResult {
        let mut text = if self.has_composition() {
            self.preedit()
        } else {
            String::new()
        };
        text.push(char::from(letter));
        self.reset_composition();
        self.chain.reset();
        KeyResult::committed(text)
    }

    /// 空格（`mark` 为音节点）或 `/`（`mark` 为垂符）结束藏文音节串：转换出的藏文连同 `mark` 一起上屏，按键已处理；没有组字时只上屏 `mark`。Esc 锁定原文后组字只是拉丁字母：空格只上屏原文并交回宿主插入空格，`/` 上屏原文加 `/`。不学习任何东西。
    fn end_tibetan_syllables(&mut self, mark: char, key: u8) -> KeyResult {
        if self.engine.tibetan_raw_locked() {
            if key == b' ' {
                return self.commit_tibetan_composition(false);
            }
            let mut text = self.preedit();
            text.push(char::from(key));
            self.reset_composition();
            self.chain.reset();
            return KeyResult::committed(text);
        }
        let mut text = self.preedit();
        // 藏文正字法在以 ང 结尾的音节和垂符之间保留音节点（ང་།）。
        if mark == SHAD && text.ends_with('\u{0F44}') {
            text.push(TSHEG);
        }
        text.push(mark);
        self.reset_composition();
        self.chain.reset();
        KeyResult::committed(text)
    }

    /// 藏文音节串按显示上屏，不附加音节点或垂符。`handled` 为 false 时结束它的按键（光标键、Tab、数字）在上屏后仍由宿主处理；回车为 true，只确认组字。不学习任何东西。
    fn commit_tibetan_composition(&mut self, handled: bool) -> KeyResult {
        let text = self.preedit();
        self.reset_composition();
        self.chain.reset();
        KeyResult {
            handled,
            commit: Some(text),
            diagnostic: None,
        }
    }

    /// Zhuyin keys go to the bopomofo editor, which claims its phonetic keys, the tone keys while composing and the Shift punctuation keys; a claimed key is handled and carries whatever it committed (the Shift marks, an auto-shift). An unclaimed key leaves the editor alone when nothing is composing, when it is a selection key (a digit 1-9 or Space) with the list open, so the runtime's page selection picks the row, and when it is punctuation, because the punctuation route commits the conversion ahead of the mark; any other unclaimed key commits the conversion and stays unhandled, so the host inserts the key after the commit.
    fn handle_zhuyin_character(&mut self, value: u8) -> KeyResult {
        // A long pause before a new composition usually means the user moved to another field or application.
        if !self.has_composition()
            && self.chain.previous.is_some()
            && self.chain.paused(self.steady_now())
        {
            self.chain.reset();
        }
        let claimed = self.engine.handle_zhuyin_key(ZhuyinKey::Char(value));
        let committed = self.engine.take_zhuyin_committed();
        self.update_mixed_candidates();
        let claimed = match claimed {
            Ok(claimed) => claimed,
            Err(error) => return KeyResult::handled().with_diagnostic(Some(error.to_string())),
        };
        if claimed {
            self.invalidate_online_requests();
            if !self.has_composition() {
                self.reset_composition();
            }
            if committed.is_empty() {
                return KeyResult::handled();
            }
            self.chain.reset();
            return KeyResult::committed(committed);
        }
        if !self.has_composition() {
            self.reset_commit_context();
            return KeyResult::unhandled();
        }
        let selects =
            self.engine.zhuyin_list_open() && (value == b' ' || (b'1'..=b'9').contains(&value));
        if selects || value.is_ascii_punctuation() {
            return KeyResult::unhandled();
        }
        self.commit_zhuyin_composition()
    }

    /// 打开或关闭注音九键模式。当前在注音里组字时先结束组字（什么都不提交），因为编辑器换模式会丢掉它；其他方案的组字不受影响。
    pub fn set_zhuyin_nine_key(&mut self, enabled: bool) {
        if self.is_zhuyin() && self.has_composition() {
            self.reset_composition();
        }
        self.engine.set_zhuyin_nine_key(enabled);
    }

    /// 注音九键钉读音：钉住时 handled，没有可钉的目标或下标越界时 unhandled，读词库失败时 handled 并附诊断。只在注音自己的规则生效时起作用。
    pub fn choose_zhuyin_spelling(&mut self, index: usize) -> KeyResult {
        if !self.zhuyin_rules_apply() {
            return KeyResult::unhandled();
        }
        let chosen = self.engine.choose_zhuyin_spelling(index);
        self.update_mixed_candidates();
        match chosen {
            Ok(true) => {
                self.invalidate_online_requests();
                KeyResult::handled()
            }
            Ok(false) => KeyResult::unhandled(),
            Err(error) => KeyResult::handled().with_diagnostic(Some(error.to_string())),
        }
    }

    /// 注音九键供用户钉读音的候选读音；注音规则不生效时为空。
    pub fn zhuyin_spellings(&self) -> Vec<String> {
        if !self.zhuyin_rules_apply() {
            return Vec::new();
        }
        self.engine.zhuyin_spellings().to_vec()
    }

    /// The Zhuyin conversion goes to the host and the key that ended it does not: the result is unhandled, so a caret key, a capital or Tab still does its own work after the commit. The pending syllable is dropped and nothing is learned; a composition that converted nothing yet commits nothing.
    fn commit_zhuyin_composition(&mut self) -> KeyResult {
        let text = self.engine.take_zhuyin_text();
        self.reset_composition();
        self.chain.reset();
        KeyResult {
            handled: false,
            commit: (!text.is_empty()).then_some(text),
            diagnostic: None,
        }
    }

    /// The Zhuyin editor's commands. ConvertHanja (command 16) opens the candidate list or closes it; Cancel closes an open list, else clears the composition; Backspace closes an open list, else removes the last key or syllable; CommitRaw commits the converted text; CommitCandidate chooses the first row with the list open and commits the converted text otherwise. The conversion has no caret inside it, so the caret commands commit it and hand the key back. `None` leaves the command (CommitReading, CycleKanaVariant) to the shared paths, which do nothing for Zhuyin.
    fn handle_zhuyin_command(&mut self, command: Command) -> Option<KeyResult> {
        let key = match command {
            Command::ConvertHanja => ZhuyinKey::OpenList,
            Command::Cancel => ZhuyinKey::Escape,
            Command::Backspace => ZhuyinKey::Backspace,
            Command::CommitRaw => ZhuyinKey::Enter,
            Command::CommitCandidate if self.engine.zhuyin_list_open() => {
                return Some(self.select_candidate(0));
            }
            Command::CommitCandidate => ZhuyinKey::Enter,
            Command::MoveLeft
            | Command::MoveRight
            | Command::MoveHome
            | Command::MoveEnd
            | Command::DeleteForward
            | Command::ConversionLeft
            | Command::ConversionRight => return Some(self.commit_zhuyin_composition()),
            Command::CommitReading | Command::CycleKanaVariant => return None,
        };
        let claimed = self.engine.handle_zhuyin_key(key);
        let committed = self.engine.take_zhuyin_committed();
        self.update_mixed_candidates();
        if let Err(error) = claimed {
            return Some(KeyResult::handled().with_diagnostic(Some(error.to_string())));
        }
        self.invalidate_online_requests();
        if !self.has_composition() {
            // Nothing reached the host on Cancel or Backspace, so the chain still ends where it did; only the composition is over.
            self.reset_composition();
            self.chain.same_composition = false;
        }
        // Enter on a pending syllable alone converted nothing, so it commits nothing.
        if key != ZhuyinKey::Enter || committed.is_empty() {
            return Some(KeyResult::handled());
        }
        self.chain.reset();
        Some(KeyResult::committed(committed))
    }

    /// input_session.cpp:294-406.
    pub fn handle_command(&mut self, command: Command) -> KeyResult {
        // 「📅日期」页上只有 CommitCandidate（空格）算选词：选中页里的一条格式上屏；其余命令都先换回原来那一屏再照常处理。
        if command != Command::CommitCandidate {
            self.exit_date_time_menu();
        }
        if let Some(result) = self.helpcode_command(command) {
            return result;
        }
        self.end_uppercase_run();
        if !self.has_composition() {
            // With nothing composed every command is left to the host, which edits the text, moves the caret or inserts a newline, so the chain no longer ends where the host's text does.
            self.chain.reset();
            return KeyResult::unhandled();
        }
        if self.conversion.is_some() {
            if let Some(result) = self.handle_conversion_command(command) {
                return result;
            }
        }
        // 进不了整句改字时，改字的光标键就是字母光标键，各方案照常处理。
        let command = match command {
            Command::ConversionLeft | Command::ConversionRight => {
                if let Some(result) = self.move_conversion(command) {
                    return result;
                }
                if command == Command::ConversionLeft {
                    Command::MoveLeft
                } else {
                    Command::MoveRight
                }
            }
            other => other,
        };
        if self.zhuyin_rules_apply() {
            if let Some(result) = self.handle_zhuyin_command(command) {
                return result;
            }
        }
        if self.korean_rules_apply() {
            if let Some(result) = self.handle_korean_hanja_command(command) {
                return result;
            }
        }
        // A Korean syllable has no caret inside it, and outside the Hanja list nothing to choose: the commit and caret commands all end it and hand the key back. Backspace, Cancel and the Japanese-only variant command take the shared paths below.
        if self.korean_rules_apply()
            && matches!(
                command,
                Command::CommitCandidate
                    | Command::CommitRaw
                    | Command::CommitReading
                    | Command::MoveLeft
                    | Command::MoveRight
                    | Command::MoveHome
                    | Command::MoveEnd
                    | Command::DeleteForward
            )
        {
            return self.commit_korean_composition();
        }
        // A Vietnamese word has no caret inside it and no list: the commit and caret commands end it as displayed and hand the key back. The first Cancel shows the raw keystrokes instead of the transformed word, and the second takes the shared path and discards it. Backspace removes one keystroke through the shared path.
        if self.vietnamese_rules_apply() {
            match command {
                Command::CommitCandidate
                | Command::CommitRaw
                | Command::CommitReading
                | Command::MoveLeft
                | Command::MoveRight
                | Command::MoveHome
                | Command::MoveEnd
                | Command::DeleteForward => return self.commit_vietnamese_composition(),
                Command::Cancel if self.engine.restore_vietnamese_raw() => {
                    self.update_mixed_candidates();
                    return KeyResult::handled();
                }
                _ => {}
            }
        }
        // 藏文音节串内部没有光标，也没有列表：CommitCandidate（空格）带音节点上屏，CommitRaw（回车）只上屏藏文并吞掉按键，其余提交和光标命令按显示上屏并把按键交回宿主。第一次 Cancel 把显示切回威利原文，第二次走共用路径丢弃组字。Backspace 经共用路径删一个原文按键。
        if self.tibetan_rules_apply() {
            match command {
                Command::CommitCandidate => return self.end_tibetan_syllables(TSHEG, b' '),
                Command::CommitRaw => return self.commit_tibetan_composition(true),
                Command::CommitReading
                | Command::MoveLeft
                | Command::MoveRight
                | Command::MoveHome
                | Command::MoveEnd
                | Command::DeleteForward => return self.commit_tibetan_composition(false),
                Command::Cancel if self.engine.restore_tibetan_raw() => {
                    self.update_mixed_candidates();
                    return KeyResult::handled();
                }
                _ => {}
            }
        }
        match command {
            Command::MoveLeft
            | Command::MoveRight
            | Command::MoveHome
            | Command::MoveEnd
            | Command::DeleteForward => self.edit_at_caret(command),
            Command::CycleKanaVariant => {
                if !self.engine.cycle_japanese_kana_variant() {
                    return KeyResult::unhandled();
                }
                self.update_mixed_candidates();
                KeyResult::handled()
            }
            Command::Backspace => self.backspace(),
            Command::CommitCandidate => self.commit(0),
            Command::CommitRaw => self.commit_raw(),
            Command::CommitReading => {
                // Only Japanese has a reading to commit; the other schemes' normalized segmentation is pinyin, not text.
                if self.scheme() != SchemeType::JapaneseRomaji {
                    return KeyResult::unhandled();
                }
                let reading = self.normalized_segmentation();
                if reading.is_empty() {
                    return KeyResult::unhandled();
                }
                self.reset_composition();
                self.chain.reset();
                KeyResult::committed(reading)
            }
            Command::Cancel => {
                // Nothing reached the host, so the chain still ends where it did; only the composition is over.
                self.reset_composition();
                self.chain.same_composition = false;
                KeyResult::handled()
            }
            // Only a Korean syllable converts to Hanja and only a Zhuyin conversion opens its list; the host keeps the key.
            Command::ConvertHanja => KeyResult::unhandled(),
            // 上面已经换成了字母光标命令。
            Command::ConversionLeft | Command::ConversionRight => KeyResult::unhandled(),
        }
    }

    /// `Session::conversion_left_from`。
    pub fn conversion_left_from(&mut self, index: usize) -> KeyResult {
        self.exit_date_time_menu();
        self.helpcode_entry = None;
        if self.conversion.is_none() && self.has_composition() {
            if let Some(result) = self.move_conversion_from(Command::ConversionLeft, index) {
                return result;
            }
            // 高亮的那一行进不了改字时不换成首选去改，按字母光标左移。
            return self.handle_command(Command::MoveLeft);
        }
        self.handle_command(Command::ConversionLeft)
    }

    /// The Hanja list of the composing Korean syllable. The trigger opens it, or closes it when it is open, and is unhandled when the syllable has no Hanja (a lone jamo), so the host keeps the key. With the list open, Cancel and Backspace only close it and leave the syllable composing, and CommitCandidate chooses the first Hanja. `None` leaves the command to the Korean rules.
    fn handle_korean_hanja_command(&mut self, command: Command) -> Option<KeyResult> {
        match command {
            Command::ConvertHanja => {
                if self.close_korean_hanja() {
                    return Some(KeyResult::handled());
                }
                if !self.engine.open_korean_hanja() {
                    return Some(KeyResult::unhandled());
                }
                self.update_mixed_candidates();
                Some(KeyResult::handled())
            }
            Command::Cancel | Command::Backspace if self.close_korean_hanja() => {
                Some(KeyResult::handled())
            }
            Command::CommitCandidate if self.engine.korean_hanja_open() => Some(self.commit(0)),
            _ => None,
        }
    }

    /// Closes an open Korean Hanja list and rebuilds the displayed list from the engine's, which is empty again; false when no list was open. Every path that ends the list without choosing goes through here, so `candidates()` never keeps Hanja rows the engine no longer has.
    pub(super) fn close_korean_hanja(&mut self) -> bool {
        if !self.engine.close_korean_hanja() {
            return false;
        }
        self.update_mixed_candidates();
        true
    }

    /// input_session.cpp:247-258.
    pub fn handle_candidate_key(&mut self, value: u8) -> KeyResult {
        self.end_uppercase_run();
        // A VNI digit spells the composing word; there is no list to select from.
        if self.vietnamese_rules_apply()
            && self
                .engine
                .vietnamese_spelling_symbols()
                .as_bytes()
                .contains(&value)
        {
            return self.handle_character(value, false);
        }
        // A Zhuyin tone digit or phonetic digit spells while the editor claims it; with the list open the editor leaves 1-9 to selection.
        if self.zhuyin_rules_apply()
            && self
                .engine
                .zhuyin_spelling_symbols()
                .as_bytes()
                .contains(&value)
        {
            return self.handle_character(value, false);
        }
        if !self.has_composition() || !(b'1'..=b'9').contains(&value) {
            // The host inserts the key itself, so the next word no longer follows the last one.
            if !self.has_composition() {
                self.reset_commit_context();
            }
            return KeyResult::unhandled();
        }
        self.select_candidate(usize::from(value - b'1'))
    }

    /// 组字接收的符号先回到字符入口；`/` 和 `@` 仅经字符入口开启模式，避免 runtime 结束组字后的空状态被误当成空闲输入。
    pub fn handle_punctuation(&mut self, value: u8) -> KeyResult {
        self.exit_date_time_menu();
        if let Some(result) = self.helpcode_character(value) {
            return result;
        }
        self.end_uppercase_run();
        if self.takes_literal_symbol(value) {
            return self.handle_character(value, false);
        }
        if matches!(
            self.local_mode,
            LocalInputMode::Command | LocalInputMode::Mention
        ) && self.local_preedit.len() == 1
        {
            return self.commit_bare_prefix(value);
        }
        // A host that reports the apostrophe as punctuation still separates a unit from its target (`3jin'g`), or the words of `/fy`, instead of ending the mode.
        if value == b'\'' && self.takes_local_separator() {
            return self.handle_character(value, false);
        }
        // 拆字里微软双拼的韵母 `;` 同样是输入，宿主按组字原文数奇偶时会把它当标点送来。
        if value == b';'
            && self.local_mode == LocalInputMode::Chaizi
            && self.chaizi_takes_symbol(value, self.caret_position())
        {
            return self.handle_character(value, false);
        }
        // A spelling symbol of the active mode is part of the input, not a mark that ends it.
        if self
            .local_mode
            .spelling_symbols()
            .as_bytes()
            .contains(&value)
        {
            return self.handle_character(value, false);
        }
        // 网址触发键（`www` 后的 `.` 等）直接走到这里的调用方（golden、单测）也进入网址模式；真实宿主经 runtime 按 `spelling_symbols` 先送到 `handle_character`。
        if self.url_entry(value).is_some() {
            return self.handle_character(value, false);
        }
        // A Zhuyin phonetic key (`,` `.` `/` `;` `-`) spells, and a Shift punctuation key (`<` `?` `[` ...) commits the conversion with its full-width mark through the editor, whatever the Chinese punctuation switches say.
        if self.zhuyin_rules_apply()
            && (self
                .engine
                .zhuyin_spelling_symbols()
                .as_bytes()
                .contains(&value)
                || zhuyin::layout::SHIFT_PUNCTUATION
                    .iter()
                    .any(|(key, _)| *key == value))
        {
            return self.handle_character(value, false);
        }
        // 藏文的拼写符号和 `/` 是输入，不是结束组字的标点：`/` 由 `handle_character` 变成垂符。
        if self.tibetan_rules_apply()
            && self
                .engine
                .tibetan_spelling_symbols()
                .as_bytes()
                .contains(&value)
        {
            return self.handle_character(value, false);
        }
        // A scheme without Chinese punctuation (Korean) writes half-width ASCII punctuation whatever the Chinese punctuation switches say. With a syllable open the mark follows it in one commit; with nothing open the host inserts the key itself.
        // 专用英文模式同样写半角英文标点：英文模式不往宿主漏中文标点，字符键那条路早就如此（`handle_character` 吞掉非字母键）。只有「固定标点」锁成「始终使用中文标点」（lock 1）时才照中文标点开关走；默认的「跟随中英文状态」（lock 0）在英文模式下就是英文标点。以前这里把专用英文排除在外，于是英文模式下按 `.` 上屏的是「。」。
        let ascii_marks = if self.dedicated_english {
            self.punctuation_lock != 1
        } else {
            !self.engine.current_scheme_type().uses_chinese_punctuation()
        };
        if ascii_marks && self.local_mode == LocalInputMode::None {
            if !self.has_composition() {
                self.reset_commit_context();
                return KeyResult::unhandled();
            }
            let mut result = self.finish_composition(0);
            self.chain.reset();
            result.handled = true;
            result
                .commit
                .get_or_insert_with(String::new)
                .push(char::from(value));
            return result;
        }
        // Lock 1 forces Chinese, which is what an enabled session does anyway; only lock 2 changes anything here (core-session.md §15.8).
        let mark = if self.chinese_punctuation_enabled && self.punctuation_lock != 2 {
            self.punctuation.translate(value)
        } else {
            None
        };
        let Some(mark) = mark else {
            // The host inserts the untranslated key itself, which breaks the run of words just as translated punctuation does.
            self.reset_commit_context();
            return KeyResult::unhandled();
        };
        let mut result = self.finish_composition(0);
        // Whatever follows punctuation starts a new run of words.
        self.chain.reset();
        result.handled = true;
        let mut text = result.commit.take().unwrap_or_default();
        text.push_str(mark);
        result.commit = Some(text);
        result
    }

    /// 数值已经出现后，表达式才收单位小写字母；光标插入与末尾输入共用这条规则。
    pub(super) fn expression_takes_unit_letter(&self, value: u8, caret: usize) -> bool {
        let text = self.local_preedit.as_bytes();
        value.is_ascii_lowercase() && text[..caret.min(text.len())].iter().any(u8::is_ascii_digit)
    }

    /// 数值单位与目标之间的单个撇号只能插在单位字母后，且整段尚无其他撇号。
    pub(super) fn expression_takes_unit_separator(&self, caret: usize) -> bool {
        let text = self.local_preedit.as_bytes();
        let caret = caret.min(text.len());
        text[..caret].last().is_some_and(u8::is_ascii_lowercase) && !text.contains(&b'\'')
    }

    /// 当前光标处的撇号是否分隔本地模式的单位、单词或部件，而非结束组字的标点。
    pub fn takes_local_separator(&self) -> bool {
        match self.local_mode {
            LocalInputMode::Expression => {
                self.expression_takes_unit_separator(self.caret_position())
            }
            LocalInputMode::Command => self.command_takes_word_separator(self.caret_position()),
            LocalInputMode::Chaizi => self.chaizi_takes_symbol(b'\'', self.caret_position()),
            _ => false,
        }
    }

    /// 拆字在预编辑的 `caret` 处收不收这个非字母键。`'` 分隔部件：只跟在部件的字母（或韵母 `;`）后面，不能紧跟入口字母 `C`，也不能和另一个 `'` 相邻。`;` 只在双拼方案用得到这个键时（微软双拼的 ing）收，和普通组字一样只补完一个奇数长度的片段；片段从上一个 `'` 或入口字母之后算起，所以宿主按整段组字原文数奇偶会差一位，要靠 `spelling_symbols` 告诉宿主。
    pub(super) fn chaizi_takes_symbol(&self, value: u8, caret: usize) -> bool {
        let text = self.local_preedit.as_bytes();
        let caret = caret.min(text.len());
        match value {
            b'\'' => {
                caret > 1
                    && (text[caret - 1].is_ascii_alphabetic() || text[caret - 1] == b';')
                    && text.get(caret) != Some(&b'\'')
            }
            b';' => {
                if self.scheme() != SchemeType::Shuangpin || !self.profile.uses_semicolon_key() {
                    return false;
                }
                let start = text[..caret]
                    .iter()
                    .rposition(|&byte| byte == b'\'')
                    .map_or(1, |at| at + 1);
                caret > start && (caret - start) % 2 == 1
            }
            _ => false,
        }
    }

    /// 翻译命令只在光标前已有触发词和单词字母时接受分词撇号。
    pub(super) fn command_takes_word_separator(&self, caret: usize) -> bool {
        self.local_mode == LocalInputMode::Command
            && self
                .local_preedit
                .get(1..caret)
                .is_some_and(crate::local::command::takes_word_separator)
    }

    /// A mark typed on a bare `/` or `@` ends the mode instead of choosing a row: both keys commit as the punctuation they are with the mode off, so `/` `,` still types /， rather than the first command followed by ，.
    fn commit_bare_prefix(&mut self, value: u8) -> KeyResult {
        let prefix = self.local_preedit.as_bytes()[0];
        self.reset_composition();
        self.chain.reset();
        let translate = self.chinese_punctuation_enabled && self.punctuation_lock != 2;
        let keys = [prefix, value];
        let translated = keys.map(|key| {
            if translate {
                self.punctuation.translate(key)
            } else {
                None
            }
        });
        let capacity = keys
            .into_iter()
            .zip(&translated)
            .map(|(key, mark)| mark.map_or_else(|| char::from(key).len_utf8(), str::len))
            .sum();
        let mut text = String::with_capacity(capacity);
        for (key, mark) in keys.into_iter().zip(translated) {
            match mark {
                Some(mark) => text.push_str(mark),
                None => text.push(char::from(key)),
            }
        }
        KeyResult::committed(text)
    }

    /// `SessionSnapshot::spelling_symbols`.
    pub fn spelling_symbols(&self) -> String {
        let mut symbols = self.base_spelling_symbols();
        symbols.extend(self.literal_spelling_symbols());
        if self.shuangpin_helpcode_marker(b'\t').is_some() {
            symbols.push('\t');
        }
        symbols
    }

    fn base_spelling_symbols(&self) -> String {
        if self.dedicated_english {
            return String::new();
        }
        // 拆字的 `'` 和微软双拼的 `;` 都只在光标处收得下时才列出：宿主照这里把它们当字符送进来，不再当标点结束组字，也不让「二三候选」把它们当选词键。
        if self.local_mode == LocalInputMode::Chaizi {
            let caret = self.caret_position();
            return (*b"';")
                .into_iter()
                .filter(|&key| self.chaizi_takes_symbol(key, caret))
                .map(char::from)
                .collect();
        }
        if self.local_mode != LocalInputMode::None {
            return self.local_mode.spelling_symbols().to_owned();
        }
        if self.vietnamese_rules_apply() {
            return self.engine.vietnamese_spelling_symbols().to_owned();
        }
        if self.tibetan_rules_apply() {
            return self.engine.tibetan_spelling_symbols().to_owned();
        }
        if self.zhuyin_rules_apply() {
            return self.engine.zhuyin_spelling_symbols().to_owned();
        }
        // `'` is a Jyutping syllable boundary only inside a composition; idle it is punctuation.
        if self.cantonese_rules_apply() {
            return if self.has_composition() {
                cantonese::SPELLING_SYMBOLS_COMPOSING.to_owned()
            } else {
                String::new()
            };
        }
        // 组字原文是网址触发词时发布触发键，原文能做邮箱的本地部分时发布 `@`，runtime 才会把它当作字符送进来，而不是先结束组字。发布的键必须正是 `url_entry` 接受的键。
        let mut keys = self.url_entry_keys().unwrap_or_default();
        if self.helpcode_trigger_available() {
            keys.push(self.helpcode_trigger_key.unwrap() as char);
        }
        if !keys.is_empty() {
            return keys;
        }
        if self.has_composition() || !self.scheme().opens_table_modes() {
            return String::new();
        }
        (*b"/@")
            .into_iter()
            .filter(|key| self.local_mode_for_symbol(*key).is_some())
            .map(char::from)
            .collect()
    }

    /// Replaces the `/` mode's command table, refreshing the list on screen when that mode is open.
    pub fn set_command_table(&mut self, table: &[CommandTableEntry]) -> Option<String> {
        self.queries.set_command_table(table);
        // 表变了，生效的插件指令可能跟着变：缓存的行不再可信。
        self.plugin_rows = None;
        (self.local_mode == LocalInputMode::Command)
            .then(|| self.update_local_candidates())
            .flatten()
    }

    /// 换上插件指令，清掉缓存的插件行（授权撤销之后旧行不能再插回来）；`/` 模式打开时刷新屏幕上的列表。
    pub fn set_plugin_commands(&mut self, commands: &[PluginCommand]) -> Option<String> {
        self.queries.set_plugin_commands(commands);
        self.plugin_rows = None;
        (self.local_mode == LocalInputMode::Command)
            .then(|| self.update_local_candidates())
            .flatten()
    }

    /// 替换 K 模式的宿主短语表；K 模式打开时刷新屏幕上的列表。
    pub fn set_quick_phrase_table(&mut self, table: &[QuickPhraseEntry]) -> Option<String> {
        self.queries.set_quick_phrase_table(table);
        (self.local_mode == LocalInputMode::QuickPhrase)
            .then(|| self.update_local_candidates())
            .flatten()
    }

    /// 替换本地名单，刷新打开的 `@` 列表或普通组字中的联系人候选。
    pub fn set_mention_entries(&mut self, entries: &[MentionEntry]) -> Option<String> {
        self.queries.set_mentions(entries);
        if self.local_mode == LocalInputMode::Mention {
            self.update_local_candidates()
        } else {
            if self.local_mode == LocalInputMode::None && self.has_composition() {
                self.update_mixed_candidates();
            }
            None
        }
    }

    pub fn set_mention_places(&mut self, enabled: bool) -> Option<String> {
        self.queries.set_mention_places(enabled);
        (self.local_mode == LocalInputMode::Mention)
            .then(|| self.update_local_candidates())
            .flatten()
    }

    pub fn has_composition(&self) -> bool {
        if self.dedicated_english {
            return !self.dedicated_english_preedit.is_empty();
        }
        if self.local_mode != LocalInputMode::None {
            return !self.local_preedit.is_empty();
        }
        !self.engine.preedit().is_empty()
    }

    /// The original scheme during temporary Japanese.
    pub fn scheme(&self) -> SchemeType {
        self.temporary_original_scheme
            .unwrap_or_else(|| self.engine.current_scheme_type())
    }

    pub fn preedit(&self) -> String {
        if self.dedicated_english {
            return self.dedicated_english_preedit.clone();
        }
        if self.local_mode != LocalInputMode::None {
            return self.local_preedit.clone();
        }
        self.engine.preedit().to_owned()
    }

    pub fn raw_segmentation(&self) -> String {
        if self.dedicated_english {
            return self.dedicated_english_preedit.clone();
        }
        if self.local_mode != LocalInputMode::None {
            return self.local_preedit.clone();
        }
        self.engine.request().raw_segmentation.clone()
    }

    pub fn normalized_segmentation(&self) -> String {
        if self.dedicated_english {
            return self.dedicated_english_preedit.clone();
        }
        if self.local_mode != LocalInputMode::None {
            return self.local_preedit.clone();
        }
        self.engine.request().normalized_segmentation.clone()
    }

    /// The displayed list of whichever view is active.
    pub fn candidates(&self) -> &[WordItem] {
        if let Some(edit) = &self.conversion {
            return edit.rows();
        }
        if self.dedicated_english {
            return &self.dedicated_english_candidates;
        }
        if self.local_mode != LocalInputMode::None {
            return &self.local_candidates;
        }
        // 「📅日期」页盖在普通混排列表上：组字不变，这一屏只是换了个视图。
        if let Some(page) = &self.date_time_menu_page {
            return page;
        }
        // Fixed positions are always on through `Session`, so the merged list is the displayed one in the reference too (input_session.cpp:765-771); a caret prefix feeds it as well.
        &self.mixed_candidates
    }

    /// Discards the composition. A scheme whose dictionary cannot be opened is unavailable: the error is returned and the session stays in its scheme with its composition untouched.
    pub fn switch_scheme(&mut self, scheme: SchemeType) -> Result<()> {
        // Opening the dictionary first leaves nothing changed when it fails; the switch after it then cannot fail. Resetting must come before the switch, because leaving temporary Japanese switches back to the scheme it was entered from.
        self.engine.activate(scheme)?;
        self.reset_composition();
        self.chain.reset();
        self.engine.switch_scheme(scheme)?;
        self.update_mixed_candidates();
        Ok(())
    }

    pub fn set_dedicated_english_mode(&mut self, enabled: bool) {
        if self.dedicated_english == enabled {
            return;
        }
        self.reset_composition();
        self.chain.reset();
        self.dedicated_english = enabled;
    }

    /// False for an unsupported schema.
    pub fn set_helpcode_schema(&mut self, schema: &str) -> bool {
        if !is_supported_helpcode_schema(schema) {
            return false;
        }
        // The reference threw for a custom table that went missing after the name check; the public setter reports that as false, and the old keymap stays in force.
        self.install_helpcode_keymap(schema).is_ok()
    }

    /// 换上宿主给的辅助码表（辅助码表插件），替换当前的表。
    pub fn set_helpcode_table(&mut self, table: SharedKeymap) {
        self.install_helpcode_table(table);
    }

    pub fn set_helpcode_enabled(&mut self, enabled: bool) {
        self.helpcode_entry = None;
        self.set_quanpin_helpcode_enabled(enabled);
        self.set_shuangpin_helpcode_enabled(enabled);
    }

    pub fn set_wubi_mixed_pinyin(&mut self, enabled: bool) {
        self.set_wubi_input_options(WubiInputOptions {
            mixed_pinyin: enabled,
            ..self.engine.wubi_input_options()
        });
    }

    pub fn set_personal_context_enabled(&mut self, enabled: bool) {
        if self.personal_context_enabled == enabled {
            return;
        }
        self.personal_context_enabled = enabled;
        if !enabled {
            self.reset_commit_context();
        }
    }

    pub fn set_cloud_candidates_enabled(&mut self, enabled: bool) {
        if self.cloud_candidates_enabled == enabled {
            return;
        }
        self.cloud_candidates_enabled = enabled;
        self.update_mixed_candidates();
    }

    pub fn set_english_online_candidates_enabled(&mut self, enabled: bool) {
        if self.english_online_candidates_enabled == enabled {
            return;
        }
        self.english_online_candidates_enabled = enabled;
        self.invalidate_online_requests();
    }

    /// 输入或模式变化同时撤回英文在线行，不能只让旧查询失效却继续显示它的回答。
    pub(super) fn invalidate_online_requests(&mut self) {
        self.online_requests.invalidate();
        self.english_online_rows.clear();
        self.dedicated_english_candidates
            .retain(|row| !row.source.is_online());
        self.local_candidates.retain(|row| !row.source.is_online());
    }

    pub fn set_rescoring_context(&mut self, context: &str) {
        self.engine.set_rescoring_context(context);
    }

    /// 首页行数只决定词库要不要并入补全词；宿主改每页候选数时不必重建会话，跟着改一下即可。
    pub fn set_first_page_rows(&mut self, rows: FirstPageRows) {
        self.first_page_rows = rows.get();
        self.engine.set_first_page_rows(rows);
    }

    /// 「📅日期」入口打开换页：候选窗换成 `query_date_time` 的整组格式（T 模式同一份），组字不动。查不到格式时不动候选窗。
    pub(super) fn enter_date_time_menu(&mut self, keyword: &str) {
        let items = crate::local::date_time::query_date_time(keyword, &self.local_now());
        if !items.is_empty() {
            self.date_time_menu_page = Some(items);
        }
    }

    /// 「📅日期」页开着时收到的任何非选词动作都先把候选换回原来那一屏（`ExitCandidateSubPage`）。
    pub(super) fn exit_date_time_menu(&mut self) {
        self.date_time_menu_page = None;
    }

    /// `index` 处是「📅日期」入口时给出它记住的关键词。
    pub(super) fn date_time_menu_keyword_at(&self, index: usize) -> Option<String> {
        let item = self.candidates().get(index)?;
        crate::local::date_time::is_date_time_menu_item(item)
            .then(|| crate::local::date_time::date_time_menu_keyword(item).to_owned())
    }

    /// Clears caret, phrase progress, pending sequence, local mode, dedicated preedit and caches; returns from temporary Japanese.
    pub fn reset_composition(&mut self) {
        self.date_time_menu_page = None;
        self.helpcode_entry = None;
        self.caret = None;
        self.conversion = None;
        self.prefix_candidates.clear();
        self.prefix_query_input.clear();
        self.prefix_active = false;
        self.phrase_progress = CreatingWordProgress::default();
        self.pending_sequence = None;
        self.pending_sequence_with_cases = None;
        self.invalidate_online_requests();
        let original_scheme = self.temporary_original_scheme.take();
        self.local_mode = LocalInputMode::None;
        self.local_preedit.clear();
        self.local_candidates.clear();
        self.plugin_rows = None;
        self.dedicated_english_preedit.clear();
        self.dedicated_english_candidates.clear();
        self.mixed_candidates.clear();
        self.personal_reranked = false;
        self.ranking_candidates = None;
        self.engine.reset();
        if let Some(original) = original_scheme {
            if self.engine.current_scheme_type() != original {
                // Temporary Japanese is entered only from a scheme that opens local modes, and none of those reads a language dictionary, so returning to it cannot fail.
                let returned = self.engine.switch_scheme(original);
                debug_assert!(returned.is_ok());
            }
        }
    }

    /// Resets the chain and recomputes when the list was personally reranked.
    pub fn reset_commit_context(&mut self) {
        self.helpcode_entry = None;
        self.end_uppercase_run();
        self.chain.reset();
        // A list reordered for the forgotten context must not stay on screen.
        if self.personal_reranked {
            self.update_mixed_candidates();
        }
    }

    pub fn reset_cache(&mut self) {
        self.engine.reset_cache();
        if let Some(engine) = self.canonical_phrase_engine.as_mut() {
            engine.reset_cache();
        }
        // The visible prefix list stays until the next refresh, but its query must run again against the invalidated provider caches.
        self.prefix_query_input.clear();
    }

    pub fn clear_online_candidates(&mut self, source: CandidateSource) {
        self.engine.clear_online_candidates(source);
        self.online_requests.invalidate();
        self.english_online_rows.retain(|row| row.source != source);
        self.dedicated_english_candidates
            .retain(|row| row.source != source);
        self.local_candidates.retain(|row| row.source != source);
        if !self.dedicated_english && self.local_mode == LocalInputMode::None {
            self.update_mixed_candidates();
        }
    }

    /// input_session_composition.cpp:379-385. While a caret prefix is decoded the prefix list is the one on screen, so it is the one widened; the reference's caret-prefix overlay widened only the hidden whole-input list and reported growth the host could not see.
    pub(super) fn expand_initial_candidates(&mut self) -> bool {
        self.exit_date_time_menu();
        if self.conversion.is_some() {
            return false;
        }
        self.refresh_prefix_candidates();
        let grew = if self.prefix_active {
            self.engine.expand_current_raw_prefix_initial_candidates(
                &self.prefix_query_input,
                self.prefix_end(),
                &mut self.prefix_candidates,
            )
        } else {
            self.engine.expand_initial_candidates()
        };
        if !grew {
            return false;
        }
        // The prefix is unchanged, so the refresh keeps the widened prefix list instead of querying it again.
        self.update_mixed_candidates();
        true
    }

    pub(super) fn steady_now(&self) -> Instant {
        (self.clock.steady)()
    }

    pub(super) fn local_now(&self) -> LocalDateTime {
        (self.clock.local)()
    }

    pub(super) fn shuangpin_profile(&self) -> &'static ShuangpinProfile {
        self.profile
    }

    pub(super) fn journal_path(&self) -> &Path {
        &self.journal_path
    }

    /// The live scheme, which is Japanese during temporary Japanese.
    pub(super) fn is_wubi(&self) -> bool {
        self.engine.current_scheme_type() == SchemeType::Wubi
    }

    pub(super) fn is_shuangpin(&self) -> bool {
        self.engine.current_scheme_type() == SchemeType::Shuangpin
    }

    pub(super) fn is_japanese(&self) -> bool {
        self.engine.current_scheme_type() == SchemeType::JapaneseRomaji
    }

    pub(super) fn is_korean(&self) -> bool {
        self.engine.current_scheme_type() == SchemeType::Korean
    }

    /// The Korean scheme's own rules are in force: dedicated English and the local modes keep theirs inside it.
    pub(super) fn korean_rules_apply(&self) -> bool {
        self.is_korean() && self.local_mode == LocalInputMode::None && !self.dedicated_english
    }

    pub(super) fn is_vietnamese(&self) -> bool {
        self.engine.current_scheme_type() == SchemeType::Vietnamese
    }

    /// The Vietnamese scheme's own rules are in force: dedicated English and the local modes keep theirs inside it.
    pub(super) fn vietnamese_rules_apply(&self) -> bool {
        self.is_vietnamese() && self.local_mode == LocalInputMode::None && !self.dedicated_english
    }

    pub(super) fn is_tibetan(&self) -> bool {
        self.engine.current_scheme_type() == SchemeType::Tibetan
    }

    /// 藏文方案自己的规则生效：专用英文和本地模式在其中仍按各自的规则。
    pub(super) fn tibetan_rules_apply(&self) -> bool {
        self.is_tibetan() && self.local_mode == LocalInputMode::None && !self.dedicated_english
    }

    pub(super) fn is_cantonese(&self) -> bool {
        self.engine.current_scheme_type() == SchemeType::Cantonese
    }

    /// The Cantonese scheme's own rules are in force: dedicated English keeps its own inside it.
    pub(super) fn cantonese_rules_apply(&self) -> bool {
        self.is_cantonese() && self.local_mode == LocalInputMode::None && !self.dedicated_english
    }

    pub(super) fn is_stroke(&self) -> bool {
        self.engine.current_scheme_type() == SchemeType::Stroke
    }

    /// 笔画方案自己的规则生效：专用英文模式和本地模式内按它们自己的规则。
    pub(super) fn stroke_rules_apply(&self) -> bool {
        self.is_stroke() && self.local_mode == LocalInputMode::None && !self.dedicated_english
    }

    pub(super) fn is_zhuyin(&self) -> bool {
        self.engine.current_scheme_type() == SchemeType::Zhuyin
    }

    /// The Zhuyin scheme's own rules are in force: dedicated English and the local modes keep theirs inside it.
    pub(super) fn zhuyin_rules_apply(&self) -> bool {
        self.is_zhuyin() && self.local_mode == LocalInputMode::None && !self.dedicated_english
    }

    /// The scheme's openable candidate list is showing: the composing syllable's Hanja in Korean, the list the user opened over the conversion in Zhuyin.
    pub(super) fn candidate_list_open(&self) -> bool {
        self.engine
            .current_scheme_type()
            .has_openable_candidate_list()
            && ((self.korean_rules_apply() && self.engine.korean_hanja_open())
                || (self.zhuyin_rules_apply() && self.engine.zhuyin_list_open()))
    }

    /// `IsMidSentenceHelpcodeMarkerKey` 与 `IsEndHelpcodeMarkerKey` 的对应物：这个键是不是
    /// 辅助码触发键、记进串里的标记字符是什么。`None` 表示不是触发键，走普通路径。
    /// `;` 先让给 `ing` 韵母（`accepts_semicolon_final_at`），两侧判断顺序一致。
    fn shuangpin_helpcode_marker(&self, value: u8) -> Option<u8> {
        use crate::shuangpin::mid_sentence;
        if !matches!(value, b'`' | b';' | b'/' | b'\t')
            || self.scheme() != SchemeType::Shuangpin
            || !self.has_composition()
            || self.dedicated_english
            || self.local_mode != LocalInputMode::None
        {
            return None;
        }
        let request = self.engine.request();
        let modes = request.shuangpin_helpcode_modes;
        if value == b'\t'
            && (self.caret_position() != self.editing_text_len() || self.conversion.is_some())
        {
            return None;
        }
        let typed = self.raw_with_cases().as_bytes();
        let caret = typed.len();
        // `/` 是万象式直接的结束键（`direct_helpcode_slash`）：原样落进输入串，
        // 路径解析拿它当一位/两位辅码后的段终止符。
        if value == b'/' {
            return (modes.direct_helpcode && modes.direct_helpcode_slash)
                .then_some(mid_sentence::DIRECT_HELPCODE_SLASH);
        }
        let triggers = self.shuangpin_helpcode_uppercase_triggers();
        if value == b';'
            && self.profile.uses_semicolon_key()
            && mid_sentence::accepts_semicolon_final_at(typed, caret, triggers)
        {
            return None;
        }
        // 句中与末尾触发键在写入端已互斥；同键都给时按句中，与 Windows 的判断顺序一致。
        let mid = match value {
            b'`' => modes.mid_sentence_backtick,
            b';' => modes.mid_sentence_semicolon,
            _ => false,
        };
        if modes.mid_sentence
            && !modes.direct_helpcode
            && mid
            && mid_sentence::accepts_marker_at(typed, caret, triggers)
        {
            return Some(mid_sentence::MID_SENTENCE_MARKER);
        }
        let end = match value {
            b'`' => modes.trigger_backtick,
            b';' => modes.trigger_semicolon,
            b'\t' => modes.manual_tab,
            _ => false,
        };
        if request.enable_shuangpin_helpcode
            && !modes.direct
            && !modes.direct_helpcode
            && end
            && mid_sentence::accepts_end_marker_at(typed, caret, triggers)
        {
            return Some(mid_sentence::END_HELPCODE_MARKER);
        }
        None
    }

    /// 大写字母开哪种辅助码段，与 `ImeSession::helpcode_uppercase_triggers` 同一条规则；编辑与
    /// 键入路径用同一份，判断才不会分叉。
    pub(super) fn shuangpin_helpcode_uppercase_triggers(
        &self,
    ) -> crate::shuangpin::mid_sentence::UppercaseTriggers {
        let request = self.engine.request();
        crate::shuangpin::mid_sentence::uppercase_triggers(
            &request.shuangpin_helpcode_modes,
            request.enable_shuangpin_helpcode,
        )
    }

    /// The helpcode switch of the session's scheme; other schemes have none.
    pub(super) fn helpcode_enabled(&self) -> bool {
        match self.scheme() {
            SchemeType::Quanpin => self.quanpin_helpcode_enabled,
            SchemeType::Shuangpin => self.shuangpin_helpcode_enabled,
            SchemeType::Wubi
            | SchemeType::JapaneseRomaji
            | SchemeType::Korean
            | SchemeType::Cantonese
            | SchemeType::Zhuyin
            | SchemeType::Vietnamese
            | SchemeType::Tibetan
            | SchemeType::Stroke => false,
        }
    }

    /// The pinyin context pins and the personal rerank are keyed on.
    pub(super) fn pinyin_ranking_context(&self) -> Cow<'_, str> {
        let request = self.engine.request();
        if request.normalized_segmentation.is_empty() {
            Cow::Owned(request.segmentation.clone())
        } else {
            Cow::Borrowed(&request.normalized_segmentation)
        }
    }

    /// The phrase would be lost in progress when nothing is left to compose: backspacing the remaining pinyin away, or a continuing selection whose leftover collapses. It must not be glued onto the next composition's selections and stored as a phrase nobody typed.
    pub(super) fn discard_abandoned_phrase_progress(&mut self) {
        if !self.has_composition() {
            self.phrase_progress = CreatingWordProgress::default();
            self.chain.same_composition = false;
        }
    }

    fn install_helpcode_keymap(&mut self, schema: &str) -> Result<()> {
        let keymap = Arc::new(load_helpcode_keymap_with_fallback(
            &self.paths.resources,
            schema,
            self.helpcode_fallback_resources.as_deref(),
        )?);
        self.install_helpcode_table(keymap);
        Ok(())
    }

    fn install_helpcode_table(&mut self, keymap: SharedKeymap) {
        self.helpcode_keymap = Some(keymap.clone());
        self.engine.set_helpcode_keymap(Some(keymap));
        self.update_mixed_candidates();
    }

    fn set_autocorrect_types(&mut self, types: u32) {
        // Hosts may re-apply their configuration before every key; an unchanged value must not rebuild the list on that hot path.
        if self.autocorrect_types == types {
            return;
        }
        self.autocorrect_types = types;
        self.engine.set_autocorrect_types(types);
        self.update_mixed_candidates();
    }

    fn set_quanpin_helpcode_enabled(&mut self, enabled: bool) {
        if self.quanpin_helpcode_enabled == enabled {
            return;
        }
        self.quanpin_helpcode_enabled = enabled;
        self.engine.set_quanpin_helpcode_enabled(enabled);
        // The engine reads the flag only while querying, so a live composition has to be asked again.
        self.recompute_candidates();
        self.invalidate_online_requests();
    }

    fn set_shuangpin_helpcode_enabled(&mut self, enabled: bool) {
        if self.shuangpin_helpcode_enabled == enabled {
            return;
        }
        self.shuangpin_helpcode_enabled = enabled;
        self.engine.set_shuangpin_helpcode_enabled(enabled);
        self.recompute_candidates();
        self.invalidate_online_requests();
    }

    /// 双拼辅助码的触发方式（`SessionOptions::shuangpin_helpcode_modes`）。
    fn set_shuangpin_helpcode_modes(&mut self, modes: ShuangpinHelpcodeModes) {
        self.engine.set_shuangpin_helpcode_modes(modes);
        self.recompute_candidates();
        self.online_requests.invalidate();
    }

    fn set_local_mode_options(&mut self, options: LocalModeOptions) {
        self.local_mode_options = options;
        // Unit conversion loads rink's definitions once per process; doing it now, off this thread, keeps that load off the first key that needs it.
        if options.expression {
            crate::local::units::warm_up_in_background();
        }
        if self.local_mode != LocalInputMode::None && !local_mode_enabled(options, self.local_mode)
        {
            self.reset_composition();
        }
    }

    fn set_wubi_input_options(&mut self, options: WubiInputOptions) {
        self.engine.set_wubi_input_options(options);
        // The setting decides which dictionary answers the code in hand, so a live composition has to be asked again; otherwise the fallback rows outlive the switch being turned off.
        if self.is_wubi() && !self.dedicated_english && self.local_mode == LocalInputMode::None {
            self.recompute_candidates();
        }
    }

    fn local_mode_for_entry(&self, value: u8) -> Option<LocalInputMode> {
        let options = self.local_mode_options;
        // 总开关关掉时 Shift+字母一律不进模式，大写字母照常交给宿主输出。
        if !options.shift_letter_entry {
            return None;
        }
        let (mode, enabled) = match value {
            b'U' => (LocalInputMode::Unicode, options.unicode),
            b'T' => (LocalInputMode::DateTime, options.date_time),
            b'K' => (LocalInputMode::QuickPhrase, options.quick_phrase),
            b'E' => (LocalInputMode::Emoji, options.emoji),
            b'M' => (LocalInputMode::Kaomoji, options.kaomoji),
            b'J' => (LocalInputMode::SuperJianpin, options.super_jianpin),
            b'Y' => (LocalInputMode::TemporaryEnglish, options.temporary_english),
            // 临时日文切到日文方案，日文不在会话允许的方案里时这个模式进不去，`R` 照常当字母处理。
            b'R' => (
                LocalInputMode::TemporaryJapanese,
                options.temporary_japanese
                    && self
                        .engine
                        .enabled_schemes()
                        .contains(SchemeType::JapaneseRomaji),
            ),
            b'V' => (LocalInputMode::Expression, options.expression),
            b'C' => (LocalInputMode::Chaizi, options.chaizi),
            b'S' => (LocalInputMode::Symbol, options.symbol),
            _ => return None,
        };
        // 拆字和快捷短语一样在五笔下也能进入：它查的是编进引擎的部件拼音表，不经过当前方案的拼写，五笔用户同样会遇到不会读的字。
        let scheme_opens = if matches!(mode, LocalInputMode::QuickPhrase | LocalInputMode::Chaizi) {
            self.scheme().opens_table_modes()
        } else {
            self.scheme().opens_local_modes()
        };
        (enabled && scheme_opens).then_some(mode)
    }

    /// 没有组字时，`/` 和 `@` 只在支持本地表入口且启用中文标点时打开模式；英文标点下它们是字面字符。
    fn local_mode_for_symbol(&self, value: u8) -> Option<LocalInputMode> {
        if !self.scheme().opens_table_modes()
            || !self.chinese_punctuation_enabled
            || self.punctuation_lock == 2
        {
            return None;
        }
        let options = self.local_mode_options;
        let (mode, enabled) = match value {
            b'/' => (LocalInputMode::Command, options.command),
            b'@' => (LocalInputMode::Mention, options.mention),
            _ => return None,
        };
        enabled.then_some(mode)
    }

    /// 当前组字能否进入网址模式：方案识别网址、不在本地模式或专用英文、有组字、光标在末尾。
    fn url_entry_ready(&self) -> bool {
        !self.dedicated_english
            && self.local_mode == LocalInputMode::None
            && self.scheme().detects_urls()
            && self.has_composition()
            && self
                .caret
                .is_none_or(|caret| caret >= self.editing_text_len())
    }

    /// 当前组字进入网址模式的按键（触发键和邮箱的 `@`）；没有这样的键或条件不满足时为 `None`。
    fn url_entry_keys(&self) -> Option<String> {
        if !self.url_entry_ready() {
            return None;
        }
        Some(url::entry_keys(self.raw_with_cases())).filter(|keys| !keys.is_empty())
    }

    /// 按下 `value` 会进入网址模式时，进入后的预编辑：组字原文加上这个键。
    fn url_entry(&self, value: u8) -> Option<String> {
        if !self.url_entry_ready() {
            return None;
        }
        let raw = self.raw_with_cases();
        let opens = url::opens(raw, value)
            || (self.scheme() == SchemeType::Wubi && url::wubi_continues(raw, value));
        opens.then(|| format!("{raw}{}", char::from(value)))
    }

    /// 丢弃组字（连同引擎里的原文和候选），以 `text` 为预编辑进入网址模式；已在网址模式时换成新的预编辑（选中网址补全）。
    pub(super) fn enter_url_mode(&mut self, text: String) {
        self.reset_composition();
        self.local_mode = LocalInputMode::Url;
        self.local_preedit = text;
        self.chain.reset();
        self.add_local_fallback_candidate();
        self.add_url_completions();
    }

    /// 网址模式在整段网址那一行后面接上补全：邮箱后缀和顶级域名（`url::email_completions`、`url::tld_completions`）。整段网址留在第一行，空格、失焦和结束网址的标点上屏的仍是用户打的网址，补全要明确选中才用。来源记成 `Fallback` 而不是 `Generated`：runtime 的整句读法降位按 `Generated` 认词网格整句，同样宽的补全会被当成整句挪位。
    pub(super) fn add_url_completions(&mut self) {
        let completions = url::email_completions(&self.local_preedit)
            .into_iter()
            .chain(url::tld_completions(&self.local_preedit));
        self.local_candidates.extend(
            completions.map(|text| WordItem::new("", text, 0, CandidateSource::Fallback, "")),
        );
    }

    /// 组字原文是 `ht`、`www` 这类网址开头时，在候选里接上 `https://`、`www.` 等补全（`url::scheme_completions`）：第一行是词典或英文词典里的词（五笔 `www` 的“众”）时排在它后面，否则排在最前面。五笔四码（`http`）不加：runtime 的顶字在第 5 个字母上选第一行，补全领先时 `s` 会接到 `https://` 后面，而 `http` 加 `s` 本来直接进入网址模式。
    pub(super) fn insert_url_scheme_completions(&self, items: &mut Vec<WordItem>) {
        if !self.url_entry_ready() || (self.is_wubi() && self.raw_with_cases().len() >= 4) {
            return;
        }
        let at = usize::from(items.first().is_some_and(|item| {
            item.source.is_dictionary() || item.source == CandidateSource::EnglishDictionary
        }));
        let completions = url::scheme_completions(self.raw_with_cases())
            .map(|text| WordItem::new("", text, 0, CandidateSource::Fallback, ""));
        items.splice(at..at, completions);
    }

    /// 这一行是网址补全：组字里的 `https://` 等，或网址模式里补全了邮箱后缀、顶级域名的行。补全不算回答了按键，重排模型不拿它比较。
    pub(super) fn is_url_completion(&self, item: &WordItem) -> bool {
        if item.source != CandidateSource::Fallback {
            return false;
        }
        match self.local_mode {
            LocalInputMode::None => {
                self.url_entry_ready()
                    && url::scheme_completions(self.raw_with_cases()).any(|text| text == item.word)
            }
            LocalInputMode::Url => item.word != self.local_preedit,
            _ => false,
        }
    }

    /// 选中第 `index` 行时不上屏、以它为预编辑（重新）进入网址模式的行：组字里的 `https://` 等，和网址模式里补全了顶级域名的行（之后还可能接着打路径）。邮箱补全选中即上屏，不在此列。
    pub(super) fn url_continuation(&self, index: usize) -> Option<String> {
        let item = self.candidates().get(index)?;
        let continues = match self.local_mode {
            LocalInputMode::None => self.is_url_completion(item),
            LocalInputMode::Url => {
                item.source == CandidateSource::Fallback
                    && url::tld_completions(&self.local_preedit).contains(&item.word)
            }
            _ => false,
        };
        continues.then(|| item.word.to_string())
    }

    fn enter_local_mode(&mut self, mode: LocalInputMode, letter: u8) -> Option<String> {
        if mode == LocalInputMode::TemporaryJapanese {
            self.temporary_original_scheme = Some(self.scheme());
            // Japanese reads no language dictionary, so the switch cannot fail.
            let switched = self.engine.switch_scheme(SchemeType::JapaneseRomaji);
            debug_assert!(switched.is_ok());
        }
        self.local_mode = mode;
        self.local_preedit = (letter as char).to_string();
        self.local_candidates.clear();
        self.plugin_rows = None;
        self.chain.reset();
        // The command and mention lists are worth showing before a letter narrows them.
        if matches!(mode, LocalInputMode::Command | LocalInputMode::Mention) {
            return self.update_local_candidates();
        }
        self.add_local_fallback_candidate();
        None
    }

    /// The `R` preedit and rows follow the Japanese engine after every edit.
    pub(super) fn refresh_temporary_japanese(&mut self) {
        self.local_preedit.clear();
        self.local_preedit.push('R');
        self.local_preedit.push_str(self.engine.preedit());
        clone_candidate_rows(self.engine.candidates(), &mut self.local_candidates);
        self.add_local_fallback_candidate();
    }

    fn backspace(&mut self) -> KeyResult {
        if self
            .caret
            .is_some_and(|caret| caret < self.editing_text_len())
        {
            return self.edit_at_caret(Command::Backspace);
        }
        self.caret = None;
        if self.dedicated_english {
            self.invalidate_online_requests();
            self.dedicated_english_preedit.pop();
            self.update_dedicated_english_candidates();
            return KeyResult::handled();
        }
        // 网址模式的退格与光标处删除共用一条路径（光标在行末）。
        if self.local_mode == LocalInputMode::Url {
            return self.edit_at_caret(Command::Backspace);
        }
        if self.local_mode != LocalInputMode::None {
            // Backspacing the bare prefix letter leaves the mode.
            if self.local_preedit.len() <= 1 {
                self.reset_composition();
                return KeyResult::handled();
            }
            if self.local_mode == LocalInputMode::TemporaryJapanese {
                self.engine.handle_key(SchemeKey::Backspace);
                self.refresh_temporary_japanese();
                return KeyResult::handled();
            }
            self.invalidate_online_requests();
            self.local_preedit.pop();
            return KeyResult::handled().with_diagnostic(self.update_local_candidates());
        }
        self.engine.handle_key(SchemeKey::Backspace);
        self.update_mixed_candidates();
        self.discard_abandoned_phrase_progress();
        self.invalidate_online_requests();
        KeyResult::handled()
    }

    /// 网址模式删掉末尾的 `removed` 后剩下 `remaining` 时是否退回组字：恰为进入网址模式的逆操作，即组字原文 `remaining` 按下 `removed` 正好会进入网址模式（`www.` 删掉 `.`、五笔 `https` 删掉 `s`、`zhangsan@` 删掉 `@`），误触发后还能选回原来的字（五笔 `www` 的“众”）。`www.example` 删掉中间的 `.` 不满足，留在网址模式。退格和光标处的删除走同一条规则；删的不是末尾那个字符时由调用方排除（`abc@def` 删掉 `@`）。
    pub(super) fn url_reverts(&self, remaining: &str, removed: char) -> bool {
        let Ok(key) = u8::try_from(removed) else {
            return false;
        };
        url::opens(remaining, key)
            || (self.scheme() == SchemeType::Wubi && url::wubi_continues(remaining, key))
    }

    /// 把网址模式剩下的字母重新作为组字原文。方案装不下全部字母时留在网址模式。
    pub(super) fn restore_composition_from_url(&mut self, letters: String) {
        self.reset_composition();
        // 直接借用剩余文本还原方案，保留原文以检测码长截断。
        self.engine.replace_active_raw_input(&letters, &letters);
        self.invalidate_online_requests();
        self.update_mixed_candidates();
        // 方案装不下全部字母（五笔不开混拼时码长 4，`https:` 删掉 `:` 剩 5 个字母）时留在网址模式，不能悄悄丢掉用户键入的字母。
        if self.raw_with_cases() != letters {
            self.enter_url_mode(letters);
        }
    }

    /// 手动辅助码的 Tab 标记与辅码不属于原文；无候选回退和回车使用同一份拼写。
    pub(super) fn raw_commit_text(&self) -> String {
        let request = self.engine.request();
        if self.scheme() == SchemeType::Shuangpin
            && self.local_mode == LocalInputMode::None
            && !self.dedicated_english
            && request.shuangpin_helpcode_modes.manual_tab
            && request.raw_input_with_helpcode_marks.contains('|')
        {
            request.raw_input_with_cases.clone()
        } else {
            self.preedit()
        }
    }

    pub(super) fn commit_raw(&mut self) -> KeyResult {
        // 笔画的预编辑是字形，Enter 上屏的是键入的字母串（与粤拼一致）。
        let mut raw = if self.stroke_rules_apply() {
            self.engine.request().raw_input.clone()
        } else {
            self.raw_commit_text()
        };
        // 回车上屏的是屏幕上这段预编辑原文，本地模式的入口字母也算在内：临时模式以前把入口字母当标记丢掉，空输入时回车什么都不上屏（#7616）。进词库的词仍要去掉标记，见下面学习那一处。
        // 五笔临时拼音反查的引导键 `z` 同样是标记：回车上屏的是键入的拼音。
        if self.local_mode == LocalInputMode::None
            && !self.dedicated_english
            && self.engine.wubi_pinyin_lookup_active()
        {
            raw.remove(0);
        }
        let mut diagnostic = None;
        if self.learn_english_on_enter
            && self.dedicated_english
            && learn_entered_english_word(
                &self.paths.dictionary(assets::ENGLISH_DICTIONARY),
                self.journal_path(),
                self.local_mode.without_entry_marker(&raw),
                ENTERED_ENGLISH_WORD_WEIGHT,
            )
            .is_err()
        {
            diagnostic = Some(diagnostics::ENGLISH_WORD_NOT_LEARNED.to_owned());
        }
        if diagnostic.is_none() {
            diagnostic = self.learn_rejected_correction();
        }
        self.reset_composition();
        self.chain.reset();
        KeyResult::committed(raw).with_diagnostic(diagnostic)
    }

    /// Rows in a mixed list decide where learning goes by the scheme that produced them (overlays.md §3.3).
    pub(super) fn is_wubi_native_candidate(item: &WordItem) -> bool {
        item.scheme == SchemeType::Wubi
    }

    /// Whether a row came from a dictionary a pin, fixed position or removal can write to. Japanese rows come from a read-only model, Korean Hanja rows from the embedded table, Cantonese rows from the read-only `msime-cantonese.db`, Zhuyin rows from the read-only `msime-zhuyin.db` and Stroke rows from the read-only `msime-stroke.db`; keyed by their letters, any of them would land in the pinyin user dictionary.
    pub(super) fn is_editable_source(&self, item: &WordItem) -> bool {
        item.source == CandidateSource::EnglishDictionary
            || (item.source.is_dictionary()
                && !matches!(
                    self.scheme(),
                    SchemeType::JapaneseRomaji
                        | SchemeType::Korean
                        | SchemeType::Cantonese
                        | SchemeType::Zhuyin
                        | SchemeType::Stroke
                ))
    }
}

fn frequency_options_valid(options: FrequencyAdjustmentOptions) -> bool {
    (1..=10).contains(&options.trigger_count) && (1..=10).contains(&options.linear_step)
}

fn english_options_valid(options: EnglishInputOptions) -> bool {
    (1..=8).contains(&options.minimum_prefix) && (1..=3).contains(&options.first_page_count)
}

fn local_mode_enabled(options: LocalModeOptions, mode: LocalInputMode) -> bool {
    match mode {
        LocalInputMode::None => true,
        LocalInputMode::Unicode => options.unicode,
        LocalInputMode::DateTime => options.date_time,
        LocalInputMode::QuickPhrase => options.quick_phrase,
        LocalInputMode::Emoji => options.emoji,
        LocalInputMode::Kaomoji => options.kaomoji,
        LocalInputMode::SuperJianpin => options.super_jianpin,
        LocalInputMode::TemporaryEnglish => options.temporary_english,
        LocalInputMode::TemporaryJapanese => options.temporary_japanese,
        LocalInputMode::Expression => options.expression,
        LocalInputMode::Command => options.command,
        LocalInputMode::Mention => options.mention,
        LocalInputMode::Chaizi => options.chaizi,
        LocalInputMode::Symbol => options.symbol,
        // 网址模式默认开启，没有开关。
        LocalInputMode::Url => true,
    }
}
