//! `normalize_full_pinyin`, the import normaliser the host facade exposes (api-contract §1b, bridge.cpp:777-821).

use super::graph::{
    build_syllable_graph, enumerate_complete_segmentations, SYLLABLE_GRAPH_PATH_LIMIT,
};
use super::segment::{cut_pinyin_by_mode, join_segments, split_segments, CutMode};
use super::syllables::is_intact;

/// Strip whitespace and lowercase; split on `'` or take the correction cut, preferring an enumerated complete segmentation with exactly `expected_syllables` syllables when that is non-zero. Returns "" for anything unusable: a leading, trailing or doubled `'`, a syllable-count mismatch, an incomplete syllable, or a result that does not spell the source.
pub fn normalize_full_pinyin(input: &str, expected_syllables: usize) -> String {
    // C `isspace` also counts the vertical tab, which `char::is_ascii_whitespace` does not.
    let mut source = String::with_capacity(input.len());
    source.extend(
        input
            .chars()
            .filter(|&character| !(character.is_ascii_whitespace() || character == '\x0b'))
            .map(|character| character.to_ascii_lowercase()),
    );
    if source.is_empty()
        || source.starts_with('\'')
        || source.ends_with('\'')
        || source.contains("''")
    {
        return String::new();
    }
    let segments = if source.contains('\'') {
        split_segments(&source)
    } else {
        let Some(mut segments) = cut_pinyin_by_mode(&source, CutMode::Correction)
            .into_iter()
            .next()
        else {
            return String::new();
        };
        if expected_syllables != 0 && segments.len() != expected_syllables {
            if let Some(matching) = enumerate_complete_segmentations(
                &build_syllable_graph(&source),
                SYLLABLE_GRAPH_PATH_LIMIT,
            )
            .into_iter()
            .find(|cut| cut.len() == expected_syllables)
            {
                segments = matching;
            }
        }
        segments
    };
    if expected_syllables != 0 && segments.len() != expected_syllables {
        return String::new();
    }
    if segments.is_empty() || !segments.iter().all(|segment| is_intact(segment)) {
        return String::new();
    }
    let normalized = join_segments(&segments);
    // An alias cut (`laing` -> `liang`) changes the letters, and an import must keep what the user wrote.
    let same_letters = normalized
        .bytes()
        .filter(|&byte| byte != b'\'')
        .eq(source.bytes().filter(|&byte| byte != b'\''));
    if same_letters {
        normalized
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // engine-bridge/src/tests.rs:8-17.
    #[test]
    fn normalizes_full_pinyin_using_the_expected_word_length() {
        assert_eq!(normalize_full_pinyin("xian", 1), "xian");
        assert_eq!(normalize_full_pinyin("xian", 2), "xi'an");
        assert_eq!(
            normalize_full_pinyin("a'ba'la'ti'ya'yun'hai", 7),
            "a'ba'la'ti'ya'yun'hai"
        );
        assert_eq!(normalize_full_pinyin("xian", 3), "");
        assert_eq!(normalize_full_pinyin("ni'hao'", 2), "");
    }

    #[test]
    fn rejects_what_it_cannot_import_unchanged() {
        assert_eq!(normalize_full_pinyin(" NI HAO\t", 0), "ni'hao");
        assert_eq!(normalize_full_pinyin("ni\x0bhao", 2), "ni'hao");
        assert_eq!(normalize_full_pinyin("'nihao", 0), "");
        assert_eq!(normalize_full_pinyin("ni''hao", 0), "");
        assert_eq!(normalize_full_pinyin("ni'h", 0), "");
        assert_eq!(normalize_full_pinyin("nihz", 0), "");
        // The alias reading respells the input.
        assert_eq!(normalize_full_pinyin("laing", 0), "");
        assert_eq!(normalize_full_pinyin("", 0), "");
        assert_eq!(normalize_full_pinyin("fangan", 2), "fan'gan");
        assert_eq!(normalize_full_pinyin("fang'an", 2), "fang'an");
    }
}
