//! The typing dictionaries as an agent sees them: the user's own words, the bundled ones under a code, and the candidates a code offers.
//!
//! These are what the user types, so every tool here is offered only when the user started the server with --allow-dictionary-read, and the ones that change words need --allow-write as well.

use msime_host_api::{
    CandidateOrigin, LookupCandidate, LookupScheme, NewWord, Word, WordEdit, WordImport, WordKind,
};
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The most words one import call carries. The host takes more, but an agent sending a pasted list has to show its work across calls.
pub const MAX_IMPORT: usize = 200;
pub const DEFAULT_LOOKUP: usize = 20;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum Dictionary {
    /// Full pinyin, which double pinyin types from as well.
    Pinyin,
    Wubi,
    English,
    /// 98 五笔码表。
    Wubi98,
}

impl From<Dictionary> for WordKind {
    fn from(value: Dictionary) -> Self {
        match value {
            Dictionary::Pinyin => Self::Pinyin,
            Dictionary::Wubi => Self::Wubi,
            Dictionary::Wubi98 => Self::Wubi98,
            Dictionary::English => Self::English,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct WordListRequest {
    pub dictionary: Dictionary,
    /// Only words whose code starts with this. Pinyin codes may be written with or without apostrophes.
    pub code_prefix: Option<String>,
    /// Also list the words that shipped with the dictionary or were learned from typing. Needs a code_prefix.
    pub include_bundled: Option<bool>,
    /// How many matching words to skip.
    pub offset: Option<usize>,
    /// At most this many words, 1 to 1000. Defaults to 100.
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct WordView {
    /// What the user types.
    pub code: String,
    pub word: String,
    /// Higher ranks first among words under the same code.
    pub weight: i64,
    /// Shipped with the dictionary or learned from typing rather than added by the user. Its code and word are fixed; it can only be reweighted or removed.
    pub bundled: bool,
}

impl From<Word> for WordView {
    fn from(value: Word) -> Self {
        Self {
            code: value.code,
            word: value.word,
            weight: value.weight,
            bundled: value.bundled,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct WordListPage {
    pub words: Vec<WordView>,
    /// More words match; ask again with a larger offset.
    pub has_more: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum WordChange {
    /// Add a word. A pinyin code may be left out, and is then the word's most common reading. Refused if the word is already there under that code.
    Add {
        dictionary: Dictionary,
        code: Option<String>,
        word: String,
        /// 1 to 100000000. Defaults to the weight the settings page gives a new word.
        weight: Option<i64>,
    },
    /// Give a word, found by its code and word, another weight from 1 to 100000000.
    SetWeight {
        dictionary: Dictionary,
        code: String,
        word: String,
        weight: i64,
    },
    /// Remove a word, found by its code and word.
    Remove {
        dictionary: Dictionary,
        code: String,
        word: String,
    },
}

impl From<WordChange> for WordEdit {
    fn from(value: WordChange) -> Self {
        match value {
            WordChange::Add {
                dictionary,
                code,
                word,
                weight,
            } => Self::Add(dictionary.into(), NewWord { code, word, weight }),
            WordChange::SetWeight {
                dictionary,
                code,
                word,
                weight,
            } => Self::SetWeight {
                kind: dictionary.into(),
                code,
                word,
                weight,
            },
            WordChange::Remove {
                dictionary,
                code,
                word,
            } => Self::Remove {
                kind: dictionary.into(),
                code,
                word,
            },
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct WordEditRequest {
    /// Applied in order, at most 50. The first that fails stops the rest; those before it stay applied.
    pub edits: Vec<WordChange>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct ImportedWord {
    /// What the user types. A pinyin code may be left out, and is then the word's most common reading.
    pub code: Option<String>,
    pub word: String,
    /// 1 to 100000000. Defaults to the weight the settings page gives a new word.
    pub weight: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct WordImportRequest {
    pub dictionary: Dictionary,
    /// 1 to 200 words.
    pub words: Vec<ImportedWord>,
}

impl WordImportRequest {
    pub fn new_words(self) -> Vec<NewWord> {
        self.words
            .into_iter()
            .map(|word| NewWord {
                code: word.code,
                word: word.word,
                weight: word.weight,
            })
            .collect()
    }
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct Rejection {
    /// The position of the refused word in the request.
    pub index: usize,
    /// The rule it broke.
    pub reason: String,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct WordImportOutcome {
    pub added: usize,
    /// Already in the dictionary, the user's or bundled, and left as they were.
    pub existing: usize,
    pub rejected: Vec<Rejection>,
}

impl From<WordImport> for WordImportOutcome {
    fn from(value: WordImport) -> Self {
        Self {
            added: value.added,
            existing: value.existing,
            rejected: value
                .rejected
                .into_iter()
                .map(|(index, reason)| Rejection { index, reason })
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum LookupSchemeChoice {
    Quanpin,
    Shuangpin,
    Wubi,
}

impl From<LookupSchemeChoice> for LookupScheme {
    fn from(value: LookupSchemeChoice) -> Self {
        match value {
            LookupSchemeChoice::Quanpin => Self::Quanpin,
            LookupSchemeChoice::Shuangpin => Self::Shuangpin,
            LookupSchemeChoice::Wubi => Self::Wubi,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct LookupRequest {
    /// The keys typed: lowercase letters and apostrophes, and in double pinyin semicolons. At most 64.
    pub code: String,
    /// The scheme to type in. Defaults to the user's.
    pub scheme: Option<LookupSchemeChoice>,
    /// At most this many candidates, 1 to 50. Defaults to 20.
    pub limit: Option<usize>,
}

#[derive(Clone, Copy, Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// A word shipped with the dictionary or learned from typing.
    Dictionary,
    /// A word the user added.
    UserWord,
    /// Put together from several dictionary words, such as a sentence.
    Composed,
    English,
    QuickPhrase,
    Emoji,
    Kaomoji,
    /// Made by a rule rather than found in a dictionary, such as a date.
    Generated,
    /// The typed keys themselves, when nothing else matched.
    Fallback,
}

impl From<CandidateOrigin> for Origin {
    fn from(value: CandidateOrigin) -> Self {
        match value {
            CandidateOrigin::Dictionary => Self::Dictionary,
            CandidateOrigin::UserWord => Self::UserWord,
            CandidateOrigin::Composed => Self::Composed,
            CandidateOrigin::English => Self::English,
            CandidateOrigin::QuickPhrase => Self::QuickPhrase,
            CandidateOrigin::Emoji => Self::Emoji,
            CandidateOrigin::Kaomoji => Self::Kaomoji,
            CandidateOrigin::Generated => Self::Generated,
            CandidateOrigin::Fallback => Self::Fallback,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct CandidateView {
    pub text: String,
    /// The part of the typed keys this candidate covers.
    pub code: String,
    pub origin: Origin,
    /// The weight of the dictionary word, when the candidate is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<i64>,
}

impl From<LookupCandidate> for CandidateView {
    fn from(value: LookupCandidate) -> Self {
        Self {
            text: value.text,
            code: value.code,
            origin: value.origin.into(),
            weight: value.weight,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct LookupView {
    /// In the order the input method ranks them, first to last.
    pub candidates: Vec<CandidateView>,
}
