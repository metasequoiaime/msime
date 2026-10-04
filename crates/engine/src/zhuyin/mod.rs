//! Zhuyin (bopomofo) input on the Dachen (大千) layout with libchewing's semantics, read against `msime-zhuyin.db` (`language_dictionary`). Output is Traditional as stored, and nothing is learned.

pub mod conversion;
pub mod layout;
pub mod scheme;
pub mod syllable;
