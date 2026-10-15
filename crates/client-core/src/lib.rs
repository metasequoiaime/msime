//! Shared client business logic. Independent of UI frameworks and IME hosts.
//!
//! Modules that serve one domain are grouped under it. A flat list of
//! thirty-four modules gave no hint which of `cloud_dictionary`,
//! `dictionary_access` and `personal_dictionary` belonged together, and the
//! shared prefixes were doing the grouping work that the module tree should do.
//!
//! 存储、文件锁、有上限的读取、文本谓词、日历和简繁转换在 `msime-client-base` 里，下面用 `use`/`pub use` 接回原来的路径：宿主照旧写 `msime_client_core::file_lock`，本 crate 内部照旧写 `crate::storage::...`。

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
use msime_client_base::bounded_io;
pub use msime_client_base::calendar;
pub mod candidate_document;
pub use msime_client_base::chinese_conversion;
pub mod clipboard;
/// 自建云剪贴板中继的客户端：自己联网，所以不在只做 sans-IO 的 `cloud` 下面。
pub mod clipboard_relay;
pub mod cloud;
pub mod common_phrases;
pub mod community;
pub mod credential;
pub mod diagnostics;
pub mod dictionary;
pub mod edition;
mod endpoint;
pub use msime_client_base::file_lock;
pub mod helpcode;
pub mod host_surface;
pub mod notices;
pub mod panels;
pub mod plugins;
pub mod preferences;
pub mod punctuation;
pub mod resource_packs;
pub mod resources;
pub mod settings_document;
pub mod skin;
use msime_client_base::storage;
pub mod telemetry;
use msime_client_base::text;

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
pub mod theme;
pub mod translation;
pub mod typing_statistics;
pub mod update_check;
pub mod vocabulary;
pub mod voice;
