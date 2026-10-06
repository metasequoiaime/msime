//! Korean Hangul input on the Dubeolsik (2-beolsik) layout: a syllable automaton with no cloud rows and no learning. A syllable composes in the preedit and is committed as soon as the next key starts another one; the session commits the open syllable on Space, Enter, punctuation and caret keys. The only candidates are the Hanja of the composing syllable, listed while the user has asked for them (`Command::ConvertHanja`) from the table in `hanja`.

pub mod dubeolsik;
pub mod hanja;
pub mod scheme;

pub use scheme::KoreanScheme;
