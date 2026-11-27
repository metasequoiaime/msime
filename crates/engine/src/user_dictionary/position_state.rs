//! 固定位置的完整候选状态；`fixed_candidate_positions` 只保存槽位投影。

use rusqlite::{params, Connection};

use super::learning_sync::MAX_LEARNING_SYNC_TIMESTAMP;

fn exists(connection: &Connection) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='fixed_candidate_position_state')",
        [],
        |row| row.get(0),
    )
}

/// 建表和迁移同处一个保存点，失败后不能留下阻止下次重试的空状态表。
pub(crate) fn ensure_schema(connection: &Connection) -> rusqlite::Result<()> {
    if exists(connection)? {
        return Ok(());
    }
    connection.execute_batch("SAVEPOINT msime_fixed_position_schema")?;
    let result = (|| {
        if !exists(connection)? {
            connection.execute_batch(
                "CREATE TABLE fixed_candidate_position_state(\
                 context_key TEXT NOT NULL,entry_key TEXT NOT NULL,value TEXT NOT NULL,\
                 position INTEGER NOT NULL CHECK(position BETWEEN 1 AND 5),\
                 updated_at INTEGER NOT NULL CHECK(updated_at BETWEEN 1 AND 1099511627776),\
                 PRIMARY KEY(context_key,entry_key,value));\
                 CREATE INDEX fixed_candidate_position_slots ON fixed_candidate_position_state(context_key,position,updated_at);\
                 INSERT INTO fixed_candidate_position_state(context_key,entry_key,value,position,updated_at)\
                 SELECT context_key,entry_key,value,position,1 FROM fixed_candidate_positions;",
            )?;
        }
        connection.execute_batch("RELEASE msime_fixed_position_schema")
    })();
    if result.is_err() {
        let _ = connection.execute_batch(
            "ROLLBACK TO msime_fixed_position_schema; RELEASE msime_fixed_position_schema",
        );
    }
    result
}

/// 接纳旧写入方和无时间戳备份新增的显示行，不覆盖已有完整候选或其时间。`schema` 只由内部调用方传入固定别名。
pub(crate) fn seed_display(
    connection: &Connection,
    schema: &str,
    context: Option<&str>,
) -> rusqlite::Result<usize> {
    let filter = if context.is_some() {
        " WHERE context_key=?1"
    } else {
        ""
    };
    let sql = format!(
        "INSERT OR IGNORE INTO {schema}.fixed_candidate_position_state(context_key,entry_key,value,position,updated_at) \
         SELECT context_key,entry_key,value,position,1 FROM {schema}.fixed_candidate_positions{filter}"
    );
    if let Some(context) = context {
        connection.execute(&sql, [context])
    } else {
        connection.execute(&sql, [])
    }
}

/// 槽位冲突只影响显示；没有槽位唯一约束的完整表保留全部输家。
fn project(connection: &Connection, context: &str) -> rusqlite::Result<()> {
    project_in(connection, "main", context)
}

/// `schema` 仅接受本模块与同步合并传入的固定附加库别名。
pub(crate) fn project_in(
    connection: &Connection,
    schema: &str,
    context: &str,
) -> rusqlite::Result<()> {
    connection.execute(
        &format!("DELETE FROM {schema}.fixed_candidate_positions WHERE context_key=?1"),
        [context],
    )?;
    connection.execute(
        &format!("INSERT INTO {schema}.fixed_candidate_positions(context_key,entry_key,value,position) \
         SELECT c.context_key,c.entry_key,c.value,c.position FROM {schema}.fixed_candidate_position_state c \
         WHERE c.context_key=?1 AND NOT EXISTS(\
           SELECT 1 FROM {schema}.fixed_candidate_position_state w \
           WHERE w.context_key=c.context_key AND w.position=c.position AND (\
             w.updated_at>c.updated_at OR (w.updated_at=c.updated_at AND (\
               w.entry_key<c.entry_key OR (w.entry_key=c.entry_key AND w.value<c.value)))))"),
        [context],
    )?;
    Ok(())
}

/// 调用方持有 IMMEDIATE 事务；显式占用一个槽位时，移除该槽位的旧候选，维持本机原有的替换语义。
pub(crate) fn set(
    connection: &Connection,
    context: &str,
    key: &str,
    value: &str,
    position: i32,
) -> rusqlite::Result<()> {
    seed_display(connection, "main", Some(context))?;
    let timestamp: i64 = connection.query_row(
        "SELECT min(max(min(COALESCE(MAX(updated_at),0),?4)+1,unixepoch(),COALESCE((SELECT reset_floor FROM learning_sync_clock WHERE singleton=1),0)+1),?4) \
         FROM fixed_candidate_position_state WHERE context_key=?1 AND entry_key=?2 AND value=?3",
        params![context, key, value, MAX_LEARNING_SYNC_TIMESTAMP as i64],
        |row| row.get(0),
    )?;
    // 同一上下文按词面固定：固定记录迁移到新槽位时带走旧身份，否则重排后显示的同一行会留下两条记录，取消只删其中一条。
    connection.execute(
        "DELETE FROM fixed_candidate_position_state WHERE context_key=?1 AND (value=?2 OR position=?3)",
        params![context, value, position],
    )?;
    connection.execute(
        "INSERT INTO fixed_candidate_position_state(context_key,entry_key,value,position,updated_at) \
         VALUES(?1,?2,?3,?4,?5) ON CONFLICT(context_key,entry_key,value) \
         DO UPDATE SET position=excluded.position,updated_at=excluded.updated_at",
        params![context, key, value, position, timestamp],
    )?;
    project(connection, context)
}

/// 调用方持有 IMMEDIATE 事务；取消同时删完整候选，再从余下候选重建显示。
pub(crate) fn clear(
    connection: &Connection,
    context: &str,
    _key: &str,
    value: &str,
) -> rusqlite::Result<()> {
    seed_display(connection, "main", Some(context))?;
    // 固定记录按词面记账：重排后同一行的编码可能从简码变成全码，取消仍须按词面找到它。
    connection.execute(
        "DELETE FROM fixed_candidate_position_state WHERE context_key=?1 AND value=?2",
        params![context, value],
    )?;
    project(connection, context)
}

#[cfg(test)]
mod tests;
