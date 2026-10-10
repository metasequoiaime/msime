//! The input engine: composition state, input schemes, dictionary queries, ranking and learning.
//!
//! This crate replaces the C++ MSIME-Engine that used to be fetched from `engine-lock.json` and patched by overlay scripts. It owns the input algorithms; `msime-input-runtime` keeps orchestrating hosts and does not duplicate any of it.
//!
//! Two public layers: `Session` and the functions beside it mirror the C++ `metasequoia::` API the golden fixtures were recorded against, and `host` is the flattened surface host-api and input-runtime call. The module map and who owns what is `.migration/spec/modules.md`.

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
pub mod language_dictionary;
mod lattice;
mod local;
mod nine_key;
pub mod ordering;
mod paths;
mod pinyin;
mod punctuation;
mod quanpin;
mod session;
mod shuangpin;
pub mod stroke;
mod text;
mod tibetan;
pub mod time;
mod types;
mod user_dictionary;
pub mod vietnamese;
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub mod web;
mod wubi;
pub mod zhuyin;

pub use error::{EngineError, Result};
#[cfg(not(any(target_os = "android", target_env = "ohos")))]
pub use handwriting::handwriting_recognize;
pub use handwriting::order_handwriting_candidates;
pub use paths::RuntimePaths;
pub use pinyin::glide::{GlideKeyboard, GlidePoint};
pub use session::{Clock, Session, SessionOptions, SessionSnapshot};
pub use types::{
    autocorrect_type, fuzzy_rule, request_autocorrect_mask, CandidateEdge, CandidateSource,
    Command, CommandTranslationQuery, EnglishInputOptions, FrequencyAdjustmentMode,
    FrequencyAdjustmentOptions, FuzzyPinyinOptions, KeyResult, LocalInputMode, LocalModeOptions,
    MixedExpressiveOptions, OnlineQuery, PersonalDictionaryEntry, PersonalDictionaryKind,
    SchemeSet, SchemeType, SentenceAssociationOptions, ShuangpinCustomTable, ShuangpinProfileKind,
    WordItem, WubiInputOptions, WubiProfileKind,
};
pub use user_dictionary::bundled::{
    dictionary_table_entries, edit_bundled_dictionary_entry, DictionaryTableEntry,
    DictionaryTablePage,
};
pub use user_dictionary::generation::prepare_runtime_paths;
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

/// Drop every cached journal, personal-context and local-mode connection. Call before deleting or replacing a data directory.
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
