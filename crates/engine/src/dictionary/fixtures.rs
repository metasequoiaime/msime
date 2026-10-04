//! Test fixtures: tiny `msime-pinyin.db` / `msime-english.db` files with the shipped schemas (data-formats.md §4.1, §5).

use std::path::{Path, PathBuf};

use rusqlite::Connection;

/// `(table, key, value, weight)`; `jp` is derived the way the builder does, one first letter per syllable.
pub type PinyinRow<'a> = (&'a str, &'a str, &'a str, i64);

pub fn pinyin_db(directory: &Path, rows: &[PinyinRow<'_>]) -> PathBuf {
    let path = directory.join("msime-pinyin.db");
    let connection = Connection::open(&path).unwrap();
    for (table, key, value, weight) in rows {
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
                &format!(
                    "INSERT INTO \"{table}\" (key, jp, value, weight) VALUES (?1, ?2, ?3, ?4)"
                ),
                (key, jp, value, weight),
            )
            .unwrap();
    }
    path
}

/// `(word, display, weight)` plus en→zh and zh→en glosses, in the v3 shape.
pub fn english_db(
    directory: &Path,
    words: &[(&str, &str, i64)],
    en_zh: &[(&str, &str)],
    zh_en: &[(&str, &str)],
) -> PathBuf {
    let path = directory.join("msime-english.db");
    super::english::ensure_english_schema(&path).unwrap();
    let connection = Connection::open(&path).unwrap();
    for word in words {
        connection
            .execute(
                "INSERT INTO english_words(word,display,weight) VALUES(?1,?2,?3)",
                *word,
            )
            .unwrap();
    }
    for gloss in en_zh {
        connection
            .execute(
                "INSERT INTO en_zh_glosses(english,chinese_gloss) VALUES(?1,?2)",
                *gloss,
            )
            .unwrap();
    }
    for gloss in zh_en {
        connection
            .execute(
                "INSERT INTO zh_en_glosses(chinese,english_gloss) VALUES(?1,?2)",
                *gloss,
            )
            .unwrap();
    }
    path
}

/// The real resource set for the eval-backed tests, or `None` with the reason printed.
pub fn eval_resources(test: &str) -> Option<PathBuf> {
    match std::env::var_os("MSIME_EVAL_RESOURCES") {
        Some(path) if !path.is_empty() => Some(PathBuf::from(path)),
        _ => {
            println!("{test}: skipped, MSIME_EVAL_RESOURCES is not set");
            None
        }
    }
}
