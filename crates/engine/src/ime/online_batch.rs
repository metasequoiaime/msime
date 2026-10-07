//! `replace_online_candidate_batch` (core-session.md §7.2, quanpin.md §13, overlays.md §7.2).

use std::collections::HashSet;

use crate::types::{CandidateSource, WordItem};

pub const AI_QUOTA: usize = 10;
pub const CLOUD_QUOTA: usize = 1;
pub const MAX_ONLINE_WORD_BYTES: usize = 4_096;

/// Validate a provider batch without constructing rows that a caller may immediately discard.
pub(crate) fn validate_online_candidate_batch(words: &[String], source: CandidateSource) -> bool {
    let quota = match source {
        CandidateSource::AiSuggestion => AI_QUOTA,
        CandidateSource::CloudSuggestion => CLOUD_QUOTA,
        _ => return false,
    };
    if words.is_empty() {
        return false;
    }
    let mut seen = HashSet::with_capacity(words.len().min(quota));
    for word in words {
        if !is_acceptable_online_word(word) {
            return false;
        }
        if seen.len() == quota && !seen.contains(word.as_str()) {
            return false;
        }
        seen.insert(word.as_str());
    }
    true
}

/// Validate, deduplicate, enforce the quota, drop the source's previous rows and insert at 1 (cloud) or 2 (AI), skipping words already listed; after a cloud insert the AI rows move as a block to index 2. False leaves `list` untouched.
pub fn replace_online_candidate_batch(
    list: &mut Vec<WordItem>,
    key: &str,
    words: &[String],
    source: CandidateSource,
) -> bool {
    let quota = match source {
        CandidateSource::AiSuggestion => AI_QUOTA,
        CandidateSource::CloudSuggestion => CLOUD_QUOTA,
        _ => return false,
    };
    if words.is_empty() {
        return false;
    }
    let mut seen = HashSet::with_capacity(words.len().min(quota));
    // 在线候选的协议配额最多为 10 个，固定数组可避免每批次的临时堆分配。
    let mut unique: [Option<&str>; AI_QUOTA] = [None; AI_QUOTA];
    let mut unique_count = 0;
    for word in words {
        if !is_acceptable_online_word(word) {
            return false;
        }
        if seen.insert(word.as_str()) {
            if unique_count == quota {
                return false;
            }
            unique[unique_count] = Some(word.as_str());
            unique_count += 1;
        }
    }
    // Providers repeat candidates, so the quota counts distinct words: a repeat must neither reject a valid batch nor use up a seat (online_candidate_batch.h:32-33).

    list.retain(|item| item.source != source);
    let existing: HashSet<&str> = list.iter().map(|item| item.word.as_str()).collect();
    let mut new_count = 0;
    for word in unique.iter_mut().take(unique_count) {
        if existing.contains(word.as_ref().expect("unique online word is present")) {
            *word = None;
        } else {
            new_count += 1;
        }
    }
    drop(existing);
    list.reserve(new_count);
    let index = list.len().min(if source == CandidateSource::AiSuggestion {
        2
    } else {
        1
    });
    for (offset, word) in unique.into_iter().flatten().enumerate() {
        list.insert(index + offset, WordItem::new(key, word, 1, source, ""));
    }
    if source == CandidateSource::CloudSuggestion {
        let (ai, rest): (Vec<WordItem>, Vec<WordItem>) = std::mem::take(list)
            .into_iter()
            .partition(|item| item.source == CandidateSource::AiSuggestion);
        *list = rest;
        let at = list.len().min(2);
        list.splice(at..at, ai);
    }
    true
}

/// Non-empty, at most 4096 bytes, no C0 control byte and no DEL.
fn is_acceptable_online_word(word: &str) -> bool {
    !word.is_empty()
        && word.len() <= MAX_ONLINE_WORD_BYTES
        && !word.bytes().any(|byte| byte < 32 || byte == 127)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(word: &str, source: CandidateSource) -> WordItem {
        WordItem::new("ni", word, 100, source, "ni")
    }

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|word| word.to_string()).collect()
    }

    fn shape(list: &[WordItem]) -> Vec<(&str, CandidateSource)> {
        list.iter()
            .map(|item| (item.word.as_str(), item.source))
            .collect()
    }

    fn dictionary_list() -> Vec<WordItem> {
        vec![
            row("你", CandidateSource::Database),
            row("尼", CandidateSource::Database),
            row("泥", CandidateSource::Database),
        ]
    }

    #[test]
    fn validates_online_batches_without_constructing_candidate_rows() {
        assert!(validate_online_candidate_batch(
            &words(&["智一", "智二", "智一"]),
            CandidateSource::AiSuggestion
        ));
        assert!(!validate_online_candidate_batch(
            &words(&[
                "词0", "词1", "词2", "词3", "词4", "词5", "词6", "词7", "词8", "词9", "词10"
            ]),
            CandidateSource::AiSuggestion
        ));
        assert!(!validate_online_candidate_batch(
            &words(&["好", "坏\n"]),
            CandidateSource::AiSuggestion
        ));
        assert!(!validate_online_candidate_batch(
            &[],
            CandidateSource::CloudSuggestion
        ));
        assert!(!validate_online_candidate_batch(
            &words(&["好"]),
            CandidateSource::Database
        ));
    }

    #[test]
    fn cloud_takes_slot_one_and_ai_slot_two() {
        let mut list = dictionary_list();
        assert!(replace_online_candidate_batch(
            &mut list,
            "ni",
            &words(&["云"]),
            CandidateSource::CloudSuggestion
        ));
        assert!(replace_online_candidate_batch(
            &mut list,
            "ni",
            &words(&["智一", "智二"]),
            CandidateSource::AiSuggestion
        ));
        assert_eq!(
            shape(&list),
            vec![
                ("你", CandidateSource::Database),
                ("云", CandidateSource::CloudSuggestion),
                ("智一", CandidateSource::AiSuggestion),
                ("智二", CandidateSource::AiSuggestion),
                ("尼", CandidateSource::Database),
                ("泥", CandidateSource::Database),
            ]
        );
        let inserted = &list[1];
        assert_eq!(inserted.pinyin, "ni");
        assert_eq!(inserted.canonical_pinyin, "");
        assert_eq!(inserted.weight, 1);
    }

    #[test]
    fn a_later_cloud_row_rehomes_the_ai_block_to_slot_two() {
        let mut list = dictionary_list();
        assert!(replace_online_candidate_batch(
            &mut list,
            "ni",
            &words(&["智一", "智二"]),
            CandidateSource::AiSuggestion
        ));
        assert!(replace_online_candidate_batch(
            &mut list,
            "ni",
            &words(&["云"]),
            CandidateSource::CloudSuggestion
        ));
        assert_eq!(
            shape(&list),
            vec![
                ("你", CandidateSource::Database),
                ("云", CandidateSource::CloudSuggestion),
                ("智一", CandidateSource::AiSuggestion),
                ("智二", CandidateSource::AiSuggestion),
                ("尼", CandidateSource::Database),
                ("泥", CandidateSource::Database),
            ]
        );
    }

    #[test]
    fn a_new_batch_replaces_the_source_rows() {
        let mut list = dictionary_list();
        assert!(replace_online_candidate_batch(
            &mut list,
            "ni",
            &words(&["云一"]),
            CandidateSource::CloudSuggestion
        ));
        assert!(replace_online_candidate_batch(
            &mut list,
            "ni",
            &words(&["云二"]),
            CandidateSource::CloudSuggestion
        ));
        assert_eq!(
            list.iter()
                .filter(|item| item.source == CandidateSource::CloudSuggestion)
                .map(|item| item.word.as_str())
                .collect::<Vec<_>>(),
            vec!["云二"]
        );
        assert_eq!(list[1].word, "云二");
    }

    #[test]
    fn words_already_listed_are_skipped() {
        let mut list = dictionary_list();
        assert!(replace_online_candidate_batch(
            &mut list,
            "ni",
            &words(&["尼", "智"]),
            CandidateSource::AiSuggestion
        ));
        assert_eq!(
            shape(&list),
            vec![
                ("你", CandidateSource::Database),
                ("尼", CandidateSource::Database),
                ("智", CandidateSource::AiSuggestion),
                ("泥", CandidateSource::Database),
            ]
        );
    }

    #[test]
    fn duplicates_are_removed_before_the_quota() {
        // Engine #208: twelve AI words with two repeats are ten distinct words.
        let mut batch: Vec<String> = (0..10).map(|index| format!("词{index}")).collect();
        batch.push("词0".to_string());
        batch.push("词1".to_string());
        let mut list = dictionary_list();
        assert!(replace_online_candidate_batch(
            &mut list,
            "ni",
            &batch,
            CandidateSource::AiSuggestion
        ));
        let ai: Vec<&str> = list
            .iter()
            .filter(|item| item.source == CandidateSource::AiSuggestion)
            .map(|item| item.word.as_str())
            .collect();
        assert_eq!(ai.len(), 10);
        assert_eq!(ai[0], "词0");
        assert_eq!(ai[9], "词9");

        let mut cloud = dictionary_list();
        assert!(replace_online_candidate_batch(
            &mut cloud,
            "ni",
            &words(&["云", "云"]),
            CandidateSource::CloudSuggestion
        ));
    }

    #[test]
    fn over_quota_and_invalid_batches_leave_the_list_alone() {
        let original = dictionary_list();
        let eleven: Vec<String> = (0..11).map(|index| format!("词{index}")).collect();
        let cases: Vec<(Vec<String>, CandidateSource)> = vec![
            (eleven, CandidateSource::AiSuggestion),
            (words(&["云一", "云二"]), CandidateSource::CloudSuggestion),
            (Vec::new(), CandidateSource::AiSuggestion),
            (words(&["好", ""]), CandidateSource::AiSuggestion),
            (words(&["好", "坏\n"]), CandidateSource::AiSuggestion),
            (words(&["\u{7f}"]), CandidateSource::CloudSuggestion),
            (
                vec!["字".repeat(MAX_ONLINE_WORD_BYTES / 3 + 1)],
                CandidateSource::CloudSuggestion,
            ),
            (words(&["好"]), CandidateSource::Database),
            (words(&["好"]), CandidateSource::Generated),
        ];
        for (batch, source) in cases {
            let mut list = original.clone();
            assert!(
                !replace_online_candidate_batch(&mut list, "ni", &batch, source),
                "{batch:?} {source:?}"
            );
            assert_eq!(list, original);
        }

        let mut list = original.clone();
        assert!(replace_online_candidate_batch(
            &mut list,
            "ni",
            &["a".repeat(MAX_ONLINE_WORD_BYTES)],
            CandidateSource::CloudSuggestion
        ));
    }

    #[test]
    fn short_lists_clamp_the_slots() {
        let mut empty = Vec::new();
        assert!(replace_online_candidate_batch(
            &mut empty,
            "ni",
            &words(&["智"]),
            CandidateSource::AiSuggestion
        ));
        assert_eq!(shape(&empty), vec![("智", CandidateSource::AiSuggestion)]);

        let mut one = vec![row("你", CandidateSource::Database)];
        assert!(replace_online_candidate_batch(
            &mut one,
            "ni",
            &words(&["智一", "智二"]),
            CandidateSource::AiSuggestion
        ));
        assert!(replace_online_candidate_batch(
            &mut one,
            "ni",
            &words(&["云"]),
            CandidateSource::CloudSuggestion
        ));
        assert_eq!(
            shape(&one),
            vec![
                ("你", CandidateSource::Database),
                ("云", CandidateSource::CloudSuggestion),
                ("智一", CandidateSource::AiSuggestion),
                ("智二", CandidateSource::AiSuggestion),
            ]
        );
    }

    #[test]
    fn batch_insertion_reserves_new_rows() {
        let mut list = Vec::with_capacity(1);
        list.push(row("你", CandidateSource::Database));
        let batch: Vec<String> = (0..10).map(|index| format!("词{index}")).collect();

        assert!(replace_online_candidate_batch(
            &mut list,
            "ni",
            &batch,
            CandidateSource::AiSuggestion
        ));
        assert_eq!(list.len(), 11);
        assert_eq!(list.capacity(), 11);
    }
}
