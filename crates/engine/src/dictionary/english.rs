//! `english.db` (schemes-lang.md §4, data-formats.md §5): prefix completion, the en↔zh gloss tables, the custom translations sidecar and the learned-gloss store. English weights exceed `i32` in the shipped data.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, TransactionBehavior};

use super::{column_i64, column_text, sql_limit};
use crate::assets;
use crate::error::Result;
use crate::types::{CandidateSource, WordItem};

/// Hand-written translations (`custom_translations.txt`), which outrank every database gloss.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomTranslations {
    pub en_zh: HashMap<String, String>,
    pub zh_en: HashMap<String, String>,
}

const PREFIX_SQL: &str = "SELECT word,display,weight FROM english_words WHERE word >= ?1 AND word < ?2 ORDER BY CASE WHEN word = ?1 THEN 0 ELSE 1 END, weight DESC, length(word), word, display LIMIT ?3";
const EN_ZH_SQL: &str = "SELECT chinese_gloss FROM en_zh_glosses WHERE english=?1";
const ZH_EN_SQL: &str = "SELECT english_gloss FROM zh_en_glosses WHERE chinese=?1";
const ENGLISH_WORDS_DDL: &str = "CREATE TABLE english_words(word TEXT COLLATE BINARY NOT NULL,display TEXT NOT NULL,weight INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(word,display)) WITHOUT ROWID;";
const GLOSS_TABLES_DDL: &str = "CREATE TABLE IF NOT EXISTS en_zh_glosses(english TEXT COLLATE BINARY PRIMARY KEY,chinese_gloss TEXT NOT NULL) WITHOUT ROWID;CREATE TABLE IF NOT EXISTS zh_en_glosses(chinese TEXT COLLATE BINARY PRIMARY KEY,english_gloss TEXT NOT NULL) WITHOUT ROWID;PRAGMA user_version=3;";
const GLOSS_BUSY_TIMEOUT: Duration = Duration::from_millis(250);
/// The reference never set a busy timeout on its read connections (english_dictionary.cpp:325,371), so SQLite's default of none applied: a locked file answers "no rows" at once instead of stalling the keystroke. rusqlite would otherwise wait 5 s.
const READ_BUSY_TIMEOUT: Duration = Duration::ZERO;

pub struct EnglishDictionary {
    connection: Option<Connection>,
    translations: CustomTranslations,
    gloss_cache: Option<PathBuf>,
    /// The learned-gloss store, opened on the first lookup that reaches it and retried while the file does not exist yet: nothing may have been learned when the dictionary opens (english_dictionary.cpp:363-386).
    gloss_cache_connection: RefCell<Option<Connection>>,
}

impl EnglishDictionary {
    /// Read-only; never creates or migrates the file. `translations` defaults to `custom_translations.txt` beside the database when `None`. `gloss_cache` is the learned-gloss database, read-only and never created on read.
    pub fn open(database: &Path, translations: Option<&Path>, gloss_cache: Option<&Path>) -> Self {
        // The reference spelled "no path" as an empty string, and `RuntimePaths` still yields empty paths for missing roots, so an empty path means the default here too.
        let translations = translations.filter(|path| !path.as_os_str().is_empty());
        let gloss_cache = gloss_cache
            .filter(|path| !path.as_os_str().is_empty())
            .map(Path::to_path_buf);
        // Without a database path the reference loaded no sidecar at all, even an explicit one (english_dictionary.cpp:183-184).
        let translations = if database.as_os_str().is_empty() {
            CustomTranslations::default()
        } else {
            let sidecar = translations.map_or_else(
                || {
                    database
                        .parent()
                        .unwrap_or(Path::new(""))
                        .join(assets::TRANSLATIONS)
                },
                Path::to_path_buf,
            );
            load_custom_translations(&sidecar)
        };
        Self {
            connection: open_prefix_connection(database),
            translations,
            gloss_cache,
            gloss_cache_connection: RefCell::new(None),
        }
    }

    pub fn ready(&self) -> bool {
        self.connection.is_some()
    }

    /// Exact word first, then weight, then shorter, then word and display (english_dictionary.cpp:39-81). Rows are `WordItem(pinyin = word, word = display, weight, EnglishDictionary)`. Empty unless the prefix is non-empty `a..=z`.
    pub fn query_prefix(&self, prefix: &str, limit: usize) -> Vec<WordItem> {
        let Some(connection) = &self.connection else {
            return Vec::new();
        };
        if !is_lower_ascii_word(prefix) || limit == 0 {
            return Vec::new();
        }
        let upper_bound = format!("{prefix}{{");
        let Ok(mut statement) = connection.prepare_cached(PREFIX_SQL) else {
            return Vec::new();
        };
        let Ok(mut rows) = statement.query((prefix, upper_bound.as_str(), sql_limit(limit))) else {
            return Vec::new();
        };
        let mut candidates = Vec::with_capacity(limit);
        loop {
            match rows.next() {
                Ok(Some(row)) => {
                    // A NULL word or display is skipped rather than shown blank (english_dictionary.cpp:64-69).
                    let is_null = |index| matches!(row.get_ref(index), Ok(ValueRef::Null));
                    if is_null(0) || is_null(1) {
                        continue;
                    }
                    let (Ok(word), Ok(display), Ok(weight)) =
                        (column_text(row, 0), column_text(row, 1), column_i64(row, 2))
                    else {
                        return Vec::new();
                    };
                    candidates.push(WordItem::new(
                        word,
                        display,
                        weight,
                        CandidateSource::EnglishDictionary,
                        "",
                    ));
                }
                Ok(None) => return candidates,
                // A step that ends in anything but DONE discards the partial page (english_dictionary.cpp:75-79).
                Err(_) => return Vec::new(),
            }
        }
    }

    /// Custom sidecar, then `en_zh_glosses`, then the learned store; exact key, English not lowercased. Empty when none has it.
    pub fn query_chinese_gloss(&self, english: &str) -> String {
        self.query_gloss(false, english)
    }

    /// Custom sidecar, then `zh_en_glosses`, then the learned store.
    pub fn query_english_gloss(&self, chinese: &str) -> String {
        self.query_gloss(true, chinese)
    }

    /// Three layers in order of authority: what the user wrote, the shipped ECDICT tables, then glosses fetched online, which are exactly the words the dictionary lacked and so the least trustworthy (english_dictionary.cpp:108-140).
    fn query_gloss(&self, chinese_to_english: bool, key: &str) -> String {
        let custom = if chinese_to_english {
            &self.translations.zh_en
        } else {
            &self.translations.en_zh
        };
        if let Some(gloss) = custom.get(key) {
            return gloss.clone();
        }
        if key.is_empty() {
            return String::new();
        }
        if let Some(connection) = &self.connection {
            if let Some(gloss) = lookup_gloss(connection, chinese_to_english, key) {
                if !gloss.is_empty() {
                    return gloss;
                }
            }
        }
        let Some(cache_path) = &self.gloss_cache else {
            return String::new();
        };
        let mut cache = self.gloss_cache_connection.borrow_mut();
        if cache.is_none() {
            *cache = open_read_only(cache_path);
        }
        let Some(connection) = cache.as_ref() else {
            return String::new();
        };
        match lookup_gloss(connection, chinese_to_english, key) {
            Some(gloss) => gloss,
            None => {
                // A store that cannot answer is closed so the next lookup opens it afresh (english_dictionary.cpp:374-384).
                *cache = None;
                String::new()
            }
        }
    }
}

/// Create or migrate `english_words` to the composite-key, weighted shape, create the gloss tables, `user_version = 3` (english_dictionary.cpp:242-315). The golden harness calls this for fixtures without an `english.db`.
pub fn ensure_english_schema(path: &Path) -> Result<()> {
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        if !metadata.file_type().is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "English dictionary path is not a regular file",
            )
            .into());
        }
    }
    let mut connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let mut has_table = false;
    let mut has_weight = false;
    let mut primary_key_columns = 0;
    {
        let mut columns = connection.prepare_cached("PRAGMA table_info(english_words)")?;
        let mut rows = columns.query(())?;
        while let Some(row) = rows.next()? {
            has_table = true;
            has_weight |= row.get::<_, Option<String>>(1)?.as_deref() == Some("weight");
            if row.get::<_, i64>(5)? > 0 {
                primary_key_columns += 1;
            }
        }
    }

    if !has_table {
        connection.execute_batch(ENGLISH_WORDS_DDL)?;
    } else if !(has_weight && primary_key_columns == 2) {
        // An older table (no weight column, or keyed on word alone) is rebuilt in one immediate transaction; dropping it rolls back on any failure.
        let copy_sql = if has_weight {
            "INSERT OR IGNORE INTO english_words_new(word,display,weight) SELECT word,display,weight FROM english_words;"
        } else {
            "INSERT OR IGNORE INTO english_words_new(word,display,weight) SELECT word,display,0 FROM english_words;"
        };
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(
            "CREATE TABLE english_words_new(word TEXT COLLATE BINARY NOT NULL,display TEXT NOT NULL,weight INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(word,display)) WITHOUT ROWID",
        )?;
        transaction.execute_batch(copy_sql)?;
        transaction.execute_batch("DROP TABLE english_words")?;
        transaction.execute_batch("ALTER TABLE english_words_new RENAME TO english_words")?;
        transaction.commit()?;
    }
    connection.execute_batch(GLOSS_TABLES_DDL)?;
    Ok(())
}

/// `INSERT OR REPLACE` one learned gloss, creating the store's schema first (english_dictionary.cpp:152-177). False on an empty path, key or gloss, or any failure.
pub fn upsert_gloss(path: &Path, chinese_to_english: bool, key: &str, gloss: &str) -> bool {
    if path.as_os_str().is_empty() || key.is_empty() || gloss.is_empty() {
        return false;
    }
    // The host treats a failed save as "not cached" and carries on, so the reference answered with a bool and so does this (english_dictionary.cpp:152-177).
    write_gloss(path, chinese_to_english, key, gloss).is_ok()
}

fn write_gloss(path: &Path, chinese_to_english: bool, key: &str, gloss: &str) -> Result<()> {
    ensure_english_schema(path)?;
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(GLOSS_BUSY_TIMEOUT)?;
    let sql = if chinese_to_english {
        "INSERT OR REPLACE INTO zh_en_glosses(chinese,english_gloss) VALUES(?1,?2)"
    } else {
        "INSERT OR REPLACE INTO en_zh_glosses(english,chinese_gloss) VALUES(?1,?2)"
    };
    connection.prepare_cached(sql)?.execute((key, gloss))?;
    Ok(())
}

/// Parse the sidecar (english_dictionary.cpp:179-240): BOM stripped, any line ending, trimmed, `#` skipped, `source<TAB>gloss`; a source with any non-ASCII byte is Chinese. Last line wins. A missing file is empty.
pub fn load_custom_translations(path: &Path) -> CustomTranslations {
    let mut translations = CustomTranslations::default();
    // An unreadable sidecar is the same as none (english_dictionary.cpp:189-191).
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return translations;
    };
    if !metadata.file_type().is_file() {
        return translations;
    }
    let Ok(text) = std::fs::read(path) else {
        return translations;
    };
    let text = text.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&text);
    for line in split_lines(text) {
        let line = trim_start(trim_end(line));
        if line.is_empty() || line[0] == b'#' {
            continue;
        }
        let Some(tab) = line.iter().position(|&byte| byte == b'\t') else {
            continue;
        };
        if tab == 0 || tab + 1 >= line.len() {
            continue;
        }
        let source = trim_end(&line[..tab]);
        let gloss = trim_start(trim_end(&line[tab + 1..]));
        if source.is_empty() || gloss.is_empty() {
            continue;
        }
        let chinese_source = source.iter().any(|&byte| byte >= 0x80);
        let source = String::from_utf8_lossy(source).into_owned();
        let gloss = String::from_utf8_lossy(gloss).into_owned();
        if chinese_source {
            translations.zh_en.insert(source, gloss);
        } else {
            translations.en_zh.insert(source, gloss);
        }
    }
    translations
}

/// Opens the dictionary and checks the prefix statement prepares, which is what the reference's `ready()` meant: a file without `english_words` is not a dictionary (english_dictionary.cpp:317-343).
fn open_prefix_connection(path: &Path) -> Option<Connection> {
    let connection = open_read_only(path)?;
    if connection.prepare_cached(PREFIX_SQL).is_err() {
        return None;
    }
    Some(connection)
}

fn open_read_only(path: &Path) -> Option<Connection> {
    // An empty path is a missing file; SQLite would open a private temporary database for it.
    if path.as_os_str().is_empty() {
        return None;
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    connection.busy_timeout(READ_BUSY_TIMEOUT).ok()?;
    Some(connection)
}

/// The first row's gloss, "" for none, or `None` when the gloss tables cannot be prepared. Both are prepared together, so a store with only one of them answers neither, as in the reference (english_dictionary.cpp:345-361).
fn lookup_gloss(connection: &Connection, chinese_to_english: bool, key: &str) -> Option<String> {
    let en_zh = connection.prepare_cached(EN_ZH_SQL).ok()?;
    let zh_en = connection.prepare_cached(ZH_EN_SQL).ok()?;
    let mut statement = if chinese_to_english { zh_en } else { en_zh };
    let Ok(mut rows) = statement.query((key,)) else {
        return Some(String::new());
    };
    Some(match rows.next() {
        Ok(Some(row)) => column_text(row, 0).unwrap_or_default(),
        _ => String::new(),
    })
}

fn is_lower_ascii_word(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_lowercase())
}

/// C `isspace` in the C locale, which includes the vertical tab that `u8::is_ascii_whitespace` leaves out.
fn is_c_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\x0B' | b'\x0C' | b'\r')
}

fn trim_start(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|&byte| !is_c_space(byte))
        .unwrap_or(bytes.len());
    &bytes[start..]
}

fn trim_end(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .rposition(|&byte| !is_c_space(byte))
        .map_or(0, |at| at + 1);
    &bytes[..end]
}

/// Lines ended by `\r\n`, `\r` or `\n`.
fn split_lines(text: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut rest = text;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let end = rest
            .iter()
            .position(|&byte| byte == b'\r' || byte == b'\n')
            .unwrap_or(rest.len());
        let line = &rest[..end];
        let skip = if rest[end..].starts_with(b"\r\n") {
            2
        } else {
            (end < rest.len()) as usize
        };
        rest = &rest[end + skip..];
        Some(line)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::fixtures::{english_db, eval_resources};

    fn words(items: &[WordItem]) -> Vec<(&str, &str)> {
        items
            .iter()
            .map(|item| (item.pinyin.as_str(), item.word.as_str()))
            .collect()
    }

    fn user_version(path: &Path) -> i64 {
        Connection::open(path)
            .unwrap()
            .query_row("PRAGMA user_version", (), |row| row.get(0))
            .unwrap()
    }

    /// english_dictionary.cpp:325,371 set no busy timeout on the read connections, so a file another connection holds locked answers "no rows" at once instead of stalling the keystroke for rusqlite's default 5 s.
    #[test]
    fn a_locked_dictionary_answers_empty_at_once() {
        let directory = tempfile::tempdir().unwrap();
        let path = english_db(directory.path(), &[("hello", "hello", 10)], &[], &[]);
        let gloss_directory = tempfile::tempdir().unwrap();
        let gloss_path = gloss_directory.path().join("translation-glosses.db");
        assert!(upsert_gloss(&gloss_path, false, "hello", "你好"));
        let dictionary = EnglishDictionary::open(&path, None, Some(&gloss_path));
        assert_eq!(dictionary.query_prefix("hel", 5).len(), 1);
        assert_eq!(dictionary.query_chinese_gloss("hello"), "你好");
        let english_lock = Connection::open(&path).unwrap();
        english_lock.execute_batch("BEGIN EXCLUSIVE;").unwrap();
        let gloss_lock = Connection::open(&gloss_path).unwrap();
        gloss_lock.execute_batch("BEGIN EXCLUSIVE;").unwrap();
        let start = std::time::Instant::now();
        assert!(dictionary.query_prefix("hel", 5).is_empty());
        assert_eq!(dictionary.query_chinese_gloss("hello"), "");
        assert!(
            start.elapsed() < Duration::from_millis(1_000),
            "locked lookups took {:?}",
            start.elapsed()
        );
        english_lock.execute_batch("ROLLBACK;").unwrap();
        gloss_lock.execute_batch("ROLLBACK;").unwrap();
        assert_eq!(dictionary.query_prefix("hel", 5).len(), 1);
        assert_eq!(dictionary.query_chinese_gloss("hello"), "你好");
    }

    #[test]
    fn prefix_completion_puts_the_exact_word_first_then_weight_then_length() {
        let directory = tempfile::tempdir().unwrap();
        let path = english_db(
            directory.path(),
            &[
                ("he", "he", 110),
                ("he", "HE", 50),
                ("hello", "hello", 100),
                ("hex", "hex", 100),
                ("helper", "helper", 95),
                ("help", "help", 90),
                ("dont", "don't", 10),
                ("hi", "hi", 1_000_000),
            ],
            &[],
            &[],
        );
        let dictionary = EnglishDictionary::open(&path, None, None);
        assert!(dictionary.ready());
        let rows = dictionary.query_prefix("he", 10);
        assert_eq!(
            words(&rows),
            [
                ("he", "he"),
                ("he", "HE"),
                ("hex", "hex"),
                ("hello", "hello"),
                ("helper", "helper"),
                ("help", "help"),
            ]
        );
        assert_eq!(rows[0].source, CandidateSource::EnglishDictionary);
        assert_eq!(rows[0].canonical_pinyin, "");
        assert_eq!(
            words(&dictionary.query_prefix("dont", 5)),
            [("dont", "don't")]
        );
        assert_eq!(dictionary.query_prefix("he", 2).len(), 2);
        assert!(dictionary.query_prefix("He", 5).is_empty());
        assert!(dictionary.query_prefix("he-", 5).is_empty());
        assert!(dictionary.query_prefix("", 5).is_empty());
        assert!(dictionary.query_prefix("he", 0).is_empty());
    }

    #[test]
    fn weights_beyond_i32_survive() {
        let directory = tempfile::tempdir().unwrap();
        let path = english_db(
            directory.path(),
            &[("the", "the", 23_135_851_162)],
            &[],
            &[],
        );
        let dictionary = EnglishDictionary::open(&path, None, None);
        assert_eq!(dictionary.query_prefix("the", 1)[0].weight, 23_135_851_162);
    }

    #[test]
    fn a_missing_or_foreign_file_is_not_ready_and_is_never_created() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("english.db");
        let dictionary = EnglishDictionary::open(&missing, None, None);
        assert!(!dictionary.ready());
        assert!(dictionary.query_prefix("he", 5).is_empty());
        assert_eq!(dictionary.query_chinese_gloss("hello"), "");
        assert!(!missing.exists());

        let foreign = directory.path().join("foreign.db");
        Connection::open(&foreign)
            .unwrap()
            .execute_batch("CREATE TABLE other(x);")
            .unwrap();
        assert!(!EnglishDictionary::open(&foreign, None, None).ready());
        assert!(!EnglishDictionary::open(Path::new(""), None, None).ready());

        // test_english_input_session.cpp:304-328: a file that is not a database at all is not ready either.
        let corrupt = directory.path().join("corrupt.db");
        std::fs::write(&corrupt, "not a sqlite database").unwrap();
        let dictionary = EnglishDictionary::open(&corrupt, None, None);
        assert!(!dictionary.ready());
        assert!(dictionary.query_prefix("ni", 5).is_empty());
    }

    // test_runtime_isolation.cpp:665-705: the sidecar outranks english.db, which outranks the learned store, and a gloss learned after opening is visible to the open dictionary.
    #[test]
    fn glosses_follow_sidecar_then_dictionary_then_learned_store() {
        let directory = tempfile::tempdir().unwrap();
        let path = english_db(
            directory.path(),
            &[("hello", "hello", 1)],
            &[
                ("hello", "你好"),
                ("world", "世界"),
                ("AI Controller", "人工智能控制器"),
            ],
            &[("世界", "world")],
        );
        let sidecar = directory.path().join("resource-translations.txt");
        std::fs::write(&sidecar, "hello\t你翻译\n华科\tHUST\n").unwrap();
        let cache = directory.path().join("user").join(assets::LEARNED_GLOSSES);
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();

        let dictionary = EnglishDictionary::open(&path, Some(&sidecar), Some(&cache));
        assert_eq!(dictionary.query_chinese_gloss("hello"), "你翻译");
        assert_eq!(dictionary.query_english_gloss("华科"), "HUST");
        assert_eq!(dictionary.query_chinese_gloss("world"), "世界");
        assert_eq!(dictionary.query_english_gloss("世界"), "world");
        // Exact keys: English is not lowercased.
        assert_eq!(
            dictionary.query_chinese_gloss("AI Controller"),
            "人工智能控制器"
        );
        assert_eq!(dictionary.query_chinese_gloss("ai controller"), "");
        assert_eq!(dictionary.query_chinese_gloss(""), "");

        assert_eq!(dictionary.query_chinese_gloss("nimbus"), "");
        assert!(
            !cache.exists(),
            "a lookup must not create the learned store"
        );
        assert!(upsert_gloss(&cache, false, "nimbus", "积雨云"));
        assert_eq!(dictionary.query_chinese_gloss("nimbus"), "积雨云");
        assert!(upsert_gloss(&cache, false, "hello", "机翻"));
        assert!(upsert_gloss(&cache, false, "world", "机翻世界"));
        assert_eq!(dictionary.query_chinese_gloss("hello"), "你翻译");
        assert_eq!(dictionary.query_chinese_gloss("world"), "世界");
        assert!(upsert_gloss(&cache, true, "积云", "cumulus"));
        assert_eq!(dictionary.query_english_gloss("积云"), "cumulus");
        // INSERT OR REPLACE: the last learned gloss wins.
        assert!(upsert_gloss(&cache, false, "nimbus", "雨云"));
        assert_eq!(dictionary.query_chinese_gloss("nimbus"), "雨云");
    }

    #[test]
    fn the_default_sidecar_sits_beside_the_database() {
        let directory = tempfile::tempdir().unwrap();
        let path = english_db(directory.path(), &[], &[("hello", "你好")], &[]);
        std::fs::write(directory.path().join(assets::TRANSLATIONS), "hello\t手写\n").unwrap();
        assert_eq!(
            EnglishDictionary::open(&path, None, None).query_chinese_gloss("hello"),
            "手写"
        );
        assert_eq!(
            EnglishDictionary::open(&path, Some(Path::new("")), None).query_chinese_gloss("hello"),
            "手写"
        );
        let elsewhere = directory.path().join("absent.txt");
        assert_eq!(
            EnglishDictionary::open(&path, Some(&elsewhere), None).query_chinese_gloss("hello"),
            "你好"
        );
        // A dictionary that is missing still reads its sidecar; one with no path reads none.
        let missing = directory.path().join("missing.db");
        assert_eq!(
            EnglishDictionary::open(&missing, None, None).query_chinese_gloss("hello"),
            "手写"
        );
        let sidecar = directory.path().join(assets::TRANSLATIONS);
        assert_eq!(
            EnglishDictionary::open(Path::new(""), Some(&sidecar), None)
                .query_chinese_gloss("hello"),
            ""
        );
    }

    #[test]
    fn a_dictionary_without_gloss_tables_falls_through_to_the_learned_store() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("english.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch(ENGLISH_WORDS_DDL)
            .unwrap();
        let cache = directory.path().join(assets::LEARNED_GLOSSES);
        assert!(upsert_gloss(&cache, false, "hello", "你好"));
        let dictionary = EnglishDictionary::open(&path, None, Some(&cache));
        assert!(dictionary.ready());
        assert_eq!(dictionary.query_chinese_gloss("hello"), "你好");
    }

    // test_engine_smoke.cpp:317-321: the schema step creates a missing database.
    #[test]
    fn schema_setup_creates_a_missing_database_and_is_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("english.db");
        ensure_english_schema(&path).unwrap();
        assert!(path.exists());
        assert_eq!(user_version(&path), 3);
        ensure_english_schema(&path).unwrap();
        let connection = Connection::open(&path).unwrap();
        for table in ["english_words", "en_zh_glosses", "zh_en_glosses"] {
            let count: i64 = connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    (table,),
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "{table}");
        }
        assert!(EnglishDictionary::open(&path, None, None).ready());
    }

    #[test]
    fn schema_setup_migrates_older_english_word_tables() {
        let directory = tempfile::tempdir().unwrap();
        let unweighted = directory.path().join("unweighted.db");
        Connection::open(&unweighted)
            .unwrap()
            .execute_batch(
                "CREATE TABLE english_words(word TEXT PRIMARY KEY, display TEXT NOT NULL);
                 INSERT INTO english_words VALUES ('dont', 'don''t'), ('hello', 'Hello');",
            )
            .unwrap();
        ensure_english_schema(&unweighted).unwrap();
        let dictionary = EnglishDictionary::open(&unweighted, None, None);
        let rows = dictionary.query_prefix("hello", 5);
        assert_eq!(words(&rows), [("hello", "Hello")]);
        assert_eq!(rows[0].weight, 0);
        assert_eq!(user_version(&unweighted), 3);

        let single_key = directory.path().join("single-key.db");
        Connection::open(&single_key)
            .unwrap()
            .execute_batch(
                "CREATE TABLE english_words(word TEXT PRIMARY KEY, display TEXT NOT NULL, weight INTEGER);
                 INSERT INTO english_words VALUES ('hello', 'Hello', 42);",
            )
            .unwrap();
        ensure_english_schema(&single_key).unwrap();
        let connection = Connection::open(&single_key).unwrap();
        connection
            .execute(
                "INSERT INTO english_words(word,display,weight) VALUES('hello','hello',7)",
                (),
            )
            .unwrap();
        let primary_key_columns: i64 = connection
            .query_row(
                "SELECT count(*) FROM pragma_table_info('english_words') WHERE pk > 0",
                (),
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(primary_key_columns, 2);
        let weights: Vec<(String, i64)> = connection
            .prepare("SELECT display, weight FROM english_words ORDER BY display")
            .unwrap()
            .query_map((), |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(|row| row.unwrap())
            .collect();
        assert_eq!(weights, [("Hello".to_owned(), 42), ("hello".to_owned(), 7)]);
    }

    #[test]
    fn upsert_gloss_rejects_empty_input_and_creates_the_store_schema() {
        let directory = tempfile::tempdir().unwrap();
        let cache = directory.path().join(assets::LEARNED_GLOSSES);
        assert!(!upsert_gloss(Path::new(""), false, "hello", "你好"));
        assert!(!upsert_gloss(&cache, false, "", "你好"));
        assert!(!upsert_gloss(&cache, false, "hello", ""));
        assert!(!cache.exists());
        assert!(upsert_gloss(&cache, false, "hello", "你好"));
        assert_eq!(user_version(&cache), 3);
        // The store carries the full english.db schema, including an empty word table.
        assert!(EnglishDictionary::open(&cache, None, None).ready());
        assert!(!upsert_gloss(
            &directory.path().join("no/such/dir.db"),
            false,
            "a",
            "b"
        ));
    }

    #[test]
    fn sidecar_parsing_follows_the_reference_rules() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(assets::TRANSLATIONS);
        std::fs::write(
            &path,
            b"\xEF\xBB\xBF# \xE6\xA0\xBC\xE5\xBC\x8F\thidden\r\n  hello \t  \xE4\xBD\xA0\xE5\xA5\xBD  \r\
\xE5\x8D\x8E\xE7\xA7\x91\tHUST\n\
\tleading tab\n\
no tab here\n\
empty gloss\t   \n\
\x0Bvtab\tstripped\x0B\n\
hello\tlast wins\n\
\n",
        )
        .unwrap();
        let translations = load_custom_translations(&path);
        assert_eq!(
            translations.en_zh.get("hello").map(String::as_str),
            Some("last wins")
        );
        assert_eq!(
            translations.en_zh.get("vtab").map(String::as_str),
            Some("stripped")
        );
        assert_eq!(
            translations.zh_en.get("华科").map(String::as_str),
            Some("HUST")
        );
        // A leading tab is trimmed away with the rest of the whitespace, and so is a trailing one, so neither line has a separator left.
        assert_eq!(translations.en_zh.len(), 2, "{:?}", translations.en_zh);
        assert_eq!(translations.zh_en.len(), 1);
        assert_eq!(
            load_custom_translations(&directory.path().join("absent.txt")),
            CustomTranslations::default()
        );
    }

    #[cfg(unix)]
    #[test]
    fn sidecar_loading_rejects_symlinked_files() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let external = outside.path().join("translations.txt");
        std::fs::write(&external, "hello\t外部内容\n").unwrap();
        let linked = directory.path().join(assets::TRANSLATIONS);
        symlink(&external, &linked).unwrap();

        assert_eq!(
            load_custom_translations(&linked),
            CustomTranslations::default()
        );
    }

    #[test]
    fn real_english_dictionary_completes_and_glosses() {
        let Some(resources) = eval_resources("real_english_dictionary_completes_and_glosses")
        else {
            return;
        };
        let dictionary =
            EnglishDictionary::open(&resources.join(assets::ENGLISH_DICTIONARY), None, None);
        assert!(dictionary.ready());
        let rows = dictionary.query_prefix("hello", 3);
        assert_eq!(rows[0].pinyin, "hello");
        assert!(rows[0].weight > 0);
        assert!(!dictionary.query_chinese_gloss("hello").is_empty());
    }
}
