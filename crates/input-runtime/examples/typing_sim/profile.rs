//! 被测的配置档：手机出厂配置，或者 `convert_eval` 那一套（好和冷启动的数字接上）。
//!
//! 手机出厂配置的每个值都从 `Preferences::default()` 读，再照 host-api 建会话时的映射（`HostSession::apply_pending`、`apply_local_mode_resource_gates`、`ffi::session` 里给 runtime 的设置）换成 `EngineOptions`。映射本身是照抄的，所以 host-api 那边加了新开关这里不会自动跟上；报告头原样列出这里设下的每一项（[`describe`]），配置漂移一眼就能看出来。值是读出来的，出厂默认值改了这里自动跟着变。

use msime_client_core::edition::Edition;
use msime_client_core::preferences::{Preferences, PunctuationLock};
use msime_engine::host::EngineOptions;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// 手机出厂配置：学习开、调频、混输英文和行内日期时间开，不挂模型。
    ProductMobile,
    /// `convert_eval` 的设置：关学习、关调频、关混输和本地模式。
    ConvertEval,
}

impl Profile {
    pub fn name(self) -> &'static str {
        match self {
            Profile::ProductMobile => "product-mobile",
            Profile::ConvertEval => "convert-eval",
        }
    }
}

/// 宿主的每页候选数，host-api 同时交给引擎（`first_page_rows`）和 runtime（分页）。
pub fn page_size(profile: Profile) -> usize {
    match profile {
        Profile::ProductMobile => {
            usize::from(Preferences::default().candidate_page_size).clamp(1, 9)
        }
        Profile::ConvertEval => 9,
    }
}

pub fn apply(profile: Profile, options: &mut EngineOptions, first_page_rows: usize) {
    options.scheme = 0;
    options.first_page_rows = first_page_rows;
    // 宿主总是要全部整句读法，由 runtime 自己裁剪（host-api `apply_pending`）。
    options.sentence_alternatives = true;
    match profile {
        Profile::ProductMobile => product_mobile(options),
        Profile::ConvertEval => convert_eval(options),
    }
}

fn product_mobile(options: &mut EngineOptions) {
    let preferences = Preferences::default();
    let edition = Edition::full();
    options.learning = preferences.learning;
    // 上屏后联想只在声明能显示它的宿主上、而且要用户自己打开（默认关）。
    options.next_word_prediction = false;
    options.learn_english_on_enter = preferences.enter_learns_english_word;
    options.autocorrect_transposition = preferences.quanpin.autocorrect_transposition;
    options.autocorrect_neighbor = preferences.quanpin.autocorrect_neighbor;
    options.fuzzy_pinyin_rules = preferences.fuzzy_pinyin.active_rules();
    options.single_character_only = preferences.single_character_only;
    // 只有三个桌面宿主交给引擎。
    options.caret_edit_suffix = false;
    options.frequency_mode = preferences.frequency.mode.as_str().into();
    options.frequency_trigger_count = preferences.frequency.trigger_count;
    options.frequency_linear_step = preferences.frequency.linear_step;
    options.mixed_candidates = preferences.mixed_input.enabled;
    options.mixed_english = preferences.mixed_input.english;
    // 触屏宿主读用户存的触发字符数（`HostPlatform::reads_mixed_input_minimum_prefix`）。
    options.english_minimum_prefix = preferences.mixed_input.minimum_prefix;
    options.english_first_page_count = preferences.mixed_input.first_page_count;
    options.mixed_emoji = preferences.mixed_input.emoji;
    options.mixed_kaomoji = preferences.mixed_input.kaomoji;
    options.mixed_symbols = preferences.mixed_input.symbols;
    options.mixed_japanese = preferences.mixed_input.japanese;
    // 连续多语言默认关；开着时 host-api 还要看版本和日文模型，这里不建模。
    assert!(
        preferences.multilingual_input.active_languages().len() < 2,
        "the default preferences turned multilingual input on; typing_sim does not model it"
    );
    options.multilingual_languages = Vec::new();
    // 普通候选追加快捷短语只在 macOS 桌面宿主打开。
    options.quick_phrase_candidates = false;
    options.quick_phrase_frequency = preferences.mixed_input.quick_phrase_frequency;
    let local = &preferences.local_modes;
    options.local_unicode = local.unicode;
    options.local_date_time = local.date_time;
    options.local_date_time_inline = local.date_time_inline;
    options.local_date_time_menu = local.date_time_menu;
    options.local_quick_phrase = local.quick_phrase;
    options.local_emoji = local.emoji;
    options.local_kaomoji = local.kaomoji;
    options.local_super_jianpin = local.super_jianpin;
    options.local_temporary_english = local.temporary_english;
    options.local_temporary_japanese = local.temporary_japanese;
    options.local_expression = local.expression;
    options.local_command = local.command;
    options.local_mention = local.mention;
    // 偏好没写时按宿主形态定，触屏宿主是开（host-api `chaizi_enabled`）。
    options.local_chaizi = local.chaizi.unwrap_or(true);
    options.local_symbol = local.symbol;
    options.local_shift_letter_entry = local.shift_letter_entry;
    options.chaizi_traditional_output = preferences.traditional_chinese_output;
    let helpcode = preferences.active_helpcode();
    options.helpcode = helpcode.enabled;
    options.strict_quanpin_helpcode = helpcode.strict_filter;
    options.show_helpcode = helpcode.show_in_candidate_window;
    options.helpcode_schema = helpcode.schema.as_str().into();
    options.helpcode_trigger_key = helpcode.trigger_key.bytes().next();
    let association = &preferences.sentence_association;
    // 出厂没有选语法模型（`GrammarModel::None`），引擎退回无模型基线。
    assert_eq!(
        association.grammar_model,
        msime_client_core::preferences::GrammarModel::None,
        "the default preferences picked a grammar model; typing_sim does not install packs"
    );
    options.sentence_association = msime_engine::SentenceAssociationOptions {
        word_lattice: association.word_lattice,
        // host-api 不再把这个偏好交给引擎，它只决定挂不挂 runtime 重排器（`keyboard_reranker_enabled`，#10197）。
        neural_keyboard: false,
        show_next_on_duplicate: association.show_next_on_duplicate,
        collocation_model: std::path::PathBuf::new(),
        collocation_rerank: false,
        collocation_weight: association.sentence_collocation_weight,
        collocation_grammar: msime_engine::octagram::GramConfig {
            collocation_max_length: association.sentence_collocation_max_length,
            collocation_min_length: association.sentence_collocation_min_length,
            collocation_penalty: association.sentence_collocation_penalty,
            non_collocation_penalty: association.sentence_collocation_non_penalty,
            weak_collocation_penalty: association.sentence_collocation_weak_penalty,
            rear_penalty: association.sentence_collocation_rear_penalty,
        },
    };
    options.paired_punctuation = preferences.paired_punctuation;
    options.punctuation_lock = match preferences.punctuation_lock {
        PunctuationLock::Follow => 0,
        PunctuationLock::Chinese => 1,
        PunctuationLock::English => 2,
    };
    options.chinese_punctuation = preferences.chinese_punctuation || options.punctuation_lock == 1;
    // `apply_local_mode_resource_gates`：资源目录里没有的东西不打开。
    let resources = Path::new(&options.resources);
    let has_emoji_catalog = resources.join("msime-others.db").is_file();
    let has_japanese_model = resources.join("msime-japanese.dat").is_file();
    options.local_emoji &= has_emoji_catalog;
    options.local_kaomoji &= has_emoji_catalog;
    options.local_temporary_english &= resources.join("msime-english.db").is_file();
    options.local_temporary_japanese &= has_japanese_model && edition.features.temporary_japanese;
    options.mixed_japanese &= has_japanese_model && edition.features.temporary_japanese;
    options.sentence_association.neural_keyboard &= edition.features.neural_keyboard;
}

/// `convert_eval::open_runtime` 的设置，停在 2026-10-29 改口径之前：那次 `convert_eval` 的默认每页候选数改成共享偏好的出厂值，并关掉了行内日期时间等其余本地模式；这里仍是每页 9 个（见 [`page_size`]），行内日期时间由虚拟时钟钉住，结果不随运行日期变化。
fn convert_eval(options: &mut EngineOptions) {
    options.learning = false;
    options.frequency_mode = "disabled".into();
    options.autocorrect_transposition = false;
    options.autocorrect_neighbor = false;
    options.fuzzy_pinyin_rules = 0;
    options.helpcode = false;
    options.show_helpcode = false;
    options.mixed_english = false;
    options.mixed_emoji = false;
    options.mixed_kaomoji = false;
    options.mixed_symbols = false;
    options.local_unicode = false;
    options.local_date_time = false;
    options.local_quick_phrase = false;
    options.local_emoji = false;
    options.local_kaomoji = false;
    options.local_super_jianpin = false;
    options.local_temporary_english = false;
    options.local_temporary_japanese = false;
    options.sentence_association.neural_keyboard = false;
}

/// 报告头里列出的配置：路径以外的每一个会影响候选的开关。
pub fn describe(options: &EngineOptions) -> serde_json::Value {
    let association = &options.sentence_association;
    serde_json::json!({
        "scheme": options.scheme,
        "learning": options.learning,
        "next_word_prediction": options.next_word_prediction,
        "frequency": [options.frequency_mode, options.frequency_trigger_count, options.frequency_linear_step],
        "autocorrect": [options.autocorrect_transposition, options.autocorrect_neighbor],
        "fuzzy_pinyin_rules": options.fuzzy_pinyin_rules,
        "single_character_only": options.single_character_only,
        "helpcode": [options.helpcode, options.strict_quanpin_helpcode, options.show_helpcode],
        "helpcode_schema": options.helpcode_schema,
        "mixed": {
            "candidates": options.mixed_candidates,
            "english": options.mixed_english,
            "english_minimum_prefix": options.english_minimum_prefix,
            "english_first_page_count": options.english_first_page_count,
            "emoji": options.mixed_emoji,
            "kaomoji": options.mixed_kaomoji,
            "symbols": options.mixed_symbols,
            "japanese": options.mixed_japanese,
        },
        "local": {
            "unicode": options.local_unicode,
            "date_time": options.local_date_time,
            "date_time_inline": options.local_date_time_inline,
            "quick_phrase": options.local_quick_phrase,
            "emoji": options.local_emoji,
            "kaomoji": options.local_kaomoji,
            "super_jianpin": options.local_super_jianpin,
            "temporary_english": options.local_temporary_english,
            "temporary_japanese": options.local_temporary_japanese,
            "chaizi": options.local_chaizi,
            "symbol": options.local_symbol,
            "shift_letter_entry": options.local_shift_letter_entry,
        },
        "sentence_association": {
            "word_lattice": association.word_lattice,
            "neural_keyboard": association.neural_keyboard,
            "show_next_on_duplicate": association.show_next_on_duplicate,
            "grammar_model": association.collocation_model.display().to_string(),
            "collocation_rerank": association.collocation_rerank,
        },
        "sentence_alternatives": options.sentence_alternatives,
        "first_page_rows": options.first_page_rows,
        "multilingual_languages": options.multilingual_languages.len(),
        "quick_phrase_candidates": options.quick_phrase_candidates,
    })
}
