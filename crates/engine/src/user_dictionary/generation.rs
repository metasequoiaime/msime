//! 代次准备（core-session.md §12、data-formats.md §3、`runtime_paths.cpp:116-182`）：`user_data/dictionaries/<content id>` 里是经 backup API 复制的 `msime-pinyin.db` 与 `msime-english.db`（资源单独发布的 `msime-wubi.db` 五笔码表并回前者），回放过用户日志，旁边还有 n-gram 表。不读 `msime-pinyin.db` 的方案集合（见 `SchemeSet::reads_main_dictionary`）准备的代次只有 `msime-english.db`。

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind};
use std::path::{Component, Path, PathBuf};

use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};

use crate::assets;
use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::paths::RuntimePaths;
use crate::types::SchemeSet;
use crate::user_dictionary::journal::open_database;
use crate::user_dictionary::replay::{replay, replay_english};
use crate::wubi::provider::ensure_reverse_indexes;

pub const MAX_CONTENT_ID_LENGTH: usize = 128;

/// Tables the lattice reads through `RuntimePaths::dictionary`. They are built from one generation's vocabulary, so they live beside it, and a resource set without them simply leaves the decoder without them (RP:56-59).
const GENERATION_COPIES: [&str; 2] = [assets::BIGRAM_TABLE, assets::TRIGRAM_TABLE];

/// `msime-wubi.db` 里单独发布、准备代次时并回工作主词库的五笔码表。
const SPLIT_WUBI_TABLES: [&str; 2] = ["wubi86", "wubi98"];

/// Validate the content id (1..=128 of `[0-9A-Za-z_-]`) and the disjoint absolute roots, create `user_data` and `cache`, then either re-replay an existing ready generation (copying n-gram tables it lacks) or stage `<id>.incoming`, replay, write `.ready` and rename it into place. A failed staging removes only its own directory.
pub fn prepare_runtime_paths(
    resources: &Path,
    user_data: &Path,
    cache: &Path,
    content_id: &str,
) -> Result<RuntimePaths> {
    prepare_runtime_paths_for(resources, user_data, cache, content_id, SchemeSet::ALL)
}

/// 按会话允许的方案准备代次。`schemes` 读 `msime-pinyin.db`（[`SchemeSet::reads_main_dictionary`]）时与 [`prepare_runtime_paths`] 完全相同：`msime-pinyin.db` 和 `msime-english.db` 都必须在资源目录里，都复制进代次。不读它时（只有日文、越南文、藏文这类方案的版本）资源目录里本来就没有它：代次只复制并要求 `msime-english.db`，日志只回放英文行（`replay_english`）。
pub fn prepare_runtime_paths_for(
    resources: &Path,
    user_data: &Path,
    cache: &Path,
    content_id: &str,
    schemes: SchemeSet,
) -> Result<RuntimePaths> {
    let main_dictionary = schemes.reads_main_dictionary();
    if !valid_content_id(content_id) {
        return Err(EngineError::invalid(
            diagnostics::INVALID_RUNTIME_CONTENT_ID,
        ));
    }
    let result = RuntimePaths {
        resources: resources.to_owned(),
        user_data: user_data.to_owned(),
        cache: cache.to_owned(),
        dictionaries: user_data.join("dictionaries").join(content_id),
    };
    result.validate()?;
    // Each root has a different lifetime, so validate before creating anything: clearing the cache must not delete durable data, and preparation must not write inside immutable resources (RP:126-128).
    let resource_root = weakly_canonical(resources)?;
    let user_root = weakly_canonical(user_data)?;
    let cache_root = weakly_canonical(cache)?;
    if roots_overlap(&resource_root, &user_root)
        || roots_overlap(&resource_root, &cache_root)
        || roots_overlap(&user_root, &cache_root)
    {
        return Err(EngineError::invalid(diagnostics::RUNTIME_ROOTS_OVERLAP));
    }
    reject_redirected_directory(user_data)?;
    reject_redirected_directory(cache)?;
    fs::create_dir_all(user_data)?;
    fs::create_dir_all(cache)?;
    reject_redirected_directory(user_data)?;
    reject_redirected_directory(cache)?;

    let marker = result.dictionaries.join(assets::GENERATION_READY);
    let generation_ready = match fs::symlink_metadata(&marker) {
        Ok(metadata) if metadata.file_type().is_file() => true,
        Ok(_) => {
            // A directory, device or symlink at the marker path is not a
            // completed generation. Do not follow it or try to replace the
            // existing generation directory during staging.
            return Err(EngineError::failed(
                diagnostics::INCOMPLETE_RUNTIME_GENERATION,
            ));
        }
        Err(error) if error.kind() == ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    if generation_ready {
        for name in generation_dictionaries(main_dictionary) {
            if !is_real_file(&result.dictionary(name)) {
                return Err(EngineError::failed(
                    diagnostics::INCOMPLETE_RUNTIME_GENERATION,
                ));
            }
        }
        // 拆分五笔码表之后准备的代次也要有五笔表；表已在时这一步只读不写。没有工作主词库的代次没有地方放五笔表。
        if main_dictionary {
            merge_split_wubi(resources, &result.dictionary(assets::MAIN_DICTIONARY))?;
        }
        // A host can switch back to a previously prepared generation. Replay the current journal again so changes learned on a newer generation survive that switch (RP:143-144).
        replay_into(&result, &result.dictionaries, main_dictionary)?;
        // 旧版本准备的代次没有反查索引，重新打开时补上；已有索引时只读一次 schema。
        index_reverse_lookup(&result.dictionaries, main_dictionary);
        stage_generation_copies(resources, &result.dictionaries, false)?;
        return Ok(result);
    }

    let parent = user_data.join("dictionaries");
    let stage = parent.join(format!("{content_id}.incoming"));
    fs::create_dir_all(&parent)?;
    // Exclusive creation also keeps two preparations of the same id from overlapping (RP:155-157).
    match fs::create_dir(&stage) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            return Err(EngineError::failed(diagnostics::RUNTIME_STAGING_EXISTS));
        }
        Err(error) => return Err(error.into()),
    }
    let staged = (|| -> Result<()> {
        for name in generation_dictionaries(main_dictionary) {
            copy_database(&resources.join(name), &stage.join(name))?;
        }
        if main_dictionary {
            merge_split_wubi(resources, &stage.join(assets::MAIN_DICTIONARY))?;
        }
        stage_generation_copies(resources, &stage, true)?;
        replay_into(&result, &stage, main_dictionary)?;
        index_reverse_lookup(&stage, main_dictionary);
        write_private_file(
            &stage.join(assets::GENERATION_READY),
            format!("{content_id}\n").as_bytes(),
        )
        .map_err(|_| EngineError::failed(diagnostics::RUNTIME_FINALIZE_FAILED))?;
        fs::rename(&stage, &result.dictionaries)?;
        Ok(())
    })();
    if let Err(error) = staged {
        // The staging directory is this call's own; what the caller needs is the error that stopped it, not whether the cleanup also failed (RP:174-179).
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }
    Ok(result)
}

fn valid_content_id(content_id: &str) -> bool {
    !content_id.is_empty()
        && content_id.len() <= MAX_CONTENT_ID_LENGTH
        && content_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// 代次里的词库工作副本：`main_dictionary` 为假时没有 `msime-pinyin.db`，只有 `msime-english.db`。
fn generation_dictionaries(main_dictionary: bool) -> &'static [&'static str] {
    if main_dictionary {
        &[assets::MAIN_DICTIONARY, assets::ENGLISH_DICTIONARY]
    } else {
        &[assets::ENGLISH_DICTIONARY]
    }
}

/// Replay the journal of `paths` into the dictionaries in `generation`; any failed row or error refuses the generation. 没有 `msime-pinyin.db` 的代次只回放英文行。
fn replay_into(paths: &RuntimePaths, generation: &Path, main_dictionary: bool) -> Result<()> {
    let journal = paths.user(assets::USER_JOURNAL);
    let english = generation.join(assets::ENGLISH_DICTIONARY);
    let replayed = if main_dictionary {
        replay(
            &journal,
            &generation.join(assets::MAIN_DICTIONARY),
            &english,
        )
    } else {
        replay_english(&journal, &english)
    };
    if replayed.failed != 0 || !replayed.error.is_empty() {
        return Err(EngineError::failed(format!(
            "{}{}",
            diagnostics::RUNTIME_REPLAY_FAILED,
            replayed.error
        )));
    }
    Ok(())
}

/// 给代次里每个词库副本（`main_dictionary` 为假时只有英文词库）补上五笔反查索引（见 `wubi::provider::ensure_reverse_indexes`）。代次是用户可写的副本，资源目录保持只读、原样；索引不进日志，也不影响词库状态的摘要。
///
/// 只是提速，从不失败：建索引要写盘（约 1.3 MB 加回滚日志），磁盘满、文件只读或被别的进程长时间锁住时记一条日志、照常用没有索引的副本，反查仍然正确，只是慢。这里报错会让宿主起不来。新词库在 dict-builder 里就带着同名索引，这里只为旧词库和旧代次补建。
fn index_reverse_lookup(generation: &Path, main_dictionary: bool) {
    for name in generation_dictionaries(main_dictionary) {
        let database = generation.join(name);
        let indexed = open_database(&database, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .and_then(|connection| ensure_reverse_indexes(&connection).map_err(Into::into));
        if let Err(error) = indexed {
            eprintln!(
                "msime: wubi reverse lookup index unavailable in {}, lookups stay unindexed: {error}",
                database.display()
            );
        }
    }
}

/// Copy through the SQLite backup API, which includes committed WAL content that a plain file copy of a live database would lose (RP:38-55).
fn copy_database(source: &Path, target: &Path) -> Result<()> {
    if !is_real_file(source) {
        return Err(EngineError::failed(format!(
            "{}{}",
            diagnostics::RUNTIME_COPY_FAILED,
            source.display()
        )));
    }
    let copied = (|| -> rusqlite::Result<StepResult> {
        let source = crate::paths::sqlite_path_no_follow(source)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let target = crate::paths::sqlite_path_no_follow(target)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let input = Connection::open_with_flags(
            &source,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NOFOLLOW
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let mut output = Connection::open_with_flags(
            &target,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NOFOLLOW
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        // runtime_paths.cpp:43-44 set no busy timeout on either side; rusqlite would otherwise add a 5 s wait the reference never had.
        input.busy_timeout(std::time::Duration::ZERO)?;
        output.busy_timeout(std::time::Duration::ZERO)?;
        let backup = Backup::new(&input, &mut output)?;
        backup.step(-1)
    })();
    if !matches!(copied, Ok(StepResult::Done)) {
        return Err(EngineError::failed(format!(
            "{}{}",
            diagnostics::RUNTIME_COPY_FAILED,
            source.display()
        )));
    }
    Ok(())
}

pub(super) fn copy_private_file(source: &Path, target: &Path) -> io::Result<u64> {
    let mut input = crate::paths::open_file_no_follow(source)?;
    let mut output = create_private_file(target)?;
    let copied = io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    Ok(copied)
}

fn create_private_file(path: &Path) -> io::Result<fs::File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path)
}

fn write_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut output = create_private_file(path)?;
    io::Write::write_all(&mut output, bytes)?;
    output.sync_all()?;
    Ok(())
}

/// 词库发布把五笔码表单独放在只读的 `msime-wubi.db` 里，`msime-pinyin.db` 不再含 `wubi86`/`wubi98`。五笔的学习调序、删词、个人词典编辑与日志回放都写代次里的工作主词库，五笔 provider 也从它读，所以准备代次（以及重置学习数据）时把这两张表连同索引并回工作副本：读写落在同一个文件上，学到的权重立即可见。资源目录没有 `msime-wubi.db`（旧的合并发布）或工作副本里已有同名表时不动。
pub(crate) fn merge_split_wubi(resources: &Path, main_db: &Path) -> Result<()> {
    let source = resources.join(assets::WUBI_DICTIONARY);
    if !is_real_file(&source) {
        return Ok(());
    }
    let merged = (|| -> rusqlite::Result<()> {
        let source = crate::paths::sqlite_path_no_follow(&source)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let main_db = crate::paths::sqlite_path_no_follow(main_db)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let input = Connection::open_with_flags(
            &source,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NOFOLLOW
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let mut output = Connection::open_with_flags(
            &main_db,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NOFOLLOW
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        input.busy_timeout(std::time::Duration::ZERO)?;
        output.busy_timeout(std::time::Duration::ZERO)?;
        let transaction = output.transaction()?;
        for table in SPLIT_WUBI_TABLES {
            let present: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM main.sqlite_master WHERE type='table' AND name=?1)",
                [table],
                |row| row.get(0),
            )?;
            if present {
                continue;
            }
            // 先建表（`UNIQUE` 约束的自动索引随表建立），灌完数据再建显式索引。
            let schema = input
                .prepare("SELECT sql FROM sqlite_master WHERE tbl_name=?1 AND sql IS NOT NULL ORDER BY type='index', rowid")?
                .query_map([table], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let Some((create, indexes)) = schema.split_first() else {
                continue;
            };
            transaction.execute_batch(create)?;
            let mut select = input.prepare(&format!("SELECT rowid,* FROM \"{table}\""))?;
            let columns: Vec<String> = select
                .column_names()
                .iter()
                .map(|name| format!("\"{name}\""))
                .collect();
            let placeholders = vec!["?"; columns.len()].join(",");
            // 保留 rowid：反查五笔编码时用 rowid 作最后的排序键。
            let mut insert = transaction.prepare(&format!(
                "INSERT INTO \"{table}\"({}) VALUES({placeholders})",
                columns.join(",")
            ))?;
            let mut rows = select.query([])?;
            while let Some(row) = rows.next()? {
                let values = (0..columns.len())
                    .map(|index| row.get::<_, rusqlite::types::Value>(index))
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                insert.execute(rusqlite::params_from_iter(values))?;
            }
            for index in indexes {
                transaction.execute_batch(index)?;
            }
            transaction.execute_batch(&format!("ANALYZE \"{table}\""))?;
        }
        transaction.commit()
    })();
    merged.map_err(|_| {
        EngineError::failed(format!(
            "{}{}",
            diagnostics::RUNTIME_COPY_FAILED,
            source.display()
        ))
    })
}

/// Copy the optional n-gram tables the resource set carries. Also run for an already prepared generation with `replace_existing` false, so a table a later build starts shipping arrives without a new generation while the one a session may have mapped stays untouched (RP:76-90).
pub(super) fn stage_generation_copies(
    resources: &Path,
    generation: &Path,
    replace_existing: bool,
) -> Result<()> {
    for name in GENERATION_COPIES {
        let source = resources.join(name);
        if !is_real_file(&source) {
            continue;
        }
        let target = generation.join(name);
        if !replace_existing && is_real_file(&target) {
            continue;
        }
        // Rename into place rather than writing the target: a session may have the previous file mapped, and a half-written table under an mmap is a crash rather than a miss (RP:62-63).
        let mut incoming = OsString::from(name);
        incoming.push(".incoming");
        let incoming = generation.join(incoming);
        match fs::remove_file(&incoming) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        copy_private_file(&source, &incoming)?;
        fs::rename(&incoming, &target)?;
    }
    Ok(())
}

fn is_real_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

/// `std::filesystem::weakly_canonical`: the longest existing prefix resolved through symlinks, the rest appended with `.` and `..` folded lexically, so roots that do not exist yet compare by what they will resolve to. No crate resolves symlinks for a partly missing path, which the overlap check needs (a symlinked resource alias must still be caught).
pub(super) fn weakly_canonical(path: &Path) -> Result<PathBuf> {
    let components: Vec<Component<'_>> = path.components().collect();
    for split in (1..=components.len()).rev() {
        let head: PathBuf = components[..split].iter().collect();
        match head.canonicalize() {
            Ok(mut resolved) => {
                append_lexically(&mut resolved, &components[split..]);
                return Ok(resolved);
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    let mut resolved = PathBuf::new();
    append_lexically(&mut resolved, &components);
    Ok(resolved)
}

fn append_lexically(base: &mut PathBuf, rest: &[Component<'_>]) {
    for component in rest {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                base.pop();
            }
            other => base.push(other),
        }
    }
}

fn reject_redirected_directory(path: &Path) -> std::io::Result<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                if crate::paths::is_trusted_system_alias(ancestor) {
                    continue;
                }
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "runtime directory has a symbolic-link ancestor",
                ));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotADirectory,
                    "runtime directory parent is not a directory",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// One root contains the other, compared by whole components (RP:34-37).
pub(super) fn roots_overlap(first: &Path, second: &Path) -> bool {
    first.starts_with(second) || second.starts_with(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_dictionary::replay::tests::{sql, weight, OPERATIONS_DDL};

    fn resources(root: &Path) -> PathBuf {
        let resources = root.join("resources");
        fs::create_dir_all(&resources).unwrap();
        sql(
            &resources.join(assets::MAIN_DICTIONARY),
            "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
             INSERT INTO tbl_1_n VALUES('ni','n','你',100);",
        );
        crate::dictionary::english::ensure_english_schema(
            &resources.join(assets::ENGLISH_DICTIONARY),
        )
        .unwrap();
        resources
    }

    fn learn(journal: &Path, word: &str, updated_at: i64) {
        sql(journal, OPERATIONS_DDL);
        sql(
            journal,
            &format!("INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,user_inserted,updated_at) VALUES('pinyin','ni','{word}','upsert',50,1,{updated_at})"),
        );
    }

    #[test]
    fn content_ids_are_validated_before_anything_is_created() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let user = root.path().join("user");
        for id in ["", "../escape", "a b", "é", &"x".repeat(129)] {
            let error = prepare_runtime_paths(&resources, &user, &root.path().join("cache"), id)
                .unwrap_err();
            assert_eq!(error.to_string(), diagnostics::INVALID_RUNTIME_CONTENT_ID);
        }
        assert!(!user.exists());
        let relative = prepare_runtime_paths(
            Path::new("resources"),
            &user,
            &root.path().join("cache"),
            "v1",
        )
        .unwrap_err();
        assert_eq!(
            relative.to_string(),
            diagnostics::RUNTIME_DIRECTORIES_MUST_BE_ABSOLUTE
        );
        assert!(prepare_runtime_paths(
            &resources,
            &user,
            &root.path().join("cache"),
            &"x".repeat(128)
        )
        .is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn resource_dictionary_symlinks_are_rejected_before_staging() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let external = root.path().join("external-msime-pinyin.db");
        fs::rename(resources.join(assets::MAIN_DICTIONARY), &external).unwrap();
        symlink(&external, resources.join(assets::MAIN_DICTIONARY)).unwrap();

        let error = prepare_runtime_paths(
            &resources,
            &root.path().join("user"),
            &root.path().join("cache"),
            "v1",
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .starts_with(diagnostics::RUNTIME_COPY_FAILED));
        assert!(!root.path().join("user/dictionaries/v1").exists());
    }

    #[cfg(unix)]
    #[test]
    fn user_and_cache_roots_reject_symlinked_directories() {
        use std::os::unix::fs::symlink;

        for linked_name in ["user", "cache"] {
            let root = tempfile::tempdir().unwrap();
            let resources = resources(root.path());
            let outside = tempfile::tempdir().unwrap();
            let user = root.path().join("user");
            let cache = root.path().join("cache");
            symlink(outside.path(), root.path().join(linked_name)).unwrap();

            assert!(prepare_runtime_paths(&resources, &user, &cache, "v1").is_err());
            assert!(!outside.path().join("dictionaries").exists());
        }
    }

    // test_runtime_isolation.cpp:140-168.
    #[test]
    fn overlapping_roots_are_rejected_without_creating_directories() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let resource_a = resources(root);
        let reject = |resources: &Path, user: &Path, cache: &Path| {
            let error = prepare_runtime_paths(resources, user, cache, "overlap").unwrap_err();
            assert!(matches!(error, EngineError::InvalidArgument(_)), "{error}");
            assert_eq!(error.to_string(), diagnostics::RUNTIME_ROOTS_OVERLAP);
            assert!(!user.join("dictionaries/overlap").exists());
        };
        reject(
            &resource_a,
            &root.join("same-user-cache"),
            &root.join("same-user-cache"),
        );
        reject(
            &resource_a,
            &root.join("cache-parent/user"),
            &root.join("cache-parent"),
        );
        reject(
            &resource_a,
            &root.join("user-parent"),
            &root.join("user-parent/cache"),
        );
        reject(
            &resource_a,
            &resource_a.join("user"),
            &root.join("separate-cache"),
        );
        reject(
            &resource_a,
            &root.join("separate-user"),
            &resource_a.join("cache"),
        );
        reject(&resource_a, root, &root.join("separate-cache"));
        reject(
            &resource_a,
            &root.join("dotted/../resources/user"),
            &root.join("separate-cache"),
        );
        for created in [
            root.join("same-user-cache"),
            root.join("cache-parent"),
            resource_a.join("user"),
            resource_a.join("cache"),
        ] {
            assert!(!created.exists(), "{}", created.display());
        }
        #[cfg(unix)]
        {
            let alias = root.join("resource-alias");
            std::os::unix::fs::symlink(&resource_a, &alias).unwrap();
            reject(&resource_a, &root.join("alias-user"), &alias.join("cache"));
        }
        // Siblings that share a name prefix do not overlap.
        assert!(prepare_runtime_paths(
            &resource_a,
            &root.join("resources-user"),
            &root.join("resources-cache"),
            "v1"
        )
        .is_ok());
    }

    #[test]
    fn a_new_generation_is_a_replayed_backup_copy_marked_ready() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let original = fs::read(resources.join(assets::MAIN_DICTIONARY)).unwrap();
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        fs::create_dir_all(&user).unwrap();
        learn(&user.join(assets::USER_JOURNAL), "伱", 1);

        let paths = prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        assert_eq!(paths.dictionaries, user.join("dictionaries/v1"));
        assert!(cache.is_dir());
        assert_eq!(
            fs::read_to_string(paths.dictionary(assets::GENERATION_READY)).unwrap(),
            "v1\n"
        );
        assert!(!user.join("dictionaries/v1.incoming").exists());
        assert_eq!(
            weight(
                &paths.dictionary(assets::MAIN_DICTIONARY),
                "SELECT weight FROM tbl_1_n WHERE value='伱'"
            ),
            Some(50)
        );
        assert!(paths.dictionary(assets::ENGLISH_DICTIONARY).is_file());
        assert_eq!(
            fs::read(resources.join(assets::MAIN_DICTIONARY)).unwrap(),
            original
        );

        // Clearing the cache keeps the journal and the working copies.
        fs::remove_dir_all(&cache).unwrap();
        assert!(user.join(assets::USER_JOURNAL).is_file());
        let again = prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        assert_eq!(again, paths);
    }

    // test_runtime_isolation.cpp:648-723: learning on a newer generation survives switching back, and a replay failure publishes nothing.
    #[test]
    fn switching_back_replays_newer_learning_and_a_failed_replay_publishes_nothing() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        let journal = user.join(assets::USER_JOURNAL);
        let v1 = prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        learn(&journal, "伱", 1);
        let v2 = prepare_runtime_paths(&resources, &user, &cache, "v2").unwrap();
        assert_eq!(
            weight(
                &v2.dictionary(assets::MAIN_DICTIONARY),
                "SELECT weight FROM tbl_1_n WHERE value='伱'"
            ),
            Some(50)
        );
        learn(&journal, "倪", 2);
        let restored = prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        assert_eq!(restored, v1);
        assert_eq!(
            weight(
                &v1.dictionary(assets::MAIN_DICTIONARY),
                "SELECT weight FROM tbl_1_n WHERE value='倪'"
            ),
            Some(50)
        );

        let before_failure = fs::read(v2.dictionary(assets::MAIN_DICTIONARY)).unwrap();
        sql(
            &journal,
            "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight) VALUES('pinyin','@','无效词','upsert',10)",
        );
        let error = prepare_runtime_paths(&resources, &user, &cache, "v3").unwrap_err();
        assert!(matches!(error, EngineError::Failed(_)));
        assert_eq!(
            error.to_string(),
            format!(
                "{}one or more operations failed; changes were rolled back",
                diagnostics::RUNTIME_REPLAY_FAILED
            )
        );
        assert_eq!(
            fs::read(v2.dictionary(assets::MAIN_DICTIONARY)).unwrap(),
            before_failure
        );
        assert!(!user.join("dictionaries/v3").exists());
        assert!(!user.join("dictionaries/v3.incoming").exists());
        // A ready generation refuses the same journal as well.
        assert!(prepare_runtime_paths(&resources, &user, &cache, "v2").is_err());
    }

    #[test]
    fn staging_failures_clean_up_and_leftovers_are_refused() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        let missing = root.path().join("missing-resources");
        fs::create_dir_all(&missing).unwrap();
        let error = prepare_runtime_paths(&missing, &user, &cache, "v1").unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "{}{}",
                diagnostics::RUNTIME_COPY_FAILED,
                missing.join(assets::MAIN_DICTIONARY).display()
            )
        );
        assert!(!user.join("dictionaries/v1.incoming").exists());
        assert!(!missing.join(assets::MAIN_DICTIONARY).exists());

        fs::create_dir_all(user.join("dictionaries/v1.incoming")).unwrap();
        let error = prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap_err();
        assert_eq!(error.to_string(), diagnostics::RUNTIME_STAGING_EXISTS);
        assert!(user.join("dictionaries/v1.incoming").is_dir());

        let incomplete = user.join("dictionaries/v2");
        fs::create_dir_all(&incomplete).unwrap();
        fs::write(incomplete.join(assets::GENERATION_READY), "v2\n").unwrap();
        let error = prepare_runtime_paths(&resources, &user, &cache, "v2").unwrap_err();
        assert_eq!(
            error.to_string(),
            diagnostics::INCOMPLETE_RUNTIME_GENERATION
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_generation_marker_is_not_treated_as_ready() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        let generation = user.join("dictionaries/v1");
        fs::create_dir_all(&generation).unwrap();
        for name in generation_dictionaries(true) {
            fs::copy(resources.join(name), generation.join(name)).unwrap();
        }
        let outside = tempfile::tempdir().unwrap();
        let marker_target = outside.path().join("marker");
        fs::write(&marker_target, b"v1\n").unwrap();
        symlink(&marker_target, generation.join(assets::GENERATION_READY)).unwrap();

        assert_eq!(
            prepare_runtime_paths(&resources, &user, &cache, "v1")
                .unwrap_err()
                .to_string(),
            diagnostics::INCOMPLETE_RUNTIME_GENERATION
        );
    }

    #[test]
    fn the_backup_copy_includes_committed_wal_content() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let live = Connection::open(resources.join(assets::MAIN_DICTIONARY)).unwrap();
        live.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(()))
            .unwrap();
        live.execute_batch(
            "PRAGMA wal_autocheckpoint=0; INSERT INTO tbl_1_n VALUES('ni','n','妮',70);",
        )
        .unwrap();
        let paths = prepare_runtime_paths(
            &resources,
            &root.path().join("user"),
            &root.path().join("cache"),
            "v1",
        )
        .unwrap();
        assert_eq!(
            weight(
                &paths.dictionary(assets::MAIN_DICTIONARY),
                "SELECT weight FROM tbl_1_n WHERE value='妮'"
            ),
            Some(70)
        );
    }

    fn reverse_indexes(database: &Path) -> Vec<String> {
        let connection = Connection::open(database).unwrap();
        let mut statement = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_wubi%_value' ORDER BY name")
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn generations_get_the_wubi_reverse_index_and_resources_stay_untouched() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        // 只有 wubi86：没有的 wubi98 不会凭空建表或建索引。
        sql(
            &resources.join(assets::MAIN_DICTIONARY),
            "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));
             INSERT INTO wubi86 VALUES('wqvb','你好',300);",
        );
        let original = fs::read(resources.join(assets::MAIN_DICTIONARY)).unwrap();
        let user = root.path().join("user");
        let cache = root.path().join("cache");

        let paths = prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        let main = paths.dictionary(assets::MAIN_DICTIONARY);
        assert_eq!(reverse_indexes(&main), ["idx_wubi86_value"]);
        assert!(reverse_indexes(&paths.dictionary(assets::ENGLISH_DICTIONARY)).is_empty());
        assert_eq!(
            fs::read(resources.join(assets::MAIN_DICTIONARY)).unwrap(),
            original
        );

        sql(&main, "DROP INDEX idx_wubi86_value;");
        // 补建索引只是提速：副本写不进去（磁盘满、只读、被锁）时照常打开代次，只是没有索引。
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&main, fs::Permissions::from_mode(0o444)).unwrap();
            let reopened = prepare_runtime_paths(&resources, &user, &cache, "v1");
            fs::set_permissions(&main, fs::Permissions::from_mode(0o644)).unwrap();
            reopened.unwrap();
            assert!(reverse_indexes(&main).is_empty());
        }
        // 旧版本准备的代次没有这个索引：再次打开同一代次时补上。
        prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        assert_eq!(reverse_indexes(&main), ["idx_wubi86_value"]);
        // 已有索引时什么也不改。
        let indexed = fs::read(&main).unwrap();
        prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        assert_eq!(reverse_indexes(&main), ["idx_wubi86_value"]);
        assert_eq!(fs::read(&main).unwrap(), indexed);
        assert_eq!(
            weight(&main, "SELECT weight FROM wubi86 WHERE value='你好'"),
            Some(300)
        );
    }

    // test_runtime_isolation.cpp:754-782.
    #[test]
    fn generation_tables_are_staged_and_later_ones_added_without_replacing() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        fs::write(resources.join(assets::BIGRAM_TABLE), "first").unwrap();
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        let paths = prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        assert_eq!(
            fs::read(paths.dictionary(assets::BIGRAM_TABLE)).unwrap(),
            b"first"
        );
        assert!(!paths.dictionary(assets::TRIGRAM_TABLE).exists());

        fs::write(resources.join(assets::TRIGRAM_TABLE), "third").unwrap();
        fs::write(resources.join(assets::BIGRAM_TABLE), "second").unwrap();
        let paths = prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        assert_eq!(
            fs::read(paths.dictionary(assets::TRIGRAM_TABLE)).unwrap(),
            b"third"
        );
        assert_eq!(
            fs::read(paths.dictionary(assets::BIGRAM_TABLE)).unwrap(),
            b"first"
        );
        assert!(!paths.dictionary("msime-trigram.bin.incoming").exists());
        let fresh = prepare_runtime_paths(&resources, &user, &cache, "v2").unwrap();
        assert_eq!(
            fs::read(fresh.dictionary(assets::BIGRAM_TABLE)).unwrap(),
            b"second"
        );
    }

    /// 发布把五笔码表拆进 `msime-wubi.db` 后，准备代次要把它并回工作主词库：日志里的五笔行照常回放，不会让整个代次因为缺表被拒；索引与 rowid 原样保留；再次准备已就绪的代次时不重复导入。
    #[test]
    fn split_wubi_tables_are_merged_into_the_generation_before_replay() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        sql(
            &resources.join(assets::WUBI_DICTIONARY),
            "CREATE TABLE wubi86(\"key\" TEXT NOT NULL,\"value\" TEXT NOT NULL,\"weight\" INTEGER NOT NULL DEFAULT 0,UNIQUE(\"key\",\"value\"));
             CREATE INDEX idx_wubi86_key_weight ON wubi86(\"key\",\"weight\" DESC);
             CREATE TABLE wubi98(\"key\" TEXT NOT NULL,\"value\" TEXT NOT NULL,\"weight\" INTEGER NOT NULL DEFAULT 0,UNIQUE(\"key\",\"value\"));
             INSERT INTO wubi86(rowid,\"key\",\"value\",weight) VALUES(7,'wqvb','你好',100),(3,'aaaa','工',90);
             INSERT INTO wubi98 VALUES('wqvb','你好',80);",
        );
        let user = root.path().join("user");
        let journal = user.join(assets::USER_JOURNAL);
        fs::create_dir_all(&user).unwrap();
        sql(&journal, OPERATIONS_DDL);
        sql(
            &journal,
            "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,user_inserted,updated_at) VALUES
               ('wubi','wqvb','合成五笔','upsert',9,1,1),
               ('wubi98','wqvb','你好','upsert',99,1,2)",
        );
        let cache = root.path().join("cache");
        let paths = prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        let main = paths.dictionary(assets::MAIN_DICTIONARY);
        assert_eq!(
            weight(&main, "SELECT weight FROM wubi86 WHERE value='合成五笔'"),
            Some(9)
        );
        assert_eq!(
            weight(&main, "SELECT weight FROM wubi98 WHERE value='你好'"),
            Some(99)
        );
        assert_eq!(
            weight(&main, "SELECT rowid FROM wubi86 WHERE value='你好'"),
            Some(7)
        );
        assert_eq!(
            weight(
                &main,
                "SELECT count(*) FROM sqlite_master WHERE name='idx_wubi86_key_weight'"
            ),
            Some(1)
        );
        // 只读资源不变。
        assert_eq!(
            weight(
                &resources.join(assets::WUBI_DICTIONARY),
                "SELECT count(*) FROM wubi86"
            ),
            Some(2)
        );
        assert!(!paths.dictionary(assets::WUBI_DICTIONARY).exists());

        let again = prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        assert_eq!(
            weight(
                &again.dictionary(assets::MAIN_DICTIONARY),
                "SELECT count(*) FROM wubi86"
            ),
            Some(3)
        );
    }

    #[test]
    fn weakly_canonical_resolves_the_existing_prefix() {
        let root = tempfile::tempdir().unwrap();
        let base = root.path().canonicalize().unwrap();
        assert_eq!(
            weakly_canonical(&root.path().join("a/./b/../c")).unwrap(),
            base.join("a/c")
        );
        assert_eq!(weakly_canonical(root.path()).unwrap(), base);
        assert!(roots_overlap(Path::new("/a/b"), Path::new("/a")));
        assert!(!roots_overlap(Path::new("/a/bc"), Path::new("/a/b")));
    }

    /// 发布的整套词库能完整准备：两个数据库复制进来（五笔码表并回主词库），两张 n-gram 表逐字节一致，每种日志行都回放到真实的表上。
    #[test]
    fn the_real_dictionary_set_stages_and_replays() {
        let Some(resources) = std::env::var_os("MSIME_EVAL_RESOURCES") else {
            eprintln!(
                "skipped: MSIME_EVAL_RESOURCES is not set to the dict-v2.0.5 resource directory"
            );
            return;
        };
        let resources = PathBuf::from(resources);
        let root = tempfile::tempdir().unwrap();
        let user = root.path().join("user");
        fs::create_dir_all(&user).unwrap();
        let journal = user.join(assets::USER_JOURNAL);
        sql(&journal, OPERATIONS_DDL);
        sql(
            &journal,
            "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted) VALUES
               ('pinyin','ni''hao','拟好蒿','upsert',12345,'',1),
               ('pinyin','ni''hao''ni''hao''ni''hao''ni''hao','你好你好你好你好','upsert',7,'',1),
               ('wubi','wqvb','合成五笔','upsert',9,'',1),
               ('quick','zzfixture','合成短语','upsert',10,'',1),
               ('english','zzfixture','Zzfixture','upsert',11,'Zzfixture',1)",
        );
        let paths =
            prepare_runtime_paths(&resources, &user, &root.path().join("cache"), "real").unwrap();
        for name in [assets::BIGRAM_TABLE, assets::TRIGRAM_TABLE] {
            assert_eq!(
                fs::read(paths.dictionary(name)).unwrap(),
                fs::read(resources.join(name)).unwrap(),
                "{name}"
            );
        }
        let main = paths.dictionary(assets::MAIN_DICTIONARY);
        assert_eq!(
            weight(
                &main,
                "SELECT weight FROM tbl_2_n WHERE key='ni''hao' AND value='拟好蒿'"
            ),
            Some(12345)
        );
        assert_eq!(
            weight(
                &main,
                "SELECT weight FROM tbl_others_n WHERE value='你好你好你好你好'"
            ),
            Some(7)
        );
        assert_eq!(
            weight(&main, "SELECT weight FROM wubi86 WHERE value='合成五笔'"),
            Some(9)
        );
        assert_eq!(
            weight(
                &main,
                "SELECT weight FROM quick_parases WHERE value='合成短语'"
            ),
            Some(10)
        );
        assert_eq!(
            weight(
                &paths.dictionary(assets::ENGLISH_DICTIONARY),
                "SELECT weight FROM english_words WHERE word='zzfixture' AND display='Zzfixture'"
            ),
            Some(11)
        );
        let shipped =
            |path: &Path| weight(path, "SELECT count(*) FROM tbl_2_n WHERE key='ni''hao'").unwrap();
        assert_eq!(
            shipped(&main),
            shipped(&resources.join(assets::MAIN_DICTIONARY)) + 1
        );
    }

    fn japanese_only() -> SchemeSet {
        SchemeSet::of(&[crate::types::SchemeType::JapaneseRomaji])
    }

    /// 只有日文（或越南文、藏文）的版本不带 `msime-pinyin.db`：代次只复制 `msime-english.db`，日志里的英文词照样回放，拼音行跳过而不拒绝这个代次；再次准备同一个代次也不要求 `msime-pinyin.db`。读 `msime-pinyin.db` 的集合缺了它仍然失败，与以前相同。
    #[test]
    fn a_generation_without_the_main_dictionary_holds_only_english() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        fs::remove_file(resources.join(assets::MAIN_DICTIONARY)).unwrap();
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        let journal = user.join(assets::USER_JOURNAL);
        fs::create_dir_all(&user).unwrap();
        learn(&journal, "你", 1);
        sql(
            &journal,
            "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted,updated_at) VALUES('english','zzfixture','Zzfixture','upsert',11,'Zzfixture',1,2)",
        );

        let paths =
            prepare_runtime_paths_for(&resources, &user, &cache, "v1", japanese_only()).unwrap();
        assert!(paths.dictionaries.join(assets::GENERATION_READY).is_file());
        assert!(!paths.dictionary(assets::MAIN_DICTIONARY).exists());
        assert!(!resources.join(assets::MAIN_DICTIONARY).exists());
        assert_eq!(
            weight(
                &paths.dictionary(assets::ENGLISH_DICTIONARY),
                "SELECT weight FROM english_words WHERE word='zzfixture' AND display='Zzfixture'"
            ),
            Some(11)
        );

        // 已经准备好的代次再准备一次：不要求 msime-pinyin.db，日志再回放一次。
        let again =
            prepare_runtime_paths_for(&resources, &user, &cache, "v1", japanese_only()).unwrap();
        assert_eq!(again, paths);
        assert!(!again.dictionary(assets::MAIN_DICTIONARY).exists());

        // 同一个资源目录给读 msime-pinyin.db 的集合准备，照旧因为缺 msime-pinyin.db 失败。
        let error = prepare_runtime_paths(&resources, &user, &cache, "v2").unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "{}{}",
                diagnostics::RUNTIME_COPY_FAILED,
                resources.join(assets::MAIN_DICTIONARY).display()
            )
        );
        assert!(!user.join("dictionaries/v2").exists());
        // 读 msime-pinyin.db 的集合也不接受一个没有 msime-pinyin.db 的现成代次。
        let error = prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap_err();
        assert_eq!(
            error.to_string(),
            diagnostics::INCOMPLETE_RUNTIME_GENERATION
        );
    }

    /// 不读 msime-pinyin.db 的代次仍然要求 `msime-english.db`：资源目录缺了它就失败，不留下暂存目录。
    #[test]
    fn a_generation_without_the_main_dictionary_still_needs_english() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("missing-resources");
        fs::create_dir_all(&missing).unwrap();
        let user = root.path().join("user");
        let error = prepare_runtime_paths_for(
            &missing,
            &user,
            &root.path().join("cache"),
            "v1",
            japanese_only(),
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "{}{}",
                diagnostics::RUNTIME_COPY_FAILED,
                missing.join(assets::ENGLISH_DICTIONARY).display()
            )
        );
        assert!(!user.join("dictionaries/v1.incoming").exists());
        assert!(!user.join("dictionaries/v1").exists());
    }

    #[cfg(unix)]
    #[test]
    fn generation_copy_rejects_a_symlinked_source() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let source = outside.path().join("source.bin");
        let target = directory.path().join("target.bin");
        std::fs::write(&source, b"outside").unwrap();
        symlink(&source, directory.path().join("linked.bin")).unwrap();

        assert!(copy_private_file(&directory.path().join("linked.bin"), &target).is_err());
        assert!(!target.exists());
    }

    #[cfg(unix)]
    #[test]
    fn generation_marker_writing_rejects_a_symlink() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.txt");
        std::fs::write(&target, b"keep").unwrap();
        let marker = directory.path().join(".ready");
        symlink(&target, &marker).unwrap();

        assert!(write_private_file(&marker, b"ready\n").is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"keep");
    }
}
