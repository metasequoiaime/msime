//! Hotwords from the user's dictionary for on-device recognition.
//!
//! Models with native hotword support take the list as recognizer input. For the ones without (the catalog marks them `"hotwords": "pinyin"`), `correct` applies the same list to the final transcript: a run of Chinese characters that sounds like a hotword but is written differently is replaced by the hotword, which is how a name the model has never seen still comes out the way the user spells it.

use pinyin::ToPinyinMulti;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// How many hotwords a session carries by default. Recognizers slow down with long lists, and a user dictionary can hold thousands of words.
pub const DEFAULT_HOTWORD_LIMIT: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hotword {
    pub text: String,
    /// Toneless, lowercase syllables separated by single spaces, one per character (`ü` written `v`).
    pub pinyin: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DictionaryHotwordPage {
    pub entries: Vec<(String, String, i64)>,
    pub has_more: bool,
}

/// Read bounded dictionary pages and turn their weighted entries into hotwords.
pub fn hotwords_from_dictionary_pages<E>(
    limit: usize,
    page_size: usize,
    max_rows: usize,
    mut read_page: impl FnMut(usize, usize) -> Result<Option<DictionaryHotwordPage>, E>,
) -> Result<Vec<Hotword>, E> {
    let mut rows: Vec<(String, String, i64)> = Vec::with_capacity(max_rows);
    let mut offset = 0;
    while limit > 0 && offset < max_rows {
        let Some(page) = read_page(offset, page_size)? else {
            break;
        };
        let count = page.entries.len();
        rows.extend(page.entries);
        offset += count;
        if count == 0 || !page.has_more {
            break;
        }
    }
    rows.sort_by_key(|(_, _, weight)| std::cmp::Reverse(*weight));
    Ok(hotwords_from_entries(
        rows.iter()
            .map(|(text, pinyin, _)| (text.as_str(), pinyin.as_str())),
        limit,
    ))
}

/// Build the hotword list from `(text, stored pinyin)` dictionary entries, in the order given.
///
/// Only words of at least two characters, all of them Chinese, are kept: a single character matches far too much of any transcript, and other scripts have no pinyin to compare. The stored pinyin (`ni'hao`, `ni hao`, `ni3hao3`) is normalized; when it does not give one syllable per character it is derived from the text instead. Repeated texts are kept once, and at most `limit` words are returned.
pub fn hotwords_from_entries<'a>(
    entries: impl IntoIterator<Item = (&'a str, &'a str)>,
    limit: usize,
) -> Vec<Hotword> {
    let mut seen = HashSet::with_capacity(limit);
    let mut hotwords = Vec::with_capacity(limit);
    for (text, stored) in entries {
        if hotwords.len() >= limit {
            break;
        }
        let text = text.trim();
        let characters: Vec<char> = text.chars().collect();
        if characters.len() < 2 || !characters.iter().all(|&ch| is_han(ch)) {
            continue;
        }
        let syllables = match normalized_syllables(stored) {
            Some(syllables) if syllables.len() == characters.len() => syllables,
            _ => match characters
                .iter()
                .map(|&ch| readings(ch).into_iter().next())
                .collect::<Option<Vec<_>>>()
            {
                Some(syllables) => syllables,
                None => continue,
            },
        };
        if seen.insert(text.to_owned()) {
            hotwords.push(Hotword {
                text: text.to_owned(),
                pinyin: syllables.join(" "),
            });
        }
    }
    hotwords
}

/// Replace every run of Chinese characters that sounds like a hotword (fuzzy initials zh/z, ch/c, sh/s, n/l and finals an/ang, en/eng, in/ing treated alike) but is written differently with that hotword. Longer hotwords win, replacements never overlap, and text already spelled as a hotword is left alone.
pub fn correct(text: &str, hotwords: &[Hotword]) -> String {
    let mut prepared: Vec<(Vec<char>, Vec<String>)> = Vec::with_capacity(hotwords.len());
    prepared.extend(hotwords.iter().filter_map(|hotword| {
        let characters: Vec<char> = hotword.text.chars().collect();
        let syllables = normalized_syllables(&hotword.pinyin)?;
        (characters.len() >= 2
            && syllables.len() == characters.len()
            && characters.iter().all(|&ch| is_han(ch)))
        .then(|| {
            (
                characters,
                syllables.iter().map(|syllable| fuzzy(syllable)).collect(),
            )
        })
    }));
    if prepared.is_empty() {
        return text.to_owned();
    }
    // Stable, so hotwords of one length keep the dictionary's order.
    prepared.sort_by_key(|(characters, _)| std::cmp::Reverse(characters.len()));

    let characters: Vec<char> = text.chars().collect();
    let mut sounds: Vec<Option<Vec<String>>> = Vec::with_capacity(characters.len());
    sounds.extend(characters.iter().map(|&ch| {
        is_han(ch).then(|| readings(ch).iter().map(|reading| fuzzy(reading)).collect())
    }));
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    'outer: while index < characters.len() {
        if sounds[index].is_some() {
            for (word, keys) in &prepared {
                let end = index + word.len();
                if end > characters.len() {
                    continue;
                }
                let matches = keys.iter().enumerate().all(|(offset, key)| {
                    sounds[index + offset]
                        .as_ref()
                        .is_some_and(|options| options.contains(key))
                });
                if matches {
                    // Written exactly as the hotword or not, the window is consumed so a shorter hotword cannot rewrite part of it.
                    output.extend(word.iter());
                    index = end;
                    continue 'outer;
                }
            }
        }
        output.push(characters[index]);
        index += 1;
    }
    output
}

/// CJK unified ideographs, including the extension blocks and compatibility ideographs.
fn is_han(ch: char) -> bool {
    matches!(ch as u32,
        0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xF900..=0xFAFF
        | 0x20000..=0x2A6DF
        | 0x2A700..=0x2EBEF
        | 0x30000..=0x3134F)
}

/// Every toneless reading of one character, most common first.
fn readings(ch: char) -> Vec<String> {
    if let Some(multi) = ch.to_pinyin_multi() {
        let mut readings: Vec<String> = Vec::with_capacity(multi.count());
        for reading in multi {
            let reading = normalize_syllable(reading.plain());
            if !reading.is_empty() && !readings.contains(&reading) {
                readings.push(reading);
            }
        }
        readings
    } else {
        Vec::new()
    }
}

/// `Lü3` -> `lv`: lowercase, tone digits and marks dropped, `ü` written `v`.
fn normalize_syllable(syllable: &str) -> String {
    let mut out = String::with_capacity(syllable.len());
    for ch in syllable.chars().flat_map(char::to_lowercase) {
        match ch {
            'a'..='z' => out.push(ch),
            'ü' => out.push('v'),
            'ā' | 'á' | 'ǎ' | 'à' => out.push('a'),
            'ē' | 'é' | 'ě' | 'è' => out.push('e'),
            'ī' | 'í' | 'ǐ' | 'ì' => out.push('i'),
            'ō' | 'ó' | 'ǒ' | 'ò' => out.push('o'),
            'ū' | 'ú' | 'ǔ' | 'ù' => out.push('u'),
            'ǖ' | 'ǘ' | 'ǚ' | 'ǜ' => out.push('v'),
            _ => {}
        }
    }
    out
}

/// Split stored pinyin into normalized syllables on apostrophes, whitespace, hyphens and tone digits. `None` if anything is left that is not a syllable.
fn normalized_syllables(pinyin: &str) -> Option<Vec<String>> {
    let token_capacity = pinyin
        .chars()
        .filter(|ch| *ch == '\'' || *ch == '-' || ch.is_whitespace() || ch.is_ascii_digit())
        .count()
        .saturating_add(1);
    let mut syllables = Vec::with_capacity(token_capacity);
    let mut current = String::new();
    let mut chars = pinyin.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\'' || ch == '-' || ch.is_whitespace() || ch.is_ascii_digit() {
            if !current.is_empty() {
                syllables.push(normalize_syllable(&current));
                current.clear();
            }
        } else if ch == 'u' && chars.peek() == Some(&':') {
            chars.next();
            current.push('ü');
        } else if ch.is_alphabetic() {
            current.push(ch);
        } else {
            return None;
        }
    }
    if !current.is_empty() {
        syllables.push(normalize_syllable(&current));
    }
    (!syllables.is_empty() && syllables.iter().all(|syllable| !syllable.is_empty()))
        .then_some(syllables)
}

/// The comparison key for one syllable with the fuzzy pairs folded together.
fn fuzzy(syllable: &str) -> String {
    let (initial, rest) = if let Some(rest) = syllable.strip_prefix("zh") {
        ("z", rest)
    } else if let Some(rest) = syllable.strip_prefix("ch") {
        ("c", rest)
    } else if let Some(rest) = syllable.strip_prefix("sh") {
        ("s", rest)
    } else if let Some(rest) = syllable
        .strip_prefix('n')
        .filter(|rest| !rest.is_empty() && *rest != "g")
    {
        ("l", rest)
    } else {
        ("", syllable)
    };
    let rest = if let Some(stem) = rest.strip_suffix("ang") {
        format!("{stem}an")
    } else if let Some(stem) = rest.strip_suffix("eng") {
        format!("{stem}en")
    } else if let Some(stem) = rest.strip_suffix("ing") {
        format!("{stem}in")
    } else {
        rest.to_owned()
    };
    format!("{initial}{rest}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hotword(text: &str, pinyin: &str) -> Hotword {
        Hotword {
            text: text.into(),
            pinyin: pinyin.into(),
        }
    }

    #[test]
    fn a_homophone_is_replaced_by_the_hotword() {
        let hotwords = [hotword("明天科技", "ming tian ke ji")];
        assert_eq!(correct("我在名天科技上班", &hotwords), "我在明天科技上班");
    }

    #[test]
    fn fuzzy_initials_and_finals_match() {
        // zh/z, sh/s, n/l, in/ing, en/eng, an/ang.
        let hotwords = [
            hotword("张三", "zhang san"),
            hotword("牛郎", "niu lang"),
            hotword("心声", "xin sheng"),
        ];
        assert_eq!(correct("找赞散来了", &hotwords), "找张三来了");
        assert_eq!(correct("刘郎织女", &hotwords), "牛郎织女");
        assert_eq!(correct("星神", &hotwords), "心声");
    }

    #[test]
    fn longest_hotword_wins_and_replacements_do_not_overlap() {
        let hotwords = [
            hotword("天科", "tian ke"),
            hotword("明天科技", "ming tian ke ji"),
        ];
        assert_eq!(correct("名天科技", &hotwords), "明天科技");
        // Already written as the longer hotword: the shorter one must not rewrite part of it.
        let hotwords = [
            hotword("田可", "tian ke"),
            hotword("明天科技", "ming tian ke ji"),
        ];
        assert_eq!(correct("明天科技和天科", &hotwords), "明天科技和田可");
    }

    #[test]
    fn non_chinese_text_and_single_characters_are_left_alone() {
        let hotwords = [hotword("张", "zhang"), hotword("张三", "zhang san")];
        assert_eq!(correct("章 hello 三", &hotwords), "章 hello 三");
        assert_eq!(correct("", &hotwords), "");
        assert_eq!(correct("章", &hotwords), "章");
        assert_eq!(correct("彰散。", &hotwords), "张三。");
    }

    #[test]
    fn a_polyphonic_character_matches_any_of_its_readings() {
        // 长 reads chang and zhang.
        let hotwords = [hotword("张江", "zhang jiang")];
        assert_eq!(correct("长江", &hotwords), "张江");
    }

    #[test]
    fn entries_are_normalized_filtered_deduplicated_and_capped() {
        let entries = [
            ("明天科技", "ming'tian'ke'ji"),
            ("张三", "Zhang3 San1"),
            ("张三", "zhang'san"),
            ("女儿", "nü'er"),
            ("绿色", "lu:'se"),
            ("我", "wo"),
            ("GitHub", "github"),
            ("A股", "a'gu"),
            // Stored pinyin that does not fit the word is derived from the text instead.
            ("北京", "beijing"),
        ];
        let hotwords = hotwords_from_entries(entries.iter().map(|(t, p)| (*t, *p)), 200);
        assert_eq!(
            hotwords,
            vec![
                hotword("明天科技", "ming tian ke ji"),
                hotword("张三", "zhang san"),
                hotword("女儿", "nv er"),
                hotword("绿色", "lv se"),
                hotword("北京", "bei jing"),
            ]
        );
        let capped = hotwords_from_entries(entries.iter().map(|(t, p)| (*t, *p)), 2);
        assert_eq!(capped.len(), 2);
    }

    #[test]
    fn normalized_syllables_reserve_one_slot_per_token_boundary() {
        let syllables = normalized_syllables("a b c d e").unwrap();
        assert_eq!(syllables, ["a", "b", "c", "d", "e"]);
        assert_eq!(syllables.capacity(), 5);
    }

    #[test]
    fn fuzzy_keys_fold_the_listed_pairs_only() {
        assert_eq!(fuzzy("zhang"), fuzzy("zan"));
        assert_eq!(fuzzy("ning"), fuzzy("lin"));
        assert_eq!(fuzzy("sheng"), fuzzy("sen"));
        assert_ne!(fuzzy("zhang"), fuzzy("jiang"));
        assert_ne!(fuzzy("fan"), fuzzy("huan"));
        assert_eq!(fuzzy("ng"), "ng");
    }
}
