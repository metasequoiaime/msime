//! 置顶的同步比较状态与显示投影；删词隐藏置顶时不能丢掉全序胜者。

use rusqlite::{params, Connection};

/// 创建与旧显示行迁移同处保存点，失败可以重试。
pub(crate) fn ensure_schema(connection: &Connection) -> rusqlite::Result<()> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='pinned_candidate_state')",
        [], |row| row.get(0),
    )?;
    if exists {
        return Ok(());
    }
    connection.execute_batch("SAVEPOINT msime_pin_state_schema")?;
    let result = connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS pinned_candidate_state(\
         context_key TEXT PRIMARY KEY,value TEXT NOT NULL,updated_at INTEGER NOT NULL);\
         INSERT OR IGNORE INTO pinned_candidate_state SELECT context_key,value,updated_at FROM pinned_candidates;\
         RELEASE msime_pin_state_schema",
    );
    if result.is_err() {
        let _ = connection
            .execute_batch("ROLLBACK TO msime_pin_state_schema; RELEASE msime_pin_state_schema");
    }
    result
}

/// 接纳旧写入方和备份新增的显示行；别名由内部调用方固定。
pub(crate) fn seed_display(connection: &Connection, schema: &str) -> rusqlite::Result<()> {
    connection.execute_batch(&format!(
        "INSERT INTO {schema}.pinned_candidate_state(context_key,value,updated_at) \
         SELECT context_key,value,updated_at FROM {schema}.pinned_candidates WHERE true \
         ON CONFLICT(context_key) DO UPDATE SET value=excluded.value,updated_at=excluded.updated_at \
         WHERE excluded.updated_at>updated_at OR (excluded.updated_at=updated_at AND excluded.value>value)"
    ))
}

/// 删除记录只隐藏显示，不改变按 context 比较的置顶状态。
pub(crate) fn project(connection: &Connection, schema: &str) -> rusqlite::Result<()> {
    connection.execute_batch(&format!(
        "DELETE FROM {schema}.pinned_candidates;\
         INSERT INTO {schema}.pinned_candidates(context_key,value,updated_at) \
         SELECT p.context_key,p.value,p.updated_at FROM {schema}.pinned_candidate_state p \
         WHERE NOT EXISTS(SELECT 1 FROM {schema}.user_dictionary_operations d \
         WHERE d.dictionary='pinyin' AND d.operation='delete' AND d.value=p.value)"
    ))
}

/// 本机显式置顶推进隐藏状态的时间，不能因先前删词而倒退。
pub(crate) fn set(connection: &Connection, context: &str, value: &str) -> rusqlite::Result<()> {
    seed_display(connection, "main")?;
    connection.execute(
        "INSERT INTO pinned_candidate_state(context_key,value,updated_at) \
         VALUES(?1,?2,min(max(unixepoch(),1),1099511627776)) ON CONFLICT(context_key) \
         DO UPDATE SET value=excluded.value,updated_at=min(max(excluded.updated_at,min(updated_at,1099511627776)+1),1099511627776)",
        params![context,value],
    )?;
    connection.execute(
        "INSERT INTO pinned_candidates(context_key,value,updated_at) \
         SELECT context_key,value,updated_at FROM pinned_candidate_state WHERE context_key=?1 \
         ON CONFLICT(context_key) DO UPDATE SET value=excluded.value,updated_at=excluded.updated_at",
        [context],
    )?;
    Ok(())
}

/// 取消只删本机对应状态，不留下协议未定义的取消记录。
pub(crate) fn clear(connection: &Connection, context: &str, value: &str) -> rusqlite::Result<()> {
    for table in ["pinned_candidates", "pinned_candidate_state"] {
        connection.execute(
            &format!("DELETE FROM {table} WHERE context_key=?1 AND value=?2"),
            params![context, value],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
