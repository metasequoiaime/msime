//! Which inputs carry no redistribution grant. By default the build leaves them out, so a release holds only data the project may redistribute; `--include-unlicensed` (or `MSIME_DICT_INCLUDE_UNLICENSED=1`) builds the complete dictionary for local evaluation, never for a release. When an upstream grants permission in writing, remove its entry here and update `resources/licenses/msime-engine-dictionary-NOTICE.md` in the same change.

pub const ENV_FLAG: &str = "MSIME_DICT_INCLUDE_UNLICENSED";

pub const BASE_DICT_PART1: &str = "sources/unlicensed/custom-pinyin-dictionary-part1.txt";
pub const BASE_DICT_PART2: &str = "sources/unlicensed/custom-pinyin-dictionary-part2.txt";
pub const SINGLE_CHAR_WHITELIST: &str = "sources/unlicensed/single-char-whitelist.txt";
pub const OALDPE_WORDS: &str = "sources/unlicensed/oaldpe-words.txt";

/// Input, why it cannot be redistributed, and the licensed input used in its place.
pub const UNLICENSED_INPUTS: &[(&str, &str, Option<&str>)] = &[
    (
        BASE_DICT_PART1,
        "merged from CustomPinyinDictionary, which declares no licence",
        Some("sources/pinyin/rime-ice.txt"),
    ),
    (
        BASE_DICT_PART2,
        "merged from CustomPinyinDictionary, which declares no licence",
        None,
    ),
    (
        SINGLE_CHAR_WHITELIST,
        "origin unrecorded; provenance has to be established first",
        None,
    ),
    (OALDPE_WORDS, "extracted from a commercial dictionary", None),
];

/// Only these spellings turn the exclusions on: anything unrecognised has to land on the side that leaves unlicensed data out.
pub fn env_requests_unlicensed(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

pub fn describe_exclusions() -> Vec<String> {
    UNLICENSED_INPUTS
        .iter()
        .map(|(input, reason, fallback)| match fallback {
            Some(fallback) => format!("excluded {input}: {reason}, using {fallback} instead"),
            None => format!("excluded {input}: {reason}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_explicit_spellings_include_unlicensed_inputs() {
        for on in ["1", "true", "YES", " on "] {
            assert!(env_requests_unlicensed(Some(on)), "{on}");
        }
        for off in ["", "0", "no", "off", "FALSE", "tru"] {
            assert!(!env_requests_unlicensed(Some(off)), "{off}");
        }
        assert!(!env_requests_unlicensed(None));
    }
}
