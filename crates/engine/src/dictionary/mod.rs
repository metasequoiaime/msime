//! SQLite access to the working dictionaries: the pinyin tables of `msime-pinyin.db` (quanpin.md §9), `msime-english.db` with its glosses and the custom translations sidecar (schemes-lang.md §4, data-formats.md §5, §10, §11), and the reading lookup `hanzi_to_pinyin` the import path uses. Every statement goes through `prepare_cached`; weights are `i64`.

pub mod english;
#[cfg(test)]
mod fixtures;
pub mod hanzi;
pub mod pinyin;

use rusqlite::types::ValueRef;
use rusqlite::Row;

/// One dictionary row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictRow {
    pub key: String,
    pub value: String,
    pub weight: i64,
}

/// A `LIMIT` bind. The reference bound an `int`, and callers that want everything pass `usize::MAX` where it passed `INT_MAX`.
fn sql_limit(limit: usize) -> i64 {
    limit.min(i32::MAX as usize) as i64
}

/// `sqlite3_column_text` semantics: NULL reads as empty. The shipped tables hold UTF-8, and a stray invalid byte is replaced rather than dropping the row.
fn column_text(row: &Row<'_>, index: usize) -> rusqlite::Result<String> {
    Ok(match row.get_ref(index)? {
        ValueRef::Null => String::new(),
        ValueRef::Text(bytes) | ValueRef::Blob(bytes) => {
            String::from_utf8_lossy(bytes).into_owned()
        }
        ValueRef::Integer(value) => value.to_string(),
        ValueRef::Real(value) => value.to_string(),
    })
}

/// `sqlite3_column_int64` semantics for the weight columns: NULL reads as 0 and a REAL is truncated. Every shipped weight is an INTEGER; this only keeps a hand-edited row from ending the result early.
fn column_i64(row: &Row<'_>, index: usize) -> rusqlite::Result<i64> {
    Ok(match row.get_ref(index)? {
        ValueRef::Integer(value) => value,
        ValueRef::Real(value) => value as i64,
        ValueRef::Null => 0,
        ValueRef::Text(bytes) | ValueRef::Blob(bytes) => std::str::from_utf8(bytes)
            .ok()
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(0),
    })
}
