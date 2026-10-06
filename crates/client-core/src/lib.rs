//! Shared client business logic. Independent of UI frameworks and IME hosts.
//!
//! Modules that serve one domain are grouped under it. A flat list of
//! thirty-four modules gave no hint which of `cloud_dictionary`,
//! `dictionary_access` and `personal_dictionary` belonged together, and the
//! shared prefixes were doing the grouping work that the module tree should do.

/// The `Uuid` this crate's public API is written in terms of.
///
/// `SavedTouchKeyboardSkin::id`, `CustomSkinLibraryStore::import_download` and
/// `KeyboardSkinTrialStore::finish` all name it, so a caller cannot use those
/// without it. Re-exporting is how it stays one version: a host that reached
/// for the crate itself would pick a version independently, and two `Uuid`
/// types that are structurally identical and nominally different produce a
/// mismatch error with no obvious cause.
pub use uuid;

pub mod account;
pub mod ai;
mod bounded_io;
pub mod calendar;
pub mod candidate_document;
pub mod chinese_conversion;
pub mod clipboard;
pub mod cloud;
pub mod common_phrases;
pub mod community;
pub mod credential;
pub mod diagnostics;
pub mod dictionary;
pub mod edition;
pub mod file_lock;
pub mod helpcode;
pub mod host_surface;
pub mod notices;
pub mod panels;
pub mod plugins;
pub mod preferences;
pub mod punctuation;
pub mod resource_packs;
pub mod resources;
pub mod skin;
mod storage;
pub mod telemetry;
mod text;

/// Shared text and hexadecimal validation predicates used by host boundaries.
pub use text::{
    has_disallowed_control_with_allowed, has_disallowed_control_with_line_breaks,
    has_disallowed_control_with_options, is_ascii_alphabetic, is_ascii_alphanumeric_dash,
    is_ascii_digits, is_ascii_graphic, is_ascii_hex, is_ascii_identifier,
    is_ascii_identifier_with_dots, is_ascii_lowercase, is_ascii_lowercase_identifier_with_dots,
    is_bounded_ascii_identifier, is_bounded_chars, is_bounded_chars_with_options,
    is_bounded_chars_without_nul, is_bounded_text, is_bounded_text_with_chars,
    is_bounded_text_with_options, is_bounded_utf16, is_hex_color, is_lower_hex,
};
pub mod translation;
pub mod typing_statistics;
pub mod vocabulary;
pub mod voice;
