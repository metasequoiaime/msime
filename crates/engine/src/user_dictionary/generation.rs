//! 代次准备（core-session.md §12、data-formats.md §3、`runtime_paths.cpp:116-182`）：`user_data/dictionaries/<content id>` 里是经 backup API 复制的 `msime-pinyin.db` 与 `msime-english.db`（资源单独发布的 `msime-wubi.db` 五笔码表并回前者），回放过用户日志，旁边还有 n-gram 表。不读 `msime-pinyin.db` 的方案集合（见 `SchemeSet::reads_main_dictionary`）准备的代次没有它；不要求 `msime-english.db` 的集合（见 `SchemeSet::requires_english_dictionary`）在资源目录没带它时，代次里也没有它。

use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};

use crate::assets;
use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::paths::RuntimePaths;
use crate::types::SchemeSet;
use crate::user_dictionary::journal::{open_database, quarantine_corrupt_journal};
use crate::user_dictionary::replay::{replay, replay_english};
use crate::user_dictionary::reset::restore_interrupted_reset;
use crate::wubi::provider::ensure_reverse_indexes;

pub const MAX_CONTENT_ID_LENGTH: usize = 128;

/// Tables the lattice reads through `RuntimePaths::dictionary`. They are built from one generation's vocabulary, so they live beside it, and a resource set without them simply leaves the decoder without them (RP:56-59).
const GENERATION_COPIES: [&str; 2] = [assets::BIGRAM_TABLE, assets::TRIGRAM_TABLE];

/// 暂存目录名的后缀，见 [`prepare_runtime_paths_for`]。
const STAGE_SUFFIX: &str = ".incoming";

/// 清理时先把被丢下的暂存目录改名成这个后缀认领，再删。
const CLAIMED_STAGE_SUFFIX: &str = ".removing";

/// 暂存目录多久没有任何写入才算被丢下。真在准备的进程一直在写：复制约 200 MB 的词库、回放日志、建索引，最慢的设备上两次写入之间也只隔几十秒，一次完整准备在几分钟以内；一小时留出几十倍余量。代价是被杀留下的半截目录要等下一次在一小时后的准备才被删，期间只占空间，不再挡住准备。
const ABANDONED_STAGE_AGE: Duration = Duration::from_secs(60 * 60);

/// `msime-wubi.db` 里单独发布、准备代次时并回工作主词库的五笔码表。
const SPLIT_WUBI_TABLES: [&str; 2] = ["wubi86", "wubi98"];

/// 校验代次 id（1 到 128 个 `[0-9A-Za-z_-]`）和三个互不重叠的绝对路径根，建好 `user_data` 和 `cache`，清掉被丢下的暂存目录，然后要么对已就绪的代次重新回放日志（补上它缺的 n-gram 表），要么在这次尝试独有的 `<id>.<随机串>.incoming` 里暂存、回放、写 `.ready` 再改名就位。暂存失败只删自己的目录；同一个代次被别的进程抢先发布时用那一份。
pub fn prepare_runtime_paths(
    resources: &Path,
    user_data: &Path,
    cache: &Path,
    content_id: &str,
) -> Result<RuntimePaths> {
    prepare_runtime_paths_for(resources, user_data, cache, content_id, SchemeSet::ALL)
}

/// 按会话允许的方案准备代次，资源目录只要有这些方案读的词库。`schemes` 读 `msime-pinyin.db`（[`SchemeSet::reads_main_dictionary`]，即含全拼、双拼或五笔）时与 [`prepare_runtime_paths`] 完全相同：`msime-pinyin.db` 和 `msime-english.db` 都必须在资源目录里，都复制进代次。不读它时（只有日文、韩语、越南语、藏文、粤语、注音、笔画这类方案）资源目录不必有它，代次里也没有它；这样的集合也不要求 `msime-english.db`（[`SchemeSet::requires_english_dictionary`]），资源目录带了它就复制进代次、日志只回放英文行（`replay_english`），没带就连它也没有、日志一行也不回放（记录留在日志里，以后带英文库的代次照样回放）。粤语、注音、笔画的语言库和日文模型不经过代次，会话从 `SessionOptions` 给的路径或资源目录直接读。
pub fn prepare_runtime_paths_for(
    resources: &Path,
    user_data: &Path,
    cache: &Path,
    content_id: &str,
    schemes: SchemeSet,
) -> Result<RuntimePaths> {
    let main_dictionary = schemes.reads_main_dictionary();
    let english_required = main_dictionary || schemes.requires_english_dictionary();
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
    // 重置途中被杀时日志可能还挪在备份里，先放回来，否则下面会当作没有日志、从空日志开始。
    restore_interrupted_reset(&result.user(assets::USER_JOURNAL));
    // 不是数据库的日志会在下面的回放中每次启动都失败，让宿主永远无法启动；先把它挪开，从空日志开始。宿主 API 会在进入这里之前上报挪动结果；这里覆盖其余调用方。
    if let Some(moved) = quarantine_corrupt_journal(&result.user(assets::USER_JOURNAL))? {
        crate::logging::warn(format_args!(
            "user dictionary journal is not a database, moved aside to {}",
            moved.display()
        ));
    }

    let parent = user_data.join("dictionaries");
    sweep_abandoned_stages(&parent, SystemTime::now());

    let marker = result.dictionaries.join(assets::GENERATION_READY);
    if generation_ready(&marker)? {
        return open_ready_generation(result, resources, main_dictionary, english_required);
    }
    // 不要求英文库的集合有没有它，看资源目录里有没有这一项：符号链接之类也算有，交给下面的复制按原来的方式拒绝。
    let dictionaries = GenerationDictionaries {
        main: main_dictionary,
        english: english_required
            || fs::symlink_metadata(resources.join(assets::ENGLISH_DICTIONARY)).is_ok(),
    };

    fs::create_dir_all(&parent)?;
    // 每次尝试在 `<id>.<随机串>.incoming` 里暂存，目录由 `tempdir_in` 独占新建（同名已存在就换一个名字），两次准备绝不共用一个暂存目录，这是 RP:155-157 独占新建要保证的；名字每次不同，进程在中途被杀留下的暂存目录也就挡不住下一次准备，由 [`sweep_abandoned_stages`] 收拾。出错时 `TempDir` 被丢弃，只删这次自己的目录（RP:174-179）。
    let stage = tempfile::Builder::new()
        .prefix(&format!("{content_id}."))
        .suffix(STAGE_SUFFIX)
        .tempdir_in(&parent)?;
    let staged = (|| -> Result<()> {
        if main_dictionary {
            check_dictionary_format(
                &resources.join(assets::MAIN_DICTIONARY),
                assets::MAIN_DICTIONARY,
            )?;
        }
        for name in dictionaries.names() {
            copy_database(&resources.join(name), &stage.path().join(name))?;
        }
        if main_dictionary {
            merge_split_wubi(resources, &stage.path().join(assets::MAIN_DICTIONARY))?;
        }
        stage_generation_copies(resources, stage.path(), true)?;
        replay_into(&result, stage.path(), dictionaries)?;
        index_reverse_lookup(stage.path(), dictionaries);
        write_private_file(
            &stage.path().join(assets::GENERATION_READY),
            format!("{content_id}\n").as_bytes(),
        )
        .map_err(|_| EngineError::failed(diagnostics::RUNTIME_FINALIZE_FAILED))?;
        // 只把整套做完的目录改名成代次，读的一方要么看不到代次，要么看到带 `.ready` 的完整代次。代次已经存在且非空时改名一定失败（Unix 报 `ENOTEMPTY`/`EEXIST`，Windows 报 `ERROR_DIR_NOT_EMPTY` 或拒绝访问），不会顶掉别人发布的代次。
        fs::rename(stage.path(), &result.dictionaries)?;
        Ok(())
    })();
    match staged {
        Ok(()) => {
            // 目录已经改名成代次，不能再让 `TempDir` 按原路径去删。
            let _ = stage.keep();
            Ok(result)
        }
        Err(error) => {
            drop(stage);
            // 另一个进程同时在准备同一个代次并抢先发布了它（这次的改名因此失败），或者这次停顿太久、暂存目录被当作丢下的清掉了：用已经就绪的那一份，和一开始就看到它一样再回放一遍日志。
            if generation_ready(&marker).unwrap_or(false) {
                return open_ready_generation(result, resources, main_dictionary, english_required);
            }
            Err(error)
        }
    }
}

/// 代次目录里是不是一个普通文件的 `.ready`。目录、设备或符号链接不算完成的代次，也不跟随、不去替换它所在的代次。
fn generation_ready(marker: &Path) -> Result<bool> {
    match fs::symlink_metadata(marker) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        Ok(_) => Err(EngineError::failed(
            diagnostics::INCOMPLETE_RUNTIME_GENERATION,
        )),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

/// 打开已经就绪的代次：放回重置途中被杀时挪开的词库，检查要求的词库副本都在，补上五笔表、反查索引和后来才随资源发布的 n-gram 表，再把当前日志回放一遍。不要求的英文库在不在，以代次准备时有没有复制它为准。
fn open_ready_generation(
    result: RuntimePaths,
    resources: &Path,
    main_dictionary: bool,
    english_required: bool,
) -> Result<RuntimePaths> {
    // 重置途中被杀时词库可能还挪在备份里，先放回来再看在不在，见 `restore_interrupted_reset`。
    if main_dictionary {
        restore_interrupted_reset(&result.dictionary(assets::MAIN_DICTIONARY));
        if !is_real_file(&result.dictionary(assets::MAIN_DICTIONARY)) {
            return Err(EngineError::failed(
                diagnostics::INCOMPLETE_RUNTIME_GENERATION,
            ));
        }
    }
    restore_interrupted_reset(&result.dictionary(assets::ENGLISH_DICTIONARY));
    let english = is_real_file(&result.dictionary(assets::ENGLISH_DICTIONARY));
    if english_required && !english {
        return Err(EngineError::failed(
            diagnostics::INCOMPLETE_RUNTIME_GENERATION,
        ));
    }
    let dictionaries = GenerationDictionaries {
        main: main_dictionary,
        english,
    };
    // 拆分五笔码表之后准备的代次也要有五笔表；表已在时这一步只读不写。没有工作主词库的代次没有地方放五笔表。
    if main_dictionary {
        // 代次里的主库副本带着来源文件的 `metadata`。就绪的代次可能是更新的引擎用更新格式的词库准备的（宿主降级后仍选着那份下载的词库更新），这里不查，会话就会照旧格式去读它。
        check_dictionary_format(
            &result.dictionary(assets::MAIN_DICTIONARY),
            assets::MAIN_DICTIONARY,
        )?;
        merge_split_wubi(resources, &result.dictionary(assets::MAIN_DICTIONARY))?;
    }
    // A host can switch back to a previously prepared generation. Replay the current journal again so changes learned on a newer generation survive that switch (RP:143-144).
    replay_into(&result, &result.dictionaries, dictionaries)?;
    // 旧版本准备的代次没有反查索引，重新打开时补上；已有索引时只读一次 schema。
    index_reverse_lookup(&result.dictionaries, dictionaries);
    stage_generation_copies(resources, &result.dictionaries, false)?;
    Ok(result)
}

/// 清掉 `dictionaries` 下被丢下的暂存目录：准备途中被杀的进程留下的 `<id>.<随机串>.incoming`、旧版本固定名字的 `<id>.incoming`，以及上一次清理没删完的 `.removing`。每次准备都先做一遍，出什么错都不影响准备本身。
///
/// 暂存目录本身和里面每一项最近一次修改都早于 [`ABANDONED_STAGE_AGE`] 才算被丢下。不是自己的暂存目录无从判断主人是否还活着：引擎不能用文件锁（只有 client-base 知道 Android 上怎么锁，引擎不依赖它），进程号在 Linux 的不同 PID 命名空间之间、iOS 沙盒的 App 与扩展之间都不可靠，只剩“多久没写”这一条。被丢下的目录先改名成 `.removing` 认领再删：万一主人只是停顿太久（iOS 把 App 挂起在准备中途），它之后的写入和改名找不到原路径，只会让那次准备失败，不会把删到一半的目录发布成代次。
fn sweep_abandoned_stages(parent: &Path, now: SystemTime) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let path = entry.path();
        if is_stage_name(name, CLAIMED_STAGE_SUFFIX) {
            // 已经认领的目录不会再被谁发布，不必等它变旧。
            if is_real_directory(&path) {
                let _ = fs::remove_dir_all(&path);
            }
            continue;
        }
        if !is_stage_name(name, STAGE_SUFFIX) || !abandoned(&path, now) {
            continue;
        }
        let mut claimed = name[..name.len() - STAGE_SUFFIX.len()].to_owned();
        claimed.push_str(CLAIMED_STAGE_SUFFIX);
        let claimed = parent.join(claimed);
        // Windows 上目录里还有打开的文件时改名失败，跳过即可。
        if fs::rename(&path, &claimed).is_ok() {
            let _ = fs::remove_dir_all(&claimed);
        }
    }
}

/// `<id><suffix>` 或 `<id>.<字母数字><suffix>`，`<id>` 是合法的代次 id。
fn is_stage_name(name: &str, suffix: &str) -> bool {
    let Some(stem) = name.strip_suffix(suffix) else {
        return false;
    };
    let (content_id, attempt) = match stem.split_once('.') {
        Some((content_id, attempt)) => (content_id, Some(attempt)),
        None => (stem, None),
    };
    valid_content_id(content_id)
        && attempt.is_none_or(|attempt| {
            !attempt.is_empty() && attempt.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

/// 暂存目录（不是符号链接）和里面每一项的修改时间都早于 `now - ABANDONED_STAGE_AGE`。读不到时间、时间在将来、或者读的同时有项目消失，都当作还有人在写。
fn abandoned(stage: &Path, now: SystemTime) -> bool {
    let Some(cutoff) = now.checked_sub(ABANDONED_STAGE_AGE) else {
        return false;
    };
    let untouched = |path: &Path| {
        fs::symlink_metadata(path)
            .and_then(|metadata| metadata.modified())
            .is_ok_and(|modified| modified < cutoff)
    };
    if !is_real_directory(stage) || !untouched(stage) {
        return false;
    }
    let Ok(entries) = fs::read_dir(stage) else {
        return false;
    };
    entries
        .into_iter()
        .all(|entry| entry.is_ok_and(|entry| untouched(&entry.path())))
}

fn is_real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
}

fn valid_content_id(content_id: &str) -> bool {
    !content_id.is_empty()
        && content_id.len() <= MAX_CONTENT_ID_LENGTH
        && content_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// 代次里有哪些词库工作副本。有 `msime-pinyin.db` 时一定有 `msime-english.db`（读主库的方案集合也要求英文库）；只有不要求英文库、资源目录又没带它的集合，两者都没有。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GenerationDictionaries {
    main: bool,
    english: bool,
}

impl GenerationDictionaries {
    fn names(self) -> impl Iterator<Item = &'static str> {
        [
            (self.main, assets::MAIN_DICTIONARY),
            (self.english, assets::ENGLISH_DICTIONARY),
        ]
        .into_iter()
        .filter_map(|(present, name)| present.then_some(name))
    }
}

/// Replay the journal of `paths` into the dictionaries in `generation`; any failed row or error refuses the generation. 没有 `msime-pinyin.db` 的代次只回放英文行，两个库都没有的代次什么也不回放。
fn replay_into(
    paths: &RuntimePaths,
    generation: &Path,
    dictionaries: GenerationDictionaries,
) -> Result<()> {
    let journal = paths.user(assets::USER_JOURNAL);
    let english = generation.join(assets::ENGLISH_DICTIONARY);
    let replayed = if dictionaries.main {
        replay(
            &journal,
            &generation.join(assets::MAIN_DICTIONARY),
            &english,
        )
    } else if dictionaries.english {
        replay_english(&journal, &english)
    } else {
        return Ok(());
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

/// 给代次里每个词库副本（没有主库时只有英文词库，或者一个也没有）补上五笔反查索引（见 `wubi::provider::ensure_reverse_indexes`）。代次是用户可写的副本，资源目录保持只读、原样；索引不进日志，也不影响词库状态的摘要。
///
/// 只是提速，从不失败：建索引要写盘（约 1.3 MB 加回滚日志），磁盘满、文件只读或被别的进程长时间锁住时记一条日志、照常用没有索引的副本，反查仍然正确，只是慢。这里报错会让宿主起不来。新词库在 dict-builder 里就带着同名索引，这里只为旧词库和旧代次补建。
fn index_reverse_lookup(generation: &Path, dictionaries: GenerationDictionaries) {
    for name in dictionaries.names() {
        let database = generation.join(name);
        let indexed = open_database(&database, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .and_then(|connection| ensure_reverse_indexes(&connection).map_err(Into::into));
        if let Err(error) = indexed {
            crate::logging::warn(format_args!(
                "wubi reverse lookup index unavailable in {}, lookups stay unindexed: {error}",
                database.display()
            ));
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
        let source = crate::paths::sqlite_read_only_path_no_follow(source)
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

/// 资源目录不带 `msime-wubi.db`、五笔码表来自宿主另行下载的那份 `wubi`（`EngineOptions::wubi_dictionary`，macOS 的 `wubi` 资源包）时，把它并进代次的工作主词库，再回放一遍日志：准备代次时码表还不在，日志里的五笔行被跳过（`replay`），并进来之后才有地方写。返回是否并入了码表。
///
/// 工作主词库已有五笔表、代次就是资源目录（只读布局）、没有工作主词库（不读拼音库的版本），或者 `wubi` 不是普通文件 `msime-wubi.db` 时什么也不做。已有五笔表时只读一次 schema，所以每次建会话都调用也不贵。
pub(crate) fn merge_downloaded_wubi(paths: &RuntimePaths, wubi: &Path) -> Result<bool> {
    let main_db = paths.dictionary(assets::MAIN_DICTIONARY);
    if paths.dictionaries == paths.resources
        || !is_real_file(&main_db)
        || wubi.file_name() != Some(std::ffi::OsStr::new(assets::WUBI_DICTIONARY))
        || !is_real_file(wubi)
    {
        return Ok(false);
    }
    if has_split_wubi_tables(&main_db)? {
        return Ok(false);
    }
    let Some(directory) = wubi.parent() else {
        return Ok(false);
    };
    merge_split_wubi(directory, &main_db)?;
    // 有工作主词库的代次一定也有英文库（读主库的集合要求它）。
    let dictionaries = GenerationDictionaries {
        main: true,
        english: true,
    };
    replay_into(paths, &paths.dictionaries, dictionaries)?;
    index_reverse_lookup(&paths.dictionaries, dictionaries);
    Ok(true)
}

/// 工作主词库里是否已有 `wubi86` 或 `wubi98`。
pub(crate) fn has_split_wubi_tables(main_db: &Path) -> Result<bool> {
    let present = (|| -> rusqlite::Result<bool> {
        let path = crate::paths::sqlite_read_only_path_no_follow(main_db)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        let connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NOFOLLOW
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name IN ('wubi86','wubi98'))",
            [],
            |row| row.get(0),
        )
    })();
    present.map_err(|error| {
        EngineError::failed(format!("cannot read {}: {error}", assets::MAIN_DICTIONARY))
    })
}

/// 词库发布把五笔码表单独放在只读的 `msime-wubi.db` 里，`msime-pinyin.db` 不再含 `wubi86`/`wubi98`。五笔的学习调序、删词、个人词典编辑与日志回放都写代次里的工作主词库，五笔 provider 也从它读，所以准备代次（以及重置学习数据）时把这两张表连同索引并回工作副本：读写落在同一个文件上，学到的权重立即可见。资源目录没有 `msime-wubi.db`（旧的合并发布）或工作副本里已有同名表时不动。
pub(crate) fn merge_split_wubi(resources: &Path, main_db: &Path) -> Result<()> {
    let source = resources.join(assets::WUBI_DICTIONARY);
    if !is_real_file(&source) {
        return Ok(());
    }
    check_dictionary_format(&source, assets::WUBI_DICTIONARY)?;
    let merged = (|| -> rusqlite::Result<()> {
        let source = crate::paths::sqlite_read_only_path_no_follow(&source)
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

/// 资源目录里的主库或五笔库在复制、合并进代次之前先查文件内版本（[`crate::format::check_format_version`]）：比这个引擎新的格式拒绝，旧格式和没有 `metadata` 表的旧发布照常使用。`name` 只用于错误信息。文件不在、打不开或不是数据库时这里不报错，复制与合并照原来的方式报告它。
pub(crate) fn check_dictionary_format(path: &Path, name: &str) -> Result<()> {
    let Ok(resolved) = crate::paths::sqlite_read_only_path_no_follow(path) else {
        return Ok(());
    };
    let Ok(connection) = Connection::open_with_flags(
        &resolved,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NOFOLLOW
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return Ok(());
    };
    // 与 `copy_database` 一样不等锁：资源文件没有写者，读不了就交给后面的步骤报告。
    if connection.busy_timeout(Duration::ZERO).is_err() {
        return Ok(());
    }
    match crate::format::check_format_version(&connection, name) {
        Ok(_) | Err(EngineError::Sqlite(_)) => Ok(()),
        Err(error) => Err(error),
    }
}

/// 只读布局（代次目录就是资源目录，网页引擎、后端和部分测试这样用）没有准备代次这一步，建会话时在这里查主库和五笔库的文件内版本，规则与准备代次时相同。其他布局什么也不做：它们的文件在准备代次时已经查过。
pub(crate) fn check_read_only_layout(paths: &RuntimePaths) -> Result<()> {
    if paths.dictionaries != paths.resources {
        return Ok(());
    }
    for name in [assets::MAIN_DICTIONARY, assets::WUBI_DICTIONARY] {
        check_dictionary_format(&paths.dictionary(name), name)?;
    }
    Ok(())
}

/// 把资源带的可选 n-gram 表复制进代次。已就绪的代次也会以 `replace_existing` 为假调用它：后来的版本才开始发布的表不用换代次就能补上，会话可能已经映射着的那张保持不动（RP:76-90）。
///
/// 每张表先写进这次尝试独有的 `<表名>.<随机串>.incoming`（独占新建，重名就换一个），落盘后再改名就位：会话可能映射着旧文件，映射下面的表写到一半就是崩溃而不是查不到（RP:62-63）。以前用固定的 `<表名>.incoming`，两个进程同时补同一张表时，一方会删掉另一方正在写的文件，再把对方写到一半的副本改名就位。几方同时补同一张表时，后改名的一方在 Unix 上以同样的字节替换前一方的文件（映射着前一份的会话不受影响）；Windows 上目标被映射时改名失败，这时目标已经是完整的表，丢掉自己的副本即可。
pub(super) fn stage_generation_copies(
    resources: &Path,
    generation: &Path,
    replace_existing: bool,
) -> Result<()> {
    sweep_abandoned_copies(generation, SystemTime::now());
    for name in GENERATION_COPIES {
        let source = resources.join(name);
        if !is_real_file(&source) {
            continue;
        }
        let target = generation.join(name);
        if !replace_existing && is_real_file(&target) {
            continue;
        }
        let mut input = crate::paths::open_file_no_follow(&source)?;
        // 出错时丢弃 `TempPath` 会删掉这次自己的临时文件。写入句柄在块结束时关闭，Windows 不能改名打开着的文件。
        let incoming = {
            let (mut output, incoming) = tempfile::Builder::new()
                .prefix(&format!("{name}."))
                .suffix(STAGE_SUFFIX)
                .make_in(generation, create_private_file)?
                .into_parts();
            io::copy(&mut input, &mut output)?;
            output.sync_all()?;
            incoming
        };
        match fs::rename(&incoming, &target) {
            Ok(()) => {
                // 文件已经改名就位，不能再让 `TempPath` 按原路径去删。
                let _ = incoming.keep();
            }
            Err(_) if !replace_existing && is_real_file(&target) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// 删掉代次目录里一小时没被写过的 n-gram 临时副本：准备途中被杀的进程留下的 `<表名>.<随机串>.incoming`，以及旧版本固定名字的 `<表名>.incoming`。判据与 [`sweep_abandoned_stages`] 相同；文件按路径删除，主人若其实还活着，之后的改名找不到原路径，只会让那次准备失败，已就位的表不受影响。出什么错都不影响准备本身。
fn sweep_abandoned_copies(generation: &Path, now: SystemTime) {
    let Some(cutoff) = now.checked_sub(ABANDONED_STAGE_AGE) else {
        return;
    };
    let Ok(entries) = fs::read_dir(generation) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_str().is_some_and(is_copy_name) {
            continue;
        }
        let path = entry.path();
        let stale = fs::symlink_metadata(&path).is_ok_and(|metadata| {
            metadata.file_type().is_file()
                && metadata.modified().is_ok_and(|modified| modified < cutoff)
        });
        if stale {
            let _ = fs::remove_file(&path);
        }
    }
}

/// `<表名>.incoming` 或 `<表名>.<字母数字>.incoming`，表名是 [`GENERATION_COPIES`] 之一。
fn is_copy_name(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(STAGE_SUFFIX) else {
        return false;
    };
    GENERATION_COPIES.iter().any(|copy| {
        stem == *copy
            || stem
                .strip_prefix(copy)
                .and_then(|rest| rest.strip_prefix('.'))
                .is_some_and(|attempt| {
                    !attempt.is_empty() && attempt.bytes().all(|byte| byte.is_ascii_alphanumeric())
                })
    })
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

    /// `user/dictionaries` 下还剩的暂存目录。
    fn stages(user: &Path) -> Vec<String> {
        fs::read_dir(user.join("dictionaries"))
            .map(|entries| {
                entries
                    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                    .filter(|name| name.ends_with(STAGE_SUFFIX))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 目录下名字以 `.incoming` 结尾的项。
    fn incoming_entries(directory: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(STAGE_SUFFIX))
            .collect();
        names.sort();
        names
    }

    fn learn(journal: &Path, word: &str, updated_at: i64) {
        sql(journal, OPERATIONS_DDL);
        sql(
            journal,
            &format!("INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,user_inserted,updated_at) VALUES('pinyin','ni','{word}','upsert',50,1,{updated_at})"),
        );
    }

    #[test]
    fn a_journal_that_is_not_a_database_does_not_stop_preparation() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let user = root.path().join("user");
        fs::create_dir_all(&user).unwrap();
        let journal = user.join(assets::USER_JOURNAL);
        fs::write(&journal, vec![0u8; 8 * 4096]).unwrap();

        let prepared =
            prepare_runtime_paths(&resources, &user, &root.path().join("cache"), "v1").unwrap();
        assert!(prepared
            .dictionaries
            .join(assets::GENERATION_READY)
            .exists());
        assert!(!journal.exists());
        let moved: Vec<_> = fs::read_dir(&user)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("msime_user.db.corrupt-"))
            .collect();
        assert_eq!(moved.len(), 1, "{moved:?}");
        // 之后写入的日志仍会按原流程回放到代次中。
        learn(&journal, "你", 1);
        prepare_runtime_paths(&resources, &user, &root.path().join("cache"), "v1").unwrap();
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
        assert!(stages(&user).is_empty());
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
        assert!(stages(&user).is_empty());
        // A ready generation refuses the same journal as well.
        assert!(prepare_runtime_paths(&resources, &user, &cache, "v2").is_err());
    }

    #[test]
    fn staging_failures_clean_up_and_incomplete_generations_are_refused() {
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
        assert_eq!(fs::read_dir(user.join("dictionaries")).unwrap().count(), 0);
        assert!(!missing.join(assets::MAIN_DICTIONARY).exists());

        let incomplete = user.join("dictionaries/v2");
        fs::create_dir_all(&incomplete).unwrap();
        fs::write(incomplete.join(assets::GENERATION_READY), "v2\n").unwrap();
        let error = prepare_runtime_paths(&resources, &user, &cache, "v2").unwrap_err();
        assert_eq!(
            error.to_string(),
            diagnostics::INCOMPLETE_RUNTIME_GENERATION
        );
    }

    /// 进程在准备途中被杀（iOS 键盘扩展超出内存上限、Android 与 HarmonyOS 退到后台被系统回收）会留下复制到一半的暂存目录。下一次准备照常成功，不再每次都报暂存目录已存在；刚留下的暂存目录分不清是不是另一个进程正在写的，原样留着。
    #[test]
    fn a_stage_left_by_a_killed_preparation_does_not_block_the_next_one() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        let parent = user.join("dictionaries");
        // 旧版本用的固定暂存名，和现在每次尝试各自的暂存名。
        let leftovers = ["v1.incoming", "v1.k3Xq9z.incoming"];
        for leftover in leftovers {
            fs::create_dir_all(parent.join(leftover)).unwrap();
            fs::write(
                parent.join(leftover).join(assets::MAIN_DICTIONARY),
                b"SQLite format 3\0partial",
            )
            .unwrap();
        }
        learn(&user.join(assets::USER_JOURNAL), "伱", 1);

        let paths = prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        assert_eq!(paths.dictionaries, parent.join("v1"));
        assert_eq!(
            fs::read_to_string(paths.dictionary(assets::GENERATION_READY)).unwrap(),
            "v1\n"
        );
        assert_eq!(
            weight(
                &paths.dictionary(assets::MAIN_DICTIONARY),
                "SELECT weight FROM tbl_1_n WHERE value='伱'"
            ),
            Some(50)
        );
        for leftover in leftovers {
            assert_eq!(
                fs::read(parent.join(leftover).join(assets::MAIN_DICTIONARY)).unwrap(),
                b"SQLite format 3\0partial",
                "{leftover}"
            );
        }
        assert_eq!(
            prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap(),
            paths
        );
    }

    /// 重置学习数据在把工作主词库挪成 `.msime-pinyin.db.backup.<时间戳>` 之后、新副本改名就位之前被杀：下一次准备把时间戳最新的那份备份放回去、删掉同一时间戳没发布的新副本，学到的词仍在；更旧的备份、以及词库在时出现的备份都不动。
    #[test]
    fn a_dictionary_left_aside_by_a_killed_reset_is_put_back() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        fs::create_dir_all(&user).unwrap();
        learn(&user.join(assets::USER_JOURNAL), "伱", 1);
        let paths = prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        let main = paths.dictionary(assets::MAIN_DICTIONARY);
        let sibling = |name: &str| paths.dictionaries.join(name);
        fs::rename(
            &main,
            sibling(".msime-pinyin.db.backup.1700000000000000000"),
        )
        .unwrap();
        fs::write(
            sibling(".msime-pinyin.db.reset.1700000000000000000"),
            b"fresh copy",
        )
        .unwrap();
        fs::write(sibling(".msime-pinyin.db.backup.999"), b"older").unwrap();

        assert_eq!(
            prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap(),
            paths
        );
        assert_eq!(
            weight(&main, "SELECT weight FROM tbl_1_n WHERE value='伱'"),
            Some(50)
        );
        assert!(!sibling(".msime-pinyin.db.backup.1700000000000000000").exists());
        assert!(!sibling(".msime-pinyin.db.reset.1700000000000000000").exists());
        assert_eq!(
            fs::read(sibling(".msime-pinyin.db.backup.999")).unwrap(),
            b"older"
        );

        let restored = fs::read(&main).unwrap();
        fs::write(
            sibling(".msime-pinyin.db.backup.1800000000000000000"),
            b"newer",
        )
        .unwrap();
        prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        assert_eq!(fs::read(&main).unwrap(), restored);
        assert_eq!(
            fs::read(sibling(".msime-pinyin.db.backup.1800000000000000000")).unwrap(),
            b"newer"
        );
    }

    fn assert_a_legacy_dictionary_left_aside_is_put_back(name: &str, legacy: &str) {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        fs::create_dir_all(&user).unwrap();
        let journal = user.join(assets::USER_JOURNAL);
        learn(&journal, "伱", 1);
        sql(
            &journal,
            "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted,updated_at) VALUES('english','zzfixture','Zzfixture','upsert',11,'Zzfixture',1,2)",
        );
        let paths = prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        let current = paths.dictionaries.join(name);
        let old = paths.dictionaries.join(legacy);
        fs::rename(&current, &old).unwrap();
        assert_eq!(paths.dictionary(name), old);
        let backup = paths.dictionaries.join(format!(".{legacy}.backup.200"));
        let staged = paths.dictionaries.join(format!(".{legacy}.reset.200"));
        fs::rename(&old, &backup).unwrap();
        fs::write(&staged, b"unpublished synthetic dictionary").unwrap();

        assert_eq!(
            prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap(),
            paths
        );
        assert_eq!(paths.dictionary(name), current);
        assert!(!old.exists());
        assert!(!backup.exists());
        assert!(!staged.exists());
        assert_eq!(
            weight(
                &paths.dictionary(assets::MAIN_DICTIONARY),
                "SELECT weight FROM tbl_1_n WHERE value='伱'"
            ),
            Some(50)
        );
        assert_eq!(
            weight(
                &paths.dictionary(assets::ENGLISH_DICTIONARY),
                "SELECT weight FROM english_words WHERE word='zzfixture' AND display='Zzfixture'"
            ),
            Some(11)
        );
    }

    /// 旧主词库被挪成备份后，路径解析已看不到旧名，准备仍应找到它。
    #[test]
    fn a_legacy_main_dictionary_left_aside_by_a_reset_is_put_back() {
        assert_a_legacy_dictionary_left_aside_is_put_back(assets::MAIN_DICTIONARY, "msime.db");
    }

    /// 英文词库也从旧名备份恢复到现名，继续使用同一套回放路径。
    #[test]
    fn a_legacy_english_dictionary_left_aside_by_a_reset_is_put_back() {
        assert_a_legacy_dictionary_left_aside_is_put_back(assets::ENGLISH_DICTIONARY, "english.db");
    }

    /// 重置在把日志挪成 `.msime_user.db.backup.<时间戳>` 之后被杀：日志放回去，学到的词不会因为下一次准备新建了空日志而丢掉。
    #[test]
    fn a_journal_left_aside_by_a_killed_reset_is_put_back() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        fs::create_dir_all(&user).unwrap();
        let journal = user.join(assets::USER_JOURNAL);
        learn(&journal, "伱", 1);
        prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        fs::rename(
            &journal,
            user.join(".msime_user.db.backup.1700000000000000000"),
        )
        .unwrap();
        fs::write(
            user.join(".msime_user.db.reset.1700000000000000000"),
            b"empty journal",
        )
        .unwrap();

        let paths = prepare_runtime_paths(&resources, &user, &cache, "v2").unwrap();
        assert_eq!(
            weight(&journal, "SELECT count(*) FROM user_dictionary_operations"),
            Some(1)
        );
        assert_eq!(
            weight(
                &paths.dictionary(assets::MAIN_DICTIONARY),
                "SELECT weight FROM tbl_1_n WHERE value='伱'"
            ),
            Some(50)
        );
        assert!(!user
            .join(".msime_user.db.backup.1700000000000000000")
            .exists());
        assert!(!user
            .join(".msime_user.db.reset.1700000000000000000")
            .exists());
    }

    /// 同一个代次被几方同时准备（iOS 的 App 与键盘扩展、Windows 的 TIP 与 Server、IBus 与 Fcitx5）：每一方都拿到同一个可用的代次，谁也不报错，输掉的一方不留下暂存目录，也不动赢的一方发布的代次。
    #[test]
    fn concurrent_preparations_of_one_generation_all_succeed() {
        const PREPARERS: usize = 8;
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        fs::write(resources.join(assets::BIGRAM_TABLE), "bigram").unwrap();
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        fs::create_dir_all(&user).unwrap();
        learn(&user.join(assets::USER_JOURNAL), "伱", 1);

        let barrier = std::sync::Barrier::new(PREPARERS);
        let results: Vec<_> = std::thread::scope(|scope| {
            let preparers: Vec<_> = (0..PREPARERS)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        prepare_runtime_paths(&resources, &user, &cache, "v1")
                            .map_err(|error| error.to_string())
                    })
                })
                .collect();
            preparers
                .into_iter()
                .map(|preparer| preparer.join().unwrap())
                .collect()
        });
        let generation = user.join("dictionaries/v1");
        for result in &results {
            assert_eq!(result.as_ref().unwrap().dictionaries, generation);
        }
        assert_eq!(
            fs::read_to_string(generation.join(assets::GENERATION_READY)).unwrap(),
            "v1\n"
        );
        assert_eq!(
            weight(
                &generation.join(assets::MAIN_DICTIONARY),
                "SELECT weight FROM tbl_1_n WHERE value='伱'"
            ),
            Some(50)
        );
        assert_eq!(
            fs::read(generation.join(assets::BIGRAM_TABLE)).unwrap(),
            b"bigram"
        );
        let entries: Vec<_> = fs::read_dir(user.join("dictionaries"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(entries, ["v1"]);
    }

    /// 清理只删一小时没被写过的暂存目录和已经认领的 `.removing`；代次、名字对不上的目录、普通文件和符号链接都不动。
    #[test]
    fn only_stages_untouched_for_an_hour_are_swept() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("dictionaries");
        let directories = [
            "v1.incoming",
            "v1.k3Xq9z.incoming",
            "v2.Ab12cd.removing",
            "v1",
            "a b.incoming",
            "v1.k3.x.incoming",
            "v1..incoming",
            "v1.incoming.bak",
        ];
        for name in directories {
            fs::create_dir_all(parent.join(name)).unwrap();
            fs::write(parent.join(name).join(assets::MAIN_DICTIONARY), b"partial").unwrap();
        }
        fs::write(parent.join("v3.incoming"), b"not a directory").unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("keep"), b"keep").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), parent.join("v4.incoming")).unwrap();
        let names = || {
            let mut names: Vec<_> = fs::read_dir(&parent)
                .unwrap()
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        };
        let mut untouched: Vec<String> = [
            "a b.incoming",
            "v1",
            "v1..incoming",
            "v1.incoming.bak",
            "v1.k3.x.incoming",
            "v3.incoming",
        ]
        .map(String::from)
        .to_vec();
        if cfg!(unix) {
            untouched.push("v4.incoming".into());
        }
        untouched.sort();

        // 刚写过的暂存目录可能还有人在写，留着；认领过的不会再被发布，直接删。
        sweep_abandoned_stages(&parent, SystemTime::now());
        let mut fresh = untouched.clone();
        fresh.extend(["v1.incoming".into(), "v1.k3Xq9z.incoming".into()]);
        fresh.sort();
        assert_eq!(names(), fresh);

        sweep_abandoned_stages(
            &parent,
            SystemTime::now() + ABANDONED_STAGE_AGE + Duration::from_secs(60),
        );
        assert_eq!(names(), untouched);
        assert_eq!(fs::read(outside.path().join("keep")).unwrap(), b"keep");
    }

    /// 一小时前被丢下的暂存目录和 n-gram 临时副本在下一次准备时删掉，哪怕那次准备走的是已就绪代次的路径。
    #[cfg(unix)]
    #[test]
    fn preparation_sweeps_stages_and_table_copies_abandoned_an_hour_ago() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        let parent = user.join("dictionaries");
        let abandoned = parent.join("v1.k3Xq9z.incoming");
        fs::create_dir_all(&abandoned).unwrap();
        let partial = abandoned.join(assets::MAIN_DICTIONARY);
        fs::write(&partial, b"partial").unwrap();
        let long_ago = SystemTime::now() - ABANDONED_STAGE_AGE - Duration::from_secs(60);
        let backdate = |path: &Path| {
            fs::File::open(path)
                .unwrap()
                .set_modified(long_ago)
                .unwrap()
        };
        // 目录本身一小时没变、里面的文件却刚写过（正在复制的大词库就是这样），仍算有人在写。
        backdate(&abandoned);
        prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        assert_eq!(fs::read(&partial).unwrap(), b"partial");

        // 代次里补 n-gram 表留下的临时副本同理：刚写过的留着，一小时没写过的删掉。
        let copy = parent.join("v1").join("msime-trigram.bin.k3Xq9z.incoming");
        fs::write(&copy, b"partial").unwrap();
        prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        assert!(copy.is_file());

        backdate(&partial);
        backdate(&abandoned);
        backdate(&copy);
        prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        assert!(!abandoned.exists());
        assert!(!copy.exists());
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 1);
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
        for name in [assets::MAIN_DICTIONARY, assets::ENGLISH_DICTIONARY] {
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
        assert!(incoming_entries(&paths.dictionaries).is_empty());
        let fresh = prepare_runtime_paths(&resources, &user, &cache, "v2").unwrap();
        assert_eq!(
            fs::read(fresh.dictionary(assets::BIGRAM_TABLE)).unwrap(),
            b"second"
        );
    }

    /// 补 n-gram 表时被杀留下的临时副本（旧版本的固定名字和现在的名字）不挡住下一次补表，刚写过的原样留着；一小时没被写过的在之后的清理里删掉，代次里别的文件不动。
    #[test]
    fn table_copies_left_by_a_killed_preparation_do_not_block_and_are_swept_later() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        let generation = prepare_runtime_paths(&resources, &user, &cache, "v1")
            .unwrap()
            .dictionaries;
        let leftovers = [
            "msime-trigram.bin.incoming",
            "msime-trigram.bin.k3Xq9z.incoming",
        ];
        for leftover in leftovers {
            fs::write(generation.join(leftover), b"partial").unwrap();
        }
        let unrelated = [
            "other.incoming",
            "msime-trigram.bin.k3.x.incoming",
            "msime-trigram.bin..incoming",
            "msime-trigram.bin.incoming.bak",
        ];
        for name in unrelated {
            fs::write(generation.join(name), b"keep").unwrap();
        }
        fs::create_dir(generation.join("msime-bigram.bin.Ab12cd.incoming")).unwrap();
        fs::write(resources.join(assets::TRIGRAM_TABLE), "third").unwrap();

        prepare_runtime_paths(&resources, &user, &cache, "v1").unwrap();
        assert_eq!(
            fs::read(generation.join(assets::TRIGRAM_TABLE)).unwrap(),
            b"third"
        );
        for leftover in leftovers {
            assert_eq!(fs::read(generation.join(leftover)).unwrap(), b"partial");
        }

        sweep_abandoned_copies(
            &generation,
            SystemTime::now() + ABANDONED_STAGE_AGE + Duration::from_secs(60),
        );
        for leftover in leftovers {
            assert!(!generation.join(leftover).exists(), "{leftover}");
        }
        for name in unrelated {
            assert_eq!(fs::read(generation.join(name)).unwrap(), b"keep", "{name}");
        }
        assert!(generation.join("msime-bigram.bin.Ab12cd.incoming").is_dir());
        for name in [
            assets::MAIN_DICTIONARY,
            assets::ENGLISH_DICTIONARY,
            assets::GENERATION_READY,
            assets::TRIGRAM_TABLE,
        ] {
            assert!(generation.join(name).is_file(), "{name}");
        }
    }

    /// 后来的版本开始随资源发布一张 n-gram 表，几个进程同时打开同一个已就绪的代次、同时补这张表：每一方都成功，就位的表与资源逐字节一致，谁也不会把别人写到一半的副本改名就位，也不留下临时文件。
    #[test]
    fn concurrent_preparations_add_a_newly_shipped_table_once() {
        const PREPARERS: usize = 8;
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        let generation = prepare_runtime_paths(&resources, &user, &cache, "v1")
            .unwrap()
            .dictionaries;
        // 大到复制要花一点时间，几方的复制才会互相重叠。
        let table: Vec<u8> = (0..8 * 1024 * 1024u32).map(|index| index as u8).collect();
        fs::write(resources.join(assets::TRIGRAM_TABLE), &table).unwrap();

        let barrier = std::sync::Barrier::new(PREPARERS);
        let results: Vec<_> = std::thread::scope(|scope| {
            let preparers: Vec<_> = (0..PREPARERS)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        prepare_runtime_paths(&resources, &user, &cache, "v1")
                            .map_err(|error| error.to_string())
                    })
                })
                .collect();
            preparers
                .into_iter()
                .map(|preparer| preparer.join().unwrap())
                .collect()
        });
        for result in &results {
            assert_eq!(result.as_ref().unwrap().dictionaries, generation);
        }
        assert!(fs::read(generation.join(assets::TRIGRAM_TABLE)).unwrap() == table);
        assert!(incoming_entries(&generation).is_empty());
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

    /// 资源目录不带 `msime-wubi.db`（macOS 发布包改为按需下载）：准备代次时五笔行跳过、代次照常就绪；宿主交来下载的码表后，建会话时并进工作主词库并补上日志里的五笔行，再来一次什么也不做。
    #[test]
    fn a_downloaded_wubi_dictionary_is_merged_into_a_generation_prepared_without_one() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        assert!(!resources.join(assets::WUBI_DICTIONARY).exists());
        let user = root.path().join("user");
        fs::create_dir_all(&user).unwrap();
        let journal = user.join(assets::USER_JOURNAL);
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
            weight(
                &main,
                "SELECT count(*) FROM sqlite_master WHERE name IN ('wubi86','wubi98')"
            ),
            Some(0)
        );

        let pack = root.path().join("resource-packs").join("wubi");
        fs::create_dir_all(&pack).unwrap();
        let downloaded = pack.join(assets::WUBI_DICTIONARY);
        sql(
            &downloaded,
            "CREATE TABLE wubi86(\"key\" TEXT NOT NULL,\"value\" TEXT NOT NULL,\"weight\" INTEGER NOT NULL DEFAULT 0,UNIQUE(\"key\",\"value\"));
             CREATE TABLE wubi98(\"key\" TEXT NOT NULL,\"value\" TEXT NOT NULL,\"weight\" INTEGER NOT NULL DEFAULT 0,UNIQUE(\"key\",\"value\"));
             INSERT INTO wubi86 VALUES('aaaa','工',90);
             INSERT INTO wubi98 VALUES('wqvb','你好',80);",
        );
        // 不是 `msime-wubi.db` 的文件不认。
        let renamed = pack.join("other.db");
        fs::copy(&downloaded, &renamed).unwrap();
        assert!(!merge_downloaded_wubi(&paths, &renamed).unwrap());
        assert!(!merge_downloaded_wubi(
            &paths,
            &pack.join("missing").join(assets::WUBI_DICTIONARY)
        )
        .unwrap());

        assert!(merge_downloaded_wubi(&paths, &downloaded).unwrap());
        assert_eq!(
            weight(&main, "SELECT weight FROM wubi86 WHERE value='合成五笔'"),
            Some(9)
        );
        assert_eq!(
            weight(&main, "SELECT weight FROM wubi98 WHERE value='你好'"),
            Some(99)
        );
        assert_eq!(
            weight(&main, "SELECT weight FROM wubi86 WHERE value='工'"),
            Some(90)
        );
        // 下载的码表不变；已有五笔表时不再导入。
        assert_eq!(weight(&downloaded, "SELECT count(*) FROM wubi86"), Some(1));
        assert!(!merge_downloaded_wubi(&paths, &downloaded).unwrap());
        assert_eq!(weight(&main, "SELECT count(*) FROM wubi86"), Some(2));
        // 代次就是资源目录（只读布局）时不写。
        let read_only = RuntimePaths {
            dictionaries: paths.resources.clone(),
            ..paths.clone()
        };
        assert!(!merge_downloaded_wubi(&read_only, &downloaded).unwrap());
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

    /// 不要求 `msime-english.db` 的集合（不含全拼、双拼、五笔）在资源目录没带它时，代次里两个库都没有，日志一行也不回放、原样留着；就绪的代次再准备也不要求它，后来才放进资源目录的英文库不改变这个代次。中文集合缺了英文库仍然失败，不留下暂存目录。
    #[test]
    fn english_follows_the_scheme_set() {
        let root = tempfile::tempdir().unwrap();
        let empty = root.path().join("empty-resources");
        fs::create_dir_all(&empty).unwrap();
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
            prepare_runtime_paths_for(&empty, &user, &cache, "v1", japanese_only()).unwrap();
        assert!(paths.dictionaries.join(assets::GENERATION_READY).is_file());
        assert!(!paths.dictionary(assets::MAIN_DICTIONARY).exists());
        assert!(!paths.dictionary(assets::ENGLISH_DICTIONARY).exists());
        assert_eq!(
            weight(
                &journal,
                "SELECT weight FROM user_dictionary_operations WHERE key='zzfixture'"
            ),
            Some(11)
        );
        let full = resources(root.path());
        fs::copy(
            full.join(assets::ENGLISH_DICTIONARY),
            empty.join(assets::ENGLISH_DICTIONARY),
        )
        .unwrap();
        let again =
            prepare_runtime_paths_for(&empty, &user, &cache, "v1", japanese_only()).unwrap();
        assert_eq!(again, paths);
        assert!(!again.dictionary(assets::ENGLISH_DICTIONARY).exists());
        // 换一个代次 id 才会把新放进来的英文库复制进去，日志里的英文词也回放进去。
        let shipped =
            prepare_runtime_paths_for(&empty, &user, &cache, "v2", japanese_only()).unwrap();
        assert_eq!(
            weight(
                &shipped.dictionary(assets::ENGLISH_DICTIONARY),
                "SELECT weight FROM english_words WHERE word='zzfixture' AND display='Zzfixture'"
            ),
            Some(11)
        );

        let chinese = root.path().join("chinese-resources");
        fs::create_dir_all(&chinese).unwrap();
        fs::copy(
            full.join(assets::MAIN_DICTIONARY),
            chinese.join(assets::MAIN_DICTIONARY),
        )
        .unwrap();
        let error = prepare_runtime_paths(&chinese, &user, &cache, "v3").unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "{}{}",
                diagnostics::RUNTIME_COPY_FAILED,
                chinese.join(assets::ENGLISH_DICTIONARY).display()
            )
        );
        assert!(stages(&user).is_empty());
        assert!(!user.join("dictionaries/v3").exists());
        // 读主库的集合也不接受一个没有英文库的现成代次。
        let error = prepare_runtime_paths(&empty, &user, &cache, "v1").unwrap_err();
        assert_eq!(
            error.to_string(),
            diagnostics::INCOMPLETE_RUNTIME_GENERATION
        );
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

    fn stamp(path: &Path, version: i32) {
        sql(
            path,
            &format!(
                "{} INSERT OR REPLACE INTO metadata VALUES('format_version', '{version}');",
                crate::format::METADATA_SCHEMA
            ),
        );
    }

    fn assert_unsupported(error: &EngineError, file: &str) {
        let text = error.to_string();
        assert!(
            text.starts_with(diagnostics::DICTIONARY_FORMAT_UNSUPPORTED),
            "{text}"
        );
        assert!(text.contains(file), "{text}");
    }

    /// 带文件内版本的主库照常准备，`metadata` 随备份一起进代次；比引擎新的主库拒绝准备，不留下代次也不留下暂存目录。没有 `metadata` 的旧发布由其余测试覆盖。
    #[test]
    fn preparation_checks_the_main_dictionary_format_version() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        let main = resources.join(assets::MAIN_DICTIONARY);

        stamp(&main, crate::format::FORMAT_VERSION);
        let prepared = prepare_runtime_paths(&resources, &user, &cache, "current").unwrap();
        let copy = Connection::open(prepared.dictionary(assets::MAIN_DICTIONARY)).unwrap();
        assert_eq!(
            crate::format::check_format_version(&copy, assets::MAIN_DICTIONARY).unwrap(),
            crate::format::FORMAT_VERSION
        );

        stamp(&main, crate::format::FORMAT_VERSION + 1);
        let error = prepare_runtime_paths(&resources, &user, &cache, "newer").unwrap_err();
        assert_unsupported(&error, assets::MAIN_DICTIONARY);
        assert!(!user.join("dictionaries").join("newer").exists());
        assert!(stages(&user).is_empty());
    }

    /// 已经就绪的代次再次打开时也查主库副本：更新的引擎用更新格式准备好的代次，降级后的引擎不能照旧格式去读。
    #[test]
    fn reopening_a_ready_generation_checks_the_main_dictionary_format_version() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        let prepared = prepare_runtime_paths(&resources, &user, &cache, "current").unwrap();
        let copy = prepared.dictionary(assets::MAIN_DICTIONARY);
        assert!(prepared
            .dictionaries
            .join(assets::GENERATION_READY)
            .is_file());

        stamp(&copy, crate::format::FORMAT_VERSION);
        prepare_runtime_paths(&resources, &user, &cache, "current").unwrap();

        stamp(&copy, crate::format::FORMAT_VERSION + 1);
        let error = prepare_runtime_paths(&resources, &user, &cache, "current").unwrap_err();
        assert_unsupported(&error, assets::MAIN_DICTIONARY);
    }

    /// 单独发布的五笔库同样检查：比引擎新的 `msime-wubi.db` 不会并进代次。
    #[test]
    fn preparation_checks_the_wubi_dictionary_format_version() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let user = root.path().join("user");
        let cache = root.path().join("cache");
        let wubi = resources.join(assets::WUBI_DICTIONARY);
        sql(
            &wubi,
            "CREATE TABLE wubi86(key TEXT NOT NULL,value TEXT NOT NULL,weight INTEGER NOT NULL DEFAULT 0,UNIQUE(key,value));
             INSERT INTO wubi86 VALUES('wq','你',10);",
        );
        stamp(&wubi, crate::format::FORMAT_VERSION + 1);
        let error = prepare_runtime_paths(&resources, &user, &cache, "newer").unwrap_err();
        assert_unsupported(&error, assets::WUBI_DICTIONARY);
        assert!(!user.join("dictionaries").join("newer").exists());

        stamp(&wubi, crate::format::FORMAT_VERSION);
        let prepared = prepare_runtime_paths(&resources, &user, &cache, "current").unwrap();
        assert_eq!(
            weight(
                &prepared.dictionary(assets::MAIN_DICTIONARY),
                "SELECT weight FROM wubi86 WHERE value='你'"
            ),
            Some(10)
        );
    }

    /// 只读布局在建会话时检查；其他布局不在这里查。
    #[test]
    fn the_read_only_layout_checks_the_format_version() {
        let root = tempfile::tempdir().unwrap();
        let resources = resources(root.path());
        let paths = RuntimePaths {
            resources: resources.clone(),
            user_data: root.path().join("user"),
            cache: root.path().join("cache"),
            dictionaries: resources.clone(),
        };
        check_read_only_layout(&paths).unwrap();
        stamp(
            &resources.join(assets::MAIN_DICTIONARY),
            crate::format::FORMAT_VERSION + 1,
        );
        let error = check_read_only_layout(&paths).unwrap_err();
        assert_unsupported(&error, assets::MAIN_DICTIONARY);
        let error = crate::Session::new(crate::SessionOptions::new(paths.clone()))
            .err()
            .expect("a newer dictionary format refuses the session");
        assert_unsupported(&error, assets::MAIN_DICTIONARY);
        let generation = RuntimePaths {
            dictionaries: root.path().join("user/dictionaries/v1"),
            ..paths
        };
        check_read_only_layout(&generation).unwrap();
    }
}
