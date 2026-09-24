//! A word-by-word English breakdown of a Chinese candidate no dictionary has as a whole — typically a sentence the
//! Engine composed, such as 我喜欢你.
//!
//! The candidate is cut left to right by longest match against the same local dictionaries the glosses use — Unihan
//! for single characters, CC-CEDICT for everything — and each piece shows the first phrase of its gloss:
//! `我 I · 喜欢 to like · 你 you`. It is not a translation; it lines the words of the sentence up with English ones,
//! which is what a learner reading the candidate wants, and it needs nothing but the tables already installed. Hosts
//! draw it on its own line under the gloss lines, never as a committable gloss column.

use std::collections::HashMap;
use std::path::Path;

/// The longest piece tried at one position; longer CC-CEDICT entries are idioms a sentence rarely contains whole.
const MAX_WORD_CHARS: usize = 8;
/// A breakdown past this many pieces is too long to read beside a candidate.
pub(crate) const MAX_PIECES: usize = 8;
/// Candidates longer than this are not broken down at all.
pub(crate) const MAX_CANDIDATE_CHARS: usize = 32;

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

/// Cut `text` by longest match against `known`, a map from Chinese piece to gloss. Characters with no gloss stay as
/// pieces of their own without one.
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
                known
                    .get(&piece)
                    .map(|gloss| (length, piece, gloss.as_str()))
            });
        match longest {
            Some((length, piece, gloss)) => {
                pieces.push((piece, Some(first_phrase(gloss))));
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
            ("的", "(possessive particle)"),
        ]);
        assert_eq!(
            render(&segment("这个英语的", &table)).as_deref(),
            Some("这个 this · 英语 English · 的 (possessive particle)")
        );
    }

    #[test]
    fn an_unknown_character_stays_without_a_gloss() {
        let table = known(&[("我", "I"), ("你", "you")]);
        assert_eq!(
            render(&segment("我爱你", &table)).as_deref(),
            Some("我 I · 爱 · 你 you")
        );
    }

    #[test]
    fn a_breakdown_that_does_not_help_is_not_shown() {
        let table = known(&[("我", "I")]);
        assert_eq!(
            render(&segment("我爱", &table)),
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
