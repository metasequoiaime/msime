//! Session-free dictionary calls on `EngineOptions` paths (api-contract §1b), and the snapshot record stream.

use std::path::Path;

use super::options::{runtime_paths, EngineOptions};
use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::types::{PersonalDictionaryEntry, PersonalDictionaryKind};
use crate::user_dictionary::journal::release_thread_journal;
use crate::user_dictionary::{bundled, habits, personal, replay, reset, state};

// 产品胶水：桥接层沿用下来的名字，指的就是根上文档化的 `PersonalDictionaryKind`、`PersonalDictionaryEntry`、`PersonalDictionaryPage`。msime 自己的宿主照旧用这几个名字，第三方用根上的名字。
pub type DictionaryKind = PersonalDictionaryKind;
pub type DictionaryEntry = PersonalDictionaryEntry;
use crate::user_dictionary::bundled::DictionaryTablePage;
pub use crate::user_dictionary::habits::{habit_is_storable, LearningHabit, LearningHabitsMerge};
pub use crate::user_dictionary::personal::PersonalDictionaryPage as DictionaryPage;
pub use crate::user_dictionary::state::DictionaryStateMerge;
use crate::user_dictionary::state::DictionaryStateRecord;

/// A sanitised transport failure (cancellation, truncation, bad checksum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("Snapshot record stream failed")]
pub struct SnapshotReadError;

/// 代次里有没有 `msime-pinyin.db`：会话允许的方案里有读它的方案时才有（`SchemeSet::reads_main_dictionary`）。没有它的版本，个人词库只收英文词。
fn main_dictionary(options: &EngineOptions) -> bool {
    options.enabled_schemes.reads_main_dictionary()
}

/// These calls come from pool threads (a Tauri blocking task, the MCP server) that may never touch the journal again, so each one closes its thread's cached connection before returning, on success and failure alike.
fn released<T>(value: T) -> T {
    release_thread_journal();
    value
}

pub fn dictionary_entries(
    options: &EngineOptions,
    offset: usize,
    limit: usize,
) -> Result<DictionaryPage> {
    released(personal::personal_dictionary_entries_with(
        &runtime_paths(options),
        main_dictionary(options),
        offset,
        limit,
        false,
    ))
}

/// [`dictionary_entries`] 和 [`dictionary_entries_for_kind`] 的读取来源：是否读主词库，加上用户日志和它的 `-wal` 的戳（Unix 上是 inode、长度、修改时间和状态改变时间，别处是长度和修改时间；文件不在记作 `-`）。两次结果相同，同一页就读得出同样的内容，宿主据此跳过重查。空的 `-wal` 没有任何帧，与没有 `-wal` 同等看待：只读连接打开 WAL 日志时会建出空 `-wal`，读写连接最后关闭时又会删掉它，不这样处理的话，列表读取本身就会让下一次的戳变掉。读不出戳（路径无效、元数据读取失败）时为 `None`，调用方必须照常重查。
///
/// 文件最后一次改动离现在还不到 `state::REVISION_SETTLE`（两秒）时也是 `None`，与 `state::cached_dictionary_state_revision` 同一个道理：修改时间的精度有限，取戳、读页之后同一个时间粒度里又来一次长度不变的写入（WAL 重置后从头重写 `-wal`、回滚日志模式下就地改页），戳会完全一样，旧的那一页就会一直被当成最新的。刚写过的日志每次都重查，落定以后的戳才拿来跳过。
pub fn dictionary_entries_source(options: &EngineOptions) -> Option<String> {
    let paths = runtime_paths(options);
    paths.validate().ok()?;
    entries_source(
        &paths.user(crate::assets::USER_JOURNAL),
        main_dictionary(options),
        std::time::SystemTime::now,
    )
}

/// `now` 只在有文件要判断是否落定时才取：wasm32-unknown-unknown 上取系统时间会 panic，而那里读元数据本来就失败，走不到这一步。
fn entries_source(
    journal: &Path,
    main_dictionary: bool,
    now: impl FnOnce() -> std::time::SystemTime,
) -> Option<String> {
    let mut wal = journal.as_os_str().to_owned();
    wal.push("-wal");
    let (journal_stamp, journal_change) = source_stamp(journal, false)?;
    let (wal_stamp, wal_change) = source_stamp(Path::new(&wal), true)?;
    let changes = [journal_change, wal_change];
    if changes.iter().any(Option::is_some) {
        let horizon = now().checked_sub(crate::user_dictionary::state::REVISION_SETTLE)?;
        if !changes.iter().flatten().all(|change| *change < horizon) {
            return None;
        }
    }
    Some(format!(
        "{}|{}|{}",
        u8::from(main_dictionary),
        journal_stamp,
        wal_stamp
    ))
}

/// 一个文件的戳和它最后一次改动的时间（修改时间与 Unix 上的状态改变时间里较晚的一个）；不在（或 `empty_is_missing` 时为空）的文件是 `("-", None)`。
fn source_stamp(
    path: &Path,
    empty_is_missing: bool,
) -> Option<(String, Option<std::time::SystemTime>)> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) if empty_is_missing && metadata.len() == 0 => return Some(("-".into(), None)),
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Some(("-".into(), None))
        }
        Err(_) => return None,
    };
    let modified = metadata.modified().ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let changed = std::time::UNIX_EPOCH.checked_add(std::time::Duration::new(
            u64::try_from(metadata.ctime()).ok()?,
            u32::try_from(metadata.ctime_nsec()).ok()?,
        ))?;
        Some((
            format!(
                "{}:{}:{}.{}:{}.{}",
                metadata.ino(),
                metadata.len(),
                metadata.mtime(),
                metadata.mtime_nsec(),
                metadata.ctime(),
                metadata.ctime_nsec()
            ),
            Some(modified.max(changed)),
        ))
    }
    #[cfg(not(unix))]
    {
        let since_epoch = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
        Some((
            format!("{}:{}", metadata.len(), since_epoch.as_nanos()),
            Some(modified),
        ))
    }
}

pub fn dictionary_export_entries(
    options: &EngineOptions,
    offset: usize,
    limit: usize,
    include_learned_pinyin: bool,
) -> Result<DictionaryPage> {
    released(personal::personal_dictionary_entries_with(
        &runtime_paths(options),
        main_dictionary(options),
        offset,
        limit,
        include_learned_pinyin,
    ))
}

/// 只查询指定词库的一页，偏移和后续页状态都以该词库为准。
pub fn dictionary_export_entries_for_kind(
    options: &EngineOptions,
    kind: DictionaryKind,
    offset: usize,
    limit: usize,
) -> Result<DictionaryPage> {
    released(personal::personal_dictionary_export_entries_for_kind(
        &runtime_paths(options),
        main_dictionary(options),
        kind,
        offset,
        limit,
    ))
}

/// 只查询指定词库的用户词条一页，供设置列表在数据库层先筛选再分页。
pub fn dictionary_entries_for_kind(
    options: &EngineOptions,
    kind: DictionaryKind,
    offset: usize,
    limit: usize,
) -> Result<DictionaryPage> {
    released(personal::personal_dictionary_entries_for_kind(
        &runtime_paths(options),
        main_dictionary(options),
        kind,
        offset,
        limit,
    ))
}

pub fn dictionary_table_entries(
    options: &EngineOptions,
    kind: DictionaryKind,
    query: &str,
    offset: usize,
    limit: usize,
) -> Result<DictionaryTablePage> {
    released(bundled::dictionary_table_entries(
        &runtime_paths(options),
        kind,
        query,
        offset,
        limit,
    ))
}

pub fn dictionary_edit_bundled(
    options: &EngineOptions,
    previous: &DictionaryEntry,
    weight: Option<i64>,
    request_id: &str,
) -> Result<()> {
    released(bundled::edit_bundled_dictionary_entry_with(
        &runtime_paths(options),
        main_dictionary(options),
        previous,
        weight,
        request_id,
    ))
}

pub fn dictionary_validate(entry: &DictionaryEntry) -> Result<DictionaryEntry> {
    personal::validate_personal_dictionary_entry(entry)
}

/// `dictionary_validate` for the row an edit or removal starts from: a listed row keeps whatever weight learning gave it, from 0 for a derived wubi word to past the ceiling for a learned English word, so its weight is only held to what the store can hold.
pub fn dictionary_validate_previous(entry: &DictionaryEntry) -> Result<DictionaryEntry> {
    personal::validate_existing_entry(entry)
}

pub fn dictionary_edit(
    options: &EngineOptions,
    previous: Option<&DictionaryEntry>,
    replacement: Option<&DictionaryEntry>,
    request_id: &str,
) -> Result<()> {
    released(personal::edit_personal_dictionary_with(
        &runtime_paths(options),
        main_dictionary(options),
        previous,
        replacement,
        request_id,
    ))
}

/// 有界批量编辑的参数；维护租约由 host-api 持有。
pub use personal::{
    PersonalDictionaryApplied as DictionaryApplied, PersonalDictionaryEdit as DictionaryEdit,
};

/// 一次事务提交一批编辑，逐项结果只在外层提交成功后返回。
pub fn dictionary_edit_batch(
    options: &EngineOptions,
    edits: &[DictionaryEdit<'_>],
) -> Result<Vec<Result<DictionaryApplied>>> {
    released(personal::edit_personal_dictionary_batch_with(
        &runtime_paths(options),
        main_dictionary(options),
        edits,
    ))
}

/// 集合直达导入的专用批量入口，使用更大的有界事务；普通编辑仍限制为 128 项。
pub fn dictionary_edit_collection_batch(
    options: &EngineOptions,
    edits: &[DictionaryEdit<'_>],
) -> Result<Vec<Result<DictionaryApplied>>> {
    released(personal::edit_personal_dictionary_collection_batch_with(
        &runtime_paths(options),
        main_dictionary(options),
        edits,
    ))
}

/// Refuses to reset the packaged bundle in place and needs both packaged dictionaries. 方案集合不读 `msime-pinyin.db` 时只需要、也只换回 `msime-english.db`。
pub fn reset_learned_data(options: &EngineOptions) -> Result<()> {
    released(reset::reset_learned_data_with(
        &runtime_paths(options),
        main_dictionary(options),
    ))
}

/// `(applied, skipped, failed, error)`; never errors.
pub fn replay_user_dictionary(
    user_db_path: &str,
    main_db_path: &str,
    english_db_path: &str,
) -> (i32, i32, i32, String) {
    let result = released(replay::replay(
        Path::new(user_db_path),
        Path::new(main_db_path),
        Path::new(english_db_path),
    ));
    (result.applied, result.skipped, result.failed, result.error)
}

/// `INVALID_SNAPSHOT_RECORD_LIMIT` for a zero limit; returns `options` with the four paths of the staged generation.
pub fn stage_dictionary_state(
    options: &EngineOptions,
    generation: &str,
    content_id: &str,
    maximum_records: usize,
    records: impl Iterator<Item = std::result::Result<DictionaryStateRecord, SnapshotReadError>>,
) -> Result<EngineOptions> {
    if maximum_records == 0 {
        return Err(EngineError::invalid(
            diagnostics::INVALID_SNAPSHOT_RECORD_LIMIT,
        ));
    }
    // A transport failure reaches the stager as an error, never as the end of the stream: only verified EOF may finish a generation (bridge.cpp:473-476).
    let mut records = records
        .map(|record| record.map_err(|_| EngineError::failed(diagnostics::SNAPSHOT_STREAM_FAILED)));
    let paths = released(state::stage_dictionary_state_for(
        Path::new(&options.resources),
        Path::new(generation),
        content_id,
        &mut records,
        maximum_records,
        options.enabled_schemes,
    ))?;
    let text = |path: &Path| path.to_string_lossy().into_owned();
    Ok(EngineOptions {
        resources: text(&paths.resources),
        user_data: text(&paths.user_data),
        cache: text(&paths.cache),
        dictionaries: text(&paths.dictionaries),
        ..options.clone()
    })
}

pub fn dictionary_state_revision(options: &EngineOptions) -> Result<String> {
    released(state::dictionary_state_revision(&runtime_paths(options)))
}

/// 与 [`dictionary_state_revision`] 相同，但日志文件自上次算过以来没变时直接返回上次的结果（见 `state::cached_dictionary_state_revision`）。只给空闲时发布本机词库版本这类预览用；准备、激活快照前的比对仍用不走缓存的 [`dictionary_state_revision`]。
pub fn cached_dictionary_state_revision(options: &EngineOptions) -> Result<String> {
    released(state::cached_dictionary_state_revision(&runtime_paths(
        options,
    )))
}

/// 在一个只读事务里依次读出日志的全部操作行（含学习调权和删除记录）、固定位置和选词计数，交给 `emit`；`emit` 返回假时停下并报 `INVALID_DICTIONARY_STATE`。本地备份导出输入记录用它。
pub fn stream_dictionary_state(
    options: &EngineOptions,
    emit: &mut dyn FnMut(&DictionaryStateRecord) -> bool,
) -> Result<()> {
    released(state::stream_dictionary_state(
        &runtime_paths(options),
        emit,
    ))
}

/// 把一串词库状态记录合并进本机正在用的日志和词库，本机已有的保留本机、选词计数取大（规则见 `state::merge_dictionary_state`）。读流失败与 [`stage_dictionary_state`] 一样报 `SNAPSHOT_STREAM_FAILED`，整体回滚。调用方必须持有独占维护权。
pub fn merge_dictionary_state(
    options: &EngineOptions,
    maximum_records: usize,
    records: impl Iterator<Item = std::result::Result<DictionaryStateRecord, SnapshotReadError>>,
) -> Result<DictionaryStateMerge> {
    let mut records = records
        .map(|record| record.map_err(|_| EngineError::failed(diagnostics::SNAPSHOT_STREAM_FAILED)));
    released(state::merge_dictionary_state(
        &runtime_paths(options),
        main_dictionary(options),
        &mut records,
        maximum_records,
    ))
}

/// 把用户加的粤语词写进一代的日志，已有同一个词的保留已有的，返回写进去的条数（规则见 `state::add_cantonese_words`）。激活整份快照前把旧的一代里的粤语词带进新的一代用它：快照装不下粤语词。调用方必须持有独占维护权。
pub fn add_cantonese_words(
    options: &EngineOptions,
    words: &[DictionaryStateRecord],
) -> Result<usize> {
    released(state::add_cantonese_words(&runtime_paths(options), words))
}

/// 在一个只读事务里依次读出日志里的六张输入习惯表（整句联想、选词对、拼写纠错、自动纠错抑制、置顶），交给 `emit`；`emit` 返回假时停下并报 `INVALID_DICTIONARY_STATE`。本地备份导出输入习惯用它，规则见 `habits::stream_learning_habits`。
pub fn stream_learning_habits(
    options: &EngineOptions,
    emit: &mut dyn FnMut(&LearningHabit) -> bool,
) -> Result<()> {
    released(habits::stream_learning_habits(
        &runtime_paths(options),
        emit,
    ))
}

/// 本机输入习惯的行数，只读。
pub fn count_learning_habits(options: &EngineOptions) -> Result<usize> {
    released(habits::count_learning_habits(&runtime_paths(options)))
}

/// 把一串输入习惯合并进本机正在用的日志，计数取大、置顶保留本机，写完压回各表的上限（规则见 `habits::merge_learning_habits`）。读流失败报 `SNAPSHOT_STREAM_FAILED`，整体回滚。调用方必须持有独占维护权。
pub fn merge_learning_habits(
    options: &EngineOptions,
    maximum_records: usize,
    records: impl Iterator<Item = std::result::Result<LearningHabit, SnapshotReadError>>,
) -> Result<LearningHabitsMerge> {
    let mut records = records
        .map(|record| record.map_err(|_| EngineError::failed(diagnostics::SNAPSHOT_STREAM_FAILED)));
    released(habits::merge_learning_habits(
        &runtime_paths(options),
        &mut records,
        maximum_records,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    /// 刚写过的日志没有可用的来源戳，落定以后才有；之后没有写入就一直相同，空的 `-wal` 出现也不算改动，写一次就变。
    #[test]
    fn the_entries_source_is_withheld_until_the_journal_settles() {
        let directory = tempfile::tempdir().unwrap();
        let journal = directory.path().join("user.db");
        let later = || SystemTime::now() + crate::user_dictionary::state::REVISION_SETTLE * 30;

        // 没有日志：没什么要落定的，戳是确定的。
        assert_eq!(
            entries_source(&journal, true, || -> SystemTime {
                panic!("nothing to settle, the clock must not be read")
            })
            .as_deref(),
            Some("1|-|-")
        );

        std::fs::write(&journal, b"first").unwrap();
        assert_eq!(entries_source(&journal, true, SystemTime::now), None);
        let settled = entries_source(&journal, true, later).unwrap();
        assert_eq!(
            entries_source(&journal, true, later).as_deref(),
            Some(settled.as_str())
        );

        let mut wal = journal.as_os_str().to_owned();
        wal.push("-wal");
        std::fs::write(&wal, b"").unwrap();
        assert_eq!(
            entries_source(&journal, true, later).as_deref(),
            Some(settled.as_str())
        );

        std::fs::write(&wal, b"frame").unwrap();
        assert_eq!(entries_source(&journal, true, SystemTime::now), None);
        let written = entries_source(&journal, true, later).unwrap();
        assert_ne!(written, settled);
    }
}
