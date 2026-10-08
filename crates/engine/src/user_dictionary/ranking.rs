//! Frequency learning (user-dictionary.md §5-§7, overlays.md §9.1): the selection counter, the target slot per mode, the weight arithmetic with its rebalance, and the journal/dictionary writes. The ranking set is every visible dictionary row stable-sorted by weight.

use std::path::Path;

use rusqlite::{params, Connection, Transaction, TransactionBehavior};

use super::journal::{
    open_dictionary_for_writing, open_journal, pinyin_segments, pinyin_table, write_upsert,
    MISSING_ROW, UNSTORABLE_ENTRY,
};
use super::positions::is_fixed;
use crate::error::{EngineError, Result};
use crate::types::{CandidateSource, FrequencyAdjustmentMode, PersonalDictionaryKind, WordItem};

pub const MANAGED_WEIGHT_CEILING: i64 = 100_000_000;
pub const MANAGED_WEIGHT_FLOOR: i64 = 1;
pub const REBALANCE_GAP: i64 = 1_000;
pub const REBALANCE_COUNT: usize = 16;

/// A fixed candidate keeps its slot by position, not by weight, so learning never moves it (J:1356-1358).
const FIXED_CANDIDATE: &str = "The candidate has a fixed position";

/// One learning pick. `main_db` is `msime-pinyin.db` for pinyin and wubi and `msime-english.db` for English.
#[derive(Debug, Clone, Copy)]
pub struct RankingRequest<'a> {
    pub main_db: &'a Path,
    pub user_db: &'a Path,
    pub context_key: &'a str,
    /// The candidates in ranking (un-reranked) order; mixed wubi passes only the selected row's producer.
    pub ordered: &'a [WordItem],
    pub entry_key: &'a str,
    pub value: &'a str,
    pub mode: FrequencyAdjustmentMode,
    pub linear_step: i32,
    pub trigger_count: i32,
    /// Pin: no counter, straight to the top.
    pub force_top: bool,
    pub kind: PersonalDictionaryKind,
}

/// Keep learned weights in the same general range as the shipped dictionary (J:214-221).
pub(crate) fn clamp_managed_weight(weight: i64) -> i64 {
    weight.clamp(MANAGED_WEIGHT_FLOOR, MANAGED_WEIGHT_CEILING)
}

const COUNT_SELECTION_SQL: &str = "INSERT INTO candidate_selection_state(context_key,entry_key,value,selection_count) VALUES(?1,?2,?3,1) ON CONFLICT(context_key,entry_key,value) DO UPDATE SET selection_count=selection_count+1 RETURNING selection_count";
const RESET_SELECTION_SQL: &str =
    "DELETE FROM candidate_selection_state WHERE context_key=?1 AND entry_key=?2 AND value=?3";

/// Shared opening of both writers: the argument checks, the fixed-row refusal, the disabled no-op, and the journal transaction with the selection counter. `None` means nothing more to do; otherwise the counter reached the trigger (or the pick is forced) and the caller ranks.
fn begin_ranking<'a>(
    request: &RankingRequest<'_>,
    journal: &'a Connection,
) -> Result<Option<Transaction<'a>>> {
    let transaction = Transaction::new_unchecked(journal, TransactionBehavior::Immediate)?;
    if !request.force_top {
        let trigger_count = i64::from(request.trigger_count.clamp(1, 10));
        let count: i64 = transaction.prepare_cached(COUNT_SELECTION_SQL)?.query_row(
            params![request.context_key, request.entry_key, request.value],
            |row| row.get(0),
        )?;
        if count < trigger_count {
            transaction.commit()?;
            return Ok(None);
        }
    }
    Ok(Some(transaction))
}

fn check_request(request: &RankingRequest<'_>) -> Result<()> {
    if request.entry_key.is_empty() || request.value.is_empty() || request.ordered.is_empty() {
        return Err(EngineError::invalid(UNSTORABLE_ENTRY));
    }
    // Runs before the disabled check and opens (creates) the journal, as the reference does.
    if is_fixed(
        request.user_db,
        request.context_key,
        request.entry_key,
        request.value,
    ) {
        return Err(EngineError::invalid(FIXED_CANDIDATE));
    }
    Ok(())
}

fn reset_selection(transaction: &Transaction<'_>, request: &RankingRequest<'_>) -> Result<()> {
    transaction
        .prepare_cached(RESET_SELECTION_SQL)?
        .execute(params![
            request.context_key,
            request.entry_key,
            request.value
        ])?;
    Ok(())
}

/// J:284-295. Promote always moves at least one slot and at most to the fifth.
fn ranking_target(
    rank: usize,
    mode: FrequencyAdjustmentMode,
    linear_step: i32,
    force_top: bool,
) -> usize {
    if force_top {
        return 0;
    }
    match mode {
        FrequencyAdjustmentMode::Halve => rank / 2,
        FrequencyAdjustmentMode::Linear => {
            let step = usize::try_from(linear_step).unwrap_or(0);
            rank.saturating_sub(step)
        }
        FrequencyAdjustmentMode::Promote => {
            if rank > 4 {
                4
            } else {
                rank - 1
            }
        }
        FrequencyAdjustmentMode::Disabled | FrequencyAdjustmentMode::Pin => 0,
    }
}

/// J:251-262. Does not check that a row changed: the reference journals the weight either way, and replay then inserts the row.
fn update_pinyin_weight(
    main: &Connection,
    journal: &Connection,
    key: &str,
    value: &str,
    weight: i64,
) -> Result<()> {
    let weight = clamp_managed_weight(weight);
    let table = pinyin_table(key).ok_or_else(|| EngineError::invalid(UNSTORABLE_ENTRY))?;
    main.prepare_cached(&format!(
        "UPDATE \"{table}\" SET weight=?1 WHERE key=?2 AND value=?3"
    ))?
    .execute(params![weight, key, value])?;
    write_upsert(
        journal,
        PersonalDictionaryKind::Pinyin,
        key,
        value,
        weight,
        "",
    )
}

/// J:264-274. Unlike pinyin, the row must exist before it is journaled.
fn update_wubi_weight(
    main: &Connection,
    journal: &Connection,
    kind: PersonalDictionaryKind,
    key: &str,
    value: &str,
    weight: i64,
) -> Result<()> {
    let weight = clamp_managed_weight(weight);
    let Some(table) = kind.wubi_table() else {
        return Err(EngineError::invalid(UNSTORABLE_ENTRY));
    };
    if key.is_empty() || value.is_empty() {
        return Err(EngineError::invalid(UNSTORABLE_ENTRY));
    }
    let changed = main
        .prepare_cached(&format!(
            "UPDATE {table} SET weight=?1 WHERE key=?2 AND value=?3"
        ))?
        .execute(params![weight, key, value])?;
    if changed == 0 {
        return Err(EngineError::failed(MISSING_ROW));
    }
    write_upsert(journal, kind, key, value, weight, "")
}

fn update_ranked_weight(
    main: &Connection,
    journal: &Connection,
    kind: PersonalDictionaryKind,
    key: &str,
    value: &str,
    weight: i64,
) -> Result<()> {
    if kind.is_wubi() {
        update_wubi_weight(main, journal, kind, key, value, weight)
    } else {
        update_pinyin_weight(main, journal, key, value, weight)
    }
}

/// The weights to write: the selected row's, and the staircase over `[target, target + 16)` of the rows that own the entry key, when the neighbours leave no room (J:1504-1608).
#[derive(Debug, PartialEq, Eq)]
struct WeightPlan {
    selected: i64,
    /// `(index into the weight-sorted set, weight before clamping)`.
    staircase: Vec<(usize, i64)>,
}

fn plan_weights(
    weights: &[i64],
    owns_entry_key: impl Fn(usize) -> bool,
    target: usize,
) -> WeightPlan {
    let top_near_limit = target == 0 && weights[0] > i64::MAX - REBALANCE_GAP;
    let upper = if target == 0 {
        if top_near_limit {
            weights[0]
        } else {
            weights[0] + REBALANCE_GAP
        }
    } else {
        weights[target - 1]
    };
    let lower = weights[target];
    let oversized = upper > MANAGED_WEIGHT_CEILING || lower > MANAGED_WEIGHT_CEILING;
    let mut need_rebalance = top_near_limit || lower == i64::MAX || upper <= lower || oversized;
    let mut new_weight = 0;
    if !need_rebalance {
        if upper == lower + 1 {
            // There is no integer midpoint. Move one point above the upper candidate and skip a short run of occupied weights. This may advance one extra position, but keeps ordering deterministic without paying for a rebalance in the common case.
            new_weight = upper + 1;
            let mut conflicts = 0;
            for &occupied in weights[..target].iter().rev() {
                if occupied < new_weight {
                    continue;
                }
                if occupied > new_weight {
                    break;
                }
                conflicts += 1;
                if conflicts >= REBALANCE_COUNT || new_weight >= MANAGED_WEIGHT_CEILING {
                    need_rebalance = true;
                    break;
                }
                new_weight += 1;
            }
        } else {
            // `upper > lower + 1` here, so half the gap fits and the sum stays below `upper`.
            new_weight = lower + (upper.abs_diff(lower) / 2) as i64;
        }
    }
    let rebalance_end = weights.len().min(target + REBALANCE_COUNT);
    // Lift the selected row above the whole cluster and leave every other row exactly where it is. Does nothing when the cluster sits within one gap of the integer ceiling; the rebalance then stays needed and the ceiling compact path below takes over.
    let promote_selection_alone = |new_weight: &mut i64, need_rebalance: &mut bool| {
        let cluster = upper.max(lower);
        if cluster > i64::MAX - REBALANCE_GAP {
            return;
        }
        *new_weight = clamp_managed_weight(cluster + REBALANCE_GAP);
        *need_rebalance = false;
    };
    let gap = |slots: usize| REBALANCE_GAP.saturating_mul(i64::try_from(slots).unwrap_or(i64::MAX));
    if need_rebalance && !oversized && !top_near_limit && target != 0 {
        let last_index = rebalance_end - 1;
        let last_weight = upper
            .saturating_sub(REBALANCE_GAP)
            .saturating_sub(gap(last_index - target));
        // Equal/low weights have no room for a 16-slot descending staircase. Promote only the selected row instead of writing negatives onto neighbors (and onto shorter-syllable singles mixed into the UI list).
        if last_weight < MANAGED_WEIGHT_FLOOR {
            promote_selection_alone(&mut new_weight, &mut need_rebalance);
        }
    }
    if need_rebalance {
        // The staircase below only demotes rows whose own key is the entry key; a row belonging to another key keeps the weight it already has. The selection's weight is built on the assumption that index `target` was demoted by that loop, so a staircase with a hole in it writes the selection underneath a row that never moved and a tie turns into last place. The staircase also compacts the whole window up against `upper`, which would lift rows from the bottom of the window over a foreign row it is not allowed to touch. When the window is not all ours, promote the selected row on its own the way the low-weight cluster above does.
        if !(target..rebalance_end).all(&owns_entry_key) {
            promote_selection_alone(&mut new_weight, &mut need_rebalance);
        }
    }
    let mut staircase = Vec::with_capacity(rebalance_end.saturating_sub(target));
    if need_rebalance {
        let mut base = if target == 0 {
            MANAGED_WEIGHT_CEILING - REBALANCE_GAP
        } else {
            MANAGED_WEIGHT_CEILING
        };
        if target != 0 && !top_near_limit && !oversized {
            base = upper
                .saturating_add(gap(target))
                .saturating_sub(REBALANCE_GAP)
                .min(MANAGED_WEIGHT_CEILING);
        }
        // Absolute indices: the first demoted row lands one gap under `upper`.
        for index in target..rebalance_end {
            if owns_entry_key(index) {
                staircase.push((index, base.saturating_sub(gap(index))));
            }
        }
        new_weight = if target == 0 {
            base + REBALANCE_GAP
        } else {
            base.saturating_sub(gap(target)) + REBALANCE_GAP / 2
        };
    }
    WeightPlan {
        selected: new_weight,
        staircase,
    }
}

/// J:1348-1640. `Ok(true)` when a dictionary weight changed, so the caller resets its caches. A fixed candidate, empty key or value, or any write failure is an error the caller reports as `FREQUENCY_NOT_PERSISTED`.
pub fn adjust_candidate_ranking(request: &RankingRequest<'_>) -> Result<bool> {
    check_request(request)?;
    if !request.force_top && request.mode == FrequencyAdjustmentMode::Disabled {
        return Ok(false);
    }
    let journal = open_journal(request.user_db)?;
    let Some(transaction) = begin_ranking(request, &journal)? else {
        return Ok(false);
    };

    // One list mixes several entry keys: the context "y" shows 一/yi beside 有/you, a nine-key digit mixes whole single-character tables, and a re-segmentation puts 吉安 (ji'an) inside the jian list. All of them share one scale, because the user's 调频 means "move this above what I can see".
    //
    // The midpoint arithmetic assumes this set is sorted by weight, descending: it brackets the target position between its neighbours' weights and splits the gap. The displayed order does not satisfy that - an alternative segmentation takes a protected slot near the top and pinned candidates sit at fixed positions, both regardless of weight - and ranking on display order is what wrote 写 as 506, being 西鄂's weight of 6 plus 500. Sorting by weight fixes it at the source: no row can sit above its own weight and be the basis.
    //
    // Restricting the set to one syllable count kept 西鄂 out too, but walled every candidate into its own group: 吉安 (weight 1) could only be compared against 积案 and 几案, so its learnable weight was capped around ten thousand and it could never pass 见 at 3460998 however often it was picked. Weight order removes the wall without bringing the bug back. Writes still go to entry-key rows only, so a rebalance cannot land on another key's row the way it did in #36.
    let mut database_candidates: Vec<&WordItem> = request
        .ordered
        .iter()
        .filter(|item| item.source.is_dictionary())
        .collect();
    database_candidates.sort_by_key(|item| std::cmp::Reverse(item.weight));
    let owns_entry_key = |index: usize| {
        let item = database_candidates[index];
        if request.kind.is_wubi() {
            item.pinyin == request.entry_key
        } else {
            candidate_dictionary_key(item, request.context_key) == request.entry_key
        }
    };
    // By word only, whatever key the row was read under.
    let rank = database_candidates
        .iter()
        .position(|item| item.word == request.value)
        .ok_or_else(|| EngineError::failed(MISSING_ROW))?;
    if rank == 0 {
        // Already the heaviest: the counter has done its job and no weight moves.
        reset_selection(&transaction, request)?;
        transaction.commit()?;
        return Ok(false);
    }
    let target = ranking_target(rank, request.mode, request.linear_step, request.force_top);
    let mut main = open_dictionary_for_writing(request.main_db)?;
    let main_transaction = main.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let weights: Vec<i64> = database_candidates.iter().map(|item| item.weight).collect();
    let plan = plan_weights(&weights, owns_entry_key, target);
    for (index, weight) in plan.staircase {
        update_ranked_weight(
            &main_transaction,
            &transaction,
            request.kind,
            request.entry_key,
            &database_candidates[index].word,
            weight,
        )?;
    }
    update_ranked_weight(
        &main_transaction,
        &transaction,
        request.kind,
        request.entry_key,
        request.value,
        plan.selected,
    )?;
    reset_selection(&transaction, request)?;
    // The journal commits first: a journal row one step ahead of the dictionary is recoverable by replay, a dictionary weight the journal never saw is lost at the next upgrade. A failed dictionary commit rolls back the dictionary only.
    transaction.commit()?;
    main_transaction.commit()?;
    Ok(true)
}

/// J:929-1101; `ordered` holds English rows only and `kind` is ignored.
pub fn adjust_english_candidate_ranking(request: &RankingRequest<'_>) -> Result<()> {
    check_request(request)?;
    if !request.force_top && request.mode == FrequencyAdjustmentMode::Disabled {
        return Ok(());
    }
    let journal = open_journal(request.user_db)?;
    let Some(transaction) = begin_ranking(request, &journal)? else {
        return Ok(());
    };
    let ordered = request.ordered;
    let rank = ordered
        .iter()
        .position(|item| {
            item.source == CandidateSource::EnglishDictionary
                && item.pinyin == request.entry_key
                && item.word == request.value
        })
        .ok_or_else(|| EngineError::failed(MISSING_ROW))?;
    if rank == 0 {
        reset_selection(&transaction, request)?;
        transaction.commit()?;
        return Ok(());
    }
    let step = usize::try_from(request.linear_step.max(1)).unwrap_or(1);
    let target = if request.force_top {
        0
    } else {
        match request.mode {
            FrequencyAdjustmentMode::Halve => rank / 2,
            FrequencyAdjustmentMode::Linear => rank.saturating_sub(step),
            FrequencyAdjustmentMode::Promote => {
                if rank > 4 {
                    4
                } else {
                    rank - 1
                }
            }
            FrequencyAdjustmentMode::Disabled | FrequencyAdjustmentMode::Pin => 0,
        }
    };
    // English corpus weights live on their own scale (up to 2.3e10), so they are neither clamped nor rebalanced.
    let maximum = ordered.iter().map(|item| item.weight).fold(0, i64::max);
    let new_weight = if target == 0 {
        maximum.max(ordered[rank].weight).saturating_add(1000)
    } else {
        ordered[target - 1].weight.saturating_add(1)
    };

    let english = open_dictionary_for_writing(request.main_db)?;
    // Journal the new weight before msime-english.db carries it. The journal upsert is idempotent and replay reapplies it, so a journal row one step ahead of the dictionary is recoverable, while a boosted weight that never reached the journal is silently reverted at the next dictionary upgrade. The row must already exist, otherwise replay would insert a word the user never learned.
    let exists = english
        .prepare_cached("SELECT 1 FROM english_words WHERE word=?1 AND display=?2")?
        .exists(params![request.entry_key, request.value])?;
    if !exists {
        return Err(EngineError::failed(MISSING_ROW));
    }
    write_upsert(
        &transaction,
        PersonalDictionaryKind::English,
        request.entry_key,
        request.value,
        new_weight,
        request.value,
    )?;
    reset_selection(&transaction, request)?;
    transaction.commit()?;
    let changed = english
        .prepare_cached("UPDATE english_words SET weight=?1 WHERE word=?2 AND display=?3")?
        .execute(params![new_weight, request.entry_key, request.value])?;
    if changed == 0 {
        return Err(EngineError::failed(MISSING_ROW));
    }
    Ok(())
}

/// The key a row is stored under for a context (J:229-249).
pub fn candidate_dictionary_key(item: &WordItem, context_key: &str) -> String {
    if !item.canonical_pinyin.is_empty() {
        return item.canonical_pinyin.clone();
    }
    let mut segments = pinyin_segments(context_key);
    if segments.len() <= 1 {
        return item.pinyin.clone();
    }
    // A context "ni'hao'a" answered by the two-character row 你好 stores it under the first two syllables.
    let characters = item.word.chars().count();
    if characters > 0 && segments.len() > characters {
        segments.truncate(characters);
    }
    segments.join("'")
}

#[cfg(test)]
mod tests {
    use super::super::journal::test_support::{count, item, query_i64, Dir};
    use super::*;
    use rusqlite::Connection as Sqlite;

    const FQ: [(&str, i64); 6] = [
        ("甲", 100),
        ("乙", 90),
        ("丙", 80),
        ("丁", 70),
        ("戊", 60),
        ("己", 50),
    ];

    /// Fixture FQ (test_input_session.cpp:131-144): six `ni` rows.
    fn frequency_fixture() -> Dir {
        let dir = Dir::new();
        let rows: Vec<(&str, &str, i64)> = FQ
            .iter()
            .map(|(word, weight)| ("ni", *word, *weight))
            .collect();
        dir.pinyin(&rows);
        dir
    }

    /// The list a fresh `ni` query shows: every `ni` row by weight.
    fn listed(dir: &Dir) -> Vec<WordItem> {
        let connection = Sqlite::open(dir.main_db()).unwrap();
        let mut statement = connection
            .prepare("SELECT key,value,weight FROM tbl_1_n ORDER BY weight DESC")
            .unwrap();
        statement
            .query_map([], |row| {
                Ok(item(
                    &row.get::<_, String>(0)?,
                    &row.get::<_, String>(1)?,
                    row.get(2)?,
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    fn pick(
        dir: &Dir,
        word: &str,
        mode: FrequencyAdjustmentMode,
        linear_step: i32,
        trigger_count: i32,
    ) -> Result<bool> {
        let ordered = listed(dir);
        let main_db = dir.main_db();
        let user_db = dir.journal();
        adjust_candidate_ranking(&RankingRequest {
            main_db: &main_db,
            user_db: &user_db,
            context_key: "ni",
            ordered: &ordered,
            entry_key: "ni",
            value: word,
            mode,
            linear_step,
            trigger_count,
            force_top: mode == FrequencyAdjustmentMode::Pin,
            kind: PersonalDictionaryKind::Pinyin,
        })
    }

    fn index_of(dir: &Dir, word: &str) -> usize {
        listed(dir)
            .iter()
            .position(|item| item.word == word)
            .unwrap()
    }

    /// F1 (test_input_session.cpp:1091-1119): where 己 lands after one pick in each mode, and whether any state was written.
    #[test]
    fn each_mode_moves_the_pick_to_its_slot() {
        for (mode, step, expected) in [
            (FrequencyAdjustmentMode::Disabled, 1, 5),
            (FrequencyAdjustmentMode::Pin, 1, 0),
            (FrequencyAdjustmentMode::Halve, 1, 2),
            (FrequencyAdjustmentMode::Linear, 2, 3),
            (FrequencyAdjustmentMode::Promote, 1, 4),
        ] {
            let dir = frequency_fixture();
            let changed = pick(&dir, "己", mode, step, 1).unwrap();
            assert_eq!(
                changed,
                mode != FrequencyAdjustmentMode::Disabled,
                "{mode:?}"
            );
            assert_eq!(index_of(&dir, "己"), expected, "{mode:?}");
            let journal = dir.journal();
            let state = count(&journal, "SELECT count(*) FROM user_dictionary_operations")
                + count(&journal, "SELECT count(*) FROM candidate_selection_state");
            assert_eq!(
                state > 0,
                mode != FrequencyAdjustmentMode::Disabled,
                "{mode:?}"
            );
            // The counter is cleared once the weight moved.
            assert_eq!(
                count(&journal, "SELECT count(*) FROM candidate_selection_state"),
                0
            );
        }
        // Halve splits 乙 90 and 丙 80.
        let dir = frequency_fixture();
        pick(&dir, "己", FrequencyAdjustmentMode::Halve, 1, 1).unwrap();
        assert_eq!(dir.weight("ni", "己"), Some(85));
        assert_eq!(
            query_i64(&dir.journal(), "SELECT weight FROM user_dictionary_operations WHERE dictionary='pinyin' AND key='ni' AND value='己' AND user_inserted=0"),
            Some(85)
        );
    }

    /// F2 (test_input_session.cpp:1122-1139).
    #[test]
    fn the_trigger_count_delays_the_move() {
        let dir = frequency_fixture();
        assert!(!pick(&dir, "己", FrequencyAdjustmentMode::Halve, 1, 2).unwrap());
        assert_eq!(index_of(&dir, "己"), 5);
        assert_eq!(
            count(
                &dir.journal(),
                "SELECT selection_count FROM candidate_selection_state"
            ),
            1
        );
        assert!(pick(&dir, "己", FrequencyAdjustmentMode::Halve, 1, 2).unwrap());
        assert_eq!(index_of(&dir, "己"), 2);
        // Out-of-range triggers are clamped to 1..=10.
        let dir = frequency_fixture();
        assert!(pick(&dir, "己", FrequencyAdjustmentMode::Halve, 1, 0).unwrap());
    }

    /// F3 (test_input_session.cpp:1141-1151).
    #[test]
    fn picking_the_leader_writes_no_state() {
        let dir = frequency_fixture();
        assert!(!pick(&dir, "甲", FrequencyAdjustmentMode::Promote, 1, 1).unwrap());
        let journal = dir.journal();
        assert_eq!(
            count(&journal, "SELECT count(*) FROM user_dictionary_operations"),
            0
        );
        assert_eq!(
            count(&journal, "SELECT count(*) FROM candidate_selection_state"),
            0
        );
    }

    /// F4 (test_input_session.cpp:1153-1172): another key's rows in the window are never demoted; the pick is lifted alone.
    #[test]
    fn mixed_keys_promote_the_pick_alone() {
        let dir = Dir::new();
        dir.pinyin(&[
            ("na", "甲", 5_000_000),
            ("ne", "乙", 5_000_000),
            ("ni", "丙", 3_000_000),
        ]);
        let ordered = vec![
            item("na", "甲", 5_000_000),
            item("ne", "乙", 5_000_000),
            item("ni", "丙", 3_000_000),
        ];
        let (main_db, user_db) = (dir.main_db(), dir.journal());
        assert!(adjust_candidate_ranking(&RankingRequest {
            main_db: &main_db,
            user_db: &user_db,
            context_key: "n",
            ordered: &ordered,
            entry_key: "ni",
            value: "丙",
            mode: FrequencyAdjustmentMode::Promote,
            linear_step: 1,
            trigger_count: 1,
            force_top: false,
            kind: PersonalDictionaryKind::Pinyin,
        })
        .unwrap());
        assert_eq!(dir.weight("ni", "丙"), Some(5_001_000));
        assert_eq!(dir.weight("na", "甲"), Some(5_000_000));
        assert_eq!(dir.weight("ne", "乙"), Some(5_000_000));
        assert_eq!(
            count(&user_db, "SELECT count(*) FROM user_dictionary_operations"),
            1
        );
    }

    #[test]
    fn a_fixed_candidate_is_not_ranked() {
        let dir = frequency_fixture();
        super::super::positions::set_fixed_position(&dir.journal(), "ni", "ni", "己", 2).unwrap();
        assert!(pick(&dir, "己", FrequencyAdjustmentMode::Promote, 1, 1).is_err());
        // Before the disabled check, as in the reference.
        assert!(pick(&dir, "己", FrequencyAdjustmentMode::Disabled, 1, 1).is_err());
        assert_eq!(dir.weight("ni", "己"), Some(50));
    }

    /// F7 (test_input_session.cpp:1204-1217).
    #[test]
    fn a_journal_that_is_a_directory_fails() {
        let dir = frequency_fixture();
        std::fs::create_dir(dir.journal()).unwrap();
        assert!(pick(&dir, "己", FrequencyAdjustmentMode::Promote, 1, 1).is_err());
        assert_eq!(dir.weight("ni", "己"), Some(50));
    }

    /// F8 (test_input_session.cpp:1219-1245): a journal that refuses the upsert leaves the dictionary untouched.
    #[test]
    fn a_refused_journal_write_rolls_the_dictionary_back() {
        let dir = frequency_fixture();
        super::super::journal::ensure_user_database(&dir.journal()).unwrap();
        Sqlite::open(dir.journal())
            .unwrap()
            .execute_batch("CREATE TRIGGER refuse BEFORE INSERT ON user_dictionary_operations BEGIN SELECT RAISE(ABORT, 'refused'); END;")
            .unwrap();
        assert!(pick(&dir, "己", FrequencyAdjustmentMode::Promote, 1, 1).is_err());
        assert_eq!(dir.weight("ni", "己"), Some(50));
        assert_eq!(
            count(
                &dir.journal(),
                "SELECT count(*) FROM candidate_selection_state"
            ),
            0
        );
    }

    #[test]
    fn a_pick_missing_from_the_list_fails() {
        let dir = frequency_fixture();
        assert!(pick(&dir, "庚", FrequencyAdjustmentMode::Promote, 1, 1).is_err());
        assert_eq!(
            count(
                &dir.journal(),
                "SELECT count(*) FROM candidate_selection_state"
            ),
            0
        );
    }

    #[test]
    fn wubi_writes_to_wubi86_and_requires_the_row() {
        let dir = Dir::new();
        dir.wubi(&[("aaaa", "工", 100), ("aaaa", "或", 50)]);
        let mut ordered = vec![item("aaaa", "工", 100), item("aaaa", "或", 50)];
        for row in &mut ordered {
            row.canonical_pinyin.clear();
        }
        let (main_db, user_db) = (dir.main_db(), dir.journal());
        let request = RankingRequest {
            main_db: &main_db,
            user_db: &user_db,
            context_key: "aaaa",
            ordered: &ordered,
            entry_key: "aaaa",
            value: "或",
            mode: FrequencyAdjustmentMode::Pin,
            linear_step: 1,
            trigger_count: 1,
            force_top: true,
            kind: PersonalDictionaryKind::Wubi,
        };
        assert!(adjust_candidate_ranking(&request).unwrap());
        assert_eq!(
            query_i64(&main_db, "SELECT weight FROM wubi86 WHERE value='或'"),
            Some(600)
        );
        assert_eq!(
            count(&user_db, "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='wubi' AND key='aaaa' AND value='或' AND weight=600"),
            1
        );
        Sqlite::open(&main_db)
            .unwrap()
            .execute_batch("DELETE FROM wubi86 WHERE value='或'")
            .unwrap();
        assert!(adjust_candidate_ranking(&request).is_err());
    }

    #[test]
    fn wubi98_writes_to_wubi98_under_its_own_kind() {
        let dir = Dir::new();
        dir.wubi(&[("aaaa", "工", 100)]);
        let main_db = dir.main_db();
        Sqlite::open(&main_db)
            .unwrap()
            .execute_batch(
                "CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER);\
                 INSERT INTO wubi98 VALUES('aaaa','工',100);\
                 INSERT INTO wubi98 VALUES('aaaa','式',50);",
            )
            .unwrap();
        let mut ordered = vec![item("aaaa", "工", 100), item("aaaa", "式", 50)];
        for row in &mut ordered {
            row.canonical_pinyin.clear();
        }
        let user_db = dir.journal();
        let request = RankingRequest {
            main_db: &main_db,
            user_db: &user_db,
            context_key: "aaaa",
            ordered: &ordered,
            entry_key: "aaaa",
            value: "式",
            mode: FrequencyAdjustmentMode::Pin,
            linear_step: 1,
            trigger_count: 1,
            force_top: true,
            kind: PersonalDictionaryKind::Wubi98,
        };
        assert!(adjust_candidate_ranking(&request).unwrap());
        assert_eq!(
            query_i64(&main_db, "SELECT weight FROM wubi98 WHERE value='式'"),
            Some(600)
        );
        assert_eq!(
            query_i64(&main_db, "SELECT weight FROM wubi86 WHERE value='工'"),
            Some(100)
        );
        assert_eq!(
            count(&user_db, "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='wubi98' AND key='aaaa' AND value='式' AND weight=600"),
            1
        );
        assert_eq!(
            count(
                &user_db,
                "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='wubi'"
            ),
            0
        );
    }

    fn plan(weights: &[i64], owned: &[bool], target: usize) -> WeightPlan {
        plan_weights(weights, |index| owned[index], target)
    }

    #[test]
    fn weight_arithmetic_covers_every_branch() {
        let all = [true; 20];
        // Midpoint, and the leader's slot one gap above it.
        assert_eq!(plan(&[100, 90, 50], &all, 1).selected, 95);
        assert_eq!(plan(&[100, 90, 50], &all, 0).selected, 600);
        // No integer midpoint: one above `upper`, skipping a run of equal weights above it.
        assert_eq!(plan(&[11, 10, 5], &all, 1).selected, 12);
        assert_eq!(plan(&[12, 12, 11, 10], &all, 2).selected, 13);
        assert_eq!(plan(&[13, 12, 11], &all, 2).selected, 14);
        // Equal weights leave no room: a staircase over the key's own rows.
        let stairs = plan(&[5000, 5000, 5000, 5000, 5000], &all, 3);
        assert_eq!(stairs.staircase, vec![(3, 4000), (4, 3000)]);
        assert_eq!(stairs.selected, 4500);
        // Too low for a staircase: the selection alone.
        let alone = plan(&[1, 1, 1], &all, 1);
        assert!(alone.staircase.is_empty());
        assert_eq!(alone.selected, 1001);
        // A foreign row in the window: the selection alone.
        let foreign = plan(&[5_000_000, 5_000_000, 3_000_000], &[true, false, true], 1);
        assert!(foreign.staircase.is_empty());
        assert_eq!(foreign.selected, 5_001_000);
        // Oversized weights from the unclamped compatibility path compact under the ceiling.
        let oversized = plan(&[200_000_000, 150_000_000, 10], &all, 1);
        assert_eq!(oversized.staircase, vec![(1, 99_999_000), (2, 99_998_000)]);
        assert_eq!(oversized.selected, 99_999_500);
        // At the top of a cluster near the ceiling the pick takes the ceiling.
        let ceiling = plan(&[99_999_990, 99_999_990], &all, 0);
        assert_eq!(
            clamp_managed_weight(ceiling.selected),
            MANAGED_WEIGHT_CEILING
        );
        // Near i64::MAX nothing overflows.
        let extreme = plan(&[i64::MAX, i64::MAX - 5], &all, 0);
        assert_eq!(
            clamp_managed_weight(extreme.selected),
            MANAGED_WEIGHT_CEILING
        );
    }

    #[test]
    fn staircase_reserves_the_rebalance_window_capacity() {
        let weights = [100_000; REBALANCE_COUNT + 1];
        let owns_entry_key = [true; REBALANCE_COUNT + 1];
        let target = 3;
        let plan = plan(&weights, &owns_entry_key, target);

        assert_eq!(
            plan.staircase.capacity(),
            weights.len() - target,
            "the staircase should reserve its maximum number of entries"
        );
    }

    #[test]
    fn a_staircase_journals_every_row_it_moves() {
        let dir = Dir::new();
        let rows: Vec<(&str, &str, i64)> = ["甲", "乙", "丙", "丁", "戊"]
            .iter()
            .map(|word| ("ni", *word, 5000))
            .collect();
        dir.pinyin(&rows);
        assert!(pick(&dir, "戊", FrequencyAdjustmentMode::Promote, 1, 1).unwrap());
        assert_eq!(dir.weight("ni", "丁"), Some(4000));
        assert_eq!(dir.weight("ni", "戊"), Some(4500));
        assert_eq!(index_of(&dir, "戊"), 3);
        assert_eq!(
            count(
                &dir.journal(),
                "SELECT count(*) FROM user_dictionary_operations"
            ),
            2
        );
    }

    #[test]
    fn dictionary_keys_follow_the_context() {
        let mut row = item("ni", "你好", 1);
        assert_eq!(candidate_dictionary_key(&row, "ni'hao'a"), "ni");
        row.canonical_pinyin.clear();
        assert_eq!(candidate_dictionary_key(&row, "ni'hao'a"), "ni'hao");
        assert_eq!(candidate_dictionary_key(&row, "nihao"), "ni");
        assert_eq!(candidate_dictionary_key(&row, "ni''hao"), "ni");
        row.word = "你好啊呀".to_owned();
        assert_eq!(candidate_dictionary_key(&row, "ni'hao"), "ni'hao");
    }

    fn english_item(word: &str, display: &str, weight: i64) -> WordItem {
        WordItem::new(
            word,
            display,
            weight,
            CandidateSource::EnglishDictionary,
            "",
        )
    }

    #[test]
    fn english_picks_go_above_the_list_and_journal_first() {
        let dir = Dir::new();
        dir.english(&[
            ("hello", "Hello", 1000),
            ("help", "Help", 100),
            ("helm", "Helm", 50),
        ]);
        let ordered = vec![
            english_item("hello", "Hello", 1000),
            english_item("help", "Help", 100),
            english_item("helm", "Helm", 50),
        ];
        let (english_db, user_db) = (dir.english_db(), dir.journal());
        let request = |value: &'static str, entry: &'static str, mode| RankingRequest {
            main_db: &english_db,
            user_db: &user_db,
            context_key: "english:he",
            ordered: &ordered,
            entry_key: entry,
            value,
            mode,
            linear_step: 1,
            trigger_count: 1,
            force_top: false,
            kind: PersonalDictionaryKind::English,
        };
        adjust_english_candidate_ranking(&request(
            "Help",
            "help",
            FrequencyAdjustmentMode::Promote,
        ))
        .unwrap();
        assert_eq!(
            query_i64(
                &english_db,
                "SELECT weight FROM english_words WHERE word='help'"
            ),
            Some(2000)
        );
        assert_eq!(
            count(&user_db, "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='english' AND key='help' AND value='Help' AND display='Help' AND weight=2000"),
            1
        );
        // Below the top the pick goes one above the row now in its target slot's place above it.
        adjust_english_candidate_ranking(&request(
            "Helm",
            "helm",
            FrequencyAdjustmentMode::Promote,
        ))
        .unwrap();
        assert_eq!(
            query_i64(
                &english_db,
                "SELECT weight FROM english_words WHERE word='helm'"
            ),
            Some(1001)
        );
        // The leader and the disabled mode write nothing.
        adjust_english_candidate_ranking(&request(
            "Hello",
            "hello",
            FrequencyAdjustmentMode::Promote,
        ))
        .unwrap();
        adjust_english_candidate_ranking(&request(
            "Helm",
            "helm",
            FrequencyAdjustmentMode::Disabled,
        ))
        .unwrap();
        assert_eq!(
            count(&user_db, "SELECT count(*) FROM user_dictionary_operations"),
            2
        );
        // A row the dictionary lacks is never journaled.
        Sqlite::open(&english_db)
            .unwrap()
            .execute_batch("DELETE FROM english_words WHERE word='helm'")
            .unwrap();
        assert!(adjust_english_candidate_ranking(&request(
            "Helm",
            "helm",
            FrequencyAdjustmentMode::Promote
        ))
        .is_err());
        assert!(adjust_english_candidate_ranking(&request(
            "Helm",
            "hel",
            FrequencyAdjustmentMode::Promote
        ))
        .is_err());
        assert_eq!(
            count(&user_db, "SELECT count(*) FROM user_dictionary_operations"),
            2
        );
    }

    /// The comparison set is ranked by weight, not display order: 西鄂 (xi'e, weight 6) shown in a protected slot above 写 must not be the basis for 写's new weight.
    #[test]
    fn the_comparison_set_is_ranked_by_weight_not_display_order() {
        let dir = Dir::new();
        dir.pinyin(&[
            ("xie", "些", 3_752_167),
            ("xie", "写", 605_147),
            ("xi'e", "西鄂", 6),
        ]);
        let ordered = vec![
            item("xie", "些", 3_752_167),
            item("xi'e", "西鄂", 6),
            item("xie", "写", 605_147),
        ];
        let (main_db, user_db) = (dir.main_db(), dir.journal());
        assert!(adjust_candidate_ranking(&RankingRequest {
            main_db: &main_db,
            user_db: &user_db,
            context_key: "xie",
            ordered: &ordered,
            entry_key: "xie",
            value: "写",
            mode: FrequencyAdjustmentMode::Promote,
            linear_step: 1,
            trigger_count: 1,
            force_top: false,
            kind: PersonalDictionaryKind::Pinyin,
        })
        .unwrap());
        // Weight order makes 写 rank 1, so Promote lifts it above 些. Display order would make it rank 2 and bracket it between 西鄂 and 些 instead.
        let written = dir.weight("xie", "写").unwrap();
        assert!(written > 3_752_167, "写 was written as {written}");
        assert_eq!(dir.weight("xie", "些"), Some(3_752_167));
        assert_eq!(dir.weight("xi'e", "西鄂"), Some(6));
    }

    /// test_english_input_session.cpp:203-240: while another connection holds the journal's write lock the pick fails before msime-english.db is opened, so the dictionary never carries a weight the journal lacks.
    #[test]
    fn a_locked_journal_leaves_english_db_untouched() {
        let dir = Dir::new();
        dir.english(&[("ninja", "Ninja", 200), ("nimbus", "Nimbus", 100)]);
        let (english_db, user_db) = (dir.english_db(), dir.journal());
        super::super::journal::ensure_user_database(&user_db).unwrap();
        let ordered = vec![
            english_item("ninja", "Ninja", 200),
            english_item("nimbus", "Nimbus", 100),
        ];
        let request = RankingRequest {
            main_db: &english_db,
            user_db: &user_db,
            context_key: "english:ni",
            ordered: &ordered,
            entry_key: "nimbus",
            value: "Nimbus",
            mode: FrequencyAdjustmentMode::Promote,
            linear_step: 1,
            trigger_count: 1,
            force_top: false,
            kind: PersonalDictionaryKind::English,
        };
        let blocker = Sqlite::open(&user_db).unwrap();
        blocker.execute_batch("BEGIN IMMEDIATE").unwrap();
        // Waits out the journal's busy timeout.
        assert!(adjust_english_candidate_ranking(&request).is_err());
        assert_eq!(
            query_i64(
                &english_db,
                "SELECT weight FROM english_words WHERE word='nimbus'"
            ),
            Some(100)
        );
        blocker.execute_batch("ROLLBACK").unwrap();
        drop(blocker);
        assert_eq!(
            count(&user_db, "SELECT count(*) FROM user_dictionary_operations"),
            0
        );

        adjust_english_candidate_ranking(&request).unwrap();
        assert_eq!(
            query_i64(
                &english_db,
                "SELECT weight FROM english_words WHERE word='nimbus'"
            ),
            Some(1200)
        );
        assert_eq!(
            count(&user_db, "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='english' AND key='nimbus' AND value='Nimbus' AND weight=1200"),
            1
        );
    }
}
