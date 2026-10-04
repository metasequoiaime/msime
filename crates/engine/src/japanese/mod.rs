//! Japanese romaji input (schemes-lang.md §5): romaji-to-kana conversion with pending letters, kana variants, the `msime-japanese.dat` (MSJPDT1) lemma dictionary and its matrix search, the provider and the scheme. The `japanese_lexicon` SQL stage is dropped: the shipped `msime-pinyin.db` has no such table. `wana_kana` covers the plain kana conversions; the romaji table with pending input and sokuon rules is IME-specific and stays hand-written.

pub mod decoder;
pub mod matrix;
pub mod provider;
pub mod romaji;
pub mod scheme;

pub use provider::JapaneseProvider;
pub use scheme::JapaneseRomajiScheme;
