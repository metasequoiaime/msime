//! 客户端里对接后端服务和管理用户内容的部分：账号、社区、遥测、公告、检查更新、诊断包、自建云剪贴板中继，以及皮肤、插件、词库、生词本、常用语和打字统计。
//!
//! 这些模块要联网、要解 zip 和图片、要跑脚本插件的校验，单独成一个 crate，`msime-client-core` 就不必带上 reqwest、image、zip、wasmi 这些依赖，只要偏好和协议的宿主（输入运行时、后端引擎）编译图更小。下面用私有 `use` 把 `msime-client-base` 和 `msime-client-core` 里用到的模块和文本谓词接到本 crate 的根上，模块内部照旧写 `crate::storage::...`、`crate::preferences::...`、`crate::is_bounded_utf16`。

/// 本 crate 公开接口里出现的 `Uuid`（`SavedTouchKeyboardSkin::id`、`CustomSkinLibraryStore::import_download`、`KeyboardSkinTrialStore::finish` 等）。和 `msime_client_core::uuid` 是同一个 crate，宿主用哪一个都行，不会出现两个名义上不同的 `Uuid`。
pub use uuid;

use msime_client_base::text::{
    has_disallowed_control_with_allowed, has_disallowed_control_with_line_breaks,
    has_disallowed_control_with_options, is_ascii_digits, is_ascii_identifier_with_dots,
    is_ascii_lowercase, is_bounded_ascii_identifier, is_bounded_utf16,
};
use msime_client_base::{bounded_io, calendar, file_lock, storage, text};
use msime_client_core::{cloud, edition, endpoint, helpcode, host_surface, preferences, theme};

pub mod account;
/// 自建云剪贴板中继的客户端：自己联网，所以不在只做 sans-IO 的 `cloud` 下面。
pub mod clipboard_relay;
pub mod common_phrases;
pub mod community;
pub mod diagnostics;
pub mod dictionary;
pub mod notices;
pub mod plugins;
pub mod skin;
pub mod telemetry;
pub mod typing_statistics;
pub mod update_check;
pub mod vocabulary;
