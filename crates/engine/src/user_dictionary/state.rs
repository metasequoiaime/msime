//! Dictionary-state export and import (user-dictionary.md §14, api-contract §5.13): one consistent read of the journal's entries, fixed positions and selection counts, its SHA-256 revision, and staging a complete replacement generation from a record stream.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use rusqlite::{params, OpenFlags, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::assets;
use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::paths::RuntimePaths;
use crate::types::{PersonalDictionaryKind, SchemeSet};
use crate::user_dictionary::generation::{
    prepare_runtime_paths_for, roots_overlap, weakly_canonical,
};
use crate::user_dictionary::journal::{ensure_schema, open_database};

pub const DEFAULT_MAXIMUM_RECORDS: usize = 500_000;

const MAX_KEY_BYTES: usize = 512;
const MAX_VALUE_BYTES: usize = 4_096;
const MAX_STAGED_WEIGHT: i64 = 100_000_000;
const MAX_STAGED_SELECTION_COUNT: i64 = 10;

/// The domain separator every revision starts with; Apple's `DictionaryStateRevision` uses the same bytes.
const REVISION_DOMAIN: &str = "msime-local-dictionary-state-v1";

/// The host's record shape (engine-bridge `DictionaryStateRecord`); positions and counts are range-checked on import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictionaryStateRecord {
    Entry {
        kind: PersonalDictionaryKind,
        key: String,
        value: String,
        weight: i64,
        display: String,
        deleted: bool,
        user_inserted: bool,
    },
    Position {
        context: String,
        key: String,
        value: String,
        position: i64,
    },
    Selection {
        context: String,
        key: String,
        value: String,
        count: i64,
    },
}

/// Every failure reading or staging a state has the one message the reference's `require` threw (DS:18-22): hosts show it as is, and the cause is not user-actionable.
fn invalid_state() -> EngineError {
    EngineError::failed(diagnostics::INVALID_DICTIONARY_STATE)
}

fn require(condition: bool) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(invalid_state())
    }
}

/// Operations by (dictionary, key, value), then positions, then selections, in one read transaction with `query_only` (DS:93-138). `emit` returning false stops. A missing journal emits nothing and is not created; a corrupt one is an error and is left untouched.
///
/// Stopping is reported as `INVALID_DICTIONARY_STATE`, as the reference threw for a cancelled export. Text is read strictly: a row that is not UTF-8 is an invalid state rather than a lossy copy, because an import would otherwise store different bytes than were exported.
pub fn stream_dictionary_state(
    paths: &RuntimePaths,
    emit: &mut dyn FnMut(&DictionaryStateRecord) -> bool,
) -> Result<()> {
    paths.validate()?;
    let file = paths.user(assets::USER_JOURNAL);
    if !file.exists() {
        return Ok(());
    }
    read_state(&file, emit).map_err(|error| match error {
        EngineError::Sqlite(_) => invalid_state(),
        other => other,
    })
}

fn read_state(file: &Path, emit: &mut dyn FnMut(&DictionaryStateRecord) -> bool) -> Result<()> {
    // SQLite may need to create WAL coordination files even for a reader, so the connection may write files, but `query_only` forbids changing the journal itself (DS:98-102).
    let mut connection = open_database(file, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    connection.execute_batch("PRAGMA query_only=ON")?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Deferred)?;
    let mut records = 0usize;
    {
        let mut rows = transaction.prepare(
            "SELECT dictionary,key,value,weight,display,operation,user_inserted,
                    length(CAST(dictionary AS BLOB)),length(CAST(key AS BLOB)),
                    length(CAST(value AS BLOB)),length(CAST(display AS BLOB)),
                    length(CAST(operation AS BLOB))
             FROM user_dictionary_operations ORDER BY dictionary,key,value",
        )?;
        let mut cursor = rows.query([])?;
        while let Some(row) = cursor.next()? {
            require(records < DEFAULT_MAXIMUM_RECORDS)?;
            require(
                row.get::<_, Option<i64>>(7)?
                    .is_some_and(|length| (0..=32).contains(&length))
                    && row
                        .get::<_, Option<i64>>(8)?
                        .is_some_and(|length| (0..=MAX_KEY_BYTES as i64).contains(&length))
                    && row
                        .get::<_, Option<i64>>(9)?
                        .is_some_and(|length| (0..=MAX_VALUE_BYTES as i64).contains(&length))
                    && row
                        .get::<_, Option<i64>>(10)?
                        .is_some_and(|length| (0..=MAX_VALUE_BYTES as i64).contains(&length))
                    && row
                        .get::<_, Option<i64>>(11)?
                        .is_some_and(|length| (0..=16).contains(&length)),
            )?;
            let kind = PersonalDictionaryKind::from_journal_name(&row.get::<_, String>(0)?)
                .ok_or_else(invalid_state)?;
            // `sqlite3_column_int64` reads NULL as 0; legacy journals built without NOT NULL can hold one.
            let record = DictionaryStateRecord::Entry {
                kind,
                key: row.get(1)?,
                value: row.get(2)?,
                weight: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                display: row.get(4)?,
                deleted: row.get::<_, String>(5)? == "delete",
                user_inserted: row.get::<_, Option<i64>>(6)?.unwrap_or(0) != 0,
            };
            require(emit(&record))?;
            records += 1;
        }
    }
    for fixed in [true, false] {
        let mut rows = transaction.prepare(if fixed {
            "SELECT context_key,entry_key,value,position,
                    length(CAST(context_key AS BLOB)),length(CAST(entry_key AS BLOB)),
                    length(CAST(value AS BLOB))
             FROM fixed_candidate_positions ORDER BY context_key,entry_key,value"
        } else {
            "SELECT context_key,entry_key,value,selection_count,
                    length(CAST(context_key AS BLOB)),length(CAST(entry_key AS BLOB)),
                    length(CAST(value AS BLOB))
             FROM candidate_selection_state ORDER BY context_key,entry_key,value"
        })?;
        let mut cursor = rows.query([])?;
        while let Some(row) = cursor.next()? {
            require(records < DEFAULT_MAXIMUM_RECORDS)?;
            require(
                row.get::<_, Option<i64>>(4)?
                    .is_some_and(|length| (0..=MAX_KEY_BYTES as i64).contains(&length))
                    && row
                        .get::<_, Option<i64>>(5)?
                        .is_some_and(|length| (0..=MAX_KEY_BYTES as i64).contains(&length))
                    && row
                        .get::<_, Option<i64>>(6)?
                        .is_some_and(|length| (0..=MAX_VALUE_BYTES as i64).contains(&length)),
            )?;
            let context: String = row.get(0)?;
            let key: String = row.get(1)?;
            let value: String = row.get(2)?;
            let number = row.get::<_, Option<i64>>(3)?.unwrap_or(0);
            let record = if fixed {
                DictionaryStateRecord::Position {
                    context,
                    key,
                    value,
                    position: number,
                }
            } else {
                DictionaryStateRecord::Selection {
                    context,
                    key,
                    value,
                    count: number,
                }
            };
            require(emit(&record))?;
            records += 1;
        }
    }
    transaction.commit()?;
    Ok(())
}

/// SHA-256 hex over `text("msime-local-dictionary-state-v1")` and every streamed record, where `text(s)` is `integer(len)` then the bytes and `integer` is a big-endian u64 (dictionary_revision.rs). Apple computes the same digest; keep the framing byte for byte.
pub fn dictionary_state_revision(paths: &RuntimePaths) -> Result<String> {
    let mut revision = Revision::new();
    stream_dictionary_state(paths, &mut |record| {
        revision.record(record);
        true
    })?;
    Ok(revision.finish())
}

/// The framing of `dictionary_revision.rs` and `hash_dictionary_state` (bridge.cpp:441-466).
struct Revision(Sha256);

impl Revision {
    fn new() -> Self {
        let mut revision = Self(Sha256::new());
        revision.text(REVISION_DOMAIN);
        revision
    }

    fn integer(&mut self, value: u64) {
        self.0.update(value.to_be_bytes());
    }

    fn text(&mut self, value: &str) {
        self.integer(value.len() as u64);
        self.0.update(value.as_bytes());
    }

    /// Negative numbers are hashed as their two's-complement `u64`, the reference's `static_cast<uint64_t>`.
    fn record(&mut self, record: &DictionaryStateRecord) {
        match record {
            DictionaryStateRecord::Entry {
                kind,
                key,
                value,
                weight,
                display,
                deleted,
                user_inserted,
            } => {
                self.text("entry");
                self.text(kind.journal_name());
                self.text(key);
                self.text(value);
                self.integer(*weight as u64);
                self.text(display);
                self.integer(u64::from(*deleted));
                self.integer(u64::from(*user_inserted));
            }
            DictionaryStateRecord::Position {
                context,
                key,
                value,
                position,
            } => {
                self.text("position");
                self.text(context);
                self.text(key);
                self.text(value);
                self.integer(*position as u64);
            }
            DictionaryStateRecord::Selection {
                context,
                key,
                value,
                count,
            } => {
                self.text("selection");
                self.text(context);
                self.text(key);
                self.text(value);
                self.integer(*count as u64);
            }
        }
    }

    fn finish(self) -> String {
        hex::encode(self.0.finalize())
    }
}

/// Build a complete replacement in an exclusively created `generation/<content_id>` from `records`, consumed lazily. `None` from the iterator is verified EOF; an `Err` aborts. Duplicates and more than `maximum_records` records are rejected. Failure removes only the new directory (DS:144-231).
///
/// The journal lands in `generation/user` and the rebuilt dictionaries in `generation/user/dictionaries/<content_id>`; resources are copied afresh, so a replacement never inherits the dictionaries it replaces.
pub fn stage_dictionary_state(
    resources: &Path,
    generation: &Path,
    content_id: &str,
    records: &mut dyn Iterator<Item = Result<DictionaryStateRecord>>,
    maximum_records: usize,
) -> Result<RuntimePaths> {
    stage_dictionary_state_for(
        resources,
        generation,
        content_id,
        records,
        maximum_records,
        SchemeSet::ALL,
    )
}

/// [`stage_dictionary_state`]，词库按 `schemes` 准备（`prepare_runtime_paths_for`）：不读 `msime-pinyin.db` 的集合只复制 `msime-english.db`，记录照收，回放时只有英文行写进词库。
pub fn stage_dictionary_state_for(
    resources: &Path,
    generation: &Path,
    content_id: &str,
    records: &mut dyn Iterator<Item = Result<DictionaryStateRecord>>,
    maximum_records: usize,
    schemes: SchemeSet,
) -> Result<RuntimePaths> {
    require(resources.is_absolute() && generation.is_absolute())?;
    require(!roots_overlap(
        &weakly_canonical(resources)?,
        &weakly_canonical(generation)?,
    ))?;
    // Exclusive creation: an existing generation, published or not, is never touched (DS:150).
    match fs::create_dir(generation) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => return Err(invalid_state()),
        Err(error) => return Err(error.into()),
    }
    let staged = build_generation(
        resources,
        generation,
        content_id,
        records,
        maximum_records,
        schemes,
    );
    if staged.is_err() {
        // The directory is this call's own; the caller needs the error that stopped the staging, not whether its cleanup also failed (DS:225-230).
        let _ = fs::remove_dir_all(generation);
    }
    staged
}

fn build_generation(
    resources: &Path,
    generation: &Path,
    content_id: &str,
    records: &mut dyn Iterator<Item = Result<DictionaryStateRecord>>,
    maximum_records: usize,
    schemes: SchemeSet,
) -> Result<RuntimePaths> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(generation, fs::Permissions::from_mode(0o700))?;
    }
    let user = generation.join("user");
    fs::create_dir(&user)?;
    let journal = user.join(assets::USER_JOURNAL);
    // A connection of its own rather than the cached journal: this file is deleted with the directory if staging fails, and a cached handle would keep it open.
    let mut connection = open_database(
        &journal,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )?;
    ensure_schema(&connection).map_err(|_| invalid_state())?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| invalid_state())?;
    {
        // Plain INSERTs: the primary keys and `UNIQUE(context_key,position)` reject duplicate identities and slots instead of letting a later record overwrite an earlier one.
        let mut entry = transaction
            .prepare("INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted) VALUES(?1,?2,?3,?4,?5,?6,?7)")
            .map_err(|_| invalid_state())?;
        let mut position = transaction
            .prepare("INSERT INTO fixed_candidate_positions(context_key,entry_key,value,position) VALUES(?1,?2,?3,?4)")
            .map_err(|_| invalid_state())?;
        let mut selection = transaction
            .prepare("INSERT INTO candidate_selection_state(context_key,entry_key,value,selection_count) VALUES(?1,?2,?3,?4)")
            .map_err(|_| invalid_state())?;
        let mut count = 0usize;
        for record in records {
            let record = record?;
            count += 1;
            require(count <= maximum_records)?;
            let inserted = match &record {
                DictionaryStateRecord::Entry {
                    kind,
                    key,
                    value,
                    weight,
                    display,
                    deleted,
                    user_inserted,
                } => {
                    require(
                        bounded(key, MAX_KEY_BYTES, false)
                            && bounded(value, MAX_VALUE_BYTES, false),
                    )?;
                    require(bounded(display, MAX_VALUE_BYTES, true))?;
                    require((0..=MAX_STAGED_WEIGHT).contains(weight))?;
                    entry.execute(params![
                        kind.journal_name(),
                        key,
                        value,
                        if *deleted { "delete" } else { "upsert" },
                        weight,
                        display,
                        i64::from(*user_inserted),
                    ])
                }
                DictionaryStateRecord::Position {
                    context,
                    key,
                    value,
                    position: slot,
                } => {
                    require(
                        bounded(key, MAX_KEY_BYTES, false)
                            && bounded(value, MAX_VALUE_BYTES, false),
                    )?;
                    require(bounded(context, MAX_KEY_BYTES, false))?;
                    require((1..=5).contains(slot))?;
                    position.execute(params![context, key, value, slot])
                }
                DictionaryStateRecord::Selection {
                    context,
                    key,
                    value,
                    count: selections,
                } => {
                    require(
                        bounded(key, MAX_KEY_BYTES, false)
                            && bounded(value, MAX_VALUE_BYTES, false),
                    )?;
                    require(bounded(context, MAX_KEY_BYTES, false))?;
                    require((0..=MAX_STAGED_SELECTION_COUNT).contains(selections))?;
                    selection.execute(params![context, key, value, selections])
                }
            };
            inserted.map_err(|_| invalid_state())?;
        }
    }
    transaction.commit().map_err(|_| invalid_state())?;
    drop(connection);
    prepare_runtime_paths_for(
        resources,
        &user,
        &generation.join("cache"),
        content_id,
        schemes,
    )
}

/// [`merge_dictionary_state`] 的结果：各类记录写进本机的条数，以及没写的条数和原因。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DictionaryStateMerge {
    /// 写进来的学习调权（`user_inserted = 0` 的 upsert）和删除记录。
    pub entries: usize,
    /// 写进来的固定位置。
    pub positions: usize,
    /// 新增或调大的选词计数。
    pub selections: usize,
    /// 本机已有、按规则保留本机的记录：同一个词本机已有日志行，同一个词或同一个位置本机已固定，本机的选词计数不小于记录里的。
    pub kept: usize,
    /// 用户自己的词（`user_inserted = 1` 的 upsert）。这里不写，由调用方经个人词库队列合并。
    pub words: usize,
    /// 这一代存不下的记录：没有中文词库的版本里的非英文行、编码拼不出拼音表名或词库里没有那张表的行、权重低于 1 的拼音调权（回放同样跳过它）。
    pub skipped: usize,
}

/// 把 `records`（一份词库状态，通常是本地备份里的快照）合并进本机正在用的日志和词库，而不是像 [`stage_dictionary_state`] 那样另建一代整份替换。全部在一个 IMMEDIATE 事务里完成，任何一条记录不合规、读流失败或写库出错（磁盘满、I/O 失败等）都整体回滚。
///
/// 合并规则是本机优先：学习调权和删除记录只在本机日志里还没有这个词（同一 `dictionary,key,value`）时写入，写法与个人词库编辑相同（先改工作词库，再记日志，`user_inserted` 照记录保留）；固定位置在本机既没有固定这个词、这个位置也没被占用时才写入；选词计数取两边较大的那个。用户自己的词不在这里写，只计数。`main_dictionary` 为假（代次里没有 `msime-pinyin.db`）时只写英文行，其余跳过。
///
/// 调用方必须持有词库的独占维护权：这里直接改工作词库，与会话的写入不能并发。
pub fn merge_dictionary_state(
    paths: &RuntimePaths,
    main_dictionary: bool,
    records: &mut dyn Iterator<Item = Result<DictionaryStateRecord>>,
    maximum_records: usize,
) -> Result<DictionaryStateMerge> {
    use super::personal::open_edit_connection;
    use super::replay::{apply_english, apply_pinyin, apply_simple};
    use rusqlite::OptionalExtension;

    paths.validate()?;
    let mut connection = open_edit_connection(paths, main_dictionary)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut merge = DictionaryStateMerge::default();
    {
        let mut existing = transaction.prepare(
            "SELECT 1 FROM personal_journal.user_dictionary_operations WHERE dictionary=?1 AND key=?2 AND value=?3",
        )?;
        let mut journal = transaction.prepare(
            "INSERT INTO personal_journal.user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        )?;
        // 同一个词已固定，或同一个位置已被占用，都由主键和 `UNIQUE(context_key,position)` 拒绝，`OR IGNORE` 让本机的那条留下。
        let mut position = transaction.prepare(
            "INSERT OR IGNORE INTO personal_journal.fixed_candidate_positions(context_key,entry_key,value,position) VALUES(?1,?2,?3,?4)",
        )?;
        let mut selection = transaction.prepare(
            "INSERT INTO personal_journal.candidate_selection_state(context_key,entry_key,value,selection_count) VALUES(?1,?2,?3,?4) ON CONFLICT(context_key,entry_key,value) DO UPDATE SET selection_count=excluded.selection_count WHERE excluded.selection_count>selection_count",
        )?;
        let mut count = 0usize;
        for record in records {
            let record = record?;
            count += 1;
            require(count <= maximum_records)?;
            match &record {
                DictionaryStateRecord::Entry {
                    kind,
                    key,
                    value,
                    weight,
                    display,
                    deleted,
                    user_inserted,
                } => {
                    require(
                        bounded(key, MAX_KEY_BYTES, false)
                            && bounded(value, MAX_VALUE_BYTES, false)
                            && bounded(display, MAX_VALUE_BYTES, true)
                            && (0..=MAX_STAGED_WEIGHT).contains(weight),
                    )?;
                    if *user_inserted && !*deleted {
                        merge.words += 1;
                        continue;
                    }
                    if (!main_dictionary && *kind != PersonalDictionaryKind::English)
                        || (!*deleted && *kind == PersonalDictionaryKind::Pinyin && *weight < 1)
                    {
                        merge.skipped += 1;
                        continue;
                    }
                    let name = kind.journal_name();
                    if existing
                        .query_row(params![name, key, value], |_| Ok(()))
                        .optional()?
                        .is_some()
                    {
                        merge.kept += 1;
                        continue;
                    }
                    let (key_bytes, value_bytes) = (key.as_bytes(), value.as_bytes());
                    // 编码拼不出表名（`Ok(false)`），或这一代词库里没有那张表（准备语句时报 no such table，什么也没执行），这一行就存不下，计入 skipped。别的 SQL 错误（磁盘满、I/O 失败、库损坏、约束）照常上抛，整个合并回滚，调用方留着待合并的文件下次再试，而不是把没写进去的记录当成跳过。
                    let applied = match kind {
                        PersonalDictionaryKind::Pinyin => {
                            apply_pinyin(&transaction, key_bytes, value_bytes, *deleted, *weight)
                        }
                        PersonalDictionaryKind::Wubi | PersonalDictionaryKind::Wubi98 => {
                            apply_simple(
                                &transaction,
                                kind.wubi_table().unwrap_or_default(),
                                key_bytes,
                                value_bytes,
                                *deleted,
                                *weight,
                            )
                        }
                        PersonalDictionaryKind::QuickPhrase => apply_simple(
                            &transaction,
                            "quick_parases",
                            key_bytes,
                            value_bytes,
                            *deleted,
                            *weight,
                        ),
                        PersonalDictionaryKind::English => apply_english(
                            &transaction,
                            key_bytes,
                            value_bytes,
                            *deleted,
                            *weight,
                            display.as_bytes(),
                        ),
                    };
                    let applied = match applied {
                        Ok(applied) => applied,
                        Err(error) if missing_table(&error) => false,
                        Err(error) => return Err(error.into()),
                    };
                    if !applied {
                        merge.skipped += 1;
                        continue;
                    }
                    journal.execute(params![
                        name,
                        key,
                        value,
                        if *deleted { "delete" } else { "upsert" },
                        weight,
                        display,
                        i64::from(*user_inserted),
                    ])?;
                    merge.entries += 1;
                }
                DictionaryStateRecord::Position {
                    context,
                    key,
                    value,
                    position: slot,
                } => {
                    require(
                        bounded(context, MAX_KEY_BYTES, false)
                            && bounded(key, MAX_KEY_BYTES, false)
                            && bounded(value, MAX_VALUE_BYTES, false)
                            && (1..=5).contains(slot),
                    )?;
                    if position.execute(params![context, key, value, slot])? == 1 {
                        merge.positions += 1;
                    } else {
                        merge.kept += 1;
                    }
                }
                DictionaryStateRecord::Selection {
                    context,
                    key,
                    value,
                    count: selections,
                } => {
                    require(
                        bounded(context, MAX_KEY_BYTES, false)
                            && bounded(key, MAX_KEY_BYTES, false)
                            && bounded(value, MAX_VALUE_BYTES, false)
                            && (0..=MAX_STAGED_SELECTION_COUNT).contains(selections),
                    )?;
                    if selection.execute(params![context, key, value, selections])? == 1 {
                        merge.selections += 1;
                    } else {
                        merge.kept += 1;
                    }
                }
            }
        }
    }
    transaction.commit()?;
    Ok(merge)
}

/// 语句准备时报「没有这张表」：SQLite 对它用通用的 `SQLITE_ERROR`，只能看消息区分。这时语句一条也没执行，跳过这一行不会留下半截修改。
fn missing_table(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(failure, Some(message))
            if failure.code == rusqlite::ErrorCode::Unknown && message.starts_with("no such table")
    )
}

/// Non-empty unless `allow_empty`, at most `maximum` bytes and free of NUL (DS:82-86); `String` already guarantees UTF-8.
fn bounded(text: &str, maximum: usize, allow_empty: bool) -> bool {
    (allow_empty || !text.is_empty()) && text.len() <= maximum && !text.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_dictionary::replay::tests::{sql, weight};
    use rusqlite::Connection;
    use std::cell::Cell;
    use std::path::PathBuf;
    use std::rc::Rc;

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

    // dictionary_revision.rs `framing_is_length_prefixed_and_big_endian`.
    #[test]
    fn framing_is_length_prefixed_and_big_endian() {
        let mut actual = Revision::new();
        actual.text("synthetic");
        actual.integer(258);
        let mut expected = Sha256::new();
        let domain = b"msime-local-dictionary-state-v1";
        expected.update((domain.len() as u64).to_be_bytes());
        expected.update(domain);
        expected.update(9_u64.to_be_bytes());
        expected.update(b"synthetic");
        expected.update([0, 0, 0, 0, 0, 0, 1, 2]);
        assert_eq!(actual.finish(), hex::encode(expected.finalize()));
    }

    #[test]
    fn text_boundaries_cannot_collide() {
        let mut first = Revision::new();
        first.text("ab");
        first.text("c");
        let mut second = Revision::new();
        second.text("a");
        second.text("bc");
        assert_ne!(first.finish(), second.finish());
    }

    // dictionary_revision.rs `real_journal_covers_every_record_and_field_without_mutation`, on the legacy journal shape the bridge tests use.
    #[test]
    fn real_journal_covers_every_record_and_field_without_mutation() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let journal = paths.user(assets::USER_JOURNAL);
        sql(
            &journal,
            "CREATE TABLE user_dictionary_operations (
                dictionary TEXT, key TEXT, value TEXT, weight INTEGER,
                display TEXT, operation TEXT, user_inserted INTEGER);
             CREATE TABLE fixed_candidate_positions (
                context_key TEXT, entry_key TEXT, value TEXT, position INTEGER);
             CREATE TABLE candidate_selection_state (
                context_key TEXT, entry_key TEXT, value TEXT, selection_count INTEGER);
             INSERT INTO user_dictionary_operations VALUES
                ('wubi', 'synthetic-key', '测试', 258, 'fixture', 'upsert', 1),
                ('quick', 'synthetic-key', '测试', 258, 'fixture', 'upsert', 1),
                ('pinyin', 'synthetic-key', '测试', 258, 'fixture', 'upsert', 1),
                ('english', 'synthetic-key', '测试', -2, 'fixture', 'delete', 0);
             INSERT INTO fixed_candidate_positions VALUES ('ctx', 'key', '测试', 3);
             INSERT INTO candidate_selection_state VALUES ('ctx', 'key', '测试', 7);",
        );
        let mut expected = Revision::new();
        for kind in ["english", "pinyin", "quick", "wubi"] {
            expected.text("entry");
            expected.text(kind);
            expected.text("synthetic-key");
            expected.text("测试");
            expected.integer(if kind == "english" {
                (-2_i64) as u64
            } else {
                258
            });
            expected.text("fixture");
            expected.integer(u64::from(kind == "english"));
            expected.integer(u64::from(kind != "english"));
        }
        for (kind, number) in [("position", 3), ("selection", 7)] {
            expected.text(kind);
            expected.text("ctx");
            expected.text("key");
            expected.text("测试");
            expected.integer(number);
        }
        let original = fs::read(&journal).unwrap();
        let revision = dictionary_state_revision(&paths).unwrap();
        assert_eq!(revision, expected.finish());
        assert_eq!(fs::read(&journal).unwrap(), original);
        for update in [
            "UPDATE user_dictionary_operations SET dictionary='quick' WHERE dictionary='english'",
            "UPDATE user_dictionary_operations SET key='changed'",
            "UPDATE user_dictionary_operations SET value='changed'",
            "UPDATE user_dictionary_operations SET weight=259",
            "UPDATE user_dictionary_operations SET display='changed'",
            "UPDATE user_dictionary_operations SET operation='delete'",
            "UPDATE user_dictionary_operations SET user_inserted=0",
            "UPDATE fixed_candidate_positions SET context_key='changed'",
            "UPDATE fixed_candidate_positions SET entry_key='changed'",
            "UPDATE fixed_candidate_positions SET value='changed'",
            "UPDATE fixed_candidate_positions SET position=4",
            "UPDATE candidate_selection_state SET context_key='changed'",
            "UPDATE candidate_selection_state SET entry_key='changed'",
            "UPDATE candidate_selection_state SET value='changed'",
            "UPDATE candidate_selection_state SET selection_count=8",
        ] {
            sql(&journal, update);
            assert_ne!(
                dictionary_state_revision(&paths).unwrap(),
                revision,
                "{update}"
            );
            fs::write(&journal, &original).unwrap();
            assert_eq!(dictionary_state_revision(&paths).unwrap(), revision);
        }
    }

    // engine-bridge tests.rs `dictionary_revision_uses_real_journal_and_rejects_corruption`.
    #[test]
    fn a_missing_journal_is_not_created_and_a_corrupt_one_is_not_rewritten() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let before = dictionary_state_revision(&paths).unwrap();
        assert_eq!(before.len(), 64);
        assert_eq!(before, dictionary_state_revision(&paths).unwrap());
        let journal = paths.user(assets::USER_JOURNAL);
        assert!(!journal.exists());
        fs::write(&journal, b"synthetic invalid database").unwrap();
        let error = dictionary_state_revision(&paths).unwrap_err();
        assert_eq!(error.to_string(), diagnostics::INVALID_DICTIONARY_STATE);
        assert_eq!(fs::read(&journal).unwrap(), b"synthetic invalid database");
    }

    #[test]
    fn unknown_kinds_text_that_is_not_utf8_and_a_stopped_export_are_invalid() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let journal = paths.user(assets::USER_JOURNAL);
        sql(
            &journal,
            "CREATE TABLE user_dictionary_operations(dictionary TEXT,key TEXT,value TEXT,weight INTEGER,display TEXT,operation TEXT,user_inserted INTEGER);
             CREATE TABLE fixed_candidate_positions(context_key TEXT,entry_key TEXT,value TEXT,position INTEGER);
             CREATE TABLE candidate_selection_state(context_key TEXT,entry_key TEXT,value TEXT,selection_count INTEGER);
             INSERT INTO user_dictionary_operations VALUES('pinyin','ni','你',1,'','upsert',1);",
        );
        let mut seen = 0;
        let stopped = stream_dictionary_state(&paths, &mut |_| {
            seen += 1;
            false
        });
        assert_eq!(
            stopped.unwrap_err().to_string(),
            diagnostics::INVALID_DICTIONARY_STATE
        );
        assert_eq!(seen, 1);
        for corruption in [
            "INSERT INTO user_dictionary_operations VALUES('japanese','ni','你',1,'','upsert',1)",
            "INSERT INTO user_dictionary_operations VALUES('wubi','ni',CAST(x'ff' AS TEXT),1,'','upsert',1)",
            "INSERT INTO user_dictionary_operations VALUES('wubi','ni',NULL,1,'','upsert',1)",
        ] {
            let original = fs::read(&journal).unwrap();
            sql(&journal, corruption);
            let error = dictionary_state_revision(&paths).unwrap_err();
            assert_eq!(error.to_string(), diagnostics::INVALID_DICTIONARY_STATE, "{corruption}");
            fs::write(&journal, &original).unwrap();
        }
        assert!(dictionary_state_revision(&paths).is_ok());
    }

    #[test]
    fn oversized_state_text_is_rejected_before_export() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        let journal = paths.user(assets::USER_JOURNAL);
        sql(
            &journal,
            "CREATE TABLE user_dictionary_operations(dictionary TEXT,key TEXT,value TEXT,weight INTEGER,display TEXT,operation TEXT,user_inserted INTEGER);
             CREATE TABLE fixed_candidate_positions(context_key TEXT,entry_key TEXT,value TEXT,position INTEGER);
             CREATE TABLE candidate_selection_state(context_key TEXT,entry_key TEXT,value TEXT,selection_count INTEGER);",
        );
        let huge = "x".repeat(MAX_KEY_BYTES + 1);
        Connection::open(&journal)
            .unwrap()
            .execute(
                "INSERT INTO user_dictionary_operations VALUES('pinyin',?1,'你',1,'','upsert',1)",
                [&huge],
            )
            .unwrap();

        assert!(dictionary_state_revision(&paths).is_err());
    }

    fn resources(root: &Path) -> PathBuf {
        let resources = root.join("resources");
        fs::create_dir_all(&resources).unwrap();
        // Synthetic resource schema of the pinned test_dictionary_state.cpp.
        sql(
            &resources.join(assets::MAIN_DICTIONARY),
            "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
             INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',100),('ni''hao','nh','拟好',80);
             CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
             CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);
             CREATE INDEX idx_quick_parases_key_weight ON quick_parases(key,weight DESC);",
        );
        sql(
            &resources.join(assets::ENGLISH_DICTIONARY),
            "CREATE TABLE english_words(word TEXT COLLATE BINARY NOT NULL,display TEXT NOT NULL,
             weight INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(word,display)) WITHOUT ROWID;
             CREATE TABLE en_zh_glosses(english TEXT COLLATE BINARY PRIMARY KEY,chinese_gloss TEXT NOT NULL) WITHOUT ROWID;
             CREATE TABLE zh_en_glosses(chinese TEXT COLLATE BINARY PRIMARY KEY,english_gloss TEXT NOT NULL) WITHOUT ROWID;
             PRAGMA user_version=3;",
        );
        resources
    }

    fn entry(
        kind: PersonalDictionaryKind,
        key: &str,
        value: &str,
        weight: i64,
        display: &str,
        deleted: bool,
        user_inserted: bool,
    ) -> DictionaryStateRecord {
        DictionaryStateRecord::Entry {
            kind,
            key: key.into(),
            value: value.into(),
            weight,
            display: display.into(),
            deleted,
            user_inserted,
        }
    }

    /// engine-bridge dictionary_stage/tests.rs `records()`.
    fn records() -> Vec<DictionaryStateRecord> {
        use PersonalDictionaryKind::*;
        vec![
            entry(Pinyin, "ni'hao", "你好", 0, "", true, true),
            entry(Pinyin, "ni'hao", "拟好", 200, "", false, false),
            entry(Pinyin, "ni'hao", "拟蒿", 150, "", false, true),
            entry(Wubi, "fixture", "合成五笔", 100, "", false, true),
            entry(QuickPhrase, "fixture", "合成短语", 100, "", false, true),
            entry(
                English,
                "cloudfixture",
                "Cloudfixture",
                100,
                "Cloudfixture",
                false,
                true,
            ),
            DictionaryStateRecord::Position {
                context: "ni'hao".into(),
                key: "ni'hao".into(),
                value: "拟蒿".into(),
                position: 1,
            },
            DictionaryStateRecord::Selection {
                context: "ni'hao".into(),
                key: "ni'hao".into(),
                value: "拟好".into(),
                count: 7,
            },
        ]
    }

    fn stage(
        resources: &Path,
        generation: &Path,
        records: Vec<DictionaryStateRecord>,
    ) -> Result<RuntimePaths> {
        stage_dictionary_state(
            resources,
            generation,
            "fixture",
            &mut records.into_iter().map(Ok),
            100,
        )
    }

    fn streamed(paths: &RuntimePaths) -> Vec<DictionaryStateRecord> {
        let mut records = Vec::new();
        stream_dictionary_state(paths, &mut |record| {
            records.push(record.clone());
            true
        })
        .unwrap();
        records
    }

    // dictionary_stage/tests.rs `complete_state_rebuild_changes_real_engine_candidates_without_touching_source`: the rebuilt tables row by row; `a_session_on_a_staged_generation_offers_its_candidates` checks what a session makes of them.
    #[test]
    fn a_complete_state_rebuild_replays_into_fresh_dictionaries_without_touching_resources() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let original = fs::read(resources.join(assets::MAIN_DICTIONARY)).unwrap();
        let restored = stage(&resources, &root.path().join("restored"), records()).unwrap();
        assert_eq!(restored.resources, resources);
        assert_eq!(restored.user_data, root.path().join("restored/user"));
        assert_eq!(restored.cache, root.path().join("restored/cache"));
        assert_eq!(
            restored.dictionaries,
            root.path().join("restored/user/dictionaries/fixture")
        );
        let journal = restored.user(assets::USER_JOURNAL);
        assert_eq!(
            weight(&journal, "SELECT count(*) FROM user_dictionary_operations"),
            Some(6)
        );
        let deletion: (String, i64) = Connection::open(&journal)
            .unwrap()
            .query_row(
                "SELECT operation,user_inserted FROM user_dictionary_operations WHERE value='你好'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(deletion, ("delete".into(), 1));
        assert_eq!(
            weight(
                &journal,
                "SELECT selection_count FROM candidate_selection_state"
            ),
            Some(7)
        );
        assert_eq!(
            weight(
                &journal,
                "SELECT position FROM fixed_candidate_positions WHERE value='拟蒿'"
            ),
            Some(1)
        );
        let main = restored.dictionary(assets::MAIN_DICTIONARY);
        assert_eq!(
            weight(&main, "SELECT count(*) FROM tbl_2_n WHERE value='你好'"),
            Some(0)
        );
        assert_eq!(
            weight(&main, "SELECT weight FROM tbl_2_n WHERE value='拟好'"),
            Some(200)
        );
        assert_eq!(
            weight(&main, "SELECT weight FROM tbl_2_n WHERE value='拟蒿'"),
            Some(150)
        );
        assert_eq!(
            weight(&main, "SELECT weight FROM wubi86 WHERE value='合成五笔'"),
            Some(100)
        );
        assert_eq!(
            weight(
                &main,
                "SELECT weight FROM quick_parases WHERE value='合成短语'"
            ),
            Some(100)
        );
        assert_eq!(
            weight(&restored.dictionary(assets::ENGLISH_DICTIONARY), "SELECT weight FROM english_words WHERE word='cloudfixture' AND display='Cloudfixture'"),
            Some(100)
        );
        assert_eq!(
            fs::read(resources.join(assets::MAIN_DICTIONARY)).unwrap(),
            original
        );

        // Export and restage round-trip every record, ownership of tombstones included (test_dictionary_state.cpp:86-128).
        let exported = streamed(&restored);
        assert_eq!(exported.len(), records().len());
        let second = stage(&resources, &root.path().join("second"), exported.clone()).unwrap();
        assert_eq!(streamed(&second), exported);
        assert_eq!(
            dictionary_state_revision(&second).unwrap(),
            dictionary_state_revision(&restored).unwrap()
        );

        // An empty replacement starts from the immutable resources, never the restored overlay.
        let empty = stage(&resources, &root.path().join("empty"), vec![]).unwrap();
        assert_eq!(
            weight(
                &empty.dictionary(assets::MAIN_DICTIONARY),
                "SELECT weight FROM tbl_2_n WHERE value='你好'"
            ),
            Some(100)
        );
        assert!(streamed(&empty).is_empty());
    }

    /// test_dictionary_state.cpp:129-147: a session opened on the staged generation reads its dictionaries and journal, so the fixed 拟蒿 leads and the deleted 你好 is gone; an empty stage offers the resource rows again.
    #[test]
    fn a_session_on_a_staged_generation_offers_its_candidates() {
        use crate::session::{Session, SessionOptions};
        let words = |paths: RuntimePaths| {
            let mut options = SessionOptions::new(paths);
            options.learning = false;
            let mut session = Session::new(options).unwrap();
            for letter in "nihao".bytes() {
                session.character(letter, false);
            }
            session
                .snapshot()
                .candidates
                .into_iter()
                .map(|item| item.word)
                .collect::<Vec<_>>()
        };
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let restored = words(stage(&resources, &root.path().join("restored"), records()).unwrap());
        assert_eq!(
            restored.first().map(String::as_str),
            Some("拟蒿"),
            "{restored:?}"
        );
        assert!(!restored.iter().any(|word| word == "你好"), "{restored:?}");
        let empty = words(stage(&resources, &root.path().join("empty"), vec![]).unwrap());
        assert_eq!(empty.first().map(String::as_str), Some("你好"), "{empty:?}");
    }

    #[test]
    fn an_export_is_one_consistent_read_despite_a_concurrent_writer() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let first = stage(&resources, &root.path().join("first"), records()).unwrap();
        let journal = first.user(assets::USER_JOURNAL);
        Connection::open(&journal)
            .unwrap()
            .query_row("PRAGMA journal_mode=WAL", [], |_| Ok(()))
            .unwrap();
        let mut wrote = false;
        let mut count = 0;
        stream_dictionary_state(&first, &mut |_| {
            count += 1;
            if !wrote {
                wrote = true;
                sql(
                    &journal,
                    "INSERT INTO fixed_candidate_positions VALUES('ni''hao','ni''hao','拟好',2)",
                );
            }
            true
        })
        .unwrap();
        assert_eq!(count, records().len());
        assert_eq!(streamed(&first).len(), records().len() + 1);
    }

    // dictionary_stage/tests.rs `rejected_records_and_transport_failures_remove_only_new_generation` and test_dictionary_state.cpp:148-208.
    #[test]
    fn rejected_records_and_transport_failures_remove_only_the_new_generation() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let target = root.path().join("rejected");
        let mut duplicate = records();
        duplicate.push(duplicate[0].clone());
        let mut conflicting_slot = records();
        conflicting_slot.push(DictionaryStateRecord::Position {
            context: "ni'hao".into(),
            key: "ni'hao".into(),
            value: "拟好".into(),
            position: 1,
        });
        let position = |slot| DictionaryStateRecord::Position {
            context: "ctx".into(),
            key: "key".into(),
            value: "value".into(),
            position: slot,
        };
        let selection = |context: &str, count| DictionaryStateRecord::Selection {
            context: context.into(),
            key: "key".into(),
            value: "value".into(),
            count,
        };
        for invalid in [
            duplicate,
            conflicting_slot,
            vec![position(i64::MAX)],
            vec![position(0)],
            vec![position(6)],
            vec![selection("ctx", -1)],
            vec![selection("ctx", 11)],
            vec![selection("ctx\0bad", 1)],
            vec![selection("", 1)],
            vec![entry(
                PersonalDictionaryKind::Pinyin,
                "ni",
                "你",
                -1,
                "",
                false,
                true,
            )],
            vec![entry(
                PersonalDictionaryKind::Pinyin,
                "ni",
                "你",
                100_000_001,
                "",
                false,
                true,
            )],
            vec![entry(
                PersonalDictionaryKind::Pinyin,
                "",
                "你",
                1,
                "",
                false,
                true,
            )],
            vec![entry(
                PersonalDictionaryKind::Pinyin,
                "ni",
                "",
                1,
                "",
                false,
                true,
            )],
            vec![entry(
                PersonalDictionaryKind::Pinyin,
                &"n".repeat(513),
                "你",
                1,
                "",
                false,
                true,
            )],
            vec![entry(
                PersonalDictionaryKind::Pinyin,
                "ni",
                &"你".repeat(1366),
                1,
                "",
                false,
                true,
            )],
            vec![entry(
                PersonalDictionaryKind::Pinyin,
                "ni",
                "你",
                1,
                &"x".repeat(4097),
                false,
                true,
            )],
        ] {
            let error = stage(&resources, &target, invalid).unwrap_err();
            assert_eq!(error.to_string(), diagnostics::INVALID_DICTIONARY_STATE);
            assert!(!target.exists());
        }
        for maximum in [0, 1] {
            assert!(stage_dictionary_state(
                &resources,
                &target,
                "fixture",
                &mut records().into_iter().map(Ok),
                maximum
            )
            .is_err());
            assert!(!target.exists());
        }
        // A checksum or truncation error after the last record is not a successful EOF, and its error reaches the caller unchanged.
        let mut stream =
            records()
                .into_iter()
                .map(Ok)
                .chain(std::iter::once(Err(EngineError::failed(
                    diagnostics::SNAPSHOT_STREAM_FAILED,
                ))));
        let error =
            stage_dictionary_state(&resources, &target, "fixture", &mut stream, 8).unwrap_err();
        assert_eq!(error.to_string(), diagnostics::SNAPSHOT_STREAM_FAILED);
        assert!(!target.exists());
        let error = stage_dictionary_state(
            &resources,
            &target,
            "../escape",
            &mut records().into_iter().map(Ok),
            8,
        )
        .unwrap_err();
        assert_eq!(error.to_string(), diagnostics::INVALID_RUNTIME_CONTENT_ID);
        assert!(!target.exists());

        let existing = stage(&resources, &target, records()).unwrap();
        let revision = dictionary_state_revision(&existing).unwrap();
        assert!(stage(&resources, &target, vec![]).is_err());
        assert_eq!(dictionary_state_revision(&existing).unwrap(), revision);
        let inside_resources = resources.join("forbidden");
        assert!(stage(&resources, &inside_resources, records()).is_err());
        assert!(!inside_resources.exists());
        assert!(stage(
            Path::new("resources"),
            &root.path().join("relative"),
            records()
        )
        .is_err());
        assert!(!root.path().join("relative").exists());
    }

    // dictionary_stage/tests.rs `stream_is_lazy_and_rebuild_failure_cleans_staging`.
    #[test]
    fn the_stream_is_consumed_lazily_and_a_rebuild_failure_cleans_up() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let target = root.path().join("limited");
        let consumed = Rc::new(Cell::new(0));
        let counter = consumed.clone();
        let mut stream = (0..100_000).map(move |index| {
            counter.set(counter.get() + 1);
            Ok(entry(
                PersonalDictionaryKind::QuickPhrase,
                &format!("fixture{index}"),
                "合成短语",
                100,
                "",
                false,
                true,
            ))
        });
        assert!(stage_dictionary_state(&resources, &target, "fixture", &mut stream, 2).is_err());
        assert_eq!(consumed.get(), 3);
        assert!(!target.exists());

        // Every record is valid, but copying the dictionaries fails after the stream is consumed.
        let missing = root.path().join("missing-resources");
        fs::create_dir(&missing).unwrap();
        assert!(stage(&missing, &target, records()).is_err());
        assert!(!target.exists());
        assert!(missing.is_dir());
    }

    fn position(context: &str, key: &str, value: &str, slot: i64) -> DictionaryStateRecord {
        DictionaryStateRecord::Position {
            context: context.into(),
            key: key.into(),
            value: value.into(),
            position: slot,
        }
    }

    fn selection(context: &str, key: &str, value: &str, count: i64) -> DictionaryStateRecord {
        DictionaryStateRecord::Selection {
            context: context.into(),
            key: key.into(),
            value: value.into(),
            count,
        }
    }

    fn merge(
        paths: &RuntimePaths,
        records: Vec<DictionaryStateRecord>,
    ) -> Result<DictionaryStateMerge> {
        merge_dictionary_state(paths, true, &mut records.into_iter().map(Ok), 100)
    }

    /// 本机已有拟好的学习调权、拟蒿这个自造词和它的固定位置，以及两条选词计数。
    fn local_state(resources: &Path, generation: &Path) -> RuntimePaths {
        use PersonalDictionaryKind::*;
        stage(
            resources,
            generation,
            vec![
                entry(Pinyin, "ni'hao", "拟好", 200, "", false, false),
                entry(Pinyin, "ni'hao", "拟蒿", 150, "", false, true),
                position("ni'hao", "ni'hao", "拟蒿", 1),
                selection("ni'hao", "ni'hao", "拟好", 7),
                selection("ni'hao", "ni'hao", "你好", 2),
            ],
        )
        .unwrap()
    }

    fn backup_records() -> Vec<DictionaryStateRecord> {
        use PersonalDictionaryKind::*;
        vec![
            // 本机已有这个词的日志行：保留本机的 200。
            entry(Pinyin, "ni'hao", "拟好", 999, "", false, false),
            // 用户在旧设备上删掉的随包词。
            entry(Pinyin, "ni'hao", "你好", 0, "", true, true),
            // 旧设备学到的一条调权，本机还没有。
            entry(Pinyin, "ni'hao", "泥好", 120, "", false, false),
            // 用户自己的词由个人词库队列合并，这里只计数。
            entry(Pinyin, "ni'hao", "拟蒿", 150, "", false, true),
            // 编码里有空音节，拼不出表名。
            entry(Pinyin, "ni''hao", "坏键", 120, "", false, false),
            // 同一个词本机已固定在 1。
            position("ni'hao", "ni'hao", "拟蒿", 3),
            // 位置 1 已被本机占用。
            position("ni'hao", "ni'hao", "泥好", 1),
            position("ni'hao", "ni'hao", "泥好", 2),
            // 本机是 7，取大保留本机。
            selection("ni'hao", "ni'hao", "拟好", 3),
            // 本机是 2，取大调成 5。
            selection("ni'hao", "ni'hao", "你好", 5),
            selection("ni", "ni", "你", 4),
        ]
    }

    #[test]
    fn a_merge_keeps_local_rows_takes_the_larger_count_and_writes_the_rest() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let local = local_state(&resources, &root.path().join("local"));
        let merged = merge(&local, backup_records()).unwrap();
        assert_eq!(
            merged,
            DictionaryStateMerge {
                entries: 2,
                positions: 1,
                selections: 2,
                kept: 4,
                words: 1,
                skipped: 1,
            }
        );
        let journal = local.user(assets::USER_JOURNAL);
        let main = local.dictionary(assets::MAIN_DICTIONARY);
        assert_eq!(
            weight(&main, "SELECT weight FROM tbl_2_n WHERE value='拟好'"),
            Some(200)
        );
        assert_eq!(
            weight(&main, "SELECT count(*) FROM tbl_2_n WHERE value='你好'"),
            Some(0)
        );
        assert_eq!(
            weight(&main, "SELECT weight FROM tbl_2_n WHERE value='泥好'"),
            Some(120)
        );
        assert_eq!(
            weight(
                &journal,
                "SELECT weight FROM user_dictionary_operations WHERE value='拟好'"
            ),
            Some(200)
        );
        let rows: Vec<(String, String, i64)> = Connection::open(&journal)
            .unwrap()
            .prepare("SELECT value,operation,user_inserted FROM user_dictionary_operations WHERE value IN ('你好','泥好') ORDER BY value")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            rows,
            vec![
                ("你好".into(), "delete".into(), 1),
                ("泥好".into(), "upsert".into(), 0),
            ]
        );
        assert_eq!(
            weight(
                &journal,
                "SELECT count(*) FROM user_dictionary_operations WHERE key='ni''''hao'"
            ),
            Some(0)
        );
        assert_eq!(
            weight(
                &journal,
                "SELECT position FROM fixed_candidate_positions WHERE value='拟蒿'"
            ),
            Some(1)
        );
        assert_eq!(
            weight(
                &journal,
                "SELECT position FROM fixed_candidate_positions WHERE value='泥好'"
            ),
            Some(2)
        );
        for (value, count) in [("拟好", 7), ("你好", 5), ("你", 4)] {
            assert_eq!(
                weight(
                    &journal,
                    &format!(
                        "SELECT selection_count FROM candidate_selection_state WHERE value='{value}'"
                    )
                ),
                Some(count),
                "{value}"
            );
        }

        // 再合并一次什么也不改：上一次写进来的都成了本机已有的。
        let revision = dictionary_state_revision(&local).unwrap();
        let again = merge(&local, backup_records()).unwrap();
        assert_eq!(
            again,
            DictionaryStateMerge {
                entries: 0,
                positions: 0,
                selections: 0,
                kept: 9,
                words: 1,
                skipped: 1,
            }
        );
        assert_eq!(dictionary_state_revision(&local).unwrap(), revision);
    }

    #[test]
    fn a_failed_merge_changes_nothing() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let local = local_state(&resources, &root.path().join("local"));
        let main = local.dictionary(assets::MAIN_DICTIONARY);
        let revision = dictionary_state_revision(&local).unwrap();
        let dictionary = fs::read(&main).unwrap();
        let mut invalid = backup_records();
        invalid.push(position("ni", "ni", "你", 6));
        assert_eq!(
            merge(&local, invalid).unwrap_err().to_string(),
            diagnostics::INVALID_DICTIONARY_STATE
        );
        let mut stream = backup_records()
            .into_iter()
            .map(Ok)
            .chain(std::iter::once(Err(EngineError::failed(
                diagnostics::SNAPSHOT_STREAM_FAILED,
            ))));
        assert_eq!(
            merge_dictionary_state(&local, true, &mut stream, 100)
                .unwrap_err()
                .to_string(),
            diagnostics::SNAPSHOT_STREAM_FAILED
        );
        assert!(
            merge_dictionary_state(&local, true, &mut backup_records().into_iter().map(Ok), 2)
                .is_err()
        );
        assert_eq!(dictionary_state_revision(&local).unwrap(), revision);
        assert_eq!(fs::read(&main).unwrap(), dictionary);
    }

    /// 词库里没有那张表的行跳过，合并照常提交；写库时别的 SQL 错误整体回滚，不当成跳过。
    #[test]
    fn a_missing_table_is_skipped_but_a_write_failure_rolls_back() {
        use PersonalDictionaryKind::*;
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let local = local_state(&resources, &root.path().join("local"));
        // 测试词库里没有 wubi98 这张表。
        let merged = merge(
            &local,
            vec![
                entry(Wubi98, "wqvb", "你好", 120, "", false, false),
                selection("ni", "ni", "你", 4),
            ],
        )
        .unwrap();
        assert_eq!(
            (merged.entries, merged.selections, merged.skipped),
            (0, 1, 1)
        );

        // 写词库时出错（这里用触发器模拟，真实情况是磁盘满或 I/O 失败）：前面已经调大的选词计数也一起撤销。
        sql(
            &local.dictionary(assets::MAIN_DICTIONARY),
            "CREATE TRIGGER synthetic_failure BEFORE INSERT ON tbl_2_n BEGIN SELECT RAISE(ABORT,'synthetic write failure'); END;",
        );
        let revision = dictionary_state_revision(&local).unwrap();
        assert!(merge(
            &local,
            vec![
                selection("ni", "ni", "你", 9),
                entry(Pinyin, "ni'hao", "泥好", 120, "", false, false),
            ],
        )
        .is_err());
        assert_eq!(dictionary_state_revision(&local).unwrap(), revision);
        assert_eq!(
            weight(
                &local.user(assets::USER_JOURNAL),
                "SELECT selection_count FROM candidate_selection_state WHERE value='你'"
            ),
            Some(4)
        );
    }

    /// 没有中文词库的代次只收英文行，其余计入 skipped，日志不动。
    #[test]
    fn a_generation_without_the_main_dictionary_merges_english_rows_only() {
        use PersonalDictionaryKind::*;
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let local = stage(&resources, &root.path().join("local"), vec![]).unwrap();
        let merged = merge_dictionary_state(
            &local,
            false,
            &mut vec![
                entry(Pinyin, "ni'hao", "泥好", 120, "", false, false),
                entry(
                    English,
                    "cloudfixture",
                    "Cloudfixture",
                    300,
                    "Cloudfixture",
                    false,
                    false,
                ),
            ]
            .into_iter()
            .map(Ok),
            100,
        )
        .unwrap();
        assert_eq!((merged.entries, merged.skipped), (1, 1));
        assert_eq!(
            weight(&local.dictionary(assets::ENGLISH_DICTIONARY), "SELECT weight FROM english_words WHERE word='cloudfixture' AND display='Cloudfixture'"),
            Some(300)
        );
        assert_eq!(
            weight(
                &local.user(assets::USER_JOURNAL),
                "SELECT count(*) FROM user_dictionary_operations"
            ),
            Some(1)
        );
    }

    // test_dictionary_state.cpp `--capacity`: a large state stages and streams back whole.
    #[test]
    fn a_large_state_stages_and_streams_back_whole() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let total = 20_000;
        let mut stream = (0..total).map(|index| {
            Ok(entry(
                PersonalDictionaryKind::QuickPhrase,
                &format!("bulk{index}"),
                &format!("合成短语{index}"),
                100,
                "",
                false,
                true,
            ))
        });
        let staged = stage_dictionary_state(
            &resources,
            &root.path().join("capacity"),
            "fixture",
            &mut stream,
            DEFAULT_MAXIMUM_RECORDS,
        )
        .unwrap();
        let mut received = 0;
        stream_dictionary_state(&staged, &mut |record| {
            assert!(matches!(
                record,
                DictionaryStateRecord::Entry {
                    user_inserted: true,
                    weight: 100,
                    ..
                }
            ));
            received += 1;
            true
        })
        .unwrap();
        assert_eq!(received, total);
        assert_eq!(
            weight(
                &staged.dictionary(assets::MAIN_DICTIONARY),
                "SELECT count(*) FROM quick_parases"
            ),
            Some(total as i64)
        );
    }
}
