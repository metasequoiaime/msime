//! Generation staging (core-session.md §12, data-formats.md §3, `runtime_paths.cpp:116-182`): `user_data/dictionaries/<content id>` holds backup-API copies of `msime.db` and `english.db` with the journal replayed, plus the n-gram tables. 不读 `msime.db` 的方案集合（见 `SchemeSet::reads_main_dictionary`）准备的代次只有 `english.db`。

use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};

use crate::assets;
use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::paths::RuntimePaths;
use crate::types::SchemeSet;
use crate::user_dictionary::replay::{replay, replay_english};

pub const MAX_CONTENT_ID_LENGTH: usize = 128;

/// Tables the lattice reads through `RuntimePaths::dictionary`. They are built from one generation's vocabulary, so they live beside it, and a resource set without them simply leaves the decoder without them (RP:56-59).
const GENERATION_COPIES: [&str; 2] = [assets::BIGRAM_TABLE, assets::TRIGRAM_TABLE];

/// Validate the content id (1..=128 of `[0-9A-Za-z_-]`) and the disjoint absolute roots, create `user_data` and `cache`, then either re-replay an existing ready generation (copying n-gram tables it lacks) or stage `<id>.incoming`, replay, write `.ready` and rename it into place. A failed staging removes only its own directory.
pub fn prepare_runtime_paths(
    resources: &Path,
    user_data: &Path,
    cache: &Path,
    content_id: &str,
) -> Result<RuntimePaths> {
    prepare_runtime_paths_for(resources, user_data, cache, content_id, SchemeSet::ALL)
}

/// 按会话允许的方案准备代次。`schemes` 读 `msime.db`（[`SchemeSet::reads_main_dictionary`]）时与 [`prepare_runtime_paths`] 完全相同：`msime.db` 和 `english.db` 都必须在资源目录里，都复制进代次。不读它时（只有日文、越南文、藏文这类方案的版本）资源目录里本来就没有它：代次只复制并要求 `english.db`，日志只回放英文行（`replay_english`）。
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

    if result.dictionaries.join(assets::GENERATION_READY).exists() {
        for name in generation_dictionaries(main_dictionary) {
            if !is_real_file(&result.dictionary(name)) {
                return Err(EngineError::failed(
                    diagnostics::INCOMPLETE_RUNTIME_GENERATION,
                ));
            }
        }
        // A host can switch back to a previously prepared generation. Replay the current journal again so changes learned on a newer generation survive that switch (RP:143-144).
        replay_into(&result, &result.dictionaries, main_dictionary)?;
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
        stage_generation_copies(resources, &stage, true)?;
        replay_into(&result, &stage, main_dictionary)?;
        fs::write(
            stage.join(assets::GENERATION_READY),
            format!("{content_id}\n"),
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

/// 代次里的词库工作副本：`main_dictionary` 为假时没有 `msime.db`，只有 `english.db`。
fn generation_dictionaries(main_dictionary: bool) -> &'static [&'static str] {
    if main_dictionary {
        &[assets::MAIN_DICTIONARY, assets::ENGLISH_DICTIONARY]
    } else {
        &[assets::ENGLISH_DICTIONARY]
    }
}

/// Replay the journal of `paths` into the dictionaries in `generation`; any failed row or error refuses the generation. 没有 `msime.db` 的代次只回放英文行。
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
        let input = Connection::open_with_flags(
            source,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let mut output = Connection::open_with_flags(
            target,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
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
        fs::copy(&source, &incoming)?;
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
        let external = root.path().join("external-msime.db");
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
        assert!(!paths.dictionary("trigram.bin.incoming").exists());
        let fresh = prepare_runtime_paths(&resources, &user, &cache, "v2").unwrap();
        assert_eq!(
            fs::read(fresh.dictionary(assets::BIGRAM_TABLE)).unwrap(),
            b"second"
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

    /// The shipped dict-v2.0.1 set stages whole: both databases copied, both n-gram tables byte for byte, and a journal of every kind replays onto the real tables.
    #[test]
    fn the_real_dictionary_set_stages_and_replays() {
        let Some(resources) = std::env::var_os("MSIME_EVAL_RESOURCES") else {
            eprintln!(
                "skipped: MSIME_EVAL_RESOURCES is not set to the dict-v2.0.1 resource directory"
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

    /// 只有日文（或越南文、藏文）的版本不带 `msime.db`：代次只复制 `english.db`，日志里的英文词照样回放，拼音行跳过而不拒绝这个代次；再次准备同一个代次也不要求 `msime.db`。读 `msime.db` 的集合缺了它仍然失败，与以前相同。
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

        // 已经准备好的代次再准备一次：不要求 msime.db，日志再回放一次。
        let again =
            prepare_runtime_paths_for(&resources, &user, &cache, "v1", japanese_only()).unwrap();
        assert_eq!(again, paths);
        assert!(!again.dictionary(assets::MAIN_DICTIONARY).exists());

        // 同一个资源目录给读 msime.db 的集合准备，照旧因为缺 msime.db 失败。
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
        // 读 msime.db 的集合也不接受一个没有 msime.db 的现成代次。
        let error = prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap_err();
        assert_eq!(
            error.to_string(),
            diagnostics::INCOMPLETE_RUNTIME_GENERATION
        );
    }

    /// 不读 msime.db 的代次仍然要求 `english.db`：资源目录缺了它就失败，不留下暂存目录。
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
}
