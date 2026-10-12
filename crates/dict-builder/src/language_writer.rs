//! Shared writer for the language dictionary databases shipped by `languages`.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use msime_engine::language_dictionary;

use crate::sqlite;

pub fn write<'a>(
    path: &Path,
    source_commit: &str,
    license: &str,
    syllables: impl IntoIterator<Item = &'a str>,
    entries: &BTreeMap<(String, String), i64>,
) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
    }
    let mut connection = sqlite::open(path)?;
    connection.execute_batch(language_dictionary::SCHEMA)?;
    let transaction = connection.transaction()?;
    {
        let mut metadata = transaction.prepare("INSERT INTO metadata VALUES (?1, ?2)")?;
        for (name, value) in [
            (
                language_dictionary::METADATA_FORMAT_VERSION,
                language_dictionary::FORMAT_VERSION.to_string().as_str(),
            ),
            (language_dictionary::METADATA_SOURCE_COMMIT, source_commit),
            (language_dictionary::METADATA_LICENSE, license),
        ] {
            metadata.execute((name, value))?;
        }
        let mut syllables_statement = transaction.prepare("INSERT INTO syllables VALUES (?1)")?;
        for syllable in syllables {
            syllables_statement.execute((syllable,))?;
        }
        let mut entries_statement =
            transaction.prepare("INSERT INTO entries VALUES (?1, ?2, ?3)")?;
        for ((key, text), weight) in entries {
            entries_statement.execute((key, text, weight))?;
        }
    }
    transaction.commit()?;
    sqlite::analyze(&connection, false)?;
    sqlite::integrity_check(&connection)?;
    drop(connection);
    sqlite::freeze(path)
}
