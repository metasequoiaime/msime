//! 粤语的选词学习（#10544）：调频和造词，都只写用户日志里 `dictionary='cantonese'` 的行，`cantonese::UserWords` 读回来并进候选。开关沿用拼音的：「学习」（`SessionOptions::learning`）管两样，调频方式（`FrequencyAdjustmentOptions`）另管调频，规则与拼音相同（`session/learning.rs`）。
//!
//! 调频写的行 `user_inserted=0`，不进「我的词条」；造出的词 `user_inserted=1`，和拼音造的词一样列在「我的词条」里，可以改、可以删。两种行都不进云同步、社区词库和词库快照：那几条路按词库种类挑行，本来就不收 `cantonese`。选词计数记在 `candidate_selection_state`，上下文以 [`SELECTION_CONTEXT_PREFIX`] 开头，导出快照时按这个前缀跳过。

use std::path::Path;

use rusqlite::{params, Transaction, TransactionBehavior};

use crate::error::{EngineError, Result};
use crate::types::{
    FrequencyAdjustmentMode, FrequencyAdjustmentOptions, PersonalDictionaryEntry,
    PersonalDictionaryKind,
};
use crate::user_dictionary::journal::{
    is_user_deleted, open_journal, record_user_insert, write_upsert, MISSING_ROW,
};
use crate::user_dictionary::personal::validate_personal_dictionary_entry;
use crate::user_dictionary::ranking::{
    clamp_managed_weight, ranking_target, COUNT_SELECTION_SQL, RESET_SELECTION_SQL,
};

/// 粤语选词计数在 `candidate_selection_state` 里的上下文前缀，后面接选中那一行的粤拼键。拼音的上下文是带 `'` 的切分，不会以它开头。词库快照（云同步与本地备份共用的格式）不收粤语，导出时按它跳过这些计数。
pub const SELECTION_CONTEXT_PREFIX: &str = "cantonese:";

/// 造出的词的权重，与拼音造词相同（`dictionary::pinyin::INSERTED_WEIGHT`）。`msime-cantonese.db` 里多字词的权重大多在几万以下，新词落在同一读音的常用词后面、首页之内，之后再选它由调频往前挪；手动加的词另用 `PersonalDictionaryEntry::DEFAULT_WEIGHT`，要一加上就排在前面。
pub const LEARNED_WORD_WEIGHT: i64 = crate::dictionary::pinyin::INSERTED_WEIGHT;

/// 造词最多几个音节，与拼音记整句的上限相同（`session::learning::MAX_LEARNED_SENTENCE_SYLLABLES`）：分几次选完一长串时不把整句记成词。
pub const MAX_LEARNED_WORD_SYLLABLES: usize = 7;

/// 调频的一行：候选文字和它现在的权重。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedRow {
    pub text: String,
    pub weight: i64,
}

/// 选中 `key` 下的 `value` 一次。`rows` 是候选里与它覆盖同一段输入的那几行（含它自己），按权重比名次，与拼音调频按权重而不按显示顺序比的道理相同（`adjust_candidate_ranking`）。计数够了 `trigger_count` 次、它又不是权重最重的那一行时，按调频方式算出目标名次，把它的权重写成目标名次那一行的权重加一。返回是否写了权重。调频方式为「关闭」时什么也不写。
pub fn adjust_frequency(
    journal: &Path,
    rows: &[RankedRow],
    key: &str,
    value: &str,
    options: FrequencyAdjustmentOptions,
) -> Result<bool> {
    if options.mode == FrequencyAdjustmentMode::Disabled {
        return Ok(false);
    }
    if key.is_empty() || value.is_empty() {
        return Err(EngineError::invalid(MISSING_ROW));
    }
    let mut weights: Vec<&RankedRow> = rows.iter().collect();
    weights.sort_by_key(|row| std::cmp::Reverse(row.weight));
    let rank = weights
        .iter()
        .position(|row| row.text == value)
        .ok_or_else(|| EngineError::failed(MISSING_ROW))?;
    let context = selection_context(key);
    let connection = open_journal(journal)?;
    let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
    let trigger_count = i64::from(options.trigger_count.clamp(1, 10));
    let count: i64 = transaction
        .prepare_cached(COUNT_SELECTION_SQL)?
        .query_row(params![context, key, value], |row| row.get(0))?;
    if count < trigger_count {
        transaction.commit()?;
        return Ok(false);
    }
    transaction
        .prepare_cached(RESET_SELECTION_SQL)?
        .execute(params![context, key, value])?;
    if rank == 0 {
        transaction.commit()?;
        return Ok(false);
    }
    let target = ranking_target(rank, options.mode, options.linear_step, false);
    let weight = clamp_managed_weight(weights[target].weight.saturating_add(1));
    write_upsert(
        &transaction,
        PersonalDictionaryKind::Cantonese,
        key,
        value,
        weight,
        "",
    )?;
    transaction.commit()?;
    Ok(true)
}

/// 把分几次选出来的一个词记进用户日志（`user_inserted=1`，权重 [`LEARNED_WORD_WEIGHT`]）。调用方已经确认粤语词库（发布的和用户的）在 `key` 下没有它。写法过不了个人词库的校验（音节数与字数不等）、超过 [`MAX_LEARNED_WORD_SYLLABLES`] 个音节、或用户删过这个词时什么也不写，返回 `Ok(false)`：删掉的词不该由同一个习惯再造回来，同拼音的选词对（`learn_pick_pair`）。
pub fn create_word(journal: &Path, key: &str, value: &str) -> Result<bool> {
    let syllables = key.split(' ').count();
    if !(2..=MAX_LEARNED_WORD_SYLLABLES).contains(&syllables) {
        return Ok(false);
    }
    let Ok(entry) = validate_personal_dictionary_entry(&PersonalDictionaryEntry {
        kind: PersonalDictionaryKind::Cantonese,
        key: key.to_owned(),
        value: value.to_owned(),
        weight: LEARNED_WORD_WEIGHT,
    }) else {
        return Ok(false);
    };
    if is_user_deleted(journal, entry.kind, &entry.key, &entry.value) {
        return Ok(false);
    }
    record_user_insert(
        journal,
        entry.kind,
        &entry.key,
        &entry.value,
        entry.weight,
        "",
    )?;
    Ok(true)
}

fn selection_context(key: &str) -> String {
    let mut context = String::with_capacity(SELECTION_CONTEXT_PREFIX.len() + key.len());
    context.push_str(SELECTION_CONTEXT_PREFIX);
    context.push_str(key);
    context
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_dictionary::journal::test_support::{count, query_i64, Dir};

    fn rows(rows: &[(&str, i64)]) -> Vec<RankedRow> {
        rows.iter()
            .map(|(text, weight)| RankedRow {
                text: (*text).to_owned(),
                weight: *weight,
            })
            .collect()
    }

    fn options(mode: FrequencyAdjustmentMode, trigger_count: i32) -> FrequencyAdjustmentOptions {
        FrequencyAdjustmentOptions {
            mode,
            trigger_count,
            linear_step: 1,
        }
    }

    fn learned_weight(dir: &Dir, key: &str, value: &str) -> Option<i64> {
        query_i64(
            &dir.journal(),
            &format!(
                "SELECT weight FROM user_dictionary_operations WHERE dictionary='cantonese' AND key='{key}' AND value='{value}' AND operation='upsert' AND user_inserted=0"
            ),
        )
    }

    #[test]
    fn a_pick_lands_one_above_the_row_at_its_target() {
        let dir = Dir::new();
        let group = rows(&[("你好", 900), ("妳好", 40), ("你號", 10)]);
        let promote = options(FrequencyAdjustmentMode::Promote, 1);
        assert!(adjust_frequency(&dir.journal(), &group, "nei hou", "你號", promote).unwrap());
        assert_eq!(learned_weight(&dir, "nei hou", "你號"), Some(41));
        let pin = options(FrequencyAdjustmentMode::Pin, 1);
        assert!(adjust_frequency(&dir.journal(), &group, "nei hou", "妳好", pin).unwrap());
        assert_eq!(learned_weight(&dir, "nei hou", "妳好"), Some(901));
        // 计数用过就清掉，粤语的计数带前缀。
        assert_eq!(
            count(
                &dir.journal(),
                "SELECT count(*) FROM candidate_selection_state"
            ),
            0
        );
    }

    #[test]
    fn the_trigger_count_and_the_heaviest_row_write_no_weight() {
        let dir = Dir::new();
        let group = rows(&[("你好", 900), ("妳好", 40)]);
        let twice = options(FrequencyAdjustmentMode::Pin, 2);
        assert!(!adjust_frequency(&dir.journal(), &group, "nei hou", "妳好", twice).unwrap());
        assert_eq!(
            count(
                &dir.journal(),
                "SELECT count(*) FROM candidate_selection_state WHERE context_key='cantonese:nei hou' AND selection_count=1"
            ),
            1
        );
        assert!(adjust_frequency(&dir.journal(), &group, "nei hou", "妳好", twice).unwrap());
        assert_eq!(learned_weight(&dir, "nei hou", "妳好"), Some(901));
        let once = options(FrequencyAdjustmentMode::Pin, 1);
        assert!(!adjust_frequency(&dir.journal(), &group, "nei hou", "你好", once).unwrap());
        assert_eq!(learned_weight(&dir, "nei hou", "你好"), None);
        let disabled = options(FrequencyAdjustmentMode::Disabled, 1);
        assert!(!adjust_frequency(&dir.journal(), &group, "nei hou", "妳好", disabled).unwrap());
    }

    #[test]
    fn a_user_word_keeps_its_flag_when_ranked() {
        let dir = Dir::new();
        record_user_insert(
            &dir.journal(),
            PersonalDictionaryKind::Cantonese,
            "nei hou",
            "妳好",
            40,
            "",
        )
        .unwrap();
        let group = rows(&[("你好", 900), ("妳好", 40)]);
        let pin = options(FrequencyAdjustmentMode::Pin, 1);
        assert!(adjust_frequency(&dir.journal(), &group, "nei hou", "妳好", pin).unwrap());
        assert_eq!(
            query_i64(
                &dir.journal(),
                "SELECT weight FROM user_dictionary_operations WHERE key='nei hou' AND value='妳好' AND user_inserted=1"
            ),
            Some(901)
        );
    }

    #[test]
    fn words_are_created_only_when_they_can_be_stored() {
        let dir = Dir::new();
        assert!(create_word(&dir.journal(), "nei hou", "你號").unwrap());
        assert_eq!(
            query_i64(
                &dir.journal(),
                "SELECT weight FROM user_dictionary_operations WHERE dictionary='cantonese' AND key='nei hou' AND value='你號' AND user_inserted=1"
            ),
            Some(LEARNED_WORD_WEIGHT)
        );
        // 单个音节、超过上限、音节数与字数不等的都不记。
        assert!(!create_word(&dir.journal(), "nei", "你").unwrap());
        assert!(!create_word(&dir.journal(), "a a a a a a a a", "啊啊啊啊啊啊啊啊").unwrap());
        assert!(!create_word(&dir.journal(), "nei hou", "你好呀").unwrap());
        // 用户删过的词不再造回来。
        let connection = open_journal(&dir.journal()).unwrap();
        connection
            .execute(
                "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,user_inserted) VALUES('cantonese','hou nei','好你','delete',0,1)",
                [],
            )
            .unwrap();
        drop(connection);
        assert!(!create_word(&dir.journal(), "hou nei", "好你").unwrap());
        assert_eq!(
            count(
                &dir.journal(),
                "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='cantonese' AND operation='upsert'"
            ),
            1
        );
    }
}
