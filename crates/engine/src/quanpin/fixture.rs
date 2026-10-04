//! Test fixtures: a throwaway runtime directory with a `msime-pinyin.db` built row by row, in the shape the dictionary builder ships (`key`, `jp`, `value`, `weight`).

use std::path::PathBuf;

use rusqlite::{params, Connection};
use tempfile::TempDir;

use crate::assets::{MAIN_DICTIONARY, USER_JOURNAL};
use crate::format::build_table_name;
use crate::paths::RuntimePaths;
use crate::pinyin::segment::split_segments;

pub struct Fixture {
    _root: TempDir,
    pub paths: RuntimePaths,
}

impl Fixture {
    pub fn new() -> Self {
        let root = tempfile::tempdir().expect("temporary fixture directory");
        let dictionaries = root.path().join("dictionary");
        let user = root.path().join("user");
        std::fs::create_dir_all(&dictionaries).expect("fixture dictionary directory");
        std::fs::create_dir_all(&user).expect("fixture user directory");
        let paths = RuntimePaths {
            resources: dictionaries.clone(),
            user_data: user.clone(),
            cache: user,
            dictionaries,
        };
        Connection::open(paths.dictionary(MAIN_DICTIONARY)).expect("fixture dictionary");
        Self { _root: root, paths }
    }

    pub fn database(&self) -> PathBuf {
        self.paths.dictionary(MAIN_DICTIONARY)
    }

    pub fn journal(&self) -> PathBuf {
        self.paths.user(USER_JOURNAL)
    }

    pub fn connection(&self) -> Connection {
        Connection::open(self.database()).expect("fixture dictionary")
    }

    /// An empty table, for a key the test writes to later.
    pub fn table(&self, key: &str) -> String {
        let table = build_table_name(&split_segments(key)).expect("fixture key names a table");
        self.connection()
            .execute_batch(&format!(
                "CREATE TABLE IF NOT EXISTS \"{table}\"(key TEXT, jp TEXT, value TEXT, weight INTEGER);"
            ))
            .expect("fixture table");
        table
    }

    pub fn insert(&self, key: &str, word: &str, weight: i64) -> &Self {
        let table = self.table(key);
        let jp: String = split_segments(key)
            .iter()
            .filter_map(|segment| segment.chars().next())
            .collect();
        self.connection()
            .execute(
                &format!("INSERT INTO \"{table}\"(key, jp, value, weight) VALUES (?1, ?2, ?3, ?4)"),
                params![key, jp, word, weight],
            )
            .expect("fixture row");
        self
    }

    pub fn weight(&self, key: &str, word: &str) -> Option<i64> {
        let table = build_table_name(&split_segments(key))?;
        self.connection()
            .query_row(
                &format!("SELECT weight FROM \"{table}\" WHERE key = ?1 AND value = ?2"),
                params![key, word],
                |row| row.get(0),
            )
            .ok()
    }
}
