//! 客户端里对接第三方服务的部分：语音会话与本地语音模型、服务凭据的校验与探测、AI 候选的设置和地址校验、按需下载的资源包。
//!
//! 这几个模块要联网、要跑 tokio 和 WebSocket、要解 tar.bz2，单独成一个 crate，`msime-client-core` 就不必带上这些依赖。下面用私有 `use` 把 `msime-client-base` 和 `msime-client-core` 里用到的模块接到本 crate 的根上，模块内部照旧写 `crate::storage::...`、`crate::preferences::...`。

use msime_client_base::text::{is_ascii_alphanumeric_dash, is_lower_hex};
use msime_client_base::{bounded_io, file_lock, storage, text};
use msime_client_core::{endpoint, preferences, resources, translation};

pub mod ai;
pub mod credential;
pub mod dictionary_updates;
pub mod resource_packs;
pub mod voice;
