//! Query-layer helpers (schemes-lang.md §1.5, `shuangpin_query.cpp`). `raw_length_for_effective_prefix` and `apply_segmentation_cases` are also used by quanpin and the session.
//!
//! Raw input is ASCII (see `utils`), so offsets are byte offsets and letters are bytes.

use std::borrow::Cow;

use super::utils::{
    convert_seg_shuangpin_to_seg_complete_pinyin, is_all_complete_pinyin, is_full_help_mode,
    pinyin_segmentation, takes_two_keys,
};
use super::ShuangpinProfile;

pub fn segment_input(raw: &str, profile: &ShuangpinProfile) -> String {
    // Empty chunks from `''` or a leading or trailing `'` contribute nothing (:20-54).
    let mut output_len = 0usize;
    let mut piece_count = 0usize;
    let mut chunk_count = 0usize;
    for chunk in raw.split('\'').filter(|chunk| !chunk.is_empty()) {
        chunk_count += 1;
        output_len += chunk.len();
        let bytes = chunk.as_bytes();
        let mut position = 0;
        while position < bytes.len() {
            position += if takes_two_keys(bytes, position, profile) {
                2
            } else {
                1
            };
            piece_count += 1;
        }
    }
    // Segmentation keeps one delimiter between every pair of pieces, including chunks that
    // were separated manually; empty chunks contribute neither letters nor delimiters.
    output_len += piece_count.saturating_sub(1) + chunk_count.saturating_sub(1);
    let mut result = String::with_capacity(output_len);
    for chunk in raw.split('\'').filter(|chunk| !chunk.is_empty()) {
        if !result.is_empty() {
            result.push('\'');
        }
        result.push_str(&pinyin_segmentation(chunk, profile));
    }
    result
}

/// Raw byte offsets of unit starts, always including 0 and the length (:56-102).
pub fn segment_raw_boundaries(raw: &str, profile: &ShuangpinProfile) -> Vec<usize> {
    let bytes = raw.as_bytes();
    let mut boundaries = Vec::with_capacity(raw.len() + 1);
    if bytes.is_empty() {
        return boundaries;
    }
    boundaries.push(0);
    let mut chunk_start = 0;
    loop {
        let separator = bytes[chunk_start..]
            .iter()
            .position(|&byte| byte == b'\'')
            .map(|offset| chunk_start + offset);
        let chunk_end = separator.unwrap_or(bytes.len());
        let mut position = chunk_start;
        while position < chunk_end {
            // The same forward-greedy rule as `pinyin_segmentation`, bounded by the chunk.
            position += if takes_two_keys(&bytes[..chunk_end], position, profile) {
                2
            } else {
                1
            };
            boundaries.push(position);
        }
        let Some(separator) = separator else {
            break;
        };
        chunk_start = separator + 1;
        if chunk_start < bytes.len() {
            boundaries.push(chunk_start);
        }
    }
    // A trailing delimiter starts no unit, but the end of the spelling always belongs to the last unit.
    if boundaries.last() != Some(&bytes.len()) {
        boundaries.push(bytes.len());
    }
    boundaries.dedup();
    boundaries
}

pub fn to_quanpin_segmentation(segmentation: &str, profile: &ShuangpinProfile) -> String {
    convert_seg_shuangpin_to_seg_complete_pinyin(segmentation, profile)
}

pub fn normalize_input_with_delimiters(raw: &str, profile: &ShuangpinProfile) -> String {
    to_quanpin_segmentation(&segment_input(raw, profile), profile)
}

pub fn normalize_input(raw: &str, profile: &ShuangpinProfile) -> String {
    remove_manual_delimiters(&normalize_input_with_delimiters(raw, profile))
}

pub fn remove_manual_delimiters(raw: &str) -> String {
    let capacity = raw.bytes().filter(|&byte| byte != b'\'').count();
    let mut compact = String::with_capacity(capacity);
    for part in raw.split('\'') {
        compact.push_str(part);
    }
    compact
}

pub fn effective_input_length(raw: &str) -> usize {
    raw.bytes().filter(|&byte| byte != b'\'').count()
}

/// The raw byte count covering `length` non-`'` characters (:129-142).
pub fn raw_length_for_effective_prefix(raw: &str, length: usize) -> usize {
    let mut raw_length = 0;
    let mut effective = 0;
    for &byte in raw.as_bytes() {
        if effective >= length {
            break;
        }
        if byte != b'\'' {
            effective += 1;
        }
        raw_length += 1;
    }
    raw_length
}

/// Drop the last `count` letters, keeping the `'` before them; "" with fewer letters (:144-169).
pub fn trim_trailing_letters_preserve_delimiters(raw: &str, count: usize) -> String {
    if count == 0 || raw.is_empty() {
        return raw.to_string();
    }
    let mut remaining = count;
    for (position, &byte) in raw.as_bytes().iter().enumerate().rev() {
        if byte == b'\'' {
            continue;
        }
        remaining -= 1;
        if remaining == 0 {
            return raw[..position].to_string();
        }
    }
    String::new()
}

/// 2 for a double helpcode after a complete base not followed by `'`, else 0 (:171-190).
pub fn detect_active_double_helpcode_length(
    raw: &str,
    raw_with_cases: &str,
    profile: &ShuangpinProfile,
) -> usize {
    let cased = if raw_with_cases.is_empty() {
        raw
    } else {
        raw_with_cases
    };
    let effective_with_cases = if cased.contains('\'') {
        Cow::Owned(remove_manual_delimiters(cased))
    } else {
        Cow::Borrowed(cased)
    };
    if !is_full_help_mode(&effective_with_cases, profile) {
        return 0;
    }
    let raw_base_length =
        raw_length_for_effective_prefix(raw, effective_input_length(raw).saturating_sub(2));
    if raw.as_bytes().get(raw_base_length) == Some(&b'\'') {
        return 0;
    }
    if is_complete_input(&raw[..raw_base_length], profile) {
        2
    } else {
        0
    }
}

/// :192-219.
pub fn is_complete_input(raw: &str, profile: &ShuangpinProfile) -> bool {
    if raw.is_empty() || raw.starts_with('\'') || raw.ends_with('\'') || raw.contains("''") {
        return false;
    }
    raw.split('\'').all(|chunk| {
        !chunk.is_empty() && is_all_complete_pinyin(chunk, &pinyin_segmentation(chunk, profile))
    })
}

/// Re-apply the typed case to a lowercased segmentation, skipping `'` in both; unchanged if the letters differ (:221-282).
pub fn apply_segmentation_cases(segmentation: &str, raw_with_cases: &str) -> String {
    if segmentation.is_empty() || raw_with_cases.is_empty() {
        return String::new();
    }
    let letters = remove_manual_delimiters(segmentation);
    if letters != remove_manual_delimiters(raw_with_cases).to_ascii_lowercase() {
        return segmentation.to_string();
    }
    let cased = raw_with_cases.as_bytes();
    let mut result = String::with_capacity(segmentation.len());
    let mut index = 0;
    for &byte in segmentation.as_bytes() {
        if byte == b'\'' {
            result.push('\'');
            continue;
        }
        while index < cased.len() && cased[index] == b'\'' {
            index += 1;
        }
        let Some(&typed) = cased.get(index) else {
            return segmentation.to_string();
        };
        // The reference compares `ch == cased + ('a' - 'A')`, i.e. the typed uppercase letter of a lowercase key.
        if byte == typed || u16::from(byte) == u16::from(typed) + 32 {
            result.push(char::from(typed));
        } else {
            result.push(char::from(byte));
        }
        index += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shuangpin::profile::profile;
    use crate::types::ShuangpinProfileKind;

    fn xiaohe() -> &'static ShuangpinProfile {
        profile(ShuangpinProfileKind::Xiaohe).unwrap()
    }

    fn microsoft() -> &'static ShuangpinProfile {
        profile(ShuangpinProfileKind::Microsoft).unwrap()
    }

    #[test]
    fn segments_each_manual_chunk() {
        let segmentation = segment_input("nihcc", xiaohe());
        assert_eq!(segmentation, "ni'hc'c");
        assert_eq!(segmentation.capacity(), segmentation.len());
        assert_eq!(segment_input("ni''hc'", xiaohe()), "ni'hc");
        assert_eq!(segment_input("'ui'u", xiaohe()), "ui'u");
        assert_eq!(segment_input("", xiaohe()), "");
        assert_eq!(normalize_input_with_delimiters("nihc", xiaohe()), "ni'hao");
        assert_eq!(normalize_input("nihc", xiaohe()), "nihao");
        assert_eq!(normalize_input("uj'uv", xiaohe()), "shanshui");
    }

    #[test]
    fn raw_boundaries_follow_the_greedy_units() {
        // test_input_session.cpp:1312-1318.
        assert_eq!(
            segment_raw_boundaries("nihaoma", xiaohe()),
            vec![0, 2, 4, 5, 7]
        );
        assert_eq!(segment_raw_boundaries("nihcc", xiaohe()), vec![0, 2, 4, 5]);
        assert_eq!(segment_raw_boundaries("ni'hc", xiaohe()), vec![0, 2, 3, 5]);
        assert_eq!(segment_raw_boundaries("ni'", xiaohe()), vec![0, 2, 3]);
        assert_eq!(segment_raw_boundaries("", xiaohe()), Vec::<usize>::new());
        // test_runtime_isolation.cpp:203-253 and engine-bridge tests.rs:577-596.
        assert_eq!(segment_raw_boundaries("b;ni", microsoft()), vec![0, 2, 4]);
        assert_eq!(
            segment_raw_boundaries("nihkb;", microsoft()),
            vec![0, 2, 4, 6]
        );
        assert_eq!(
            segment_raw_boundaries("nihcb;", microsoft()),
            vec![0, 2, 3, 5, 6]
        );
        assert_eq!(normalize_input("n;", microsoft()), "ning");
    }

    #[test]
    fn effective_prefixes_skip_delimiters() {
        let compact = remove_manual_delimiters("ni'hc'");
        assert_eq!(compact, "nihc");
        assert_eq!(compact.capacity(), compact.len());
        let many = remove_manual_delimiters("a'a'a'a'a");
        assert_eq!(many, "aaaaa");
        assert_eq!(many.capacity(), many.len());
        assert_eq!(effective_input_length("ni'hc'"), 4);
        assert_eq!(raw_length_for_effective_prefix("ni'hc", 2), 2);
        assert_eq!(raw_length_for_effective_prefix("ni'hc", 3), 4);
        assert_eq!(raw_length_for_effective_prefix("ni'hc", 9), 5);
        assert_eq!(raw_length_for_effective_prefix("ni", 0), 0);
        assert_eq!(
            trim_trailing_letters_preserve_delimiters("nihcab", 2),
            "nihc"
        );
        assert_eq!(
            trim_trailing_letters_preserve_delimiters("ni'hc'ab", 2),
            "ni'hc'"
        );
        assert_eq!(trim_trailing_letters_preserve_delimiters("ui'u", 1), "ui'");
        assert_eq!(trim_trailing_letters_preserve_delimiters("a", 2), "");
        assert_eq!(trim_trailing_letters_preserve_delimiters("ab", 0), "ab");
    }

    #[test]
    fn double_helpcode_needs_a_complete_undelimited_base() {
        assert_eq!(
            detect_active_double_helpcode_length("nihcab", "nihcAB", xiaohe()),
            2
        );
        assert_eq!(
            detect_active_double_helpcode_length("nihcab", "nihcab", xiaohe()),
            0
        );
        assert_eq!(
            detect_active_double_helpcode_length("nihc'ab", "nihc'AB", xiaohe()),
            0
        );
        assert_eq!(
            detect_active_double_helpcode_length("ni'hcab", "ni'hcAB", xiaohe()),
            2
        );
        // A trailing delimiter after the codes does not reach the base (test_input_session.cpp:1019).
        assert_eq!(
            detect_active_double_helpcode_length("nihcab'", "nihcAB'", xiaohe()),
            2
        );
        assert_eq!(
            detect_active_double_helpcode_length("nihcab", "", xiaohe()),
            0
        );
    }

    #[test]
    fn double_helpcode_detection_without_delimiters_does_not_copy_input() {
        let profile = xiaohe();
        assert_eq!(
            detect_active_double_helpcode_length("nihcab", "nihcAB", profile),
            2
        );
        let (length, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            detect_active_double_helpcode_length("nihcab", "nihcAB", profile)
        });

        assert_eq!(length, 2);
        assert_eq!(allocations, 10);
    }

    #[test]
    fn complete_input_rejects_stray_delimiters() {
        assert!(is_complete_input("ni'hc", xiaohe()));
        assert!(is_complete_input("nihc", xiaohe()));
        assert!(!is_complete_input("nihcc", xiaohe()));
        assert!(!is_complete_input("'ni", xiaohe()));
        assert!(!is_complete_input("ni'", xiaohe()));
        assert!(!is_complete_input("ni''hc", xiaohe()));
        assert!(!is_complete_input("", xiaohe()));
    }

    #[test]
    fn cases_return_to_the_segmentation() {
        assert_eq!(apply_segmentation_cases("ni'hc'ab", "nihcAB"), "ni'hc'AB");
        assert_eq!(apply_segmentation_cases("ni'hc'ab", "nihcAB'"), "ni'hc'AB");
        assert_eq!(apply_segmentation_cases("ni'hc", "Ni'hC"), "Ni'hC");
        assert_eq!(apply_segmentation_cases("ni'hc", "nihx"), "ni'hc");
        assert_eq!(apply_segmentation_cases("", "ni"), "");
        assert_eq!(apply_segmentation_cases("ni", ""), "");
    }
}
