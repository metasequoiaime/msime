//! Typo learning and autocorrect suppression (user-dictionary.md §10.3, quanpin.md §7.8). The profile is a per-journal snapshot shared by the process and reloaded when the journal or its `-wal` file changes.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, RwLock, Weak};
use std::time::SystemTime;

use rusqlite::{params, params_from_iter, Connection, ToSql};

use super::journal::{open_existing_read_only, open_journal, UNSTORABLE_ENTRY};
use crate::error::{EngineError, Result};
use crate::pinyin::typos::syllable_typo_kind;

/// Typo learning describes habits, not dictionary content, so a small bounded table is enough; the rows that matter are the ones seen often (J:728-732).
pub const MAX_TYPO_STATE_ROWS: usize = 2_048;
pub const MAX_TYPO_STATE_COUNT: i32 = 1_000;
pub const MAX_SUPPRESSED_INPUT_LENGTH: usize = 64;
/// The longest quanpin syllable (`zhuang`).
const MAX_SYLLABLE_LENGTH: usize = 6;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PinyinTypoState {
    pub accepted: HashMap<String, HashMap<String, i32>>,
    pub suppressed: HashSet<String>,
}

fn is_lowercase_letters(text: &str, max_length: usize) -> bool {
    !text.is_empty()
        && text.len() <= max_length
        && text.bytes().all(|byte| byte.is_ascii_lowercase())
}

/// Whether a folded input fits the suppression table (J:864-867); an input that does not is not a write failure to report on every Enter.
pub fn is_storable_autocorrect_suppression(input: &str) -> bool {
    is_lowercase_letters(input, MAX_SUPPRESSED_INPUT_LENGTH)
}

/// Deletes the least used rows, oldest first, until at most `MAX_TYPO_STATE_ROWS` remain. Rows matching `keep` (bound from `?2`) are the ones this transaction just wrote: a new row starts at the lowest count, so without the exclusion a full table would delete it in the same commit that created it (J:746-764).
fn trim_typo_table(
    connection: &Connection,
    table: &str,
    count_column: &str,
    keep: &str,
    keep_values: &[&str],
) -> Result<()> {
    let sql = format!(
        "DELETE FROM {table} WHERE rowid IN (SELECT rowid FROM {table} WHERE NOT ({keep}) ORDER BY {count_column} ASC, updated_at ASC, rowid ASC LIMIT max(0, (SELECT count(*) FROM {table}) - ?1))"
    );
    let limit = MAX_TYPO_STATE_ROWS as i64;
    let mut values: Vec<&dyn ToSql> = vec![&limit];
    values.extend(keep_values.iter().map(|value| value as &dyn ToSql));
    connection
        .prepare(&sql)?
        .execute(params_from_iter(values))?;
    Ok(())
}

/// Increment each accepted `(typed, intended)` pair; the whole batch is refused if any pair is not two different lowercase syllables of at most six letters (J:811-862).
pub fn record_pinyin_typos(user_db: &Path, pairs: &[(String, String)]) -> Result<()> {
    let valid = !pairs.is_empty()
        && pairs.iter().all(|(typed, intended)| {
            is_lowercase_letters(typed, MAX_SYLLABLE_LENGTH)
                && is_lowercase_letters(intended, MAX_SYLLABLE_LENGTH)
                && typed != intended
        });
    if !valid {
        return Err(EngineError::invalid(UNSTORABLE_ENTRY));
    }
    let journal = open_journal(user_db)?;
    journal.execute_batch("BEGIN IMMEDIATE")?;
    {
        let mut upsert = journal.prepare_cached(
            "INSERT INTO pinyin_typo_counts(typed,intended,accepted) VALUES(?1,?2,1) ON CONFLICT(typed,intended) DO UPDATE SET accepted=min(accepted+1,?3),updated_at=unixepoch()",
        )?;
        for (typed, intended) in pairs {
            upsert.execute(params![typed, intended, MAX_TYPO_STATE_COUNT])?;
        }
    }
    // A row-value IN list rather than chained ORs: every OR adds a level of expression depth, and SQLite rejects statements deeper than SQLITE_MAX_EXPR_DEPTH (1000 by default).
    let rows: Vec<String> = (0..pairs.len())
        .map(|index| format!("(?{},?{})", 2 * index + 2, 2 * index + 3))
        .collect();
    let keep = format!("(typed,intended) IN (VALUES {})", rows.join(","));
    let keep_values: Vec<&str> = pairs
        .iter()
        .flat_map(|(typed, intended)| [typed.as_str(), intended.as_str()])
        .collect();
    trim_typo_table(
        &journal,
        "pinyin_typo_counts",
        "accepted",
        &keep,
        &keep_values,
    )?;
    // Any `?` above leaves the transaction to the connection guard's rollback.
    journal.execute_batch("COMMIT")?;
    Ok(())
}

/// J:869-889.
pub fn record_autocorrect_suppression(user_db: &Path, input: &str) -> Result<()> {
    if !is_storable_autocorrect_suppression(input) {
        return Err(EngineError::invalid(UNSTORABLE_ENTRY));
    }
    let journal = open_journal(user_db)?;
    journal.execute_batch("BEGIN IMMEDIATE")?;
    journal
        .prepare_cached(
            "INSERT INTO pinyin_autocorrect_suppressions(input,commits) VALUES(?1,1) ON CONFLICT(input) DO UPDATE SET commits=min(commits+1,?2),updated_at=unixepoch()",
        )?
        .execute(params![input, MAX_TYPO_STATE_COUNT])?;
    trim_typo_table(
        &journal,
        "pinyin_autocorrect_suppressions",
        "commits",
        "input=?2",
        &[input],
    )?;
    journal.execute_batch("COMMIT")?;
    Ok(())
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool> {
    Ok(connection
        .prepare_cached("SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1")?
        .exists(params![table])?)
}

/// Read-only; a missing journal or table is an empty state (J:767-809).
pub fn load_pinyin_typo_state(user_db: &Path) -> Result<PinyinTypoState> {
    let mut state = PinyinTypoState::default();
    let Some(connection) = open_existing_read_only(user_db)? else {
        return Ok(state);
    };
    if table_exists(&connection, "pinyin_typo_counts")? {
        let count: i64 = connection.query_row(
            "SELECT count(*) FROM pinyin_typo_counts WHERE accepted > 0 AND accepted <= ?1 AND length(CAST(typed AS BLOB)) BETWEEN 1 AND ?2 AND length(CAST(intended AS BLOB)) BETWEEN 1 AND ?2",
            params![MAX_TYPO_STATE_COUNT, MAX_SYLLABLE_LENGTH as i64],
            |row| row.get(0),
        )?;
        if count > MAX_TYPO_STATE_ROWS as i64 {
            return Err(EngineError::failed(
                "pinyin typo state exceeds its memory limit",
            ));
        }
        let mut statement = connection
            .prepare("SELECT typed,intended,accepted FROM pinyin_typo_counts WHERE accepted > 0 AND accepted <= ?1 AND length(CAST(typed AS BLOB)) BETWEEN 1 AND ?2 AND length(CAST(intended AS BLOB)) BETWEEN 1 AND ?2")?;
        let rows = statement.query_map(
            params![MAX_TYPO_STATE_COUNT, MAX_SYLLABLE_LENGTH as i64],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i32>(2)?,
                ))
            },
        )?;
        for row in rows {
            let (typed, intended, accepted) = row?;
            if is_lowercase_letters(&typed, MAX_SYLLABLE_LENGTH)
                && is_lowercase_letters(&intended, MAX_SYLLABLE_LENGTH)
                && typed != intended
            {
                state
                    .accepted
                    .entry(typed)
                    .or_default()
                    .insert(intended, accepted);
            }
        }
    }
    if table_exists(&connection, "pinyin_autocorrect_suppressions")? {
        let count: i64 = connection.query_row(
            "SELECT count(*) FROM pinyin_autocorrect_suppressions WHERE commits >= 1 AND length(CAST(input AS BLOB)) BETWEEN 1 AND ?1",
            params![MAX_SUPPRESSED_INPUT_LENGTH as i64],
            |row| row.get(0),
        )?;
        if count > MAX_TYPO_STATE_ROWS as i64 {
            return Err(EngineError::failed(
                "pinyin suppression state exceeds its memory limit",
            ));
        }
        let mut statement =
            connection.prepare("SELECT input FROM pinyin_autocorrect_suppressions WHERE commits >= 1 AND length(CAST(input AS BLOB)) BETWEEN 1 AND ?1")?;
        let rows = statement.query_map(params![MAX_SUPPRESSED_INPUT_LENGTH as i64], |row| {
            row.get::<_, String>(0)
        })?;
        for row in rows {
            let input = row?;
            if is_lowercase_letters(&input, MAX_SUPPRESSED_INPUT_LENGTH) {
                state.suppressed.insert(input);
            }
        }
    }
    Ok(state)
}

/// `(modified, size)` of a file, `None` when it cannot be read; the journal and its `-wal` together say whether anything was written since the last load.
type FileStamp = Option<(SystemTime, u64)>;

fn file_stamp(path: &Path) -> FileStamp {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

/// Profiles are shared weakly: one per journal while any engine holds it (TP:22-51).
static PROFILES: LazyLock<Mutex<HashMap<PathBuf, Weak<PersonalTypoProfile>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct PersonalTypoProfile {
    state: std::sync::RwLock<PinyinTypoState>,
    path: PathBuf,
    wal: PathBuf,
    /// Serialises loads and writes; holds the stamp of the files the state was last loaded from, `None` before the first successful load.
    loaded: Mutex<Option<(FileStamp, FileStamp)>>,
    generation: AtomicU64,
}

impl PersonalTypoProfile {
    /// One profile per journal path for the process.
    pub fn shared(user_db: &Path) -> Arc<PersonalTypoProfile> {
        let profile = {
            let mut profiles = PROFILES
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(profile) = profiles.get(user_db).and_then(Weak::upgrade) {
                return profile;
            }
            let mut wal = user_db.as_os_str().to_owned();
            wal.push("-wal");
            let profile = Arc::new(PersonalTypoProfile {
                state: RwLock::new(PinyinTypoState::default()),
                path: user_db.to_path_buf(),
                wal: PathBuf::from(wal),
                loaded: Mutex::new(None),
                generation: AtomicU64::new(0),
            });
            profiles.retain(|_, weak| weak.strong_count() > 0);
            profiles.insert(user_db.to_path_buf(), Arc::downgrade(&profile));
            profile
        };
        // A new profile loads here, while the session is being created, rather than on its first keystroke.
        profile.refresh_if_changed();
        profile
    }

    fn stamp(&self) -> (FileStamp, FileStamp) {
        (file_stamp(&self.path), file_stamp(&self.wal))
    }

    fn replace(&self, state: PinyinTypoState) {
        *self
            .state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = state;
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Adopt a freshly loaded state; the generation only moves when the contents did.
    fn adopt(&self, state: PinyinTypoState) {
        let unchanged = *self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            == state;
        if !unchanged {
            self.replace(state);
        }
    }

    /// Reload when the journal's or its `-wal`'s (mtime, size) moved; the generation advances on every reload.
    pub fn refresh_if_changed(&self) {
        let mut loaded = self
            .loaded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let stamp = self.stamp();
        if loaded.as_ref() == Some(&stamp) {
            return;
        }
        // A journal that exists but cannot be read keeps the held state; the unchanged stamp retries on the next query (TP:115-116).
        let Ok(state) = load_pinyin_typo_state(&self.path) else {
            return;
        };
        *loaded = Some(stamp);
        self.adopt(state);
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub fn accepted(&self, typed: &str, intended: &str) -> i32 {
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state
            .accepted
            .get(typed)
            .and_then(|by_intended| by_intended.get(intended))
            .copied()
            .unwrap_or(0)
    }

    pub fn suppressed(&self, input_key: &str) -> bool {
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        !state.suppressed.is_empty() && state.suppressed.contains(input_key)
    }

    /// After this profile's own write: read the journal back, or when that fails apply the write to the held state so the session sees it at once (TP:78-91).
    fn reload_after_write(
        &self,
        loaded: &mut Option<(FileStamp, FileStamp)>,
        patch: impl FnOnce(&mut PinyinTypoState),
    ) {
        let stamp = self.stamp();
        match load_pinyin_typo_state(&self.path) {
            Ok(state) => {
                *loaded = Some(stamp);
                self.adopt(state);
            }
            Err(_) => {
                let mut state = self
                    .state
                    .read()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clone();
                patch(&mut state);
                self.replace(state);
            }
        }
    }

    /// Write and reload. Every pair must be a known one-edit typo (TP:132-151).
    pub fn record_accepted(&self, pairs: &[(String, String)]) -> Result<()> {
        if pairs
            .iter()
            .any(|(typed, intended)| syllable_typo_kind(typed, intended).is_none())
        {
            return Err(EngineError::invalid(UNSTORABLE_ENTRY));
        }
        let mut loaded = self
            .loaded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        record_pinyin_typos(&self.path, pairs)?;
        self.reload_after_write(&mut loaded, |state| {
            for pair in pairs {
                let count = state
                    .accepted
                    .entry(pair.0.clone())
                    .or_default()
                    .entry(pair.1.clone())
                    .or_insert(0);
                *count = (*count + 1).min(MAX_TYPO_STATE_COUNT);
            }
        });
        Ok(())
    }

    pub fn record_suppression(&self, input_key: &str) -> Result<()> {
        let mut loaded = self
            .loaded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        record_autocorrect_suppression(&self.path, input_key)?;
        self.reload_after_write(&mut loaded, |state| {
            state.suppressed.insert(input_key.to_owned());
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::journal::test_support::{count, Dir};
    use super::*;

    fn pair(typed: &str, intended: &str) -> (String, String) {
        (typed.to_owned(), intended.to_owned())
    }

    fn typo_count(journal: &Path, typed: &str, intended: &str) -> i64 {
        count(
            journal,
            &format!("SELECT accepted FROM pinyin_typo_counts WHERE typed='{typed}' AND intended='{intended}'"),
        )
    }

    #[test]
    fn accepted_lookup_borrows_both_syllables_without_allocating() {
        let dir = Dir::new();
        let profile = PersonalTypoProfile::shared(&dir.journal());
        profile.record_accepted(&[pair("gan", "guan")]).unwrap();

        let (values, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            (
                profile.accepted("gan", "guan"),
                profile.accepted("sahng", "shang"),
            )
        });

        assert_eq!(values, (1, 0));
        assert_eq!(allocations, 0);
    }

    /// test_typo_correction_input_session.cpp:345-378.
    #[test]
    fn batches_are_validated_and_tables_are_capped() {
        let dir = Dir::new();
        let journal = dir.journal();
        assert!(record_pinyin_typos(&journal, &[pair("gan", "guan"), pair("", "x")]).is_err());
        assert!(record_pinyin_typos(&journal, &[pair("gan", "gan")]).is_err());
        assert!(record_pinyin_typos(&journal, &[pair("zhuangg", "zhuang")]).is_err());
        assert!(record_pinyin_typos(&journal, &[]).is_err());
        assert!(!journal.exists(), "a rejected batch opened the journal");
        assert!(record_autocorrect_suppression(&journal, "Sa'hng").is_err());

        record_pinyin_typos(&journal, &[pair("gan", "guan")]).unwrap();
        let rows: Vec<String> = (0..2100)
            .map(|i: usize| {
                let code: String = (0..3)
                    .map(|digit| (b'a' + (i / 26usize.pow(digit) % 26) as u8) as char)
                    .collect();
                format!("('{code}','{code}z',1)")
            })
            .collect();
        Connection::open(&journal)
            .unwrap()
            .execute_batch(&format!(
                "INSERT INTO pinyin_typo_counts(typed,intended,accepted) VALUES{};",
                rows.join(",")
            ))
            .unwrap();
        record_pinyin_typos(&journal, &[pair("sahng", "shang"), pair("gan", "guan")]).unwrap();
        assert_eq!(
            count(&journal, "SELECT count(*) FROM pinyin_typo_counts"),
            2048
        );
        assert_eq!(typo_count(&journal, "sahng", "shang"), 1);
        assert_eq!(typo_count(&journal, "gan", "guan"), 2);

        let inputs: Vec<String> = (0..2100).map(|i| format!("('input{i}',1)")).collect();
        Connection::open(&journal)
            .unwrap()
            .execute_batch(&format!(
                "INSERT INTO pinyin_autocorrect_suppressions(input,commits) VALUES{};",
                inputs.join(",")
            ))
            .unwrap();
        record_autocorrect_suppression(&journal, "shabg").unwrap();
        assert_eq!(
            count(
                &journal,
                "SELECT count(*) FROM pinyin_autocorrect_suppressions"
            ),
            2048
        );
        assert_eq!(
            count(
                &journal,
                "SELECT count(*) FROM pinyin_autocorrect_suppressions WHERE input='shabg'"
            ),
            1
        );
    }

    #[test]
    fn counts_saturate_at_the_cap() {
        let dir = Dir::new();
        let journal = dir.journal();
        record_pinyin_typos(&journal, &[pair("gan", "guan")]).unwrap();
        Connection::open(&journal)
            .unwrap()
            .execute_batch("UPDATE pinyin_typo_counts SET accepted=1000")
            .unwrap();
        record_pinyin_typos(&journal, &[pair("gan", "guan")]).unwrap();
        assert_eq!(typo_count(&journal, "gan", "guan"), 1000);
    }

    #[test]
    fn loading_a_typo_table_over_the_memory_cap_fails_closed() {
        let dir = Dir::new();
        let journal = dir.journal();
        record_pinyin_typos(&journal, &[pair("gan", "guan")]).unwrap();
        let connection = Connection::open(&journal).unwrap();
        for index in 0..=MAX_TYPO_STATE_ROWS {
            let typed = format!("a{:02}", index % 100);
            let intended = format!("b{:02}", index / 100);
            connection
                .execute(
                    "INSERT INTO pinyin_typo_counts(typed,intended,accepted) VALUES(?1,?2,1)",
                    (&typed, &intended),
                )
                .unwrap();
        }

        assert!(load_pinyin_typo_state(&journal).is_err());
    }

    #[test]
    fn loading_a_suppression_table_over_the_memory_cap_fails_closed() {
        let dir = Dir::new();
        let journal = dir.journal();
        record_autocorrect_suppression(&journal, "shabg").unwrap();
        let connection = Connection::open(&journal).unwrap();
        for index in 0..=MAX_TYPO_STATE_ROWS {
            let input = format!("input{index}");
            connection
                .execute(
                    "INSERT INTO pinyin_autocorrect_suppressions(input,commits) VALUES(?1,1)",
                    [&input],
                )
                .unwrap();
        }

        assert!(load_pinyin_typo_state(&journal).is_err());
    }

    #[test]
    fn a_missing_or_old_journal_reads_as_empty() {
        let dir = Dir::new();
        let nowhere = dir.root.path().join("nowhere").join("msime_user.db");
        assert_eq!(
            load_pinyin_typo_state(&nowhere).unwrap(),
            PinyinTypoState::default()
        );
        assert!(!dir.root.path().join("nowhere").exists());

        let legacy = dir.journal();
        Connection::open(&legacy)
            .unwrap()
            .execute_batch("CREATE TABLE user_dictionary_operations(dictionary TEXT NOT NULL,key TEXT NOT NULL,value TEXT NOT NULL,operation TEXT NOT NULL,PRIMARY KEY(dictionary,key,value));PRAGMA user_version=3;")
            .unwrap();
        assert_eq!(
            load_pinyin_typo_state(&legacy).unwrap(),
            PinyinTypoState::default()
        );
        assert_eq!(
            count(&legacy, "PRAGMA user_version"),
            3,
            "a read upgraded the journal"
        );
    }

    #[test]
    fn the_profile_sees_its_own_writes_and_other_writers() {
        let dir = Dir::new();
        let journal = dir.journal();
        let profile = PersonalTypoProfile::shared(&journal);
        assert!(Arc::ptr_eq(
            &profile,
            &PersonalTypoProfile::shared(&journal)
        ));
        assert_eq!(profile.accepted("gan", "guan"), 0);
        let before = profile.generation();

        assert!(profile.record_accepted(&[pair("gan", "zzz")]).is_err());
        profile.record_accepted(&[pair("gan", "guan")]).unwrap();
        assert_eq!(profile.accepted("gan", "guan"), 1);
        assert!(profile.generation() > before);

        profile.record_suppression("shabg").unwrap();
        assert!(profile.suppressed("shabg"));
        assert!(!profile.suppressed("shang"));

        // Another writer on the same file is seen once the stamp moves.
        let generation = profile.generation();
        record_autocorrect_suppression(&journal, "sahng").unwrap();
        // A one-row write may leave the size alone and land within the file system's time granularity, so the stamp is moved on purpose.
        std::fs::File::options()
            .write(true)
            .open(&journal)
            .unwrap()
            .set_modified(SystemTime::now() + std::time::Duration::from_secs(2))
            .unwrap();
        profile.refresh_if_changed();
        assert!(profile.suppressed("sahng"));
        assert!(profile.generation() > generation);

        // An unchanged file costs a stat and keeps the generation.
        let generation = profile.generation();
        profile.refresh_if_changed();
        assert_eq!(profile.generation(), generation);
    }
}
