//! 同步记录与日志的转换；联网、重置屏障持久化和维护租约由调用方持有。

use std::collections::BTreeSet;

use rusqlite::types::ValueRef;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Row, TransactionBehavior};

use super::{LearningSyncRecord, LearningSyncState, MAX_LEARNING_SYNC_TIMESTAMP};
use crate::assets;
use crate::error::{EngineError, Result};
use crate::paths::RuntimePaths;
use crate::types::PersonalDictionaryKind;
use crate::user_dictionary::{journal, ngram_store, personal, pin_state, position_state, state};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LearningSyncExport {
    pub records: usize,
    pub skipped: usize,
    /// 包括未知词库的有效时间，调用方发布导出前须持久化这个上限。
    pub observed_max: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LearningSyncMerge {
    pub operations: usize,
    pub pins: usize,
    pub positions: usize,
    pub kept: usize,
    pub filtered: usize,
    pub skipped: usize,
}

fn invalid() -> EngineError {
    EngineError::failed("Invalid learning sync state")
}

fn text(row: &Row<'_>, column: usize, limit: usize, empty: bool) -> rusqlite::Result<String> {
    let ValueRef::Text(bytes) = row.get_ref(column)? else {
        return Err(rusqlite::Error::InvalidQuery);
    };
    if bytes.len() > limit || (!empty && bytes.is_empty()) || bytes.contains(&0) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| rusqlite::Error::InvalidQuery)
}

fn timestamp(row: &Row<'_>, column: usize) -> rusqlite::Result<u64> {
    let time = row.get::<_, i64>(column)?;
    if !(1..=MAX_LEARNING_SYNC_TIMESTAMP as i64).contains(&time) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(time as u64)
}

fn operation(row: &Row<'_>) -> rusqlite::Result<LearningSyncRecord> {
    let dictionary = text(row, 0, 512, false)?;
    let key = text(row, 1, 512, false)?;
    let value = text(row, 2, 4096, false)?;
    let updated_at = timestamp(row, 6)?;
    match text(row, 3, 16, false)?.as_str() {
        "delete" => Ok(LearningSyncRecord::Delete {
            dictionary,
            key,
            value,
            updated_at,
        }),
        "upsert" => Ok(LearningSyncRecord::Weight {
            dictionary,
            key,
            value,
            weight: u64::try_from(row.get::<_, i64>(4)?)
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            display: text(row, 5, 4096, true)?,
            updated_at,
        }),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn position(row: &Row<'_>) -> rusqlite::Result<LearningSyncRecord> {
    let slot = row.get::<_, i64>(3)?;
    if !(1..=5).contains(&slot) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(LearningSyncRecord::Position {
        context: text(row, 0, 1024, false)?,
        key: text(row, 1, 512, false)?,
        value: text(row, 2, 4096, false)?,
        position: slot as u8,
        updated_at: timestamp(row, 4)?,
    })
}

fn pin(row: &Row<'_>) -> rusqlite::Result<LearningSyncRecord> {
    Ok(LearningSyncRecord::Pin {
        context: text(row, 0, 1024, false)?,
        value: text(row, 1, 4096, false)?,
        updated_at: timestamp(row, 2)?,
    })
}

fn has_table(connection: &Connection, name: &str) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [name],
        |row| row.get(0),
    )
}

/// 在一致只读事务中流出真实时间和全部固定候选，不创建或迁移日志。
///
/// 本机行数可超过单份传输上限；调用方负责按协议截断调权并完成整份编码，成功前不能发布。
/// `emit` 返回假时整次导出失败；它已经见到的前缀不能作为完整同步状态使用。
pub fn stream_learning_sync_state(
    paths: &RuntimePaths,
    emit: &mut dyn FnMut(&LearningSyncRecord) -> bool,
) -> Result<LearningSyncExport> {
    paths.validate()?;
    let file = paths.user(assets::USER_JOURNAL);
    if !file.try_exists()? {
        return Ok(LearningSyncExport::default());
    }
    let mut read = || -> Result<LearningSyncExport> {
        let mut connection = journal::open_database(&file, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        connection.execute_batch("PRAGMA query_only=ON")?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let mut result = LearningSyncExport::default();
        let mut push = |record: LearningSyncRecord| -> Result<()> {
            if !record.valid() {
                return Err(invalid());
            }
            result.observed_max = result.observed_max.max(record.updated_at());
            let stale_pinyin = matches!(&record, LearningSyncRecord::Weight { dictionary, weight: 0, .. } if dictionary == "pinyin");
            if record.supported() && !stale_pinyin {
                if !emit(&record) {
                    return Err(invalid());
                }
                result.records += 1;
            } else {
                result.skipped += 1;
            }
            Ok(())
        };
        {
            let mut statement = transaction.prepare(
                "SELECT dictionary,key,value,operation,weight,display,updated_at \
                 FROM user_dictionary_operations WHERE operation='delete' OR user_inserted=0 \
                 ORDER BY dictionary,key,value",
            )?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                push(operation(row)?)?;
            }
        }
        if has_table(&transaction, "pinned_candidates")? {
            let query = if has_table(&transaction, "pinned_candidate_state")? {
                "SELECT c.context_key,c.value,c.updated_at FROM pinned_candidate_state c \
                 WHERE NOT EXISTS(SELECT 1 FROM pinned_candidates d WHERE d.context_key=c.context_key \
                   AND (d.updated_at>c.updated_at OR (d.updated_at=c.updated_at AND d.value>c.value))) \
                 UNION ALL SELECT d.context_key,d.value,d.updated_at FROM pinned_candidates d \
                 WHERE NOT EXISTS(SELECT 1 FROM pinned_candidate_state c WHERE c.context_key=d.context_key \
                   AND (c.updated_at>d.updated_at OR (c.updated_at=d.updated_at AND c.value>=d.value))) ORDER BY 1"
            } else {
                "SELECT context_key,value,updated_at FROM pinned_candidates ORDER BY context_key"
            };
            let mut statement = transaction.prepare(query)?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                push(pin(row)?)?;
            }
        }
        if has_table(&transaction, "fixed_candidate_positions")? {
            let query = if has_table(&transaction, "fixed_candidate_position_state")? {
                "SELECT context_key,entry_key,value,position,updated_at FROM fixed_candidate_position_state \
                 UNION ALL SELECT d.context_key,d.entry_key,d.value,d.position,1 \
                 FROM fixed_candidate_positions d WHERE NOT EXISTS(\
                   SELECT 1 FROM fixed_candidate_position_state c WHERE c.context_key=d.context_key \
                   AND c.entry_key=d.entry_key AND c.value=d.value) ORDER BY 1,2,3"
            } else {
                "SELECT context_key,entry_key,value,position,1 FROM fixed_candidate_positions ORDER BY context_key,entry_key,value"
            };
            let mut statement = transaction.prepare(query)?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                push(position(row)?)?;
            }
        }
        transaction.commit()?;
        Ok(result)
    };
    read().map_err(|_| invalid())
}

/// 按同步全序比较并应用胜出行，完整候选和显示投影同事务更新。
///
/// 调用方必须持有独占维护权，并在调用前持久化已认证完整远端流的观察时间。
/// `reset_floor` 来自账号的持久屏障，在真正应用待合并文件时重新读取，不能使用下载时的旧值。
/// SQLite 的多库 WAL 崩溃边界与个人词库编辑相同；本函数保证普通写入错误整体回滚。
pub fn merge_learning_sync_state(
    paths: &RuntimePaths,
    main_dictionary: bool,
    remote: &LearningSyncState,
    reset_floor: u64,
) -> Result<LearningSyncMerge> {
    paths.validate()?;
    if reset_floor > MAX_LEARNING_SYNC_TIMESTAMP {
        return Err(invalid());
    }
    if remote
        .records()
        .all(|record| record.updated_at() <= reset_floor)
    {
        return Ok(LearningSyncMerge {
            filtered: remote.records().len(),
            ..LearningSyncMerge::default()
        });
    }
    merge(paths, main_dictionary, remote, reset_floor).map_err(|_| invalid())
}

fn merge(
    paths: &RuntimePaths,
    main_dictionary: bool,
    remote: &LearningSyncState,
    reset_floor: u64,
) -> Result<LearningSyncMerge> {
    let file = paths.user(assets::USER_JOURNAL);
    ngram_store::flush_journal(&file)?;
    let mut connection = personal::open_edit_connection(paths, main_dictionary)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    position_state::seed_display(&transaction, "personal_journal", None)?;
    pin_state::seed_display(&transaction, "personal_journal")?;
    let mut result = LearningSyncMerge::default();
    let mut contexts = BTreeSet::new();
    let mut removed_context = Vec::new();
    for incoming in remote.records() {
        if incoming.updated_at() <= reset_floor {
            result.filtered += 1;
            continue;
        }
        match incoming {
            LearningSyncRecord::Weight {
                dictionary,
                key,
                value,
                ..
            }
            | LearningSyncRecord::Delete {
                dictionary,
                key,
                value,
                ..
            } => {
                let local = transaction.query_row(
                    "SELECT dictionary,key,value,operation,weight,display,updated_at,user_inserted \
                     FROM personal_journal.user_dictionary_operations WHERE dictionary=?1 AND key=?2 AND value=?3",
                    params![dictionary,key,value], |row| {
                        if row.get::<_, i64>(7)? != 0 { Ok((None,true)) }
                        else { Ok((Some(operation(row)?),false)) }
                    },
                ).optional()?;
                if local.is_some_and(|(record, protected)| {
                    protected || record.is_some_and(|record| !incoming.priority(&record).is_gt())
                }) {
                    result.kept += 1;
                    continue;
                }
                let kind =
                    PersonalDictionaryKind::from_journal_name(dictionary).ok_or_else(invalid)?;
                if !main_dictionary && kind != PersonalDictionaryKind::English {
                    result.skipped += 1;
                    continue;
                }
                let (deleted, weight, display) = match incoming {
                    LearningSyncRecord::Weight {
                        weight, display, ..
                    } => (false, *weight as i64, display.as_str()),
                    _ => (true, 0, ""),
                };
                if kind == PersonalDictionaryKind::Pinyin && !deleted && weight == 0 {
                    result.skipped += 1;
                    continue;
                }
                let applied = match state::apply_entry(
                    &transaction,
                    kind,
                    key.as_bytes(),
                    value.as_bytes(),
                    deleted,
                    weight,
                    display.as_bytes(),
                ) {
                    Ok(applied) => applied,
                    Err(error) if state::missing_table(&error) => false,
                    Err(error) => return Err(error.into()),
                };
                if !applied {
                    result.skipped += 1;
                    continue;
                }
                transaction.execute(
                    "INSERT INTO personal_journal.user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted,updated_at) \
                     VALUES(?1,?2,?3,?4,?5,?6,0,?7) ON CONFLICT(dictionary,key,value) DO UPDATE SET \
                     operation=excluded.operation,weight=excluded.weight,display=excluded.display,user_inserted=0,updated_at=excluded.updated_at",
                    params![dictionary,key,value,if deleted {"delete"} else {"upsert"},weight,display,incoming.updated_at() as i64],
                )?;
                if deleted && kind == PersonalDictionaryKind::Pinyin {
                    transaction.execute(
                        "DELETE FROM personal_journal.pinned_candidates WHERE value=?1",
                        [value],
                    )?;
                    removed_context.extend(ngram_store::delete_personal_ngram_word(
                        &transaction,
                        "personal_journal",
                        value,
                    )?);
                }
                result.operations += 1;
            }
            LearningSyncRecord::Pin {
                context,
                value,
                updated_at,
            } => {
                let local = transaction.query_row(
                    "SELECT context_key,value,updated_at FROM personal_journal.pinned_candidate_state WHERE context_key=?1",
                    [context], pin,
                ).optional()?;
                if local.is_some_and(|record| !incoming.priority(&record).is_gt()) {
                    result.kept += 1;
                    continue;
                }
                transaction.execute(
                    "INSERT INTO personal_journal.pinned_candidate_state(context_key,value,updated_at) VALUES(?1,?2,?3) \
                     ON CONFLICT(context_key) DO UPDATE SET value=excluded.value,updated_at=excluded.updated_at",
                    params![context,value,*updated_at as i64],
                )?;
                result.pins += 1;
            }
            LearningSyncRecord::Position {
                context,
                key,
                value,
                position: slot,
                updated_at,
            } => {
                let local = transaction.query_row(
                    "SELECT context_key,entry_key,value,position,updated_at FROM personal_journal.fixed_candidate_position_state \
                     WHERE context_key=?1 AND entry_key=?2 AND value=?3",
                    params![context,key,value], position,
                ).optional()?;
                if local.is_some_and(|record| !incoming.priority(&record).is_gt()) {
                    result.kept += 1;
                    continue;
                }
                transaction.execute(
                    "INSERT INTO personal_journal.fixed_candidate_position_state(context_key,entry_key,value,position,updated_at) \
                     VALUES(?1,?2,?3,?4,?5) ON CONFLICT(context_key,entry_key,value) DO UPDATE SET \
                     position=excluded.position,updated_at=excluded.updated_at",
                    params![context,key,value,slot,*updated_at as i64],
                )?;
                contexts.insert(context);
                result.positions += 1;
            }
        }
    }
    for context in contexts {
        position_state::project_in(&transaction, "personal_journal", context)?;
    }
    pin_state::project(&transaction, "personal_journal")?;
    transaction.commit()?;
    ngram_store::forget_journal_rows(&file, &removed_context);
    Ok(result)
}

#[cfg(test)]
mod tests;
