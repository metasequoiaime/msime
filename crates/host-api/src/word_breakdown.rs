//! A word-by-word English breakdown of a Chinese candidate no dictionary has as a whole — typically a sentence the
//! Engine composed, such as 我喜欢你.
//!
//! The candidate is cut left to right by longest match against the same local dictionaries the glosses use — Unihan
//! for single characters, CC-CEDICT for everything — and each piece shows the first phrase of its gloss:
//! `我 I · 喜欢 to like · 你 you`. It is not a translation; it lines the words of the sentence up with English ones,
//! which is what a learner reading the candidate wants, and it needs nothing but the tables already installed. Hosts
//! draw it on its own line under the gloss lines, never as a committable gloss column.
//!
//! A dictionary's first phrase is the sense it lists first, not the one a sentence means: Unihan leads 是 with
//! "indeed", 要 with "necessary" and 和 with "harmony", and CC-CEDICT leads 一起 with "in the same place". The
//! candidate glosses keep the full definition, so a reader sees every sense there; a breakdown shows one phrase per
//! word, so for the words that make up most sentences `LEARNER_PHRASES` names the sense a learner needs.

use std::collections::HashMap;
use std::path::Path;

/// The longest piece tried at one position; longer CC-CEDICT entries are idioms a sentence rarely contains whole.
const MAX_WORD_CHARS: usize = 8;
/// A breakdown past this many pieces is too long to read beside a candidate.
pub(crate) const MAX_PIECES: usize = 8;
/// Candidates longer than this are not broken down at all.
pub(crate) const MAX_CANDIDATE_CHARS: usize = 32;

/// One phrase per frequent word or particle, used in the breakdown instead of the dictionary's first phrase.
///
/// Only words whose first dictionary phrase misleads in a sentence are listed; 很 (very), 学生 (student) and the like
/// need no entry. A grammatical particle gets a parenthesised role, which `first_phrase` keeps whole, in the same
/// style CC-CEDICT uses for 的. Add to it by word, with the sense a sentence almost always means.
const LEARNER_PHRASES: &[(&str, &str)] = &[
    // pronouns and question words
    ("他", "he"),
    ("哪", "which"),
    ("哪儿", "where"),
    ("哪里", "where"),
    ("谁", "who"),
    ("什么", "what"),
    ("怎么", "how"),
    ("为什么", "why"),
    ("多少", "how many"),
    ("那个", "that"),
    ("这样", "like this"),
    ("那样", "like that"),
    ("个", "(measure word)"),
    // verbs
    ("是", "to be"),
    ("去", "to go"),
    ("来", "to come"),
    ("有", "to have"),
    ("没有", "not have"),
    ("做", "to do"),
    ("看", "to look"),
    ("说", "to say"),
    ("想", "to want"),
    ("要", "to want"),
    ("会", "can"),
    ("能", "can"),
    ("给", "to give"),
    ("让", "to let"),
    ("在", "at"),
    ("到", "to arrive"),
    ("用", "to use"),
    ("吃", "to eat"),
    ("喝", "to drink"),
    ("爱", "to love"),
    ("走", "to walk"),
    ("回", "to return"),
    ("进", "to enter"),
    ("买", "to buy"),
    ("卖", "to sell"),
    ("找", "to look for"),
    ("学", "to study"),
    ("打", "to hit"),
    ("开", "to open"),
    ("关", "to close"),
    ("住", "to live"),
    ("坐", "to sit"),
    ("站", "to stand"),
    ("听", "to listen"),
    ("写", "to write"),
    ("读", "to read"),
    ("玩", "to play"),
    ("睡", "to sleep"),
    ("起", "to get up"),
    ("穿", "to wear"),
    ("带", "to bring"),
    ("等", "to wait"),
    ("帮", "to help"),
    ("问", "to ask"),
    ("告诉", "to tell"),
    ("觉得", "to feel"),
    ("准备", "to prepare"),
    // adverbs, conjunctions and prepositions
    ("不", "not"),
    ("都", "all"),
    ("太", "too"),
    ("就", "then"),
    ("才", "only then"),
    ("又", "again"),
    ("真", "really"),
    ("刚", "just"),
    ("别", "do not"),
    ("一起", "together"),
    ("一直", "always"),
    ("一定", "definitely"),
    ("可能", "maybe"),
    ("应该", "should"),
    ("正在", "(in progress)"),
    ("比较", "rather"),
    ("特别", "especially"),
    ("突然", "suddenly"),
    ("和", "and"),
    ("跟", "with"),
    ("对", "to / correct"),
    ("把", "(object marker)"),
    ("被", "(passive marker)"),
    ("比", "than"),
    ("为", "for"),
    ("关于", "about"),
    ("像", "like"),
    ("离", "from"),
    ("当", "when"),
    // particles
    ("的", "(possessive)"),
    ("了", "(completed)"),
    ("着", "(ongoing)"),
    ("过", "(experienced)"),
    ("得", "(complement)"),
    ("地", "(adverbial)"),
    ("吗", "(question)"),
    ("呢", "(question)"),
    ("吧", "(suggestion)"),
    // nouns and time words
    ("东西", "thing"),
    ("事", "matter"),
    ("事情", "matter"),
    ("地方", "place"),
    ("办法", "way"),
    ("意思", "meaning"),
    ("人", "person"),
    ("家", "home"),
    ("天", "day"),
    ("日", "day"),
    ("月", "month"),
    ("上", "on"),
    ("前", "before"),
    ("后", "after"),
    ("中", "middle"),
    ("晚", "late"),
    ("坏", "bad"),
    ("快", "fast"),
];

/// The learner phrase for `piece`, if it has one.
fn learner_phrase(piece: &str) -> Option<&'static str> {
    use std::sync::LazyLock;
    static TABLE: LazyLock<HashMap<&'static str, &'static str>> =
        LazyLock::new(|| LEARNER_PHRASES.iter().copied().collect());
    TABLE.get(piece).copied()
}

fn is_han(character: char) -> bool {
    matches!(character as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F)
}

/// The first phrase of a gloss line without its dictionary notes: "to like, to be fond of; ..." gives "to like",
/// "(pronoun) this" gives "this", "English (language)" gives "English". A phrase that is only a note keeps it.
fn first_phrase(gloss: &str) -> String {
    let phrase = gloss
        .split(';')
        .next()
        .unwrap_or_default()
        .split(',')
        .next()
        .unwrap_or_default()
        .trim();
    let mut plain = String::with_capacity(phrase.len());
    let mut depth = 0_usize;
    for character in phrase.chars() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => plain.push(character),
            _ => {}
        }
    }
    let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    if plain.is_empty() {
        phrase.to_owned()
    } else {
        plain
    }
}

/// Cut `text` by longest match against `known`, a map from Chinese piece to gloss, and the learner phrases.
/// Characters with no gloss stay as pieces of their own without one.
pub(crate) fn segment(
    text: &str,
    known: &HashMap<String, String>,
) -> Vec<(String, Option<String>)> {
    let characters = text.chars().collect::<Vec<_>>();
    let mut pieces = Vec::new();
    let mut start = 0;
    while start < characters.len() {
        let longest = (1..=MAX_WORD_CHARS.min(characters.len() - start))
            .rev()
            .find_map(|length| {
                let piece = characters[start..start + length].iter().collect::<String>();
                let phrase = learner_phrase(&piece)
                    .map(str::to_owned)
                    .or_else(|| known.get(&piece).map(|gloss| first_phrase(gloss)))?;
                Some((length, piece, phrase))
            });
        match longest {
            Some((length, piece, phrase)) => {
                pieces.push((piece, Some(phrase)));
                start += length;
            }
            None => {
                pieces.push((characters[start].to_string(), None));
                start += 1;
            }
        }
    }
    pieces
}

/// The displayed breakdown, or None when it would not help: fewer than two glossed pieces, or too many pieces.
pub(crate) fn render(pieces: &[(String, Option<String>)]) -> Option<String> {
    let glossed = pieces
        .iter()
        .filter(|(_, gloss)| gloss.as_deref().is_some_and(|g| !g.is_empty()))
        .count();
    if glossed < 2 || pieces.len() > MAX_PIECES {
        return None;
    }
    Some(
        pieces
            .iter()
            .map(|(piece, gloss)| match gloss.as_deref() {
                Some(gloss) if !gloss.is_empty() => format!("{piece} {gloss}"),
                _ => piece.clone(),
            })
            .collect::<Vec<_>>()
            .join(" · "),
    )
}

/// Every substring of `texts` that could be a piece, for one batched lookup.
pub(crate) fn pieces_to_ask(texts: &[String]) -> Vec<String> {
    let mut asked = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for text in texts {
        let characters = text.chars().collect::<Vec<_>>();
        for start in 0..characters.len() {
            for length in 1..=MAX_WORD_CHARS.min(characters.len() - start) {
                let piece = characters[start..start + length].iter().collect::<String>();
                if seen.insert(piece.clone()) {
                    asked.push(piece);
                }
            }
        }
    }
    asked
}

/// Whether a candidate is broken down: two to MAX_CANDIDATE_CHARS characters, all of them Han.
pub(crate) fn eligible(text: &str) -> bool {
    let count = text.chars().count();
    (2..=MAX_CANDIDATE_CHARS).contains(&count) && text.chars().all(is_han)
}

/// Look every possible piece of `texts` up in the installed tables: Unihan for single characters first (CC-CEDICT
/// orders a character's readings by pinyin, not by use), then CC-CEDICT. An absent or unreadable table contributes
/// nothing.
pub(crate) fn known_pieces(resources: &Path, texts: &[String]) -> HashMap<String, String> {
    let pieces = pieces_to_ask(texts);
    let mut known = HashMap::new();
    let lookup = |directory: &str, asked: &[String]| -> Vec<(String, String)> {
        let Some(database) = crate::supplementary_glosses::database_beside(resources, directory)
        else {
            return Vec::new();
        };
        let Some(database) = database.to_str() else {
            return Vec::new();
        };
        let candidates = asked
            .iter()
            .map(|piece| (piece.clone(), 0_u8))
            .collect::<Vec<_>>();
        msime_engine::host::candidate_target_glosses(database, "en", &candidates)
            .map(|found| {
                asked
                    .iter()
                    .cloned()
                    .zip(found)
                    .filter(|(_, gloss)| !gloss.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    };
    let singles = pieces
        .iter()
        .filter(|piece| piece.chars().count() == 1)
        .cloned()
        .collect::<Vec<_>>();
    known.extend(lookup("character-glosses", &singles));
    for (piece, gloss) in lookup("word-glosses", &pieces) {
        known.entry(piece).or_insert(gloss);
    }
    known
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn a_sentence_is_cut_by_longest_match_and_shows_first_phrases() {
        let table = known(&[
            ("我", "I, me; we, us"),
            ("喜欢", "to like, to be fond of; to be happy"),
            ("喜", "happy"),
            ("欢", "joyous"),
            ("你", "you"),
        ]);
        let pieces = segment("我喜欢你", &table);
        assert_eq!(
            render(&pieces).as_deref(),
            Some("我 I · 喜欢 to like · 你 you")
        );
    }

    #[test]
    fn dictionary_notes_are_dropped_from_a_piece() {
        let table = known(&[
            ("这个", "(pronoun) this; that"),
            ("英语", "English (language)"),
            ("啊", "(exclamatory particle)"),
        ]);
        assert_eq!(
            render(&segment("这个英语啊", &table)).as_deref(),
            Some("这个 this · 英语 English · 啊 (exclamatory particle)")
        );
    }

    #[test]
    fn an_unknown_character_stays_without_a_gloss() {
        let table = known(&[("我", "I"), ("你", "you")]);
        assert_eq!(
            render(&segment("我疼你", &table)).as_deref(),
            Some("我 I · 疼 · 你 you")
        );
    }

    #[test]
    fn a_breakdown_that_does_not_help_is_not_shown() {
        let table = known(&[("我", "I")]);
        assert_eq!(
            render(&segment("我疼", &table)),
            None,
            "one glossed piece is not a breakdown"
        );
        let every = known(&[("一", "one")]);
        assert_eq!(
            render(&segment("一一一一一一一一一", &every)),
            None,
            "too many pieces"
        );
    }

    #[test]
    fn a_learner_phrase_replaces_the_dictionary_first_phrase() {
        let table = known(&[
            ("我", "I, me; we, us"),
            ("是", "indeed, yes, right; to be"),
            ("学生", "student; schoolchild"),
            ("一起", "(in) the same place; together, in company (with)"),
            ("去", "go away, leave, depart"),
            ("学校", "school"),
        ]);
        assert_eq!(
            render(&segment("我是学生", &table)).as_deref(),
            Some("我 I · 是 to be · 学生 student")
        );
        assert_eq!(
            render(&segment("一起去学校", &table)).as_deref(),
            Some("一起 together · 去 to go · 学校 school")
        );
    }

    #[test]
    fn a_listed_word_is_cut_out_even_when_no_table_has_it() {
        let table = known(&[("你", "you"), ("好", "good")]);
        assert_eq!(
            render(&segment("你好吗", &table)).as_deref(),
            Some("你 you · 好 good · 吗 (question)")
        );
    }

    #[test]
    fn the_learner_table_has_no_duplicates_and_no_dictionary_notes() {
        let mut seen = std::collections::HashSet::new();
        for (piece, phrase) in LEARNER_PHRASES {
            assert!(seen.insert(piece), "{piece} is listed twice");
            assert!(piece.chars().all(is_han), "{piece} is not Han text");
            assert_eq!(
                first_phrase(phrase),
                *phrase,
                "{piece}: {phrase} would be cut by first_phrase"
            );
        }
    }

    #[test]
    fn only_han_text_of_two_or_more_characters_is_eligible() {
        assert!(eligible("我喜欢你"));
        assert!(!eligible("我"));
        assert!(!eligible("hello"));
        assert!(!eligible("我love你"));
        assert!(!eligible(&"好".repeat(MAX_CANDIDATE_CHARS + 1)));
    }

    #[test]
    fn every_substring_up_to_the_longest_piece_is_asked_once() {
        let asked = pieces_to_ask(&["我喜欢".to_owned(), "喜欢".to_owned()]);
        assert_eq!(asked, vec!["我", "我喜", "我喜欢", "喜", "喜欢", "欢"]);
    }
}
