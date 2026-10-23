//! The host facade: the surface host-api and input-runtime call (api-contract.md), the one the removed C++ bridge used to expose. It holds the logic that lived in `bridge.cpp` rather than in the engine: the `EngineOptions` mapping and its error strings, the flattened snapshot with the helpcode annotation rule and the nine-key and Microsoft mirrors, the raw-commit learning policy and `CommitRawWithoutLearning`, and the session-free dictionary, gloss, catalog and text helpers.
//!
//! Errors are `EngineError`, whose `Display` is the C++ exception text; callers read it through `to_string()`. Audio capture lives in host-api, and offline handwriting recognition is `crate::handwriting`.

pub mod dictionary;
pub mod dictionary_lookup;
pub mod glosses;
pub mod options;
pub mod session;
#[cfg(test)]
mod tests;
pub mod text;

pub use crate::helpcode::{load_helpcode_keymap, HelpcodeKeymap, SharedKeymap};
pub use crate::local::catalog::{EmojiCatalogItem, EmojiCatalogSlice, EmojiSymbolGroup};
// 自定义双拼方案的表和它的校验：宿主（host-api、MCP）在把 `custom` 交给 Engine 之前先校验，不合法就不要传。编辑中的表（设置页）用不留存的 `check_shuangpin_custom_profile`，内置方案的表是编辑的起点。
pub use crate::shuangpin::custom::{
    builtin_table as shuangpin_builtin_table,
    check_custom_profile as check_shuangpin_custom_profile,
    validate_custom_profile as validate_shuangpin_custom_profile,
    CustomProfileError as ShuangpinCustomProfileError, CustomTablePart as ShuangpinCustomTablePart,
};
pub use crate::shuangpin::hints::ShuangpinKeyHint;
// 九键双拼的表：内置方案、用户表的校验（编辑中的表用不留存的 `check_nine_key_shuangpin_table`）、按方案取表，以及键面提示。宿主拿到的 `NineKeyShuangpin` 经 `KeyGrid::NineKeyShuangpin` 交给会话。
pub use crate::local::command::{
    builtin_titles as builtin_command_titles, builtin_triggers as builtin_command_triggers,
    usable_command_table, usable_plugin_commands,
};
pub use crate::nine_key_shuangpin::{
    builtin_table as nine_key_shuangpin_builtin_table,
    check_table as check_nine_key_shuangpin_table,
    nine_key_shuangpin as validate_nine_key_shuangpin_table, resolve as resolve_nine_key_shuangpin,
    resolve_fourteen_key_shuangpin, table_key_hints as nine_key_shuangpin_table_key_hints,
    NineKeyShuangpinError, NineKeyShuangpinKeyHint,
};
pub use crate::types::{
    CandidateEdge, CommandTableEntry, CommandTranslationQuery, MentionEntry, PluginCommand,
    PluginCommandQuery, QuickPhraseEntry, SentenceAssociationOptions, ShuangpinCustomTable,
};
pub use crate::types::{NineKeyShuangpinKind, NineKeyShuangpinTable};
// Hosts call one of these at shutdown (or before replacing a data directory) so the delayed personal-context writes reach the journal; the bridge had no counterpart because the C++ flushed from `atexit`.
pub use crate::wubi::lookup::{
    WubiCodeLookup, WubiTextCodes, MAX_LOOKUP_CHARACTERS as MAX_WUBI_LOOKUP_CHARACTERS,
};
pub use crate::{close_cached_databases, flush_personal_learning};
pub use dictionary::{
    count_learning_habits, dictionary_edit, dictionary_edit_batch, dictionary_edit_bundled,
    dictionary_edit_collection_batch, dictionary_entries, dictionary_entries_for_kind,
    dictionary_export_entries, dictionary_export_entries_for_kind, dictionary_state_revision,
    dictionary_table_entries, dictionary_validate, dictionary_validate_previous, habit_is_storable,
    merge_dictionary_state, merge_learning_habits, replay_user_dictionary, reset_learned_data,
    stage_dictionary_state, stream_dictionary_state, stream_learning_habits, DictionaryApplied,
    DictionaryEdit, DictionaryEntry, DictionaryKind, DictionaryPage, DictionaryStateMerge,
    DictionaryStateRecord, DictionaryTableEntry, DictionaryTablePage, LearningHabit,
    LearningHabitsMerge, SnapshotReadError,
};
pub use glosses::{
    candidate_glosses, candidate_glosses_with_user, candidate_target_glosses, english_completions,
    english_corrections, english_phonetics, save_candidate_gloss,
};
pub use options::{prepare_options, prepare_options_for, EngineOptions};
pub use session::{
    local_mode_counts_as_typing, Command, EngineResult, EngineSnapshot, OnlineQuerySnapshot,
    Session,
};
pub use text::{
    emoji_catalog_filtered_page, emoji_catalog_groups, emoji_catalog_slice, emoji_symbol_groups,
    handwriting_order_candidates, hanzi_to_pinyin, normalize_full_pinyin, shuangpin_key_hints,
    shuangpin_zero_initials, wubi_code_lookup,
};
