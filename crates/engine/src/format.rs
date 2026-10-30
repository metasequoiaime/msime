//! 词库格式契约：拼音表的命名规则，以及 `msime-pinyin.db`、`msime-wubi.db`、`msime-others.db` 的文件内版本（`metadata` 表）。dict-builder 和运行时必须对它们取得一致，所以只在这里定义一次。完整规格见 `docs/data-formats.md` §4、§9、§12。

use rusqlite::{Connection, OptionalExtension};

use crate::diagnostics;
use crate::error::{EngineError, Result};

/// 词库格式版本：发布清单的 `format_version` 与 `engine_compatibility.dictionary_format`，也是 dict-builder 写进三个主库 `metadata` 表的 `format_version`。引擎读得懂这个版本及以下的文件。
pub const FORMAT_VERSION: i32 = 1;

/// 没有 `metadata` 表的文件按这个版本读：文件内版本出现之前发布的词库（`dict-v2.0.39` 及更早）都是这样，必须照常能用。
pub const LEGACY_FORMAT_VERSION: i32 = 0;

/// 三个主库的 `metadata` 表，形状与语言词库（`language_dictionary::SCHEMA`）的同名表相同：`name` 是键，`value` 是十进制文本。
pub const METADATA_SCHEMA: &str =
    "CREATE TABLE IF NOT EXISTS metadata(name TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;";

/// `metadata` 里存放格式版本的那一行，与语言词库同名。
pub const METADATA_FORMAT_VERSION: &str = crate::language_dictionary::METADATA_FORMAT_VERSION;

/// Tables are numbered by syllable count up to this; longer keys go to the overflow bucket.
pub const MAXIMUM_NUMBERED_SYLLABLES: usize = 7;
/// First letters that have tables. `i`, `u` and `v` start no syllable, so they have none.
pub const SHIPPED_INITIALS: &str = "abcdefghjklmnopqrstwxyz";
pub const TABLE_PREFIX: &str = "tbl_";
pub const OVERFLOW_BUCKET: &str = "others";

/// `tbl_<n>_<initial>`, or `tbl_others_<initial>` past seven syllables. `None` for no syllables or an initial outside `a..=z`. A shipped-set miss (`i`, `u`, `v`) still gets a name; the table simply does not exist, and readers treat that as no rows.
pub fn quanpin_table(syllables: usize, initial: u8) -> Option<String> {
    if syllables == 0 || !initial.is_ascii_lowercase() {
        return None;
    }
    let bucket = match syllables {
        1 => "1",
        2 => "2",
        3 => "3",
        4 => "4",
        5 => "5",
        6 => "6",
        7 => "7",
        _ => OVERFLOW_BUCKET,
    };
    let mut table = String::with_capacity(TABLE_PREFIX.len() + bucket.len() + 2);
    table.push_str(TABLE_PREFIX);
    table.push_str(bucket);
    table.push('_');
    table.push(initial as char);
    Some(table)
}

/// The table for a segmented key: by segment count and the first letter of the first segment.
pub fn build_table_name(segments: &[String]) -> Option<String> {
    let first = segments.first()?.as_bytes().first()?;
    quanpin_table(segments.len(), *first)
}

/// 读 `connection` 所在文件的格式版本并检查引擎读不读得懂；`file` 是写进错误信息的文件名（只给名字，不带路径）。没有 `metadata` 表时是 [`LEGACY_FORMAT_VERSION`]。有表却读不出 `format_version`（没有这一行，或表的列不对）、值不是非负整数、或比 [`FORMAT_VERSION`] 新时报 `DICTIONARY_FORMAT_UNSUPPORTED`（`EngineError::Failed`），调用方不能把这样的文件当作已知格式去读：新版本的表可能换了列或换了语义，照旧读只会悄悄出错。连 `sqlite_master` 都读不出（不是数据库、被锁住），或读 `metadata` 时遇到被锁、读写出错、文件损坏这类与格式无关的错误时，原样返回 `EngineError::Sqlite`，由调用方按它原来对读不了的文件的处理来办。
pub fn check_format_version(connection: &Connection, file: &str) -> Result<i32> {
    let unsupported = |detail: String| {
        EngineError::failed(format!(
            "{}{file} {detail}, this engine reads format_version {FORMAT_VERSION} or older",
            diagnostics::DICTIONARY_FORMAT_UNSUPPORTED
        ))
    };
    let has_metadata = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='metadata')",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    if !has_metadata {
        return Ok(LEGACY_FORMAT_VERSION);
    }
    let value = connection
        .query_row(
            "SELECT value FROM metadata WHERE name = ?1",
            [METADATA_FORMAT_VERSION],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| {
            if is_environmental(&error) {
                EngineError::Sqlite(error)
            } else {
                unsupported(format!("has an unreadable metadata table ({error})"))
            }
        })?
        .ok_or_else(|| unsupported("has a metadata table without format_version".to_owned()))?;
    match value.parse::<i32>() {
        Ok(version) if (LEGACY_FORMAT_VERSION..=FORMAT_VERSION).contains(&version) => Ok(version),
        _ => Err(unsupported(format!("has format_version {value:?}"))),
    }
}

/// 读 `metadata` 时的错误里，只有 SQL 层面的形状问题（没有 `value` 列之类，SQLite 报的是笼统的 `SQLITE_ERROR`）和列值转换失败说明这张表不是认得的格式。被锁、读写出错、文件损坏这类错误与格式无关，像读 `sqlite_master` 失败一样原样交给调用方，不能说成格式不受支持。
fn is_environmental(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(failure, _) if failure.code != rusqlite::ErrorCode::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_contract() {
        assert_eq!(quanpin_table(2, b'n').as_deref(), Some("tbl_2_n"));
        assert_eq!(quanpin_table(7, b'a').as_deref(), Some("tbl_7_a"));
        assert_eq!(quanpin_table(8, b'z').as_deref(), Some("tbl_others_z"));
        assert_eq!(quanpin_table(2, b'n').unwrap().capacity(), "tbl_2_n".len());
        assert_eq!(
            quanpin_table(8, b'z').unwrap().capacity(),
            "tbl_others_z".len()
        );
        assert_eq!(quanpin_table(0, b'a'), None);
        assert_eq!(quanpin_table(1, b'A'), None);
        assert_eq!(
            build_table_name(&["ni".into(), "hao".into()]).as_deref(),
            Some("tbl_2_n")
        );
        assert_eq!(build_table_name(&[]), None);
        assert_eq!(build_table_name(&[String::new()]), None);
    }

    fn database(sql: &str) -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch(sql).unwrap();
        connection
    }

    #[test]
    fn a_file_without_metadata_is_the_legacy_version() {
        let connection =
            database("CREATE TABLE tbl_1_a(key TEXT, jp TEXT, value TEXT, weight INTEGER);");
        assert_eq!(
            check_format_version(&connection, "msime-pinyin.db").unwrap(),
            LEGACY_FORMAT_VERSION
        );
    }

    #[test]
    fn a_file_that_is_not_a_database_is_left_to_the_caller() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("msime-pinyin.db");
        std::fs::write(&path, b"not a database, not even close to one").unwrap();
        let connection = Connection::open(&path).unwrap();
        assert!(matches!(
            check_format_version(&connection, "msime-pinyin.db"),
            Err(EngineError::Sqlite(_))
        ));
    }

    #[test]
    fn only_shape_errors_on_the_metadata_table_mean_an_unknown_format() {
        let failure = |code| {
            rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(code), Some(String::new()))
        };
        for code in [
            rusqlite::ffi::SQLITE_BUSY,
            rusqlite::ffi::SQLITE_LOCKED,
            rusqlite::ffi::SQLITE_IOERR,
            rusqlite::ffi::SQLITE_CORRUPT,
            rusqlite::ffi::SQLITE_NOTADB,
        ] {
            assert!(is_environmental(&failure(code)), "{code}");
        }
        assert!(!is_environmental(&failure(rusqlite::ffi::SQLITE_ERROR)));
        assert!(!is_environmental(&rusqlite::Error::InvalidColumnType(
            0,
            "value".to_owned(),
            rusqlite::types::Type::Blob
        )));
    }

    #[test]
    fn the_current_and_older_versions_are_read() {
        for version in [LEGACY_FORMAT_VERSION, FORMAT_VERSION] {
            let connection = database(&format!(
                "{METADATA_SCHEMA} INSERT INTO metadata VALUES('format_version', '{version}');"
            ));
            assert_eq!(
                check_format_version(&connection, "msime-wubi.db").unwrap(),
                version
            );
        }
    }

    #[test]
    fn a_newer_or_malformed_version_is_refused_with_the_file_name() {
        let newer = FORMAT_VERSION + 1;
        for (sql, detail) in [
            (
                format!(
                    "{METADATA_SCHEMA} INSERT INTO metadata VALUES('format_version', '{newer}');"
                ),
                format!("has format_version \"{newer}\""),
            ),
            (
                format!("{METADATA_SCHEMA} INSERT INTO metadata VALUES('format_version', 'one');"),
                "has format_version \"one\"".to_owned(),
            ),
            (
                format!("{METADATA_SCHEMA} INSERT INTO metadata VALUES('format_version', '-1');"),
                "has format_version \"-1\"".to_owned(),
            ),
            (
                format!("{METADATA_SCHEMA} INSERT INTO metadata VALUES('license', 'MIT');"),
                "has a metadata table without format_version".to_owned(),
            ),
            (
                "CREATE TABLE metadata(key TEXT, data TEXT);".to_owned(),
                "has an unreadable metadata table".to_owned(),
            ),
        ] {
            let connection = database(&sql);
            let error = check_format_version(&connection, "msime-others.db")
                .unwrap_err()
                .to_string();
            assert!(
                error.starts_with(diagnostics::DICTIONARY_FORMAT_UNSUPPORTED),
                "{error}"
            );
            assert!(error.contains("msime-others.db"), "{error}");
            assert!(error.contains(&detail), "{error}");
            assert!(
                error.ends_with(&format!(
                    "this engine reads format_version {FORMAT_VERSION} or older"
                )),
                "{error}"
            );
        }
    }
}
