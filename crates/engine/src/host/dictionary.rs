//! Session-free dictionary calls on `EngineOptions` paths (api-contract §1b), and the snapshot record stream.

use std::path::Path;

use super::options::{runtime_paths, EngineOptions};
use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::types::{PersonalDictionaryEntry, PersonalDictionaryKind};
use crate::user_dictionary::journal::release_thread_journal;
use crate::user_dictionary::{bundled, personal, replay, reset, state};

pub type DictionaryKind = PersonalDictionaryKind;
pub type DictionaryEntry = PersonalDictionaryEntry;
pub use crate::user_dictionary::bundled::{DictionaryTableEntry, DictionaryTablePage};
pub use crate::user_dictionary::personal::PersonalDictionaryPage as DictionaryPage;
pub use crate::user_dictionary::state::DictionaryStateRecord;

/// A sanitised transport failure (cancellation, truncation, bad checksum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("Snapshot record stream failed")]
pub struct SnapshotReadError;

/// 代次里有没有 `msime.db`：会话允许的方案里有读它的方案时才有（`SchemeSet::reads_main_dictionary`）。没有它的版本，个人词库只收英文词。
fn main_dictionary(options: &EngineOptions) -> bool {
    options.enabled_schemes.reads_main_dictionary()
}

/// These calls come from pool threads (a Tauri blocking task, the MCP server) that may never touch the journal again, so each one closes its thread's cached connection before returning, on success and failure alike.
fn released<T>(value: T) -> T {
    release_thread_journal();
    value
}

pub fn dictionary_entries(
    options: &EngineOptions,
    offset: usize,
    limit: usize,
) -> Result<DictionaryPage> {
    released(personal::personal_dictionary_entries_with(
        &runtime_paths(options),
        main_dictionary(options),
        offset,
        limit,
        false,
    ))
}

pub fn dictionary_export_entries(
    options: &EngineOptions,
    offset: usize,
    limit: usize,
    include_learned_pinyin: bool,
) -> Result<DictionaryPage> {
    released(personal::personal_dictionary_entries_with(
        &runtime_paths(options),
        main_dictionary(options),
        offset,
        limit,
        include_learned_pinyin,
    ))
}

pub fn dictionary_table_entries(
    options: &EngineOptions,
    kind: DictionaryKind,
    query: &str,
    offset: usize,
    limit: usize,
) -> Result<DictionaryTablePage> {
    released(bundled::dictionary_table_entries(
        &runtime_paths(options),
        kind,
        query,
        offset,
        limit,
    ))
}

pub fn dictionary_edit_bundled(
    options: &EngineOptions,
    previous: &DictionaryEntry,
    weight: Option<i64>,
    request_id: &str,
) -> Result<()> {
    released(bundled::edit_bundled_dictionary_entry_with(
        &runtime_paths(options),
        main_dictionary(options),
        previous,
        weight,
        request_id,
    ))
}

pub fn dictionary_validate(entry: &DictionaryEntry) -> Result<DictionaryEntry> {
    personal::validate_personal_dictionary_entry(entry)
}

/// `dictionary_validate` for the row an edit or removal starts from: a listed row keeps whatever weight learning gave it, so only the floor applies to its weight.
pub fn dictionary_validate_previous(entry: &DictionaryEntry) -> Result<DictionaryEntry> {
    personal::validate_existing_entry(entry)
}

pub fn dictionary_edit(
    options: &EngineOptions,
    previous: Option<&DictionaryEntry>,
    replacement: Option<&DictionaryEntry>,
    request_id: &str,
) -> Result<()> {
    released(personal::edit_personal_dictionary_with(
        &runtime_paths(options),
        main_dictionary(options),
        previous,
        replacement,
        request_id,
    ))
}

/// Refuses to reset the packaged bundle in place and needs both packaged dictionaries. 方案集合不读 `msime.db` 时只需要、也只换回 `english.db`。
pub fn reset_learned_data(options: &EngineOptions) -> Result<()> {
    released(reset::reset_learned_data_with(
        &runtime_paths(options),
        main_dictionary(options),
    ))
}

/// `(applied, skipped, failed, error)`; never errors.
pub fn replay_user_dictionary(
    user_db_path: &str,
    main_db_path: &str,
    english_db_path: &str,
) -> (i32, i32, i32, String) {
    let result = released(replay::replay(
        Path::new(user_db_path),
        Path::new(main_db_path),
        Path::new(english_db_path),
    ));
    (result.applied, result.skipped, result.failed, result.error)
}

/// `INVALID_SNAPSHOT_RECORD_LIMIT` for a zero limit; returns `options` with the four paths of the staged generation.
pub fn stage_dictionary_state(
    options: &EngineOptions,
    generation: &str,
    content_id: &str,
    maximum_records: usize,
    records: impl Iterator<Item = std::result::Result<DictionaryStateRecord, SnapshotReadError>>,
) -> Result<EngineOptions> {
    if maximum_records == 0 {
        return Err(EngineError::invalid(
            diagnostics::INVALID_SNAPSHOT_RECORD_LIMIT,
        ));
    }
    // A transport failure reaches the stager as an error, never as the end of the stream: only verified EOF may finish a generation (bridge.cpp:473-476).
    let mut records = records
        .map(|record| record.map_err(|_| EngineError::failed(diagnostics::SNAPSHOT_STREAM_FAILED)));
    let paths = released(state::stage_dictionary_state_for(
        Path::new(&options.resources),
        Path::new(generation),
        content_id,
        &mut records,
        maximum_records,
        options.enabled_schemes,
    ))?;
    let text = |path: &Path| path.to_string_lossy().into_owned();
    Ok(EngineOptions {
        resources: text(&paths.resources),
        user_data: text(&paths.user_data),
        cache: text(&paths.cache),
        dictionaries: text(&paths.dictionaries),
        ..options.clone()
    })
}

pub fn dictionary_state_revision(options: &EngineOptions) -> Result<String> {
    released(state::dictionary_state_revision(&runtime_paths(options)))
}
