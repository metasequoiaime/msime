//! `@` mode: the names and places of the host's mention list, filtered by the letters after `@`. The list is the user's own and stays on the device; the engine neither reads contacts nor asks any service where a place is.
//!
//! With the places switch on, the Chinese administrative divisions of the embedded table in [`super::places`] (a static WTFPL dataset, modood/Administrative-divisions-of-China, divisions as of 2025-12-27) follow the user's rows once at least one letter is typed. They share `RESULT_LIMIT` with the user's rows and never displace them, and a place the user already listed appears once, as the user's row.

use super::command::TEXT_UTF16_LIMIT;
use super::places::{places, Places};
use crate::types::{CandidateSource, MentionEntry, WordItem};
use std::collections::HashSet;

/// Two pages of the nine-row Windows candidate window; typing more of a key narrows the list.
pub const RESULT_LIMIT: usize = 18;
/// Entries kept from a host list, as many as one dictionary import takes; the rest are ignored.
pub const LIST_LIMIT: usize = 1000;
pub const KEY_LIMIT: usize = 64;

fn collect_place_matches(
    table: &Places,
    code: &str,
    matched_names: &mut HashSet<String>,
    limit: usize,
) -> Vec<(&'static str, &'static str)> {
    let mut exact = Vec::with_capacity(limit);
    let mut prefix = Vec::with_capacity(limit);
    for (place, spellings) in table.places.iter().zip(&table.spellings) {
        let is_exact = spelled(spellings, code, true);
        if !is_exact && !spelled(spellings, code, false) {
            continue;
        }
        if !matched_names.insert(place.name.to_owned()) {
            continue;
        }
        if is_exact {
            if exact.len() < limit {
                exact.push((place.key, place.name));
            }
        } else if prefix.len() < limit {
            prefix.push((place.key, place.name));
        }
    }
    exact.extend(prefix.into_iter().take(limit.saturating_sub(exact.len())));
    exact
}

/// The entries that are usable of a host list: non-empty text within the candidate text bound, a key of lowercase letters and single apostrophes between them, the first entry of a text, at most `LIST_LIMIT`.
pub fn usable_mentions(entries: &[MentionEntry]) -> Vec<MentionEntry> {
    let mut usable: Vec<MentionEntry> = Vec::with_capacity(LIST_LIMIT.min(entries.len()));
    let mut texts = HashSet::with_capacity(LIST_LIMIT.min(entries.len()));
    for entry in entries {
        if usable.len() == LIST_LIMIT {
            break;
        }
        let text_valid =
            !entry.text.trim().is_empty() && entry.text.encode_utf16().count() <= TEXT_UTF16_LIMIT;
        let key_valid = entry.key.len() <= KEY_LIMIT
            && entry
                .key
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'\'')
            && (entry.key.is_empty() || entry.key.split('\'').all(|syllable| !syllable.is_empty()));
        if text_valid && key_valid && texts.insert(entry.text.as_str()) {
            usable.push(entry.clone());
        }
    }
    usable
}

/// Whether a spelling (key letters, key initials or lowercase text) answers the input exactly or as a prefix, for the pass that wants `exact`.
fn spelled(spellings: &[String; 2], code: &str, exact: bool) -> bool {
    spellings.iter().any(|spelling| {
        !spelling.is_empty() && spelling.starts_with(code) && (spelling == code) == exact
    })
}

/// Generated rows for the letters after `@`, weight `count - index`, at most `RESULT_LIMIT`: entries the input spells out completely, then entries it begins, each group in list order. A key matches by its letters (`zhangsan`) or its initials (`zs`); an entry without a key matches by its own text in lowercase. `pinyin` holds the key. `entries` must have gone through `usable_mentions`.
///
/// With `with_places` and a non-empty `code`, the embedded places fill the rows the list leaves, matched the same way and in table order, skipping a place whose name the list already offers.
pub fn query_mentions(code: &str, entries: &[MentionEntry], with_places: bool) -> Vec<WordItem> {
    let spellings = |entry: &MentionEntry| -> [String; 2] {
        if entry.key.is_empty() {
            return [entry.text.to_ascii_lowercase(), String::new()];
        }
        [
            entry.key.replace('\'', ""),
            entry
                .key
                .split('\'')
                .filter_map(|syllable| syllable.chars().next())
                .collect(),
        ]
    };
    let row_limit = RESULT_LIMIT.min(entries.len());
    let mut rows: Vec<&MentionEntry> = Vec::with_capacity(row_limit);
    let mut prefix_rows: Vec<&MentionEntry> = Vec::with_capacity(row_limit);
    for entry in entries {
        let entry_spellings = spellings(entry);
        if spelled(&entry_spellings, code, true) {
            rows.push(entry);
            if rows.len() == RESULT_LIMIT {
                break;
            }
        } else if prefix_rows.len() < RESULT_LIMIT && spelled(&entry_spellings, code, false) {
            prefix_rows.push(entry);
        }
    }
    rows.extend(prefix_rows.into_iter().take(RESULT_LIMIT - rows.len()));
    let mut matches: Vec<(&str, &str)> = rows
        .iter()
        .map(|entry| (entry.key.as_str(), entry.text.as_str()))
        .collect();
    if with_places && !code.is_empty() {
        let mut matched_names: HashSet<String> =
            matches.iter().map(|(_, text)| (*text).to_owned()).collect();
        let table = places();
        matches.extend(collect_place_matches(
            table,
            code,
            &mut matched_names,
            RESULT_LIMIT.saturating_sub(matches.len()),
        ));
    }
    let count = matches.len();
    matches
        .into_iter()
        .enumerate()
        .map(|(index, (key, text))| {
            WordItem::new(
                key,
                text,
                (count - index) as i64,
                CandidateSource::Generated,
                "",
            )
        })
        .collect()
}

#[cfg(test)]
mod place_match_tests {
    use super::*;

    #[test]
    fn place_matches_keep_exact_rows_before_prefix_rows() {
        let table = places();
        let mut matched_names = HashSet::new();
        let matches = collect_place_matches(table, "bei", &mut matched_names, RESULT_LIMIT);
        assert!(!matches.is_empty());
        assert!(matches.iter().all(|(_, name)| !name.is_empty()));
    }
}

/// The annotation of an `@` row: a place's parent division, empty for the user's own entries (which may name a place too) and for a province.
pub fn mention_annotation(text: &str, entries: &[MentionEntry]) -> &'static str {
    if entries.iter().any(|entry| entry.text == text) {
        return "";
    }
    places().parent(text).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mention(text: &str, key: &str) -> MentionEntry {
        MentionEntry {
            text: text.to_owned(),
            key: key.to_owned(),
        }
    }

    fn list() -> Vec<MentionEntry> {
        usable_mentions(&[
            mention("张三", "zhang'san"),
            mention("张珊珊", "zhang'shan'shan"),
            mention("深圳市", "shen'zhen'shi"),
            mention("Alice", ""),
        ])
    }

    #[test]
    fn usable_mentions_reserves_the_input_bound() {
        let entries = [
            mention("甲", "jia"),
            mention("乙", "yi"),
            mention("丙", "bing"),
        ];
        assert_eq!(usable_mentions(&entries).capacity(), entries.len());
    }

    fn words(code: &str) -> Vec<String> {
        query_mentions(code, &list(), false)
            .into_iter()
            .map(|row| row.word)
            .collect()
    }

    #[test]
    fn a_bare_at_lists_the_whole_list_in_order() {
        assert_eq!(words(""), ["张三", "张珊珊", "深圳市", "Alice"]);
        let rows = query_mentions("", &list(), false);
        assert_eq!(rows[0].pinyin, "zhang'san");
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row.source, CandidateSource::Generated);
            assert_eq!(row.weight, (rows.len() - index) as i64);
        }
    }

    #[test]
    fn keys_match_by_letters_and_by_initials() {
        assert_eq!(words("zhang"), ["张三", "张珊珊"]);
        assert_eq!(words("zs"), ["张三", "张珊珊"]);
        assert_eq!(words("zss"), ["张珊珊"]);
        assert_eq!(words("szs"), ["深圳市"]);
        assert_eq!(words("shenzhen"), ["深圳市"]);
        assert_eq!(words("al"), ["Alice"]);
        assert!(words("x").is_empty());
    }

    #[test]
    fn complete_spellings_come_first() {
        let entries =
            usable_mentions(&[mention("张珊", "zhang'shan"), mention("张三", "zhang'san")]);
        let rows: Vec<String> = query_mentions("zs", &entries, false)
            .into_iter()
            .map(|row| row.word)
            .collect();
        assert_eq!(rows, ["张珊", "张三"]);
        let rows: Vec<String> = query_mentions("zhangsan", &entries, false)
            .into_iter()
            .map(|row| row.word)
            .collect();
        assert_eq!(rows, ["张三"]);
    }

    #[test]
    fn a_row_matching_both_passes_appears_once_before_prefix_rows() {
        let entries = usable_mentions(&[
            mention("前缀", "zhang'shan'shan"),
            mention("双重", "z'san"),
            mention("完整", "z's"),
        ]);
        let rows: Vec<String> = query_mentions("zs", &entries, false)
            .into_iter()
            .map(|row| row.word)
            .collect();
        assert_eq!(rows, ["双重", "完整", "前缀"]);
    }

    fn with_places(code: &str, entries: &[MentionEntry]) -> Vec<String> {
        query_mentions(code, entries, true)
            .into_iter()
            .map(|row| row.word)
            .collect()
    }

    #[test]
    fn places_follow_the_list_only_when_switched_on() {
        assert_eq!(words("chongqing"), Vec::<String>::new());
        assert_eq!(with_places("chongqing", &list()), ["重庆市"]);
        assert_eq!(with_places("cqs", &list()), ["重庆市"]);
        let rows = query_mentions("chongqing", &list(), true);
        assert_eq!(rows[0].pinyin, "chong'qing'shi");
        assert_eq!(rows[0].source, CandidateSource::Generated);
        assert!(with_places("", &list()).len() == list().len());
    }

    #[test]
    fn the_list_comes_before_places_and_a_listed_place_appears_once() {
        let rows = with_places("shenzhen", &list());
        assert_eq!(rows, ["深圳市"]);
        let rows = with_places("zh", &list());
        assert_eq!(&rows[..2], ["张三", "张珊珊"]);
        assert_eq!(rows.len(), RESULT_LIMIT);
        let weights: Vec<i64> = query_mentions("zh", &list(), true)
            .into_iter()
            .map(|row| row.weight)
            .collect();
        assert_eq!(weights.first(), Some(&(RESULT_LIMIT as i64)));
        assert_eq!(weights.last(), Some(&1));
        let full: Vec<MentionEntry> = (0..RESULT_LIMIT + 2)
            .map(|index| mention(&format!("张{index}"), "zhang"))
            .collect();
        let rows = with_places("zh", &usable_mentions(&full));
        assert_eq!(rows.len(), RESULT_LIMIT);
        assert!(rows.iter().all(|row| row.starts_with('张')));
    }

    #[test]
    fn larger_divisions_and_complete_spellings_lead_the_places() {
        let rows = with_places("bj", &[]);
        assert_eq!(rows.first().map(String::as_str), Some("北京市"));
        let rows = with_places("luanshi", &[]);
        assert_eq!(rows.first().map(String::as_str), Some("六安市"));
    }

    #[test]
    fn a_place_is_annotated_with_its_parent() {
        assert_eq!(mention_annotation("深圳市", &[]), "广东省");
        assert_eq!(mention_annotation("深圳市", &list()), "");
        assert_eq!(mention_annotation("广东省", &[]), "");
        assert_eq!(mention_annotation("张三", &list()), "");
    }

    #[test]
    fn unusable_entries_are_dropped() {
        let long = "名".repeat(TEXT_UTF16_LIMIT + 1);
        let usable = usable_mentions(&[
            mention("", "a"),
            mention("  ", "a"),
            mention(&long, "a"),
            mention("大写", "Da"),
            mention("数字", "a1"),
            mention("空音节", "a''b"),
            mention("尾撇", "a'"),
            mention("李四", "li'si"),
            mention("李四", "lisi"),
        ]);
        assert_eq!(usable, [mention("李四", "li'si")]);
        let many: Vec<MentionEntry> = (0..LIST_LIMIT + 5)
            .map(|index| mention(&index.to_string(), ""))
            .collect();
        assert_eq!(usable_mentions(&many).len(), LIST_LIMIT);
    }
}
