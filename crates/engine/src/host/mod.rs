//! The host facade: the surface host-api and input-runtime call (api-contract.md), the one the removed C++ bridge used to expose. It holds the logic that lived in `bridge.cpp` rather than in the engine: the `EngineOptions` mapping and its error strings, the flattened snapshot with the helpcode annotation rule and the nine-key and Microsoft mirrors, the raw-commit learning policy and `CommitRawWithoutLearning`, and the session-free dictionary, gloss, catalog and text helpers.
//!
//! Errors are `EngineError`, whose `Display` is the C++ exception text; callers read it through `to_string()`. Audio capture lives in host-api, and offline handwriting recognition is `crate::handwriting`.

pub mod dictionary;
pub mod glosses;
pub mod options;
pub mod session;
#[cfg(test)]
mod tests;
pub mod text;

pub use crate::helpcode::{load_helpcode_keymap, HelpcodeKeymap, SharedKeymap};
pub use crate::local::catalog::{EmojiCatalogItem, EmojiCatalogSlice, EmojiSymbolGroup};
pub use crate::shuangpin::hints::ShuangpinKeyHint;
pub use crate::types::{
    CandidateEdge, CommandTableEntry, CommandTranslationQuery, MentionEntry, QuickPhraseEntry,
    SentenceAssociationOptions,
};
// Hosts call one of these at shutdown (or before replacing a data directory) so the delayed personal-context writes reach the journal; the bridge had no counterpart because the C++ flushed from `atexit`.
pub use crate::{close_cached_databases, flush_personal_learning};
pub use dictionary::{
    dictionary_edit, dictionary_edit_bundled, dictionary_entries, dictionary_export_entries,
    dictionary_state_revision, dictionary_table_entries, dictionary_validate,
    dictionary_validate_previous, replay_user_dictionary, reset_learned_data,
    stage_dictionary_state, DictionaryEntry, DictionaryKind, DictionaryPage, DictionaryStateRecord,
    DictionaryTableEntry, DictionaryTablePage, SnapshotReadError,
};
pub use glosses::{
    candidate_glosses, candidate_glosses_with_user, candidate_target_glosses, english_completions,
    save_candidate_gloss,
};
pub use options::{prepare_options, EngineOptions};
pub use session::{
    local_mode_counts_as_typing, Command, EngineResult, EngineSnapshot, OnlineQuerySnapshot,
    Session,
};
pub use text::{
    emoji_catalog_filtered_page, emoji_catalog_groups, emoji_catalog_slice, emoji_symbol_groups,
    handwriting_order_candidates, hanzi_to_pinyin, normalize_full_pinyin, shuangpin_key_hints,
    shuangpin_zero_initials,
};
