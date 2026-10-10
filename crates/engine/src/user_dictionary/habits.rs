//! 输入习惯的导出与合并（#5659 本地备份）：词库状态（[`super::state`]）之外、同样记在日志里的六张学习表——整句联想的二元/三元计数（`personal_bigram`/`personal_trigram`）、连续选词对（`pick_transitions`）、拼写纠错计数（`pinyin_typo_counts`）、自动纠错的抑制记录（`pinyin_autocorrect_suppressions`）和置顶的候选（`pinned_candidates`）。
//!
//! 它们不进 [`super::state::DictionaryStateRecord`]：那是与 Apple 共用、按字节计算修订号的契约，加一种记录就会改掉每一台设备的词库版本。所以这里另有一套记录和一个只读的流，合并规则与词库状态的合并相同，都是本机优先：计数取两边较大的那个，置顶本机已有就保留本机。

use std::path::Path;

use rusqlite::{params, OpenFlags, TransactionBehavior};

use crate::assets;
use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::lattice::personal::PersonalNgramOptions;
use crate::paths::RuntimePaths;
use crate::user_dictionary::journal::{ensure_schema, open_database};
use crate::user_dictionary::ngram_store;
use crate::user_dictionary::picks::MAX_PICK_TRANSITIONS;
use crate::user_dictionary::state::DEFAULT_MAXIMUM_RECORDS;
use crate::user_dictionary::typo_profile::{
    is_storable_autocorrect_suppression, MAX_TYPO_STATE_COUNT, MAX_TYPO_STATE_ROWS,
};

/// 编码、上下文和抑制输入的上限，与词库状态的编码上限相同。
pub const MAX_HABIT_KEY_BYTES: usize = 512;
/// 词的上限，与词库状态的词上限相同。
pub const MAX_HABIT_VALUE_BYTES: usize = 4_096;
/// 拼写纠错计数里一个音节最多几个字母（`zhuang`），与 `typo_profile` 相同。
const MAX_SYLLABLE_LENGTH: usize = 6;

/// 一条输入习惯。计数和时间都是日志里的原值；`updated_at` 是 Unix 秒。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearningHabit {
    /// `personal_bigram`：`previous` 之后打出 `word` 的次数；句首的 `previous` 是 `SENTENCE_START`（U+0001）。
    Bigram {
        previous: String,
        word: String,
        count: i64,
    },
    /// `personal_trigram`。
    Trigram {
        earlier: String,
        previous: String,
        word: String,
        count: i64,
    },
    /// `pick_transitions`：连着选了这两个词的次数，够多时会被组成词。
    PickTransition {
        previous_key: String,
        previous_value: String,
        key: String,
        value: String,
        count: i64,
        updated_at: i64,
    },
    /// `pinyin_typo_counts`：把 `typed` 当成 `intended` 接受的次数。
    TypoCount {
        typed: String,
        intended: String,
        accepted: i64,
        updated_at: i64,
    },
    /// `pinyin_autocorrect_suppressions`：用户按原样上屏、拒绝自动纠错的输入。
    AutocorrectSuppression {
        input: String,
        commits: i64,
        updated_at: i64,
    },
    /// `pinned_candidates`：这个上下文置顶的候选。
    PinnedCandidate {
        context: String,
        value: String,
        updated_at: i64,
    },
}

/// [`merge_learning_habits`] 的结果。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LearningHabitsMerge {
    /// 新写进来或调大了计数的记录。
    pub written: usize,
    /// 本机已有、按规则保留本机的记录：本机计数不小于记录里的，或这个上下文本机已经置顶了别的候选。
    pub kept: usize,
    /// 合并后超出表的上限、随即被修剪掉的行数（整句联想按衰减规则减半，其余按各自的规则删最旧或最少用的）。
    pub trimmed: usize,
}

fn invalid_habits() -> EngineError {
    EngineError::failed(diagnostics::INVALID_DICTIONARY_STATE)
}

fn require(condition: bool) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(invalid_habits())
    }
}

fn bounded(text: &str, maximum: usize) -> bool {
    !text.is_empty() && text.len() <= maximum && !text.contains('\0')
}

fn syllable(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_SYLLABLE_LENGTH
        && text.bytes().all(|byte| byte.is_ascii_lowercase())
}

/// 一条记录能不能写进日志：文字非空、不超长、不含 NUL，计数在各表的约束之内。导出端据此跳过装不下的行，合并端据此拒绝整份输入。
pub fn habit_is_storable(habit: &LearningHabit) -> bool {
    let key = |text: &str| bounded(text, MAX_HABIT_KEY_BYTES);
    let value = |text: &str| bounded(text, MAX_HABIT_VALUE_BYTES);
    let time = |updated_at: &i64| *updated_at >= 0;
    match habit {
        LearningHabit::Bigram {
            previous,
            word,
            count,
        } => value(previous) && value(word) && *count >= 1,
        LearningHabit::Trigram {
            earlier,
            previous,
            word,
            count,
        } => value(earlier) && value(previous) && value(word) && *count >= 1,
        LearningHabit::PickTransition {
            previous_key,
            previous_value,
            key: code,
            value: word,
            count,
            updated_at,
        } => {
            key(previous_key)
                && value(previous_value)
                && key(code)
                && value(word)
                && *count >= 1
                && time(updated_at)
        }
        LearningHabit::TypoCount {
            typed,
            intended,
            accepted,
            updated_at,
        } => {
            syllable(typed)
                && syllable(intended)
                && typed != intended
                && (1..=i64::from(MAX_TYPO_STATE_COUNT)).contains(accepted)
                && time(updated_at)
        }
        LearningHabit::AutocorrectSuppression {
            input,
            commits,
            updated_at,
        } => {
            is_storable_autocorrect_suppression(input)
                && (1..=i64::from(MAX_TYPO_STATE_COUNT)).contains(commits)
                && time(updated_at)
        }
        LearningHabit::PinnedCandidate {
            context,
            value: word,
            updated_at,
        } => key(context) && value(word) && time(updated_at),
    }
}

/// 六张表依次读出（二元、三元、选词对、纠错计数、抑制、置顶，各自按主键排序），在一个只读事务里交给 `emit`；`emit` 返回假时停下并报 `INVALID_DICTIONARY_STATE`。日志不存在时什么也不发、也不创建它；旧日志里没有的表当作空表。文字不是 UTF-8 或超过 [`DEFAULT_MAXIMUM_RECORDS`] 条时报错，与 [`super::state::stream_dictionary_state`] 相同。不检查文字长度，装不下的行由调用方用 [`habit_is_storable`] 跳过。
pub fn stream_learning_habits(
    paths: &RuntimePaths,
    emit: &mut dyn FnMut(&LearningHabit) -> bool,
) -> Result<()> {
    paths.validate()?;
    let file = paths.user(assets::USER_JOURNAL);
    if !file.exists() {
        return Ok(());
    }
    read_habits(&file, emit).map_err(|error| match error {
        EngineError::Sqlite(_) => invalid_habits(),
        other => other,
    })
}

/// 六张表的行数之和，只读；日志不存在时为 0。本地备份恢复时用它判断本机是不是还什么都没学过（整份激活会换掉这些表）。
pub fn count_learning_habits(paths: &RuntimePaths) -> Result<usize> {
    let mut count = 0usize;
    stream_learning_habits(paths, &mut |_| {
        count += 1;
        true
    })?;
    Ok(count)
}

fn table_exists(connection: &rusqlite::Connection, table: &str) -> Result<bool> {
    Ok(connection
        .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1")?
        .exists(params![table])?)
}

fn read_habits(file: &Path, emit: &mut dyn FnMut(&LearningHabit) -> bool) -> Result<()> {
    // 与 `state::read_state` 相同：读者也可能要建 WAL 协调文件，所以以读写方式打开，但 `query_only` 禁止改日志本身。
    let mut connection = open_database(file, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    connection.execute_batch("PRAGMA query_only=ON")?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Deferred)?;
    let mut records = 0usize;
    let mut send = |habit: LearningHabit| -> Result<()> {
        require(records < DEFAULT_MAXIMUM_RECORDS)?;
        records += 1;
        require(emit(&habit))
    };
    if table_exists(&transaction, "personal_bigram")? {
        let mut rows = transaction
            .prepare("SELECT previous,word,count FROM personal_bigram ORDER BY previous,word")?;
        let mut cursor = rows.query([])?;
        while let Some(row) = cursor.next()? {
            send(LearningHabit::Bigram {
                previous: row.get(0)?,
                word: row.get(1)?,
                count: row.get(2)?,
            })?;
        }
    }
    if table_exists(&transaction, "personal_trigram")? {
        let mut rows = transaction.prepare(
            "SELECT earlier,previous,word,count FROM personal_trigram ORDER BY earlier,previous,word",
        )?;
        let mut cursor = rows.query([])?;
        while let Some(row) = cursor.next()? {
            send(LearningHabit::Trigram {
                earlier: row.get(0)?,
                previous: row.get(1)?,
                word: row.get(2)?,
                count: row.get(3)?,
            })?;
        }
    }
    if table_exists(&transaction, "pick_transitions")? {
        let mut rows = transaction.prepare(
            "SELECT previous_key,previous_value,key,value,count,updated_at FROM pick_transitions ORDER BY previous_key,previous_value,key,value",
        )?;
        let mut cursor = rows.query([])?;
        while let Some(row) = cursor.next()? {
            send(LearningHabit::PickTransition {
                previous_key: row.get(0)?,
                previous_value: row.get(1)?,
                key: row.get(2)?,
                value: row.get(3)?,
                count: row.get(4)?,
                updated_at: row.get(5)?,
            })?;
        }
    }
    if table_exists(&transaction, "pinyin_typo_counts")? {
        let mut rows = transaction.prepare(
            "SELECT typed,intended,accepted,updated_at FROM pinyin_typo_counts ORDER BY typed,intended",
        )?;
        let mut cursor = rows.query([])?;
        while let Some(row) = cursor.next()? {
            send(LearningHabit::TypoCount {
                typed: row.get(0)?,
                intended: row.get(1)?,
                accepted: row.get(2)?,
                updated_at: row.get(3)?,
            })?;
        }
    }
    if table_exists(&transaction, "pinyin_autocorrect_suppressions")? {
        let mut rows = transaction.prepare(
            "SELECT input,commits,updated_at FROM pinyin_autocorrect_suppressions ORDER BY input",
        )?;
        let mut cursor = rows.query([])?;
        while let Some(row) = cursor.next()? {
            send(LearningHabit::AutocorrectSuppression {
                input: row.get(0)?,
                commits: row.get(1)?,
                updated_at: row.get(2)?,
            })?;
        }
    }
    if table_exists(&transaction, "pinned_candidates")? {
        let mut rows = transaction.prepare(
            "SELECT context_key,value,updated_at FROM pinned_candidates ORDER BY context_key",
        )?;
        let mut cursor = rows.query([])?;
        while let Some(row) = cursor.next()? {
            send(LearningHabit::PinnedCandidate {
                context: row.get(0)?,
                value: row.get(1)?,
                updated_at: row.get(2)?,
            })?;
        }
    }
    transaction.commit()?;
    Ok(())
}

/// 把 `records`（本地备份里的输入习惯）合并进本机正在用的日志。全部在一个 IMMEDIATE 事务里完成：任何一条记录不合规（[`habit_is_storable`]）、读流失败、超过 `maximum_records` 条或写库出错都整体回滚，本机一行不变。
///
/// 规则是本机优先、可重复执行：各种计数取两边较大的那个（不相加，同一份备份恢复两次不会翻倍），`updated_at` 取较新的；置顶的候选本机这个上下文已经有了就保留本机。写完后按各表平时的规则压回上限：整句联想超过 `max_transitions` 时像衰减那样反复减半，选词对删到最近的 [`MAX_PICK_TRANSITIONS`] 条，纠错计数和抑制记录各删到 [`MAX_TYPO_STATE_ROWS`] 行（先删最少用、最旧的）。否则一份大备份合并进来，下次读整句联想时会因为超出内存上限整个读不进来。
///
/// 调用方必须持有词库的独占维护权：整句联想的 store 在会话里有内存模型和待写队列，这里先把队列写下去，合并后把模型标成过期，下次用到时重读。
pub fn merge_learning_habits(
    paths: &RuntimePaths,
    records: &mut dyn Iterator<Item = Result<LearningHabit>>,
    maximum_records: usize,
) -> Result<LearningHabitsMerge> {
    paths.validate()?;
    let file = paths.user(assets::USER_JOURNAL);
    // 还排着的整句联想写入要先落盘，否则它会在合并之后写回来，按旧的内存模型加计数。
    ngram_store::flush_journal(&file)?;
    let mut connection = open_database(
        &file,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )?;
    ensure_schema(&connection)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut merge = LearningHabitsMerge::default();
    {
        let mut bigram = transaction.prepare(
            "INSERT INTO personal_bigram(previous,word,count) VALUES(?1,?2,?3) ON CONFLICT(previous,word) DO UPDATE SET count=excluded.count WHERE excluded.count>count",
        )?;
        let mut trigram = transaction.prepare(
            "INSERT INTO personal_trigram(earlier,previous,word,count) VALUES(?1,?2,?3,?4) ON CONFLICT(earlier,previous,word) DO UPDATE SET count=excluded.count WHERE excluded.count>count",
        )?;
        let mut pick = transaction.prepare(
            "INSERT INTO pick_transitions(previous_key,previous_value,key,value,count,updated_at) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(previous_key,previous_value,key,value) DO UPDATE SET count=excluded.count,updated_at=max(updated_at,excluded.updated_at) WHERE excluded.count>count",
        )?;
        let mut typo = transaction.prepare(
            "INSERT INTO pinyin_typo_counts(typed,intended,accepted,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(typed,intended) DO UPDATE SET accepted=excluded.accepted,updated_at=max(updated_at,excluded.updated_at) WHERE excluded.accepted>accepted",
        )?;
        let mut suppression = transaction.prepare(
            "INSERT INTO pinyin_autocorrect_suppressions(input,commits,updated_at) VALUES(?1,?2,?3) ON CONFLICT(input) DO UPDATE SET commits=excluded.commits,updated_at=max(updated_at,excluded.updated_at) WHERE excluded.commits>commits",
        )?;
        let mut pin = transaction.prepare(
            "INSERT OR IGNORE INTO pinned_candidates(context_key,value,updated_at) VALUES(?1,?2,?3)",
        )?;
        let mut count = 0usize;
        for record in records {
            let record = record?;
            count += 1;
            require(count <= maximum_records && habit_is_storable(&record))?;
            let changed = match &record {
                LearningHabit::Bigram {
                    previous,
                    word,
                    count,
                } => bigram.execute(params![previous, word, count])?,
                LearningHabit::Trigram {
                    earlier,
                    previous,
                    word,
                    count,
                } => trigram.execute(params![earlier, previous, word, count])?,
                LearningHabit::PickTransition {
                    previous_key,
                    previous_value,
                    key,
                    value,
                    count,
                    updated_at,
                } => pick.execute(params![
                    previous_key,
                    previous_value,
                    key,
                    value,
                    count,
                    updated_at
                ])?,
                LearningHabit::TypoCount {
                    typed,
                    intended,
                    accepted,
                    updated_at,
                } => typo.execute(params![typed, intended, accepted, updated_at])?,
                LearningHabit::AutocorrectSuppression {
                    input,
                    commits,
                    updated_at,
                } => suppression.execute(params![input, commits, updated_at])?,
                LearningHabit::PinnedCandidate {
                    context,
                    value,
                    updated_at,
                } => pin.execute(params![context, value, updated_at])?,
            };
            if changed == 1 {
                merge.written += 1;
            } else {
                merge.kept += 1;
            }
        }
    }
    merge.trimmed = trim_to_limits(&transaction)?;
    transaction.commit()?;
    ngram_store::invalidate_journal(&file);
    Ok(merge)
}

fn row_count(connection: &rusqlite::Connection, sql: &str) -> Result<usize> {
    let count: i64 = connection.query_row(sql, [], |row| row.get(0))?;
    Ok(usize::try_from(count).unwrap_or(0))
}

/// 合并后按各表平时的规则压回上限，返回删掉的行数。
fn trim_to_limits(connection: &rusqlite::Connection) -> Result<usize> {
    const NGRAM_ENTRIES: &str =
        "SELECT (SELECT count(*) FROM personal_bigram)+(SELECT count(*) FROM personal_trigram)";
    let mut trimmed = 0usize;
    // 与 `ngram_store::decay_locked` 相同：超过上限就反复减半，直到不超过上限的四分之三，免得接下来几次上屏又立刻衰减。
    let limit = PersonalNgramOptions::default().max_transitions;
    let before = row_count(connection, NGRAM_ENTRIES)?;
    if before > limit {
        let low_water = limit - limit / 4;
        loop {
            connection.execute_batch(ngram_store::HALVE_SQL)?;
            if row_count(connection, NGRAM_ENTRIES)? <= low_water {
                break;
            }
        }
        trimmed += before - row_count(connection, NGRAM_ENTRIES)?;
    }
    // 与 `picks::record_pick_transition` 相同：只留最近的那些。
    trimmed += connection.execute(
        "DELETE FROM pick_transitions WHERE updated_at < (SELECT updated_at FROM pick_transitions ORDER BY updated_at DESC LIMIT 1 OFFSET ?1)",
        params![MAX_PICK_TRANSITIONS as i64],
    )?;
    // 与 `typo_profile::trim_typo_table` 相同：先删最少用、最旧的。
    for (table, column) in [
        ("pinyin_typo_counts", "accepted"),
        ("pinyin_autocorrect_suppressions", "commits"),
    ] {
        trimmed += connection.execute(
            &format!(
                "DELETE FROM {table} WHERE rowid IN (SELECT rowid FROM {table} ORDER BY {column} ASC, updated_at ASC, rowid ASC LIMIT max(0, (SELECT count(*) FROM {table}) - ?1))"
            ),
            params![MAX_TYPO_STATE_ROWS as i64],
        )?;
    }
    Ok(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_dictionary::replay::tests::sql;
    use std::fs;

    fn paths(root: &Path) -> RuntimePaths {
        let path = |name: &str| {
            let path = root.join(name);
            fs::create_dir_all(&path).unwrap();
            path
        };
        RuntimePaths {
            resources: path("resources"),
            user_data: path("user"),
            cache: path("cache"),
            dictionaries: path("dictionaries"),
        }
    }

    fn journal(paths: &RuntimePaths) -> std::path::PathBuf {
        let file = paths.user(assets::USER_JOURNAL);
        let connection = open_database(
            &file,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .unwrap();
        ensure_schema(&connection).unwrap();
        file
    }

    fn integer(file: &Path, query: &str) -> i64 {
        open_database(file, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap()
            .query_row(query, [], |row| row.get(0))
            .unwrap()
    }

    fn streamed(paths: &RuntimePaths) -> Vec<LearningHabit> {
        let mut habits = Vec::new();
        stream_learning_habits(paths, &mut |habit| {
            habits.push(habit.clone());
            true
        })
        .unwrap();
        habits
    }

    fn merge(paths: &RuntimePaths, habits: Vec<LearningHabit>) -> Result<LearningHabitsMerge> {
        merge_learning_habits(paths, &mut habits.into_iter().map(Ok), 100)
    }

    /// 本机已经学到的：两条整句联想、一条选词对、一条纠错计数、一条抑制和一条置顶。
    fn local(file: &Path) {
        sql(
            file,
            "INSERT INTO personal_bigram(previous,word,count) VALUES(char(1),'我',4),('我','想',2);
             INSERT INTO personal_trigram(earlier,previous,word,count) VALUES(char(1),'我','想',2);
             INSERT INTO pick_transitions(previous_key,previous_value,key,value,count,updated_at) VALUES('wo','我','xiang','想',3,100);
             INSERT INTO pinyin_typo_counts(typed,intended,accepted,updated_at) VALUES('jai','jia',5,100);
             INSERT INTO pinyin_autocorrect_suppressions(input,commits,updated_at) VALUES('jai',2,100);
             INSERT INTO pinned_candidates(context_key,value,updated_at) VALUES('ni','你',100);",
        );
    }

    fn backup() -> Vec<LearningHabit> {
        vec![
            // 本机是 4，取大保留本机。
            LearningHabit::Bigram {
                previous: "\u{1}".into(),
                word: "我".into(),
                count: 3,
            },
            // 本机是 2，取大调成 6。
            LearningHabit::Bigram {
                previous: "我".into(),
                word: "想".into(),
                count: 6,
            },
            LearningHabit::Trigram {
                earlier: "我".into(),
                previous: "想".into(),
                word: "去".into(),
                count: 2,
            },
            LearningHabit::PickTransition {
                previous_key: "wo".into(),
                previous_value: "我".into(),
                key: "xiang".into(),
                value: "想".into(),
                count: 9,
                updated_at: 50,
            },
            LearningHabit::TypoCount {
                typed: "jai".into(),
                intended: "jia".into(),
                accepted: 1,
                updated_at: 300,
            },
            LearningHabit::AutocorrectSuppression {
                input: "nihoa".into(),
                commits: 1,
                updated_at: 300,
            },
            // 这个上下文本机已经置顶了「你」，保留本机。
            LearningHabit::PinnedCandidate {
                context: "ni".into(),
                value: "泥".into(),
                updated_at: 300,
            },
            LearningHabit::PinnedCandidate {
                context: "hao".into(),
                value: "好".into(),
                updated_at: 300,
            },
        ]
    }

    #[test]
    fn a_missing_journal_streams_nothing_and_is_not_created() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        assert!(streamed(&paths).is_empty());
        assert_eq!(count_learning_habits(&paths).unwrap(), 0);
        assert!(!paths.user(assets::USER_JOURNAL).exists());
    }

    #[test]
    fn every_table_streams_in_order_and_merges_into_a_fresh_journal_unchanged() {
        let root = tempfile::tempdir().unwrap();
        let source = paths(&root.path().join("source"));
        local(&journal(&source));
        let habits = streamed(&source);
        assert_eq!(habits.len(), 7);
        assert_eq!(count_learning_habits(&source).unwrap(), 7);
        assert!(matches!(habits[0], LearningHabit::Bigram { .. }));
        assert!(matches!(habits[6], LearningHabit::PinnedCandidate { .. }));
        assert!(habits.iter().all(habit_is_storable));

        let target = paths(&root.path().join("target"));
        let merged = merge(&target, habits.clone()).unwrap();
        assert_eq!(
            merged,
            LearningHabitsMerge {
                written: 7,
                kept: 0,
                trimmed: 0
            }
        );
        assert_eq!(streamed(&target), habits);
        // 再合并一次什么也不改：计数取大而不是相加。
        let again = merge(&target, habits.clone()).unwrap();
        assert_eq!(again.written, 0);
        assert_eq!(again.kept, 7);
        assert_eq!(streamed(&target), habits);
    }

    #[test]
    fn a_merge_keeps_local_rows_and_takes_the_larger_count() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let file = journal(&paths);
        local(&file);
        let merged = merge(&paths, backup()).unwrap();
        assert_eq!(
            merged,
            LearningHabitsMerge {
                written: 5,
                kept: 3,
                trimmed: 0
            }
        );
        let count = |query: &str| integer(&file, query);
        assert_eq!(
            count("SELECT count FROM personal_bigram WHERE previous=char(1) AND word='我'"),
            4
        );
        assert_eq!(
            count("SELECT count FROM personal_bigram WHERE previous='我' AND word='想'"),
            6
        );
        assert_eq!(count("SELECT count(*) FROM personal_trigram"), 2);
        // 选词对的计数调大，时间保留较新的本机。
        assert_eq!(
            count("SELECT count*1000+updated_at FROM pick_transitions WHERE key='xiang'"),
            9_100
        );
        assert_eq!(
            count("SELECT accepted*1000+updated_at FROM pinyin_typo_counts WHERE typed='jai'"),
            5_100
        );
        assert_eq!(
            count("SELECT count(*) FROM pinyin_autocorrect_suppressions"),
            2
        );
        assert_eq!(
            count("SELECT count(*) FROM pinned_candidates WHERE context_key='ni' AND value='你'"),
            1
        );
        assert_eq!(
            count("SELECT count(*) FROM pinned_candidates WHERE context_key='hao'"),
            1
        );
    }

    #[test]
    fn an_unstorable_record_or_a_failed_stream_changes_nothing() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let file = journal(&paths);
        local(&file);
        let before = streamed(&paths);
        let mut bad = backup();
        bad.push(LearningHabit::TypoCount {
            typed: "Jai".into(),
            intended: "jia".into(),
            accepted: 1,
            updated_at: 0,
        });
        assert!(merge(&paths, bad).is_err());
        assert_eq!(streamed(&paths), before);

        let mut failing = backup()
            .into_iter()
            .map(Ok)
            .chain(std::iter::once(Err(invalid_habits())));
        assert!(merge_learning_habits(&paths, &mut failing, 100).is_err());
        assert_eq!(streamed(&paths), before);

        // 超过记录上限同样整体回滚。
        assert!(merge_learning_habits(&paths, &mut backup().into_iter().map(Ok), 3).is_err());
        assert_eq!(streamed(&paths), before);
    }

    #[test]
    fn a_write_failure_rolls_back_the_whole_merge() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let file = journal(&paths);
        local(&file);
        let before = streamed(&paths);
        sql(
            &file,
            "CREATE TRIGGER reject_pin BEFORE INSERT ON pinned_candidates WHEN NEW.context_key='hao' BEGIN SELECT RAISE(FAIL,'injected'); END;",
        );
        assert!(merge(&paths, backup()).is_err());
        assert_eq!(streamed(&paths), before);
    }

    #[test]
    fn a_merge_past_the_limits_is_trimmed_like_ordinary_learning() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let file = journal(&paths);
        // 本机已有满额的选词对（`record_pick_transition` 留下上限加一行）和抑制记录，备份再各带来一条更新的。
        let picks = MAX_PICK_TRANSITIONS + 1;
        sql(
            &file,
            &format!(
                "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<{picks}) INSERT INTO pick_transitions(previous_key,previous_value,key,value,count,updated_at) SELECT 'p'||i,'x','k','y',1,1000+i FROM n;
                 WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<{MAX_TYPO_STATE_ROWS}) INSERT INTO pinyin_autocorrect_suppressions(input,commits,updated_at) SELECT 'a'||char(97+i%26)||char(97+(i/26)%26)||char(97+(i/676)%26),2,1000+i FROM n;"
            ),
        );
        let merged = merge(
            &paths,
            vec![
                LearningHabit::PickTransition {
                    previous_key: "new".into(),
                    previous_value: "新".into(),
                    key: "k".into(),
                    value: "y".into(),
                    count: 1,
                    updated_at: 999_999,
                },
                LearningHabit::AutocorrectSuppression {
                    input: "zzzz".into(),
                    commits: 5,
                    updated_at: 999_999,
                },
            ],
        )
        .unwrap();
        assert_eq!(merged.written, 2);
        assert_eq!(merged.trimmed, 2);
        assert_eq!(
            integer(&file, "SELECT count(*) FROM pick_transitions"),
            picks as i64
        );
        assert_eq!(
            integer(
                &file,
                "SELECT count(*) FROM pick_transitions WHERE previous_key='new'"
            ),
            1
        );
        assert_eq!(
            integer(
                &file,
                "SELECT count(*) FROM pick_transitions WHERE previous_key='p1'"
            ),
            0
        );
        assert_eq!(
            integer(
                &file,
                "SELECT count(*) FROM pinyin_autocorrect_suppressions"
            ),
            MAX_TYPO_STATE_ROWS as i64
        );
        assert_eq!(
            integer(
                &file,
                "SELECT count(*) FROM pinyin_autocorrect_suppressions WHERE input='zzzz'"
            ),
            1
        );
    }

    #[test]
    fn personal_ngrams_past_the_memory_limit_are_halved_back_under_it() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let file = journal(&paths);
        let limit = PersonalNgramOptions::default().max_transitions;
        // 本机已有 limit 条计数为 4 的二元，备份再带来一条：合并后超限，减半一次后每行是 2，仍然超过四分之三，再减半一次后计数为 1，再减半就全被丢掉。
        sql(
            &file,
            &format!(
                "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<{limit}) INSERT INTO personal_bigram(previous,word,count) SELECT 'p'||i,'w',4 FROM n;"
            ),
        );
        let merged = merge(
            &paths,
            vec![LearningHabit::Bigram {
                previous: "新".into(),
                word: "词".into(),
                count: 64,
            }],
        )
        .unwrap();
        let remaining = integer(&file, "SELECT count(*) FROM personal_bigram") as usize;
        assert!(remaining <= limit - limit / 4);
        assert_eq!(merged.trimmed, limit + 1 - remaining);
        assert!(
            integer(
                &file,
                "SELECT count FROM personal_bigram WHERE previous='新'"
            ) >= 1,
            "常用的那条留下"
        );
    }
}
