//! 移动端英文整词纠错与连写分词；只读索引查询，缓存和搜索长度都有上限。

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use super::EnglishDictionary;
struct Lexeme {
    key: String,
    display: String,
    weight: i64,
}

const MAX_INPUT: usize = 32;
const MAX_EDITS: usize = 2;
const MAX_WORDS: usize = 6;
const PREFIX_ROWS: usize = 128;
const CACHE_PAGES: usize = 128;

#[derive(Default)]
pub(super) struct CorrectionCache {
    pages: HashMap<String, Arc<Vec<Lexeme>>>,
    order: VecDeque<String>,
}

#[derive(Clone, Default)]
struct Path {
    text: String,
    edits: usize,
    words: usize,
    cost: f64,
}

impl Path {
    fn better_than(&self, other: &Self) -> bool {
        self.cost < other.cost
            || (self.cost == other.cost
                && ((self.edits, self.words) < (other.edits, other.words)
                    || ((self.edits, self.words) == (other.edits, other.words)
                        && self.text < other.text)))
    }
}

impl EnglishDictionary {
    /// 返回与原文不同的唯一建议；未知词、标识符、超长输入和缺失词库都允许原样输入。
    pub(crate) fn correct_mobile_english(&self, raw: &str) -> Option<String> {
        if raw.is_empty()
            || raw.len() > MAX_INPUT
            || !raw
                .bytes()
                .all(|byte| byte.is_ascii_alphabetic() || byte == b'\'')
        {
            return None;
        }
        // 显式大小写也优先于固定词形恢复，避免把 IM、ENGLISH 等自动改写。
        if raw.bytes().skip(1).any(|byte| byte.is_ascii_uppercase()) {
            return None;
        }
        let input = raw.to_ascii_lowercase();
        if protected(&input) {
            return None;
        }
        let capitalize = |mut text: String| {
            if raw.as_bytes()[0].is_ascii_uppercase() {
                text[..1].make_ascii_uppercase();
            }
            (text != raw).then_some(text)
        };
        if let Some(display) = canonical(&input.replace('\'', "")) {
            if !input.contains('\'') || display.eq_ignore_ascii_case(&input) {
                return capitalize(display.to_owned());
            }
        }
        if !self.ready() || input.contains('\'') {
            return None;
        }
        if let Some(item) = self.exact_correction_word(&input) {
            return capitalize(item.display);
        }
        {
            let mut paths =
                vec![vec![None::<Path>; (MAX_EDITS + 1) * (MAX_WORDS + 1)]; input.len() + 1];
            paths[0][0] = Some(Path::default());
            for start in 0..input.len() {
                if paths[start].iter().all(Option::is_none) {
                    continue;
                }
                let remaining = &input[start..];
                let mut choices = Vec::new();
                for end in 1..=remaining.len() {
                    let piece = &remaining[..end];
                    if let Some(display) = canonical(piece) {
                        choices.push((end, display.to_owned(), 0, 1_000_000_000_i64));
                    } else if let Some(item) = self.exact_correction_word(piece) {
                        choices.push((end, item.display, 0, item.weight));
                    }
                }
                if remaining.len() >= 3
                    && paths[start]
                        .iter()
                        .flatten()
                        .any(|path| path.edits < MAX_EDITS && path.words < MAX_WORDS)
                {
                    let mut prefixes = vec![remaining[..2].to_owned()];
                    let primary = self.correction_matches(remaining, &prefixes);
                    if primary.is_empty() {
                        prefixes = alternate_prefixes(remaining);
                        choices.extend(self.correction_matches(remaining, &prefixes));
                    } else {
                        choices.extend(primary);
                    }
                }
                for path in paths[start]
                    .clone()
                    .into_iter()
                    .flatten()
                    .filter(|path| path.words < MAX_WORDS)
                {
                    for (consumed, display, edits, weight) in &choices {
                        let next = Path {
                            text: if path.text.is_empty() {
                                display.clone()
                            } else {
                                format!("{} {display}", path.text)
                            },
                            edits: path.edits + edits,
                            words: path.words + 1,
                            cost: path.cost + 24.0 - ((*weight).max(0) as f64 + 1.0).ln().min(22.0)
                                + *edits as f64 * 8.0,
                        };
                        if next.edits > MAX_EDITS {
                            continue;
                        }
                        let slot = next.edits * (MAX_WORDS + 1) + next.words;
                        let destination = &mut paths[start + consumed][slot];
                        if destination.as_ref().is_none_or(|old| next.better_than(old)) {
                            *destination = Some(next);
                        }
                    }
                }
            }
            if let Some(path) = paths.pop().unwrap().into_iter().flatten().reduce(|a, b| {
                if a.better_than(&b) {
                    a
                } else {
                    b
                }
            }) {
                return capitalize(path.text);
            }
        }
        None
    }

    fn exact_correction_word(&self, word: &str) -> Option<Lexeme> {
        if word.len() < 2 {
            return None;
        }
        let connection = self.connection.as_ref()?;
        let mut statement = connection.prepare_cached(
            "SELECT display,weight FROM english_words WHERE word=?1 ORDER BY weight DESC,display LIMIT 1",
        ).ok()?;
        statement
            .query_row([word], |row| {
                Ok(Lexeme {
                    key: word.to_owned(),
                    display: row.get(0)?,
                    weight: row.get(1)?,
                })
            })
            .ok()
            .filter(|item| valid_display(&item.display))
    }

    fn correction_page(&self, prefix: &str) -> Arc<Vec<Lexeme>> {
        if let Some(page) = self.correction_cache.borrow().pages.get(prefix) {
            return Arc::clone(page);
        }
        let page = Arc::new(
            self.query_prefix(prefix, PREFIX_ROWS)
                .into_iter()
                .map(|item| Lexeme {
                    key: item.pinyin.into_string(),
                    display: item.word.into_string(),
                    weight: item.weight,
                })
                .collect::<Vec<_>>(),
        );
        let mut cache = self.correction_cache.borrow_mut();
        if cache.pages.len() == CACHE_PAGES {
            if let Some(key) = cache.order.pop_front() {
                cache.pages.remove(&key);
            }
        }
        cache.order.push_back(prefix.to_owned());
        cache.pages.insert(prefix.to_owned(), Arc::clone(&page));
        page
    }

    fn correction_matches(
        &self,
        remaining: &str,
        prefixes: &[String],
    ) -> Vec<(usize, String, usize, i64)> {
        let mut matches = Vec::new();
        for prefix in prefixes {
            for item in self.correction_page(prefix).iter() {
                if item.key.len() < 4 || !valid_display(&item.display) {
                    continue;
                }
                let length = item.key.len();
                for consumed in length.saturating_sub(1)..=length + 1 {
                    if consumed > remaining.len() || consumed < 3 {
                        continue;
                    }
                    if one_edit(&remaining.as_bytes()[..consumed], item.key.as_bytes()) {
                        let display = canonical(&item.key).unwrap_or(&item.display).to_owned();
                        matches.push((consumed, display, 1, item.weight));
                    }
                }
            }
        }
        matches
    }
}

fn valid_display(display: &str) -> bool {
    !display.is_empty()
        && display
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || byte == b'\'')
}

fn protected(word: &str) -> bool {
    matches!(
        word,
        "omg"
            | "thx"
            | "lol"
            | "lmao"
            | "rofl"
            | "brb"
            | "btw"
            | "idk"
            | "imo"
            | "imho"
            | "tbh"
            | "wfh"
            | "fyi"
            | "afk"
            | "gg"
            | "ok"
    )
}

fn canonical(word: &str) -> Option<&'static str> {
    Some(match word {
        "a" => "a",
        "i" => "I",
        "im" => "I'm",
        "ive" => "I've",
        "youre" => "you're",
        "youve" => "you've",
        "youll" => "you'll",
        "hes" => "he's",
        "shes" => "she's",
        "theyre" => "they're",
        "theyve" => "they've",
        "weve" => "we've",
        "arent" => "aren't",
        "isnt" => "isn't",
        "wasnt" => "wasn't",
        "werent" => "weren't",
        "dont" => "don't",
        "doesnt" => "doesn't",
        "didnt" => "didn't",
        "cant" => "can't",
        "couldnt" => "couldn't",
        "shouldnt" => "shouldn't",
        "wouldnt" => "wouldn't",
        "chinese" => "Chinese",
        "english" => "English",
        _ => return None,
    })
}

fn neighbors(a: u8, b: u8) -> bool {
    fn position(byte: u8) -> Option<(i32, i32)> {
        for (row, (letters, offset)) in [
            (b"qwertyuiop".as_slice(), 0),
            (b"asdfghjkl".as_slice(), 1),
            (b"zxcvbnm".as_slice(), 2),
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(column) = letters.iter().position(|&key| key == byte) {
                return Some((column as i32 * 4 + offset, row as i32));
            }
        }
        None
    }
    match (position(a), position(b)) {
        (Some((ax, ay)), Some((bx, by))) => (ax - bx).abs() <= 4 && (ay - by).abs() <= 1,
        _ => false,
    }
}

fn one_edit(input: &[u8], word: &[u8]) -> bool {
    let common = input.iter().zip(word).take_while(|(a, b)| a == b).count();
    if input.len() == word.len() {
        common < input.len()
            && ((neighbors(input[common], word[common])
                && input[common + 1..] == word[common + 1..])
                || (common + 1 < input.len()
                    && input[common] == word[common + 1]
                    && input[common + 1] == word[common]
                    && input[common + 2..] == word[common + 2..]))
    } else if input.len() == word.len() + 1 {
        input[common + 1..] == word[common..]
    } else if word.len() == input.len() + 1 {
        input[common..] == word[common + 1..]
    } else {
        false
    }
}

fn alternate_prefixes(input: &str) -> Vec<String> {
    let bytes = input.as_bytes();
    let mut prefixes = Vec::new();
    for key in b'a'..=b'z' {
        for prefix in [[key, bytes[0]], [bytes[0], key]] {
            prefixes.push(String::from_utf8(prefix.to_vec()).expect("ASCII"));
        }
        if neighbors(key, bytes[0]) {
            prefixes.push(String::from_utf8(vec![key, bytes[1]]).expect("ASCII"));
        }
        if neighbors(key, bytes[1]) {
            prefixes.push(String::from_utf8(vec![bytes[0], key]).expect("ASCII"));
        }
    }
    prefixes.push(input[1..3].to_owned());
    prefixes.push(String::from_utf8(vec![bytes[0], bytes[2]]).expect("ASCII"));
    prefixes.push(String::from_utf8(vec![bytes[1], bytes[0]]).expect("ASCII"));
    prefixes.sort_unstable();
    prefixes.dedup();
    prefixes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::english::ensure_english_schema;
    use rusqlite::Connection;

    fn dictionary() -> (tempfile::TempDir, EnglishDictionary) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("english.db");
        ensure_english_schema(&path).unwrap();
        let connection = Connection::open(&path).unwrap();
        for word in [
            "what", "is", "that", "language", "speak", "hello", "world", "english", "chinese",
        ] {
            connection
                .execute(
                    "INSERT INTO english_words VALUES (?1,?1,5000000000)",
                    [word],
                )
                .unwrap();
        }
        let dictionary = EnglishDictionary::open(&path, None, None);
        (root, dictionary)
    }

    #[test]
    fn mobile_english_issue_examples() {
        let (_root, dictionary) = dictionary();
        for (input, expected) in [
            ("whatisthat", "what is that"),
            ("languaage", "language"),
            ("languge", "language"),
            ("langiage", "language"),
            ("i", "I"),
            ("i'm", "I'm"),
            ("chinese", "Chinese"),
            ("english", "English"),
            ("youre", "you're"),
            ("hes", "he's"),
            ("imchinese", "I'm Chinese"),
            ("speskebglish", "speak English"),
            ("Whatisthat", "What is that"),
        ] {
            assert_eq!(
                dictionary.correct_mobile_english(input).as_deref(),
                Some(expected),
                "{input}"
            );
        }
    }

    #[test]
    fn mobile_english_preserves_words_abbreviations_and_identifiers() {
        let (_root, dictionary) = dictionary();
        for input in [
            "language",
            "hello",
            "Hello",
            "omg",
            "OMG",
            "thx",
            "iPhone",
            "xqzv",
            "hello_world",
            "é",
            "abcdefghijklmnopqrstuvwxyzabcdefg",
        ] {
            assert_eq!(dictionary.correct_mobile_english(input), None, "{input}");
        }
    }

    #[test]
    fn mobile_english_preserves_explicit_case_before_canonical_spelling() {
        let (_root, dictionary) = dictionary();
        for input in [
            "IM", "I'M", "I'VE", "ENGLISH", "CHINESE", "eNglish", "yOuRe",
        ] {
            assert_eq!(dictionary.correct_mobile_english(input), None, "{input}");
        }
        for (input, expected) in [("Im", "I'm"), ("Youre", "You're"), ("English", "English")] {
            assert_eq!(
                dictionary.correct_mobile_english(input).as_deref(),
                (input != expected).then_some(expected),
                "{input}"
            );
        }
    }

    #[test]
    fn mobile_english_first_keys_and_transposition_can_be_corrected() {
        let (_root, dictionary) = dictionary();
        for input in ["kanguage", "alnguage", "anguage", "lnaguage", "languagge"] {
            assert_eq!(
                dictionary.correct_mobile_english(input).as_deref(),
                Some("language"),
                "{input}"
            );
        }
    }

    #[test]
    fn mobile_english_missing_dictionary_and_cache_are_bounded() {
        let dictionary = EnglishDictionary::open(std::path::Path::new(""), None, None);
        assert_eq!(dictionary.correct_mobile_english("languaage"), None);
        assert_eq!(dictionary.correct_mobile_english("i").as_deref(), Some("I"));
        for first in b'a'..=b'z' {
            for second in b'a'..=b'z' {
                dictionary.correction_page(std::str::from_utf8(&[first, second]).unwrap());
            }
        }
        let cache = dictionary.correction_cache.borrow();
        assert_eq!(cache.pages.len(), CACHE_PAGES);
        assert_eq!(cache.order.len(), CACHE_PAGES);
    }

    #[test]
    #[ignore = "需要用 MSIME_ENGLISH_CORRECTION_TEST_DB 指定固定的真实英文词库"]
    fn mobile_english_shipped_dictionary() {
        let path = std::env::var_os("MSIME_ENGLISH_CORRECTION_TEST_DB").expect("真实词库路径");
        let dictionary = EnglishDictionary::open(std::path::Path::new(&path), None, None);
        assert!(dictionary.ready());
        for (input, expected) in [
            ("whatisthat", "what is that"),
            ("languaage", "language"),
            ("languge", "language"),
            ("langiage", "language"),
            ("imchinese", "I'm Chinese"),
            ("speskebglish", "speak English"),
        ] {
            let started = std::time::Instant::now();
            assert_eq!(
                dictionary.correct_mobile_english(input).as_deref(),
                Some(expected),
                "{input}"
            );
            eprintln!("{input}: {:?}", started.elapsed());
        }
    }
}
