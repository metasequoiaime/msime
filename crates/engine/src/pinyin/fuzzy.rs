//! Fuzzy pinyin expansion (quanpin.md §6, `R/quanpin/fuzzy_pinyin.h`). The dictionary side (`fuzzy_candidates`) is in `quanpin::dictionary`.

use super::syllables::is_intact;
use crate::types::{fuzzy_rule, FuzzyPinyinOptions};

pub const FUZZY_SEGMENTATION_LIMIT: usize = 64;

const INITIAL_PAIRS: [(&str, &str, u32); 6] = [
    ("z", "zh", fuzzy_rule::Z_ZH),
    ("c", "ch", fuzzy_rule::C_CH),
    ("s", "sh", fuzzy_rule::S_SH),
    ("n", "l", fuzzy_rule::N_L),
    ("f", "h", fuzzy_rule::F_H),
    ("r", "l", fuzzy_rule::R_L),
];

const FINAL_PAIRS: [(&str, &str, u32); 5] = [
    ("an", "ang", fuzzy_rule::AN_ANG),
    ("en", "eng", fuzzy_rule::EN_ENG),
    ("in", "ing", fuzzy_rule::IN_ING),
    ("ian", "iang", fuzzy_rule::IAN_IANG),
    ("uan", "uang", fuzzy_rule::UAN_UANG),
];

/// The partners of `part` under every enabled pair, `part` itself first.
fn with_partners<'a>(
    part: &'a str,
    pairs: &[(&'a str, &'a str, u32)],
    options: FuzzyPinyinOptions,
) -> Vec<&'a str> {
    let mut variants = vec![part];
    for &(a, b, rule) in pairs {
        if !options.enabled(rule) {
            continue;
        }
        if part == a {
            variants.push(b);
        } else if part == b {
            variants.push(a);
        }
    }
    variants
}

/// `[syllable]` then every intact initial/final variant the enabled rules allow, deduplicated (FZ:10-68). A non-intact syllable or no rules gives just `[syllable]`.
pub fn fuzzy_syllables(syllable: &str, options: FuzzyPinyinOptions) -> Vec<String> {
    // Only complete syllables expand, so an unfinished prefix keeps its normal completion.
    if options.rules == 0 || !is_intact(syllable) {
        return vec![syllable.to_owned()];
    }
    let initial_length = if ["zh", "ch", "sh"]
        .iter()
        .any(|initial| syllable.starts_with(initial))
    {
        2
    } else if syllable.starts_with(|c: char| "bpmfdtnlgkhjqxrzcsyw".contains(c)) {
        1
    } else {
        0
    };
    let (initial, final_part) = syllable.split_at(initial_length);
    let starts = with_partners(initial, &INITIAL_PAIRS, options);
    let ends = with_partners(final_part, &FINAL_PAIRS, options);
    let mut result = vec![syllable.to_owned()];
    for start in &starts {
        for end in &ends {
            let mut candidate = String::with_capacity(start.len() + end.len());
            candidate.push_str(start);
            candidate.push_str(end);
            if is_intact(&candidate) && !result.contains(&candidate) {
                result.push(candidate);
            }
        }
    }
    result
}

/// Beam cartesian product capped at `limit` per step, minus the exact original (FZ:71-94).
pub fn fuzzy_segmentations(
    segments: &[String],
    options: FuzzyPinyinOptions,
    limit: usize,
) -> Vec<Vec<String>> {
    if options.rules == 0 || segments.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut paths: Vec<Vec<String>> = vec![Vec::new()];
    for syllable in segments {
        let alternatives = fuzzy_syllables(syllable, options);
        let capacity = limit.min(paths.len().saturating_mul(alternatives.len()));
        let mut next = Vec::with_capacity(capacity);
        // The C++ only breaks the inner loop at the cap, which stops the product at `limit` all the same.
        'beam: for path in &paths {
            for alternative in &alternatives {
                if next.len() == limit {
                    break 'beam;
                }
                let mut extended = Vec::with_capacity(path.len() + 1);
                extended.extend_from_slice(path);
                extended.push(alternative.clone());
                next.push(extended);
            }
        }
        paths = next;
    }
    paths.retain(|path| path.as_slice() != segments);
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(rules: u32) -> FuzzyPinyinOptions {
        FuzzyPinyinOptions { rules }
    }

    // test_fuzzy_pinyin.cpp:66-111: each of the eleven rules forward and reverse, and only under its own bit.
    #[test]
    fn every_rule_expands_both_ways_under_its_own_bit_only() {
        let pairs = [
            ("zan", "zhan"),
            ("can", "chan"),
            ("san", "shan"),
            ("na", "la"),
            ("fa", "ha"),
            ("ran", "lan"),
            ("ban", "bang"),
            ("ben", "beng"),
            ("bin", "bing"),
            ("lian", "liang"),
            ("guan", "guang"),
        ];
        for (bit, (a, b)) in pairs.iter().enumerate() {
            let own = rules(1 << bit);
            assert!(
                fuzzy_syllables(a, own).contains(&(*b).to_owned()),
                "{a} -> {b}"
            );
            assert!(
                fuzzy_syllables(b, own).contains(&(*a).to_owned()),
                "{b} -> {a}"
            );
            let other = rules(1 << ((bit + 1) % pairs.len()));
            assert!(
                !fuzzy_syllables(a, other).contains(&(*b).to_owned()),
                "{a} under another rule"
            );
        }
    }

    #[test]
    fn expansions_keep_the_original_first_and_stay_intact() {
        let variants = fuzzy_syllables("zhan", rules(fuzzy_rule::ALL));
        assert_eq!(variants, ["zhan", "zhang", "zan", "zang"]);
        assert!(variants
            .iter()
            .all(|variant| variant.capacity() == variant.len()));
        // `l` is the partner of both `n` and `r`.
        assert_eq!(
            fuzzy_syllables("lan", rules(fuzzy_rule::N_L | fuzzy_rule::R_L)),
            ["lan", "nan", "ran"]
        );
        assert_eq!(fuzzy_syllables("an", rules(fuzzy_rule::ALL)), ["an", "ang"]);
        assert_eq!(fuzzy_syllables("zh", rules(fuzzy_rule::ALL)), ["zh"]);
        assert_eq!(fuzzy_syllables("bian", rules(fuzzy_rule::AN_ANG)), ["bian"]);
        assert_eq!(fuzzy_syllables("zan", rules(0)), ["zan"]);
    }

    #[test]
    fn segmentations_are_a_bounded_beam_without_the_original() {
        let typed = vec!["zan".to_owned(), "fa".to_owned()];
        let paths = fuzzy_segmentations(
            &typed,
            rules(fuzzy_rule::Z_ZH | fuzzy_rule::F_H),
            FUZZY_SEGMENTATION_LIMIT,
        );
        let joined: Vec<String> = paths.iter().map(|path| path.join("'")).collect();
        assert_eq!(joined, ["zan'ha", "zhan'fa", "zhan'ha"]);
        let long = vec!["lan".to_owned(); 20];
        assert!(
            fuzzy_segmentations(&long, rules(fuzzy_rule::ALL), FUZZY_SEGMENTATION_LIMIT).len()
                <= 63
        );
        assert!(fuzzy_segmentations(&typed, rules(0), FUZZY_SEGMENTATION_LIMIT).is_empty());
        assert!(
            fuzzy_segmentations(&[], rules(fuzzy_rule::ALL), FUZZY_SEGMENTATION_LIMIT).is_empty()
        );
        assert!(fuzzy_segmentations(&typed, rules(fuzzy_rule::ALL), 0).is_empty());
    }
}
