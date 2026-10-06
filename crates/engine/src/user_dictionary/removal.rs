//! Deletion and the other single-row writers (user-dictionary.md §9): each attaches the journal to the dictionary connection and commits both in one transaction.

use std::path::Path;

use rusqlite::{params, Connection, TransactionBehavior};

use super::journal::{
    ensure_user_database, open_dictionary_for_writing, record_user_insert, tombstone_sql,
    MISSING_ROW, UNSTORABLE_ENTRY,
};
use super::ngram_store::{delete_personal_ngram_word, flush_journal, forget_journal_rows};
use crate::dictionary::english::ensure_english_schema;
use crate::error::{EngineError, Result};
use crate::types::PersonalDictionaryKind;

const JOURNAL_ALIAS: &str = "candidate_journal";
const MAX_LEARNED_ENGLISH_WORD_LENGTH: usize = 64;

/// The dictionary connection with the journal attached as `candidate_journal`, the journal's schema already applied by `ensure_user_database`.
fn open_with_journal(dictionary: &Path, user_db: &Path) -> Result<Connection> {
    let connection = open_dictionary_for_writing(dictionary)?;
    let journal = user_db
        .to_str()
        .ok_or_else(|| EngineError::invalid(UNSTORABLE_ENTRY))?;
    connection.execute(
        &format!("ATTACH DATABASE ?1 AS {JOURNAL_ALIAS}"),
        params![journal],
    )?;
    Ok(connection)
}

/// Delete the row and journal a tombstone; pinyin also unpins the word everywhere and drops it from the personal n-gram tables (J:1103-1174). Deleting a row that is not in the working dictionary fails and writes nothing.
pub fn delete_dictionary_candidate(
    main_db: &Path,
    user_db: &Path,
    kind: PersonalDictionaryKind,
    key: &str,
    value: &str,
) -> Result<()> {
    if key.is_empty() || value.is_empty() {
        return Err(EngineError::invalid(UNSTORABLE_ENTRY));
    }
    let table = match kind {
        PersonalDictionaryKind::Pinyin => super::journal::pinyin_table(key),
        PersonalDictionaryKind::Wubi | PersonalDictionaryKind::Wubi98 => {
            kind.wubi_table().map(str::to_owned)
        }
        PersonalDictionaryKind::English => Some("english_words".to_owned()),
        PersonalDictionaryKind::QuickPhrase => None,
    }
    .ok_or_else(|| EngineError::invalid(UNSTORABLE_ENTRY))?;
    let pinyin = kind == PersonalDictionaryKind::Pinyin;
    // Initialize the journal before attaching it; every operation owns its connections.
    ensure_user_database(user_db)?;
    // The deletion below has to see the personal context still queued in memory, or it would be written back later.
    if pinyin {
        flush_journal(user_db)?;
    }
    let mut connection = open_with_journal(main_db, user_db)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let columns = if kind == PersonalDictionaryKind::English {
        "word=?1 AND display=?2"
    } else {
        "key=?1 AND value=?2"
    };
    let deleted = transaction.execute(
        &format!("DELETE FROM main.\"{table}\" WHERE {columns}"),
        params![key, value],
    )?;
    if deleted == 0 {
        return Err(EngineError::failed(MISSING_ROW));
    }
    transaction.execute(
        &tombstone_sql(JOURNAL_ALIAS),
        params![kind.journal_name(), key, value],
    )?;
    let mut removed_context = Vec::new();
    if pinyin {
        // A removed word is no longer pinned anywhere.
        transaction.execute(
            &format!("DELETE FROM {JOURNAL_ALIAS}.pinned_candidates WHERE value=?1"),
            params![value],
        )?;
        // A removed word should not keep steering sentences through the context it was typed in.
        removed_context = delete_personal_ngram_word(&transaction, JOURNAL_ALIAS, value)?;
    }
    transaction.commit()?;
    if pinyin {
        forget_journal_rows(user_db, &removed_context);
    }
    Ok(())
}

/// Add an entered English word (1..=64 ASCII letters) at `weight` and journal it as a user insert; a word already known is a no-op success (J:1212-1246).
pub fn learn_entered_english_word(
    english_db: &Path,
    user_db: &Path,
    display: &str,
    weight: i64,
) -> Result<()> {
    if display.is_empty()
        || display.len() > MAX_LEARNED_ENGLISH_WORD_LENGTH
        || !display.bytes().all(|byte| byte.is_ascii_alphabetic())
    {
        return Err(EngineError::invalid(UNSTORABLE_ENTRY));
    }
    let word = display.to_ascii_lowercase();
    let weight = weight.max(0);
    ensure_english_schema(english_db)?;
    let connection = open_dictionary_for_writing(english_db)?;
    let inserted = connection.execute(
        "INSERT OR IGNORE INTO english_words(word,display,weight) VALUES(?1,?2,?3)",
        params![word, display, weight],
    )?;
    if inserted == 0 {
        return Ok(());
    }
    if let Err(error) = record_user_insert(
        user_db,
        PersonalDictionaryKind::English,
        &word,
        display,
        weight,
        display,
    ) {
        // Do not leave an entry that cannot survive a dictionary upgrade. The journal error is what the caller reports; a failed clean-up leaves the word usable until the next upgrade drops it.
        let _ = connection.execute(
            "DELETE FROM english_words WHERE word=?1 AND display=?2",
            params![word, display],
        );
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::journal::test_support::{count, query_i64, Dir};
    use super::super::ngram_store::PersonalNgramStore;
    use super::super::positions::{is_pinned_candidate, record_pinned_candidate};
    use super::*;
    use crate::lattice::personal::PersonalTransition;

    #[test]
    fn deleting_a_pinyin_row_tombstones_unpins_and_forgets_context() {
        let dir = Dir::new();
        dir.pinyin(&[("ni'hao", "你好", 200), ("ni'hao", "拟好", 100)]);
        let journal = dir.journal();
        record_pinned_candidate(&journal, "ni'hao", "拟好").unwrap();
        record_pinned_candidate(&journal, "ni'hao'a", "拟好").unwrap();
        let store = PersonalNgramStore::for_journal(&journal);
        store
            .record(&[PersonalTransition {
                earlier: String::new(),
                previous: String::new(),
                word: "拟好".to_owned(),
                times: 2,
            }])
            .unwrap();

        delete_dictionary_candidate(
            &dir.main_db(),
            &journal,
            PersonalDictionaryKind::Pinyin,
            "ni'hao",
            "拟好",
        )
        .unwrap();
        assert_eq!(dir.weight("ni'hao", "拟好"), None);
        assert_eq!(dir.weight("ni'hao", "你好"), Some(200));
        assert_eq!(
            count(&journal, "SELECT count(*) FROM user_dictionary_operations WHERE operation='delete' AND value='拟好' AND weight=0"),
            1
        );
        assert!(!is_pinned_candidate(&journal, "ni'hao", "拟好"));
        assert!(!is_pinned_candidate(&journal, "ni'hao'a", "拟好"));
        // The queued context was written first, then deleted with the word.
        assert_eq!(count(&journal, "SELECT count(*) FROM personal_bigram"), 0);
        assert!(store.model().is_empty());

        // A row that is not there fails and writes nothing more.
        assert!(delete_dictionary_candidate(
            &dir.main_db(),
            &journal,
            PersonalDictionaryKind::Pinyin,
            "ni'hao",
            "拟好"
        )
        .is_err());
        assert!(delete_dictionary_candidate(
            &dir.main_db(),
            &journal,
            PersonalDictionaryKind::QuickPhrase,
            "a",
            "啊"
        )
        .is_err());
        assert!(delete_dictionary_candidate(
            &dir.main_db(),
            &journal,
            PersonalDictionaryKind::Pinyin,
            "ni''hao",
            "你好"
        )
        .is_err());
        assert_eq!(
            count(&journal, "SELECT count(*) FROM user_dictionary_operations"),
            1
        );
    }

    #[test]
    fn deleting_wubi_and_english_rows() {
        let dir = Dir::new();
        dir.wubi(&[("aaaa", "工", 10), ("aaaa", "或", 5)])
            .english(&[("help", "Help", 100)]);
        let journal = dir.journal();
        delete_dictionary_candidate(
            &dir.main_db(),
            &journal,
            PersonalDictionaryKind::Wubi,
            "aaaa",
            "或",
        )
        .unwrap();
        assert_eq!(count(&dir.main_db(), "SELECT count(*) FROM wubi86"), 1);
        delete_dictionary_candidate(
            &dir.english_db(),
            &journal,
            PersonalDictionaryKind::English,
            "help",
            "Help",
        )
        .unwrap();
        assert_eq!(
            count(&dir.english_db(), "SELECT count(*) FROM english_words"),
            0
        );
        assert_eq!(
            count(
                &journal,
                "SELECT count(*) FROM user_dictionary_operations WHERE operation='delete'"
            ),
            2
        );
    }

    #[test]
    fn a_missing_dictionary_is_not_created() {
        let dir = Dir::new();
        assert!(delete_dictionary_candidate(
            &dir.main_db(),
            &dir.journal(),
            PersonalDictionaryKind::Wubi,
            "a",
            "工"
        )
        .is_err());
        assert!(!dir.main_db().exists());
    }

    #[test]
    fn entered_english_words_are_learned_once() {
        let dir = Dir::new();
        dir.english(&[("help", "Help", 100)]);
        let journal = dir.journal();
        let long = "a".repeat(65);
        for rejected in ["", "don't", "naïve", long.as_str()] {
            assert!(learn_entered_english_word(&dir.english_db(), &journal, rejected, 10).is_err());
        }
        learn_entered_english_word(&dir.english_db(), &journal, "Rustacean", -5).unwrap();
        assert_eq!(
            query_i64(
                &dir.english_db(),
                "SELECT weight FROM english_words WHERE word='rustacean' AND display='Rustacean'"
            ),
            Some(0)
        );
        assert_eq!(
            count(&journal, "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='english' AND key='rustacean' AND value='Rustacean' AND display='Rustacean' AND user_inserted=1"),
            1
        );
        // Known words are left alone and not journaled.
        learn_entered_english_word(&dir.english_db(), &journal, "Help", 10).unwrap();
        assert_eq!(
            count(&journal, "SELECT count(*) FROM user_dictionary_operations"),
            1
        );
    }

    #[test]
    fn an_english_word_the_journal_refuses_is_taken_back() {
        let dir = Dir::new();
        dir.english(&[]);
        let journal = dir.journal();
        std::fs::create_dir(&journal).unwrap();
        assert!(learn_entered_english_word(&dir.english_db(), &journal, "Rustacean", 10).is_err());
        assert_eq!(
            count(&dir.english_db(), "SELECT count(*) FROM english_words"),
            0
        );
    }

    /// test_candidate_removal.cpp:176-184: when the journal refuses the tombstone the row deletion rolls back with it, for pinyin and English rows alike.
    #[test]
    fn a_deletion_the_journal_refuses_is_rolled_back() {
        let dir = Dir::new();
        dir.pinyin(&[("ni'hao", "你好", 200), ("ni'hao", "拟好", 100)]);
        dir.english(&[("help", "help", 50)]);
        let journal = dir.journal();
        ensure_user_database(&journal).unwrap();
        let trigger = "CREATE TRIGGER reject_removal BEFORE INSERT ON user_dictionary_operations BEGIN SELECT RAISE(ABORT,'fixture rejection'); END;";
        rusqlite::Connection::open(&journal)
            .unwrap()
            .execute_batch(trigger)
            .unwrap();

        assert!(delete_dictionary_candidate(
            &dir.main_db(),
            &journal,
            PersonalDictionaryKind::Pinyin,
            "ni'hao",
            "拟好"
        )
        .is_err());
        assert_eq!(dir.weight("ni'hao", "拟好"), Some(100));
        assert!(delete_dictionary_candidate(
            &dir.english_db(),
            &journal,
            PersonalDictionaryKind::English,
            "help",
            "help"
        )
        .is_err());
        assert_eq!(
            count(
                &dir.english_db(),
                "SELECT count(*) FROM english_words WHERE word='help'"
            ),
            1
        );
        assert_eq!(
            count(&journal, "SELECT count(*) FROM user_dictionary_operations"),
            0
        );

        rusqlite::Connection::open(&journal)
            .unwrap()
            .execute_batch("DROP TRIGGER reject_removal;")
            .unwrap();
        delete_dictionary_candidate(
            &dir.main_db(),
            &journal,
            PersonalDictionaryKind::Pinyin,
            "ni'hao",
            "拟好",
        )
        .unwrap();
        assert_eq!(dir.weight("ni'hao", "拟好"), None);
    }
}
