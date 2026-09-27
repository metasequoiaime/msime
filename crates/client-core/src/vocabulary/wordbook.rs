//! The word list a review session draws from.
//!
//! A wordbook is read-only once loaded. Bundled books arrive through the pinned-resource path and
//! imported ones through [`super::import`]; both end up as the same in-memory shape, so nothing
//! downstream has to care which it was.

use serde::{Deserialize, Serialize};

/// The longest a single headword may be.
///
/// Wordbooks are English vocabulary lists, where the longest real entries are hyphenated
/// compounds. This is a document guard rather than a linguistic claim: it stops one damaged line
/// from carrying a megabyte into a store every host reads at startup.
pub const MAX_WORD_CHARS: usize = 64;
/// The longest phonetic transcription. IPA for one word does not approach this.
pub const MAX_PHONETIC_CHARS: usize = 64;
/// The longest gloss — several senses with part-of-speech tags, and still short enough that a card
/// fits on a phone without scrolling.
pub const MAX_MEANING_CHARS: usize = 256;
/// The longest wordbook identifier.
pub const MAX_ID_CHARS: usize = 64;
/// The longest wordbook display name.
pub const MAX_NAME_CHARS: usize = 64;
/// The most words one book may hold.
///
/// The largest book anyone ships is a 托福 list at roughly ten thousand. Double it, so the limit
/// is a guard against a malformed file rather than a ceiling a real book runs into.
pub const MAX_ENTRIES: usize = 20_000;

fn text_is_valid(value: &str, maximum: usize, required: bool) -> bool {
    (!required || !value.is_empty())
        && value.chars().count() <= maximum
        && !value.chars().any(char::is_control)
}

pub(crate) fn word_is_valid(value: &str) -> bool {
    text_is_valid(value, MAX_WORD_CHARS, true)
}

/// One word as a wordbook stores it.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordbookEntry {
    pub word: String,
    /// May be empty. A user's own list often has no transcription, and a card without one is
    /// still a card.
    #[serde(default)]
    pub phonetic: String,
    pub meaning: String,
}

impl WordbookEntry {
    /// Whether this entry is within the document limits and carries the two fields a card needs.
    ///
    /// One predicate because three places ask it: the import parser rejecting a row, the wordbook
    /// loader rejecting a bundled book, and the progress store refusing to key a card on a word
    /// it could not have stored. A word accepted by one and refused by another is a card that
    /// imports and then cannot be scheduled.
    pub fn is_valid(&self) -> bool {
        word_is_valid(&self.word)
            // A control character in a headword would be invisible on every host's card, and it
            // would let two words that look identical compare as different store keys.
            && text_is_valid(&self.phonetic, MAX_PHONETIC_CHARS, false)
            && text_is_valid(&self.meaning, MAX_MEANING_CHARS, true)
    }
}

/// Whether `id` is a usable wordbook identifier.
///
/// Lowercase ASCII, digits and `-`. The identifier is a store key and reaches the Windows host's
/// shell surfaces, whose `append()` accepts a narrower alphabet than the shared route parser does;
/// keeping the set small here means a book id cannot be silently dropped on one platform.
pub fn id_is_well_formed(id: &str) -> bool {
    !id.is_empty()
        && id.chars().count() <= MAX_ID_CHARS
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !id.starts_with('-')
        && !id.ends_with('-')
}

/// A loaded word list.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Wordbook {
    pub id: String,
    pub name: String,
    pub entries: Vec<WordbookEntry>,
}

impl Wordbook {
    /// Whether the book is one this code could have produced.
    ///
    /// Duplicate headwords are rejected rather than deduplicated. A card is keyed by its word, so
    /// two rows spelling the same word are one card with two glosses, and silently keeping
    /// whichever came last would make the book's own contents depend on row order.
    pub fn is_valid(&self) -> bool {
        if !id_is_well_formed(&self.id)
            || !text_is_valid(&self.name, MAX_NAME_CHARS, true)
            || self.entries.is_empty()
            || self.entries.len() > MAX_ENTRIES
            || !self.entries.iter().all(WordbookEntry::is_valid)
        {
            return false;
        }
        let mut seen = std::collections::BTreeSet::new();
        self.entries.iter().all(|entry| seen.insert(&entry.word))
    }

    /// The headwords, in the order the book lists them.
    ///
    /// Order is the book's own, never sorted here. A 考研 list is ordered by the compiler's idea
    /// of what to learn first, and re-sorting it alphabetically would throw that away.
    pub fn words(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|entry| entry.word.as_str())
    }

    /// The entry for `word`, if the book has one.
    pub fn entry(&self, word: &str) -> Option<&WordbookEntry> {
        self.entries.iter().find(|entry| entry.word == word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(word: &str) -> WordbookEntry {
        WordbookEntry {
            word: word.to_owned(),
            phonetic: "/juːˈbɪkwɪtəs/".to_owned(),
            meaning: "adj. 无处不在的".to_owned(),
        }
    }

    fn book() -> Wordbook {
        Wordbook {
            id: "cet-4".to_owned(),
            name: "CET-4".to_owned(),
            entries: vec![entry("ubiquitous"), entry("ephemeral")],
        }
    }

    #[test]
    fn an_entry_needs_a_word_and_a_meaning_within_the_limits() {
        assert!(entry("ubiquitous").is_valid());

        // A missing transcription is ordinary in a user's own list.
        assert!(WordbookEntry {
            phonetic: String::new(),
            ..entry("ubiquitous")
        }
        .is_valid());

        assert!(!WordbookEntry {
            word: String::new(),
            ..entry("x")
        }
        .is_valid());
        assert!(!WordbookEntry {
            meaning: String::new(),
            ..entry("x")
        }
        .is_valid());
        assert!(!entry(&"a".repeat(MAX_WORD_CHARS + 1)).is_valid());
        assert!(WordbookEntry {
            meaning: "词".repeat(MAX_MEANING_CHARS),
            ..entry("x")
        }
        .is_valid());
        assert!(!WordbookEntry {
            meaning: "词".repeat(MAX_MEANING_CHARS + 1),
            ..entry("x")
        }
        .is_valid());
    }

    #[test]
    fn a_control_character_is_refused_in_every_field() {
        assert!(!entry("ubiqui\ttous").is_valid());
        assert!(!WordbookEntry {
            phonetic: "/ju\u{0}/".to_owned(),
            ..entry("x")
        }
        .is_valid());
        assert!(!WordbookEntry {
            meaning: "adj.\n无处不在的".to_owned(),
            ..entry("x")
        }
        .is_valid());
    }

    #[test]
    fn an_identifier_is_lowercase_ascii_digits_and_inner_hyphens() {
        assert!(id_is_well_formed("cet-4"));
        assert!(id_is_well_formed("kaoyan"));
        assert!(id_is_well_formed("user-2026-09-23"));
        assert!(!id_is_well_formed(""));
        assert!(!id_is_well_formed("CET-4"));
        assert!(!id_is_well_formed("cet_4"));
        assert!(!id_is_well_formed("-cet"));
        assert!(!id_is_well_formed("cet-"));
        assert!(!id_is_well_formed("词书"));
        assert!(!id_is_well_formed(&"a".repeat(MAX_ID_CHARS + 1)));
    }

    #[test]
    fn a_book_is_rejected_when_it_repeats_a_headword() {
        assert!(book().is_valid());

        let mut repeated = book();
        repeated.entries.push(entry("ubiquitous"));
        assert!(
            !repeated.is_valid(),
            "two rows for one word are one card with two glosses, not a valid book"
        );
    }

    #[test]
    fn a_book_needs_an_identifier_a_name_and_at_least_one_entry() {
        assert!(!Wordbook {
            entries: Vec::new(),
            ..book()
        }
        .is_valid());
        assert!(!Wordbook {
            id: "CET 4".to_owned(),
            ..book()
        }
        .is_valid());
        assert!(!Wordbook {
            name: String::new(),
            ..book()
        }
        .is_valid());
        assert!(!Wordbook {
            name: "名".repeat(MAX_NAME_CHARS + 1),
            ..book()
        }
        .is_valid());
    }

    #[test]
    fn lookups_keep_the_books_own_order() {
        let book = book();
        assert_eq!(
            book.words().collect::<Vec<_>>(),
            vec!["ubiquitous", "ephemeral"],
            "a compiled list is ordered by what to learn first; never re-sort it"
        );
        assert_eq!(
            book.entry("ephemeral").map(|e| e.word.as_str()),
            Some("ephemeral")
        );
        assert_eq!(book.entry("absent"), None);
    }
}
