//! Local dictionary storage, import and access policy.
//!
//! The account-synchronised side lives under [`crate::cloud::dictionary`]: the
//! split follows where the data lives, because the two have different failure
//! modes and different validation.

pub mod access;
pub mod collections;
pub mod import;
pub mod personal;
pub mod quiesce;

/// Whether `character` belongs to a CJK unified ideograph block accepted by the `hans` import
/// format.
pub fn is_han_character(character: char) -> bool {
    matches!(
        character as u32,
        0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff | 0x20000..=0x2fa1f
    )
}

/// Whether `code` is a usable English input code.
///
/// The code is what the user types; the word beside it is what that types out, and the two need not be the same text - `dont` types out `don't`. The reference accepts any non-empty word beside a code of letters, hyphens and apostrophes (`IsAsciiWord` in its `dictionary_manager`), and the Engine's personal-dictionary check (`validate_personal_dictionary_entry` in `msime-engine`) lowercases the code and accepts the same three kinds of byte.
///
/// One predicate because five places ask this question - the import parser, the personal-word
/// transport check, both sides of the account validator, and the host API's own entry check - and
/// a code accepted by one and refused by another is an entry that imports and then fails to sync,
/// or one the API refuses after the file it came from was read successfully.
pub fn english_code_is_well_formed(code: &str) -> bool {
    code.bytes()
        .all(|byte| byte.is_ascii_alphabetic() || byte == b'-' || byte == b'\'')
}

/// Whether `code` is a quick phrase code a new entry or an import may use: lowercase letters only, as the reference's `valid_code` in its `dictionary_manager` accepts. Checks on stored rows stay lenient so an entry saved with a digit still loads, lists, syncs and can be deleted.
pub fn quick_phrase_code_is_well_formed(code: &str) -> bool {
    !code.is_empty() && crate::is_ascii_lowercase(code)
}

/// Whether `code` contains the lowercase letters and digits accepted by the quick phrase transport.
/// Checks on stored rows stay lenient so an entry saved with a digit still loads, lists, syncs and
/// can be deleted.
pub fn quick_phrase_transport_code_is_well_formed(code: &str) -> bool {
    code.bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

/// Whether `code` contains only the lowercase ASCII letters accepted by Wubi.
pub fn wubi_code_is_well_formed(code: &str) -> bool {
    crate::is_ascii_lowercase(code)
}

/// Whether `code` contains the lowercase letters and apostrophes accepted by pinyin input.
/// Rime's word-first import format also permits spaces between syllables.
pub fn pinyin_code_is_well_formed(code: &str, allow_spaces: bool) -> bool {
    code.bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte == b'\'' || (allow_spaces && byte == b' '))
}
