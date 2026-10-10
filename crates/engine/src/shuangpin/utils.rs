//! Code conversion (schemes-lang.md §1.4, `shuangpin_utils.cpp`).
//!
//! Every input here is ASCII: the scheme only appends letters, `'` and the Microsoft `;`, and the host facade rejects non-ASCII characters before they reach a scheme. The functions index bytes accordingly.

use super::profile::accepted_syllables;
use super::ShuangpinProfile;

/// The first source whose key equals `code` (shuangpin_utils.cpp:24-34).
fn source_by_code(mapping: &[(&'static str, &'static str)], code: &str) -> Option<&'static str> {
    mapping
        .iter()
        .find(|(_, mapped)| *mapped == code)
        .map(|(source, _)| *source)
}

/// A one-key piece read as an initial: `u` is `sh` in xiaohe, any other letter is itself.
pub(crate) fn initial_for_key<'a>(key: &'a str, profile: &ShuangpinProfile) -> &'a str {
    source_by_code(profile.initials, key).unwrap_or(key)
}

/// One two-key code (or a zero-initial code) to its syllable, "" when it is not accepted (:64-105).
pub fn cvt_single_sp_to_pinyin(code: &str, profile: &ShuangpinProfile) -> String {
    // The zero-initial table is consulted before the length check, as the reference does.
    if let Some(syllable) = source_by_code(profile.zero_initials, code) {
        return syllable.to_string();
    }
    if code.len() != 2 || !code.is_char_boundary(1) {
        return String::new();
    }
    let initial = initial_for_key(&code[..1], profile);
    let final_key = &code[1..];
    let accepted = accepted_syllables();
    // No two-key code decodes to two accepted syllables in any profile (schemes-lang.md §1.3), so the last match the reference kept is the only match.
    let mut result = String::new();
    for (final_unit, _) in profile.finals.iter().filter(|(_, key)| *key == final_key) {
        let normalized = if *final_unit == "v" && matches!(initial, "j" | "q" | "x" | "y") {
            "u"
        } else {
            final_unit
        };
        let mut syllable = String::with_capacity(initial.len() + normalized.len());
        syllable.push_str(initial);
        syllable.push_str(normalized);
        if accepted.contains(syllable.as_str()) {
            result = syllable;
        }
    }
    result
}

pub fn is_accepted_syllable_code(code: &str, profile: &ShuangpinProfile) -> bool {
    accepted_syllables().contains(cvt_single_sp_to_pinyin(code, profile).as_str())
}

/// Whether the two keys at `position` form an accepted syllable, case-insensitively.
pub(crate) fn takes_two_keys(input: &[u8], position: usize, profile: &ShuangpinProfile) -> bool {
    input.len() >= position + 2 && {
        let pair = lowercase_pair(input, position);
        std::str::from_utf8(&pair).is_ok_and(|pair| is_accepted_syllable_code(pair, profile))
    }
}

fn lowercase_pair(input: &[u8], position: usize) -> [u8; 2] {
    [
        input[position].to_ascii_lowercase(),
        input[position + 1].to_ascii_lowercase(),
    ]
}

/// Forward-greedy two-then-one split, case kept (:118-158).
pub fn pinyin_segmentation(input: &str, profile: &ShuangpinProfile) -> String {
    if input.len() == 1 {
        return input.to_string();
    }
    let bytes = input.as_bytes();
    let width_at = |position: usize| {
        if takes_two_keys(bytes, position, profile) {
            2
        } else {
            1
        }
    };
    let mut piece_count = 0usize;
    let mut position = 0;
    while position < bytes.len() {
        let width = width_at(position);
        piece_count += 1;
        position += width;
    }
    let leading_apostrophes = bytes.iter().take_while(|&&byte| byte == b'\'').count();
    let copied_bytes = bytes.len() - leading_apostrophes;
    let separators = piece_count.saturating_sub(leading_apostrophes + 1);
    let mut result = String::with_capacity(copied_bytes + separators);
    position = 0;
    while position < bytes.len() {
        let width = width_at(position);
        if !result.is_empty() {
            result.push('\'');
        }
        for &byte in &bytes[position..position + width] {
            if !result.is_empty() || byte != b'\'' {
                result.push(char::from(byte));
            }
        }
        position += width;
    }
    // A chunk containing `'` can be reread for single-helpcode matching. Its leading delimiters are stripped, while delimiters after the first key stay in the output.
    result
}

/// Even length and every chunk exactly two keys (:221-234).
pub fn is_all_complete_pinyin(pure: &str, segmentation: &str) -> bool {
    if !pure.len().is_multiple_of(2) {
        return false;
    }
    let bytes = segmentation.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        // The reference read `seg[i + 1]` up to the terminating NUL, which is never `'`.
        if bytes[index] == b'\'' || bytes.get(index + 1) == Some(&b'\'') {
            return false;
        }
        index += 3;
    }
    true
}

/// Shuangpin segmentation to quanpin segmentation (:242-260).
pub fn convert_seg_shuangpin_to_seg_complete_pinyin(
    segmentation: &str,
    profile: &ShuangpinProfile,
) -> String {
    let mut result = String::with_capacity(segmentation.len() * 2);
    for piece in segmentation.split('\'') {
        match piece.len() {
            1 => {
                result.push_str(initial_for_key(piece, profile));
                result.push('\'');
            }
            // An unaccepted pair leaves an empty syllable between two `'`, as the reference did.
            2 => {
                result.push_str(&cvt_single_sp_to_pinyin(piece, profile));
                result.push('\'');
            }
            _ => {}
        }
    }
    result.pop();
    result
}

/// Double helpcode after a complete even-length prefix (:269-287).
pub fn is_full_help_mode(pinyin_with_cases: &str, profile: &ShuangpinProfile) -> bool {
    let bytes = pinyin_with_cases.as_bytes();
    let length = bytes.len();
    if length <= 2 || !length.is_multiple_of(2) {
        return false;
    }
    let base = &pinyin_with_cases[..length - 2];
    is_all_complete_pinyin(base, &pinyin_segmentation(base, profile))
        && (bytes[length - 2].is_ascii_uppercase() || bytes[length - 1].is_ascii_uppercase())
}

/// The last two letters lowercased, swapped when the first was uppercase (:289-305).
pub fn get_full_help_codes(pinyin_with_cases: &str) -> String {
    let bytes = pinyin_with_cases.as_bytes();
    if bytes.len() < 2 {
        return String::new();
    }
    let first = bytes[bytes.len() - 2];
    let second = bytes[bytes.len() - 1];
    let (first, second) = if first.is_ascii_uppercase() {
        (second, first)
    } else {
        (first, second)
    };
    let mut result = String::with_capacity(2);
    result.push(char::from(first.to_ascii_lowercase()));
    result.push(char::from(second.to_ascii_lowercase()));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shuangpin::profile::profile;
    use crate::types::ShuangpinProfileKind;

    const ALL: [ShuangpinProfileKind; 4] = [
        ShuangpinProfileKind::Xiaohe,
        ShuangpinProfileKind::Ziranma,
        ShuangpinProfileKind::Shoudao,
        ShuangpinProfileKind::Microsoft,
    ];

    fn xiaohe() -> &'static ShuangpinProfile {
        profile(ShuangpinProfileKind::Xiaohe).unwrap()
    }

    #[test]
    fn converts_codes_per_profile() {
        assert_eq!(cvt_single_sp_to_pinyin("ni", xiaohe()), "ni");
        assert_eq!(cvt_single_sp_to_pinyin("hc", xiaohe()), "hao");
        assert_eq!(cvt_single_sp_to_pinyin("ui", xiaohe()), "shi");
        assert_eq!(cvt_single_sp_to_pinyin("vs", xiaohe()), "zhong");
        assert_eq!(cvt_single_sp_to_pinyin("ah", xiaohe()), "ang");
        // Vowel-initial codes decode only through the zero-initial table.
        assert_eq!(cvt_single_sp_to_pinyin("ak", xiaohe()), "");
        // `v` after j/q/x/y is spelled `u`.
        assert_eq!(cvt_single_sp_to_pinyin("jv", xiaohe()), "ju");
        assert_eq!(cvt_single_sp_to_pinyin("lv", xiaohe()), "lv");
        assert_eq!(cvt_single_sp_to_pinyin("n", xiaohe()), "");
        let microsoft = profile(ShuangpinProfileKind::Microsoft).unwrap();
        assert_eq!(cvt_single_sp_to_pinyin("n;", microsoft), "ning");
        assert_eq!(cvt_single_sp_to_pinyin("oa", microsoft), "a");
        assert_eq!(cvt_single_sp_to_pinyin("lv", microsoft), "lve");
        let shoudao = profile(ShuangpinProfileKind::Shoudao).unwrap();
        assert_eq!(cvt_single_sp_to_pinyin("ei", shoudao), "shi");
        assert_eq!(cvt_single_sp_to_pinyin("ue", shoudao), "e");
        let ziranma = profile(ShuangpinProfileKind::Ziranma).unwrap();
        assert_eq!(cvt_single_sp_to_pinyin("hk", ziranma), "hao");
    }

    #[test]
    fn converted_syllable_uses_exact_string_capacity() {
        let result = cvt_single_sp_to_pinyin("vs", xiaohe());
        assert_eq!(result, "zhong");
        assert_eq!(result.capacity(), result.len());
    }

    #[test]
    fn key_pair_lowering_stays_on_the_stack() {
        assert_eq!(lowercase_pair(b"NI", 0), [b'n', b'i']);
    }

    #[test]
    fn full_help_codes_use_exact_string_capacity() {
        let result = get_full_help_codes("xxAb");
        assert_eq!(result, "ba");
        assert_eq!(result.capacity(), result.len());
    }

    #[test]
    fn every_profile_accepts_yo_as_one_syllable() {
        for kind in ALL {
            let selected = profile(kind).unwrap();
            assert!(is_accepted_syllable_code("yo", selected), "{kind:?}");
            assert_eq!(pinyin_segmentation("yo", selected), "yo", "{kind:?}");
        }
    }

    #[test]
    fn segments_forward_greedy_keeping_case() {
        assert_eq!(pinyin_segmentation("nihaoma", xiaohe()), "ni'ha'o'ma");
        let segmentation = pinyin_segmentation("nihcc", xiaohe());
        assert_eq!(segmentation, "ni'hc'c");
        assert_eq!(segmentation.capacity(), segmentation.len());
        assert_eq!(pinyin_segmentation("NiHc", xiaohe()), "Ni'Hc");
        assert_eq!(pinyin_segmentation("n", xiaohe()), "n");
        assert_eq!(pinyin_segmentation("cls", xiaohe()), "c'ls");
        assert_eq!(pinyin_segmentation("", xiaohe()), "");
        // A delimited input read as one chunk (the single-helpcode tail) treats `'` as a key that forms nothing.
        assert_eq!(pinyin_segmentation("ni'hck", xiaohe()), "ni'''hc'k");
        assert_eq!(pinyin_segmentation("''ni", xiaohe()), "ni");
        assert_eq!(pinyin_segmentation("ni''hc", xiaohe()), "ni'''''hc");
    }

    #[test]
    fn complete_means_every_chunk_is_two_keys() {
        assert!(is_all_complete_pinyin("nihc", "ni'hc"));
        assert!(!is_all_complete_pinyin("nihcc", "ni'hc'c"));
        assert!(!is_all_complete_pinyin("nh", "n'h"));
        assert!(is_all_complete_pinyin("", ""));
        assert!(is_all_complete_pinyin("ni", "ni"));
    }

    #[test]
    fn converts_segmentations_to_quanpin() {
        assert_eq!(
            convert_seg_shuangpin_to_seg_complete_pinyin("ni'hc'c", xiaohe()),
            "ni'hao'c"
        );
        assert_eq!(
            convert_seg_shuangpin_to_seg_complete_pinyin("u'a", xiaohe()),
            "sh'a"
        );
        assert_eq!(
            convert_seg_shuangpin_to_seg_complete_pinyin("ni'ak", xiaohe()),
            "ni'"
        );
        assert_eq!(
            convert_seg_shuangpin_to_seg_complete_pinyin("", xiaohe()),
            ""
        );
        assert_eq!(
            convert_seg_shuangpin_to_seg_complete_pinyin("ni'''hc'k", xiaohe()),
            "ni'hao'k"
        );
    }

    #[test]
    fn detects_double_helpcodes() {
        assert!(is_full_help_mode("nihcAB", xiaohe()));
        assert!(is_full_help_mode("nihcaB", xiaohe()));
        assert!(!is_full_help_mode("nihcab", xiaohe()));
        assert!(!is_full_help_mode("AB", xiaohe()));
        assert!(!is_full_help_mode("nihAB", xiaohe()));
        assert!(!is_full_help_mode("akAB", xiaohe()));
        assert_eq!(get_full_help_codes("xxAb"), "ba");
        assert_eq!(get_full_help_codes("xxaB"), "ab");
        assert_eq!(get_full_help_codes("xxAB"), "ba");
        assert_eq!(get_full_help_codes("x"), "");
    }
}
