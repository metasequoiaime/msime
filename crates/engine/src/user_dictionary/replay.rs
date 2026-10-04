//! Replaying the journal into a fresh dictionary generation (user-dictionary.md §12): on install, upgrade and every generation switch, so learning survives a new dictionary release.
//!
//! The row writers here (`apply_pinyin`, `apply_simple`, `apply_english`) are also what the personal dictionary edit applies to the working copy, so an edit and its later replay can never disagree about what a journal row means.

use std::path::Path;

use rusqlite::types::{ToSqlOutput, ValueRef};
use rusqlite::{params, Connection, OpenFlags, Row, ToSql};

use crate::dictionary::english::ensure_english_schema;
use crate::format;
use crate::user_dictionary::journal::{open_database, BUSY_TIMEOUT_MS};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReplayResult {
    pub applied: i32,
    pub skipped: i32,
    pub failed: i32,
    /// Empty on success.
    pub error: String,
}

/// Apply every operation in `(updated_at, rowid)` order to `main_db` and `english_db` (J:1642-1735). Never panics or errors; failures are counted and described.
pub fn replay(user_db: &Path, main_db: &Path, english_db: &Path) -> ReplayResult {
    replay_with(user_db, Some(main_db), english_db)
}

/// 没有 `msime.db` 的代次（方案集合不读它，见 `SchemeSet::reads_main_dictionary`）：只把英文行回放进 `english_db`。拼音、五笔和快捷短语的行在这里没有能写的表，计入 `skipped` 而不是 `failed`，所以它们（例如从别的版本恢复来的快照）不会让这个代次被拒；行本身留在日志里不动。
pub fn replay_english(user_db: &Path, english_db: &Path) -> ReplayResult {
    replay_with(user_db, None, english_db)
}

/// 不带 `msime.db` 时代替它的连接：一个内存库，`english.db` 和日志照常 attach 上去，写进去的只有 attach 的那几个库。等待锁的时间与 `open_database` 打开的连接相同。
pub(super) fn open_without_main_dictionary() -> rusqlite::Result<Connection> {
    let connection = Connection::open_in_memory_with_flags(
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )?;
    connection.busy_timeout(std::time::Duration::from_millis(BUSY_TIMEOUT_MS))?;
    Ok(connection)
}

fn replay_with(user_db: &Path, main_db: Option<&Path>, english_db: &Path) -> ReplayResult {
    let mut result = ReplayResult::default();
    // A first install has no journal and nothing to replay; replay never creates one (J:1646-1647).
    if !user_db.exists() {
        return result;
    }
    let fail = |mut result: ReplayResult, error: &str| {
        result.error = error.to_owned();
        result
    };
    let Ok(journal) = open_database(user_db, OpenFlags::SQLITE_OPEN_READ_ONLY) else {
        return fail(result, "cannot open user dictionary database");
    };
    if ensure_english_schema(english_db).is_err() {
        return fail(result, "cannot migrate English dictionary database");
    }
    let main = match main_db {
        Some(main_db) => open_database(main_db, OpenFlags::SQLITE_OPEN_READ_WRITE).ok(),
        None => open_without_main_dictionary().ok(),
    };
    let Some(main) = main else {
        return fail(result, "cannot open target dictionary database");
    };
    if attach(&main, english_db, "replay_english").is_err() {
        return fail(result, "cannot attach English dictionary database");
    }
    let Ok(mut rows) = journal.prepare(
        "SELECT dictionary,key,value,operation,weight,display FROM user_dictionary_operations ORDER BY updated_at,rowid",
    ) else {
        return fail(result, "invalid user dictionary database");
    };
    // Plain statements rather than a rusqlite transaction: the reference reports whether its ROLLBACK itself succeeded, which a dropped `Transaction` cannot tell.
    if main.execute_batch("BEGIN IMMEDIATE").is_err() {
        return fail(result, "cannot start replay transaction");
    }
    let read_complete = (|| -> rusqlite::Result<()> {
        let mut cursor = rows.query([])?;
        while let Some(row) = cursor.next()? {
            let operation = JournalRow::read(row)?;
            if main_db.is_none() && operation.kind != b"english" {
                result.skipped += 1;
                continue;
            }
            if operation.kind == b"pinyin"
                && operation.operation == b"upsert"
                && operation.weight < 1
            {
                // Rows the old rebalance staircase left below the floor would bury shipped frequencies (J:1695-1699).
                result.skipped += 1;
                continue;
            }
            // A failed row is counted, not fatal here: the whole replay is rolled back below once any row failed (J:1700-1709).
            if matches!(operation.apply(&main), Ok(true)) {
                result.applied += 1;
            } else {
                result.failed += 1;
            }
        }
        Ok(())
    })();
    if read_complete.is_err() {
        result.error = if main.execute_batch("ROLLBACK").is_ok() {
            "cannot read the complete user dictionary journal; changes were rolled back"
        } else {
            "cannot read the complete user dictionary journal and rollback also failed"
        }
        .to_owned();
        return result;
    }
    if result.failed == 0 {
        if main.execute_batch("COMMIT").is_err() {
            result.error = if main.execute_batch("ROLLBACK").is_ok() {
                "cannot commit replay transaction; changes were rolled back"
            } else {
                "cannot commit or roll back replay transaction"
            }
            .to_owned();
        }
    } else {
        result.error = if main.execute_batch("ROLLBACK").is_ok() {
            "one or more operations failed; changes were rolled back"
        } else {
            "one or more operations failed and rollback also failed"
        }
        .to_owned();
    }
    result
}

/// One `user_dictionary_operations` row as replay reads it. Text columns are raw bytes, as `sqlite3_column_text` gave them to the reference (J:1688-1694), and are written back as the same bytes: a journal an older build wrote with text that is not UTF-8 replays exactly as it did there, rather than rolling back every generation built from it. A NULL, which the reference could not read either, still ends the read as a cursor failure.
struct JournalRow {
    kind: Vec<u8>,
    key: Vec<u8>,
    value: Vec<u8>,
    operation: Vec<u8>,
    weight: i64,
    display: Vec<u8>,
}

impl JournalRow {
    fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        let text = |index: usize| -> rusqlite::Result<Vec<u8>> {
            Ok(row.get_ref(index)?.as_bytes()?.to_vec())
        };
        Ok(Self {
            kind: text(0)?,
            key: text(1)?,
            value: text(2)?,
            operation: text(3)?,
            weight: row.get(4)?,
            display: text(5)?,
        })
    }

    /// `Ok(false)` for a row that cannot be stored: an unknown dictionary or a malformed pinyin key.
    fn apply(&self, main: &Connection) -> rusqlite::Result<bool> {
        let delete = self.operation == b"delete";
        match self.kind.as_slice() {
            b"pinyin" => apply_pinyin(main, &self.key, &self.value, delete, self.weight),
            b"wubi" => apply_simple(main, "wubi86", &self.key, &self.value, delete, self.weight),
            b"wubi98" => apply_simple(main, "wubi98", &self.key, &self.value, delete, self.weight),
            b"quick" => apply_simple(
                main,
                "quick_parases",
                &self.key,
                &self.value,
                delete,
                self.weight,
            ),
            b"english" => apply_english(
                main,
                &self.key,
                &self.value,
                delete,
                self.weight,
                &self.display,
            ),
            _ => Ok(false),
        }
    }
}

/// Bytes bound as TEXT the way `sqlite3_bind_text` bound the reference's `std::string`, whatever their encoding; `&[u8]` alone would bind a BLOB, which never equals a stored text value.
struct Text<'a>(&'a [u8]);

impl ToSql for Text<'_> {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Borrowed(ValueRef::Text(self.0)))
    }
}

/// `ATTACH DATABASE ?1 AS <schema>`; `schema` is one of this module's fixed aliases, never user input.
pub(super) fn attach(connection: &Connection, path: &Path, schema: &str) -> rusqlite::Result<()> {
    let name = path
        .to_str()
        .ok_or_else(|| rusqlite::Error::InvalidPath(path.to_owned()))?;
    connection.execute(&format!("ATTACH DATABASE ?1 AS {schema}"), [name])?;
    Ok(())
}

/// Delete, or update and insert when no row changed, a pinyin row with its jianpin column (J:306-332). A delete that finds nothing still succeeds.
pub(super) fn apply_pinyin(
    main: &Connection,
    key: &[u8],
    value: &[u8],
    delete: bool,
    weight: i64,
) -> rusqlite::Result<bool> {
    // The reference's `pinyin_segments` on bytes (J:182-199): an empty segment means the key cannot be stored.
    let segments: Vec<&[u8]> = key.split(|&byte| byte == b'\'').collect();
    if segments.iter().any(|segment| segment.is_empty()) {
        return Ok(false);
    }
    // An initial outside `a..=z` names no table: the row cannot be stored.
    let Some(table) = format::quanpin_table(segments.len(), segments[0][0]) else {
        return Ok(false);
    };
    let (key, value) = (Text(key), Text(value));
    if delete {
        main.prepare_cached(&format!(
            "DELETE FROM \"{table}\" WHERE key=?1 AND value=?2"
        ))?
        .execute(params![key, value])?;
        return Ok(true);
    }
    // The first character of each syllable. The reference took the first byte, which is the same for every key a writer accepts (lowercase ASCII); a segment that does not start with valid UTF-8 contributes U+FFFD here rather than a stray byte, since `jp` is only ever matched against typed letters.
    let jianpin: String = segments
        .iter()
        .filter_map(|segment| String::from_utf8_lossy(segment).chars().next())
        .collect();
    let changed = main
        .prepare_cached(&format!(
            "UPDATE \"{table}\" SET jp=?1,weight=?2 WHERE key=?3 AND value=?4"
        ))?
        .execute(params![jianpin, weight, key, value])?;
    if changed > 0 {
        return Ok(true);
    }
    main.prepare_cached(&format!(
        "INSERT INTO \"{table}\"(key,jp,value,weight) VALUES(?1,?2,?3,?4)"
    ))?
    .execute(params![key, jianpin, value, weight])?;
    Ok(true)
}

/// The same for `wubi86` and `quick_parases`, which have no jianpin column (J:334-355).
pub(super) fn apply_simple(
    main: &Connection,
    table: &str,
    key: &[u8],
    value: &[u8],
    delete: bool,
    weight: i64,
) -> rusqlite::Result<bool> {
    let (key, value) = (Text(key), Text(value));
    if delete {
        main.prepare_cached(&format!(
            "DELETE FROM \"{table}\" WHERE \"key\"=?1 AND \"value\"=?2"
        ))?
        .execute(params![key, value])?;
        return Ok(true);
    }
    let changed = main
        .prepare_cached(&format!(
            "UPDATE \"{table}\" SET weight=?1 WHERE \"key\"=?2 AND \"value\"=?3"
        ))?
        .execute(params![weight, key, value])?;
    if changed > 0 {
        return Ok(true);
    }
    main.prepare_cached(&format!(
        "INSERT INTO \"{table}\"(\"key\",\"value\",weight) VALUES(?1,?2,?3)"
    ))?
    .execute(params![key, value, weight])?;
    Ok(true)
}

/// Upsert or delete in the English dictionary attached as `replay_english`, keyed by `(word, display)`; an empty journal display means the value is the display (J:357-371).
pub(super) fn apply_english(
    main: &Connection,
    key: &[u8],
    value: &[u8],
    delete: bool,
    weight: i64,
    display: &[u8],
) -> rusqlite::Result<bool> {
    let display = Text(if display.is_empty() { value } else { display });
    let key = Text(key);
    if delete {
        main.prepare_cached(
            "DELETE FROM replay_english.english_words WHERE word=?1 AND display=?2",
        )?
        .execute(params![key, display])?;
        return Ok(true);
    }
    main.prepare_cached(
        "INSERT INTO replay_english.english_words(word,display,weight) VALUES(?1,?2,?3) ON CONFLICT(word,display) DO UPDATE SET weight=excluded.weight",
    )?
    .execute(params![key, display, weight])?;
    Ok(true)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The v4 journal table as the reference creates it (data-formats.md §6.2), for fixtures that must not depend on the journal module.
    pub(crate) const OPERATIONS_DDL: &str = "CREATE TABLE IF NOT EXISTS user_dictionary_operations(dictionary TEXT NOT NULL,key TEXT NOT NULL,value TEXT NOT NULL,operation TEXT NOT NULL CHECK(operation IN ('upsert','delete')),weight INTEGER NOT NULL DEFAULT 0,display TEXT NOT NULL DEFAULT '',user_inserted INTEGER NOT NULL DEFAULT 0,updated_at INTEGER NOT NULL DEFAULT(unixepoch()),PRIMARY KEY(dictionary,key,value));";

    pub(crate) fn sql(path: &Path, statements: &str) {
        Connection::open(path)
            .unwrap()
            .execute_batch(statements)
            .unwrap();
    }

    pub(crate) fn weight(path: &Path, query: &str) -> Option<i64> {
        use rusqlite::OptionalExtension;
        Connection::open(path)
            .unwrap()
            .query_row(query, [], |row| row.get(0))
            .optional()
            .unwrap()
    }

    struct Fixture {
        _root: tempfile::TempDir,
        journal: PathBuf,
        main: PathBuf,
        english: PathBuf,
    }

    fn fixture() -> Fixture {
        let root = tempfile::tempdir().unwrap();
        let journal = root.path().join("msime_user.db");
        let main = root.path().join("msime-pinyin.db");
        let english = root.path().join("msime-english.db");
        sql(
            &main,
            "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
             INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',100),('ni''hao','xx','您好',100);
             CREATE TABLE tbl_7_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
             CREATE TABLE tbl_others_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
             CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
             CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER);
             CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);",
        );
        sql(&journal, OPERATIONS_DDL);
        Fixture {
            _root: root,
            journal,
            main,
            english,
        }
    }

    fn journal_row(fixture: &Fixture, row: &str) {
        sql(
            &fixture.journal,
            &format!("INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,display,updated_at) VALUES{row};"),
        );
    }

    #[test]
    fn wubi_and_wubi98_rows_land_in_their_own_tables() {
        let fixture = fixture();
        journal_row(&fixture, "('wubi','kl','号','upsert',11,'',1)");
        journal_row(&fixture, "('wubi98','kg','号','upsert',12,'',2)");
        replay(&fixture.journal, &fixture.main, &fixture.english);
        assert_eq!(
            weight(&fixture.main, "SELECT weight FROM wubi86 WHERE key='kl'"),
            Some(11)
        );
        assert_eq!(
            weight(&fixture.main, "SELECT weight FROM wubi98 WHERE key='kg'"),
            Some(12)
        );
        assert_eq!(
            weight(&fixture.main, "SELECT weight FROM wubi86 WHERE key='kg'"),
            None
        );
    }

    #[test]
    fn a_missing_journal_is_an_empty_success_and_is_not_created() {
        let fixture = fixture();
        std::fs::remove_file(&fixture.journal).unwrap();
        let result = replay(&fixture.journal, &fixture.main, &fixture.english);
        assert_eq!(result, ReplayResult::default());
        assert!(!fixture.journal.exists());
    }

    #[test]
    fn applies_every_kind_in_journal_order() {
        let fixture = fixture();
        journal_row(&fixture, "('pinyin','ni''hao','您好','upsert',500,'',2)");
        journal_row(&fixture, "('pinyin','ni''hao','拟好','upsert',80,'',3)");
        journal_row(
            &fixture,
            "('pinyin','ni''hao''ni''hao''ni''hao''ni','你好你好你好你','upsert',7,'',1)",
        );
        journal_row(
            &fixture,
            "('pinyin','ni''ni''ni''ni''ni''ni''ni''ni','你你你你你你你你','upsert',8,'',1)",
        );
        journal_row(&fixture, "('wubi','wq','你','upsert',9,'',1)");
        journal_row(&fixture, "('quick','dh','电话','upsert',10,'',1)");
        journal_row(
            &fixture,
            "('english','hello','Hello','upsert',300,'Hello',1)",
        );
        journal_row(&fixture, "('english','world','World','upsert',301,'',1)");
        journal_row(&fixture, "('pinyin','ni''hao','你好','delete',0,'',4)");
        journal_row(&fixture, "('pinyin','ni''hao','泥好','upsert',0,'',5)");

        let result = replay(&fixture.journal, &fixture.main, &fixture.english);
        assert_eq!(
            result,
            ReplayResult {
                applied: 9,
                skipped: 1,
                failed: 0,
                error: String::new()
            }
        );
        assert_eq!(
            weight(
                &fixture.main,
                "SELECT count(*) FROM tbl_2_n WHERE value='你好'"
            ),
            Some(0)
        );
        // An existing row is updated in place, jianpin included.
        assert_eq!(
            weight(
                &fixture.main,
                "SELECT weight FROM tbl_2_n WHERE value='您好' AND jp='nh'"
            ),
            Some(500)
        );
        assert_eq!(
            weight(
                &fixture.main,
                "SELECT weight FROM tbl_2_n WHERE value='拟好' AND jp='nh'"
            ),
            Some(80)
        );
        assert_eq!(
            weight(
                &fixture.main,
                "SELECT count(*) FROM tbl_2_n WHERE value='泥好'"
            ),
            Some(0)
        );
        assert_eq!(
            weight(
                &fixture.main,
                "SELECT weight FROM tbl_7_n WHERE jp='nhnhnhn'"
            ),
            Some(7)
        );
        assert_eq!(
            weight(
                &fixture.main,
                "SELECT weight FROM tbl_others_n WHERE jp='nnnnnnnn'"
            ),
            Some(8)
        );
        assert_eq!(
            weight(&fixture.main, "SELECT weight FROM wubi86 WHERE key='wq'"),
            Some(9)
        );
        assert_eq!(
            weight(
                &fixture.main,
                "SELECT weight FROM quick_parases WHERE key='dh'"
            ),
            Some(10)
        );
        assert_eq!(
            weight(
                &fixture.english,
                "SELECT weight FROM english_words WHERE word='hello' AND display='Hello'"
            ),
            Some(300)
        );
        assert_eq!(
            weight(
                &fixture.english,
                "SELECT weight FROM english_words WHERE word='world' AND display='World'"
            ),
            Some(301)
        );

        // Replaying again changes nothing: upserts update in place instead of duplicating rows.
        let again = replay(&fixture.journal, &fixture.main, &fixture.english);
        assert!(again.error.is_empty() && again.failed == 0);
        assert_eq!(
            weight(&fixture.main, "SELECT count(*) FROM tbl_2_n"),
            Some(2)
        );
    }

    #[test]
    fn a_failed_row_rolls_the_whole_replay_back() {
        let fixture = fixture();
        journal_row(&fixture, "('pinyin','ni''hao','您好','upsert',500,'',1)");
        // `@` has no table, an empty segment cannot be stored, and an unknown dictionary is not applied (runtime_isolation `@` fixture).
        journal_row(&fixture, "('pinyin','@','无效词','upsert',10,'',2)");
        journal_row(&fixture, "('pinyin','ni''''hao','无效词','upsert',10,'',3)");
        journal_row(&fixture, "('japanese','ni','无效词','upsert',10,'',4)");
        let result = replay(&fixture.journal, &fixture.main, &fixture.english);
        assert_eq!(result.applied, 1);
        assert_eq!(result.failed, 3);
        assert_eq!(
            result.error,
            "one or more operations failed; changes were rolled back"
        );
        assert_eq!(
            weight(
                &fixture.main,
                "SELECT weight FROM tbl_2_n WHERE value='您好'"
            ),
            Some(100)
        );
    }

    /// user_dictionary_journal.cpp:1688-1708 copies each column's bytes with no encoding check and binds them back as text, so a legacy row whose text is not UTF-8 replays byte for byte instead of blocking every future generation.
    #[test]
    fn a_journal_row_that_is_not_utf8_replays_byte_for_byte() {
        let fixture = fixture();
        journal_row(&fixture, "('pinyin','ni''hao','您好','upsert',500,'',1)");
        journal_row(
            &fixture,
            "('pinyin','ni''hao',CAST(x'ff' AS TEXT),'upsert',10,'',2)",
        );
        journal_row(
            &fixture,
            "('quick',CAST(x'6bff' AS TEXT),'phrase','upsert',5,'',3)",
        );
        journal_row(
            &fixture,
            "('english','hello','hello','upsert',7,CAST(x'68ff' AS TEXT),4)",
        );
        let result = replay(&fixture.journal, &fixture.main, &fixture.english);
        assert_eq!(result.error, "");
        assert_eq!(result.applied, 4);
        assert_eq!(
            weight(
                &fixture.main,
                "SELECT weight FROM tbl_2_n WHERE value='您好'"
            ),
            Some(500)
        );
        assert_eq!(
            weight(
                &fixture.main,
                "SELECT weight FROM tbl_2_n WHERE key='ni''hao' AND hex(value)='FF' AND jp='nh' AND typeof(value)='text'"
            ),
            Some(10)
        );
        assert_eq!(
            weight(
                &fixture.main,
                "SELECT weight FROM quick_parases WHERE hex(key)='6BFF' AND value='phrase' AND typeof(key)='text'"
            ),
            Some(5)
        );
        assert_eq!(
            weight(
                &fixture.english,
                "SELECT weight FROM english_words WHERE word='hello' AND hex(display)='68FF'"
            ),
            Some(7)
        );
    }

    /// A key that names no table fails in the reference too (pinyin_table yields no name, so the statement does not prepare), which rolls the whole replay back.
    #[test]
    fn a_pinyin_key_that_is_not_utf8_and_names_no_table_rolls_back() {
        let fixture = fixture();
        journal_row(&fixture, "('pinyin','ni''hao','您好','upsert',500,'',1)");
        journal_row(
            &fixture,
            "('pinyin',CAST(x'ff' AS TEXT),'你','upsert',10,'',2)",
        );
        let result = replay(&fixture.journal, &fixture.main, &fixture.english);
        assert_eq!(result.failed, 1);
        assert_eq!(
            result.error,
            "one or more operations failed; changes were rolled back"
        );
        assert_eq!(
            weight(
                &fixture.main,
                "SELECT weight FROM tbl_2_n WHERE value='您好'"
            ),
            Some(100)
        );
    }

    // test_engine_smoke.cpp:317-403: an unreadable journal, a missing target and a locked target are all reported.
    #[test]
    fn open_and_cursor_failures_are_reported() {
        let fixture = fixture();
        let unreadable = fixture.journal.with_file_name("msime_user.unreadable.db");
        sql(
            &unreadable,
            "CREATE VIEW user_dictionary_operations AS SELECT 'pinyin' AS dictionary,'ni' AS key,'你' AS value,'upsert' AS operation,100 AS weight,'' AS display,1 AS updated_at UNION ALL SELECT 'pinyin','bad',value,'upsert',1,'',2 FROM json_each('not-json')",
        );
        assert!(!replay(&unreadable, &fixture.main, &fixture.english)
            .error
            .is_empty());

        let not_a_database = fixture.journal.with_file_name("garbage.db");
        std::fs::write(&not_a_database, b"synthetic invalid database").unwrap();
        assert_eq!(
            replay(&not_a_database, &fixture.main, &fixture.english).error,
            "invalid user dictionary database"
        );

        let missing_main = fixture.main.with_file_name("missing.db");
        assert_eq!(
            replay(&fixture.journal, &missing_main, &fixture.english).error,
            "cannot open target dictionary database"
        );
        assert!(!missing_main.exists());
    }

    #[test]
    fn a_locked_target_is_reported_after_the_busy_timeout() {
        let fixture = fixture();
        let locked = Connection::open(&fixture.main).unwrap();
        locked.execute_batch("BEGIN EXCLUSIVE").unwrap();
        let result = replay(&fixture.journal, &fixture.main, &fixture.english);
        locked.execute_batch("ROLLBACK").unwrap();
        // The exclusive lock already blocks the English attach, which reads the main schema; either way the replay must not report success.
        assert!(!result.error.is_empty());
        assert_eq!(result.applied, 0);
    }

    /// 没有 `msime.db` 的代次只回放英文行：拼音、五笔和快捷短语的行跳过，不算失败，也不会去碰（或创建）`msime.db`。
    #[test]
    fn without_the_main_dictionary_only_english_rows_are_replayed() {
        let fixture = fixture();
        std::fs::remove_file(&fixture.main).unwrap();
        journal_row(&fixture, "('pinyin','ni''hao','您好','upsert',500,'',1)");
        journal_row(&fixture, "('wubi','wq','你','upsert',9,'',2)");
        journal_row(&fixture, "('quick','dh','电话','upsert',10,'',3)");
        journal_row(
            &fixture,
            "('english','hello','Hello','upsert',300,'Hello',4)",
        );
        journal_row(&fixture, "('english','world','World','upsert',301,'',5)");

        let result = replay_english(&fixture.journal, &fixture.english);
        assert_eq!(
            result,
            ReplayResult {
                applied: 2,
                skipped: 3,
                failed: 0,
                error: String::new(),
            }
        );
        assert!(!fixture.main.exists());
        assert_eq!(
            weight(
                &fixture.english,
                "SELECT weight FROM english_words WHERE word='hello' AND display='Hello'"
            ),
            Some(300)
        );
        assert_eq!(
            weight(
                &fixture.english,
                "SELECT weight FROM english_words WHERE word='world' AND display='World'"
            ),
            Some(301)
        );
    }
}
