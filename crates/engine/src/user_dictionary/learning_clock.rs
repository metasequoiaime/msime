//! 本机学习写入的逻辑时间；同步合并仍原样保留远端时间。

use rusqlite::{params, Connection, OptionalExtension};

use super::learning_sync::MAX_LEARNING_SYNC_TIMESTAMP;
use crate::types::PersonalDictionaryKind;

/// 调用方已持有日志的 IMMEDIATE 事务；仅扫描这次实际要改写的身份。
pub(crate) fn ranking_timestamp<'a>(
    connection: &Connection,
    kind: PersonalDictionaryKind,
    key: &str,
    values: impl IntoIterator<Item = &'a str>,
) -> rusqlite::Result<i64> {
    let ceiling = MAX_LEARNING_SYNC_TIMESTAMP as i64;
    let mut timestamp =
        connection.query_row("SELECT min(max(unixepoch(),1),?1)", [ceiling], |row| {
            row.get::<_, i64>(0)
        })?;
    let mut existing = connection.prepare_cached(
        "SELECT updated_at FROM user_dictionary_operations WHERE dictionary=?1 AND key=?2 AND value=?3",
    )?;
    for value in values {
        let previous: Option<i64> = existing
            .query_row(params![kind.journal_name(), key, value], |row| row.get(0))
            .optional()?;
        if let Some(previous) = previous {
            timestamp = timestamp.max(previous.saturating_add(1).min(ceiling));
        }
    }
    Ok(timestamp)
}

/// 同一次调频的阶梯行和选中行共用预先计算的时间，不在逐行写入时再递增。
pub(crate) fn write_weight_at(
    connection: &Connection,
    kind: PersonalDictionaryKind,
    key: &str,
    value: &str,
    weight: i64,
    timestamp: i64,
) -> rusqlite::Result<()> {
    connection.prepare_cached(
        "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,updated_at) \
         VALUES(?1,?2,?3,'upsert',?4,'',?5) ON CONFLICT(dictionary,key,value) \
         DO UPDATE SET operation='upsert',weight=excluded.weight,display='',updated_at=excluded.updated_at",
    )?.execute(params![kind.journal_name(), key, value, weight, timestamp])?;
    Ok(())
}

#[cfg(test)]
mod tests;
