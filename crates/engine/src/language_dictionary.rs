//! The read-only dictionaries of the Cantonese, Zhuyin and Stroke schemes (`msime-cantonese.db`, `msime-zhuyin.db`, `msime-stroke.db`). They ship beside the resource set rather than inside it, so the engine opens one only when its scheme is activated and treats a missing or unknown file as the scheme being unavailable.
//!
//! dict-builder writes every file with `SCHEMA` and the metadata below; this module is the one definition of that contract.

use std::path::Path;

use rusqlite::{types::Type, Connection, OpenFlags, OptionalExtension};

use crate::diagnostics;
use crate::error::{EngineError, Result};

/// The schema version the engine reads. A file with any other `format_version` is refused.
pub const FORMAT_VERSION: u32 = 1;

/// The whole schema. `entries.key` is the syllables of an entry joined by a single space; in `msime-stroke.db` it is the character's stroke code (`hspnz` letters, no spaces).
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
    let path = crate::paths::sqlite_path_no_follow_allow_parent_symlinks(path)
        .map_err(|_| EngineError::failed(diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE))?;
    let connection = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NOFOLLOW
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
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
        let mut result = query_capacity(limit).map_or_else(Vec::new, Vec::with_capacity);
        self.lookup_into(key, limit, &mut result)?;
        Ok(result)
    }

    /// 将精确键的查询结果写入已有缓冲，保留其中字符串的容量。
    pub fn lookup_into(
        &self,
        key: &str,
        limit: usize,
        result: &mut Vec<LanguageEntry>,
    ) -> Result<()> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare_cached(
            "SELECT text, weight FROM entries WHERE key = ?1 ORDER BY weight DESC, text ASC LIMIT ?2",
        )?;
        let mut length = 0;
        let mut rows = statement.query((key, limit))?;
        while let Some(row) = rows.next()? {
            let text = row.get_ref(0)?.as_str().map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error))
            })?;
            let weight = row.get(1)?;
            if let Some(entry) = result.get_mut(length) {
                entry.text.clear();
                entry.text.push_str(text);
                entry.weight = weight;
            } else {
                result.push(LanguageEntry {
                    text: text.to_owned(),
                    weight,
                });
            }
            length += 1;
        }
        result.truncate(length);
        Ok(())
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
        let mut result = query_capacity(limit).map_or_else(Vec::new, Vec::with_capacity);
        self.lookup_completions_into(prefix, limit, &mut result)?;
        Ok(result)
    }

    /// 将补全查询结果写入已有缓冲，保留键和值字符串的容量。
    pub fn lookup_completions_into(
        &self,
        prefix: &str,
        limit: usize,
        result: &mut Vec<(String, LanguageEntry)>,
    ) -> Result<()> {
        // Keys are space-joined syllables, so every key starting with `prefix` sorts at or after it and before `prefix` with its last character incremented.
        let Some(upper) = completion_upper_bound(prefix) else {
            result.clear();
            return Ok(());
        };
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare_cached(
            "SELECT key, text, weight FROM entries WHERE key >= ?1 AND key < ?2 AND instr(substr(key, length(?1) + 1), ' ') = 0 ORDER BY weight DESC, text ASC LIMIT ?3",
        )?;
        let mut length = 0;
        let mut rows = statement.query((prefix, upper, limit))?;
        while let Some(row) = rows.next()? {
            let key = row.get_ref(0)?.as_str().map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error))
            })?;
            let text = row.get_ref(1)?.as_str().map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(1, Type::Text, Box::new(error))
            })?;
            let weight = row.get(2)?;
            if let Some((existing_key, entry)) = result.get_mut(length) {
                existing_key.clear();
                existing_key.push_str(key);
                entry.text.clear();
                entry.text.push_str(text);
                entry.weight = weight;
            } else {
                result.push((
                    key.to_owned(),
                    LanguageEntry {
                        text: text.to_owned(),
                        weight,
                    },
                ));
            }
            length += 1;
        }
        result.truncate(length);
        Ok(())
    }

    /// The entries whose key matches `pattern`, where `wildcard` stands for any one character other than a space and every other character for itself. With `completions` the pattern only has to match the start of the key, as in `lookup_completions`, and the rest of the key may not hold a syllable boundary; without it the key must match the whole pattern. Each comes with its key, heaviest first and by text within a weight, at most `limit`. The literal characters before the first wildcard bound the scan to their key range, so only a leading wildcard reads the whole table.
    pub fn lookup_pattern(
        &self,
        pattern: &str,
        wildcard: char,
        completions: bool,
        limit: usize,
    ) -> Result<Vec<(String, LanguageEntry)>> {
        if pattern.is_empty() {
            return Ok(Vec::new());
        }
        let glob = glob_pattern(pattern, wildcard, completions);
        let length = i64::try_from(pattern.chars().count()).unwrap_or(i64::MAX);
        let literal = pattern.split(wildcard).next().unwrap_or_default();
        let requested_limit = limit;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let read = |row: &rusqlite::Row<'_>| {
            Ok((
                row.get(0)?,
                LanguageEntry {
                    text: row.get(1)?,
                    weight: row.get(2)?,
                },
            ))
        };
        let mut result = query_capacity(requested_limit).map_or_else(Vec::new, Vec::with_capacity);
        // 字面前缀为空（通配符打头）时没有可用的键范围，只能整表按 GLOB 过滤。
        match completion_upper_bound(literal) {
            Some(upper) => {
                let mut statement = self.connection.prepare_cached(
                    "SELECT key, text, weight FROM entries WHERE key >= ?1 AND key < ?2 AND key GLOB ?3 AND instr(substr(key, ?4 + 1), ' ') = 0 ORDER BY weight DESC, text ASC LIMIT ?5",
                )?;
                for row in statement.query_map((literal, upper, glob, length, limit), read)? {
                    result.push(row?);
                }
            }
            None => {
                let mut statement = self.connection.prepare_cached(
                    "SELECT key, text, weight FROM entries WHERE key GLOB ?1 AND instr(substr(key, ?2 + 1), ' ') = 0 ORDER BY weight DESC, text ASC LIMIT ?3",
                )?;
                for row in statement.query_map((glob, length, limit), read)? {
                    result.push(row?);
                }
            }
        }
        Ok(result)
    }

    /// 键按位置逐段取自 `positions[i]`（以单个空格连接）的词条，各带自己的键；按权重从重到轻，再按文字、键排序，至多 `limit` 条。每个位置都只有一个读音时就是 `lookup` 对连接后的键查询，结果与它逐条相同。
    pub fn lookup_readings(
        &self,
        positions: &[&[String]],
        limit: usize,
    ) -> Result<Vec<(String, LanguageEntry)>> {
        let Some((first, rest)) = positions.split_first() else {
            return Ok(Vec::new());
        };
        if positions.iter().all(|readings| readings.len() == 1) {
            let key = join_single_readings(positions);
            return Ok(self
                .lookup(&key, limit)?
                .into_iter()
                .map(|entry| (key.clone(), entry))
                .collect());
        }
        let mut result = Vec::new();
        if rest.is_empty() {
            for reading in first.iter() {
                result.extend(
                    self.lookup(reading, limit)?
                        .into_iter()
                        .map(|entry| (reading.clone(), entry)),
                );
            }
        } else {
            // 后面各位置拼成一个 GLOB，配合首个读音的键范围只扫以它开头的多音节词；不加 SQL `LIMIT`，因为 GLOB 只是粗筛，精确的校验在 Rust 里做，提前截断可能丢掉正确的行。
            let tail = readings_glob(rest);
            let mut statement = self.connection.prepare_cached(
                "SELECT key, text, weight FROM entries WHERE key >= ?1 AND key < ?2 AND key GLOB ?3",
            )?;
            for reading in first.iter() {
                let lower = format!("{reading} ");
                let upper = format!("{reading}!");
                let mut glob = String::with_capacity(reading.len() * 2 + 1 + tail.len());
                push_glob_literal(&mut glob, reading);
                glob.push(' ');
                glob.push_str(&tail);
                let rows = statement.query_map((lower, upper, glob), |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        LanguageEntry {
                            text: row.get(1)?,
                            weight: row.get(2)?,
                        },
                    ))
                })?;
                for row in rows {
                    let (key, entry) = row?;
                    if key_matches(&key, positions) {
                        result.push((key, entry));
                    }
                }
            }
        }
        result.sort_by(|(left_key, left), (right_key, right)| {
            right
                .weight
                .cmp(&left.weight)
                .then_with(|| left.text.cmp(&right.text))
                .then_with(|| left_key.cmp(right_key))
        });
        result.truncate(limit);
        Ok(result)
    }
}

fn join_single_readings(positions: &[&[String]]) -> String {
    let capacity = positions
        .iter()
        .map(|readings| readings[0].len())
        .sum::<usize>()
        .saturating_add(positions.len().saturating_sub(1));
    let mut key = String::with_capacity(capacity);
    for (index, readings) in positions.iter().enumerate() {
        if index != 0 {
            key.push(' ');
        }
        key.push_str(&readings[0]);
    }
    key
}

/// `positions` 各位置的 GLOB，位置之间用空格。只有一个读音的位置按字面匹配；多个读音且字数相同时，逐个字符下标写出该下标上出现过的字符组成的字符类；字数不同、或字符类里会混进 GLOB 自己的 `]` `^` `-` 时退成任意字符，交给 `key_matches` 精确校验。
fn readings_glob(positions: &[&[String]]) -> String {
    let mut glob = String::new();
    for (index, readings) in positions.iter().enumerate() {
        if index > 0 {
            glob.push(' ');
        }
        if let [reading] = readings {
            push_glob_literal(&mut glob, reading);
            continue;
        }
        let lengths: Vec<usize> = readings
            .iter()
            .map(|reading| reading.chars().count())
            .collect();
        if lengths.windows(2).any(|pair| pair[0] != pair[1]) {
            glob.push('*');
            continue;
        }
        for position in 0..lengths.first().copied().unwrap_or_default() {
            let mut class: Vec<char> = readings
                .iter()
                .filter_map(|reading| reading.chars().nth(position))
                .collect();
            class.sort_unstable();
            class.dedup();
            match class.as_slice() {
                [only] => push_glob_literal(&mut glob, &only.to_string()),
                _ if class
                    .iter()
                    .any(|character| matches!(character, ']' | '^' | '-')) =>
                {
                    glob.push('?');
                }
                _ => {
                    glob.push('[');
                    glob.extend(class);
                    glob.push(']');
                }
            }
        }
    }
    glob
}

/// 按字面匹配 `text`：GLOB 的元字符写成只含它自己的字符类。
fn push_glob_literal(glob: &mut String, text: &str) {
    for character in text.chars() {
        match character {
            '*' | '?' | '[' => {
                glob.push('[');
                glob.push(character);
                glob.push(']');
            }
            _ => glob.push(character),
        }
    }
}

/// `key` 的每个音节是否都是对应位置允许的读音，且音节数与位置数相同。
fn key_matches(key: &str, positions: &[&[String]]) -> bool {
    let mut syllables = key.split(' ');
    positions.iter().all(|readings| {
        syllables
            .next()
            .is_some_and(|syllable| readings.iter().any(|reading| reading == syllable))
    }) && syllables.next().is_none()
}

/// `pattern` as a GLOB: the wildcard becomes `[^ ]`, so it never matches a syllable boundary, GLOB's own metacharacters are matched literally, and a pattern for completions ends in `*`.
fn glob_pattern(pattern: &str, wildcard: char, completions: bool) -> String {
    let mut glob = String::with_capacity(pattern.len() * 4 + 1);
    for character in pattern.chars() {
        match character {
            _ if character == wildcard => glob.push_str("[^ ]"),
            '*' | '?' | '[' => {
                glob.push('[');
                glob.push(character);
                glob.push(']');
            }
            _ => glob.push(character),
        }
    }
    if completions {
        glob.push('*');
    }
    glob
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

fn query_capacity(limit: usize) -> Option<usize> {
    (limit < i32::MAX as usize).then_some(limit)
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
        let path = dir.path().join("msime-cantonese.db");
        build(&path, &FORMAT_VERSION.to_string());

        let dictionary = open_read_only(&path).unwrap();
        let entry = |text: &str, weight| LanguageEntry {
            text: text.to_owned(),
            weight,
        };
        let entries = dictionary.lookup("nei hou", 10).unwrap();
        assert_eq!(entries.capacity(), 10);
        assert_eq!(
            entries,
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
        let path = dir.path().join("msime-cantonese.db");
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
            let rows = dictionary.lookup_completions(prefix, limit).unwrap();
            (
                rows.capacity(),
                rows.into_iter()
                    .map(|(key, entry)| (key, entry.text))
                    .collect::<Vec<_>>(),
            )
        };
        let pair = |key: &str, text: &str| (key.to_owned(), text.to_owned());
        let (capacity, rows) = completions("nei h", 10);
        assert_eq!(capacity, 10);
        assert_eq!(
            rows,
            [
                pair("nei hou", "你好"),
                pair("nei hou", "你號"),
                pair("nei hou", "妳好")
            ]
        );
        assert_eq!(completions("nei h", 1).1, [pair("nei hou", "你好")]);
        assert_eq!(completions("ne", 10).1, [pair("nei", "你")]);
        assert!(completions("nei ho", 0).1.is_empty());
        assert!(completions("ngo", 10).1.is_empty());
        assert!(completions("", 10).1.is_empty());
    }

    #[test]
    fn glob_pattern_maps_the_wildcard_and_escapes_glob_metacharacters() {
        assert_eq!(glob_pattern("hxs", 'x', false), "h[^ ]s");
        assert_eq!(glob_pattern("hxs", 'x', true), "h[^ ]s*");
        assert_eq!(glob_pattern("a*?[", 'x', false), "a[*][?][[]");
    }

    // 合成的笔画数据：键是笔顺字母串。通配符可以在中间、末尾或开头；补全只匹配前缀，精确只匹配同长度的键；带空格的键（将来的词组）不进单字结果。
    #[test]
    fn matches_wildcard_patterns_within_the_literal_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("msime-stroke.db");
        build(&path, &FORMAT_VERSION.to_string());
        let connection = Connection::open(&path).unwrap();
        for (key, text, weight) in [
            ("hs", "十", 800),
            ("hh", "二", 900),
            ("hhh", "三", 700),
            ("hsh", "土", 600),
            ("hpn", "大", 950),
            ("sh", "上", 650),
            ("hs hs", "十十", 5000),
            ("a*b", "星", 1),
        ] {
            connection
                .execute(
                    "INSERT INTO entries VALUES (?1, ?2, ?3)",
                    (key, text, weight),
                )
                .unwrap();
        }
        drop(connection);

        let dictionary = open_read_only(&path).unwrap();
        let texts = |pattern: &str, completions, limit| {
            let rows = dictionary
                .lookup_pattern(pattern, 'x', completions, limit)
                .unwrap();
            (
                rows.capacity(),
                rows.into_iter()
                    .map(|(key, entry)| format!("{key}:{}", entry.text))
                    .collect::<Vec<_>>(),
            )
        };
        let (capacity, rows) = texts("hx", false, 10);
        assert_eq!(capacity, 10);
        assert_eq!(rows, ["hh:二", "hs:十"]);
        assert_eq!(
            texts("hx", true, 10).1,
            ["hpn:大", "hh:二", "hs:十", "hhh:三", "hsh:土"]
        );
        assert_eq!(texts("hx", true, 2).1, ["hpn:大", "hh:二"]);
        assert_eq!(texts("hxh", false, 10).1, ["hhh:三", "hsh:土"]);
        assert_eq!(texts("xh", false, 10).1, ["hh:二", "sh:上"]);
        assert_eq!(
            texts("xxx", false, 10).1,
            ["nei:你", "hpn:大", "hhh:三", "hsh:土", "a*b:星"]
        );
        assert_eq!(texts("hs", false, 10).1, ["hs:十"]);
        assert_eq!(texts("hsx", true, 10).1, ["hsh:土"]);
        // GLOB 的元字符按字面匹配。
        assert_eq!(texts("a*b", false, 10).1, ["a*b:星"]);
        assert!(texts("a?b", false, 10).1.is_empty());
        assert!(texts("", true, 10).1.is_empty());
        assert!(texts("xxxxxx", true, 10).1.is_empty());
        assert!(texts("hx", true, 0).1.is_empty());
    }

    fn readings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    // 合成的注音数据：九键下一个位置有多个读音，查询只返回逐段都属于对应位置、且音节数一致的键。
    #[test]
    fn looks_up_keys_whose_syllables_fall_in_each_position() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("msime-zhuyin.db");
        build(&path, &FORMAT_VERSION.to_string());
        let connection = Connection::open(&path).unwrap();
        for (key, text, weight) in [
            ("ㄋㄧˇ", "你", 1000),
            ("ㄋㄧˇ", "妳", 300),
            ("ㄌㄧˇ", "李", 1200),
            ("ㄌㄧˇ", "你", 5),
            ("ㄋㄧˇ ㄏㄠˇ", "你好", 500),
            ("ㄌㄧˇ ㄏㄠˇ", "李好", 500),
            ("ㄋㄧˇ ㄏㄠˋ", "你號", 900),
            ("ㄋㄧ ㄏㄠˇ", "妮好", 900),
            ("ㄋㄧˇ ㄏㄠˇ ㄇㄚ˙", "你好嗎", 900),
            ("ㄋㄧˇ ㄏㄨㄚˇ", "你畫", 900),
            ("ㄌㄧˇ ㄎㄠˇ", "李考", 400),
            ("ㄋㄧˇ ㄍㄠˇ", "你搞", 700),
        ] {
            connection
                .execute(
                    "INSERT INTO entries VALUES (?1, ?2, ?3)",
                    (key, text, weight),
                )
                .unwrap();
        }
        drop(connection);
        let dictionary = open_read_only(&path).unwrap();
        let ni_li = readings(&["ㄋㄧˇ", "ㄌㄧˇ"]);
        let hao = readings(&["ㄏㄠˇ"]);
        let gao_kao_hao = readings(&["ㄍㄠˇ", "ㄎㄠˇ", "ㄏㄠˇ"]);
        let rows = |positions: &[&[String]], limit| {
            dictionary
                .lookup_readings(positions, limit)
                .unwrap()
                .into_iter()
                .map(|(key, entry)| format!("{key}:{}:{}", entry.text, entry.weight))
                .collect::<Vec<_>>()
        };

        // 声调不符（ㄏㄠˋ）、首音节一声（ㄋㄧ）、多一个音节、韵母字数不同（ㄏㄨㄚˇ）的键都被排除；同权重按文字再按键排序。
        assert_eq!(
            rows(&[&ni_li, &hao], usize::MAX),
            ["ㄋㄧˇ ㄏㄠˇ:你好:500", "ㄌㄧˇ ㄏㄠˇ:李好:500"]
        );
        // 后面的位置也可以有多个读音。
        assert_eq!(
            rows(&[&ni_li, &gao_kao_hao], usize::MAX),
            [
                "ㄋㄧˇ ㄍㄠˇ:你搞:700",
                "ㄋㄧˇ ㄏㄠˇ:你好:500",
                "ㄌㄧˇ ㄏㄠˇ:李好:500",
                "ㄌㄧˇ ㄎㄠˇ:李考:400",
            ]
        );
        assert_eq!(rows(&[&ni_li, &gao_kao_hao], 1), ["ㄋㄧˇ ㄍㄠˇ:你搞:700"]);
        // 单个位置多个读音：按权重合并，同一个字可以在两个读音下各出现一次。
        assert_eq!(
            rows(&[&ni_li], usize::MAX),
            [
                "ㄌㄧˇ:李:1200",
                "ㄋㄧˇ:你:1000",
                "ㄋㄧˇ:妳:300",
                "ㄌㄧˇ:你:5"
            ]
        );
        assert_eq!(rows(&[&ni_li], 2), ["ㄌㄧˇ:李:1200", "ㄋㄧˇ:你:1000"]);
        // 每个位置只有一个读音时与 `lookup` 逐条相同。
        let ni = readings(&["ㄋㄧˇ"]);
        let expected: Vec<String> = dictionary
            .lookup("ㄋㄧˇ ㄏㄠˇ", usize::MAX)
            .unwrap()
            .into_iter()
            .map(|entry| format!("ㄋㄧˇ ㄏㄠˇ:{}:{}", entry.text, entry.weight))
            .collect();
        assert_eq!(rows(&[&ni, &hao], usize::MAX), expected);
        assert_eq!(rows(&[&ni], 1), ["ㄋㄧˇ:你:1000"]);
        assert!(rows(&[], 10).is_empty());
        assert!(rows(&[&hao, &ni_li], 10).is_empty());
    }

    #[test]
    fn joins_single_readings_in_order() {
        let ni = readings(&["ㄋㄧˇ"]);
        let hao = readings(&["ㄏㄠˇ"]);
        assert_eq!(join_single_readings(&[&ni, &hao]), "ㄋㄧˇ ㄏㄠˇ");
    }

    #[test]
    fn readings_glob_writes_literals_classes_and_wildcards() {
        let ni_li = readings(&["ㄋㄧˇ", "ㄌㄧˇ"]);
        let hao = readings(&["ㄏㄠˇ"]);
        let uneven = readings(&["ㄏㄠˇ", "ㄏㄨㄚˇ"]);
        let meta = readings(&["a*", "b-"]);
        assert_eq!(readings_glob(&[&hao]), "ㄏㄠˇ");
        assert_eq!(readings_glob(&[&hao, &ni_li]), "ㄏㄠˇ [ㄋㄌ]ㄧˇ");
        assert_eq!(readings_glob(&[&uneven, &hao]), "* ㄏㄠˇ");
        assert_eq!(readings_glob(&[&meta]), "[ab]?");
        assert_eq!(readings_glob(&[&readings(&["a?"])]), "a[?]");
        assert!(key_matches("ㄋㄧˇ ㄏㄠˇ", &[&ni_li, &hao]));
        assert!(!key_matches("ㄋㄧˇ", &[&ni_li, &hao]));
        assert!(!key_matches("ㄋㄧˇ ㄏㄠˇ ㄏㄠˇ", &[&ni_li, &hao]));
    }

    #[cfg(unix)]
    #[test]
    fn opens_through_a_symlinked_directory() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir(&real).unwrap();
        build(&real.join("msime-zhuyin.db"), &FORMAT_VERSION.to_string());
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let dictionary = open_read_only(&link.join("msime-zhuyin.db")).unwrap();
        assert!(dictionary.has_syllable("nei").unwrap());
    }

    #[test]
    fn refuses_an_unknown_format_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("msime-zhuyin.db");
        build(&path, "2");
        assert_eq!(
            message(open_read_only(&path).err().unwrap()),
            diagnostics::LANGUAGE_DICTIONARY_VERSION_UNSUPPORTED
        );
    }

    #[test]
    fn refuses_missing_empty_and_foreign_files_without_creating_them() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("msime-cantonese.db");
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
