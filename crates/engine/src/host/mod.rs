//! The host facade: product glue of 水杉输入法 itself, the surface host-api and input-runtime call (api-contract.md), the one the removed C++ bridge used to expose. It holds the logic that lived in `bridge.cpp` rather than in the engine: the `EngineOptions` mapping with the product defaults and the translation sidecar, the flattened snapshot with the helpcode annotation rule and the nine-key and Microsoft mirrors, the host command numbering (the raw-commit learning policy and `CommitRawWithoutLearning` are core options and commands; the facade only turns the policy on), and the session-free dictionary management, sync, gloss, catalog, `/`-mode table and text helpers the settings pages and providers use.
//!
//! Everything here is product glue: it exists for msime's own hosts, is not part of the documented library API (the core layer at the crate root) and may change with them. A type or function the core layer itself takes or returns, or that a third-party host needs to use a core option (the helpcode table, the `/`, `@` and K mode tables, validating and resolving custom shuangpin and nine-key shuangpin tables), lives at the crate root instead, and msime's hosts import it from there too. The settings-page helpers for editing those tables stay here.
//!
//! `host::Session` is a thin wrapper over the core [`crate::Session`]: the core owns the candidate revision, teardown, the key grid, the helpcode table and the shuangpin profile, and the wrapper only adds the product's contract on top. Errors are `EngineError`, whose `Display` is the C++ exception text; callers read it through `to_string()`. Audio capture lives in host-api, and offline handwriting recognition is `crate::handwriting`.

pub mod dictionary;
pub mod dictionary_lookup;
pub mod glosses;
pub mod options;
pub mod session;
#[cfg(test)]
mod tests;
pub mod text;

pub use crate::local::catalog::{EmojiCatalogItem, EmojiCatalogSlice, EmojiSymbolGroup};
pub use crate::local::command::{
    builtin_titles as builtin_command_titles, builtin_triggers as builtin_command_triggers,
    usable_command_table, usable_plugin_commands,
};
pub use crate::shuangpin::hints::ShuangpinKeyHint;
// 设置页编辑自定义双拼表和九键双拼表用的辅助函数：内置方案写成表的样子作编辑的起点，编辑中的表用不留存的校验，预览编辑中的九键表的键面。第三方宿主把表交给会话只需要根上的校验和取表。
pub use crate::nine_key_shuangpin::{
    builtin_table as nine_key_shuangpin_builtin_table,
    check_table as check_nine_key_shuangpin_table,
    table_key_hints as nine_key_shuangpin_table_key_hints,
};
pub use crate::shuangpin::custom::{
    builtin_table as shuangpin_builtin_table,
    check_custom_profile as check_shuangpin_custom_profile,
};
pub use crate::wubi::lookup::{
    WubiCodeLookup, WubiTextCodes, MAX_LOOKUP_CHARACTERS as MAX_WUBI_LOOKUP_CHARACTERS,
};
pub use dictionary::{
    add_cantonese_words, cached_dictionary_state_revision, count_learning_habits, dictionary_edit,
    dictionary_edit_batch, dictionary_edit_bundled, dictionary_edit_collection_batch,
    dictionary_entries, dictionary_entries_for_kind, dictionary_entries_source,
    dictionary_export_entries, dictionary_export_entries_for_kind, dictionary_state_revision,
    dictionary_table_entries, dictionary_validate, dictionary_validate_previous, habit_is_storable,
    merge_dictionary_state, merge_learning_habits, replay_user_dictionary, reset_learned_data,
    stage_dictionary_state, stream_dictionary_state, stream_learning_habits, DictionaryApplied,
    DictionaryEdit, DictionaryEntry, DictionaryKind, DictionaryPage, DictionaryStateMerge,
    LearningHabit, LearningHabitsMerge, SnapshotReadError,
};
pub use glosses::{
    candidate_glosses, candidate_glosses_with_user, candidate_target_glosses, english_completions,
    english_correction_display, english_corrections, english_phonetics, phonetics,
    release_gloss_caches, save_candidate_gloss, PHONETIC_LANGUAGES,
};
pub use options::{prepare_options, prepare_options_for, EngineOptions};
pub use session::{
    local_mode_counts_as_typing, Command, EngineResult, EngineSnapshot, OnlineQuerySnapshot,
    Session,
};
pub use text::{
    emoji_catalog_filtered_page, emoji_catalog_groups, emoji_catalog_slice, emoji_symbol_groups,
    encode_shuangpin, hanzi_to_pinyin, hanzi_to_pinyin_batch, is_shuangpin_syllable,
    main_dictionary_path, normalize_full_pinyin, shuangpin_key_hints, shuangpin_uses_semicolon,
    shuangpin_zero_initials, wubi_code_lookup,
};
