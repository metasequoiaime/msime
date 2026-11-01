//! 候选释义行的统一读音，供宿主绘制在释义下面。
//!
//! 英文：`scripts/build_pronunciations.py` 从 ECDICT 构建 `pronunciations/en-phonetic.db`。释义只读首个词组，去掉不定式 `to` 和冠词；任一词未知时整组不显示音标，避免把不完整的读音当作完整读音。
//!
//! 法、日、西、俄、德、韩：`scripts/build_target_pronunciations.py` 从维基词典（与离线释义同一来源）构建 `target-pronunciations/<语言>-phonetic.db`。释义行只读第一个词条，整条查表，查不到就不显示：这几种语言逐词拼读会出错（法语的联诵、俄语的重音随词形变化），所以不像英文那样逐词拼起来。日文给赫本式罗马字，韩文给文化观光部式罗马字（Revised Romanization，生成器按规范从谚文生成），其余给维基词典写的 IPA，连同它的斜线或方括号（德语优先取方括号的语音标音）。
//!
//! 两种表都优先读取资源目录旁的，缺失时使用宿主显式传入的补充目录（资源包的根）。

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

/// 非英文读音表所在的目录名：随包时在资源目录旁，下载时是资源包 `target-pronunciations` 的安装目录。
pub(crate) const TARGET_DIRECTORY: &str = "target-pronunciations";
/// 非英文释义词条超过这么多个字符就是哪张表都没有的短语。
const MAX_TARGET_TERM_CHARS: usize = 40;

/// `language` 的读音表：活动资源旁的优先，只在缺失时使用补充目录。`language` 必须是 [`msime_engine::host::PHONETIC_LANGUAGES`] 里英文以外的一种。
pub(crate) fn target_database_beside(
    resources: &Path,
    supplementary_resources: Option<&Path>,
    language: &str,
) -> Option<PathBuf> {
    let name = format!("{language}-phonetic.db");
    [
        resources
            .parent()
            .map(|root| root.join(TARGET_DIRECTORY).join(&name)),
        supplementary_resources.map(|root| root.join(TARGET_DIRECTORY).join(&name)),
    ]
    .into_iter()
    .flatten()
    .find(|path| path.is_file())
}

/// 一行非英文释义所教的词条：第一个分隔符前的部分，去掉括号里的注释，去掉首尾空白并把连续空白合成一个空格。什么也不剩或长得不像一个词时为 `None`。
///
/// `"chat, chatte; félin"` 是 `chat`，`"(zoologie) chat"` 是 `chat`，`"pomme de terre"` 整条保留。
pub(crate) fn target_term(text: &str) -> Option<String> {
    let first = text
        .split([';', '；', ',', '，', '、', '/', '?', '？'])
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
    let term = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    (!term.is_empty() && term.chars().count() <= MAX_TARGET_TERM_CHARS).then_some(term)
}

/// 每一行非英文释义要显示的读音，与 `texts` 一一对应；没有读音时为空串。
///
/// 表的键是维基词典的写法（`Katze`、`кот`），所以词条先按原样查，再按小写查，照顾句首大写的行。不论多少行共用一个键，每个键只查一次。读音按表里存的原样显示。
pub(crate) fn target_pronunciations(
    database: &str,
    language: &str,
    texts: &[String],
) -> Result<Vec<String>, String> {
    let terms = texts
        .iter()
        .map(|text| target_term(text))
        .collect::<Vec<_>>();
    let mut unique = terms
        .iter()
        .flatten()
        .flat_map(|term| [term.clone(), term.to_lowercase()])
        .collect::<Vec<_>>();
    unique.sort();
    unique.dedup();
    let readings = msime_engine::host::phonetics(database, language, &unique)
        .map_err(|_| "pronunciation dictionary unavailable".to_owned())?;
    if readings.len() != unique.len() {
        return Err("pronunciation dictionary response mismatch".into());
    }
    let known = unique
        .iter()
        .zip(&readings)
        .filter(|(_, reading)| !reading.is_empty())
        .collect::<std::collections::HashMap<_, _>>();
    Ok(terms
        .into_iter()
        .map(|term| {
            term.and_then(|term| {
                known
                    .get(&term)
                    .or_else(|| known.get(&term.to_lowercase()))
                    .map(|reading| (*reading).clone())
            })
            .unwrap_or_default()
        })
        .collect())
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
    fn the_first_term_of_a_target_gloss_line_is_read_whole() {
        assert_eq!(target_term("chat, chatte; félin"), Some("chat".into()));
        assert_eq!(target_term("pomme de terre"), Some("pomme de terre".into()));
        assert_eq!(target_term("(zoologie) chat"), Some("chat".into()));
        assert_eq!(target_term("  Katze ,Kater"), Some("Katze".into()));
        assert_eq!(target_term("じゃがいも, 馬鈴薯"), Some("じゃがいも".into()));
        assert_eq!(target_term("習う、学ぶ"), Some("習う".into()));
        assert_eq!(
            target_term("사랑하다,  애정을 품다"),
            Some("사랑하다".into())
        );
        assert_eq!(
            target_term("estación  de tren"),
            Some("estación de tren".into())
        );
        assert_eq!(target_term(""), None);
        assert_eq!(target_term("; chat"), None);
        assert_eq!(target_term("(note seule)"), None);
        assert_eq!(target_term("(déséquilibré"), None);
        assert_eq!(target_term(&"a".repeat(41)), None);
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
