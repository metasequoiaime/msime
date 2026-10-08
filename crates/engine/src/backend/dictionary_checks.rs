//! Read-only checks of a word submission against the shipped dictionaries: whether an entry is already there, and the weight a new entry of a given length should get. The msime-dictionary `check-words` gate (`crates/dict-builder/src/check_words.rs`) runs the same exact-match queries on the same files, so a submission the server accepts is one that gate accepts too.

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::{json, Value};

use super::request::{member, Request};
use super::{BackendError, Outcome};
use crate::format;
use crate::pinyin::segment::split_segments;

pub(super) fn open_read_only(path: &Path) -> Result<Connection, BackendError> {
    let path = crate::paths::sqlite_path_no_follow(path)
        .map_err(|_| BackendError::ResourcesUnavailable)?;
    Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NOFOLLOW
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| BackendError::ResourcesUnavailable)
}

pub(super) fn table_exists(connection: &Connection, table: &str) -> Result<bool, BackendError> {
    Ok(connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// `{"entries":[{"code":"ce'shi","word":"测试"}]}` → `{"listed":[true]}`. Codes are `'`-separated syllables exactly as stored; a code whose table does not exist is simply not listed.
pub(super) fn listed_pinyin_batch(request: &Request, dictionary: &Path) -> Outcome {
    let entries = request.batch("entries")?;
    let connection = open_read_only(dictionary)?;
    let mut listed = Vec::with_capacity(entries.len());
    for entry in entries {
        let code = member(entry, "code")?;
        let word = member(entry, "word")?;
        let table =
            format::build_table_name(&split_segments(code)).ok_or(BackendError::InvalidRequest)?;
        if word.is_empty() {
            return Err(BackendError::InvalidRequest);
        }
        if !table_exists(&connection, &table)? {
            listed.push(false);
            continue;
        }
        // The table name comes from the format contract, never from the request.
        let found = connection
            .query_row(
                &format!("SELECT 1 FROM \"{table}\" WHERE key=?1 AND value=?2 LIMIT 1"),
                [code, word],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        listed.push(found);
    }
    Ok(json!({ "listed": listed }))
}

/// `{"entries":[{"word":"hello","display":"hello"}]}` → `{"listed":[true]}`.
pub(super) fn listed_english_batch(request: &Request, dictionary: &Path) -> Outcome {
    let entries = request.batch("entries")?;
    let connection = open_read_only(dictionary)?;
    // A file without english_words is not an English dictionary.
    let mut statement = connection
        .prepare("SELECT 1 FROM english_words WHERE word=?1 AND display=?2 LIMIT 1")
        .map_err(|_| BackendError::ResourcesUnavailable)?;
    let mut listed = Vec::with_capacity(entries.len());
    for entry in entries {
        let word = member(entry, "word")?;
        let display = member(entry, "display")?;
        if word.is_empty() || display.is_empty() {
            return Err(BackendError::InvalidRequest);
        }
        listed.push(
            statement
                .query_row([word, display], |_| Ok(()))
                .optional()?
                .is_some(),
        );
    }
    Ok(json!({ "listed": listed }))
}

/// The median weight of the shipped quanpin entries for each syllable count, keyed `"1"` to `"7"`, with `"8"` for the overflow tables that hold every entry of eight or more syllables. The lower middle value is taken, so a median is always a weight the dictionary stores.
pub(super) fn pinyin_weight_medians(dictionary: &Path) -> Outcome {
    let connection = open_read_only(dictionary)?;
    let mut medians = BTreeMap::new();
    let mut weights: Vec<i64> = Vec::new();
    for syllables in 1..=format::MAXIMUM_NUMBERED_SYLLABLES + 1 {
        weights.clear();
        for initial in format::SHIPPED_INITIALS.bytes() {
            let Some(table) = format::quanpin_table(syllables, initial) else {
                continue;
            };
            if !table_exists(&connection, &table)? {
                continue;
            }
            let mut statement = connection.prepare(&format!(
                "SELECT weight FROM \"{table}\" WHERE weight IS NOT NULL"
            ))?;
            let rows = statement.query_map([], |row| row.get::<_, i64>(0))?;
            for weight in rows {
                weights.push(weight?);
            }
        }
        if weights.is_empty() {
            continue;
        }
        let middle = (weights.len() - 1) / 2;
        let (_, median, _) = weights.select_nth_unstable(middle);
        medians.insert(syllables.to_string(), Value::from(*median));
    }
    if medians.is_empty() {
        return Err(BackendError::ResourcesUnavailable);
    }
    Ok(json!({ "medians": medians }))
}
