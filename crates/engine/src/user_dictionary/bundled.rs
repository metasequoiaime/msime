//! Browsing and editing shipped dictionary rows (overlays.md §9.3, user-dictionary.md §15.4-§15.5).

use rusqlite::{params, OpenFlags, OptionalExtension, TransactionBehavior};

use crate::assets;
use crate::error::Result;
use crate::format;
use crate::paths::RuntimePaths;
use crate::types::{PersonalDictionaryEntry, PersonalDictionaryKind};
use crate::user_dictionary::journal::{open_database, pinyin_table};
use crate::user_dictionary::personal::{
    check_receipt, column_i64, column_text, failed, open_edit_connection, page_bind, save_receipt,
    valid_request_id, Receipt, DICTIONARY_NOT_OPENED, EDIT_NOT_BEGUN, ENTRY_CHANGED,
    INVALID_REQUEST_ID, MAX_ENTRY_WEIGHT, MAX_PAGE_LIMIT, MAX_PAGE_OFFSET, RETRY_NOT_FINISHED,
    STORAGE_NOT_ATTACHED, STORAGE_UNAVAILABLE,
};
use crate::user_dictionary::replay::attach;

pub const MAX_QUERY_BYTES: usize = 256;

const MAX_BUNDLED_KEY_BYTES: usize = 1_024;
const MAX_BUNDLED_VALUE_BYTES: usize = 4_096;

const INVALID_LOOKUP: &str = "Invalid dictionary lookup";
const DICTIONARY_NOT_READ: &str = "Cannot read dictionary";
const LOOKUP_UNFINISHED: &str = "Cannot finish reading dictionary";
const WEIGHT_OUT_OF_RANGE: &str = "Weight is outside 1 to 100000000";
const INVALID_BUNDLED_ENTRY: &str = "Invalid bundled dictionary entry";
const PERSONAL_NOT_READ: &str = "Cannot read personal dictionary";
const USER_ENTRY: &str = "The entry is a user entry; edit it as one";
const ENTRY_NOT_SAVED: &str = "Cannot save dictionary entry";
const ENTRY_NOT_REMOVED: &str = "Cannot remove dictionary entry";
const EDIT_NOT_JOURNALED: &str = "Cannot journal dictionary edit";
const EDIT_NOT_COMMITTED: &str = "Cannot commit dictionary edit";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryTableEntry {
    pub entry: PersonalDictionaryEntry,
    /// The user's own word, edited through `edit_personal_dictionary`; any other row shipped (or was learned) and only its weight can change.
    pub user_inserted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DictionaryTablePage {
    pub entries: Vec<DictionaryTableEntry>,
    pub has_more: bool,
}

/// Read-only lookup by code prefix: pinyin ignoring separators, spaces and case; `wubi86`; `quick_parases` (an empty query lists every phrase); `english_words`. User rows first, then exact codes, then weight, key, value.
///
/// A query holding a character no code of the kind can contain, and a generation without the dictionary, answer an empty page. Neither the dictionary nor the journal is ever created or written (J:2051-2188).
pub fn dictionary_table_entries(
    paths: &RuntimePaths,
    kind: PersonalDictionaryKind,
    query: &str,
    offset: usize,
    limit: usize,
) -> Result<DictionaryTablePage> {
    if limit == 0
        || limit > MAX_PAGE_LIMIT
        || offset > MAX_PAGE_OFFSET
        || query.len() > MAX_QUERY_BYTES
    {
        return Err(failed(INVALID_LOOKUP));
    }
    let Some(code) = lookup_code(kind, query) else {
        return Ok(DictionaryTablePage::default());
    };
    if code.is_empty() && kind != PersonalDictionaryKind::QuickPhrase {
        return Ok(DictionaryTablePage::default());
    }
    paths.validate().map_err(|_| failed(STORAGE_UNAVAILABLE))?;
    let english = kind == PersonalDictionaryKind::English;
    let dictionary = paths.dictionary(if english {
        assets::ENGLISH_DICTIONARY
    } else {
        assets::MAIN_DICTIONARY
    });
    if !dictionary.exists() {
        return Ok(DictionaryTablePage::default());
    }
    let connection = open_database(&dictionary, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| failed(DICTIONARY_NOT_OPENED))?;
    // Attached through a read-only connection, the journal is read-only too: the lookup never takes a write lock.
    let journal_path = paths.user(assets::USER_JOURNAL);
    let journal = journal_path.exists()
        && {
            attach(&connection, &journal_path, "lookup_journal")
                .map_err(|_| failed(STORAGE_NOT_ATTACHED))?;
            connection
            .query_row(
                "SELECT 1 FROM lookup_journal.sqlite_master WHERE type='table' AND name='user_dictionary_operations'",
                [],
                |_| Ok(()),
            )
            .optional()
            .is_ok_and(|found| found.is_some())
        };

    let tables = lookup_tables(kind, &code);
    let (key_column, value_column) = if english {
        ("word", "display")
    } else {
        ("key", "value")
    };
    let (matches, exact) = match kind {
        PersonalDictionaryKind::Pinyin => (
            "substr(replace(key,'''',''),1,?2)=?1".to_owned(),
            "replace(key,'''','')=?1".to_owned(),
        ),
        PersonalDictionaryKind::QuickPhrase => (
            "substr(lower(key),1,?2)=?1".to_owned(),
            "lower(key)=?1".to_owned(),
        ),
        // Wubi and English codes are stored lower-case, so the prefix is a range the primary index answers.
        PersonalDictionaryKind::Wubi | PersonalDictionaryKind::English => (
            format!("{key_column}>=?1 AND {key_column}<?3"),
            format!("{key_column}=?1"),
        ),
    };
    let mut rows = Vec::new();
    for table in &tables {
        let present = connection
            .query_row(
                "SELECT 1 FROM main.sqlite_master WHERE type='table' AND name=?1",
                [table],
                |_| Ok(()),
            )
            .optional()
            .map_err(|_| failed(DICTIONARY_NOT_READ))?;
        if present.is_some() {
            rows.push(format!(
                "SELECT {key_column} AS key,{value_column} AS value,weight,{exact} AS exact FROM main.\"{table}\" WHERE {matches}"
            ));
        }
    }
    if rows.is_empty() {
        return Ok(DictionaryTablePage::default());
    }
    let mut sql = format!(
        "SELECT r.key,r.value,{} AS entry_weight,{} AS user_row FROM (SELECT key,value,MAX(weight) AS weight,MAX(exact) AS exact FROM ({}) GROUP BY key,value) AS r",
        if journal { "COALESCE(j.weight,r.weight)" } else { "r.weight" },
        if journal { "j.key IS NOT NULL" } else { "0" },
        rows.join(" UNION ALL "),
    );
    if journal {
        sql.push_str(" LEFT JOIN lookup_journal.user_dictionary_operations AS j ON j.dictionary=?4 AND j.key=r.key AND j.value=r.value AND j.user_inserted=1 AND j.operation='upsert'");
    }
    sql.push_str(
        " ORDER BY user_row DESC,r.exact DESC,entry_weight DESC,r.key,r.value LIMIT ?5 OFFSET ?6",
    );
    // The exclusive upper bound of the prefix range: the last byte plus one. Codes are ASCII, so the result is still text.
    let mut upper = code.clone().into_bytes();
    if let Some(last) = upper.last_mut() {
        *last += 1;
    }
    let upper = String::from_utf8(upper).expect("codes are ASCII");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|_| failed(DICTIONARY_NOT_READ))?;
    let mut page = DictionaryTablePage {
        entries: Vec::with_capacity(limit),
        has_more: false,
    };
    let read = (|| -> rusqlite::Result<()> {
        // Every statement binds all six numbers even when a clause does not use one: SQLite counts parameters up to the highest number.
        let mut cursor = statement.query(params![
            code,
            page_bind(code.len()),
            upper,
            kind.journal_name(),
            page_bind(limit + 1),
            page_bind(offset),
        ])?;
        while let Some(row) = cursor.next()? {
            if page.entries.len() == limit {
                page.has_more = true;
                break;
            }
            page.entries.push(DictionaryTableEntry {
                entry: PersonalDictionaryEntry {
                    kind,
                    key: column_text(row, 0)?,
                    value: column_text(row, 1)?,
                    weight: column_i64(row, 2)?,
                },
                user_inserted: column_i64(row, 3)? != 0,
            });
        }
        Ok(())
    })();
    if read.is_err() {
        return Err(failed(LOOKUP_UNFINISHED));
    }
    Ok(page)
}

/// Re-weight (`Some`) or delete (`None`) a row that is not user-inserted, journaled so replay repeats it; `previous.weight` must be the current weight. Request ids behave as for `edit_personal_dictionary`.
///
/// Only the new weight is range-checked: a shipped English weight can lie far above what a user entry may carry, and such a row must still be deletable.
pub fn edit_bundled_dictionary_entry(
    paths: &RuntimePaths,
    previous: &PersonalDictionaryEntry,
    weight: Option<i64>,
    request_id: &str,
) -> Result<()> {
    if !valid_request_id(request_id) {
        return Err(failed(INVALID_REQUEST_ID));
    }
    if weight.is_some_and(|weight| !(1..=MAX_ENTRY_WEIGHT).contains(&weight)) {
        return Err(failed(WEIGHT_OUT_OF_RANGE));
    }
    let Some(table) = bundled_table(previous.kind, &previous.key) else {
        return Err(failed(INVALID_BUNDLED_ENTRY));
    };
    if previous.key.is_empty()
        || previous.value.is_empty()
        || previous.key.len() > MAX_BUNDLED_KEY_BYTES
        || previous.value.len() > MAX_BUNDLED_VALUE_BYTES
    {
        return Err(failed(INVALID_BUNDLED_ENTRY));
    }
    let english = previous.kind == PersonalDictionaryKind::English;
    let target = format!(
        "{}.\"{table}\"",
        if english { "replay_english" } else { "main" }
    );
    let row = if english {
        " WHERE word=?1 AND display=?2"
    } else {
        " WHERE key=?1 AND value=?2"
    };
    let dictionary = previous.kind.journal_name();

    paths.validate().map_err(|_| failed(STORAGE_UNAVAILABLE))?;
    let mut connection = open_edit_connection(paths)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| failed(EDIT_NOT_BEGUN))?;
    // Prefixed so a receipt can never be mistaken for one written by edit_personal_dictionary.
    let payload = format!(
        "bundled;{dictionary};{};{};{}:{}{}:{}",
        previous.weight,
        weight.map_or_else(|| "delete".to_owned(), |weight| weight.to_string()),
        previous.key.len(),
        previous.key,
        previous.value.len(),
        previous.value,
    );
    if !request_id.is_empty()
        && check_receipt(&transaction, request_id, &payload)? == Receipt::Retry
    {
        return transaction.commit().map_err(|_| failed(RETRY_NOT_FINISHED));
    }
    let user_row = transaction
        .query_row(
            "SELECT 1 FROM personal_journal.user_dictionary_operations WHERE dictionary=?1 AND key=?2 AND value=?3 AND user_inserted=1 AND operation='upsert'",
            params![dictionary, previous.key, previous.value],
            |_| Ok(()),
        )
        .optional()
        .map_err(|_| failed(PERSONAL_NOT_READ))?;
    if user_row.is_some() {
        return Err(failed(USER_ENTRY));
    }
    // Pinyin tables have no unique key, so a pair can repeat; the lookup lists the highest weight, and that is what `previous` carries.
    let current = transaction
        .query_row(
            &format!("SELECT MAX(weight) FROM {target}{row}"),
            params![previous.key, previous.value],
            |row| row.get::<_, Option<i64>>(0),
        )
        .map_err(|_| failed(ENTRY_CHANGED))?;
    if current != Some(previous.weight) {
        return Err(failed(ENTRY_CHANGED));
    }
    let changed = match weight {
        Some(weight) => transaction.execute(
            &format!("UPDATE {target} SET weight=?3{row}"),
            params![previous.key, previous.value, weight],
        ),
        None => transaction.execute(
            &format!("DELETE FROM {target}{row}"),
            params![previous.key, previous.value],
        ),
    };
    if !matches!(changed, Ok(count) if count > 0) {
        return Err(failed(if weight.is_some() {
            ENTRY_NOT_SAVED
        } else {
            ENTRY_NOT_REMOVED
        }));
    }
    // The same rows `record_upsert` and `record_delete` write, so replay repeats the change on a fresh dictionary.
    let journaled = match weight {
        Some(weight) => transaction.execute(
            "INSERT INTO personal_journal.user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted) VALUES(?1,?2,?3,'upsert',?4,?5,0) ON CONFLICT(dictionary,key,value) DO UPDATE SET operation='upsert',weight=excluded.weight,display=excluded.display,user_inserted=0, updated_at=unixepoch()",
            params![
                dictionary,
                previous.key,
                previous.value,
                weight,
                if english { previous.value.as_str() } else { "" }
            ],
        ),
        None => transaction.execute(
            "INSERT INTO personal_journal.user_dictionary_operations(dictionary,key,value,operation) VALUES(?1,?2,?3,'delete') ON CONFLICT(dictionary,key,value) DO UPDATE SET operation='delete',weight=0,display='',updated_at=unixepoch()",
            params![dictionary, previous.key, previous.value],
        ),
    };
    if journaled.is_err() {
        return Err(failed(EDIT_NOT_JOURNALED));
    }
    if !request_id.is_empty() {
        save_receipt(&transaction, request_id, &payload)?;
    }
    transaction
        .commit()
        .map_err(|_| failed(EDIT_NOT_COMMITTED))?;
    Ok(())
}

/// The table a row of the kind lives in, `None` for a malformed pinyin key; English rows live in the attached English dictionary. Unlike the learning tables, quick phrases have one here (J:2001-2015).
fn bundled_table(kind: PersonalDictionaryKind, key: &str) -> Option<String> {
    match kind {
        PersonalDictionaryKind::Pinyin => pinyin_table(key),
        PersonalDictionaryKind::Wubi => Some("wubi86".to_owned()),
        PersonalDictionaryKind::QuickPhrase => Some("quick_parases".to_owned()),
        PersonalDictionaryKind::English => Some("english_words".to_owned()),
    }
}

/// Every table a prefix lookup can inspect: all numbered pinyin tables plus
/// the overflow bucket, or the single table used by another dictionary kind.
fn lookup_tables(kind: PersonalDictionaryKind, code: &str) -> Vec<String> {
    let capacity = match kind {
        PersonalDictionaryKind::Pinyin => format::MAXIMUM_NUMBERED_SYLLABLES + 1,
        PersonalDictionaryKind::Wubi
        | PersonalDictionaryKind::QuickPhrase
        | PersonalDictionaryKind::English => 1,
    };
    let mut tables = Vec::with_capacity(capacity);
    match kind {
        // Every syllable count of the query's initial, the overflow bucket included, since separators are ignored and the query does not say how many syllables the key has.
        PersonalDictionaryKind::Pinyin => {
            for syllables in 1..=format::MAXIMUM_NUMBERED_SYLLABLES + 1 {
                if let Some(table) = format::quanpin_table(syllables, code.as_bytes()[0]) {
                    tables.push(table);
                }
            }
        }
        PersonalDictionaryKind::Wubi => tables.push("wubi86".to_owned()),
        PersonalDictionaryKind::QuickPhrase => tables.push("quick_parases".to_owned()),
        PersonalDictionaryKind::English => tables.push("english_words".to_owned()),
    }
    tables
}

/// The query folded to the form codes of the kind are stored in, or `None` when it holds a character no such code can contain. Pinyin drops its separators so a query matches a key however it was split (J:2033-2049).
fn lookup_code(kind: PersonalDictionaryKind, query: &str) -> Option<String> {
    let mut code = String::with_capacity(query.len());
    for byte in query.bytes() {
        let byte = byte.to_ascii_lowercase();
        if kind == PersonalDictionaryKind::Pinyin && (byte == b'\'' || byte == b' ') {
            continue;
        }
        let allowed = byte.is_ascii_lowercase()
            || (kind == PersonalDictionaryKind::QuickPhrase && byte.is_ascii_digit())
            || (kind == PersonalDictionaryKind::English && (byte == b'\'' || byte == b'-'));
        if !allowed {
            return None;
        }
        code.push(byte as char);
    }
    Some(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::EngineError;
    use crate::user_dictionary::personal::{edit_personal_dictionary, personal_dictionary_entries};
    use crate::user_dictionary::replay::replay;
    use crate::user_dictionary::replay::tests::sql;

    /// host-api tests/dictionary_bundled_entries.rs `Fixture`.
    struct Fixture {
        _root: tempfile::TempDir,
        paths: RuntimePaths,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().canonicalize().unwrap();
            let path = |name: &str| {
                let path = root.join(name);
                std::fs::create_dir_all(&path).unwrap();
                path
            };
            let paths = RuntimePaths {
                resources: path("resources"),
                user_data: path("user"),
                cache: path("cache"),
                dictionaries: path("dictionaries"),
            };
            sql(
                &paths.resource(assets::MAIN_DICTIONARY),
                "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_1_n VALUES('ni','n','你',900000);
                 CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',500000);
                 INSERT INTO tbl_2_n VALUES('ni''hao','nh','拟好',1000);
                 INSERT INTO tbl_2_n VALUES('ni''men','nm','你们',400000);
                 CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));
                 INSERT INTO wubi86 VALUES('wqvb','你好',300);
                 CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));
                 INSERT INTO quick_parases VALUES('dh','电话',1);",
            );
            sql(
                &paths.resource(assets::ENGLISH_DICTIONARY),
                "CREATE TABLE english_words(word TEXT COLLATE BINARY NOT NULL,display TEXT NOT NULL,weight INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(word,display)) WITHOUT ROWID;
                 CREATE TABLE en_zh_glosses(english TEXT PRIMARY KEY,chinese_gloss TEXT);
                 CREATE TABLE zh_en_glosses(chinese TEXT PRIMARY KEY,english_gloss TEXT);
                 INSERT INTO english_words VALUES('hello','hello',611054034);",
            );
            let fixture = Self {
                _root: directory,
                paths,
            };
            fixture.install_packaged_dictionaries();
            fixture
        }

        fn install_packaged_dictionaries(&self) {
            for name in [assets::MAIN_DICTIONARY, assets::ENGLISH_DICTIONARY] {
                std::fs::copy(self.paths.resource(name), self.paths.dictionary(name)).unwrap();
            }
        }

        /// What an upgrade does: fresh copies of the packaged dictionaries, then the journal replayed onto them.
        fn upgrade(&self) {
            self.install_packaged_dictionaries();
            let result = replay(
                &self.paths.user(assets::USER_JOURNAL),
                &self.paths.dictionary(assets::MAIN_DICTIONARY),
                &self.paths.dictionary(assets::ENGLISH_DICTIONARY),
            );
            assert!(
                result.applied > 0 && result.failed == 0 && result.error.is_empty(),
                "{}",
                result.error
            );
        }

        fn lookup(&self, kind: PersonalDictionaryKind, query: &str) -> Vec<DictionaryTableEntry> {
            dictionary_table_entries(&self.paths, kind, query, 0, 100)
                .unwrap()
                .entries
        }
    }

    fn row<'a>(rows: &'a [DictionaryTableEntry], value: &str) -> Option<&'a DictionaryTableEntry> {
        rows.iter().find(|row| row.entry.value == value)
    }

    fn values(rows: &[DictionaryTableEntry]) -> Vec<&str> {
        rows.iter().map(|row| row.entry.value.as_str()).collect()
    }

    #[test]
    fn lookup_tables_reserve_the_bounded_table_count() {
        use PersonalDictionaryKind::*;

        let pinyin = lookup_tables(Pinyin, "ni");
        assert_eq!(pinyin.len(), format::MAXIMUM_NUMBERED_SYLLABLES + 1);
        assert_eq!(pinyin.capacity(), format::MAXIMUM_NUMBERED_SYLLABLES + 1);

        let wubi = lookup_tables(Wubi, "wqv");
        assert_eq!(wubi.capacity(), 1);
    }

    #[test]
    fn a_bundled_word_is_found_reweighted_and_deleted_and_the_change_survives_an_upgrade() {
        use PersonalDictionaryKind::*;
        let fixture = Fixture::new();
        for query in ["ni'hao", "nihao", "NiHao", "ni hao"] {
            let rows = fixture.lookup(Pinyin, query);
            assert_eq!(values(&rows), ["你好", "拟好"], "{query}");
            assert!(rows.iter().all(|row| !row.user_inserted), "{query}");
        }
        let partial = fixture.lookup(Pinyin, "nih");
        assert!(row(&partial, "你好").is_some() && row(&partial, "你们").is_none());
        assert!(fixture.lookup(Pinyin, "hao").is_empty());
        // Exact codes lead heavier prefix matches.
        assert_eq!(
            values(&fixture.lookup(Pinyin, "ni")),
            ["你", "你好", "你们", "拟好"]
        );

        let nihao = row(&fixture.lookup(Pinyin, "nihao"), "你好")
            .unwrap()
            .entry
            .clone();
        assert_eq!(nihao.weight, 500_000);
        assert_eq!(
            edit_bundled_dictionary_entry(&fixture.paths, &nihao, Some(100_000_001), "heavy")
                .unwrap_err()
                .to_string(),
            WEIGHT_OUT_OF_RANGE
        );
        edit_bundled_dictionary_entry(&fixture.paths, &nihao, Some(1), "demote-bundled").unwrap();
        edit_bundled_dictionary_entry(&fixture.paths, &nihao, Some(1), "demote-bundled").unwrap();
        assert_eq!(
            edit_bundled_dictionary_entry(&fixture.paths, &nihao, Some(2), "demote-bundled")
                .unwrap_err()
                .to_string(),
            "Request ID was already used for another edit"
        );
        let rows = fixture.lookup(Pinyin, "nihao");
        assert_eq!(rows[0].entry.value, "拟好");
        assert_eq!(row(&rows, "你好").unwrap().entry.weight, 1);
        assert!(!row(&rows, "你好").unwrap().user_inserted);
        // The listed weight is now stale.
        let error = edit_bundled_dictionary_entry(&fixture.paths, &nihao, None, "stale-bundled")
            .unwrap_err();
        assert!(matches!(error, EngineError::Failed(_)));
        assert_eq!(error.to_string(), ENTRY_CHANGED);

        // A learned single character is journaled too, but the pinyin export leaves it out.
        let ni = row(&fixture.lookup(Pinyin, "ni"), "你")
            .unwrap()
            .entry
            .clone();
        edit_bundled_dictionary_entry(&fixture.paths, &ni, Some(2), "demote-single").unwrap();
        let exported = personal_dictionary_entries(&fixture.paths, 0, 1000, true)
            .unwrap()
            .entries;
        assert!(exported
            .iter()
            .any(|entry| entry.value == "你好" && entry.weight == 1));
        assert!(!exported.iter().any(|entry| entry.value == "你"));
        let wubi = row(&fixture.lookup(Wubi, "wqv"), "你好")
            .unwrap()
            .entry
            .clone();
        edit_bundled_dictionary_entry(&fixture.paths, &wubi, Some(3), "demote-wubi").unwrap();
        assert!(!personal_dictionary_entries(&fixture.paths, 0, 1000, false)
            .unwrap()
            .entries
            .iter()
            .any(|entry| entry.kind == Wubi));

        fixture.upgrade();
        assert_eq!(
            row(&fixture.lookup(Pinyin, "nihao"), "你好")
                .unwrap()
                .entry
                .weight,
            1
        );
        assert_eq!(
            row(&fixture.lookup(Wubi, "wqvb"), "你好")
                .unwrap()
                .entry
                .weight,
            3
        );

        let nihao = row(&fixture.lookup(Pinyin, "nihao"), "你好")
            .unwrap()
            .entry
            .clone();
        edit_bundled_dictionary_entry(&fixture.paths, &nihao, None, "delete-bundled").unwrap();
        assert!(row(&fixture.lookup(Pinyin, "nihao"), "你好").is_none());
        fixture.upgrade();
        let rows = fixture.lookup(Pinyin, "nihao");
        assert!(row(&rows, "你好").is_none());
        assert!(row(&rows, "拟好").is_some());
        assert!(!personal_dictionary_entries(&fixture.paths, 0, 1000, true)
            .unwrap()
            .entries
            .iter()
            .any(|entry| entry.value == "你好"));
    }

    #[test]
    fn user_words_lead_the_lookup_and_are_not_edited_as_bundled_rows() {
        use PersonalDictionaryKind::*;
        let fixture = Fixture::new();
        let user = PersonalDictionaryEntry {
            kind: Pinyin,
            key: "nihao".into(),
            value: "妳好".into(),
            weight: 10,
        };
        assert!(edit_personal_dictionary(&fixture.paths, None, Some(&user), "add-user").is_err());
        let user = PersonalDictionaryEntry {
            key: "ni'hao".into(),
            ..user
        };
        edit_personal_dictionary(&fixture.paths, None, Some(&user), "add-user").unwrap();
        let rows = fixture.lookup(Pinyin, "nihao");
        assert_eq!(rows[0].entry, user);
        assert!(rows[0].user_inserted);
        assert!(rows[1..].iter().all(|row| !row.user_inserted));
        assert_eq!(
            edit_bundled_dictionary_entry(&fixture.paths, &user, None, "disguised-user")
                .unwrap_err()
                .to_string(),
            USER_ENTRY
        );
        // A receipt of a personal edit is never taken for a bundled one.
        assert_eq!(
            edit_bundled_dictionary_entry(&fixture.paths, &user, None, "add-user")
                .unwrap_err()
                .to_string(),
            "Request ID was already used for another edit"
        );
    }

    #[test]
    fn quick_phrases_list_whole_and_english_words_are_found_by_prefix() {
        use PersonalDictionaryKind::*;
        let fixture = Fixture::new();
        let phrases = fixture.lookup(QuickPhrase, "");
        assert_eq!(
            phrases,
            [DictionaryTableEntry {
                entry: PersonalDictionaryEntry {
                    kind: QuickPhrase,
                    key: "dh".into(),
                    value: "电话".into(),
                    weight: 1
                },
                user_inserted: false
            }]
        );
        assert_eq!(fixture.lookup(QuickPhrase, "D").len(), 1);
        assert!(fixture.lookup(QuickPhrase, "x").is_empty());
        // A shipped English weight can lie above the range a user entry may have; the row is still listed and can be deleted.
        let words = fixture.lookup(English, "hel");
        assert_eq!(values(&words), ["hello"]);
        assert_eq!(words[0].entry.weight, 611_054_034);
        assert!(fixture.lookup(English, "help").is_empty());
        edit_bundled_dictionary_entry(&fixture.paths, &words[0].entry, None, "delete-english")
            .unwrap();
        assert!(fixture.lookup(English, "hel").is_empty());
        fixture.upgrade();
        assert!(fixture.lookup(English, "hel").is_empty());
        // A code no dictionary of the kind can hold matches nothing rather than failing.
        assert!(fixture.lookup(Wubi, "你").is_empty());
        assert!(fixture.lookup(Pinyin, "").is_empty());
        assert!(fixture.lookup(Wubi, "a1").is_empty());
    }

    #[test]
    fn lookups_validate_their_page_and_never_create_files() {
        use PersonalDictionaryKind::*;
        let fixture = Fixture::new();
        for (offset, limit, query) in [
            (0, 0, "ni"),
            (0, 1001, "ni"),
            (1_000_001, 1, "ni"),
            (0, 1, &"n".repeat(257)[..]),
        ] {
            assert_eq!(
                dictionary_table_entries(&fixture.paths, Pinyin, query, offset, limit)
                    .unwrap_err()
                    .to_string(),
                INVALID_LOOKUP
            );
        }
        let page = dictionary_table_entries(&fixture.paths, Pinyin, "ni", 1, 2).unwrap();
        assert_eq!(values(&page.entries), ["你好", "你们"]);
        assert!(page.has_more);
        assert!(!fixture.paths.user(assets::USER_JOURNAL).exists());
        std::fs::remove_file(fixture.paths.dictionary(assets::MAIN_DICTIONARY)).unwrap();
        assert!(fixture.lookup(Pinyin, "ni").is_empty());
        assert!(!fixture.paths.dictionary(assets::MAIN_DICTIONARY).exists());
    }

    #[test]
    fn bundled_edits_validate_before_touching_storage() {
        use PersonalDictionaryKind::*;
        let fixture = Fixture::new();
        let entry = |key: &str, value: &str| PersonalDictionaryEntry {
            kind: Pinyin,
            key: key.into(),
            value: value.into(),
            weight: 1000,
        };
        for (previous, message) in [
            (entry("ni''hao", "拟好"), INVALID_BUNDLED_ENTRY),
            (entry("", "拟好"), INVALID_BUNDLED_ENTRY),
            (entry("ni'hao", ""), INVALID_BUNDLED_ENTRY),
        ] {
            assert_eq!(
                edit_bundled_dictionary_entry(&fixture.paths, &previous, None, "")
                    .unwrap_err()
                    .to_string(),
                message
            );
        }
        assert_eq!(
            edit_bundled_dictionary_entry(&fixture.paths, &entry("ni'hao", "拟好"), Some(0), "")
                .unwrap_err()
                .to_string(),
            WEIGHT_OUT_OF_RANGE
        );
        assert_eq!(
            edit_bundled_dictionary_entry(&fixture.paths, &entry("ni'hao", "拟好"), None, "bad id")
                .unwrap_err()
                .to_string(),
            INVALID_REQUEST_ID
        );
        assert!(!fixture.paths.user(assets::USER_JOURNAL).exists());
        // A row that is not in the dictionary has no current weight to match.
        assert_eq!(
            edit_bundled_dictionary_entry(&fixture.paths, &entry("ni'hao", "尼好"), None, "")
                .unwrap_err()
                .to_string(),
            ENTRY_CHANGED
        );
    }
}
