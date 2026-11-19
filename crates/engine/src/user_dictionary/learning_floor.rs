//! 本机日志的学习时间下限；账号屏障仍由服务层独立持久化。

use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension};

use super::journal::{open_database, open_journal};
use super::learning_sync::MAX_LEARNING_SYNC_TIMESTAMP;
use crate::assets;
use crate::error::{EngineError, Result};
use crate::paths::RuntimePaths;

pub(crate) fn ensure_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS learning_sync_clock(\
         singleton INTEGER PRIMARY KEY CHECK(singleton=1),\
         reset_floor INTEGER NOT NULL CHECK(reset_floor BETWEEN 0 AND 1099511627776));",
    )
}

/// 只读旧日志；旧版本没有此表时下限为零，不创建或迁移源文件。
pub(crate) fn read_path(path: &Path) -> Result<u64> {
    if !path.try_exists()? {
        return Ok(0);
    }
    let connection = open_database(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='learning_sync_clock')",
        [], |row| row.get(0),
    )?;
    if !exists {
        return Ok(0);
    }
    let floor: Option<i64> = connection
        .query_row(
            "SELECT reset_floor FROM learning_sync_clock WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    match floor {
        None => Ok(0),
        Some(value) if (0..=MAX_LEARNING_SYNC_TIMESTAMP as i64).contains(&value) => {
            Ok(value as u64)
        }
        _ => Err(EngineError::failed("Invalid learning clock")),
    }
}

/// 别名由内部调用方固定；与该次写入共用调用方的事务。
pub(crate) fn advance_in(
    connection: &Connection,
    schema: &str,
    floor: u64,
) -> rusqlite::Result<()> {
    if floor > MAX_LEARNING_SYNC_TIMESTAMP {
        return Err(rusqlite::Error::InvalidQuery);
    }
    if floor != 0 {
        connection.execute(
            &format!("INSERT INTO {schema}.learning_sync_clock(singleton,reset_floor) VALUES(1,?1) \
             ON CONFLICT(singleton) DO UPDATE SET reset_floor=max(reset_floor,excluded.reset_floor)"),
            [floor as i64],
        )?;
    }
    Ok(())
}

/// 调用方持有独占维护权，并已持久化账号屏障；不读取账号或网络。
/// v1 支持的本机写入直接读取日志下限，不依赖宿主缓存的 EngineOptions。
pub fn advance_learning_sync_clock(paths: &RuntimePaths, floor: u64) -> Result<()> {
    paths.validate()?;
    if floor > MAX_LEARNING_SYNC_TIMESTAMP {
        return Err(EngineError::invalid("Invalid learning clock"));
    }
    if floor != 0 {
        let connection = open_journal(&paths.user(assets::USER_JOURNAL))?;
        advance_in(&connection, "main", floor)?;
    }
    Ok(())
}

/// 在独占维护权内将本机下限带到尚未发布的日志；备份和云端内容不能降低它。
pub fn carry_learning_sync_clock(active: &RuntimePaths, staged: &RuntimePaths) -> Result<()> {
    active.validate()?;
    staged.validate()?;
    let floor = read_path(&active.user(assets::USER_JOURNAL))?;
    advance_learning_sync_clock(staged, floor)
}

#[cfg(test)]
mod tests;
