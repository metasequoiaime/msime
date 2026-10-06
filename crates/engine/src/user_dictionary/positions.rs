//! Fixed candidate positions and pinned leaders (user-dictionary.md §8).

use std::path::Path;

use rusqlite::params;

use super::journal::{open_journal, UNSTORABLE_ENTRY};
use crate::error::{EngineError, Result};
use crate::types::{CandidateSource, WordItem};

/// Looks a fixed row up by `(entry_key, value)` when it is not in the list; the session passes its engine's `find_candidate`.
pub type FindCandidate<'a> = dyn FnMut(&str, &str) -> Option<WordItem> + 'a;

const FIXED_SLOT_COUNT: usize = 5;

/// Slot 1..=5; the previous owner of the slot is evicted (J:905-927).
pub fn set_fixed_position(
    user_db: &Path,
    context: &str,
    entry_key: &str,
    value: &str,
    position: i32,
) -> Result<()> {
    if !(1..=FIXED_SLOT_COUNT as i32).contains(&position)
        || context.is_empty()
        || entry_key.is_empty()
        || value.is_empty()
    {
        return Err(EngineError::invalid(UNSTORABLE_ENTRY));
    }
    let journal = open_journal(user_db)?;
    journal.execute_batch("BEGIN IMMEDIATE")?;
    journal
        .prepare_cached(
            "DELETE FROM fixed_candidate_positions WHERE context_key=?1 AND position=?2",
        )?
        .execute(params![context, position])?;
    journal
        .prepare_cached(
            "INSERT INTO fixed_candidate_positions(context_key,entry_key,value,position) VALUES(?1,?2,?3,?4) ON CONFLICT(context_key,entry_key,value) DO UPDATE SET position=excluded.position",
        )?
        .execute(params![context, entry_key, value, position])?;
    // Any `?` above leaves the transaction to the connection guard's rollback.
    journal.execute_batch("COMMIT")?;
    Ok(())
}

pub fn clear_fixed_position(
    user_db: &Path,
    context: &str,
    entry_key: &str,
    value: &str,
) -> Result<()> {
    let journal = open_journal(user_db)?;
    journal
        .prepare_cached(
            "DELETE FROM fixed_candidate_positions WHERE context_key=?1 AND entry_key=?2 AND value=?3",
        )?
        .execute(params![context, entry_key, value])?;
    Ok(())
}

/// Opens (creating) the journal like the reference (J:1260-1270); false when it cannot be read.
pub fn is_fixed(user_db: &Path, context: &str, entry_key: &str, value: &str) -> bool {
    let Ok(journal) = open_journal(user_db) else {
        return false;
    };
    journal
        .prepare_cached(
            "SELECT 1 FROM fixed_candidate_positions WHERE context_key=?1 AND entry_key=?2 AND value=?3",
        )
        .and_then(|mut statement| statement.exists(params![context, entry_key, value]))
        .unwrap_or(false)
}

/// The context's fixed rows `(entry_key, value, position)` in slot order.
fn fixed_rows(user_db: &Path, context: &str) -> Result<Vec<(String, String, i32)>> {
    let journal = open_journal(user_db)?;
    let mut statement = journal.prepare_cached(
        "SELECT entry_key,value,position FROM fixed_candidate_positions WHERE context_key=?1 ORDER BY position",
    )?;
    let rows = statement.query_map(params![context], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?;
    let mut result = Vec::with_capacity(FIXED_SLOT_COUNT);
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

/// Move the context's fixed rows into their slots, matching by word; with `include_missing`, rows not in the list are fetched through `find_candidate(entry_key, value)`. Online rows are lifted out and put back at 1 (cloud) and 2 (AI) unless `keep_dynamic_candidate_positions` (J:1272-1346). No journal file means nothing to apply.
pub fn apply_fixed_positions(
    user_db: &Path,
    context: &str,
    candidates: &mut Vec<WordItem>,
    include_missing: bool,
    mut find_candidate: Option<&mut FindCandidate<'_>>,
    keep_dynamic_candidate_positions: bool,
) {
    if context.is_empty() || candidates.is_empty() {
        return;
    }
    // This runs for every candidate list, so a session that never fixed anything must not create the journal on its first keystroke. The reference created it here and found no rows, so the online rows are still re-homed below.
    let fixed = if user_db.try_exists().unwrap_or(false) {
        // An unreadable journal leaves the list as it is: the reference returns before touching it (J:1279-1287), and the list must still be shown.
        let Ok(fixed) = fixed_rows(user_db, context) else {
            return;
        };
        fixed
    } else {
        Vec::new()
    };

    let mut dynamic_candidates = if keep_dynamic_candidate_positions {
        Vec::new()
    } else {
        Vec::with_capacity(candidates.len())
    };
    if !keep_dynamic_candidate_positions {
        candidates.retain(|item| {
            if item.source.is_online() {
                dynamic_candidates.push(item.clone());
                false
            } else {
                true
            }
        });
    }

    let mut rows: Vec<(WordItem, i32)> = Vec::with_capacity(fixed.len());
    for (entry_key, value, position) in fixed {
        let found = match candidates.iter().find(|item| item.word == value) {
            Some(existing) => Some(existing.clone()),
            None if include_missing => find_candidate
                .as_deref_mut()
                .and_then(|find| find(&entry_key, &value)),
            None => None,
        };
        if let Some(mut item) = found {
            item.fixed_position = position;
            rows.push((item, position));
        }
    }
    candidates.retain(|item| !rows.iter().any(|(fixed, _)| fixed.word == item.word));
    for (item, position) in rows {
        let index = usize::try_from(position - 1)
            .unwrap_or(0)
            .min(candidates.len());
        candidates.insert(index, item);
    }
    for item in dynamic_candidates {
        let preferred = if item.source == CandidateSource::CloudSuggestion {
            1
        } else {
            2
        };
        let index = preferred.min(candidates.len());
        candidates.insert(index, item);
    }
}

/// One pinned value per context; a later pin replaces it (J:671-680).
pub fn record_pinned_candidate(user_db: &Path, context: &str, value: &str) -> Result<()> {
    let journal = open_journal(user_db)?;
    journal
        .prepare_cached(
            "INSERT INTO pinned_candidates(context_key,value) VALUES(?1,?2) ON CONFLICT(context_key) DO UPDATE SET value=excluded.value,updated_at=unixepoch()",
        )?
        .execute(params![context, value])?;
    Ok(())
}

/// Never creates the journal (J:682-694).
pub fn is_pinned_candidate(user_db: &Path, context: &str, value: &str) -> bool {
    // A lookup must not create the journal.
    if !user_db.try_exists().unwrap_or(false) {
        return false;
    }
    let Ok(journal) = open_journal(user_db) else {
        return false;
    };
    journal
        .prepare_cached("SELECT 1 FROM pinned_candidates WHERE context_key=?1 AND value=?2")
        .and_then(|mut statement| statement.exists(params![context, value]))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::super::journal::test_support::{item, Dir};
    use super::*;

    fn words(items: &[WordItem]) -> Vec<&str> {
        items.iter().map(|item| item.word.as_str()).collect()
    }

    fn online(word: &str, source: CandidateSource) -> WordItem {
        WordItem::new("", word, 1, source, "")
    }

    #[test]
    fn slots_are_validated_unique_and_cleared() {
        let dir = Dir::new();
        let journal = dir.journal();
        for position in [0, 6] {
            assert!(set_fixed_position(&journal, "ni'hao", "ni'hao", "拟好", position).is_err());
        }
        assert!(set_fixed_position(&journal, "", "ni'hao", "拟好", 1).is_err());
        set_fixed_position(&journal, "ni'hao", "ni'hao", "拟好", 1).unwrap();
        assert!(is_fixed(&journal, "ni'hao", "ni'hao", "拟好"));
        // Taking the slot evicts its holder.
        set_fixed_position(&journal, "ni'hao", "ni'hao", "你好", 1).unwrap();
        assert!(!is_fixed(&journal, "ni'hao", "ni'hao", "拟好"));
        // Moving a row keeps one row per word.
        set_fixed_position(&journal, "ni'hao", "ni'hao", "你好", 3).unwrap();
        let mut list = vec![
            item("ni'hao", "甲", 3),
            item("ni'hao", "乙", 2),
            item("ni'hao", "你好", 1),
        ];
        apply_fixed_positions(&journal, "ni'hao", &mut list, false, None, false);
        assert_eq!(words(&list), ["甲", "乙", "你好"]);
        assert_eq!(list[2].fixed_position, 3);
        clear_fixed_position(&journal, "ni'hao", "ni'hao", "你好").unwrap();
        assert!(!is_fixed(&journal, "ni'hao", "ni'hao", "你好"));
    }

    #[test]
    fn fixed_rows_move_by_word_and_online_rows_return_to_their_slots() {
        let dir = Dir::new();
        let journal = dir.journal();
        set_fixed_position(&journal, "ni", "ni", "丙", 1).unwrap();
        set_fixed_position(&journal, "ni", "ni", "戊", 2).unwrap();
        set_fixed_position(&journal, "ni", "ni", "庚", 5).unwrap();
        let mut list = vec![
            item("ni", "甲", 6),
            online("云", CandidateSource::CloudSuggestion),
            online("智", CandidateSource::AiSuggestion),
            item("ni", "乙", 5),
            item("ni", "丙", 4),
        ];
        let mut found =
            |key: &str, word: &str| (key == "ni" && word == "戊").then(|| item("ni", "戊", 1));
        apply_fixed_positions(&journal, "ni", &mut list, true, Some(&mut found), false);
        assert_eq!(words(&list), ["丙", "云", "智", "戊", "甲", "乙"]);
        assert_eq!(list[0].fixed_position, 1);
        assert_eq!(list[3].fixed_position, 2);

        // Without include_missing the lookup is never asked, and kept dynamic rows stay where they are.
        let mut list = vec![
            item("ni", "甲", 6),
            online("云", CandidateSource::CloudSuggestion),
            item("ni", "丙", 4),
        ];
        let mut never = |_: &str, _: &str| -> Option<WordItem> { panic!("looked up") };
        apply_fixed_positions(&journal, "ni", &mut list, false, Some(&mut never), true);
        assert_eq!(words(&list), ["丙", "甲", "云"]);
    }

    #[test]
    fn fixed_rows_reserve_the_five_available_slots() {
        let dir = Dir::new();
        let journal = dir.journal();
        for position in 1..=5 {
            set_fixed_position(&journal, "ni", "ni", &format!("字{position}"), position).unwrap();
        }

        let rows = fixed_rows(&journal, "ni").unwrap();
        assert_eq!(rows.len(), 5);
        assert_eq!(rows.capacity(), 5);
    }

    #[test]
    fn a_missing_journal_fixes_nothing_and_is_not_created() {
        let dir = Dir::new();
        let journal = dir.journal();
        let mut list = vec![
            item("ni", "甲", 3),
            item("ni", "乙", 2),
            online("智", CandidateSource::AiSuggestion),
            item("ni", "丙", 1),
            online("云", CandidateSource::CloudSuggestion),
        ];
        apply_fixed_positions(&journal, "ni", &mut list, true, None, false);
        // Online rows go back in list order, so the later cloud row pushes the AI row one further down, as in the reference.
        assert_eq!(words(&list), ["甲", "云", "乙", "智", "丙"]);
        assert!(!is_pinned_candidate(&journal, "ni", "甲"));
        assert!(!journal.exists());
    }

    #[test]
    fn a_later_pin_replaces_the_earlier_one() {
        let dir = Dir::new();
        let journal = dir.journal();
        record_pinned_candidate(&journal, "ni'hao", "拟好").unwrap();
        assert!(is_pinned_candidate(&journal, "ni'hao", "拟好"));
        record_pinned_candidate(&journal, "ni'hao", "你好").unwrap();
        assert!(!is_pinned_candidate(&journal, "ni'hao", "拟好"));
        assert!(is_pinned_candidate(&journal, "ni'hao", "你好"));
        assert!(!is_pinned_candidate(&journal, "ni", "你好"));
    }
}
