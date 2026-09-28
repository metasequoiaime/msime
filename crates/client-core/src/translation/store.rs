//! User-owned learned English/Chinese glosses, separate from packaged dictionaries.
//! Hosts must supply a private writable application-data directory, never resources.
//! Contents include learned words: do not log or upload them. Hashed filenames are
//! not encryption. One atomic record per key avoids rewriting unrelated glosses.

use crate::translation::{
    format_translation_gloss, is_cloud_translatable_chinese, is_cloud_translatable_english,
    is_valid_source_text, should_persist_translation,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

const MAX_RECORD_BYTES: u64 = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlossDirection {
    EnglishToChinese,
    ChineseToEnglish,
}

impl GlossDirection {
    fn key(self, text: &str) -> Option<String> {
        if !is_valid_source_text(text) {
            return None;
        }
        match self {
            Self::EnglishToChinese if is_cloud_translatable_english(text) => {
                Some(text.to_ascii_lowercase())
            }
            Self::ChineseToEnglish if is_cloud_translatable_chinese(text) => Some(text.into()),
            _ => None,
        }
    }

    fn directory(self) -> &'static str {
        match self {
            Self::EnglishToChinese => "en-zh",
            Self::ChineseToEnglish => "zh-en",
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u32,
    direction: GlossDirection,
    key: String,
    gloss: String,
}

#[derive(Debug, thiserror::Error)]
pub enum GlossStoreError {
    #[error("invalid learned translation record")]
    InvalidRecord,
    #[error("learned translation storage unavailable")]
    Io(#[from] std::io::Error),
}

pub struct TranslationGlossStore {
    root: PathBuf,
}

// Publish a record by replacing the previous one, retrying briefly on Windows.
//
// The replace is a rename over the live path. On Windows that can fail with
// ACCESS_DENIED while another process or thread is mid-operation on the same
// target, even when every handle shares delete - the window is short and the
// operation is not serialised for us. Two threads remembering the same key
// hit it readily. A few brief retries close that window; anything else is
// reported, so a real permission problem still surfaces rather than spinning.
fn persist_replacing(
    temporary: tempfile::NamedTempFile,
    path: &std::path::Path,
) -> Result<(), GlossStoreError> {
    #[cfg(windows)]
    {
        let mut file = temporary;
        for attempt in 0..16 {
            match file.persist(path) {
                Ok(_) => return Ok(()),
                Err(error)
                    if error.error.kind() == std::io::ErrorKind::PermissionDenied
                        && attempt < 15 =>
                {
                    file = error.file;
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
                Err(error) => return Err(error.error.into()),
            }
        }
        unreachable!("the loop returns on its last attempt")
    }
    #[cfg(not(windows))]
    {
        temporary.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
}

// Open a record for reading without blocking its replacement.
//
// `remember` publishes each key by writing a temporary file and renaming it
// over the old one. On Windows that rename fails with ACCESS_DENIED while any
// other handle holds the target open, unless that handle allowed deletion - so
// a reader using a plain open makes a concurrent writer fail. Sharing delete
// is what makes the atomic replace actually atomic from the reader's side:
// the reader keeps reading the bytes it opened, and the writer proceeds.
#[cfg(windows)]
fn open_shared(path: &std::path::Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const FILE_SHARE_DELETE: u32 = 0x0000_0004;
    fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(path)
}

// Every other platform already replaces a file out from under an open handle.
#[cfg(not(windows))]
fn open_shared(path: &std::path::Path) -> std::io::Result<File> {
    File::open(path)
}

impl TranslationGlossStore {
    /// Construct without touching disk. The caller chooses the private user root.
    pub fn new(user_directory: impl Into<PathBuf>) -> Self {
        Self {
            root: user_directory.into().join("learned-translations-v1"),
        }
    }

    fn path(&self, direction: GlossDirection, key: &str) -> PathBuf {
        self.root.join(direction.directory()).join(format!(
            "{}.json",
            hex::encode(Sha256::digest(key.as_bytes()))
        ))
    }

    /// Only the English target setting persists, matching Windows glossary rules.
    /// Unsupported inputs are misses; malformed existing records are errors.
    /// Call on an IO worker, not the input event thread.
    pub fn lookup(
        &self,
        configured_target: &str,
        direction: GlossDirection,
        text: &str,
    ) -> Result<Option<String>, GlossStoreError> {
        if configured_target != "en" {
            return Ok(None);
        }
        let Some(key) = direction.key(text) else {
            return Ok(None);
        };
        let file = match open_shared(&self.path(direction, &key)) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let bytes = crate::bounded_io::read_bounded_file(file, MAX_RECORD_BYTES, || {
            GlossStoreError::InvalidRecord
        })?;
        let record: Record =
            serde_json::from_slice(&bytes).map_err(|_| GlossStoreError::InvalidRecord)?;
        if record.version != 1
            || record.direction != direction
            || record.key != key
            || !should_persist_translation(&key, &record.gloss)
            || format_translation_gloss(&record.gloss).as_ref() != Some(&record.gloss)
        {
            return Err(GlossStoreError::InvalidRecord);
        }
        Ok(Some(record.gloss))
    }

    /// Returns false for ineligible results without creating directories/files.
    /// Provider identity is intentionally absent: learned offline glosses outlive
    /// provider selection, as with Windows english.db. Last completed writer wins.
    pub fn remember(
        &self,
        configured_target: &str,
        direction: GlossDirection,
        text: &str,
        gloss: &str,
    ) -> Result<bool, GlossStoreError> {
        if configured_target != "en" {
            return Ok(false);
        }
        let (Some(key), Some(gloss)) = (direction.key(text), format_translation_gloss(gloss))
        else {
            return Ok(false);
        };
        if !should_persist_translation(&key, &gloss) {
            return Ok(false);
        }
        let path = self.path(direction, &key);
        let directory = path.parent().ok_or(GlossStoreError::InvalidRecord)?;
        let bytes = serde_json::to_vec(&Record {
            version: 1,
            direction,
            key,
            gloss,
        })
        .map_err(|_| GlossStoreError::InvalidRecord)?;
        if bytes.len() as u64 > MAX_RECORD_BYTES {
            return Err(GlossStoreError::InvalidRecord);
        }
        fs::create_dir_all(directory)?;
        // NamedTempFile creates private files and persist atomically replaces only
        // this key; concurrent independent keys cannot lose each other's writes.
        let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        persist_replacing(temporary, &path)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use GlossDirection::{ChineseToEnglish as Zh, EnglishToChinese as En};

    #[test]
    fn persists_reopens_normalizes_and_replaces() {
        let root = tempfile::tempdir().unwrap();
        let store = TranslationGlossStore::new(root.path());
        assert_eq!(store.lookup("en", En, "Hello").unwrap(), None);
        assert!(!store.root.exists());
        assert!(store.remember("en", En, "Hello", "  你好\n世界  ").unwrap());
        assert!(store.remember("en", Zh, "测试", "test").unwrap());
        let reopened = TranslationGlossStore::new(root.path());
        assert_eq!(
            reopened.lookup("en", En, "HELLO").unwrap().as_deref(),
            Some("你好 世界")
        );
        assert_eq!(
            reopened.lookup("en", Zh, "测试").unwrap().as_deref(),
            Some("test")
        );
        assert!(reopened.remember("en", En, "hello", "你好").unwrap());
        assert_eq!(
            store.lookup("en", En, "Hello").unwrap().as_deref(),
            Some("你好")
        );
        assert!(!root.path().join("english.db").exists());
    }

    #[test]
    fn rejects_ineligible_results_without_writes() {
        let root = tempfile::tempdir().unwrap();
        let store = TranslationGlossStore::new(root.path());
        for (target, direction, key, gloss) in [
            ("fr", En, "hello", "你好"),
            ("en", En, "hello", "HELLO"),
            ("en", Zh, "测试", "测试"),
            ("en", En, "../hello", "你好"),
            ("en", En, "hello", ""),
            ("en", En, "hello", "bad\0gloss"),
            ("en", En, "测试", "test"),
            ("en", Zh, "hello", "你好"),
        ] {
            assert!(!store.remember(target, direction, key, gloss).unwrap());
        }
        assert!(!store.remember("en", En, &"a".repeat(41), "你好").unwrap());
        assert!(!store.remember("en", En, "hello", &"字".repeat(33)).unwrap());
        assert!(!store.root.exists());
    }

    #[test]
    fn corrupt_and_mismatched_records_are_not_used() {
        let root = tempfile::tempdir().unwrap();
        let store = TranslationGlossStore::new(root.path());
        store.remember("en", En, "hello", "你好").unwrap();
        let path = store.path(En, "hello");
        for contents in [
            b"broken".to_vec(),
            vec![b'x'; 4097],
            br#"{"version":2,"direction":"english_to_chinese","key":"hello","gloss":"test"}"#
                .to_vec(),
            br#"{"version":1,"direction":"english_to_chinese","key":"other","gloss":"test"}"#
                .to_vec(),
            br#"{"version":1,"direction":"chinese_to_english","key":"hello","gloss":"test"}"#
                .to_vec(),
        ] {
            fs::write(&path, contents).unwrap();
            assert!(matches!(
                store.lookup("en", En, "Hello"),
                Err(GlossStoreError::InvalidRecord)
            ));
        }
        store.remember("en", En, "hello", "你好").unwrap();
        assert_eq!(store.lookup("fr", En, "hello").unwrap(), None);
        assert_eq!(store.lookup("en", Zh, "hello").unwrap(), None);
    }

    #[test]
    fn concurrent_keys_and_private_files() {
        let root = tempfile::tempdir().unwrap();
        std::thread::scope(|scope| {
            for key in ["hello", "world", "sample", "test"] {
                let path = root.path();
                scope.spawn(move || {
                    let store = TranslationGlossStore::new(path);
                    for _ in 0..8 {
                        store.remember("en", En, key, "示例").unwrap();
                    }
                });
            }
        });
        let store = TranslationGlossStore::new(root.path());
        for key in ["hello", "world", "sample", "test"] {
            assert_eq!(
                store.lookup("en", En, key).unwrap().as_deref(),
                Some("示例")
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(store.path(En, "hello"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o077,
                0
            );
        }
    }

    #[test]
    fn concurrent_replacement_never_exposes_partial_records() {
        let root = tempfile::tempdir().unwrap();
        let store = TranslationGlossStore::new(root.path());
        store.remember("en", En, "hello", "你好").unwrap();
        std::thread::scope(|scope| {
            for gloss in ["你好", "您好"] {
                let store = &store;
                scope.spawn(move || {
                    for _ in 0..20 {
                        store.remember("en", En, "hello", gloss).unwrap();
                        let value = store.lookup("en", En, "hello").unwrap().unwrap();
                        assert!(value == "你好" || value == "您好");
                    }
                });
            }
        });
        assert!(store
            .remember("en", En, "boundary", &"字".repeat(32))
            .unwrap());
        let blocked = root.path().join("not-a-directory");
        fs::write(&blocked, b"synthetic").unwrap();
        assert!(matches!(
            TranslationGlossStore::new(blocked).remember("en", En, "hello", "你好"),
            Err(GlossStoreError::Io(_))
        ));
    }
}
