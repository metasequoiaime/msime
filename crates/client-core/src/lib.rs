//! Shared client business logic. Independent of UI frameworks and IME hosts.
//!
//! Modules that serve one domain are grouped under it. A flat list of
//! thirty-four modules gave no hint which of `cloud_dictionary`,
//! `dictionary_access` and `personal_dictionary` belonged together, and the
//! shared prefixes were doing the grouping work that the module tree should do.
//!
//! 存储、文件锁、有上限的读取、文本谓词、日历和简繁转换在 `msime-client-base` 里，下面用 `use`/`pub use` 接回原来的路径：宿主照旧写 `msime_client_core::file_lock`，本 crate 内部照旧写 `crate::storage::...`。
//!
//! 语音、服务凭据、AI 候选设置和按需资源包在 `msime-client-providers` 里：它依赖本 crate，本 crate 不引用它，宿主写 `msime_client_providers::voice` 这类路径。`endpoint` 只为它和下面的 `msime-client-services` 导出，不是宿主接口，所以是 `#[doc(hidden)]`。
//!
//! 账号、社区、遥测、公告、检查更新、诊断、自建剪贴板中继、皮肤、插件、词库、生词本、常用语和打字统计在 `msime-client-services` 里，依赖方向与 providers 相同：它依赖本 crate，本 crate 不引用它，宿主写 `msime_client_services::skin` 这类路径。

/// The `Uuid` this crate's public API is written in terms of.
///
/// `SavedTouchKeyboardSkin::id`, `CustomSkinLibraryStore::import_download` and
/// `KeyboardSkinTrialStore::finish` all name it, so a caller cannot use those
/// without it. Re-exporting is how it stays one version: a host that reached
/// for the crate itself would pick a version independently, and two `Uuid`
/// types that are structurally identical and nominally different produce a
/// mismatch error with no obvious cause.
///
/// 上面这几个类型已随皮肤搬到 `msime-client-services`，那边同样 `pub use uuid`；这里保留，是因为宿主一直写 `msime_client_core::uuid`，两处导出的是同一个 crate。
pub use uuid;

use msime_client_base::bounded_io;
pub use msime_client_base::calendar;
pub mod candidate_document;
pub use msime_client_base::chinese_conversion;
pub mod clipboard;
pub mod cloud;
pub mod edition;
#[doc(hidden)]
pub mod endpoint;
pub use msime_client_base::file_lock;
pub mod helpcode;
pub mod host_surface;
pub mod ios_keyboard;
pub mod key_chord;
pub mod keyboard_skin;
pub mod language_cycle;
pub mod network;
pub mod panels;
pub mod preferences;
pub mod punctuation;
pub mod resources;
pub mod settings_document;
use msime_client_base::storage;
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
pub mod world_clock;
