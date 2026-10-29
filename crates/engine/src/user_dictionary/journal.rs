//! Journal connections, schema and the row primitives every writer builds on (user-dictionary.md §2-§4).
//!
//! Connections are kept per thread and per path, because `apply_fixed_positions` reads the journal while every candidate list is built and an open plus schema pass per keystroke dominated that build in the reference (J:402-412). A connection has one transaction state, so it is never shared across threads. A thread keeps one journal open at a time: switching to another path releases the previous connection first, so a thread never holds two journal files open (J:459-461).
//!
//! While no operation of its thread uses it, a thread's connection is parked in a list every thread can reach, so `close_cached_journals` (before a directory is replaced) and the end of the last session on a journal close it from whichever thread runs them, not only the caller's own. A connection an operation still holds closes when that operation ends.

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};

use crate::error::Result;
use crate::format::build_table_name;
use crate::pinyin::segment::split_segments;
use crate::types::PersonalDictionaryKind;

pub const BUSY_TIMEOUT_MS: u64 = 5_000;
pub const SCHEMA_VERSION: i32 = 4;

/// A pinyin key with an empty syllable, or a kind that has no learning table. Internal: callers turn every learning failure into their own diagnostic.
pub(crate) const UNSTORABLE_ENTRY: &str = "The entry has no table in the user dictionary";
/// A write that must change an existing dictionary row found none.
pub(crate) const MISSING_ROW: &str = "The dictionary has no such row";

/// Every table of the v4 journal (J:114-154, NS:156-167), verbatim. Existing users are at v3 with only the first four, so all of it runs on every open.
const SCHEMA_SQL: &str = "CREATE TABLE IF NOT EXISTS user_dictionary_operations(\
dictionary TEXT NOT NULL,\
key TEXT NOT NULL,\
value TEXT NOT NULL,\
operation TEXT NOT NULL CHECK(operation IN ('upsert','delete')),\
weight INTEGER NOT NULL DEFAULT 0,\
display TEXT NOT NULL DEFAULT '',\
user_inserted INTEGER NOT NULL DEFAULT 0,\
updated_at INTEGER NOT NULL DEFAULT(unixepoch()),\
PRIMARY KEY(dictionary,key,value));\
CREATE TABLE IF NOT EXISTS personal_dictionary_receipts( request_id TEXT PRIMARY KEY, payload TEXT NOT NULL);\
CREATE TABLE IF NOT EXISTS candidate_selection_state(\
context_key TEXT NOT NULL,entry_key TEXT NOT NULL,value TEXT NOT NULL,\
selection_count INTEGER NOT NULL DEFAULT 0,\
PRIMARY KEY(context_key,entry_key,value));\
CREATE TABLE IF NOT EXISTS fixed_candidate_positions(\
context_key TEXT NOT NULL,entry_key TEXT NOT NULL,value TEXT NOT NULL,\
position INTEGER NOT NULL CHECK(position BETWEEN 1 AND 5),\
PRIMARY KEY(context_key,entry_key,value),UNIQUE(context_key,position));\
CREATE TABLE IF NOT EXISTS pick_transitions(\
previous_key TEXT NOT NULL,previous_value TEXT NOT NULL,key TEXT NOT NULL,value TEXT NOT NULL,\
count INTEGER NOT NULL DEFAULT 0,\
updated_at INTEGER NOT NULL DEFAULT(unixepoch()),\
PRIMARY KEY(previous_key,previous_value,key,value));\
CREATE INDEX IF NOT EXISTS pick_transitions_updated_at ON pick_transitions(updated_at);\
CREATE TABLE IF NOT EXISTS pinyin_typo_counts(\
typed TEXT NOT NULL,intended TEXT NOT NULL,\
accepted INTEGER NOT NULL CHECK(accepted BETWEEN 0 AND 1000),\
updated_at INTEGER NOT NULL DEFAULT(unixepoch()),\
PRIMARY KEY(typed,intended));\
CREATE TABLE IF NOT EXISTS pinyin_autocorrect_suppressions(\
input TEXT PRIMARY KEY,\
commits INTEGER NOT NULL CHECK(commits >= 1),\
updated_at INTEGER NOT NULL DEFAULT(unixepoch()));\
CREATE TABLE IF NOT EXISTS shuangpin_typo_counts(\
profile TEXT NOT NULL,typed TEXT NOT NULL,intended TEXT NOT NULL,\
accepted INTEGER NOT NULL DEFAULT 0 CHECK(accepted BETWEEN 0 AND 1000),\
rejected INTEGER NOT NULL DEFAULT 0 CHECK(rejected BETWEEN 0 AND 1000),\
updated_at INTEGER NOT NULL DEFAULT(unixepoch()),\
PRIMARY KEY(profile,typed,intended));\
CREATE TABLE IF NOT EXISTS shuangpin_autocorrect_suppressions(\
profile TEXT NOT NULL,input TEXT NOT NULL,\
commits INTEGER NOT NULL CHECK(commits >= 1),\
updated_at INTEGER NOT NULL DEFAULT(unixepoch()),\
PRIMARY KEY(profile,input));\
CREATE TABLE IF NOT EXISTS pinned_candidates(\
context_key TEXT PRIMARY KEY,\
value TEXT NOT NULL,\
updated_at INTEGER NOT NULL DEFAULT(unixepoch()));\
CREATE TABLE IF NOT EXISTS personal_bigram(\
previous TEXT NOT NULL,word TEXT NOT NULL,count INTEGER NOT NULL CHECK(count>0),\
PRIMARY KEY(previous,word)) WITHOUT ROWID;\
CREATE INDEX IF NOT EXISTS personal_bigram_word ON personal_bigram(word);\
CREATE TABLE IF NOT EXISTS personal_trigram(\
earlier TEXT NOT NULL,previous TEXT NOT NULL,word TEXT NOT NULL,\
count INTEGER NOT NULL CHECK(count>0),\
PRIMARY KEY(earlier,previous,word)) WITHOUT ROWID;\
CREATE INDEX IF NOT EXISTS personal_trigram_word ON personal_trigram(word);\
CREATE TABLE IF NOT EXISTS quick_phrase_slots(\
code TEXT PRIMARY KEY,slot INTEGER NOT NULL CHECK(slot>=0));";

/// J:96-102. A ranking change on a user word keeps `user_inserted = 1`, so the conflict branch leaves that column alone.
pub(crate) const UPSERT_JOURNAL_SQL: &str = "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display) VALUES(?1,?2,?3,'upsert',?4,?5) ON CONFLICT(dictionary,key,value) DO UPDATE SET operation='upsert',weight=excluded.weight, display=excluded.display,updated_at=unixepoch()";

/// J:547-559.
const USER_INSERT_SQL: &str = "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted) VALUES(?1,?2,?3,'upsert',?4,?5,1) ON CONFLICT(dictionary,key,value) DO UPDATE SET operation='upsert',weight=excluded.weight, display=excluded.display,user_inserted=1,updated_at=unixepoch()";

/// J:583-596, run against `schema` (`main` on the journal connection, the attach alias on a dictionary connection). `user_inserted` is preserved on conflict: removing a user's own word still records that it was theirs.
pub(crate) fn tombstone_sql(schema: &str) -> String {
    format!("INSERT INTO {schema}.user_dictionary_operations(dictionary,key,value,operation) VALUES(?1,?2,?3,'delete') ON CONFLICT(dictionary,key,value) DO UPDATE SET operation='delete',weight=0,display='', updated_at=unixepoch()")
}

/// Bumped by `close_cached_journals`; a connection opened under an older value closes when its operation ends instead of being kept.
static CACHE_GENERATION: AtomicU64 = AtomicU64::new(0);

/// The idle connections, at most one per thread, each tagged with its thread's `ThreadJournal::id`. A thread's entry goes when the thread exits. A plain list rather than a slot per thread: parking and taking back then allocate nothing once the list has grown to the number of threads.
static PARKED_JOURNALS: Mutex<Vec<ParkedJournal>> = Mutex::new(Vec::new());

/// Free entries `JournalUse::new` keeps in `PARKED_JOURNALS`: a session is normally used from one or two threads.
const PARKING_HEADROOM: usize = 4;

/// Source of `ThreadJournal::id`.
static NEXT_THREAD: AtomicU64 = AtomicU64::new(0);

/// How many live sessions use each journal (`JournalUse`); the last one to end closes every thread's idle connection to it.
static JOURNAL_USERS: LazyLock<Mutex<HashMap<PathBuf, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// A thread's connection while no operation of that thread holds it. `Connection` is `Send`, so another thread may take it out and close it.
struct ParkedJournal {
    thread: u64,
    path: PathBuf,
    generation: u64,
    connection: Connection,
}

/// This thread's journal state.
struct ThreadJournal {
    id: u64,
    /// The connection while operations of this thread hold it, empty while it is parked. Allocated once per thread: moving the connection between here and `PARKED_JOURNALS` costs no allocation on the keystroke path.
    active: Rc<OnceCell<Connection>>,
    active_path: PathBuf,
    /// The generation `active` was opened or taken back under; `None` once `release_thread_journal` asked for it to close when its operation ends.
    active_generation: Option<u64>,
}

impl ThreadJournal {
    fn new() -> ThreadJournal {
        ThreadJournal {
            id: NEXT_THREAD.fetch_add(1, Ordering::Relaxed),
            active: Rc::new(OnceCell::new()),
            active_path: PathBuf::new(),
            active_generation: None,
        }
    }

    /// Takes this thread's parked connection out of the list.
    fn unpark(&self) -> Option<ParkedJournal> {
        let mut parked = lock(&PARKED_JOURNALS);
        let index = parked.iter().position(|entry| entry.thread == self.id)?;
        Some(parked.swap_remove(index))
    }
}

impl Drop for ThreadJournal {
    /// The thread is exiting: its idle connection closes with it.
    fn drop(&mut self) {
        drop(self.unpark());
    }
}

thread_local! {
    static THREAD_JOURNAL: RefCell<Option<ThreadJournal>> = const { RefCell::new(None) };
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic while the list was locked leaves each entry whole: a push or removal either happened or did not.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A journal connection with the v4 schema applied. Dropping it inside an open transaction rolls the transaction back, so an early return can never leave the cached connection holding the write lock (J:478-486).
///
/// The connection is shared with every other `JournalConnection` of this thread and path, so a caller must not open the journal again while it has a transaction open: the inner handle's drop would end the outer transaction.
pub struct JournalConnection {
    /// Always `Some` until `drop` takes it; the cell is never emptied while a handle exists, because emptying it needs the only reference.
    connection: Option<Rc<OnceCell<Connection>>>,
}

impl JournalConnection {
    fn shared(connection: &Rc<OnceCell<Connection>>) -> JournalConnection {
        JournalConnection {
            connection: Some(Rc::clone(connection)),
        }
    }
}

impl Deref for JournalConnection {
    type Target = Connection;

    fn deref(&self) -> &Connection {
        self.connection
            .as_deref()
            .and_then(OnceCell::get)
            .expect("a journal handle keeps its connection until it drops")
    }
}

impl Drop for JournalConnection {
    fn drop(&mut self) {
        let Some(connection) = self.connection.take() else {
            return;
        };
        if let Some(open) = connection.get() {
            if !open.is_autocommit() {
                // A cached connection outlives the call, so an abandoned transaction would keep its write lock and fail every later operation. A failed ROLLBACK leaves nothing further to undo here: the connection is either already out of the transaction or unusable.
                let _ = open.execute_batch("ROLLBACK");
            }
        }
        // The last handle of this thread's connection parks it. At thread exit the state may already be gone, and the handle's own reference closes the connection.
        let _ = THREAD_JOURNAL.try_with(move |journal| {
            let mut journal = journal.borrow_mut();
            let Some(journal) = journal.as_mut() else {
                return;
            };
            if !Rc::ptr_eq(&connection, &journal.active) {
                // A connection of its own, opened while one to another journal was in use: it closes here.
                return;
            }
            drop(connection);
            // Another handle of this thread still uses it.
            let Some(open) = Rc::get_mut(&mut journal.active).and_then(OnceCell::take) else {
                return;
            };
            // Read under the list's lock: `close_cached_journals` bumps the generation before it empties the list, so a connection parked here is either seen as stale now or taken out by that close.
            let mut parked = lock(&PARKED_JOURNALS);
            let generation = CACHE_GENERATION.load(Ordering::Acquire);
            if journal.active_generation == Some(generation) {
                parked.push(ParkedJournal {
                    thread: journal.id,
                    path: std::mem::take(&mut journal.active_path),
                    generation,
                    connection: open,
                });
            }
        });
    }
}

/// `sqlite3_open_v2` with the reference's flags plus the 5 s busy timeout every engine connection uses (J:65-76). Without CREATE a missing file stays missing.
pub(crate) fn open_database(path: &Path, flags: OpenFlags) -> Result<Connection> {
    let path = crate::paths::sqlite_path_no_follow(path)?;
    if let Ok(metadata) = std::fs::symlink_metadata(&path) {
        if !metadata.file_type().is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "database path is not a regular file",
            )
            .into());
        }
    }
    let connection = Connection::open_with_flags(
        &path,
        flags | OpenFlags::SQLITE_OPEN_FULL_MUTEX | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    connection.busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))?;
    Ok(connection)
}

/// 检查点以后 `-wal` 截回的大小（4 MiB），与 SQLite 默认的自动检查点（1000 页）在 4 KiB 页上的大小相同。写成字面量而不是 `format!` 拼出来：首次打开日志常发生在选词提交的路径上，那里有分配次数预算。
#[cfg(not(target_os = "ios"))]
const WAL_SIZE_LIMIT_PRAGMA: &str = "PRAGMA journal_size_limit=4194304";

/// 让日志连接用 WAL、`synchronous=NORMAL` 提交。默认的回滚日志加 `synchronous=FULL` 每次提交都要建 `-journal`、fsync 两三次再删掉它；选词计数、调频、选词转移和整句联想都在输入线程上同步提交，低端机的 eMMC 上一次 fsync 就是几到几十毫秒，选非首位候选时整个键盘随之卡顿。WAL 下一次提交只是往 `-wal` 末尾追加，NORMAL 只在检查点时 fsync；断电最多丢掉最后几次学习，不会损坏数据库，学习数据可以接受。
///
/// WAL 写进文件头、对之后所有连接生效，`synchronous` 却按连接算，所以长期持有的写连接（每线程缓存的日志连接、整句联想的写线程）各自调一次。切换要短暂独占数据库：别的进程正占着回滚日志的锁、或文件系统不支持 WAL 的共享内存（网络盘）时 SQLite 留在原来的模式，这里照常使用；只有真的进了 WAL 才降到 NORMAL，回滚日志配 NORMAL 断电时可能损坏数据库。
///
/// iOS 不切换。键盘扩展和 App 在 App Group 容器里共用日志，WAL 的连接在整个生命周期里都对 `-shm` 持有一把共享锁；挂起时还持有 App Group 里的文件锁会被系统按 `0xdead10cc` 杀掉。会话收起时会释放本线程的日志连接，但整句联想的写线程和其他线程缓存的连接要等下一次访问或 `close_cached_journals` 才关，回滚日志在两次事务之间不持锁，换成 WAL 就会在挂起时一直拿着锁。
///
/// 主词库（`msime-pinyin.db`、`msime-english.db` 的工作副本）也不切换：重置学习数据把它们挪作备份时不带旁路文件，旧的 `-wal` 会套到新复制的词库上。
///
/// 跨文件事务因此不再是整体原子的。删词、个人词库编辑、五笔造词这些操作把日志 ATTACH 到词库连接上，在一个事务里同时改词库和日志；只要有一个库是 WAL，SQLite 就不用超级日志把它们绑在一起，只保证每个文件各自原子。提交顺序是固定的：WAL 的日志在提交第一阶段就写下带提交标记的帧，回滚日志的词库要到第二阶段删掉 `-journal`（或超级日志）才算提交，所以进程被杀或断电只会留下「日志已经记下、词库工作副本回滚了」，不会反过来。日志是学习数据的权威来源，工作副本在下一次回放（安装、升级、换代次）时按日志补齐；在那之前这一次删词或编辑在候选里看不到，这是接受的代价。改动这些事务时不要依赖「两边要么都在、要么都不在」。
///
/// 进了 WAL 还设 `journal_size_limit`：默认 -1 时检查点以后 `-wal` 不截短，一次大事务（导入词库、合并快照、减半）把它撑到多大就一直多大；整句联想的写连接在输入法进程里一直开着，最后一个连接关闭时才删 `-wal` 的机会也就没有了。设了上限，检查点重置日志时把 `-wal` 截回这个大小，不影响持久性。
pub(crate) fn prefer_write_ahead_log(connection: &Connection) {
    #[cfg(not(target_os = "ios"))]
    {
        // 就地比较结果，不另外分配一个 `String`：首次打开日志常发生在选词提交的路径上。
        let wal = connection
            .query_row("PRAGMA journal_mode=WAL", [], |row| {
                Ok(row
                    .get_ref(0)?
                    .as_str()
                    .is_ok_and(|mode| mode.eq_ignore_ascii_case("wal")))
            })
            .unwrap_or(false);
        if wal {
            // 设置失败只是保持默认的 FULL：更慢，但不影响正确性。
            let _ = connection.execute_batch("PRAGMA synchronous=NORMAL");
            // 这条 PRAGMA 回一行新值；设置失败只是 `-wal` 不截短。
            let _ = connection.query_row(WAL_SIZE_LIMIT_PRAGMA, [], |_| Ok(()));
        }
    }
    #[cfg(target_os = "ios")]
    let _ = connection;
}

/// 把日志 `-wal` 里已提交的内容并回主文件并把 `-wal` 截成空文件，供要整体挪动日志文件的操作（重置学习数据）先调用：挪走的主文件本身就是完整的日志，留在原名旁边的空 `-wal` 也不会被套到新日志上。不是 WAL 的日志上这是空操作；打不开或别的连接正占着时尽力而为，调用方仍按原来的方式处理旁路文件。
pub(crate) fn checkpoint_journal(path: &Path) {
    if let Ok(connection) = open_database(path, OpenFlags::SQLITE_OPEN_READ_WRITE) {
        let _ = connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
    }
}

/// A dictionary (`msime-pinyin.db`, `msime-english.db`) opened for writing; a missing dictionary is an error, never a new empty file.
pub(crate) fn open_dictionary_for_writing(path: &Path) -> Result<Connection> {
    open_database(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
}

/// Read-only access for lookups, which must never create the file: `None` when it does not exist.
pub(crate) fn open_existing_read_only(path: &Path) -> Result<Option<Connection>> {
    if !path.try_exists()? {
        return Ok(None);
    }
    open_database(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map(Some)
}

/// 把 SQLite 无法读取为数据库的日志挪到旁边，让宿主从空日志启动，不再因为日志损坏而完全起不来。
///
/// 每个宿主都在第一个会话前准备词库代次，并在其中回放日志：不是数据库的日志会让回放、准备以及整个宿主在每次启动时都失败，用户没有可见的处理入口。2026 年 10 月在一台 Windows 机器上发现过被覆盖成零的日志；手动挪走文件后输入法才恢复。
///
/// 只采信 SQLite 自己的判断：读取 schema 失败且错误为 `SQLITE_NOTADB` 或 `SQLITE_CORRUPT` 时才改名。仅仅忙、被锁定或权限不足的日志保持不动，避免误挪正常日志。改名后的文件保留在原目录，名称为 `msime_user.db.corrupt-<unix seconds>`；`-wal`、`-shm` 和 `-journal` 旁路文件也使用同一名称前缀，因此不会删除数据，也不会让旧旁路文件套到新日志上。日志被挪动时返回新路径，否则返回 `None`。
pub fn quarantine_corrupt_journal(path: &Path) -> std::io::Result<Option<PathBuf>> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        // 文件不存在或不是普通文件：打开路径已经会拒绝后者。
        _ => return Ok(None),
    }
    let verdict = open_database(path, OpenFlags::SQLITE_OPEN_READ_ONLY).and_then(|connection| {
        connection
            .query_row("SELECT count(*) FROM sqlite_master", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(Into::into)
    });
    let corrupt = matches!(
        verdict,
        Err(crate::error::EngineError::Sqlite(rusqlite::Error::SqliteFailure(ref error, _)))
            if matches!(error.code, rusqlite::ErrorCode::NotADatabase | rusqlite::ErrorCode::DatabaseCorrupt)
    );
    if !corrupt {
        return Ok(None);
    }
    // 当前线程可能仍持有该文件的连接；Windows 不能改名移动已打开的文件。
    close_cached_journals();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let suffixes = ["-wal", "-shm", "-journal"];
    let mut target = path.with_file_name(format!("{name}.corrupt-{stamp}"));
    let mut attempt = 1;
    while std::fs::symlink_metadata(&target).is_ok()
        || suffixes.iter().any(|suffix| {
            std::fs::symlink_metadata(target.with_file_name(format!(
                "{}{suffix}",
                target.file_name().unwrap_or_default().to_string_lossy()
            )))
            .is_ok()
        })
    {
        target = path.with_file_name(format!("{name}.corrupt-{stamp}-{attempt}"));
        attempt += 1;
    }
    // 先挪旁路文件，再挪主文件；这样中途退出时，原路径不会留下会被新数据库套用的旁路文件。
    for suffix in suffixes {
        let sidecar = path.with_file_name(format!("{name}{suffix}"));
        if std::fs::symlink_metadata(&sidecar).is_ok() {
            let moved = target.with_file_name(format!(
                "{}{suffix}",
                target.file_name().unwrap_or_default().to_string_lossy()
            ));
            std::fs::rename(&sidecar, moved)?;
        }
    }
    std::fs::rename(path, &target)?;
    Ok(Some(target))
}

/// Open (creating) the journal, reusing this thread's connection for the path.
pub fn open_journal(path: &Path) -> Result<JournalConnection> {
    let generation = CACHE_GENERATION.load(Ordering::Acquire);
    let cached = THREAD_JOURNAL.try_with(|journal| {
        let mut journal = journal.borrow_mut();
        let journal = journal.get_or_insert_with(ThreadJournal::new);
        if journal.active.get().is_some() {
            // An operation of this thread holds the connection and this open is part of it. It shares the connection even when a close came in between (it closes when the operation ends): a second connection to the same file would wait on the first one's transaction.
            if journal.active_path == path {
                return Ok(JournalConnection::shared(&journal.active));
            }
            // A connection to another journal is in use: this one gets a connection of its own.
            return open_uncached(path);
        }
        let parked = journal.unpark();
        let connection = match parked {
            Some(parked) if parked.generation == generation && parked.path == path => {
                journal.active_path = parked.path;
                parked.connection
            }
            other => {
                // Released before opening, so a thread never holds two journal files open.
                drop(other);
                let connection = open_configured(path)?;
                journal.active_path = path.to_path_buf();
                connection
            }
        };
        journal.active_generation = Some(generation);
        // Empty: checked above, and nothing of this thread filled it since.
        let _ = journal.active.set(connection);
        Ok(JournalConnection::shared(&journal.active))
    });
    match cached {
        Ok(opened) => opened,
        // A session a host drops while its thread exits (sessions kept in a thread-local map) can outlive this thread's cache, which thread-local destruction order does not fix. Its last flush gets a connection of its own, closed when that flush ends.
        Err(_) => open_uncached(path),
    }
}

fn open_configured(path: &Path) -> Result<Connection> {
    let connection = open_database(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )?;
    prefer_write_ahead_log(&connection);
    ensure_schema(&connection)?;
    Ok(connection)
}

/// A connection no thread caches, closed when the handle drops.
fn open_uncached(path: &Path) -> Result<JournalConnection> {
    let connection = open_configured(path)?;
    Ok(JournalConnection {
        connection: Some(Rc::new(OnceCell::from(connection))),
    })
}

/// Every `CREATE ... IF NOT EXISTS` of J:114-154 and NS:156-167, the `user_inserted` column check, then `PRAGMA user_version = 4` (informational only).
pub fn ensure_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(SCHEMA_SQL)?;
    let has_user_inserted = {
        let mut columns = connection.prepare("PRAGMA table_info(user_dictionary_operations)")?;
        let names = columns.query_map([], |row| row.get::<_, String>(1))?;
        let mut found = false;
        for name in names {
            if name? == "user_inserted" {
                found = true;
                break;
            }
        }
        found
    };
    if !has_user_inserted {
        connection.execute_batch(
            "ALTER TABLE user_dictionary_operations ADD COLUMN user_inserted INTEGER NOT NULL DEFAULT 0",
        )?;
    }
    // Informational only: nothing reads it, and every table is created with IF NOT EXISTS, so an older engine that writes its own number back simply ignores the tables it does not know (J:177-178).
    connection.execute_batch(&format!("PRAGMA user_version={SCHEMA_VERSION}"))?;
    Ok(())
}

/// Open the journal and drop pinyin upserts with weight below 1, left by the old rebalance staircase (J:521-533).
pub fn ensure_user_database(path: &Path) -> Result<()> {
    let journal = open_journal(path)?;
    drop_stale_pinyin_upserts(&journal);
    Ok(())
}

/// Drop ranking rows left by the old rebalance staircase so installer replay cannot bury shipped frequencies such as 先/xian under negative weights. The reference ignores this statement's result (J:528-531): replay skips those rows anyway, so a busy journal must not fail the operation that asked for the journal.
pub(crate) fn drop_stale_pinyin_upserts(connection: &Connection) {
    let _ = connection.execute(
        "DELETE FROM user_dictionary_operations WHERE dictionary='pinyin' AND operation='upsert' AND weight < 1",
        [],
    );
}

/// Close every thread's cached journal connection, release the personal n-gram stores and the local-mode connections. Call before deleting or replacing a data directory.
pub fn close_cached_journals() {
    // First, because writing the stores' queues opens the journal through this thread's cache; the slot is cleared after it.
    super::ngram_store::release_all();
    CACHE_GENERATION.fetch_add(1, Ordering::AcqRel);
    // Idle connections of every thread close now; one an operation still holds closes when that operation ends, since its generation is gone.
    close_idle_journals(None);
    crate::local::database::close_cached_local_databases();
}

/// Closes the parked connections of every thread, or only those to `path`. The connections are dropped after the lock is released.
fn close_idle_journals(path: Option<&Path>) {
    let closing: Vec<ParkedJournal> = lock(&PARKED_JOURNALS)
        .extract_if(.., |parked| path.is_none_or(|path| parked.path == path))
        .collect();
    drop(closing);
}

/// Closes this thread's cached journal connection now. The reference cached only its default journal path and opened msime's journal per call (user_dictionary_journal.cpp:445-452), so a thread that is done with the journal must not keep a handle that blocks the reset's rename or a generation delete on Windows. A connection an operation still holds closes when that operation ends.
pub(crate) fn release_thread_journal() {
    // At thread exit the state may already be destroyed, which closed its connection; there is nothing left to release then.
    let _ = THREAD_JOURNAL.try_with(|journal| {
        if let Some(journal) = journal.borrow_mut().as_mut() {
            drop(journal.unpark());
            journal.active_generation = None;
        }
    });
}

/// A live session's claim on its journal, counted per path. `end` (or dropping it) after the session's last journal write gives the claim up; the last claim on a path closes every thread's idle connection to that journal, wherever the session was created or used.
pub(crate) struct JournalUse {
    path: Option<PathBuf>,
}

impl JournalUse {
    pub(crate) fn new(path: PathBuf) -> JournalUse {
        *lock(&JOURNAL_USERS).entry(path.clone()).or_default() += 1;
        // Room for the threads that will use this session to park their connections, made here on the cold path so that parking on a keystroke never grows the list.
        lock(&PARKED_JOURNALS).reserve(PARKING_HEADROOM);
        JournalUse { path: Some(path) }
    }

    pub(crate) fn end(&mut self) {
        let Some(path) = self.path.take() else {
            return;
        };
        let last = {
            let mut users = lock(&JOURNAL_USERS);
            match users.get_mut(&path) {
                Some(count) if *count > 1 => {
                    *count -= 1;
                    false
                }
                _ => {
                    users.remove(&path);
                    true
                }
            }
        };
        if last {
            close_idle_journals(Some(&path));
        }
    }
}

impl Drop for JournalUse {
    fn drop(&mut self) {
        self.end();
    }
}

/// Whether this thread still caches a journal connection, in use or parked.
#[cfg(test)]
pub(crate) fn thread_holds_journal() -> bool {
    THREAD_JOURNAL.with(|journal| {
        journal.borrow().as_ref().is_some_and(|journal| {
            journal.active.get().is_some()
                || lock(&PARKED_JOURNALS)
                    .iter()
                    .any(|parked| parked.thread == journal.id)
        })
    })
}

/// The close count `close_cached_journals` bumps before it closes every thread's idle connection. A test that expects this thread to keep a connection reads it before and after: another test may close every journal in between, which only a moved count reveals.
#[cfg(test)]
pub(crate) fn cache_generation() -> u64 {
    CACHE_GENERATION.load(Ordering::Acquire)
}

/// How many threads keep an idle connection to `path`.
#[cfg(test)]
pub(crate) fn parked_journals(path: &Path) -> usize {
    lock(&PARKED_JOURNALS)
        .iter()
        .filter(|parked| parked.path == path)
        .count()
}

/// The syllables of a journal key; empty when the key or any segment is empty, which means the key cannot be stored (J:182-199).
pub(crate) fn pinyin_segments(key: &str) -> Vec<String> {
    let segments = split_segments(key);
    if segments.iter().any(String::is_empty) {
        return Vec::new();
    }
    segments
}

/// The pinyin table for a journal key, `None` for an empty key or an empty segment (J:182-207).
pub fn pinyin_table(key: &str) -> Option<String> {
    build_table_name(&pinyin_segments(key))
}

/// One journal upsert on an open connection (J:104-112).
pub(crate) fn write_upsert(
    connection: &Connection,
    kind: PersonalDictionaryKind,
    key: &str,
    value: &str,
    weight: i64,
    display: &str,
) -> Result<()> {
    connection
        .prepare_cached(UPSERT_JOURNAL_SQL)?
        .execute(params![kind.journal_name(), key, value, weight, display])?;
    Ok(())
}

/// Upsert with `user_inserted = 1` (J:547-581).
pub fn record_user_insert(
    user_db: &Path,
    kind: PersonalDictionaryKind,
    key: &str,
    value: &str,
    weight: i64,
    display: &str,
) -> Result<()> {
    let journal = open_journal(user_db)?;
    journal.prepare_cached(USER_INSERT_SQL)?.execute(params![
        kind.journal_name(),
        key,
        value,
        weight,
        display
    ])?;
    Ok(())
}

/// The existing journal, or `None` when there is none yet; for lookups that must answer false rather than create the file.
fn existing_journal(user_db: &Path) -> Option<JournalConnection> {
    if !user_db.try_exists().unwrap_or(false) {
        return None;
    }
    open_journal(user_db).ok()
}

/// Does not look at `operation` (J:598-609). A missing journal answers false without creating it.
pub fn is_user_inserted(
    user_db: &Path,
    kind: PersonalDictionaryKind,
    key: &str,
    value: &str,
) -> bool {
    let Some(journal) = existing_journal(user_db) else {
        return false;
    };
    // The reference answers false on any read failure (J:598-609): the question is only ever asked to decide whether to treat a word specially.
    journal
        .prepare_cached(
            "SELECT user_inserted FROM user_dictionary_operations WHERE dictionary=?1 AND key=?2 AND value=?3 LIMIT 1",
        )
        .and_then(|mut statement| {
            statement
                .query_row(params![kind.journal_name(), key, value], |row| {
                    row.get::<_, i64>(0)
                })
                .optional()
        })
        .ok()
        .flatten()
        .is_some_and(|inserted| inserted != 0)
}

/// 当前候选与有效的用户覆盖权重完全一致；没有日志或读取失败时不保护首选。
pub(crate) fn has_matching_user_weight(
    user_db: &Path,
    kind: PersonalDictionaryKind,
    key: &str,
    value: &str,
    weight: i64,
) -> bool {
    if key.is_empty() || value.is_empty() {
        return false;
    }
    let Some(journal) = existing_journal(user_db) else {
        return false;
    };
    journal
        .prepare_cached(
            "SELECT 1 FROM user_dictionary_operations WHERE dictionary=?1 AND key=?2 AND value=?3 AND operation='upsert' AND weight=?4",
        )
        .and_then(|mut statement| {
            statement.exists(params![kind.journal_name(), key, value, weight])
        })
        .unwrap_or(false)
}

/// A delete tombstone exists (J:611-621).
pub fn is_user_deleted(
    user_db: &Path,
    kind: PersonalDictionaryKind,
    key: &str,
    value: &str,
) -> bool {
    let Some(journal) = existing_journal(user_db) else {
        return false;
    };
    // False on a read failure, as in J:611-621.
    journal
        .prepare_cached(
            "SELECT 1 FROM user_dictionary_operations WHERE dictionary=?1 AND key=?2 AND value=?3 AND operation='delete' LIMIT 1",
        )
        .and_then(|mut statement| statement.exists(params![kind.journal_name(), key, value]))
        .unwrap_or(false)
}

/// Fixtures the learning modules' tests share: a temporary directory with a journal path and small working dictionaries in the shipped schemas.
#[cfg(test)]
pub(crate) mod test_support {
    use std::path::PathBuf;

    use rusqlite::Connection;
    use tempfile::TempDir;

    use crate::types::{CandidateSource, WordItem};

    pub struct Dir {
        pub root: TempDir,
    }

    impl Dir {
        pub fn new() -> Self {
            Self {
                root: tempfile::tempdir().unwrap(),
            }
        }

        pub fn journal(&self) -> PathBuf {
            self.root.path().join("msime_user.db")
        }

        pub fn main_db(&self) -> PathBuf {
            self.root.path().join("msime-pinyin.db")
        }

        pub fn english_db(&self) -> PathBuf {
            self.root.path().join("msime-english.db")
        }

        /// Pinyin rows `(key, word, weight)`, each into the table its key names.
        pub fn pinyin(&self, rows: &[(&str, &str, i64)]) -> &Self {
            let connection = Connection::open(self.main_db()).unwrap();
            for (key, word, weight) in rows {
                let table = super::pinyin_table(key).unwrap();
                connection
                    .execute_batch(&format!(
                        "CREATE TABLE IF NOT EXISTS \"{table}\" (\"key\" text, \"jp\" text, \"value\" text, \"weight\" integer default 0);"
                    ))
                    .unwrap();
                let jp: String = key
                    .split('\'')
                    .filter_map(|syllable| syllable.chars().next())
                    .collect();
                connection
                    .execute(
                        &format!("INSERT INTO \"{table}\" (key, jp, value, weight) VALUES (?1, ?2, ?3, ?4)"),
                        (key, jp, word, weight),
                    )
                    .unwrap();
            }
            self
        }

        pub fn wubi(&self, rows: &[(&str, &str, i64)]) -> &Self {
            let connection = Connection::open(self.main_db()).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE IF NOT EXISTS wubi86(key TEXT, value TEXT, weight INTEGER);",
                )
                .unwrap();
            for row in rows {
                connection
                    .execute(
                        "INSERT INTO wubi86(key,value,weight) VALUES(?1,?2,?3)",
                        *row,
                    )
                    .unwrap();
            }
            self
        }

        pub fn english(&self, rows: &[(&str, &str, i64)]) -> &Self {
            crate::dictionary::english::ensure_english_schema(&self.english_db()).unwrap();
            let connection = Connection::open(self.english_db()).unwrap();
            for row in rows {
                connection
                    .execute(
                        "INSERT INTO english_words(word,display,weight) VALUES(?1,?2,?3)",
                        *row,
                    )
                    .unwrap();
            }
            self
        }

        pub fn weight(&self, key: &str, word: &str) -> Option<i64> {
            let table = super::pinyin_table(key).unwrap();
            Connection::open(self.main_db())
                .unwrap()
                .query_row(
                    &format!("SELECT weight FROM \"{table}\" WHERE key=?1 AND value=?2"),
                    (key, word),
                    |row| row.get(0),
                )
                .ok()
        }
    }

    pub fn query_i64(path: &std::path::Path, sql: &str) -> Option<i64> {
        let connection = Connection::open(path).unwrap();
        connection.query_row(sql, [], |row| row.get(0)).ok()
    }

    pub fn count(path: &std::path::Path, sql: &str) -> i64 {
        query_i64(path, sql).unwrap_or(0)
    }

    pub fn item(pinyin: &str, word: &str, weight: i64) -> WordItem {
        WordItem::new(pinyin, word, weight, CandidateSource::Database, pinyin)
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{count, query_i64, Dir};
    use super::*;

    fn upsert(
        user_db: &Path,
        kind: PersonalDictionaryKind,
        key: &str,
        value: &str,
        weight: i64,
        display: &str,
    ) -> Result<()> {
        let journal = open_journal(user_db)?;
        write_upsert(&journal, kind, key, value, weight, display)
    }

    fn tombstone(user_db: &Path, kind: PersonalDictionaryKind, key: &str, value: &str) {
        open_journal(user_db)
            .unwrap()
            .execute(
                &tombstone_sql("main"),
                params![kind.journal_name(), key, value],
            )
            .unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn opening_a_journal_rejects_a_symlinked_path() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let external = root.path().join("external.db");
        let linked = root.path().join("msime_user.db");
        symlink(&external, &linked).unwrap();

        assert!(open_journal(&linked).is_err());
        assert!(!external.exists());
    }

    #[cfg(unix)]
    #[test]
    fn opening_a_journal_rejects_a_symlinked_parent() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let linked = root.path().join("user");
        symlink(external.path(), &linked).unwrap();
        let journal = linked.join("msime_user.db");

        assert!(open_journal(&journal).is_err());
        assert!(!external.path().join("msime_user.db").exists());
    }

    #[test]
    fn table_names_follow_the_key() {
        assert_eq!(pinyin_table("ni'hao").as_deref(), Some("tbl_2_n"));
        assert_eq!(
            pinyin_table("ni'ni'ni'ni'ni'ni'ni'ni").as_deref(),
            Some("tbl_others_n")
        );
        assert_eq!(pinyin_table(""), None);
        assert_eq!(pinyin_table("ni''hao"), None);
        assert_eq!(pinyin_table("'ni"), None);
        assert_eq!(pinyin_table("ni'"), None);
    }

    #[test]
    fn pinyin_segments_reserves_key_capacity() {
        let key: String = (0..100)
            .map(|index| if index % 5 == 3 { '\'' } else { 'a' })
            .collect();
        let segments = pinyin_segments(&key);
        assert_eq!(segments.len(), 21);
        assert_eq!(segments.capacity(), 21);
    }

    /// test_typo_correction_input_session.cpp:381-403: a journal written by the shipped engine (v3, four tables) is upgraded in place.
    #[test]
    fn a_version_3_journal_becomes_version_4() {
        let dir = Dir::new();
        let journal = dir.journal();
        Connection::open(&journal)
            .unwrap()
            .execute_batch(
                "CREATE TABLE user_dictionary_operations(dictionary TEXT NOT NULL,key TEXT NOT NULL,value TEXT NOT NULL,operation TEXT NOT NULL,weight INTEGER NOT NULL DEFAULT 0,display TEXT NOT NULL DEFAULT '',user_inserted INTEGER NOT NULL DEFAULT 0,updated_at INTEGER NOT NULL DEFAULT(unixepoch()),PRIMARY KEY(dictionary,key,value));\
                 INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight) VALUES('pinyin','guan''xi','关系','upsert',5);\
                 PRAGMA user_version=3;",
            )
            .unwrap();
        ensure_user_database(&journal).unwrap();
        assert_eq!(
            count(&journal, "SELECT count(*) FROM user_dictionary_operations"),
            1
        );
        assert_eq!(query_i64(&journal, "PRAGMA user_version"), Some(4));
        for table in [
            "pick_transitions",
            "pinyin_typo_counts",
            "pinyin_autocorrect_suppressions",
            "shuangpin_typo_counts",
            "shuangpin_autocorrect_suppressions",
            "pinned_candidates",
            "personal_bigram",
            "personal_trigram",
        ] {
            assert_eq!(
                count(
                    &journal,
                    &format!("SELECT count(*) FROM sqlite_master WHERE name='{table}'")
                ),
                1,
                "{table} was not created"
            );
        }
    }

    #[test]
    fn a_journal_without_user_inserted_gains_the_column() {
        let dir = Dir::new();
        let journal = dir.journal();
        Connection::open(&journal)
            .unwrap()
            .execute_batch(
                "CREATE TABLE user_dictionary_operations(dictionary TEXT NOT NULL,key TEXT NOT NULL,value TEXT NOT NULL,operation TEXT NOT NULL,weight INTEGER NOT NULL DEFAULT 0,display TEXT NOT NULL DEFAULT '',updated_at INTEGER NOT NULL DEFAULT(unixepoch()),PRIMARY KEY(dictionary,key,value));",
            )
            .unwrap();
        record_user_insert(&journal, PersonalDictionaryKind::Pinyin, "ni", "你", 10, "").unwrap();
        assert!(is_user_inserted(
            &journal,
            PersonalDictionaryKind::Pinyin,
            "ni",
            "你"
        ));
    }

    #[test]
    fn ensure_drops_staircase_leftovers_only() {
        let dir = Dir::new();
        let journal = dir.journal();
        upsert(
            &journal,
            PersonalDictionaryKind::Pinyin,
            "xian",
            "先",
            -3,
            "",
        )
        .unwrap();
        upsert(
            &journal,
            PersonalDictionaryKind::Pinyin,
            "xian",
            "线",
            0,
            "",
        )
        .unwrap();
        upsert(
            &journal,
            PersonalDictionaryKind::Pinyin,
            "xian",
            "现",
            1,
            "",
        )
        .unwrap();
        upsert(&journal, PersonalDictionaryKind::Wubi, "a", "工", 0, "").unwrap();
        ensure_user_database(&journal).unwrap();
        assert_eq!(
            count(
                &journal,
                "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='pinyin'"
            ),
            1
        );
        assert_eq!(
            count(
                &journal,
                "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='wubi'"
            ),
            1
        );
    }

    #[test]
    fn row_primitives_keep_user_inserted_and_tombstones() {
        let dir = Dir::new();
        let journal = dir.journal();
        let kind = PersonalDictionaryKind::Pinyin;
        assert!(!is_user_inserted(&journal, kind, "ni", "你"));
        assert!(!journal.exists(), "a lookup created the journal");

        record_user_insert(&journal, kind, "ni", "你", 10_000, "").unwrap();
        // A ranking change keeps the row the user's own.
        upsert(&journal, kind, "ni", "你", 20_000, "").unwrap();
        assert!(is_user_inserted(&journal, kind, "ni", "你"));
        assert_eq!(
            query_i64(
                &journal,
                "SELECT weight FROM user_dictionary_operations WHERE key='ni'"
            ),
            Some(20_000)
        );

        tombstone(&journal, kind, "ni", "你");
        assert!(is_user_deleted(&journal, kind, "ni", "你"));
        assert!(
            is_user_inserted(&journal, kind, "ni", "你"),
            "a tombstone forgot whose word it was"
        );
        assert_eq!(
            query_i64(
                &journal,
                "SELECT weight FROM user_dictionary_operations WHERE key='ni'"
            ),
            Some(0)
        );
        // Stored again, the tombstone is gone.
        upsert(&journal, kind, "ni", "你", 5, "").unwrap();
        assert!(!is_user_deleted(&journal, kind, "ni", "你"));
    }

    #[test]
    fn dropping_inside_a_transaction_rolls_back() {
        let dir = Dir::new();
        let journal_path = dir.journal();
        {
            let journal = open_journal(&journal_path).unwrap();
            journal.execute_batch("BEGIN IMMEDIATE").unwrap();
            write_upsert(&journal, PersonalDictionaryKind::Pinyin, "ni", "你", 1, "").unwrap();
        }
        let journal = open_journal(&journal_path).unwrap();
        assert!(journal.is_autocommit());
        assert_eq!(
            journal
                .query_row(
                    "SELECT count(*) FROM user_dictionary_operations",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }

    #[cfg(not(target_os = "ios"))]
    fn wal_length(path: &Path) -> u64 {
        let mut wal = path.as_os_str().to_owned();
        wal.push("-wal");
        std::fs::metadata(PathBuf::from(wal)).map_or(0, |metadata| metadata.len())
    }

    /// 日志按 WAL、`synchronous=NORMAL` 提交：写入追加到 `-wal`，另开的只读连接照样读到刚提交的学习。
    #[cfg(not(target_os = "ios"))]
    #[test]
    fn the_journal_commits_through_a_write_ahead_log() {
        let dir = Dir::new();
        let path = dir.journal();
        let journal = open_journal(&path).unwrap();
        let mode: String = journal
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        let synchronous: i64 = journal
            .query_row("PRAGMA synchronous", [], |row| row.get(0))
            .unwrap();
        assert_eq!(synchronous, 1, "synchronous=NORMAL");
        let size_limit: i64 = journal
            .query_row("PRAGMA journal_size_limit", [], |row| row.get(0))
            .unwrap();
        assert_eq!(size_limit, 4 * 1024 * 1024);
        write_upsert(&journal, PersonalDictionaryKind::Pinyin, "ni", "你", 1, "").unwrap();
        assert!(wal_length(&path) > 0);
        let reader = open_existing_read_only(&path).unwrap().unwrap();
        assert_eq!(
            reader
                .query_row(
                    "SELECT count(*) FROM user_dictionary_operations",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
    }

    /// 整体挪走日志主文件之前先检查点：别的连接还开着、提交还在 `-wal` 里时，单独拷走的主文件也带着它，`-wal` 只剩空文件。
    #[cfg(not(target_os = "ios"))]
    #[test]
    fn a_checkpoint_folds_the_wal_into_the_journal_file() {
        let dir = Dir::new();
        let path = dir.journal();
        let journal = open_journal(&path).unwrap();
        write_upsert(&journal, PersonalDictionaryKind::Pinyin, "ni", "你", 1, "").unwrap();
        assert!(wal_length(&path) > 0);
        checkpoint_journal(&path);
        assert_eq!(wal_length(&path), 0);
        let copy = dir.root.path().join("copy.db");
        std::fs::copy(&path, &copy).unwrap();
        assert_eq!(
            count(&copy, "SELECT count(*) FROM user_dictionary_operations"),
            1
        );
        drop(journal);
    }

    /// Two opens of `path` that saw the same cache generation. The generation is process-global and other tests call `close_cached_journals` concurrently, so a pair split by a bump is retried rather than read as the cache failing to reuse.
    fn open_twice_in_one_generation(path: &Path) -> (JournalConnection, JournalConnection) {
        loop {
            let generation = CACHE_GENERATION.load(Ordering::Acquire);
            let first = open_journal(path).unwrap();
            let second = open_journal(path).unwrap();
            if CACHE_GENERATION.load(Ordering::Acquire) == generation {
                return (first, second);
            }
        }
    }

    /// Marks the connection behind `journal` so a later handle can tell whether it got the same one: temporary tables belong to one connection.
    fn mark(journal: &Connection) {
        journal
            .execute_batch("CREATE TEMP TABLE IF NOT EXISTS reuse_marker(x)")
            .unwrap();
    }

    fn marked(journal: &Connection) -> bool {
        journal
            .query_row(
                "SELECT count(*) FROM temp.sqlite_master WHERE name='reuse_marker'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
            == 1
    }

    #[test]
    fn the_cache_reuses_one_connection_per_thread_until_closed() {
        let dir = Dir::new();
        let path = dir.journal();
        let (first, second) = open_twice_in_one_generation(&path);
        mark(&first);
        assert!(marked(&second));
        drop((first, second));
        let other = Dir::new();
        let switched = open_journal(&other.journal()).unwrap();
        assert!(!marked(&switched));
        drop(switched);
        // Parked when the last handle drops and taken back by the next open, unless another test closed every journal in between.
        loop {
            let generation = CACHE_GENERATION.load(Ordering::Acquire);
            let (reopened, again) = open_twice_in_one_generation(&path);
            mark(&reopened);
            drop((reopened, again));
            let taken_back = open_journal(&path).unwrap();
            if CACHE_GENERATION.load(Ordering::Acquire) != generation {
                continue;
            }
            assert!(marked(&taken_back));
            break;
        }
        CACHE_GENERATION.fetch_add(1, Ordering::AcqRel);
        let fresh = open_journal(&path).unwrap();
        assert!(!marked(&fresh));
    }

    #[test]
    fn closing_leaves_no_journal_open_on_this_thread() {
        let dir = Dir::new();
        let journal = dir.journal();
        // A queued transition makes the close write through the journal before it lets go.
        super::super::ngram_store::PersonalNgramStore::for_journal(&journal)
            .record(&[crate::lattice::personal::PersonalTransition {
                earlier: String::new(),
                previous: String::new(),
                word: "你".to_owned(),
                times: 2,
            }])
            .unwrap();
        close_cached_journals();
        assert!(!thread_holds_journal());
        assert_eq!(count(&journal, "SELECT count(*) FROM personal_bigram"), 1);
    }

    /// A thread that used the journal and went idle, still alive, as a host's input thread is between keystrokes. Other tests close every journal at any moment, so the thread is started again until its connection is seen parked.
    fn idle_thread_with_journal(
        path: &Path,
    ) -> (std::sync::mpsc::Sender<()>, std::thread::JoinHandle<bool>) {
        loop {
            let (opened_tx, opened_rx) = std::sync::mpsc::channel();
            let (finish_tx, finish_rx) = std::sync::mpsc::channel::<()>();
            let owned = path.to_path_buf();
            let thread = std::thread::spawn(move || {
                drop(open_journal(&owned).unwrap());
                opened_tx.send(()).unwrap();
                finish_rx.recv().unwrap();
                thread_holds_journal()
            });
            opened_rx.recv().unwrap();
            if parked_journals(path) == 1 {
                return (finish_tx, thread);
            }
            finish_tx.send(()).unwrap();
            thread.join().unwrap();
        }
    }

    /// The Windows hazard of a reset: an idle thread's journal handle blocks the rename. Closing reaches it from the calling thread instead of waiting for that thread's next journal access.
    #[test]
    fn closing_closes_the_idle_connections_of_other_threads() {
        let dir = Dir::new();
        let journal = dir.journal();
        let (finish, thread) = idle_thread_with_journal(&journal);
        close_cached_journals();
        assert_eq!(parked_journals(&journal), 0);
        finish.send(()).unwrap();
        assert!(
            !thread.join().unwrap(),
            "the idle thread kept its connection"
        );
    }

    #[test]
    fn a_connection_in_use_while_closing_closes_when_its_operation_ends() {
        let dir = Dir::new();
        let journal = dir.journal();
        let (held_tx, held_rx) = std::sync::mpsc::channel();
        let (closed_tx, closed_rx) = std::sync::mpsc::channel::<()>();
        let path = journal.clone();
        let thread = std::thread::spawn(move || {
            let held = open_journal(&path).unwrap();
            held_tx.send(()).unwrap();
            closed_rx.recv().unwrap();
            // The operation still works on the connection it holds.
            let one: i64 = held.query_row("SELECT 1", [], |row| row.get(0)).unwrap();
            assert_eq!(one, 1);
            drop(held);
            thread_holds_journal()
        });
        held_rx.recv().unwrap();
        close_cached_journals();
        closed_tx.send(()).unwrap();
        assert!(
            !thread.join().unwrap(),
            "the connection was parked after the close"
        );
        assert_eq!(parked_journals(&journal), 0);
    }

    /// Sessions on one journal: ending one keeps the other threads' connections for the sessions still open; ending the last closes them.
    #[test]
    fn the_last_use_of_a_journal_closes_every_threads_idle_connection() {
        let dir = Dir::new();
        let journal = dir.journal();
        loop {
            let mut first = JournalUse::new(journal.clone());
            let mut second = JournalUse::new(journal.clone());
            let (finish, thread) = idle_thread_with_journal(&journal);
            let generation = CACHE_GENERATION.load(Ordering::Acquire);
            first.end();
            // Ending twice gives up one claim, not two.
            first.end();
            let kept = parked_journals(&journal);
            second.end();
            let closed = parked_journals(&journal);
            finish.send(()).unwrap();
            let held = thread.join().unwrap();
            if CACHE_GENERATION.load(Ordering::Acquire) != generation {
                // Another test closed every journal meanwhile; what was kept says nothing.
                continue;
            }
            assert_eq!(
                kept, 1,
                "ending one of two sessions closed the other thread's connection"
            );
            assert_eq!(closed, 0);
            assert!(!held);
            break;
        }
    }

    /// 模拟用户机器上发现的形状：除最后两页仍是有效索引页外，其余页面全部为零。
    fn write_zeroed_journal(path: &Path) {
        let mut bytes = vec![0u8; 23 * 4096];
        bytes[21 * 4096] = 0x0a;
        bytes[21 * 4096 + 4] = 0x0a;
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn a_journal_that_is_not_a_database_is_moved_aside_with_its_sidecars() {
        let dir = Dir::new();
        let journal = dir.journal();
        write_zeroed_journal(&journal);
        let wal = dir.root.path().join("msime_user.db-wal");
        std::fs::write(&wal, b"stale wal").unwrap();

        let moved = quarantine_corrupt_journal(&journal).unwrap().unwrap();
        assert!(!journal.exists());
        assert!(!wal.exists());
        let name = moved.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("msime_user.db.corrupt-"), "{name}");
        assert_eq!(std::fs::metadata(&moved).unwrap().len(), 23 * 4096);
        assert_eq!(
            std::fs::read(moved.with_file_name(format!("{name}-wal"))).unwrap(),
            b"stale wal"
        );
        // 下一次启动找不到旧日志时会创建可用的新日志。
        assert!(upsert(&journal, PersonalDictionaryKind::Pinyin, "ni", "你", 1, "").is_ok());
        close_cached_journals();
        // 同一秒再次出现损坏日志时也不会覆盖第一次挪开的文件。
        write_zeroed_journal(&journal);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let occupied_sidecar = dir
            .root
            .path()
            .join(format!("msime_user.db.corrupt-{stamp}-wal"));
        std::fs::write(&occupied_sidecar, b"occupied target").unwrap();
        let again = quarantine_corrupt_journal(&journal).unwrap().unwrap();
        assert_ne!(again, moved);
        assert_eq!(
            std::fs::read(&occupied_sidecar).unwrap(),
            b"occupied target"
        );
        assert!(moved.exists() && again.exists());
    }

    #[test]
    fn a_readable_missing_or_empty_journal_is_left_alone() {
        let dir = Dir::new();
        let journal = dir.journal();
        assert_eq!(quarantine_corrupt_journal(&journal).unwrap(), None);
        std::fs::write(&journal, b"").unwrap();
        assert_eq!(quarantine_corrupt_journal(&journal).unwrap(), None);
        assert!(journal.exists());
        std::fs::remove_file(&journal).unwrap();
        upsert(&journal, PersonalDictionaryKind::Pinyin, "ni", "你", 1, "").unwrap();
        close_cached_journals();
        assert_eq!(quarantine_corrupt_journal(&journal).unwrap(), None);
        assert_eq!(
            count(&journal, "SELECT count(*) FROM user_dictionary_operations"),
            1
        );
    }

    #[test]
    fn a_journal_that_is_a_directory_fails() {
        let dir = Dir::new();
        let journal = dir.journal();
        std::fs::create_dir(&journal).unwrap();
        assert!(upsert(&journal, PersonalDictionaryKind::Pinyin, "ni", "你", 1, "").is_err());
        assert!(ensure_user_database(&journal).is_err());
    }
}
