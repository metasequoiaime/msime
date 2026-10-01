//! Segmentation (quanpin.md §3): the minimum-segment cut, the greedy cut over `'`-parts, and the correction-mode cut with its alias table, which the quanpin scheme always uses whatever the autocorrect mask says.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::syllables::{intact_piece, intact_pinyin_list, is_prefix_piece, MAX_SYLLABLE_LENGTH};

/// At most this many correction paths per part and in the cartesian product (QQ:39).
pub const CORRECTION_PATH_LIMIT: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutMode {
    /// `cut_pinyin_greedy(p, false)`; used only to re-cut literally when autocorrect is suppressed.
    Greedy,
    /// Every correction path, falling back to the greedy cut.
    Correction,
}

/// Minimum segment count; among equals the shortest first piece, recursively (QU:354-422). Empty means no cut. Keeps `keneng -> ke'neng` and `fangan -> fan'gan`.
pub fn cut_one_piece_min_segments(pinyin: &str, intact_only: bool) -> Vec<String> {
    let bytes = pinyin.as_bytes();
    let length = bytes.len();
    let in_set = |piece: &[u8]| {
        if intact_only {
            intact_piece(piece).is_some()
        } else {
            is_prefix_piece(piece)
        }
    };
    // best[i] = (end of the first piece, segment count) of the chosen cut of `pinyin[i..]`. The C++ memoised recursion's result at an index does not depend on call order, so filling it from the back gives the same cuts.
    let mut best: Vec<Option<(usize, usize)>> = vec![None; length + 1];
    for index in (0..length).rev() {
        let mut chosen: Option<(usize, usize)> = None;
        // Longest piece first; both sets only hold pieces of at most six letters.
        for end in (index + 1..=length.min(index + MAX_SYLLABLE_LENGTH)).rev() {
            if !in_set(&bytes[index..end]) {
                continue;
            }
            let count = if end == length {
                1
            } else {
                match best[end] {
                    Some((_, suffix_count)) => 1 + suffix_count,
                    None => continue,
                }
            };
            chosen = match chosen {
                None => Some((end, count)),
                Some((_, best_count)) if count < best_count => Some((end, count)),
                // Equal counts: the shorter first piece hands the next syllable the consonant (`fan'gan`), QU:409-413.
                Some((best_end, best_count)) if count == best_count && end < best_end => {
                    Some((end, count))
                }
                unchanged => unchanged,
            };
        }
        best[index] = chosen;
    }
    let mut segments = Vec::with_capacity(length);
    let mut index = 0;
    while index < length {
        let Some((end, _)) = best[index] else {
            return Vec::new();
        };
        segments.push(pinyin[index..end].to_owned());
        index = end;
    }
    segments
}

/// QQ:913-944: cut each `'`-part; a failed part is kept raw unless `intact_only`, which fails the whole input.
pub fn cut_pinyin_greedy(pinyin: &str, intact_only: bool) -> Vec<String> {
    if pinyin.is_empty() {
        return Vec::new();
    }
    if !pinyin.contains('\'') {
        return cut_one_piece_min_segments(pinyin, intact_only);
    }
    let mut merged = Vec::with_capacity(pinyin.len());
    for part in pinyin.split('\'') {
        let cut = cut_one_piece_min_segments(part, intact_only);
        if !cut.is_empty() {
            merged.extend(cut);
        } else if !part.is_empty() {
            if intact_only {
                return Vec::new();
            }
            merged.push(part.to_owned());
        }
    }
    merged
}

/// Typed spellings the correction cut reads as syllables, each with its readings in priority order (QQ:43-95).
fn correction_aliases() -> &'static HashMap<String, Vec<&'static str>> {
    static ALIASES: OnceLock<HashMap<String, Vec<&'static str>>> = OnceLock::new();
    ALIASES.get_or_init(|| {
        fn add_alias(
            aliases: &mut HashMap<String, Vec<&'static str>>,
            typed: String,
            canonical: &'static str,
        ) {
            let readings = aliases.entry(typed).or_default();
            if !readings.contains(&canonical) {
                readings.push(canonical);
            }
        }
        fn add_suffix_aliases(
            aliases: &mut HashMap<String, Vec<&'static str>>,
            canonical_suffix: &str,
            typed_suffix: &str,
        ) {
            for &syllable in intact_pinyin_list() {
                if let Some(prefix) = syllable.strip_suffix(canonical_suffix) {
                    add_alias(aliases, format!("{prefix}{typed_suffix}"), syllable);
                }
            }
        }
        // The insertion order sets each key's primary reading, so this sequence is the contract.
        let mut aliases = HashMap::new();
        add_suffix_aliases(&mut aliases, "iang", "aing");
        add_suffix_aliases(&mut aliases, "uang", "aung");
        add_suffix_aliases(&mut aliases, "ian", "ain");
        add_suffix_aliases(&mut aliases, "uan", "aun");
        add_suffix_aliases(&mut aliases, "iao", "aio");
        add_suffix_aliases(&mut aliases, "ing", "ihng");
        add_suffix_aliases(&mut aliases, "ang", "agn");
        add_suffix_aliases(&mut aliases, "eng", "egn");
        add_alias(&mut aliases, "egn".to_owned(), "eng");
        add_alias(&mut aliases, "jv".to_owned(), "ju");
        // Missing final g before the next syllable: zhonguo -> zhong'guo.
        add_suffix_aliases(&mut aliases, "ong", "on");
        // A transposed h is the primary reading (ahng -> hang), the extra h the alternative (ahng -> ang); likewise cehng -> cheng, ceng.
        add_suffix_aliases(&mut aliases, "hang", "ahng");
        add_suffix_aliases(&mut aliases, "ang", "ahng");
        add_suffix_aliases(&mut aliases, "heng", "ehng");
        add_suffix_aliases(&mut aliases, "eng", "ehng");
        aliases
    })
}

/// The longest piece the correction cut can read: alias keys run to seven letters (`shuahng`), past the longest syllable.
fn max_correction_piece_length() -> usize {
    static LENGTH: OnceLock<usize> = OnceLock::new();
    *LENGTH.get_or_init(|| {
        correction_aliases()
            .keys()
            .map(String::len)
            .max()
            .unwrap_or(0)
            .max(MAX_SYLLABLE_LENGTH)
    })
}

#[derive(Debug, Clone)]
struct RankedPath {
    segments: Vec<&'static str>,
    typed_lengths: Vec<usize>,
    correction_ranks: Vec<usize>,
}

/// QQ:97-200. An empty part has one empty path.
fn cut_one_piece_with_corrections(pinyin: &str) -> Vec<Vec<&'static str>> {
    let bytes = pinyin.as_bytes();
    let length = bytes.len();
    let aliases = correction_aliases();
    let max_piece = max_correction_piece_length();
    // paths[i] = the ranked cuts of `pinyin[i..]`, filled from the back; the C++ memo gives the same lists whatever the call order.
    let mut paths: Vec<Vec<RankedPath>> = vec![Vec::new(); length + 1];
    paths[length] = vec![RankedPath {
        segments: Vec::new(),
        typed_lengths: Vec::new(),
        correction_ranks: Vec::new(),
    }];
    for index in (0..length).rev() {
        let mut ranked = Vec::with_capacity(CORRECTION_PATH_LIMIT);
        for end in (index + 1..=length.min(index + max_piece)).rev() {
            let Ok(typed) = std::str::from_utf8(&bytes[index..end]) else {
                continue;
            };
            // The alias lookup comes first, so the alias of `jv` shadows the literal syllable (QQ:125-136).
            let literal;
            let readings: &[&'static str] = if let Some(readings) = aliases.get(typed) {
                readings
            } else if let Some(syllable) = intact_piece(typed.as_bytes()) {
                literal = [syllable];
                &literal
            } else {
                continue;
            };
            for (rank, &reading) in readings.iter().enumerate() {
                for suffix in &paths[end] {
                    let mut segments = Vec::with_capacity(1 + suffix.segments.len());
                    segments.push(reading);
                    segments.extend_from_slice(&suffix.segments);
                    let mut typed_lengths = Vec::with_capacity(1 + suffix.typed_lengths.len());
                    typed_lengths.push(end - index);
                    typed_lengths.extend_from_slice(&suffix.typed_lengths);
                    let mut correction_ranks =
                        Vec::with_capacity(1 + suffix.correction_ranks.len());
                    correction_ranks.push(rank);
                    correction_ranks.extend_from_slice(&suffix.correction_ranks);
                    ranked.push(RankedPath {
                        segments,
                        typed_lengths,
                        correction_ranks,
                    });
                }
            }
        }
        ranked.sort_by(|lhs, rhs| {
            lhs.segments
                .len()
                .cmp(&rhs.segments.len())
                .then_with(|| lhs.typed_lengths.cmp(&rhs.typed_lengths))
                .then_with(|| lhs.correction_ranks.cmp(&rhs.correction_ranks))
        });
        ranked.dedup_by(|later, earlier| later.segments == earlier.segments);
        if let Some(minimum) = ranked.first().map(|path| path.segments.len()) {
            ranked.retain(|path| path.segments.len() == minimum);
        }
        ranked.truncate(CORRECTION_PATH_LIMIT);
        paths[index] = ranked;
    }
    std::mem::take(&mut paths[0])
        .into_iter()
        .map(|path| path.segments)
        .collect()
}

/// QQ:97-234: alias-aware cut of every `'`-part and their cartesian product, capped at `CORRECTION_PATH_LIMIT`. An empty part (trailing `'`) contributes nothing; a non-empty part without a path empties the result.
pub fn cut_pinyin_with_corrections(pinyin: &str) -> Vec<Vec<String>> {
    let mut merged: Vec<Vec<&'static str>> = vec![Vec::new()];
    for part in pinyin.split('\'') {
        let part_paths = cut_one_piece_with_corrections(part);
        if part_paths.is_empty() {
            return Vec::new();
        }
        let capacity = CORRECTION_PATH_LIMIT.min(merged.len().saturating_mul(part_paths.len()));
        let mut combined = Vec::with_capacity(capacity);
        'product: for head in &merged {
            for tail in &part_paths {
                let mut path = Vec::with_capacity(head.len() + tail.len());
                path.extend_from_slice(head);
                path.extend_from_slice(tail);
                combined.push(path);
                if combined.len() == CORRECTION_PATH_LIMIT {
                    break 'product;
                }
            }
        }
        merged = combined;
    }
    merged
        .into_iter()
        .map(|path| path.into_iter().map(str::to_owned).collect())
        .collect()
}

/// QQ:946-976.
pub fn cut_pinyin_by_mode(pinyin: &str, mode: CutMode) -> Vec<Vec<String>> {
    if mode == CutMode::Correction {
        let corrected = cut_pinyin_with_corrections(pinyin);
        if !corrected.is_empty() {
            return corrected;
        }
    }
    let greedy = cut_pinyin_greedy(pinyin, false);
    if greedy.is_empty() {
        Vec::new()
    } else {
        vec![greedy]
    }
}

/// Split on `'`, keeping empty parts; empty input gives no segments (QQ:978-986).
pub fn split_segments(segmentation: &str) -> Vec<String> {
    if segmentation.is_empty() {
        return Vec::new();
    }
    let count = segmentation.bytes().filter(|&byte| byte == b'\'').count() + 1;
    let mut segments = Vec::with_capacity(count);
    segments.extend(segmentation.split('\'').map(str::to_owned));
    segments
}

/// Join with `'` (QQ:988-1000).
pub fn join_segments(segments: &[String]) -> String {
    segments.join("'")
}

/// Non-empty and every `'`-part a complete intact cut (QU:424-445).
pub fn is_complete_pinyin_input(pinyin: &str) -> bool {
    !pinyin.is_empty()
        && pinyin
            .split('\'')
            .all(|part| !part.is_empty() && !cut_one_piece_min_segments(part, true).is_empty())
}

/// The first letter of each non-empty segment (QQ:245-256).
pub fn segments_to_jianpin(segments: &[String]) -> String {
    let mut result = String::with_capacity(segments.len());
    for segment in segments {
        if let Some(initial) = segment.chars().next() {
            result.push(initial);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(pinyin: &str, intact_only: bool) -> String {
        join_segments(&cut_one_piece_min_segments(pinyin, intact_only))
    }

    fn correction_paths(pinyin: &str) -> Vec<String> {
        cut_pinyin_by_mode(pinyin, CutMode::Correction)
            .iter()
            .map(|path| join_segments(path))
            .collect()
    }

    #[test]
    fn min_segment_cut_prefers_fewer_segments_then_a_shorter_head() {
        assert_eq!(joined("keneng", true), "ke'neng");
        assert_eq!(joined("fangan", true), "fan'gan");
        assert_eq!(joined("dangan", true), "dan'gan");
        assert_eq!(joined("shangai", true), "shan'gai");
        assert_eq!(joined("zhonge", true), "zhong'e");
        assert_eq!(joined("tiane", true), "tian'e");
        assert_eq!(joined("jianmingeyao", true), "jian'min'ge'yao");
        assert_eq!(joined("zhonge", false), "zhon'ge");
        assert_eq!(joined("tiane", false), "tia'ne");
        assert_eq!(joined("nihaoz", false), "ni'hao'z");
        assert_eq!(joined("sahng", false), "sa'h'n'g");
        assert!(cut_one_piece_min_segments("nihaoz", true).is_empty());
        assert!(cut_one_piece_min_segments("", true).is_empty());
        assert!(cut_one_piece_min_segments("你好", false).is_empty());
    }

    #[test]
    fn greedy_cut_handles_apostrophe_parts() {
        assert_eq!(join_segments(&cut_pinyin_greedy("xi'an", true)), "xi'an");
        assert_eq!(
            join_segments(&cut_pinyin_greedy("ni'hao'", false)),
            "ni'hao"
        );
        assert_eq!(
            join_segments(&cut_pinyin_greedy("ni''hao", false)),
            "ni'hao"
        );
        assert_eq!(join_segments(&cut_pinyin_greedy("ni'hz", true)), "");
        assert_eq!(join_segments(&cut_pinyin_greedy("ni'ü", false)), "ni'ü");
        assert!(cut_pinyin_greedy("", false).is_empty());
    }

    #[test]
    fn the_alias_table_has_the_reference_keys() {
        let aliases = correction_aliases();
        assert_eq!(aliases.len(), 186);
        let ambiguous: Vec<_> = {
            let mut keys: Vec<_> = aliases
                .iter()
                .filter(|(_, readings)| readings.len() > 1)
                .map(|(key, readings)| format!("{key}:{}", readings.join(",")))
                .collect();
            keys.sort();
            keys
        };
        assert_eq!(
            ambiguous,
            [
                "ahng:hang,ang",
                "cahng:chang,cang",
                "cehng:cheng,ceng",
                "sahng:shang,sang",
                "sehng:sheng,seng",
                "zahng:zhang,zang",
                "zehng:zheng,zeng"
            ]
        );
        assert_eq!(max_correction_piece_length(), 7);
    }

    // test_pinyin.cpp:680-708.
    #[test]
    fn correction_mode_orders_corrections() {
        let cases = [
            ("laing", "liang"),
            ("haung", "huang"),
            ("bain", "bian"),
            ("daun", "duan"),
            ("laio", "liao"),
            ("mihng", "ming"),
            ("ahng", "hang"),
            ("behng", "beng"),
            ("agn", "ang"),
            ("zagn", "zang"),
            ("egn", "eng"),
            ("zhegn", "zheng"),
            ("jv", "ju"),
            ("wojv", "wo'ju"),
            ("wo'jv", "wo'ju"),
            ("woxainxin", "wo'xian'xin"),
        ];
        for (typed, expected) in cases {
            let paths = correction_paths(typed);
            assert_eq!(paths.first().map(String::as_str), Some(expected), "{typed}");
        }
        let cehng = correction_paths("cehng");
        assert!(cehng.len() >= 2 && cehng[0] == "cheng" && cehng.contains(&"ceng".to_owned()));
        let ahng = correction_paths("ahng");
        assert!(ahng.len() >= 2 && ahng[0] == "hang" && ahng.contains(&"ang".to_owned()));
    }

    #[test]
    fn correction_mode_keeps_every_minimum_path_in_order() {
        assert_eq!(correction_paths("zhonge"), ["zhong'ge", "zhong'e"]);
        assert_eq!(correction_paths("fangan"), ["fan'gan", "fang'an"]);
        assert_eq!(correction_paths("sahng"), ["shang", "sang"]);
        assert_eq!(
            correction_paths("jianmingeyao"),
            ["jian'min'ge'yao", "jian'ming'e'yao"]
        );
        assert_eq!(correction_paths("gonge"), ["gong'ge", "gong'e"]);
        assert_eq!(correction_paths("woxainxin"), ["wo'xian'xin"]);
        assert_eq!(correction_paths("zhonguo"), ["zhong'guo"]);
    }

    #[test]
    fn correction_mode_treats_empty_parts_as_empty_paths() {
        assert_eq!(correction_paths("nihao'"), ["ni'hao"]);
        assert_eq!(
            cut_pinyin_by_mode("", CutMode::Correction),
            vec![Vec::<String>::new()]
        );
        assert!(cut_pinyin_by_mode("", CutMode::Greedy).is_empty());
        // No correction path: the greedy prefix cut is the fallback.
        assert_eq!(correction_paths("nihz"), ["ni'h'z"]);
        // A part without a correction path fails the whole correction cut.
        assert_eq!(correction_paths("sahng'x"), ["sa'h'n'g'x"]);
        assert_eq!(correction_paths("sahng'ni"), ["shang'ni", "sang'ni"]);
        assert_eq!(
            cut_pinyin_by_mode("sahng", CutMode::Greedy),
            vec![vec![
                "sa".to_owned(),
                "h".to_owned(),
                "n".to_owned(),
                "g".to_owned()
            ]]
        );
    }

    #[test]
    fn the_product_is_capped() {
        // Seven ambiguous parts would give 128 paths.
        let input = ["sahng"; 7].join("'");
        assert_eq!(
            cut_pinyin_with_corrections(&input).len(),
            CORRECTION_PATH_LIMIT
        );
    }

    #[test]
    fn utilities_follow_the_reference() {
        assert!(split_segments("").is_empty());
        let segments = split_segments("ni''hao'");
        assert_eq!(segments, ["ni", "", "hao", ""]);
        assert_eq!(segments.capacity(), segments.len());
        let many = split_segments("a'a'a'a'a");
        assert_eq!(many.len(), 5);
        assert_eq!(many.capacity(), many.len());
        assert_eq!(join_segments(&split_segments("ni'hao")), "ni'hao");
        assert_eq!(segments_to_jianpin(&split_segments("ni''hao")), "nh");
        // Spellings only a minimum-segment cut can split are complete (test_input_session.cpp:978-986).
        assert!(is_complete_pinyin_input("linian"));
        assert!(is_complete_pinyin_input("jinian"));
        assert!(is_complete_pinyin_input("jinianri"));
        assert!(is_complete_pinyin_input("xi'an"));
        assert!(!is_complete_pinyin_input("nih"));
        assert!(!is_complete_pinyin_input("zhonguo"));
        assert!(!is_complete_pinyin_input("ni'"));
        assert!(!is_complete_pinyin_input(""));
    }
}
