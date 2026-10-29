//! The input engine: composition state, input schemes, dictionary queries, ranking and learning.
//!
//! This crate replaces the C++ MSIME-Engine that used to be fetched from `engine-lock.json` and patched by overlay scripts. It owns the input algorithms; `msime-input-runtime` keeps orchestrating hosts and does not duplicate any of it.
//!
//! Two public layers: `Session` and the functions beside it mirror the C++ `metasequoia::` API the golden fixtures were recorded against, and `host` is the flattened surface host-api and input-runtime call. The module map and who owns what is `.migration/spec/modules.md`.
//!
//! 引擎在运行时读写的每个文件（词库、模型、学习日志、代次目录）的格式、版本检查、产出方与数据许可证见仓库里的 `docs/data-formats.md`；代码注释里的 `data-formats.md §N` 指它的章节。

pub mod assets;
pub mod backend;
mod cache;
pub mod cantonese;
pub mod diagnostics;
mod dictionary;
mod error;
pub mod format;
mod handwriting;
mod helpcode;
pub mod host;
mod ime;
mod japanese;
mod korean;
pub use korean::KoreanLayout;
pub mod language_dictionary;
mod lattice;
mod local;
pub mod multilingual;
mod nine_key;
mod nine_key_shuangpin;
pub mod ordering;
mod paths;
mod pinyin;
mod punctuation;
mod quanpin;
mod session;
mod shared_string;
mod shuangpin;
pub mod stroke;
mod text;
mod tibetan;
pub mod time;
pub mod touch;
mod types;
mod user_dictionary;
pub mod vietnamese;
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub mod web;
mod wubi;
pub mod zhuyin;

pub use error::{EngineError, Result};
#[cfg(not(target_os = "android"))]
pub use handwriting::handwriting_recognize;
pub use handwriting::order_handwriting_candidates;
// 按需下载的 `.gram` 语法模型由宿主侧资源包管理，删除包时需要释放
// `GramDb::shared` 缓存的内存映射，经这个窄出口暴露给 host-api。
pub use lattice::octagram;
pub use nine_key::KeyGrid;
pub use nine_key_shuangpin::NineKeyShuangpin;
pub use paths::RuntimePaths;
pub use pinyin::glide::{GlideKeyboard, GlidePoint};
pub use punctuation::{BracketMarks, PunctuationMarks, QuoteMarks};
pub use session::{Clock, Session, SessionOptions, SessionSnapshot};
pub use types::{
    autocorrect_type, fuzzy_rule, request_autocorrect_mask, CandidateEdge, CandidateSource,
    Command, CommandTranslationQuery, EnglishInputOptions, FrequencyAdjustmentMode,
    FrequencyAdjustmentOptions, FuzzyPinyinOptions, KeyResult, LocalInputMode, LocalModeOptions,
    MixedExpressiveOptions, NineKeyShuangpinKind, NineKeyShuangpinTable, OnlineInputMode,
    OnlineQuery, PersonalDictionaryEntry, PersonalDictionaryKind, PluginCommand,
    PluginCommandQuery, SchemeSet, SchemeType, SentenceAssociationOptions, ShuangpinCustomTable,
    ShuangpinHelpcodeModes, ShuangpinProfileKind, SyllableHelpcode, WordItem, WubiInputOptions,
    WubiProfileKind, DEFAULT_FIRST_PAGE_ROWS,
};
pub use user_dictionary::bundled::{
    dictionary_table_entries, edit_bundled_dictionary_entry, DictionaryTableEntry,
    DictionaryTablePage,
};
pub use user_dictionary::generation::prepare_runtime_paths;
pub use user_dictionary::journal::quarantine_corrupt_journal;
pub use user_dictionary::personal::{
    edit_personal_dictionary, personal_dictionary_entries, validate_personal_dictionary_entry,
    PersonalDictionaryPage,
};
pub use user_dictionary::replay::{replay as replay_user_dictionary, ReplayResult};
pub use user_dictionary::reset::reset_learned_data;
pub use user_dictionary::state::{
    dictionary_state_revision, stage_dictionary_state, stream_dictionary_state,
    DictionaryStateRecord, DEFAULT_MAXIMUM_RECORDS,
};
pub use wubi::code::phrase_code as wubi_phrase_code;

pub use local::command::{valid_plugin_argument, EXTENDED_ARGUMENT_SYMBOLS};
pub use local::date_time::LocalDateTime;
pub use local::url;

/// Create or migrate an `msime-english.db` to the schema the engine reads. Fixtures without one need it; `prepare_runtime_paths` copies both dictionaries.
pub fn ensure_english_schema(path: &std::path::Path) -> Result<()> {
    dictionary::english::ensure_english_schema(path)
}

/// Write every queued personal-context transition now. Those writes are otherwise delayed about 2 s; call this before reading the journal directly or re-preparing a generation.
pub fn flush_personal_learning() {
    user_dictionary::ngram_store::flush_all()
}

/// Write every queued personal-context transition and stop the background thread that writes them, waiting for it to end. Call before unloading the engine or exiting the process; sessions still work afterwards, and the next transition they record starts the thread again.
pub fn shutdown() {
    user_dictionary::ngram_store::shutdown()
}

/// Drop every cached journal, personal-context and local-mode connection, on every thread (a connection an operation is using closes when that operation ends). Call before deleting or replacing a data directory.
pub fn close_cached_databases() {
    user_dictionary::journal::close_cached_journals()
}

/// Hands the engine the bytes of `msime-japanese.dat` for `path`, the location a Japanese session reads its model from (`RuntimePaths::resource(assets::JAPANESE_MODEL)` or `SessionOptions::japanese_dictionary`), so a host without a file system (the browser) can type Japanese. False, with nothing changed, when the bytes are not a valid model; a later call for the same path replaces the earlier one.
pub fn preload_japanese_dictionary(path: &std::path::Path, bytes: Box<[u8]>) -> bool {
    japanese::decoder::JapaneseDictionary::preload(path, bytes)
}

/// Forgets the model [`preload_japanese_dictionary`] put at `path`; sessions already using it keep it until they end.
pub fn unload_japanese_dictionary(path: &std::path::Path) {
    japanese::decoder::JapaneseDictionary::unload(path);
}

/// Releases the process-level Japanese model cache while sessions that still use a model keep it
/// alive. Mobile hosts call this from their memory-pressure callback when there is no session
/// handle to pass to `Session::reset_cache`.
pub fn release_japanese_dictionary_cache() {
    japanese::decoder::JapaneseDictionary::release_shared();
}

/// 放掉进程里为下一个会话留着、没有会话在用的整句联想 n-gram store（连同它的模型和日志连接）。正在用的会话不受影响；之后第一个会话重新读日志。和 [`release_japanese_dictionary_cache`] 一样由宿主的清缓存、内存告警动作调用。
pub fn release_idle_personal_models() {
    user_dictionary::ngram_store::release_idle();
}
