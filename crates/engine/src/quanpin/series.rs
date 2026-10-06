//! Correction resolution, series keys, alternative-segmentation merging and autocorrect marking (quanpin.md §4.2-§4.3, §7.5, §7.7, §11.1-§11.2). Pure functions over rows; the dictionary drives them.

use std::collections::HashSet;
use std::fmt::Write;

use crate::pinyin::autocorrect::{
    autocorrect_cut_kbest, looks_like_syllable_with_jianpin_tail, AutocorrectCut,
};
use crate::pinyin::segment::join_segments;
use crate::pinyin::syllables::{has_only_complete_pinyin_segments, is_intact, is_prefix};
use crate::types::{autocorrect_type, WordItem};

/// How many ranked correction cuts feed the query pipeline: the primary cut becomes the query key and the rest ride along as alternative segmentations. Dropped-initial keys such as `uan` have 10-15 equally weighted deletion targets (cuan/duan/guan/...), and k = 3 cut the list by table order before reaching the right reading (quan'li for uanli); k = 9 lifted R@1/R@3 on the deletion and mixed eval sets with no p95 change (QD:90-97).
pub const AUTOCORRECT_CUT_KBEST: usize = 9;
pub const ALTERNATIVE_SEGMENTATION_CANDIDATE_LIMIT: usize = 128;
/// The protected slot only lifts a reading that cannot be seen at all. Anything already on the first page keeps the rank its weight earned, because frequency learning writes weight and pinning such a word again would overwrite the rank the user just typed for. 6 is the default page size and deliberately does not follow the page-size setting (QD:29-34, overlays.md §1.1).
pub const ALTERNATIVE_SEGMENTATION_FIRST_PAGE_SIZE: usize = 6;
/// Where the promoted best alternative lands.
pub const BEST_ALTERNATIVE_SEGMENTATION_INDEX: usize = 1;
/// An alternative is promoted only when its best weight is at least 1% of the primary reading's top weight; rarer re-segmentations are noise.
pub const ALTERNATIVE_SEGMENTATION_PROMOTION_RATIO: i64 = 100;
pub const MAX_SYLLABLES_FOR_MULTIPLE_SEGMENTATIONS: usize = 4;
/// Continuations are only taken up to three syllables longer: past that the rows' weights have dropped to a few dozen and the seats are better left to prefix characters (QD:23-25).
pub const LONGER_PHRASE_EXTRA_SYLLABLES: usize = 3;
pub const LONGER_PHRASE_LIMIT: usize = 12;

/// How one raw input is read (QD:110-215).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SeriesResolution {
    pub corrected_input: bool,
    pub corrected: Vec<String>,
    pub alternative_corrected_cuts: Vec<Vec<String>>,
    pub costlier_corrected_cuts: Vec<Vec<String>>,
    pub segmentation: String,
    pub cache_key: String,
}

/// QD:110-215, before memoisation.
pub fn resolve_series_query(
    raw: &str,
    segments: &[String],
    autocorrect_types: u32,
) -> SeriesResolution {
    let mut result = SeriesResolution::default();
    // Guard order matters: the jianpin-shape predicate runs before the search so a guarded input never pays for it. Both guards say "the user did not mistype". The base segmentation is deliberately not part of the gate: the search works on the raw letters, and an empty base cut (a dropped initial such as uanli for quan'li) is often exactly the input that needs correcting.
    let eligible = autocorrect_types != 0
        && !raw.contains('\'')
        && !has_only_complete_pinyin_segments(segments)
        && !looks_like_syllable_with_jianpin_tail(raw);
    if eligible {
        let cuts = autocorrect_cut_kbest(raw, autocorrect_types, AUTOCORRECT_CUT_KBEST);
        if !cuts.is_empty() {
            result.corrected_input = true;
            populate_from_cuts(&mut result, cuts, "");
        } else {
            // The k-best search only reaches the end when every segment is a complete syllable, so "hauzh" (hau typo + zh jianpin) fails outright. Retry on the head without a trailing incomplete syllable, shortest tail first so the correction explains as much of the input as possible. Neighbor corrections are left out of the head: they have the widest false-positive surface, and stacking them on a speculative jianpin boundary turns deletion-shaped input into noise (shng -> sun + g -> 笋干).
            let head_types = autocorrect_types & !autocorrect_type::NEIGHBOR;
            let mut tail_len = 1;
            while head_types != 0 && tail_len <= 2 && tail_len < raw.len() {
                let (head, tail) = raw.split_at(raw.len() - tail_len);
                tail_len += 1;
                if !is_prefix(tail) || is_intact(tail) {
                    continue;
                }
                let head_cuts = autocorrect_cut_kbest(head, head_types, AUTOCORRECT_CUT_KBEST);
                if head_cuts.is_empty() {
                    continue;
                }
                result.corrected_input = true;
                populate_from_cuts(&mut result, head_cuts, tail);
                break;
            }
        }
    }
    // Both branches rebuild the segmentation from segments: they already carry the alias-normalised spelling, while a caller's explicit string could keep the alias spelling, and then nothing would look rewritten and "nue'hao" could never be marked. Delimiters sit on syllable boundaries, so split and join round-trip them.
    result.segmentation = if result.corrected_input {
        join_segments(&result.corrected)
    } else if segments.is_empty() {
        raw.to_string()
    } else {
        join_segments(segments)
    };
    result.cache_key = series_cache_key_with_prefix(
        raw,
        &result.segmentation,
        autocorrect_types,
        result.corrected_input,
    );
    result
}

/// The primary cut defines the cost tier; same-cost readings compete with it on frequency, costlier ones stay behind it.
fn populate_from_cuts(
    result: &mut SeriesResolution,
    cuts: Vec<AutocorrectCut>,
    jianpin_tail: &str,
) {
    let to_segments = |cut: AutocorrectCut| {
        let mut segments = cut.into_syllables();
        if !jianpin_tail.is_empty() {
            segments.push(jianpin_tail.to_string());
        }
        segments
    };
    let mut cuts = cuts.into_iter();
    let primary = cuts.next().expect("autocorrect cuts are non-empty");
    for cut in cuts {
        if cut.same_cost_as(&primary) {
            result.alternative_corrected_cuts.push(to_segments(cut));
        } else {
            result.costlier_corrected_cuts.push(to_segments(cut));
        }
    }
    result.corrected = to_segments(primary);
}

/// `"T<types>:" + ("M:" | "A:") + (segmentation or raw)` (QD:63-67). The mask is part of the key because correction alternatives and typo sentences depend on it, and one input can be asked with different masks in a session (a suppressed input clears it for that input only).
pub fn series_cache_key(raw: &str, segmentation: &str, autocorrect_types: u32) -> String {
    series_cache_key_with_prefix(raw, segmentation, autocorrect_types, false)
}

fn series_cache_key_with_prefix(
    raw: &str,
    segmentation: &str,
    autocorrect_types: u32,
    corrected: bool,
) -> String {
    let mode = if raw.contains('\'') { "M:" } else { "A:" };
    let reading = if segmentation.is_empty() {
        raw
    } else {
        segmentation
    };
    // Reserve the complete key once: `resolve_series_query` used to format around a second
    // already-allocated key when adding the correction marker.
    let type_digits = if autocorrect_types == 0 {
        1
    } else {
        autocorrect_types.ilog10() as usize + 1
    };
    let mut key =
        String::with_capacity(reading.len() + 4 + type_digits + usize::from(corrected) * 2);
    if corrected {
        key.push_str("C:");
    }
    write!(&mut key, "T{autocorrect_types}:{mode}{reading}")
        .expect("writing a series cache key to String cannot fail");
    key
}

/// Lowercase, drop `'`, keep `v` distinct from `u` (QD:75-88). The ü alias rewrite is a marking source by product decision: a typed `nue` whose primary segmentation is `nve` must compare as different.
pub fn fold_reading(text: &str) -> String {
    let capacity = text.bytes().filter(|&byte| byte != b'\'').count();
    let mut folded = String::with_capacity(capacity);
    for character in text.chars().filter(|&character| character != '\'') {
        folded.push(character.to_ascii_lowercase());
    }
    folded
}

fn folded_reading_equal(left: &str, right: &str) -> bool {
    left.chars()
        .filter(|&character| character != '\'')
        .map(|character| character.to_ascii_lowercase())
        .eq(right
            .chars()
            .filter(|&character| character != '\'')
            .map(|character| character.to_ascii_lowercase()))
}

/// QD:1006-1054: rows read from a corrected cut get `corrected_from = fold(raw)`.
pub fn mark_autocorrect_candidates(
    candidates: &mut [WordItem],
    raw: &str,
    primary_segmentation: &str,
    corrected_cuts: &[String],
) {
    // A row comes from a corrected reading exactly when its letters equal some correction cut's letters while those differ from the typed letters. Comparing letters alone would also sweep up prefix rows the user spelled correctly (keneng -> ke, single-letter jianpin expansions); both rules together keep those unmarked.
    let cuts =
        || std::iter::once(primary_segmentation).chain(corrected_cuts.iter().map(String::as_str));
    let raw_letters = fold_reading(raw);
    if cuts().all(|cut| cut.is_empty() || folded_reading_equal(cut, raw)) {
        return;
    }
    for item in candidates
        .iter_mut()
        .filter(|item| item.corrected_from.is_empty())
    {
        // A cut can fold back to exactly the typed letters; matching that set would label a row the user spelled correctly, so only the sets that differ count.
        if cuts().any(|cut| {
            !cut.is_empty()
                && !folded_reading_equal(cut, raw)
                && folded_reading_equal(cut, &item.pinyin)
        }) {
            item.corrected_from = raw_letters.clone();
        }
    }
}

/// The promotion and union rule of `alternative_segmentation_page` (QD:888-945): `primary_full` and `alternatives` are the full-key rows of the primary and alternative segmentations; `result` is the series answer so far.
pub fn merge_alternative_segmentations(
    result: Vec<WordItem>,
    primary_full: Vec<WordItem>,
    alternatives: Vec<WordItem>,
) -> Vec<WordItem> {
    let Some(best_alternative) = alternatives.first() else {
        return result;
    };
    let promote = best_alternative
        .weight
        .saturating_mul(ALTERNATIVE_SEGMENTATION_PROMOTION_RATIO)
        >= primary_full.first().map_or(0, |item| item.weight);
    let best_word = best_alternative.word.clone();

    let capacity = result
        .len()
        .saturating_add(primary_full.len())
        .saturating_add(alternatives.len());
    let mut merged = Vec::with_capacity(capacity);
    merged.extend(primary_full);
    merged.extend(alternatives);
    merged.sort_by_key(|item| std::cmp::Reverse(item.weight));
    retain_unique_sorted_rows(&mut merged);

    if promote {
        if let Some(at) = merged.iter().position(|item| item.word == best_word) {
            if at >= ALTERNATIVE_SEGMENTATION_FIRST_PAGE_SIZE {
                let promoted = merged.remove(at);
                merged.insert(BEST_ALTERNATIVE_SEGMENTATION_INDEX, promoted);
            }
        }
    }

    // All of `result`, not a suffix of it: the series answer carries whole-sentence rows inside its full-key group, so index arithmetic that assumes it starts with `primary_full` would drop them. The word dedup keeps the primary rows already merged from repeating.
    append_unique_words(&mut merged, result);
    merged
}

/// Deduplicate rows that are already sorted by weight while keeping the merged vector's allocation.
fn retain_unique_sorted_rows(rows: &mut Vec<WordItem>) {
    // Borrow words while calculating each first occurrence, then retain in place after releasing the set.
    let mut seen = HashSet::with_capacity(rows.len());
    let unique = rows
        .iter()
        .map(|item| seen.insert(item.word.as_str()))
        .collect::<Vec<_>>();
    drop(seen);
    let mut index = 0;
    rows.retain(|_| {
        let keep = unique[index];
        index += 1;
        keep
    });
}

/// Append the rows whose word is not already present (QD:993-1004). A row repeated inside `rows` is kept once, as the reference's scan over the growing list does.
pub fn append_unique_words(result: &mut Vec<WordItem>, rows: Vec<WordItem>) {
    // Borrow words while checking duplicates, then release the borrows before moving rows into the result.
    let mut seen = HashSet::with_capacity(result.len().saturating_add(rows.len()));
    seen.extend(result.iter().map(|item| item.word.as_str()));
    let unique = rows
        .iter()
        .map(|item| seen.insert(item.word.as_str()))
        .collect::<Vec<_>>();
    drop(seen);
    result.reserve(rows.len());
    result.extend(
        rows.into_iter()
            .zip(unique)
            .filter_map(|(item, unique)| unique.then_some(item)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CandidateSource;

    fn row(pinyin: &str, word: &str, weight: i64) -> WordItem {
        WordItem::new(pinyin, word, weight, CandidateSource::Database, pinyin)
    }

    fn words(items: &[WordItem]) -> Vec<&str> {
        items.iter().map(|item| item.word.as_str()).collect()
    }

    #[test]
    fn series_cache_key_carries_mask_and_delimiter_mode() {
        assert_eq!(series_cache_key("nihao", "ni'hao", 0), "T0:A:ni'hao");
        assert_eq!(series_cache_key("ni'hao", "ni'hao", 15), "T15:M:ni'hao");
        assert_eq!(series_cache_key("xyz", "", 3), "T3:A:xyz");
    }

    #[test]
    fn series_cache_key_can_add_correction_prefix_in_one_build() {
        assert_eq!(
            series_cache_key_with_prefix("gau", "gua", 3, true),
            "C:T3:A:gua"
        );
        assert_eq!(
            series_cache_key_with_prefix("gau", "gua", 3, false),
            "T3:A:gua"
        );
    }

    fn owned(segments: &[&str]) -> Vec<String> {
        segments.iter().map(|segment| segment.to_string()).collect()
    }

    #[test]
    fn resolution_splits_correction_cost_tiers() {
        let both = autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR;
        // gua (transposition, 10) is strictly cheaper than gai (neighbor, 13), so the two land in different tiers whatever the dictionary says.
        let gau = resolve_series_query("gau", &[], both);
        assert!(gau.corrected_input);
        assert_eq!(gau.corrected, owned(&["gua"]));
        assert!(gau.costlier_corrected_cuts.contains(&owned(&["gai"])));
        assert_eq!(gau.segmentation, "gua");
        assert_eq!(gau.cache_key, "C:T3:A:gua");
    }

    #[test]
    fn resolution_composes_a_corrected_head_with_a_jianpin_tail() {
        let both = autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR;
        let hauzh = resolve_series_query("hauzh", &[], both);
        assert_eq!(hauzh.corrected, owned(&["hua", "zh"]));
        assert_eq!(hauzh.cache_key, "C:T3:A:hua'zh");
    }

    #[test]
    fn resolution_leaves_legal_and_unmasked_input_alone() {
        let segments = owned(&["ke", "neng"]);
        let legal = resolve_series_query("keneng", &segments, 15);
        assert!(!legal.corrected_input);
        assert_eq!(legal.segmentation, "ke'neng");
        assert_eq!(legal.cache_key, "T15:A:ke'neng");

        let unmasked = resolve_series_query("sahng", &[], 0);
        assert!(!unmasked.corrected_input);
        assert_eq!(unmasked.segmentation, "sahng");
        assert_eq!(unmasked.cache_key, "T0:A:sahng");

        let manual = resolve_series_query("sa'hng", &owned(&["sa", "hng"]), 15);
        assert!(!manual.corrected_input, "manual delimiters never take part");
        assert_eq!(manual.cache_key, "T15:M:sa'hng");
    }

    #[test]
    fn fold_keeps_v_distinct_from_u() {
        let folded = fold_reading("Sa'Hng");
        assert_eq!(folded, "sahng");
        assert_eq!(folded.capacity(), folded.len());
        assert_eq!(fold_reading("nve"), "nve");
        assert_ne!(fold_reading("nve"), fold_reading("nue"));
    }

    #[test]
    fn folded_reading_comparison_matches_owned_folding() {
        for (left, right) in [("Sa'Hng", "sahng"), ("nve", "nue"), ("", "'")] {
            assert_eq!(
                folded_reading_equal(left, right),
                fold_reading(left) == fold_reading(right),
                "{left}/{right}"
            );
        }
    }

    #[test]
    fn marks_only_rows_whose_letters_equal_a_differing_cut() {
        let mut items = vec![
            row("shang", "上", 100),
            row("sa'h'n'g", "撒谎那个", 1000),
            row("sha", "沙", 10),
        ];
        mark_autocorrect_candidates(&mut items, "sahng", "shang", &[]);
        assert_eq!(items[0].corrected_from, "sahng");
        assert!(items[1].corrected_from.is_empty());
        assert!(items[2].corrected_from.is_empty());
    }

    #[test]
    fn marking_exits_when_every_cut_spells_the_typed_letters() {
        let mut items = vec![row("ke'neng", "可能", 100), row("ke", "可", 10)];
        mark_autocorrect_candidates(&mut items, "keneng", "ke'neng", &[]);
        assert!(items.iter().all(|item| item.corrected_from.is_empty()));
    }

    #[test]
    fn umlaut_alias_rewrite_is_marked() {
        // nue -> nve marks; a typed nve does not.
        let mut typed_nue = vec![row("nve", "虐", 100)];
        mark_autocorrect_candidates(&mut typed_nue, "nue", "nve", &[]);
        assert_eq!(typed_nue[0].corrected_from, "nue");
        let mut typed_nve = vec![row("nve", "虐", 100)];
        mark_autocorrect_candidates(&mut typed_nve, "nve", "nve", &[]);
        assert!(typed_nve[0].corrected_from.is_empty());
    }

    #[test]
    fn marking_uses_alternative_cuts_and_keeps_existing_labels() {
        let mut items = vec![row("zhan", "站", 100), row("shan", "山", 100)];
        items[1].corrected_from = "earlier".to_string();
        mark_autocorrect_candidates(&mut items, "ahan", "shan", &["zhan".to_string()]);
        assert_eq!(items[0].corrected_from, "ahan");
        assert_eq!(items[1].corrected_from, "earlier");
    }

    #[test]
    fn alternative_best_is_promoted_only_from_beyond_the_first_page() {
        // xian: 西安 sits at 16 on weight, beyond the first page, so it is lifted to index 1.
        let primary: Vec<WordItem> = (0..16)
            .map(|i| row("xian", &format!("先{i}"), 10_000 - i))
            .collect();
        let alternatives = vec![row("xi'an", "西安", 1_000)];
        let merged = merge_alternative_segmentations(Vec::new(), primary.clone(), alternatives);
        assert_eq!(merged[1].word, "西安");
        assert_eq!(merged.len(), 17);

        // tian: 天 田 提案, not 天 提案 田; a first-page alternative keeps its earned rank.
        let primary = vec![row("tian", "天", 5_000), row("tian", "田", 3_000)];
        let alternatives = vec![row("ti'an", "提案", 2_000)];
        let merged = merge_alternative_segmentations(Vec::new(), primary, alternatives);
        assert_eq!(words(&merged), ["天", "田", "提案"]);
    }

    #[test]
    fn alternative_dedup_keeps_sorted_storage() {
        let mut rows = vec![
            row("xian", "甲", 3),
            row("xi'an", "乙", 2),
            row("xian", "甲", 1),
        ];
        let pointer = rows.as_ptr();

        retain_unique_sorted_rows(&mut rows);

        assert_eq!(rows.as_ptr(), pointer);
        assert_eq!(words(&rows), ["甲", "乙"]);
    }

    #[test]
    fn rare_alternative_is_not_promoted() {
        let primary: Vec<WordItem> = (0..10)
            .map(|i| row("xie", &format!("写{i}"), 1_000_000 - i))
            .collect();
        let alternatives = vec![row("xi'e", "西鄂", 6)];
        let merged = merge_alternative_segmentations(Vec::new(), primary, alternatives);
        assert_eq!(merged.last().unwrap().word, "西鄂");
    }

    #[test]
    fn merge_appends_the_whole_series_answer_after_the_union() {
        let mut sentence = WordItem::new(
            "ni'hao",
            "泥豪",
            -5_000,
            CandidateSource::Generated,
            "ni'hao",
        );
        sentence.sentence_association = true;
        let result = vec![
            row("ni'hao", "你好", 10_000),
            sentence,
            row("ni", "你", 8_000),
        ];
        let merged = merge_alternative_segmentations(
            result,
            vec![row("ni'hao", "你好", 10_000)],
            vec![row("ni'ha'o", "拟哈哦", 20)],
        );
        assert_eq!(words(&merged), ["你好", "拟哈哦", "泥豪", "你"]);
        assert_eq!(
            merged[1].pinyin, "ni'ha'o",
            "an alternative row answers under its own key"
        );
    }

    #[test]
    fn merge_without_alternatives_returns_the_answer_unchanged() {
        let result = vec![row("a", "啊", 1)];
        assert_eq!(
            merge_alternative_segmentations(result.clone(), Vec::new(), Vec::new()),
            result
        );
    }

    #[test]
    fn append_unique_words_dedups_against_the_growing_list() {
        let mut result = vec![row("a", "啊", 1)];
        append_unique_words(
            &mut result,
            vec![row("a", "啊", 2), row("a", "阿", 3), row("a", "阿", 4)],
        );
        assert_eq!(words(&result), ["啊", "阿"]);
        assert_eq!(result[1].weight, 3);
    }

    #[test]
    fn append_unique_words_reserves_the_incoming_rows() {
        let mut result = Vec::with_capacity(1);
        result.push(row("a", "啊", 1));
        let rows: Vec<WordItem> = (0..10)
            .map(|index| row("a", &format!("词{index}"), index))
            .collect();

        append_unique_words(&mut result, rows);

        assert_eq!(result.len(), 11);
        assert_eq!(result.capacity(), 11);
    }
}
