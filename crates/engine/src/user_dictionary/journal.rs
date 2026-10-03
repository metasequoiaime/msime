//! Journal connections, schema and the row primitives every writer builds on (user-dictionary.md §2-§4).
//!
//! Connections are kept per thread and per path, because `apply_fixed_positions` reads the journal while every candidate list is built and an open plus schema pass per keystroke dominated that build in the reference (J:402-412). A connection has one transaction state, so it is never shared across threads. A thread keeps one journal open at a time: switching to another path releases the previous connection first, so a thread never holds two journal files open (J:459-461). `close_cached_journals` drops them all (by generation) before a directory is replaced.

use std::cell::RefCell;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
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
CREATE INDEX IF NOT EXISTS personal_trigram_word ON personal_trigram(word);";

/// J:96-102. A ranking change on a user word keeps `user_inserted = 1`, so the conflict branch leaves that column alone.
pub(crate) const UPSERT_JOURNAL_SQL: &str = "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display) VALUES(?1,?2,?3,'upsert',?4,?5) ON CONFLICT(dictionary,key,value) DO UPDATE SET operation='upsert',weight=excluded.weight, display=excluded.display,updated_at=unixepoch()";

/// J:547-559.
const USER_INSERT_SQL: &str = "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted) VALUES(?1,?2,?3,'upsert',?4,?5,1) ON CONFLICT(dictionary,key,value) DO UPDATE SET operation='upsert',weight=excluded.weight, display=excluded.display,user_inserted=1,updated_at=unixepoch()";

/// J:583-596, run against `schema` (`main` on the journal connection, the attach alias on a dictionary connection). `user_inserted` is preserved on conflict: removing a user's own word still records that it was theirs.
pub(crate) fn tombstone_sql(schema: &str) -> String {
    format!("INSERT INTO {schema}.user_dictionary_operations(dictionary,key,value,operation) VALUES(?1,?2,?3,'delete') ON CONFLICT(dictionary,key,value) DO UPDATE SET operation='delete',weight=0,display='', updated_at=unixepoch()")
}

/// Bumped by `close_cached_journals`; every thread drops its cached connection the next time it sees a newer value.
static CACHE_GENERATION: AtomicU64 = AtomicU64::new(0);

struct CachedJournal {
    path: PathBuf,
    generation: u64,
    connection: Rc<Connection>,
}

thread_local! {
    static CACHED_JOURNAL: RefCell<Option<CachedJournal>> = const { RefCell::new(None) };
}

/// A journal connection with the v4 schema applied. Dropping it inside an open transaction rolls the transaction back, so an early return can never leave the cached connection holding the write lock (J:478-486).
///
/// The connection is shared with every other `JournalConnection` of this thread and path, so a caller must not open the journal again while it has a transaction open: the inner handle's drop would end the outer transaction.
pub struct JournalConnection {
    connection: Rc<Connection>,
}

impl Deref for JournalConnection {
    type Target = Connection;

    fn deref(&self) -> &Connection {
        &self.connection
    }
}

impl Drop for JournalConnection {
    fn drop(&mut self) {
        if !self.connection.is_autocommit() {
            // A cached connection outlives the call, so an abandoned transaction would keep its write lock and fail every later operation. A failed ROLLBACK leaves nothing further to undo here: the connection is either already out of the transaction or unusable.
            let _ = self.connection.execute_batch("ROLLBACK");
        }
    }
}

/// `sqlite3_open_v2` with the reference's flags plus the 5 s busy timeout every engine connection uses (J:65-76). Without CREATE a missing file stays missing.
pub(crate) fn open_database(path: &Path, flags: OpenFlags) -> Result<Connection> {
    reject_database_parent(path)?;
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        if !metadata.file_type().is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "database path is not a regular file",
            )
            .into());
        }
    }
    let connection = Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_FULL_MUTEX)?;
    connection.busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))?;
    Ok(connection)
}

fn reject_database_parent(path: &Path) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "database path has no parent directory",
        )
    })?;
    let mut current = PathBuf::new();
    for component in parent.components() {
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                if !crate::paths::is_trusted_system_alias(&current) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "database path has a symbolic-link parent",
                    ));
                }
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotADirectory,
                    "database path parent is not a directory",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// A dictionary (`msime.db`, `english.db`) opened for writing; a missing dictionary is an error, never a new empty file.
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

/// Open (creating) the journal, reusing this thread's connection for the path.
pub fn open_journal(path: &Path) -> Result<JournalConnection> {
    let generation = CACHE_GENERATION.load(Ordering::Acquire);
    let cached = CACHED_JOURNAL.try_with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(cached) = cache.as_ref() {
            if cached.path == path && cached.generation == generation {
                return Ok(JournalConnection {
                    connection: Rc::clone(&cached.connection),
                });
            }
        }
        // Released before opening, so a thread never holds two journal files open.
        *cache = None;
        let connection = open_database(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        ensure_schema(&connection)?;
        let connection = Rc::new(connection);
        *cache = Some(CachedJournal {
            path: path.to_path_buf(),
            generation,
            connection: Rc::clone(&connection),
        });
        Ok(JournalConnection { connection })
    });
    match cached {
        Ok(opened) => opened,
        // A session a host drops while its thread exits (sessions kept in a thread-local map) can outlive this thread's cache, which thread-local destruction order does not fix. Its last flush gets a connection of its own, closed when that flush ends.
        Err(_) => {
            let connection = open_database(
                path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
            )?;
            ensure_schema(&connection)?;
            Ok(JournalConnection {
                connection: Rc::new(connection),
            })
        }
    }
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

/// Invalidate every thread's cached journal connection, release the personal n-gram stores and the local-mode connections. Call before deleting or replacing a data directory.
pub fn close_cached_journals() {
    // First, because writing the stores' queues opens the journal through this thread's cache; the slot is cleared after it.
    super::ngram_store::release_all();
    CACHE_GENERATION.fetch_add(1, Ordering::AcqRel);
    // The calling thread's connection closes now (once no operation still holds it); other threads' connections close on their next journal access.
    CACHED_JOURNAL.with(|cache| *cache.borrow_mut() = None);
    crate::local::database::close_cached_local_databases();
}

/// Closes this thread's cached journal connection now. The reference cached only its default journal path and opened msime's journal per call (user_dictionary_journal.cpp:445-452), so a thread that is done with the journal must not keep a handle that blocks the reset's rename or a generation delete on Windows. A connection an operation still holds closes when that operation ends.
pub(crate) fn release_thread_journal() {
    // At thread exit the cache may already be destroyed, which closed its connection; there is nothing left to release then.
    let _ = CACHED_JOURNAL.try_with(|cache| *cache.borrow_mut() = None);
}

/// Whether this thread still caches a journal connection.
#[cfg(test)]
pub(crate) fn thread_holds_journal() -> bool {
    CACHED_JOURNAL.with(|cache| cache.borrow().is_some())
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
            self.root.path().join("msime.db")
        }

        pub fn english_db(&self) -> PathBuf {
            self.root.path().join("english.db")
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

    #[test]
    fn the_cache_reuses_one_connection_per_thread_until_closed() {
        let dir = Dir::new();
        let path = dir.journal();
        let (first, second) = open_twice_in_one_generation(&path);
        assert!(Rc::ptr_eq(&first.connection, &second.connection));
        drop((first, second));
        let other = Dir::new();
        let switched = open_journal(&other.journal()).unwrap();
        drop(switched);
        let (reopened, again) = open_twice_in_one_generation(&path);
        assert!(Rc::ptr_eq(&reopened.connection, &again.connection));
        drop(again);
        CACHE_GENERATION.fetch_add(1, Ordering::AcqRel);
        let fresh = open_journal(&path).unwrap();
        assert!(!Rc::ptr_eq(&reopened.connection, &fresh.connection));
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
        assert!(CACHED_JOURNAL.with(|cache| cache.borrow().is_none()));
        assert_eq!(count(&journal, "SELECT count(*) FROM personal_bigram"), 1);
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
