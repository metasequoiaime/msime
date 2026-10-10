//! `hanzi_to_pinyin`, the reading lookup the dictionary import uses (bridge.cpp:239-305, 735-776). Bridge-local logic in the C++; it reads the working `msime-pinyin.db` directly.

use std::collections::HashMap;
use std::fmt::Write;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use lru::LruCache;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};

use super::column_text;

/// The longest text the bridge looks up (bridge.cpp:750).
const MAXIMUM_CHARACTERS: usize = 128;
const HANZI_CACHE_CAPACITY: usize = 8;

type HanziReadings = HashMap<String, String>;

static HANZI_CACHE: OnceLock<Mutex<LruCache<PathBuf, Arc<HanziReadings>>>> = OnceLock::new();

/// The highest-weighted key whose value is exactly `text`, over every `tbl_<len>_<c>`; failing that, each character's first key from the single-character tables joined with `'`. Empty unless `text` is 1..=128 Han characters and every one has a reading. Never errors. The per-character map is built once per database path for the process.
pub fn hanzi_to_pinyin(main_db: &Path, text: &str) -> String {
    let length = text.chars().count();
    if length == 0 || length > MAXIMUM_CHARACTERS || !text.chars().all(is_bridge_han) {
        return String::new();
    }
    let Some(connection) = open(main_db) else {
        return String::new();
    };
    let exact = exact_hanzi_pinyin(&connection, text, length);
    if !exact.is_empty() {
        return exact;
    }
    let singles = cached_single_hanzi_map(&connection, main_db);
    let mut readings = Vec::with_capacity(length);
    let mut buffer = [0; 4];
    for character in text.chars() {
        let Some(reading) = singles.get(character.encode_utf8(&mut buffer) as &str) else {
            return String::new();
        };
        readings.push(reading.as_str());
    }
    readings.join("'")
}

/// The bridge's own Han ranges (bridge.cpp:143-148). Narrower than `text::is_han`: no U+3007 and nothing past U+2FA1F, and the import path keeps rejecting those.
fn is_bridge_han(character: char) -> bool {
    matches!(character as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F)
}

fn open(path: &Path) -> Option<Connection> {
    // An empty path is a missing file; SQLite would open a private temporary database for it.
    if path.as_os_str().is_empty() {
        return None;
    }
    let path = crate::paths::sqlite_read_only_path_no_follow(path).ok()?;
    let connection = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NOFOLLOW
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )
    .ok()?;
    // bridge.cpp:749-753 set no busy timeout, so SQLite's default of none applied: a locked dictionary answers empty at once rather than stalling each word of a validation batch for rusqlite's default 5 s.
    connection.busy_timeout(Duration::ZERO).ok()?;
    Some(connection)
}

/// bridge.cpp:285-305. The table is `tbl_<len>_<c>` written out for every letter, not `quanpin_table`, so past seven characters no table exists and the per-character path answers; tables that fail to prepare (`i`, `u`, `v`) are skipped.
fn exact_hanzi_pinyin(connection: &Connection, word: &str, length: usize) -> String {
    for initial in b'a'..=b'z' {
        let sql = exact_hanzi_sql(length, initial);
        let Ok(mut statement) = connection.prepare_cached(&sql) else {
            continue;
        };
        let Ok(mut rows) = statement.query((word,)) else {
            continue;
        };
        if let Ok(Some(row)) = rows.next() {
            // A NULL key falls through to the next table (bridge.cpp:296-301).
            if !matches!(row.get_ref(0), Ok(ValueRef::Null)) {
                if let Ok(key) = column_text(row, 0) {
                    return key;
                }
            }
        }
    }
    String::new()
}

fn exact_hanzi_sql(length: usize, initial: u8) -> String {
    const PREFIX: &str = "SELECT \"key\" FROM \"tbl_";
    const SUFFIX: &str = "\" WHERE \"value\"=?1 ORDER BY \"weight\" DESC, \"key\" ASC LIMIT 1";
    let mut digits = 1;
    let mut value = length;
    while value >= 10 {
        digits += 1;
        value /= 10;
    }
    let mut sql = String::with_capacity(PREFIX.len() + digits + 2 + SUFFIX.len());
    sql.push_str(PREFIX);
    write!(&mut sql, "{length}").expect("writing to a String cannot fail");
    sql.push('_');
    sql.push(initial as char);
    sql.push_str(SUFFIX);
    sql
}

/// Walks all single-character tables, heaviest first, keeping the first key per character (bridge.cpp:239-256). The flag is false when a table could not be read because the file was busy, so the partial map is used once but not cached.
fn single_hanzi_map(connection: &Connection) -> (HanziReadings, bool) {
    let busy = |error: &rusqlite::Error| {
        matches!(
            error.sqlite_error_code(),
            Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
        )
    };
    let mut complete = true;
    let mut result = HanziReadings::new();
    for initial in b'a'..=b'z' {
        let sql = single_hanzi_sql(initial);
        let mut statement = match connection.prepare_cached(&sql) {
            Ok(statement) => statement,
            Err(error) => {
                complete &= !busy(&error);
                continue;
            }
        };
        let mut rows = match statement.query(()) {
            Ok(rows) => rows,
            Err(error) => {
                complete &= !busy(&error);
                continue;
            }
        };
        while let Ok(Some(row)) = rows.next() {
            let null = |index| matches!(row.get_ref(index), Ok(ValueRef::Null));
            if null(0) || null(1) {
                continue;
            }
            let (Ok(key), Ok(value)) = (column_text(row, 0), column_text(row, 1)) else {
                break;
            };
            result.entry(value).or_insert(key);
        }
    }
    (result, complete)
}

fn single_hanzi_sql(initial: u8) -> String {
    const PREFIX: &str = "SELECT \"key\", \"value\" FROM \"tbl_1_";
    const SUFFIX: &str = "\" ORDER BY \"weight\" DESC, \"key\" ASC";
    let mut sql = String::with_capacity(PREFIX.len() + 1 + SUFFIX.len());
    sql.push_str(PREFIX);
    sql.push(initial as char);
    sql.push_str(SUFFIX);
    sql
}

/// The single-character scan walks about twenty thousand rows with no index to sort by, and the personal dictionary validation calls this once per word for up to a thousand words, so the map is built once per dictionary path (bridge.cpp:257-284). Keyed by path because two sessions may point at different dictionaries. It is built outside the lock: two callers arriving together may both scan, the first to finish wins, and no caller waits behind another's scan.
fn cached_single_hanzi_map(connection: &Connection, path: &Path) -> Arc<HanziReadings> {
    let cache = HANZI_CACHE.get_or_init(|| {
        Mutex::new(LruCache::new(
            NonZeroUsize::new(HANZI_CACHE_CAPACITY).unwrap(),
        ))
    });
    if let Some(found) = cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(path)
    {
        return Arc::clone(found);
    }
    let (built, complete) = single_hanzi_map(connection);
    let built = Arc::new(built);
    // With no busy timeout (bridge.cpp parity) a scan that met a writer's lock is partial; keeping it would answer empty for the rest of the process.
    if !complete {
        return built;
    }
    let mut cache = cache.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(found) = cache.get(path) {
        return Arc::clone(found);
    }
    cache.put(path.to_path_buf(), built);
    Arc::clone(cache.get(path).expect("inserted hanzi cache entry"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::fixtures::{eval_resources, pinyin_db};

    fn fixture(directory: &Path) -> PathBuf {
        pinyin_db(
            directory,
            &[
                ("tbl_1_n", "ni", "你", 100),
                ("tbl_1_m", "men", "们", 100),
                ("tbl_1_x", "xing", "行", 1000),
                ("tbl_1_h", "hang", "行", 10),
                ("tbl_1_h", "hao", "好", 50),
                ("tbl_1_h", "hao", "号", 40),
                ("tbl_1_h", "hai", "好", 50),
                ("tbl_2_n", "ni'hao", "你好", 100),
                ("tbl_2_n", "ni'hao", "拟好", 90),
                ("tbl_2_x", "xing'xing", "行行", 500),
                ("tbl_2_h", "hang'hang", "行行", 5),
                (
                    "tbl_others_n",
                    "ni'ni'ni'ni'ni'ni'ni'ni",
                    "你你你你你你你你",
                    1,
                ),
            ],
        )
    }

    #[test]
    fn exact_hanzi_sql_writes_the_lookup_statement_directly() {
        let sql = exact_hanzi_sql(2, b'n');
        assert_eq!(
            sql,
            "SELECT \"key\" FROM \"tbl_2_n\" WHERE \"value\"=?1 ORDER BY \"weight\" DESC, \"key\" ASC LIMIT 1"
        );
        assert_eq!(sql.capacity(), sql.len());
    }

    #[test]
    fn single_hanzi_sql_writes_the_lookup_statement_directly() {
        let sql = single_hanzi_sql(b'n');
        assert_eq!(
            sql,
            "SELECT \"key\", \"value\" FROM \"tbl_1_n\" ORDER BY \"weight\" DESC, \"key\" ASC"
        );
        assert_eq!(sql.capacity(), sql.len());
    }

    #[test]
    fn an_exact_phrase_wins_and_tables_are_tried_in_letter_order() {
        let directory = tempfile::tempdir().unwrap();
        let path = fixture(directory.path());
        assert_eq!(hanzi_to_pinyin(&path, "你好"), "ni'hao");
        // The first table in a..z order with the phrase answers, whatever the weights elsewhere (bridge.cpp:286-304).
        assert_eq!(hanzi_to_pinyin(&path, "行行"), "hang'hang");
        assert_eq!(hanzi_to_pinyin(&path, "行"), "hang");
        // Within one table: heaviest, then the smaller key.
        assert_eq!(hanzi_to_pinyin(&path, "好"), "hai");
    }

    #[test]
    fn characters_fall_back_to_their_first_single_reading() {
        let directory = tempfile::tempdir().unwrap();
        let path = fixture(directory.path());
        assert_eq!(hanzi_to_pinyin(&path, "你们"), "ni'men");
        assert_eq!(hanzi_to_pinyin(&path, "行你"), "hang'ni");
        // Eight characters name `tbl_8_n`, which does not exist, so the overflow table is never consulted.
        assert_eq!(
            hanzi_to_pinyin(&path, "你你你你你你你你"),
            "ni'ni'ni'ni'ni'ni'ni'ni"
        );
        assert_eq!(hanzi_to_pinyin(&path, "你猫"), "");
    }

    #[test]
    fn only_short_all_han_text_is_looked_up() {
        let directory = tempfile::tempdir().unwrap();
        let path = fixture(directory.path());
        assert_eq!(hanzi_to_pinyin(&path, ""), "");
        assert_eq!(hanzi_to_pinyin(&path, "你a"), "");
        assert_eq!(hanzi_to_pinyin(&path, "你 好"), "");
        // U+3007 counts as Han elsewhere in the engine but not on the import path.
        assert_eq!(hanzi_to_pinyin(&path, "〇"), "");
        assert_eq!(
            hanzi_to_pinyin(&path, &"你".repeat(128)),
            "ni'".repeat(127) + "ni"
        );
        assert_eq!(hanzi_to_pinyin(&path, &"你".repeat(129)), "");
    }

    /// bridge.cpp:749-753 opened the dictionary with no busy timeout, so a locked file answers empty at once rather than stalling every word of a validation batch for rusqlite's default 5 s.
    #[test]
    fn a_locked_dictionary_answers_empty_at_once() {
        let directory = tempfile::tempdir().unwrap();
        let path = fixture(directory.path());
        let locker = Connection::open(&path).unwrap();
        locker.execute_batch("BEGIN EXCLUSIVE;").unwrap();
        let start = std::time::Instant::now();
        assert_eq!(hanzi_to_pinyin(&path, "你好"), "");
        assert!(
            start.elapsed() < std::time::Duration::from_millis(1_000),
            "a locked lookup took {:?}",
            start.elapsed()
        );
        locker.execute_batch("COMMIT;").unwrap();
        assert_eq!(hanzi_to_pinyin(&path, "你好"), "ni'hao");
        // The per-character map scanned under the lock was not kept.
        assert_eq!(hanzi_to_pinyin(&path, "你们"), "ni'men");
    }

    #[test]
    fn a_missing_dictionary_answers_empty() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("msime-pinyin.db");
        assert_eq!(hanzi_to_pinyin(&path, "你"), "");
        assert!(!path.exists());
        assert_eq!(hanzi_to_pinyin(Path::new(""), "你"), "");
    }

    #[test]
    fn the_single_character_cache_is_bounded_across_database_paths() {
        let mut directories = Vec::new();
        for index in 0..=HANZI_CACHE_CAPACITY {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("msime-pinyin.db");
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(&format!(
                    "CREATE TABLE tbl_1_n(key TEXT,value TEXT,weight INTEGER); INSERT INTO tbl_1_n VALUES('ni','合成{index}',1);"
                ))
                .unwrap();
            assert_eq!(hanzi_to_pinyin(&path, "猫"), "");
            directories.push(directory);
        }
        let cache = HANZI_CACHE.get().unwrap().lock().unwrap();
        assert!(cache.len() <= HANZI_CACHE_CAPACITY);
    }

    #[test]
    fn real_dictionary_readings() {
        let Some(resources) = eval_resources("real_dictionary_readings") else {
            return;
        };
        let path = resources.join(crate::assets::MAIN_DICTIONARY);
        assert_eq!(hanzi_to_pinyin(&path, "你好"), "ni'hao");
        assert_eq!(hanzi_to_pinyin(&path, "行"), "hang");
    }
}
