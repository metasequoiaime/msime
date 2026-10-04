//! Shift+letter local modes (core-session.md §10, overlays.md §8.1): Unicode code points, date and time, quick phrases, emoji, kaomoji and super jianpin, plus the emoji / kaomoji / symbol catalog the host's picker pages through. Quick phrase and jianpin read the generation's `msime-pinyin.db`; emoji and kaomoji read the resource `others.db`. The expression (`V`), command (`/`) and mention (`@`) modes compute their rows from the input, the clock and host-supplied tables, and read nothing from disk.

pub mod catalog;
pub mod command;
pub mod database;
pub mod date_time;
pub mod emoji;
pub mod expression;
pub mod jianpin;
pub mod mention;
pub mod places;
pub mod quick_phrase;
pub mod unicode;
pub mod units;

use crate::types::WordItem;

/// The longest preedit, prefix included, the expression, command and mention modes take; further keys are swallowed. Their rows are computed on every key, so the bound is what keeps a held key from making that work grow.
pub const GENERATED_MODE_INPUT_LIMIT: usize = 64;

/// A local query's rows and, when the database could not be read, the diagnostic the key result carries.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LocalQueryResult {
    pub candidates: Vec<WordItem>,
    pub diagnostic: Option<String>,
}

impl LocalQueryResult {
    fn failure(diagnostic: &str) -> Self {
        Self {
            candidates: Vec::new(),
            diagnostic: Some(diagnostic.to_owned()),
        }
    }
}

/// A row limit as an SQLite integer. Limits are small constants, so saturation never changes a result.
fn sql_limit(limit: usize) -> i64 {
    i64::try_from(limit).unwrap_or(i64::MAX)
}
