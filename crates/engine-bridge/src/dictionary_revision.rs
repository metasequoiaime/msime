use sha2::{Digest, Sha256};

pub(crate) struct DictionaryRevision(Sha256);

impl DictionaryRevision {
    fn new() -> Self {
        let mut value = Self(Sha256::new());
        value.text("msime-local-dictionary-state-v1");
        value
    }
    pub(crate) fn integer(&mut self, value: u64) {
        self.0.update(value.to_be_bytes());
    }
    pub(crate) fn text(&mut self, value: &str) {
        self.integer(value.len() as u64);
        self.0.update(value.as_bytes());
    }
    fn finish(self) -> String {
        hex::encode(self.0.finalize())
    }
}

/// Digest of one consistent Engine journal transaction, not the input preedit.
/// Pair with the active installation generation and recheck under an exclusive
/// dictionary lease before publishing a replacement. Contains no raw user text.
pub fn dictionary_state_revision(options: &super::EngineOptions) -> Result<String, cxx::Exception> {
    let mut digest = DictionaryRevision::new();
    super::ffi::hash_dictionary_state(options, &mut digest)?;
    Ok(digest.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_is_length_prefixed_and_big_endian() {
        let mut actual = DictionaryRevision::new();
        actual.text("synthetic");
        actual.integer(258);
        let mut expected = Sha256::new();
        let domain = b"msime-local-dictionary-state-v1";
        expected.update((domain.len() as u64).to_be_bytes());
        expected.update(domain);
        expected.update(9_u64.to_be_bytes());
        expected.update(b"synthetic");
        expected.update([0, 0, 0, 0, 0, 0, 1, 2]);
        assert_eq!(actual.finish(), hex::encode(expected.finalize()));
    }

    #[test]
    fn text_boundaries_cannot_collide() {
        let mut first = DictionaryRevision::new();
        first.text("ab");
        first.text("c");
        let mut second = DictionaryRevision::new();
        second.text("a");
        second.text("bc");
        assert_ne!(first.finish(), second.finish());
    }

    #[test]
    fn real_journal_covers_every_record_and_field_without_mutation() {
        let dir = tempfile::tempdir().unwrap();
        let options = crate::tests::options(dir.path());
        let journal = std::path::Path::new(&options.user_data).join("msime_user.db");
        let db = rusqlite::Connection::open(&journal).unwrap();
        // Synthetic fixture for the pinned Engine journal reader; no product data.
        db.execute_batch(
            "CREATE TABLE user_dictionary_operations (
                dictionary TEXT, key TEXT, value TEXT, weight INTEGER,
                display TEXT, operation TEXT, user_inserted INTEGER);
             CREATE TABLE fixed_candidate_positions (
                context_key TEXT, entry_key TEXT, value TEXT, position INTEGER);
             CREATE TABLE candidate_selection_state (
                context_key TEXT, entry_key TEXT, value TEXT, selection_count INTEGER);
             INSERT INTO user_dictionary_operations VALUES
                ('wubi', 'synthetic-key', '测试', 258, 'fixture', 'upsert', 1),
                ('quick', 'synthetic-key', '测试', 258, 'fixture', 'upsert', 1),
                ('pinyin', 'synthetic-key', '测试', 258, 'fixture', 'upsert', 1),
                ('english', 'synthetic-key', '测试', -2, 'fixture', 'delete', 0);
             INSERT INTO fixed_candidate_positions VALUES ('ctx', 'key', '测试', 3);
             INSERT INTO candidate_selection_state VALUES ('ctx', 'key', '测试', 7);",
        )
        .unwrap();
        let mut expected = DictionaryRevision::new();
        for kind in ["english", "pinyin", "quick", "wubi"] {
            expected.text("entry");
            expected.text(kind);
            expected.text("synthetic-key");
            expected.text("测试");
            expected.integer(if kind == "english" {
                (-2_i64) as u64
            } else {
                258
            });
            expected.text("fixture");
            expected.integer(u64::from(kind == "english"));
            expected.integer(u64::from(kind != "english"));
        }
        for (kind, number) in [("position", 3), ("selection", 7)] {
            expected.text(kind);
            expected.text("ctx");
            expected.text("key");
            expected.text("测试");
            expected.integer(number);
        }
        drop(db);
        let original = std::fs::read(&journal).unwrap();
        let revision = dictionary_state_revision(&options).unwrap();
        assert_eq!(revision, expected.finish());
        assert_eq!(std::fs::read(&journal).unwrap(), original);
        for update in [
            "UPDATE user_dictionary_operations SET dictionary='quick' WHERE dictionary='english'",
            "UPDATE user_dictionary_operations SET key='changed'",
            "UPDATE user_dictionary_operations SET value='changed'",
            "UPDATE user_dictionary_operations SET weight=259",
            "UPDATE user_dictionary_operations SET display='changed'",
            "UPDATE user_dictionary_operations SET operation='delete'",
            "UPDATE user_dictionary_operations SET user_inserted=0",
            "UPDATE fixed_candidate_positions SET context_key='changed'",
            "UPDATE fixed_candidate_positions SET entry_key='changed'",
            "UPDATE fixed_candidate_positions SET value='changed'",
            "UPDATE fixed_candidate_positions SET position=4",
            "UPDATE candidate_selection_state SET context_key='changed'",
            "UPDATE candidate_selection_state SET entry_key='changed'",
            "UPDATE candidate_selection_state SET value='changed'",
            "UPDATE candidate_selection_state SET selection_count=8",
        ] {
            let db = rusqlite::Connection::open(&journal).unwrap();
            db.execute_batch(update).unwrap();
            drop(db);
            assert_ne!(dictionary_state_revision(&options).unwrap(), revision);
            // Restore only this test-owned file, with all SQLite connections closed.
            std::fs::write(&journal, &original).unwrap();
            assert_eq!(dictionary_state_revision(&options).unwrap(), revision);
        }
    }
}
