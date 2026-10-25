//! 英文候选与英文释义的统一读音，供宿主绘制在释义旁。
//!
//! `scripts/build_pronunciations.py` 从 ECDICT 构建 `pronunciations/en-phonetic.db`。
//! 优先读取资源目录旁的表，缺失时使用宿主显式传入的随包补充目录；底层查询复用
//! `english_phonetics`。释义只读首个词组，去掉不定式 `to` 和冠词；任一词未知时
//! 整组不显示音标，避免把不完整的读音当作完整读音。

use std::path::{Path, PathBuf};

/// A gloss term longer than this many words is a definition, not a word to say.
pub(crate) const MAX_WORDS: usize = 4;
const MAX_WORD_BYTES: usize = 40;

/// 活动资源旁的音标表优先；只在缺失时使用宿主显式给出的补充目录。
pub(crate) fn english_database_beside(
    resources: &Path,
    supplementary_resources: Option<&Path>,
) -> Option<PathBuf> {
    [
        resources
            .parent()
            .map(|root| root.join("pronunciations").join("en-phonetic.db")),
        supplementary_resources.map(|root| root.join("pronunciations").join("en-phonetic.db")),
    ]
    .into_iter()
    .flatten()
    .find(|path| path.is_file())
}

/// The lowercase words that pronounce `text`, or `None` when it is not plain English.
///
/// `text` is either an English candidate (`"hello"`) or one line of an English gloss
/// (`"love; affection, fondness"`). Only the first term counts, and a parenthesised note inside
/// it is dropped, so `"(of a person) kind"` is pronounced as `kind`.
pub(crate) fn english_words(text: &str) -> Option<Vec<String>> {
    let first = text
        .split([';', '；', ',', '，', '/', '?', '？'])
        .next()
        .unwrap_or_default();
    let mut plain = String::with_capacity(first.len());
    let mut depth = 0_usize;
    for character in first.chars() {
        match character {
            '(' | '（' => depth += 1,
            ')' | '）' => depth = depth.checked_sub(1)?,
            _ if depth == 0 => plain.push(character),
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    let mut words = plain
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    // "to love" is a verb gloss and "a cat" an article the learner already knows; neither is
    // what the line is teaching. A lone "to" or "a" is itself the word, so it stays.
    if words.len() > 1 && matches!(words[0].as_str(), "to" | "a" | "an" | "the") {
        words.remove(0);
    }
    let usable = |word: &String| {
        !word.is_empty()
            && word.len() <= MAX_WORD_BYTES
            && word.starts_with(|character: char| character.is_ascii_alphabetic())
            && word.ends_with(|character: char| character.is_ascii_alphabetic())
            && word
                .chars()
                .all(|character| character.is_ascii_alphabetic() || matches!(character, '-' | '\''))
    };
    (!words.is_empty() && words.len() <= MAX_WORDS && words.iter().all(usable)).then_some(words)
}

/// The displayed pronunciation of each text, parallel to `texts`; empty when there is none.
///
/// Words are looked up once each, however many texts share them. The result is wrapped in
/// slashes, the IPA convention for a broad transcription.
pub(crate) fn english_pronunciations(
    database: &str,
    texts: &[String],
) -> Result<Vec<String>, String> {
    let terms = texts
        .iter()
        .map(|text| english_words(text))
        .collect::<Vec<_>>();
    let mut unique = terms
        .iter()
        .flatten()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    unique.sort();
    unique.dedup();
    let phonetics = msime_engine::host::english_phonetics(database, &unique)
        .map_err(|_| "pronunciation dictionary unavailable".to_owned())?;
    if phonetics.len() != unique.len() {
        return Err("pronunciation dictionary response mismatch".into());
    }
    let known = unique
        .iter()
        .zip(&phonetics)
        .filter(|(_, phonetic)| !phonetic.is_empty())
        .collect::<std::collections::HashMap<_, _>>();
    Ok(terms
        .into_iter()
        .map(|words| {
            words
                .and_then(|words| {
                    words
                        .iter()
                        .map(|word| known.get(word).map(|phonetic| phonetic.as_str()))
                        .collect::<Option<Vec<_>>>()
                })
                .map(|parts| format!("/{}/", parts.join(" ")))
                .unwrap_or_default()
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Option<String> {
        english_words(text).map(|words| words.join(" "))
    }

    #[test]
    fn the_first_term_of_a_gloss_line_is_pronounced() {
        assert_eq!(words("love; affection"), Some("love".into()));
        assert_eq!(words("hello, hi"), Some("hello".into()));
        assert_eq!(
            words("Computer Science; informatics"),
            Some("computer science".into())
        );
        assert_eq!(words("  sky  "), Some("sky".into()));
        assert_eq!(words("well-known"), Some("well-known".into()));
        assert_eq!(words("don't"), Some("don't".into()));
        // Unihan writes question words as "who? whom? whose?".
        assert_eq!(words("who? whom? whose? anyone?"), Some("who".into()));
    }

    #[test]
    fn infinitives_articles_and_notes_are_not_what_the_line_teaches() {
        assert_eq!(words("to love; to be fond of"), Some("love".into()));
        assert_eq!(words("a cat"), Some("cat".into()));
        assert_eq!(words("(of a person) kind"), Some("kind".into()));
        assert_eq!(words("to"), Some("to".into()));
        assert_eq!(words("a"), Some("a".into()));
    }

    #[test]
    fn text_that_is_not_plain_english_is_not_pronounced() {
        assert_eq!(words(""), None);
        assert_eq!(words("爱"), None);
        assert_eq!(words("café"), None);
        assert_eq!(words("C++"), None);
        assert_eq!(words("3D"), None);
        assert_eq!(words("-ing"), None);
        assert_eq!(words("(unbalanced"), None);
        assert_eq!(words("unbalanced)"), None);
        assert_eq!(words("one two three four five"), None);
        assert_eq!(words("(only a note)"), None);
    }
}
