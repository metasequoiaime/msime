//! Resetting learned data (user-dictionary.md §13, bridge.cpp:588-691): fresh copies of the packaged dictionaries and an empty journal, swapped in with backups and rolled back on any failure. Sibling names are built by concatenation so non-ASCII profile directories work.

use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::OpenFlags;

use crate::assets;
use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::paths::RuntimePaths;
use crate::user_dictionary::generation::{copy_private_file, merge_split_wubi, weakly_canonical};
use crate::user_dictionary::journal::{
    checkpoint_journal, close_cached_journals, ensure_schema, open_database,
};

/// SQLite files that sit beside the journal. A leftover `-wal` would bring the learned data the user just erased back on the next open.
const JOURNAL_SIDECARS: [&str; 3] = ["-wal", "-shm", "-journal"];

/// One file being swapped: the fresh copy is published over `target` while the original waits at `backup` until every file is in place.
struct Replacement {
    target: PathBuf,
    temporary: PathBuf,
    backup: PathBuf,
    had_original: bool,
    published: bool,
}

/// The caller must have quiesced every session on these paths.
///
/// 自己取独占的维护租约（`DictionaryAccess::try_maintenance`），在建好两个目录之后、动任何文件之前：还有会话或别的维护持着租约时以 `Busy`（`DICTIONARY_MAINTENANCE_BUSY`）失败，什么也不换。
pub fn reset_learned_data(paths: &RuntimePaths) -> Result<()> {
    reset(paths, true, true)
}

/// [`reset_learned_data`]，`main_dictionary` 为假（代次里没有 `msime-pinyin.db`，见 `SchemeSet::reads_main_dictionary`）时只换回 `msime-english.db` 和清空日志，不要求也不复制 `msime-pinyin.db`。不取维护租约，调用方（host 门面背后的 host-api）已经持有。
pub fn reset_learned_data_with(paths: &RuntimePaths, main_dictionary: bool) -> Result<()> {
    reset(paths, main_dictionary, false)
}

/// `take_lease` 为真时自己取维护租约，持到换完为止。
fn reset(paths: &RuntimePaths, main_dictionary: bool, take_lease: bool) -> Result<()> {
    paths.validate()?;
    if weakly_canonical(&paths.resources)? == weakly_canonical(&paths.dictionaries)? {
        return Err(EngineError::invalid(diagnostics::RESET_IN_PLACE));
    }
    let names: &[&str] = if main_dictionary {
        &[assets::MAIN_DICTIONARY, assets::ENGLISH_DICTIONARY]
    } else {
        &[assets::ENGLISH_DICTIONARY]
    };
    let sources: Vec<PathBuf> = names.iter().map(|name| paths.resource(name)).collect();
    if !sources.iter().all(|source| is_real_file(source)) {
        return Err(EngineError::failed(
            diagnostics::PACKAGED_DICTIONARY_UNAVAILABLE,
        ));
    }
    reject_redirected_directory(&paths.user_data)?;
    reject_redirected_directory(&paths.dictionaries)?;
    fs::create_dir_all(&paths.user_data)?;
    fs::create_dir_all(&paths.dictionaries)?;
    reject_redirected_directory(&paths.user_data)?;
    reject_redirected_directory(&paths.dictionaries)?;
    let _access = if take_lease {
        Some(crate::access::maintenance(paths)?)
    } else {
        None
    };
    // Every cached handle on the files about to be replaced goes first; on Windows an open handle blocks the rename.
    close_cached_journals();

    // Unique per call, so a reset interrupted earlier never collides with this one's files. A clock before 1970 would only make the names less unique, not wrong.
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos())
        .to_string();
    let journal = paths.user(assets::USER_JOURNAL);
    // 日志是 WAL 时，最近的学习可能还在 `-wal` 里、没并回主文件。先并回并清空 `-wal`：失败回滚或被杀后放回的备份本身就是完整的日志，新日志发布后、旁路文件删掉之前，原名旁边也只剩一个空 `-wal`，套不到新日志上。
    checkpoint_journal(&journal);
    // 词库在前、日志在最后：`swap` 按顺序把 `sources` 配给前面的词库。
    let mut replacements: Vec<Replacement> = names
        .iter()
        .map(|name| paths.dictionary(name))
        .chain([journal.clone()])
        .map(|target| Replacement {
            temporary: affixed(&target, ".reset.", &stamp),
            backup: affixed(&target, ".backup.", &stamp),
            target,
            had_original: false,
            published: false,
        })
        .collect();
    let journal_temporary = replacements[names.len()].temporary.clone();

    // The empty journal gets its own connection, closed before it is published: a cached one would keep the renamed file open.
    let prepared = (|| -> Result<()> {
        let connection = open_database(
            &journal_temporary,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        ensure_schema(&connection)
    })();
    if prepared.is_err() {
        remove_ignoring_errors(&journal_temporary);
        return Err(EngineError::failed(diagnostics::RESET_JOURNAL_FAILED));
    }

    let swapped = swap(
        &mut replacements,
        &paths.resources,
        main_dictionary,
        &sources,
        &journal,
    );
    match swapped {
        Ok(()) => {
            // Every replacement is published; the originals are no longer needed.
            for replacement in &replacements {
                remove_ignoring_errors(&replacement.backup);
            }
            Ok(())
        }
        Err(error) => {
            restore(&replacements);
            Err(error)
        }
    }
}

fn swap(
    replacements: &mut [Replacement],
    resources: &Path,
    main_dictionary: bool,
    sources: &[PathBuf],
    journal: &Path,
) -> Result<()> {
    // A plain copy, as the reference made: the packaged bundle is immutable, so it has no WAL to lose.
    for (source, replacement) in sources.iter().zip(replacements.iter()) {
        copy_private_file(source, &replacement.temporary)
            .map_err(|_| EngineError::failed(diagnostics::RESET_STAGE_DICTIONARIES_FAILED))?;
    }
    // 与准备代次相同：单独发布的五笔码表并回新的工作主词库，否则重置之后五笔学习与删词都找不到表。没有工作主词库时 `replacements[0]` 是英文词库，不并。
    if main_dictionary {
        merge_split_wubi(resources, &replacements[0].temporary)
            .map_err(|_| EngineError::failed(diagnostics::RESET_STAGE_DICTIONARIES_FAILED))?;
    }
    for replacement in replacements.iter_mut() {
        replacement.had_original = replacement.target.exists();
        if replacement.had_original {
            if replacement.backup.exists() {
                remove_ignoring_errors(&replacement.backup);
            }
            // The original is held aside until every replacement is published.
            fs::rename(&replacement.target, &replacement.backup)
                .map_err(|_| EngineError::failed(diagnostics::RESET_STAGE_FAILED))?;
        }
        fs::rename(&replacement.temporary, &replacement.target)
            .map_err(|_| EngineError::failed(diagnostics::RESET_PUBLISH_FAILED))?;
        replacement.published = true;
    }
    for suffix in JOURNAL_SIDECARS {
        let mut sidecar = journal.as_os_str().to_owned();
        sidecar.push(suffix);
        match fs::remove_file(PathBuf::from(sidecar)) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Undo in reverse order: drop what was published, put each original back, remove the fresh copies. Best effort, since the error being returned is the one that matters (bridge.cpp:648-655).
fn restore(replacements: &[Replacement]) {
    for replacement in replacements.iter().rev() {
        if replacement.published {
            remove_ignoring_errors(&replacement.target);
        }
        if replacement.had_original && replacement.backup.exists() {
            let _ = fs::rename(&replacement.backup, &replacement.target);
        }
        remove_ignoring_errors(&replacement.temporary);
    }
}

/// 重置在两次改名之间被杀时，从时间戳最新的普通文件备份恢复缺失的 `target`，再删除源文件名下同一时间戳没发布的 `.reset.` 副本。词库现名没有备份时也查找合法旧名备份，发布到请求的现名，供代次回放使用；现有文件与备份都按现名优先。准备代次时在打开词库和日志之前调用。
///
/// `target` 在（包括符号链接）时不动它，最终发布也不得覆盖检查之后竞争者放回的文件；失败保留备份。恢复后原重置进程若还活着，它缺少已删除的 `.reset.` 副本，后续发布失败会走原有回滚；这里不提供跨文件事务原子性。
pub(crate) fn restore_interrupted_reset(target: &Path) {
    match fs::symlink_metadata(target) {
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        _ => return,
    }
    // 旧文件挪开后 `RuntimePaths::dictionary` 已看不到它，只会给出现名；备份仍使用旧文件名。
    let backup = newest_reset_backup(target).or_else(|| {
        let name = target.file_name()?.to_str()?;
        let (_, legacy) = assets::LEGACY_NAMES
            .iter()
            .find(|(current, _)| *current == name)?;
        let legacy = target.with_file_name(legacy);
        match fs::symlink_metadata(&legacy) {
            Err(error) if error.kind() == ErrorKind::NotFound => newest_reset_backup(&legacy),
            _ => None,
        }
    });
    if let Some(backup) = backup {
        backup.publish(target);
    }
}

struct ResetBackup {
    path: PathBuf,
    unpublished: PathBuf,
}

/// 选择与发布分开：目录枚举结束后，其他准备者仍可能先把目标放回来。
fn newest_reset_backup(target: &Path) -> Option<ResetBackup> {
    let directory = target.parent()?;
    let name = target.file_name()?.to_str()?;
    let prefix = format!(".{name}.backup.");
    fs::read_dir(directory)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let stamp = entry
                .file_name()
                .to_str()?
                .strip_prefix(&prefix)?
                .to_owned();
            let order = stamp
                .parse::<u128>()
                .ok()
                .filter(|_| stamp.bytes().all(|byte| byte.is_ascii_digit()))?;
            is_real_file(&entry.path()).then_some((order, stamp, entry.path()))
        })
        .max_by_key(|(order, _, _)| *order)
        .map(|(_, stamp, path)| ResetBackup {
            path,
            unpublished: affixed(target, ".reset.", &stamp),
        })
}

impl ResetBackup {
    fn publish(self, target: &Path) {
        let Ok(mut backup) = tempfile::TempPath::try_from_path(self.path) else {
            return;
        };
        // 这是唯一的原件，失败或输掉发布竞争都不能让 `TempPath::drop` 删除它。
        backup.disable_cleanup(true);
        if backup.persist_noclobber(target).is_ok() {
            remove_ignoring_errors(&self.unpublished);
        }
    }
}

/// `.<name><infix><stamp>` beside `target`, built on the native string so a profile directory such as `C:\Users\陆傲天` survives unchanged; everything added is ASCII.
fn affixed(target: &Path, infix: &str, stamp: &str) -> PathBuf {
    let mut name = OsString::from(".");
    name.push(target.file_name().unwrap_or_default());
    name.push(infix);
    name.push(stamp);
    target.with_file_name(name)
}

fn remove_ignoring_errors(path: &Path) {
    if path.is_dir() {
        let _ = fs::remove_dir_all(path);
    } else {
        let _ = fs::remove_file(path);
    }
}

fn is_real_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
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
                    "reset directory has a symbolic-link ancestor",
                ));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotADirectory,
                    "reset directory parent is not a directory",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_dictionary::replay::tests::{sql, weight};

    /// 两个准备者先看到缺失目标；最新备份先发布后，迟到的旧备份不能覆盖它，也不能被失败路径清理。
    #[test]
    fn a_late_reset_recovery_preserves_the_winners_file_and_its_own_backup() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join(assets::USER_JOURNAL);
        let older = affixed(&target, ".backup.", "100");
        let newest = affixed(&target, ".backup.", "200");
        let older_stage = affixed(&target, ".reset.", "100");
        fs::write(&older, b"older synthetic journal").unwrap();
        fs::write(&newest, b"newest synthetic journal").unwrap();
        fs::write(&older_stage, b"unpublished synthetic journal").unwrap();
        assert_eq!(
            fs::symlink_metadata(&target).unwrap_err().kind(),
            ErrorKind::NotFound
        );

        newest_reset_backup(&target).unwrap().publish(&target);
        // 迟到者此前同样看到了缺失；枚举时最新备份已被赢家取走，只剩旧备份。
        newest_reset_backup(&target).unwrap().publish(&target);

        assert_eq!(fs::read(&target).unwrap(), b"newest synthetic journal");
        assert_eq!(fs::read(&older).unwrap(), b"older synthetic journal");
        assert_eq!(
            fs::read(&older_stage).unwrap(),
            b"unpublished synthetic journal"
        );
    }

    /// 现有词库沿用路径解析的优先级；两种名称都只有备份时优先恢复现名备份。
    #[test]
    fn reset_recovery_follows_current_and_legacy_name_priority() {
        for (name, legacy) in [
            (assets::MAIN_DICTIONARY, "msime.db"),
            (assets::ENGLISH_DICTIONARY, "english.db"),
        ] {
            let root = tempfile::tempdir().unwrap();
            let paths = RuntimePaths {
                dictionaries: root.path().to_owned(),
                ..RuntimePaths::default()
            };
            let current = root.path().join(name);
            let old = root.path().join(legacy);
            let current_backup = affixed(&current, ".backup.", "100");
            let old_backup = affixed(&old, ".backup.", "200");
            fs::write(&current, b"existing synthetic dictionary").unwrap();
            fs::write(&old_backup, b"legacy synthetic backup").unwrap();
            restore_interrupted_reset(&paths.dictionary(name));
            assert_eq!(
                fs::read(&current).unwrap(),
                b"existing synthetic dictionary"
            );
            assert!(old_backup.exists());

            fs::rename(&current, &old).unwrap();
            fs::write(&current_backup, b"current synthetic backup").unwrap();
            restore_interrupted_reset(&paths.dictionary(name));
            assert_eq!(fs::read(&old).unwrap(), b"existing synthetic dictionary");
            assert!(current_backup.exists());
            assert!(!current.exists());

            fs::remove_file(&old).unwrap();
            restore_interrupted_reset(&paths.dictionary(name));
            assert_eq!(fs::read(&current).unwrap(), b"current synthetic backup");
            assert!(!current_backup.exists());
            assert_eq!(fs::read(&old_backup).unwrap(), b"legacy synthetic backup");
        }
    }

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

    /// 引擎层的重置自己取维护租约：还有会话持共享租约时报 `Busy`，词库和日志原样不动；会话结束后照常重置。
    #[test]
    fn reset_takes_the_maintenance_lease() {
        let temporary = tempfile::tempdir().unwrap();
        let paths = paths(temporary.path());
        sql(
            &paths.resource(assets::MAIN_DICTIONARY),
            "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);",
        );
        sql(
            &paths.resource(assets::ENGLISH_DICTIONARY),
            "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);",
        );
        fs::create_dir_all(&paths.user_data).unwrap();
        fs::create_dir_all(&paths.dictionaries).unwrap();
        let journal = paths.user(assets::USER_JOURNAL);
        sql(&journal, "CREATE TABLE marker(value INTEGER);");
        let session = crate::DictionaryAccess::try_session(&paths.user_data, &paths.dictionaries)
            .unwrap()
            .unwrap();

        let error = reset_learned_data(&paths).unwrap_err();
        assert_eq!(error.code(), crate::ErrorCode::Busy);
        let marker = "SELECT count(*) FROM sqlite_master WHERE name='marker'";
        assert_eq!(weight(&journal, marker), Some(1));
        assert!(!paths.dictionary(assets::MAIN_DICTIONARY).exists());

        drop(session);
        reset_learned_data(&paths).unwrap();
        assert!(paths.dictionary(assets::MAIN_DICTIONARY).exists());
        assert_eq!(weight(&journal, marker), Some(0));
    }

    /// 拆分发布布局：`msime-pinyin.db` 没有五笔表，重置后的工作主词库要把 `msime-wubi.db` 的码表并回来，五笔学习与删词才有表可写。
    #[test]
    fn reset_merges_the_split_wubi_tables_into_the_fresh_working_copy() {
        let temporary = tempfile::tempdir().unwrap();
        let paths = paths(temporary.path());
        sql(
            &paths.resource(assets::MAIN_DICTIONARY),
            "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);",
        );
        sql(
            &paths.resource(assets::WUBI_DICTIONARY),
            "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
             INSERT INTO wubi86 VALUES('aaaa','工',200);
             CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER);",
        );
        sql(
            &paths.resource(assets::ENGLISH_DICTIONARY),
            "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);",
        );

        reset_learned_data(&paths).unwrap();

        assert_eq!(
            weight(
                &paths.dictionary(assets::MAIN_DICTIONARY),
                "SELECT weight FROM wubi86 WHERE key='aaaa'"
            ),
            Some(200)
        );
        assert_eq!(
            weight(
                &paths.dictionary(assets::MAIN_DICTIONARY),
                "SELECT count(*) FROM wubi98"
            ),
            Some(0)
        );
        assert!(!paths.dictionary(assets::WUBI_DICTIONARY).exists());
    }

    // engine-bridge tests.rs `reset_learned_data_restores_packaged_dictionaries_and_clears_journal`, for an ASCII root and one carrying Chinese characters.
    #[test]
    fn reset_restores_packaged_dictionaries_and_clears_the_journal() {
        for component in ["ascii", "陆傲天"] {
            let temporary = tempfile::tempdir().unwrap();
            let paths = paths(&temporary.path().join(component));
            sql(
                &paths.resource(assets::MAIN_DICTIONARY),
                "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',100);
                 CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                 CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);",
            );
            sql(
                &paths.resource(assets::ENGLISH_DICTIONARY),
                "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);
                 CREATE TABLE en_zh_glosses(english TEXT PRIMARY KEY,chinese_gloss TEXT);
                 CREATE TABLE zh_en_glosses(chinese TEXT PRIMARY KEY,english TEXT);
                 INSERT INTO english_words VALUES('word','word',100);",
            );
            for name in [assets::MAIN_DICTIONARY, assets::ENGLISH_DICTIONARY] {
                fs::copy(paths.resource(name), paths.dictionary(name)).unwrap();
            }
            let journal = paths.user(assets::USER_JOURNAL);
            // The legacy journal shape the bridge test used: no primary key, no `updated_at`.
            sql(
                &journal,
                "CREATE TABLE user_dictionary_operations(dictionary TEXT,key TEXT,value TEXT,operation TEXT,weight INTEGER,display TEXT,user_inserted INTEGER);
                 CREATE TABLE personal_dictionary_receipts(request_id TEXT PRIMARY KEY,payload TEXT);
                 CREATE TABLE candidate_selection_state(context_key TEXT,entry_key TEXT,value TEXT,selection_count INTEGER);
                 CREATE TABLE fixed_candidate_positions(context_key TEXT,entry_key TEXT,value TEXT,position INTEGER);
                 INSERT INTO user_dictionary_operations VALUES('pinyin','ni''hao','你好','upsert',1,'',1);
                 INSERT INTO candidate_selection_state VALUES('ni''hao','ni''hao','你好',7);",
            );
            sql(
                &paths.dictionary(assets::MAIN_DICTIONARY),
                "UPDATE tbl_2_n SET weight=1",
            );
            let mut wal = journal.as_os_str().to_owned();
            wal.push("-wal");
            fs::write(PathBuf::from(&wal), b"stale learned data").unwrap();

            reset_learned_data(&paths).unwrap();

            assert_eq!(
                weight(
                    &paths.dictionary(assets::MAIN_DICTIONARY),
                    "SELECT weight FROM tbl_2_n WHERE key='ni''hao'"
                ),
                Some(100)
            );
            assert_eq!(
                weight(&journal, "SELECT count(*) FROM user_dictionary_operations"),
                Some(0)
            );
            assert_eq!(
                weight(&journal, "SELECT count(*) FROM candidate_selection_state"),
                Some(0)
            );
            // The fresh journal has the full v4 schema.
            assert_eq!(
                weight(&journal, "SELECT count(*) FROM pinned_candidates"),
                Some(0)
            );
            assert!(!PathBuf::from(&wal).exists());
            let leftovers: Vec<_> = [&paths.user_data, &paths.dictionaries]
                .iter()
                .flat_map(|directory| fs::read_dir(directory).unwrap())
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                // 维护租约的锁文件是稳定的协调对象，留着是对的（`access::LOCK_FILE`）。
                .filter(|name| name.starts_with('.') && name != crate::access::LOCK_FILE)
                .collect();
            assert!(leftovers.is_empty(), "{leftovers:?}");
        }
    }

    /// 重置学习数据连双拼按两键码学的纠错（#10018）一起清掉：新日志里两张表是空的，共用的纠错档案下次刷新就读不到旧记录。
    #[test]
    fn reset_clears_shuangpin_typo_learning() {
        use crate::types::ShuangpinProfileKind;
        use crate::user_dictionary::typo_profile::{PersonalTypoProfile, ShuangpinTypoOutcome};
        let temporary = tempfile::tempdir().unwrap();
        let paths = paths(temporary.path());
        sql(
            &paths.resource(assets::MAIN_DICTIONARY),
            "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);",
        );
        sql(
            &paths.resource(assets::ENGLISH_DICTIONARY),
            "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);",
        );
        let journal = paths.user(assets::USER_JOURNAL);
        let profile = PersonalTypoProfile::shared(&journal);
        profile
            .record_shuangpin(
                ShuangpinProfileKind::Xiaohe,
                &[(*b"gf", *b"gr")],
                ShuangpinTypoOutcome::Accepted,
            )
            .unwrap();
        profile
            .record_shuangpin_suppression(ShuangpinProfileKind::Xiaohe, "mwgfxi")
            .unwrap();
        assert!(profile.has_shuangpin_counts());

        reset_learned_data(&paths).unwrap();

        for table in [
            "shuangpin_typo_counts",
            "shuangpin_autocorrect_suppressions",
        ] {
            assert_eq!(
                weight(&journal, &format!("SELECT count(*) FROM {table}")),
                Some(0),
                "{table}"
            );
        }
        profile.refresh_if_changed();
        assert!(!profile.has_shuangpin_counts());
        assert!(!profile.shuangpin_suppressed(ShuangpinProfileKind::Xiaohe, "mwgfxi"));
    }

    #[test]
    fn reset_refuses_the_bundle_in_place_and_a_missing_package() {
        let root = tempfile::tempdir().unwrap();
        let mut paths = paths(root.path());
        let error = reset_learned_data(&paths).unwrap_err();
        assert_eq!(
            error.to_string(),
            diagnostics::PACKAGED_DICTIONARY_UNAVAILABLE
        );
        paths.dictionaries = paths.resources.join(".");
        let error = reset_learned_data(&paths).unwrap_err();
        assert!(matches!(error, EngineError::InvalidArgument(_)));
        assert_eq!(error.to_string(), diagnostics::RESET_IN_PLACE);
    }

    /// 没有 `msime-pinyin.db` 的代次：重置只换回 `msime-english.db`、清空日志，不要求资源目录里有 `msime-pinyin.db`，也不在代次里留下它；同一个资源目录按有 `msime-pinyin.db` 重置仍然被拒。
    #[test]
    fn reset_without_the_main_dictionary_restores_only_english() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        sql(
            &paths.resource(assets::ENGLISH_DICTIONARY),
            "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);
             INSERT INTO english_words VALUES('word','word',100);",
        );
        fs::copy(
            paths.resource(assets::ENGLISH_DICTIONARY),
            paths.dictionary(assets::ENGLISH_DICTIONARY),
        )
        .unwrap();
        sql(
            &paths.dictionary(assets::ENGLISH_DICTIONARY),
            "UPDATE english_words SET weight=1",
        );
        let journal = paths.user(assets::USER_JOURNAL);
        sql(
            &journal,
            "CREATE TABLE user_dictionary_operations(dictionary TEXT,key TEXT,value TEXT,operation TEXT,weight INTEGER,display TEXT,user_inserted INTEGER);
             INSERT INTO user_dictionary_operations VALUES('english','word','word','upsert',1,'word',0);",
        );

        assert_eq!(
            reset_learned_data(&paths).unwrap_err().to_string(),
            diagnostics::PACKAGED_DICTIONARY_UNAVAILABLE
        );
        reset_learned_data_with(&paths, false).unwrap();

        assert_eq!(
            weight(
                &paths.dictionary(assets::ENGLISH_DICTIONARY),
                "SELECT weight FROM english_words WHERE word='word'"
            ),
            Some(100)
        );
        assert_eq!(
            weight(&journal, "SELECT count(*) FROM user_dictionary_operations"),
            Some(0)
        );
        assert!(!paths.dictionary(assets::MAIN_DICTIONARY).exists());
        assert!(!paths.resource(assets::MAIN_DICTIONARY).exists());
        let leftovers: Vec<_> = [&paths.user_data, &paths.dictionaries]
            .iter()
            .flat_map(|directory| fs::read_dir(directory).unwrap())
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            // 维护租约的锁文件是稳定的协调对象，留着是对的（`access::LOCK_FILE`）。
            .filter(|name| name.starts_with('.') && name != crate::access::LOCK_FILE)
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[cfg(unix)]
    #[test]
    fn reset_rejects_a_symlinked_packaged_dictionary() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        sql(
            &paths.resource(assets::ENGLISH_DICTIONARY),
            "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);",
        );
        let external = root.path().join("external-msime-pinyin.db");
        sql(
            &external,
            "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);",
        );
        symlink(&external, paths.resource(assets::MAIN_DICTIONARY)).unwrap();

        assert_eq!(
            reset_learned_data(&paths).unwrap_err().to_string(),
            diagnostics::PACKAGED_DICTIONARY_UNAVAILABLE
        );
        assert!(!paths.dictionary(assets::MAIN_DICTIONARY).exists());
    }

    #[cfg(unix)]
    #[test]
    fn reset_rejects_a_symlinked_dictionary_directory() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        for name in [assets::MAIN_DICTIONARY, assets::ENGLISH_DICTIONARY] {
            sql(&paths.resource(name), "CREATE TABLE packaged(x);");
        }
        fs::remove_dir(&paths.dictionaries).unwrap();
        symlink(outside.path(), &paths.dictionaries).unwrap();

        assert!(reset_learned_data(&paths).is_err());
        assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
    }

    #[test]
    fn a_failed_publish_puts_every_original_back() {
        let root = tempfile::tempdir().unwrap();
        let paths = paths(root.path());
        sql(
            &paths.resource(assets::MAIN_DICTIONARY),
            "CREATE TABLE fresh(x);",
        );
        sql(
            &paths.resource(assets::ENGLISH_DICTIONARY),
            "CREATE TABLE fresh(x);",
        );
        sql(
            &paths.dictionary(assets::MAIN_DICTIONARY),
            "CREATE TABLE learned(x);",
        );
        sql(
            &paths.dictionary(assets::ENGLISH_DICTIONARY),
            "CREATE TABLE learned(x);",
        );
        let journal = paths.user(assets::USER_JOURNAL);
        sql(&journal, "CREATE TABLE learned(x);");
        // A `-wal` sidecar that is a directory cannot be removed as a file, so the reset fails after all three files were published.
        let mut wal = journal.as_os_str().to_owned();
        wal.push("-wal");
        let wal = PathBuf::from(wal);
        fs::create_dir_all(wal.join("blocker")).unwrap();
        let before: Vec<Vec<u8>> = [
            paths.dictionary(assets::MAIN_DICTIONARY),
            paths.dictionary(assets::ENGLISH_DICTIONARY),
            journal.clone(),
        ]
        .iter()
        .map(|path| fs::read(path).unwrap())
        .collect();

        assert!(reset_learned_data(&paths).is_err());

        let after: Vec<Vec<u8>> = [
            paths.dictionary(assets::MAIN_DICTIONARY),
            paths.dictionary(assets::ENGLISH_DICTIONARY),
            journal.clone(),
        ]
        .iter()
        .map(|path| fs::read(path).unwrap())
        .collect();
        assert_eq!(after, before);
        assert!(wal.join("blocker").is_dir());
        let leftovers: Vec<_> = [&paths.user_data, &paths.dictionaries]
            .iter()
            .flat_map(|directory| fs::read_dir(directory).unwrap())
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            // 维护租约的锁文件是稳定的协调对象，留着是对的（`access::LOCK_FILE`）。
            .filter(|name| name.starts_with('.') && name != crate::access::LOCK_FILE)
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }
}
