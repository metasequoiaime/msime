//! Common database checks for the three shipped language dictionaries.

use std::path::Path;

use anyhow::{bail, Context, Result};
use msime_engine::language_dictionary::{self, LanguageDictionary};
use rusqlite::Connection;

use crate::sqlite;

pub fn open(path: &Path) -> Result<(LanguageDictionary, Connection)> {
    let dictionary = language_dictionary::open_read_only(path)
        .map_err(|error| anyhow::anyhow!("{}: {error}", path.display()))?;
    let connection =
        sqlite::open_read_only(path).with_context(|| format!("opening {}", path.display()))?;
    Ok((dictionary, connection))
}

pub fn count(connection: &Connection, sql: &str) -> Result<usize> {
    let count: i64 = connection.query_row(sql, [], |row| row.get(0))?;
    Ok(usize::try_from(count)?)
}

pub fn check_floors(path: &Path, checks: &[(&str, usize, usize)]) -> Result<()> {
    for &(what, found, floor) in checks {
        if found < floor {
            bail!(
                "{}: {found} {what}, below the floor of {floor}",
                path.display()
            );
        }
    }
    Ok(())
}
