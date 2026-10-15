//! The @ mode's name list: `<root>/mentions.json`, where `root` is the plugins directory.
//!
//! The list is the user's own, typed in on the settings page. It is kept out of the preferences document on purpose: that document is copied between a host's processes and read field by field for account sync, and names of people and places belong to neither. Nothing here reads a system address book or asks for a location.
//!
//! Writers serialize on `mentions.lock` and replace the document with an atomic rename, so an input process can read it at any moment without a lock and see either the old list or the new one.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};

/// Entries in the list: the Engine keeps no more.
pub const MAX_ENTRIES: usize = 1_000;
/// A name, in UTF-16 units: the Windows candidate pipe's text field.
pub const MAX_TEXT_UTF16: usize = 199;
/// A key, in bytes.
pub const MAX_KEY_BYTES: usize = 64;
/// The document. A full list of the longest names stays well inside.
pub const MAX_DOCUMENT_BYTES: u64 = 1024 * 1024;

const FORMAT_VERSION: u32 = 1;
const DOCUMENT: &str = "mentions.json";
const LOCK_FILE: &str = "mentions.lock";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MentionEntry {
    /// What is committed.
    pub text: String,
    /// Lowercase pinyin syllables joined by `'` (`zhang'san`) that the letters after `@` are matched against. Empty lets the host derive it: an ASCII name matches by its own letters, and a host fills in the reading of a Chinese one from the Engine's dictionary.
    #[serde(default)]
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    format_version: u32,
    entries: Vec<MentionEntry>,
}

#[derive(Debug, thiserror::Error)]
pub enum MentionError {
    /// An entry breaks a rule; the text says which.
    #[error("mention_invalid: {0}")]
    Invalid(String),
    #[error("mention_format")]
    Format,
    #[error("mention_storage")]
    Storage,
    #[error("mention_io: {0}")]
    Io(#[from] std::io::Error),
}

/// Why `entry` cannot be in the list, if it cannot.
pub fn validate_entry(entry: &MentionEntry) -> Result<(), String> {
    // A name or a place is one line: a candidate row cannot show a line break or tab it would commit.
    if entry.text.trim().is_empty()
        || crate::text::has_disallowed_control_with_allowed(&entry.text, &[])
        || !crate::text::is_bounded_utf16(&entry.text, MAX_TEXT_UTF16)
    {
        return Err(format!("「{}」为空或太长", entry.text));
    }
    let key = &entry.key;
    if key.len() > MAX_KEY_BYTES
        || (!key.is_empty()
            && !key.split('\'').all(|syllable| {
                !syllable.is_empty() && syllable.bytes().all(|b| b.is_ascii_lowercase())
            }))
    {
        return Err(format!(
            "「{}」的拼音只能是小写字母，音节之间用 ' 分隔",
            entry.text
        ));
    }
    Ok(())
}

pub struct MentionStore {
    directory: PathBuf,
}

struct MentionLock {
    directory: crate::file_lock::PrivateDirectory,
    _lock: File,
}

impl MentionStore {
    /// `directory` is the plugins root.
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }

    fn path(&self) -> PathBuf {
        self.directory.join(DOCUMENT)
    }

    /// The saved list; empty before anything was saved.
    pub fn load(&self) -> Result<Vec<MentionEntry>, MentionError> {
        let path = self.path();
        crate::storage::reject_symlink(&path).map_err(|_| MentionError::Storage)?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        if !metadata.is_file() {
            return Err(MentionError::Storage);
        }
        let bytes = crate::bounded_io::read_bounded_file(
            crate::storage::open_private_file_in(&path)?,
            MAX_DOCUMENT_BYTES,
            || MentionError::Format,
        )?;
        let document: Document =
            serde_json::from_slice(&bytes).map_err(|_| MentionError::Format)?;
        if document.format_version != FORMAT_VERSION {
            return Err(MentionError::Format);
        }
        check(&document.entries).map_err(|_| MentionError::Format)?;
        Ok(document.entries)
    }

    /// Replace the list. Every entry is checked, and a name may appear once.
    pub fn save(&self, entries: &[MentionEntry]) -> Result<(), MentionError> {
        check(entries).map_err(MentionError::Invalid)?;
        if !crate::storage::create_directory_and_check(&self.directory)
            .map_err(|_| MentionError::Storage)?
        {
            return Err(MentionError::Storage);
        }
        let guard = self.lock()?;
        self.write_locked(&guard, entries)
    }

    fn lock(&self) -> Result<MentionLock, MentionError> {
        let directory = crate::file_lock::open_private_directory(&self.directory)
            .map_err(|_| MentionError::Storage)?;
        let lock = crate::file_lock::open_private_lock_file_at(
            &directory,
            std::ffi::OsStr::new(LOCK_FILE),
        )
        .map_err(|_| MentionError::Storage)?;
        crate::file_lock::exclusive(&lock)?;
        Ok(MentionLock {
            directory,
            _lock: lock,
        })
    }

    fn write_locked(
        &self,
        lock: &MentionLock,
        entries: &[MentionEntry],
    ) -> Result<(), MentionError> {
        let bytes = serde_json::to_vec_pretty(&Document {
            format_version: FORMAT_VERSION,
            entries: entries.to_vec(),
        })
        .map_err(|_| MentionError::Format)?;
        if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err(MentionError::Invalid("名单太大".into()));
        }
        crate::file_lock::write_private_file_at(
            &lock.directory,
            std::ffi::OsStr::new(DOCUMENT),
            &bytes,
        )?;
        Ok(())
    }
}

fn check(entries: &[MentionEntry]) -> Result<(), String> {
    if entries.len() > MAX_ENTRIES {
        return Err("名单里的条目太多".into());
    }
    let mut texts = HashSet::with_capacity(entries.len());
    for entry in entries {
        validate_entry(entry)?;
        if !texts.insert(entry.text.as_str()) {
            return Err(format!("「{}」重复了", entry.text));
        }
    }
    Ok(())
}

/// Where the list lives for a plugins root, for hosts that watch the file for changes.
pub fn document_path(root: &Path) -> PathBuf {
    root.join(DOCUMENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(text: &str, key: &str) -> MentionEntry {
        MentionEntry {
            text: text.into(),
            key: key.into(),
        }
    }

    #[test]
    fn a_missing_list_is_empty_and_a_saved_one_round_trips() {
        let directory = tempfile::tempdir().unwrap();
        let store = MentionStore::new(directory.path().join("plugins"));
        assert!(store.load().unwrap().is_empty());
        let entries = vec![
            entry("张三", "zhang'san"),
            entry("Alice", ""),
            entry("北京市朝阳区", ""),
        ];
        store.save(&entries).unwrap();
        assert_eq!(store.load().unwrap(), entries);
        assert!(document_path(&directory.path().join("plugins")).is_file());
        store.save(&[]).unwrap();
        assert!(store.load().unwrap().is_empty());
    }

    #[test]
    fn entries_break_no_rule_the_engine_would_drop_them_for() {
        for (text, key) in [
            ("", ""),
            ("   ", ""),
            ("a\u{7}b", ""),
            ("张三\nrm", ""),
            ("a\tb", ""),
            ("张三", "Zhang"),
            ("张三", "zhang''san"),
            ("张三", "'zhang"),
            ("张三", "zhang san"),
            ("张三", "zhang1"),
        ] {
            assert!(
                validate_entry(&entry(text, key)).is_err(),
                "{text:?} {key:?}"
            );
        }
        assert!(validate_entry(&entry(&"名".repeat(MAX_TEXT_UTF16), "")).is_ok());
        assert!(validate_entry(&entry(&"名".repeat(MAX_TEXT_UTF16 + 1), "")).is_err());
        assert!(validate_entry(&entry("长", &"a".repeat(MAX_KEY_BYTES + 1))).is_err());

        let directory = tempfile::tempdir().unwrap();
        let store = MentionStore::new(directory.path());
        assert!(matches!(
            store.save(&[entry("张三", ""), entry("张三", "zhang'san")]),
            Err(MentionError::Invalid(_))
        ));
        let many: Vec<_> = (0..=MAX_ENTRIES)
            .map(|index| entry(&format!("name{index}"), ""))
            .collect();
        assert!(matches!(store.save(&many), Err(MentionError::Invalid(_))));
        assert!(store.save(&many[..MAX_ENTRIES]).is_ok());
        assert_eq!(store.load().unwrap().len(), MAX_ENTRIES);
    }

    #[test]
    fn a_damaged_or_foreign_document_is_refused_rather_than_trusted() {
        let directory = tempfile::tempdir().unwrap();
        let store = MentionStore::new(directory.path());
        for document in [
            "not json",
            r#"{"format_version":2,"entries":[]}"#,
            r#"{"format_version":1,"entries":[{"text":"x","key":"","phone":"1"}]}"#,
            r#"{"format_version":1,"entries":[{"text":"","key":""}]}"#,
            r#"{"format_version":1,"entries":[],"extra":true}"#,
        ] {
            fs::write(directory.path().join(DOCUMENT), document).unwrap();
            assert!(
                matches!(store.load(), Err(MentionError::Format)),
                "{document}"
            );
        }
        fs::write(
            directory.path().join(DOCUMENT),
            vec![b' '; MAX_DOCUMENT_BYTES as usize + 1],
        )
        .unwrap();
        assert!(matches!(store.load(), Err(MentionError::Format)));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_list_or_directory_is_not_followed() {
        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(
            outside.path().join(DOCUMENT),
            r#"{"format_version":1,"entries":[{"text":"x","key":""}]}"#,
        )
        .unwrap();
        std::os::unix::fs::symlink(
            outside.path().join(DOCUMENT),
            directory.path().join(DOCUMENT),
        )
        .unwrap();
        let store = MentionStore::new(directory.path());
        assert!(matches!(store.load(), Err(MentionError::Storage)));

        let linked = directory.path().join("linked");
        std::os::unix::fs::symlink(outside.path(), &linked).unwrap();
        let store = MentionStore::new(&linked);
        assert!(matches!(
            store.save(&[entry("x", "")]),
            Err(MentionError::Storage)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn writes_stay_bound_to_the_locked_directory_after_replacement() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("plugins");
        fs::create_dir(&directory).unwrap();
        let store = MentionStore::new(&directory);
        let lock = store.lock().unwrap();

        let moved = root.path().join("plugins-moved");
        fs::rename(&directory, &moved).unwrap();
        fs::create_dir(&directory).unwrap();

        store.write_locked(&lock, &[]).unwrap();

        assert!(moved.join(DOCUMENT).exists());
        assert!(!directory.join(DOCUMENT).exists());
        fs::remove_dir_all(moved).unwrap();
    }
}
