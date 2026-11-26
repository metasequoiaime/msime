//! 五笔同词不同码的学习治理：词库自带的简码与全码各行照常显示，但同一个词已经在别的码
//! 上时，自学习不再为它另记一条加权；历史学习记录在建会话时回退到打包词库原值。日志保留
//! 一行低权重记录，学习同步到远端仍收敛到同样的权重；不写删除墓碑，打包词库原有的行不动。

use std::path::Path;

use rusqlite::{
    params, Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior,
};

use crate::assets;
use crate::ime::registry::wubi_database;
use crate::paths::RuntimePaths;
use crate::types::WubiProfileKind;

use super::journal::{open_dictionary_for_writing, open_existing_read_only, open_journal};
use super::learning_clock::{ranking_timestamp, write_weight_at};

/// 同一个词是否已经在别的编码上存在。工作词库里查，包括打包行和用户自己加的行。
pub(crate) fn has_duplicate_code(
    paths: &RuntimePaths,
    profile: WubiProfileKind,
    code: &str,
    word: &str,
) -> bool {
    let Ok(Some(database)) = open_existing_read_only(&wubi_database(paths)) else {
        return false;
    };
    let table = profile.table();
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE value=?1 AND key<>?2)");
    database
        .query_row(&sql, params![word, code], |row| row.get::<_, i64>(0))
        .map(|exists| exists != 0)
        .unwrap_or(false)
}

/// 建会话时回退历史重复学习记录：日志里所有五笔调权行，只要同一个词在别的码上也存在，
/// 就把加权恢复到打包词库原值。用户自己加进个人词库的行（`user_inserted=1`）不动。
pub(crate) fn reset_all_duplicate_learning(
    paths: &RuntimePaths,
    journal_path: &Path,
    profile: WubiProfileKind,
) -> bool {
    let Ok(Some(journal)) = open_existing_read_only(journal_path) else {
        return false;
    };
    let kind = profile.dictionary_kind();
    let Ok(bundled) = Connection::open_with_flags(
        paths.resource(assets::WUBI_DICTIONARY),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) else {
        return false;
    };
    let Ok(Some(work)) = open_existing_read_only(&wubi_database(paths)) else {
        return false;
    };
    let table = profile.table();
    let bundled_sql = format!("SELECT weight FROM {table} WHERE key=?1 AND value=?2 LIMIT 1");
    let duplicate_sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE value=?1 AND key<>?2)");
    let Ok(mut bundled_weight) = bundled.prepare(&bundled_sql) else {
        return false;
    };
    let Ok(mut duplicate) = work.prepare(&duplicate_sql) else {
        return false;
    };
    let Ok(mut logged) = journal.prepare(
        "SELECT key,value,weight FROM user_dictionary_operations \
         WHERE dictionary=?1 AND operation='upsert' AND user_inserted=0",
    ) else {
        return false;
    };
    let rows = logged
        .query_map([kind.journal_name()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|row| {
            let Ok((code, word, weight)) = row else {
                return None;
            };
            let original: Option<i64> = bundled_weight
                .query_row(params![&code, &word], |row| row.get(0))
                .optional()
                .unwrap_or(None);
            let exists: i64 = duplicate
                .query_row(params![&word, &code], |row| row.get(0))
                .unwrap_or(0);
            if original.is_some_and(|original| weight > original) && exists != 0 {
                Some((code, word))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    reset_duplicate_learning(paths, journal_path, profile, &rows)
}

/// 把每条 `(编码, 词)` 的学习加权回退到打包词库原值；返回是否改写了工作词库。
/// 只处理日志里 `operation='upsert'` 且 `user_inserted=0` 的纯调权行；打包词库里没有的行
/// 没有原值可回退，一并跳过。读不到打包词库或日志时什么也不做。
pub(crate) fn reset_duplicate_learning(
    paths: &RuntimePaths,
    journal_path: &Path,
    profile: WubiProfileKind,
    duplicates: &[(String, String)],
) -> bool {
    if duplicates.is_empty() {
        return false;
    }
    let Ok(bundled) = Connection::open_with_flags(
        paths.resource(assets::WUBI_DICTIONARY),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) else {
        return false;
    };
    let table = profile.table();
    let kind = profile.dictionary_kind();
    let journal = match open_journal(journal_path) {
        Ok(journal) => journal,
        Err(_) => return false,
    };
    let Ok(transaction) = Transaction::new_unchecked(&journal, TransactionBehavior::Immediate)
    else {
        return false;
    };

    let mut resets = Vec::new();
    {
        let bundled_sql = format!("SELECT weight FROM {table} WHERE key=?1 AND value=?2 LIMIT 1");
        let Ok(mut bundled_weight) = bundled.prepare(&bundled_sql) else {
            let _ = transaction.rollback();
            return false;
        };
        let Ok(mut logged) = transaction.prepare(
            "SELECT weight,user_inserted FROM user_dictionary_operations \
             WHERE dictionary=?1 AND key=?2 AND value=?3 AND operation='upsert'",
        ) else {
            let _ = transaction.rollback();
            return false;
        };
        for (code, word) in duplicates {
            let original: Option<i64> = bundled_weight
                .query_row(params![code, word], |row| row.get(0))
                .optional()
                .unwrap_or(None);
            let Some(original) = original else {
                continue;
            };
            let logged: Option<(i64, i64)> = logged
                .query_row(params![kind.journal_name(), code, word], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
                .optional()
                .unwrap_or(None);
            match logged {
                Some((weight, 0)) if weight > original => {
                    resets.push((code.clone(), word.clone(), original));
                }
                _ => {}
            }
        }
    }
    if resets.is_empty() {
        let _ = transaction.commit();
        return false;
    }

    // 和调频一样逐条推进逻辑时间，避免回退后的时间戳不增反而被同步当成旧记录。
    let mut plan = Vec::with_capacity(resets.len());
    for (code, word, weight) in &resets {
        if let Ok(timestamp) = ranking_timestamp(&transaction, kind, code, [word.as_str()]) {
            plan.push((code.clone(), word.clone(), *weight, timestamp));
        }
    }
    // 先落工作词库，再落日志。反过来先提交日志的话，工作库写入失败会让日志看起来已经回退，
    // 下次会话就不再重试；这个顺序最坏只是工作库已回退而日志仍旧，下次会话会重写日志。
    let Ok(mut main) = open_dictionary_for_writing(&wubi_database(paths)) else {
        let _ = transaction.rollback();
        return false;
    };
    let Ok(main_transaction) = main.transaction_with_behavior(TransactionBehavior::Immediate)
    else {
        let _ = transaction.rollback();
        return false;
    };
    let committed = {
        let update_sql = format!("UPDATE {table} SET weight=?1 WHERE key=?2 AND value=?3");
        match main_transaction.prepare(&update_sql) {
            Ok(mut update) => {
                let mut ok = true;
                for (code, word, weight, _) in &plan {
                    // 同一行可能被多条记录指到，逐条写没有副作用；失败的行留给下次会话重试。
                    if update.execute(params![weight, code, word]).is_err() {
                        ok = false;
                    }
                }
                ok
            }
            Err(_) => false,
        }
    };
    if !committed {
        let _ = main_transaction.rollback();
        let _ = transaction.rollback();
        return false;
    }
    if main_transaction.commit().is_err() {
        let _ = transaction.rollback();
        return false;
    }

    let mut journal_changed = false;
    for (code, word, weight, timestamp) in &plan {
        if write_weight_at(&transaction, kind, code, word, *weight, *timestamp).is_ok() {
            journal_changed = true;
        }
    }
    if !journal_changed {
        let _ = transaction.rollback();
        return false;
    }
    transaction.commit().is_ok()
}
