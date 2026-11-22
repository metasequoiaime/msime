//! 云词库换代中的本机搬迁；不经有损备份格式，也不改变云端用户词。

use std::collections::BTreeSet;

use rusqlite::{params, TransactionBehavior};

use super::{stream_learning_sync_state, LearningSyncRecord};
use crate::error::Result;
use crate::paths::RuntimePaths;
use crate::types::PersonalDictionaryKind;
use crate::user_dictionary::{personal, pin_state, position_state, state};

/// 独占维护权内、发布新代次之前调用。源端先完整读取，读取失败不改新代次。
///
/// 调权与删除保留真实时间、显示值和用户来源；同身份已有云端记录时保留云端。
/// 固定候选保留完整身份，含槽位输家；同身份保留新代次，槽位显示按同步投影规则决定。
/// 置顶也保留被删词隐藏的比较状态；选词计数仍走现有备份合并。
pub fn carry_learning_sync_state(
    active: &RuntimePaths,
    staged: &RuntimePaths,
    main_dictionary: bool,
) -> Result<usize> {
    active.validate()?;
    staged.validate()?;
    let floor = crate::user_dictionary::learning_floor::read_path(
        &active.user(crate::assets::USER_JOURNAL),
    )?;
    let mut positions = Vec::new();
    let mut pins = Vec::new();
    stream_learning_sync_state(active, &mut |record| {
        if matches!(record, LearningSyncRecord::Position { .. }) {
            positions.push(record.clone());
        } else if matches!(record, LearningSyncRecord::Pin { .. }) {
            pins.push(record.clone());
        }
        true
    })?;
    let operations = state::carry_dictionary_rows_filtered(
        active,
        staged,
        main_dictionary,
        &[
            PersonalDictionaryKind::Pinyin,
            PersonalDictionaryKind::Wubi,
            PersonalDictionaryKind::Wubi98,
            PersonalDictionaryKind::QuickPhrase,
            PersonalDictionaryKind::English,
        ],
        true,
    )?;
    if positions.is_empty() && pins.is_empty() && floor == 0 {
        return Ok(operations);
    }
    let mut connection = personal::open_edit_connection(staged, main_dictionary)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    crate::user_dictionary::learning_floor::advance_in(&transaction, "personal_journal", floor)?;
    position_state::seed_display(&transaction, "personal_journal", None)?;
    pin_state::seed_display(&transaction, "personal_journal")?;
    let mut contexts = BTreeSet::new();
    let mut inserted = 0;
    for record in positions {
        if let LearningSyncRecord::Position {
            context,
            key,
            value,
            position,
            updated_at,
        } = record
        {
            inserted += transaction.execute(
                "INSERT OR IGNORE INTO personal_journal.fixed_candidate_position_state \
                 (context_key,entry_key,value,position,updated_at) VALUES(?1,?2,?3,?4,?5)",
                params![context, key, value, position, updated_at as i64],
            )?;
            contexts.insert(context);
        }
    }
    for context in contexts {
        position_state::project_in(&transaction, "personal_journal", &context)?;
    }
    for record in pins {
        if let LearningSyncRecord::Pin {
            context,
            value,
            updated_at,
        } = record
        {
            inserted += transaction.execute(
                "INSERT OR IGNORE INTO personal_journal.pinned_candidate_state(context_key,value,updated_at) VALUES(?1,?2,?3)",
                params![context,value,updated_at as i64],
            )?;
        }
    }
    pin_state::project(&transaction, "personal_journal")?;
    transaction.commit()?;
    Ok(operations + inserted)
}

#[cfg(test)]
#[path = "carry_tests.rs"]
mod tests;
