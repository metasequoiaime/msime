//! Bounded persistent clipboard history. Hosts decide which clipboard events to observe.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_segmentation::UnicodeSegmentation;

const MAX_ENTRIES: usize = 50;
pub const MAX_TEXT_UTF16_UNITS: usize = 4000;
pub const MAX_TEXT_BYTES: usize = MAX_TEXT_UTF16_UNITS * 3;
pub const MAX_MOBILE_TEXT_CHARACTERS: usize = 10_000;
pub const MAX_MOBILE_TEXT_BYTES: usize = 40_000;

/// Match the Windows history length without splitting Unicode characters.
pub fn normalize_text(text: &str) -> String {
    let text = text.trim_end_matches(['\0', '\r']);
    let mut units = 0;
    let end = text
        .char_indices()
        .find_map(|(index, character)| {
            units += character.len_utf16();
            (units > MAX_TEXT_UTF16_UNITS).then_some(index)
        })
        .unwrap_or(text.len());
    text[..end].to_owned()
}
// JSON may escape every input byte as six ASCII bytes. Structured records add
// a small, bounded amount of metadata around every string.
const MAX_HISTORY_BYTES: u64 = (MAX_ENTRIES * (MAX_MOBILE_TEXT_BYTES * 6 + 128) + 2) as u64;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardHistoryEntry {
    pub text: String,
    pub timestamp_ms: u64,
    pub pinned: bool,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum StoredHistory {
    Structured(Vec<ClipboardHistoryEntry>),
    Legacy(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct ClipboardHistoryStore {
    path: PathBuf,
    entries: Vec<ClipboardHistoryEntry>,
}

impl ClipboardHistoryStore {
    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            entries: Vec::new(),
        }
    }

    pub fn load(&mut self) -> std::io::Result<()> {
        match fs::File::open(&self.path) {
            Ok(file) => {
                let bytes = crate::bounded_io::read_bounded_file(file, MAX_HISTORY_BYTES, || {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "clipboard history exceeds size limit",
                    )
                })?;
                let stored: StoredHistory = serde_json::from_slice(&bytes).map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "invalid clipboard history document",
                    )
                })?;
                let mut values = match stored {
                    StoredHistory::Structured(values) => values,
                    // Preserve the legacy newest-first order with deterministic
                    // timestamps. The next successful mutation upgrades the file.
                    StoredHistory::Legacy(values) => {
                        let length = values.len() as u64;
                        values
                            .into_iter()
                            .enumerate()
                            .map(|(index, text)| ClipboardHistoryEntry {
                                text: normalize_text(&text),
                                timestamp_ms: length.saturating_sub(index as u64),
                                pinned: false,
                            })
                            .collect()
                    }
                };
                values.retain(|value| valid_stored(&value.text));
                sort_entries(&mut values);
                let mut entries = Vec::new();
                for value in values {
                    if !entries
                        .iter()
                        .any(|entry: &ClipboardHistoryEntry| entry.text == value.text)
                    {
                        entries.push(value);
                        if entries.len() == MAX_ENTRIES {
                            break;
                        }
                    }
                }
                self.entries = entries;
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.entries.clear();
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    pub fn entries(&self) -> &[ClipboardHistoryEntry] {
        &self.entries
    }

    /// Atomically import validated records only while the latest shared history
    /// is absent or empty. Platform migrations use this after locking and fully
    /// decoding their legacy format; an existing shared history always wins.
    pub fn import_if_empty(
        &mut self,
        mut entries: Vec<ClipboardHistoryEntry>,
    ) -> std::io::Result<bool> {
        if entries.len() > MAX_ENTRIES
            || entries
                .iter()
                .any(|entry| !mobile_text_is_valid(&entry.text))
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid clipboard history import",
            ));
        }
        let unique = entries
            .iter()
            .map(|entry| entry.text.as_str())
            .collect::<std::collections::HashSet<_>>();
        if unique.len() != entries.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "duplicate clipboard history import",
            ));
        }
        sort_entries(&mut entries);
        let _lock = self.lock_writer()?;
        let mut latest = Self::open(&self.path);
        latest.load()?;
        if !latest.entries.is_empty() {
            self.entries = latest.entries;
            return Ok(false);
        }
        self.persist(&entries)?;
        self.entries = entries;
        Ok(true)
    }

    pub fn push(&mut self, text: String) -> std::io::Result<bool> {
        let text = normalize_text(&text);
        if !valid(&text) {
            return Ok(false);
        }
        self.push_validated(text)
    }

    /// Mobile hosts preserve the original text and match Apple's Character and
    /// UTF-8 limits. Desktop capture continues to use `push` and its Windows limit.
    pub fn push_mobile(&mut self, text: String) -> std::io::Result<bool> {
        if !valid_mobile(&text) {
            return Ok(false);
        }
        self.push_validated(text)
    }

    fn push_validated(&mut self, text: String) -> std::io::Result<bool> {
        let _lock = self.lock_writer()?;
        let mut latest = Self::open(&self.path);
        latest.load()?;
        let mut next = latest.entries;
        let timestamp_ms = next_timestamp(&next);
        if let Some(index) = next.iter().position(|item| item.text == text) {
            let mut entry = next.remove(index);
            entry.timestamp_ms = timestamp_ms;
            next.push(entry);
        } else {
            if next.len() == MAX_ENTRIES {
                let Some(index) = next.iter().rposition(|entry| !entry.pinned) else {
                    self.entries = next;
                    return Ok(false);
                };
                next.remove(index);
            }
            next.push(ClipboardHistoryEntry {
                text,
                timestamp_ms,
                pinned: false,
            });
        }
        sort_entries(&mut next);
        self.persist(&next)?;
        self.entries = next;
        Ok(true)
    }

    pub fn clear(&mut self) -> std::io::Result<()> {
        let _lock = self.lock_writer()?;
        match fs::remove_file(&self.path) {
            Ok(()) => {
                self.entries.clear();
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.entries.clear();
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// Remove by content, not by a stale host's row index.
    pub fn remove(&mut self, text: &str) -> std::io::Result<bool> {
        let _lock = self.lock_writer()?;
        let mut latest = Self::open(&self.path);
        latest.load()?;
        let old_length = latest.entries.len();
        latest.entries.retain(|entry| entry.text != text);
        let removed = old_length != latest.entries.len();
        if removed {
            self.persist(&latest.entries)?;
        }
        self.entries = latest.entries;
        Ok(removed)
    }

    /// Pin or unpin by content so stale hosts never act on a shifted row index.
    pub fn set_pinned(&mut self, text: &str, pinned: bool) -> std::io::Result<bool> {
        let _lock = self.lock_writer()?;
        let mut latest = Self::open(&self.path);
        latest.load()?;
        let Some(entry) = latest.entries.iter_mut().find(|entry| entry.text == text) else {
            self.entries = latest.entries;
            return Ok(false);
        };
        if entry.pinned != pinned {
            entry.pinned = pinned;
            sort_entries(&mut latest.entries);
            self.persist(&latest.entries)?;
        }
        self.entries = latest.entries;
        Ok(true)
    }

    fn lock_writer(&self) -> std::io::Result<fs::File> {
        let parent = self.path.parent().filter(|p| !p.as_os_str().is_empty());
        if let Some(parent) = parent {
            fs::create_dir_all(parent)?;
        }
        // Keep this sidecar stable across atomic replacement and clear. Removing
        // it would let another process lock a different inode at the same path.
        let mut lock_path = self.path.as_os_str().to_owned();
        lock_path.push(".lock");
        let mut options = fs::OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(lock_path)?;
        crate::file_lock::exclusive(&lock)?;
        Ok(lock)
    }

    fn persist(&self, entries: &[ClipboardHistoryEntry]) -> std::io::Result<()> {
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        fs::create_dir_all(parent)?;
        let bytes = serde_json::to_vec(entries).expect("clipboard entries are serializable");
        // Unique temporary file (0600 on Unix); never remove the old file before replacement.
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(&bytes)?;
        temp.as_file().sync_all()?;
        temp.persist(&self.path).map_err(|error| error.error)?;
        Ok(())
    }
}

fn sort_entries(entries: &mut [ClipboardHistoryEntry]) {
    entries.sort_by(|left, right| {
        right
            .pinned
            .cmp(&left.pinned)
            .then_with(|| right.timestamp_ms.cmp(&left.timestamp_ms))
    });
}

fn next_timestamp(entries: &[ClipboardHistoryEntry]) -> u64 {
    let wall_clock = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0);
    entries
        .iter()
        .map(|entry| entry.timestamp_ms)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
        .max(wall_clock)
}

fn valid(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAX_TEXT_BYTES && valid_characters(text)
}

fn valid_mobile(text: &str) -> bool {
    mobile_text_is_valid(text)
}

/// Mobile hosts and their migration bridges share the exact persisted-text
/// validation without needing to reproduce Unicode segmentation rules.
pub fn mobile_text_is_valid(text: &str) -> bool {
    !text.trim().is_empty()
        && text.len() <= MAX_MOBILE_TEXT_BYTES
        && text.graphemes(true).count() <= MAX_MOBILE_TEXT_CHARACTERS
        && valid_characters(text)
}

fn valid_stored(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_MOBILE_TEXT_BYTES
        && text.graphemes(true).count() <= MAX_MOBILE_TEXT_CHARACTERS
        && valid_characters(text)
}

// Matches the Windows history, which keeps every control character as user content; NUL is the one exception because the source stores C strings and the native bridges rely on NUL-free text.
fn valid_characters(text: &str) -> bool {
    !text.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(store: &ClipboardHistoryStore) -> Vec<&str> {
        store
            .entries()
            .iter()
            .map(|entry| entry.text.as_str())
            .collect()
    }

    #[test]
    fn stale_writers_preserve_new_records_and_do_not_resurrect_cleared_history() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let mut first = ClipboardHistoryStore::open(&path);
        let mut stale = ClipboardHistoryStore::open(&path);
        first.push("synthetic first".into()).unwrap();
        stale.push("synthetic second".into()).unwrap();
        assert_eq!(texts(&stale), ["synthetic second", "synthetic first"]);
        stale.clear().unwrap();
        first.push("synthetic after clear".into()).unwrap();
        assert_eq!(texts(&first), ["synthetic after clear"]);
    }

    #[test]
    fn removal_uses_latest_history_and_preserves_corrupt_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let mut first = ClipboardHistoryStore::open(&path);
        let mut stale = ClipboardHistoryStore::open(&path);
        first.push("synthetic first".into()).unwrap();
        stale.load().unwrap();
        first.push("synthetic second".into()).unwrap();
        assert!(stale.remove("synthetic first").unwrap());
        assert_eq!(texts(&stale), ["synthetic second"]);
        assert!(!first.remove("synthetic absent").unwrap());
        assert_eq!(texts(&first), ["synthetic second"]);
        fs::write(&path, b"broken synthetic document").unwrap();
        assert!(first.remove("synthetic second").is_err());
        assert!(first.push("synthetic rejected".into()).is_err());
        assert_eq!(texts(&first), ["synthetic second"]);
        assert_eq!(fs::read(&path).unwrap(), b"broken synthetic document");
    }

    // Invoked by separate test processes below; no real clipboard data involved.
    #[test]
    fn process_writer() {
        let Some(path) = std::env::var_os("MSIME_TEST_CLIPBOARD_TRANSACTION_PATH") else {
            return;
        };
        let id = std::env::var("MSIME_TEST_CLIPBOARD_TRANSACTION_ID").unwrap();
        let mut store = ClipboardHistoryStore::open(path);
        for index in 0..8 {
            store.push(format!("synthetic-{id}-{index}")).unwrap();
        }
    }

    #[test]
    fn independent_processes_preserve_all_updates() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let executable = std::env::current_exe().unwrap();
        let children: Vec<_> = (0..4)
            .map(|id| {
                std::process::Command::new(&executable)
                    .args(["--exact", "clipboard::tests::process_writer"])
                    .env("MSIME_TEST_CLIPBOARD_TRANSACTION_PATH", &path)
                    .env("MSIME_TEST_CLIPBOARD_TRANSACTION_ID", id.to_string())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .unwrap()
            })
            .collect();
        for mut child in children {
            assert!(child.wait().unwrap().success());
        }
        let mut store = ClipboardHistoryStore::open(&path);
        store.load().unwrap();
        assert_eq!(store.entries().len(), 32);
        for id in 0..4 {
            for index in 0..8 {
                let expected = format!("synthetic-{id}-{index}");
                assert!(texts(&store).contains(&expected.as_str()));
            }
        }
    }

    #[test]
    fn persists_deduplicates_and_clears() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("clipboard_history.json");
        let mut store = ClipboardHistoryStore::open(&path);
        assert!(store.push("second".into()).unwrap());
        assert!(store.push("first".into()).unwrap());
        assert!(store.push("second".into()).unwrap());
        let mut loaded = ClipboardHistoryStore::open(&path);
        loaded.load().unwrap();
        assert_eq!(texts(&loaded), ["second", "first"]);
        loaded.clear().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn preserves_multiline_and_control_text_and_rejects_nul() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("clipboard_history.json");
        let mut store = ClipboardHistoryStore::open(&path);
        assert!(store.push("line\nfeed\tvalue".into()).unwrap());
        assert!(!store.push("line\0feed".into()).unwrap());
        assert!(store
            .push("bell\u{0007}form\u{000C}delete\u{007F}next\u{0085}".into())
            .unwrap());
        let expected = [
            "bell\u{0007}form\u{000C}delete\u{007F}next\u{0085}",
            "line\nfeed\tvalue",
        ];
        assert_eq!(texts(&store), expected);
        let mut loaded = ClipboardHistoryStore::open(&path);
        loaded.load().unwrap();
        assert_eq!(texts(&loaded), expected);
    }

    #[test]
    fn load_is_bounded_and_preserves_state_on_corrupt_documents() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let mut store = ClipboardHistoryStore::open(&path);
        store.push("synthetic-kept".into()).unwrap();
        fs::write(&path, b"invalid synthetic document").unwrap();
        assert_eq!(
            store.load().unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
        assert_eq!(texts(&store), ["synthetic-kept"]);
        fs::File::create(&path)
            .unwrap()
            .set_len(MAX_HISTORY_BYTES + 1)
            .unwrap();
        assert_eq!(
            store.load().unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
        assert_eq!(texts(&store), ["synthetic-kept"]);
        fs::remove_file(&path).unwrap();
        store.load().unwrap();
        assert!(store.entries().is_empty());
    }

    #[test]
    fn load_deduplicates_before_applying_entry_limit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let mut values = vec!["synthetic-first".to_string(); 55];
        values.extend((0..60).map(|i| format!("synthetic-{i}")));
        fs::write(&path, serde_json::to_vec(&values).unwrap()).unwrap();
        let mut store = ClipboardHistoryStore::open(&path);
        store.load().unwrap();
        assert_eq!(store.entries().len(), MAX_ENTRIES);
        assert_eq!(store.entries()[0].text, "synthetic-first");
        assert_eq!(store.entries()[49].text, "synthetic-48");
    }

    #[test]
    fn failed_save_and_clear_keep_in_memory_history() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let backup = directory.path().join("backup.json");
        let mut store = ClipboardHistoryStore::open(&path);
        store.push("synthetic-kept".into()).unwrap();
        fs::rename(&path, &backup).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(store.push("synthetic-rejected".into()).is_err());
        assert_eq!(texts(&store), ["synthetic-kept"]);
        assert!(store.clear().is_err());
        assert_eq!(texts(&store), ["synthetic-kept"]);
        let saved: Vec<ClipboardHistoryEntry> =
            serde_json::from_slice(&fs::read(&backup).unwrap()).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].text, "synthetic-kept");
        // Backup, failed destination directory and persistent writer-lock sidecar.
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 3);
    }

    #[test]
    #[cfg(unix)]
    fn replacement_history_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        fs::write(&path, b"[]").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let mut store = ClipboardHistoryStore::open(&path);
        store.push("synthetic-private".into()).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn structured_history_sorts_pins_and_updates_duplicate_timestamps() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let fixture = vec![
            ClipboardHistoryEntry {
                text: "synthetic older".into(),
                timestamp_ms: 10,
                pinned: false,
            },
            ClipboardHistoryEntry {
                text: "synthetic pinned".into(),
                timestamp_ms: 1,
                pinned: true,
            },
            ClipboardHistoryEntry {
                text: "synthetic newer".into(),
                timestamp_ms: 20,
                pinned: false,
            },
        ];
        fs::write(&path, serde_json::to_vec(&fixture).unwrap()).unwrap();
        let mut store = ClipboardHistoryStore::open(&path);
        store.load().unwrap();
        assert_eq!(
            texts(&store),
            ["synthetic pinned", "synthetic newer", "synthetic older"]
        );
        assert!(store.push("synthetic older".into()).unwrap());
        assert_eq!(
            texts(&store),
            ["synthetic pinned", "synthetic older", "synthetic newer"]
        );
        assert!(store.set_pinned("synthetic newer", true).unwrap());
        assert_eq!(
            texts(&store),
            ["synthetic newer", "synthetic pinned", "synthetic older"]
        );
        assert!(store.set_pinned("synthetic newer", false).unwrap());
        assert_eq!(
            texts(&store),
            ["synthetic pinned", "synthetic older", "synthetic newer"]
        );
        let encoded = serde_json::to_value(store.entries()).unwrap();
        assert_eq!(encoded[0]["text"], "synthetic pinned");
        assert_eq!(encoded[0]["timestampMs"], 1);
        assert_eq!(encoded[0]["pinned"], true);
        assert!(encoded[0].get("timestamp_ms").is_none());
    }

    #[test]
    fn import_only_replaces_absent_or_empty_history() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let mut store = ClipboardHistoryStore::open(&path);
        let imported = vec![
            ClipboardHistoryEntry {
                text: "synthetic newer".into(),
                timestamp_ms: 20,
                pinned: false,
            },
            ClipboardHistoryEntry {
                text: "synthetic pinned".into(),
                timestamp_ms: 10,
                pinned: true,
            },
        ];
        assert!(store.import_if_empty(imported).unwrap());
        assert_eq!(texts(&store), ["synthetic pinned", "synthetic newer"]);
        assert!(!store
            .import_if_empty(vec![ClipboardHistoryEntry {
                text: "synthetic ignored".into(),
                timestamp_ms: 30,
                pinned: false,
            }])
            .unwrap());
        assert_eq!(texts(&store), ["synthetic pinned", "synthetic newer"]);

        store.clear().unwrap();
        fs::write(&path, b"[]").unwrap();
        assert!(store
            .import_if_empty(vec![ClipboardHistoryEntry {
                text: "synthetic after empty".into(),
                timestamp_ms: 40,
                pinned: false,
            }])
            .unwrap());
        assert_eq!(texts(&store), ["synthetic after empty"]);
    }

    #[test]
    fn invalid_or_duplicate_import_is_rejected_without_writing() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let mut store = ClipboardHistoryStore::open(&path);
        let duplicate = vec![
            ClipboardHistoryEntry {
                text: "synthetic duplicate".into(),
                timestamp_ms: 1,
                pinned: false,
            },
            ClipboardHistoryEntry {
                text: "synthetic duplicate".into(),
                timestamp_ms: 2,
                pinned: true,
            },
        ];
        assert_eq!(
            store.import_if_empty(duplicate).unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
        assert!(!path.exists());
        assert_eq!(
            store
                .import_if_empty(vec![ClipboardHistoryEntry {
                    text: "synthetic\0invalid".into(),
                    timestamp_ms: 1,
                    pinned: false,
                }])
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidData
        );
        assert!(!path.exists());
    }

    #[test]
    fn full_pinned_history_rejects_new_text_and_evicts_oldest_unpinned() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let pinned: Vec<_> = (0..MAX_ENTRIES)
            .map(|index| ClipboardHistoryEntry {
                text: format!("synthetic pinned {index}"),
                timestamp_ms: index as u64,
                pinned: true,
            })
            .collect();
        fs::write(&path, serde_json::to_vec(&pinned).unwrap()).unwrap();
        let mut store = ClipboardHistoryStore::open(&path);
        assert!(!store.push("synthetic rejected".into()).unwrap());
        assert_eq!(store.entries().len(), MAX_ENTRIES);
        assert!(!fs::read_to_string(&path)
            .unwrap()
            .contains("synthetic rejected"));

        assert!(store.set_pinned("synthetic pinned 0", false).unwrap());
        assert!(store.push("synthetic accepted".into()).unwrap());
        assert_eq!(store.entries().len(), MAX_ENTRIES);
        assert_eq!(store.entries().last().unwrap().text, "synthetic accepted");
        assert!(!texts(&store).contains(&"synthetic pinned 0"));
    }

    #[test]
    fn legacy_strings_load_without_rewrite_and_upgrade_on_mutation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let fixture = br#"["synthetic newest","synthetic older"]"#;
        fs::write(&path, fixture).unwrap();
        let mut store = ClipboardHistoryStore::open(&path);
        store.load().unwrap();
        assert_eq!(texts(&store), ["synthetic newest", "synthetic older"]);
        assert_eq!(fs::read(&path).unwrap(), fixture);
        assert!(store.set_pinned("synthetic older", true).unwrap());
        let upgraded: Vec<ClipboardHistoryEntry> =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(upgraded[0].text, "synthetic older");
        assert!(upgraded[0].pinned);
    }

    #[test]
    fn mobile_capture_preserves_text_within_apple_character_and_byte_limits() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let mut store = ClipboardHistoryStore::open(&path);
        let text = "家".repeat(4_001);
        assert!(store.push_mobile(text.clone()).unwrap());
        assert_eq!(store.entries()[0].text, text);
        assert!(!store.push_mobile(" \n\t".into()).unwrap());
        assert!(!store
            .push_mobile("家".repeat(MAX_MOBILE_TEXT_CHARACTERS + 1))
            .unwrap());
        assert!(!store
            .push_mobile("😀".repeat(MAX_MOBILE_TEXT_BYTES / 4 + 1))
            .unwrap());

        let mut desktop = ClipboardHistoryStore::open(&path);
        assert!(desktop.push("家".repeat(4_001)).unwrap());
        assert_eq!(
            desktop
                .entries()
                .iter()
                .find(|entry| entry.text != text)
                .unwrap()
                .text
                .encode_utf16()
                .count(),
            MAX_TEXT_UTF16_UNITS
        );
    }
}
