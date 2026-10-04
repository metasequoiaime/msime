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
use crate::user_dictionary::generation::{merge_split_wubi, weakly_canonical};
use crate::user_dictionary::journal::{close_cached_journals, ensure_schema, open_database};

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
pub fn reset_learned_data(paths: &RuntimePaths) -> Result<()> {
    paths.validate()?;
    if weakly_canonical(&paths.resources)? == weakly_canonical(&paths.dictionaries)? {
        return Err(EngineError::invalid(diagnostics::RESET_IN_PLACE));
    }
    let main_source = paths.resource(assets::MAIN_DICTIONARY);
    let english_source = paths.resource(assets::ENGLISH_DICTIONARY);
    if !is_real_file(&main_source) || !is_real_file(&english_source) {
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
    // Every cached handle on the files about to be replaced goes first; on Windows an open handle blocks the rename.
    close_cached_journals();

    // Unique per call, so a reset interrupted earlier never collides with this one's files. A clock before 1970 would only make the names less unique, not wrong.
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos())
        .to_string();
    let journal = paths.user(assets::USER_JOURNAL);
    let mut replacements: Vec<Replacement> = [
        paths.dictionary(assets::MAIN_DICTIONARY),
        paths.dictionary(assets::ENGLISH_DICTIONARY),
        journal.clone(),
    ]
    .into_iter()
    .map(|target| Replacement {
        temporary: affixed(&target, ".reset.", &stamp),
        backup: affixed(&target, ".backup.", &stamp),
        target,
        had_original: false,
        published: false,
    })
    .collect();

    // The empty journal gets its own connection, closed before it is published: a cached one would keep the renamed file open.
    let prepared = (|| -> Result<()> {
        let connection = open_database(
            &replacements[2].temporary,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        ensure_schema(&connection)
    })();
    if prepared.is_err() {
        remove_ignoring_errors(&replacements[2].temporary);
        return Err(EngineError::failed(diagnostics::RESET_JOURNAL_FAILED));
    }

    let swapped = swap(
        &mut replacements,
        &paths.resources,
        &main_source,
        &english_source,
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
    main_source: &Path,
    english_source: &Path,
    journal: &Path,
) -> Result<()> {
    // A plain copy, as the reference made: the packaged bundle is immutable, so it has no WAL to lose.
    for (source, replacement) in [main_source, english_source]
        .iter()
        .zip(replacements.iter())
    {
        fs::copy(source, &replacement.temporary)
            .map_err(|_| EngineError::failed(diagnostics::RESET_STAGE_DICTIONARIES_FAILED))?;
    }
    // 与准备代次相同：单独发布的五笔码表并回新的工作主词库，否则重置之后五笔学习与删词都找不到表。
    merge_split_wubi(resources, &replacements[0].temporary)
        .map_err(|_| EngineError::failed(diagnostics::RESET_STAGE_DICTIONARIES_FAILED))?;
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
                .filter(|name| name.starts_with('.'))
                .collect();
            assert!(leftovers.is_empty(), "{leftovers:?}");
        }
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
            .filter(|name| name.starts_with('.'))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }
}
