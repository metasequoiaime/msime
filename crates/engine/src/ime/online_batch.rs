//! `replace_online_candidate_batch` (core-session.md §7.2, quanpin.md §13, overlays.md §7.2).

use std::collections::HashSet;

use crate::types::{CandidateSource, WordItem};

pub const AI_QUOTA: usize = 10;
pub const CLOUD_QUOTA: usize = 1;
pub const MAX_ONLINE_WORD_BYTES: usize = 4_096;
const EXISTING_WORD_LINEAR_SCAN_LIMIT: usize = 64;

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
    if words.iter().any(|word| !is_acceptable_online_word(word)) {
        return false;
    }
    unique_online_words(words, quota).is_some()
}

/// 校验、去重、限额，去掉同一来源之前的行后插入，已在列表里的词跳过。云候选插在第 1 位；AI 批次按回答的顺序接在列表末尾，到达时不挪动任何已有的行（#6888）。第一条 AI 候选显示在哪里由 input-runtime 按宿主的每页候选数决定（`normalize_online_slots`，第一页最后一格），引擎这一层不知道一页有几格，所以只把它们放在最后，混排英文、emoji 和固定位置时也就不会因为 AI 行在前面而错开。返回 false 时 `list` 不变。
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
    if words.iter().any(|word| !is_acceptable_online_word(word)) {
        return false;
    }
    // 在线候选的协议配额最多为 10 个，固定数组可避免每批次的临时堆分配。
    let Some((mut unique, unique_count)) = unique_online_words(words, quota) else {
        return false;
    };
    // Providers repeat candidates, so the quota counts distinct words: a repeat must neither reject a valid batch nor use up a seat (online_candidate_batch.h:32-33).

    let pending_index = (source == CandidateSource::CloudSuggestion).then(|| {
        list.iter()
            .position(|item| item.source == CandidateSource::CloudPending)
    });
    if source == CandidateSource::AiSuggestion {
        // 新的 AI 批次替换掉之前的 AI 行；旧版本留下的占位行（来源 12）也一并去掉。
        list.retain(|item| !item.source.holds_ai_seat());
    } else {
        list.retain(|item| {
            item.source != source
                && !(source == CandidateSource::CloudSuggestion
                    && item.source == CandidateSource::CloudPending)
        });
    }
    let new_count = mark_existing_online_words(list, &mut unique, unique_count);
    list.reserve(new_count);
    let words = unique.into_iter().flatten();
    if source == CandidateSource::AiSuggestion {
        list.extend(words.map(|word| WordItem::new(key, word, 1, source, "")));
        return true;
    }
    // 回包里的词全部已在本地列表时仍保留云席位，避免“回答到达但没有可见云词”再次挤动本地候选。
    if source == CandidateSource::CloudSuggestion
        && new_count == 0
        && pending_index.flatten().is_some()
    {
        let index = pending_index
            .flatten()
            .expect("pending cloud seat was found")
            .min(list.len());
        list.insert(
            index,
            WordItem::new(key, "☁", 0, CandidateSource::CloudPending, ""),
        );
        return true;
    }
    // 第 1 位；列表里只有 AI 行时排在它们前面，AI 行始终在最后。
    let index = pending_index.flatten().map_or_else(
        || {
            list.iter()
                .position(|item| item.source.holds_ai_seat())
                .unwrap_or(list.len())
                .min(1)
        },
        |index| index.min(list.len()),
    );
    for (offset, word) in words.enumerate() {
        list.insert(index + offset, WordItem::new(key, word, 1, source, ""));
    }
    true
}

/// 已有缓存页直接更新；成功后收紧外层存储，避免历史大批次的富余槽长期留存。
pub(crate) fn replace_cached_online_candidate_batch(
    list: &mut Vec<WordItem>,
    key: &str,
    words: &[String],
    source: CandidateSource,
) -> bool {
    let changed = replace_online_candidate_batch(list, key, words, source);
    if changed {
        list.shrink_to_fit();
    }
    changed
}

fn unique_online_words(
    words: &[String],
    quota: usize,
) -> Option<([Option<&str>; AI_QUOTA], usize)> {
    let mut unique = [None; AI_QUOTA];
    let mut count = 0;
    for word in words {
        if unique[..count]
            .iter()
            .flatten()
            .any(|candidate| *candidate == word.as_str())
        {
            continue;
        }
        if count == quota {
            return None;
        }
        unique[count] = Some(word.as_str());
        count += 1;
    }
    Some((unique, count))
}

fn mark_existing_online_words(
    list: &[WordItem],
    unique: &mut [Option<&str>; AI_QUOTA],
    unique_count: usize,
) -> usize {
    let mut new_count = 0;
    if list.len() <= EXISTING_WORD_LINEAR_SCAN_LIMIT {
        for word in unique.iter_mut().take(unique_count) {
            if list
                .iter()
                .any(|item| item.word == *word.as_ref().expect("unique online word is present"))
            {
                *word = None;
            } else {
                new_count += 1;
            }
        }
        return new_count;
    }
    let existing: HashSet<&str> = list.iter().map(|item| item.word.as_str()).collect();
    for word in unique.iter_mut().take(unique_count) {
        if existing.contains(word.as_ref().expect("unique online word is present")) {
            *word = None;
        } else {
            new_count += 1;
        }
    }
    new_count
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
    fn unique_online_words_keep_order_without_heap_tracking() {
        let input = words(&["甲", "乙", "甲", "丙"]);
        let (unique, count) = unique_online_words(&input, 3).unwrap();
        assert_eq!(count, 3);
        assert_eq!(unique[..count], [Some("甲"), Some("乙"), Some("丙")]);
        assert!(unique_online_words(&words(&["甲", "乙", "丙", "丁"]), 3).is_none());
    }

    #[test]
    fn existing_words_use_a_zero_allocation_scan_for_short_lists() {
        let list = dictionary_list();
        let mut unique = [
            Some("你"),
            Some("智"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ];
        let ((new_count, unique), allocations) =
            crate::ime::personal_rerank::allocations::count(|| {
                (mark_existing_online_words(&list, &mut unique, 2), unique)
            });
        assert_eq!(new_count, 1);
        assert_eq!(allocations, 0);
        assert_eq!(unique[..2], [None, Some("智")]);

        let large_list: Vec<WordItem> = (0..=EXISTING_WORD_LINEAR_SCAN_LIMIT)
            .map(|index| row(&format!("已有{index}"), CandidateSource::Database))
            .collect();
        let mut unique = [
            Some("已有0"),
            Some("新"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ];
        assert_eq!(mark_existing_online_words(&large_list, &mut unique, 2), 1);
        assert_eq!(unique[..2], [None, Some("新")]);
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
    fn cloud_takes_slot_one_and_ai_rows_go_last() {
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
        // AI 行按回答的顺序接在末尾，第一页最后一格由 input-runtime 安排（#6888）。
        assert_eq!(
            shape(&list),
            vec![
                ("你", CandidateSource::Database),
                ("云", CandidateSource::CloudSuggestion),
                ("尼", CandidateSource::Database),
                ("泥", CandidateSource::Database),
                ("智一", CandidateSource::AiSuggestion),
                ("智二", CandidateSource::AiSuggestion),
            ]
        );
        let inserted = &list[1];
        assert_eq!(inserted.pinyin, "ni");
        assert_eq!(inserted.canonical_pinyin, "");
        assert_eq!(inserted.weight, 1);
    }

    #[test]
    fn a_later_cloud_row_leaves_the_ai_rows_last() {
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
                ("尼", CandidateSource::Database),
                ("泥", CandidateSource::Database),
                ("智一", CandidateSource::AiSuggestion),
                ("智二", CandidateSource::AiSuggestion),
            ]
        );
    }

    #[test]
    fn an_ai_batch_moves_no_existing_row() {
        for batch in [vec!["智一"], vec!["智一", "智二", "智三"]] {
            let original = dictionary_list();
            let mut list = original.clone();
            assert!(replace_online_candidate_batch(
                &mut list,
                "ni",
                &words(&batch),
                CandidateSource::AiSuggestion
            ));
            assert_eq!(list[..original.len()], original[..], "{batch:?}");
            let tail: Vec<&str> = list[original.len()..]
                .iter()
                .map(|item| item.word.as_str())
                .collect();
            assert_eq!(tail, batch);
        }
    }

    /// 旧版本在 AI 座位上放的占位行（来源 12）不再产生；万一列表里还有，下一批 AI 回答连同它一起替换。
    #[test]
    fn an_ai_batch_drops_a_leftover_placeholder() {
        let mut list = dictionary_list();
        list.insert(2, row("…", CandidateSource::AiPending));
        assert!(replace_online_candidate_batch(
            &mut list,
            "ni",
            &words(&["智"]),
            CandidateSource::AiSuggestion
        ));
        assert_eq!(
            shape(&list),
            vec![
                ("你", CandidateSource::Database),
                ("尼", CandidateSource::Database),
                ("泥", CandidateSource::Database),
                ("智", CandidateSource::AiSuggestion),
            ]
        );
    }

    #[test]
    fn an_empty_answer_leaves_the_list_alone() {
        let original = dictionary_list();
        let mut list = original.clone();
        assert!(!replace_online_candidate_batch(
            &mut list,
            "ni",
            &[],
            CandidateSource::AiSuggestion
        ));
        assert_eq!(list, original);
    }

    #[test]
    fn cloud_updates_reuse_row_storage_with_and_without_ai() {
        for with_ai in [false, true] {
            let mut list = Vec::with_capacity(32);
            list.extend(dictionary_list());
            if with_ai {
                assert!(replace_online_candidate_batch(
                    &mut list,
                    "ni",
                    &words(&["智一", "智二"]),
                    CandidateSource::AiSuggestion,
                ));
            }
            let pointer = list.as_ptr();
            let capacity = list.capacity();
            let batch = words(&["云"]);

            let (accepted, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                replace_online_candidate_batch(
                    &mut list,
                    "ni",
                    &batch,
                    CandidateSource::CloudSuggestion,
                )
            });

            assert!(accepted);
            // 词面和键各一个字符串加一个 `SharedString` 的 `Arc`。
            assert_eq!(allocations, 4, "云候选只需分配词面和键：{with_ai}");
            assert_eq!(list.as_ptr(), pointer);
            assert_eq!(list.capacity(), capacity);
            assert_eq!(list[1].word, "云");
            if with_ai {
                assert_eq!(list[list.len() - 2].word, "智一");
                assert_eq!(list.last().unwrap().word, "智二");
            }
        }
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
                ("泥", CandidateSource::Database),
                ("智", CandidateSource::AiSuggestion),
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
        // 列表里只有 AI 行时云候选排在它们前面，AI 行仍在最后。
        assert!(replace_online_candidate_batch(
            &mut empty,
            "ni",
            &words(&["云"]),
            CandidateSource::CloudSuggestion
        ));
        assert_eq!(
            shape(&empty),
            vec![
                ("云", CandidateSource::CloudSuggestion),
                ("智", CandidateSource::AiSuggestion),
            ]
        );

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
