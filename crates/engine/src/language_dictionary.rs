//! The read-only dictionaries of the Cantonese and Zhuyin schemes (`cantonese.db`, `zhuyin.db`). They ship beside the resource set rather than inside it, so the engine opens one only when its scheme is activated and treats a missing or unknown file as the scheme being unavailable.
//!
//! dict-builder writes both files with `SCHEMA` and the metadata below; this module is the one definition of that contract.

use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension};

use crate::diagnostics;
use crate::error::{EngineError, Result};

/// The schema version the engine reads. A file with any other `format_version` is refused.
pub const FORMAT_VERSION: u32 = 1;

/// The whole schema. `entries.key` is the syllables of an entry joined by a single space.
pub const SCHEMA: &str = "\
CREATE TABLE metadata(name TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE syllables(syllable TEXT PRIMARY KEY) WITHOUT ROWID;
CREATE TABLE entries(key TEXT NOT NULL, text TEXT NOT NULL, weight INTEGER NOT NULL, PRIMARY KEY(key, text)) WITHOUT ROWID;
CREATE INDEX entries_by_key_weight ON entries(key, weight DESC);
";

/// `metadata` row holding `FORMAT_VERSION` as decimal text.
pub const METADATA_FORMAT_VERSION: &str = "format_version";
/// `metadata` row holding the commit of the source data the file was built from.
pub const METADATA_SOURCE_COMMIT: &str = "source_commit";
/// `metadata` row holding the SPDX identifier of the source data's licence.
pub const METADATA_LICENSE: &str = "license";

/// One row of `entries`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageEntry {
    pub text: String,
    pub weight: i64,
}

/// An open language dictionary whose format version has been checked.
pub struct LanguageDictionary {
    connection: Connection,
}

/// Opens `path` read-only and checks its format version (`LANGUAGE_DICTIONARY_UNAVAILABLE`, `LANGUAGE_DICTIONARY_VERSION_UNSUPPORTED`); never creates the file.
pub fn open_read_only(path: &Path) -> Result<LanguageDictionary> {
    // An empty path would open a private temporary database rather than fail.
    if path.as_os_str().is_empty() {
        return Err(EngineError::failed(
            diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE,
        ));
    }
    let unavailable =
        |_: rusqlite::Error| EngineError::failed(diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE);
    // A shipped resource like the offline glosses (host/glosses.rs), not user data: no symlink policy on the parent path and no busy wait, since nothing writes the file.
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )
    .map_err(unavailable)?;
    connection
        .busy_timeout(std::time::Duration::ZERO)
        .map_err(unavailable)?;
    let dictionary = LanguageDictionary { connection };
    // SQLite opens lazily, so a file that is not a database first fails on this read.
    let version = dictionary
        .metadata(METADATA_FORMAT_VERSION)
        .map_err(|_| EngineError::failed(diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE))?;
    if version.as_deref() != Some(FORMAT_VERSION.to_string().as_str()) {
        return Err(EngineError::failed(
            diagnostics::LANGUAGE_DICTIONARY_VERSION_UNSUPPORTED,
        ));
    }
    Ok(dictionary)
}

impl LanguageDictionary {
    /// The `metadata` value stored under `name`.
    pub fn metadata(&self, name: &str) -> Result<Option<String>> {
        Ok(self
            .connection
            .prepare_cached("SELECT value FROM metadata WHERE name = ?1")?
            .query_row((name,), |row| row.get(0))
            .optional()?)
    }

    /// The entries stored under exactly `key`, heaviest first and by text within a weight, at most `limit`.
    pub fn lookup(&self, key: &str, limit: usize) -> Result<Vec<LanguageEntry>> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare_cached(
            "SELECT text, weight FROM entries WHERE key = ?1 ORDER BY weight DESC, text ASC LIMIT ?2",
        )?;
        let rows = statement.query_map((key, limit), |row| {
            Ok(LanguageEntry {
                text: row.get(0)?,
                weight: row.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Whether `syllable` is in the scheme's syllable inventory.
    pub fn has_syllable(&self, syllable: &str) -> Result<bool> {
        Ok(self
            .connection
            .prepare_cached("SELECT 1 FROM syllables WHERE syllable = ?1")?
            .exists((syllable,))?)
    }

    /// The whole syllable inventory, for a scheme that segments typed letters against it in memory.
    pub fn syllables(&self) -> Result<Vec<String>> {
        let mut statement = self
            .connection
            .prepare_cached("SELECT syllable FROM syllables")?;
        let rows = statement.query_map((), |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The entries whose key completes the last syllable of `prefix`: the key starts with `prefix` and has no syllable boundary after it, so `nei h` finds `nei hou` but not `nei hou aa`. Each comes with its key, heaviest first and by text within a weight, at most `limit`.
    pub fn lookup_completions(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<(String, LanguageEntry)>> {
        // Keys are space-joined syllables, so every key starting with `prefix` sorts at or after it and before `prefix` with its last character incremented.
        let Some(upper) = completion_upper_bound(prefix) else {
            return Ok(Vec::new());
        };
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare_cached(
            "SELECT key, text, weight FROM entries WHERE key >= ?1 AND key < ?2 AND instr(substr(key, length(?1) + 1), ' ') = 0 ORDER BY weight DESC, text ASC LIMIT ?3",
        )?;
        let rows = statement.query_map((prefix, upper, limit), |row| {
            Ok((
                row.get(0)?,
                LanguageEntry {
                    text: row.get(1)?,
                    weight: row.get(2)?,
                },
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

fn completion_upper_bound(prefix: &str) -> Option<String> {
    let last = prefix.chars().next_back()?;
    let next = char::from_u32(u32::from(last) + 1)?;
    let head = &prefix[..prefix.len() - last.len_utf8()];
    let mut upper = String::with_capacity(head.len() + next.len_utf8());
    upper.push_str(head);
    upper.push(next);
    Some(upper)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(path: &Path, format_version: &str) {
        let connection = Connection::open(path).unwrap();
        connection.execute_batch(SCHEMA).unwrap();
        for (name, value) in [
            (METADATA_FORMAT_VERSION, format_version),
            (
                METADATA_SOURCE_COMMIT,
                "259f0e48bba840c3a2e0d117539e96937f3d89bc",
            ),
            (METADATA_LICENSE, "CC-BY-4.0"),
        ] {
            connection
                .execute("INSERT INTO metadata VALUES (?1, ?2)", (name, value))
                .unwrap();
        }
        for syllable in ["nei", "hou"] {
            connection
                .execute("INSERT INTO syllables VALUES (?1)", (syllable,))
                .unwrap();
        }
        for (key, text, weight) in [
            ("nei hou", "你好", 900),
            ("nei hou", "妳好", 40),
            ("nei hou", "你號", 40),
            ("nei", "你", 5000),
        ] {
            connection
                .execute(
                    "INSERT INTO entries VALUES (?1, ?2, ?3)",
                    (key, text, weight),
                )
                .unwrap();
        }
    }

    fn message(error: EngineError) -> String {
        error.to_string()
    }

    #[test]
    fn completion_upper_bound_increments_only_the_last_character() {
        assert_eq!(completion_upper_bound("nei h").as_deref(), Some("nei i"));
        assert_eq!(completion_upper_bound("ㄋㄧˇ").as_deref(), Some("ㄋㄧˈ"));
    }

    #[test]
    fn round_trips_entries_syllables_and_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cantonese.db");
        build(&path, &FORMAT_VERSION.to_string());

        let dictionary = open_read_only(&path).unwrap();
        let entry = |text: &str, weight| LanguageEntry {
            text: text.to_owned(),
            weight,
        };
        assert_eq!(
            dictionary.lookup("nei hou", 10).unwrap(),
            vec![entry("你好", 900), entry("你號", 40), entry("妳好", 40)]
        );
        assert_eq!(
            dictionary.lookup("nei hou", 1).unwrap(),
            vec![entry("你好", 900)]
        );
        assert_eq!(
            dictionary.lookup("nei", 10).unwrap(),
            vec![entry("你", 5000)]
        );
        assert!(dictionary.lookup("ngo", 10).unwrap().is_empty());
        assert!(dictionary.has_syllable("hou").unwrap());
        assert!(!dictionary.has_syllable("ho").unwrap());
        assert_eq!(
            dictionary.metadata(METADATA_LICENSE).unwrap().as_deref(),
            Some("CC-BY-4.0")
        );
        assert_eq!(dictionary.metadata("missing").unwrap(), None);
    }

    #[test]
    fn lists_syllables_and_completes_the_last_syllable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cantonese.db");
        build(&path, &FORMAT_VERSION.to_string());
        let connection = Connection::open(&path).unwrap();
        for (key, text, weight) in [("nei hou aa", "你好呀", 10), ("nei i", "你意", 5)] {
            connection
                .execute(
                    "INSERT INTO entries VALUES (?1, ?2, ?3)",
                    (key, text, weight),
                )
                .unwrap();
        }
        drop(connection);

        let dictionary = open_read_only(&path).unwrap();
        let mut syllables = dictionary.syllables().unwrap();
        syllables.sort();
        assert_eq!(syllables, ["hou", "nei"]);
        let completions = |prefix: &str, limit| {
            dictionary
                .lookup_completions(prefix, limit)
                .unwrap()
                .into_iter()
                .map(|(key, entry)| (key, entry.text))
                .collect::<Vec<_>>()
        };
        let pair = |key: &str, text: &str| (key.to_owned(), text.to_owned());
        assert_eq!(
            completions("nei h", 10),
            [
                pair("nei hou", "你好"),
                pair("nei hou", "你號"),
                pair("nei hou", "妳好")
            ]
        );
        assert_eq!(completions("nei h", 1), [pair("nei hou", "你好")]);
        assert_eq!(completions("ne", 10), [pair("nei", "你")]);
        assert!(completions("nei ho", 0).is_empty());
        assert!(completions("ngo", 10).is_empty());
        assert!(completions("", 10).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn opens_through_a_symlinked_directory() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir(&real).unwrap();
        build(&real.join("zhuyin.db"), &FORMAT_VERSION.to_string());
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let dictionary = open_read_only(&link.join("zhuyin.db")).unwrap();
        assert!(dictionary.has_syllable("nei").unwrap());
    }

    #[test]
    fn refuses_an_unknown_format_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zhuyin.db");
        build(&path, "2");
        assert_eq!(
            message(open_read_only(&path).err().unwrap()),
            diagnostics::LANGUAGE_DICTIONARY_VERSION_UNSUPPORTED
        );
    }

    #[test]
    fn refuses_missing_empty_and_foreign_files_without_creating_them() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("cantonese.db");
        assert_eq!(
            message(open_read_only(&missing).err().unwrap()),
            diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
        );
        assert!(!missing.exists());
        assert_eq!(
            message(open_read_only(Path::new("")).err().unwrap()),
            diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
        );
        let foreign = dir.path().join("foreign.db");
        std::fs::write(&foreign, b"not a database").unwrap();
        assert_eq!(
            message(open_read_only(&foreign).err().unwrap()),
            diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
        );
        let no_metadata = dir.path().join("no_metadata.db");
        Connection::open(&no_metadata)
            .unwrap()
            .execute_batch("CREATE TABLE other(x)")
            .unwrap();
        assert_eq!(
            message(open_read_only(&no_metadata).err().unwrap()),
            diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE
        );
    }
}
