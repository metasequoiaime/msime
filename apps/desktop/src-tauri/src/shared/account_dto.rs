//! webview 读取的脱敏账号应答，由 iOS 宿主和三个桌面宿主共用。
//!
//! 状态、用户、登录方式、验证挑战和资料这几种结构定义在 [`msime_client_services::account::page`]，因为原生 Windows 设置窗口经 C ABI（`msime_client_account`）返回的也是同样的结构，设置同步的字段表（`PreferenceSchemaResponse`）同理。聊天的两种结构（`ChatModelsResponse`、`ChatResponse`）同样定义在那里，`msime_client_account` 的 `chat_models` 和 `chat` 也返回它们，这里只由 iOS 和 Android 的账号命令用到。

#[cfg(not(target_os = "android"))]
pub(crate) use msime_client_services::account::page::providers_response;
#[cfg_attr(
    not(any(target_os = "ios", target_os = "android")),
    allow(unused_imports)
)]
pub(crate) use msime_client_services::account::page::providers_response_without_apple;
pub(crate) use msime_client_services::account::page::{
    ChallengeResponse, ProfileResponse, ProvidersResponse, StatusResponse,
};
// 设置同步只接在 Windows、iOS 和 Android 的账号会话上。
#[cfg_attr(
    not(any(target_os = "windows", target_os = "ios", target_os = "android")),
    allow(unused_imports)
)]
pub(crate) use msime_client_services::account::page::PreferenceSchemaResponse;
// 只有 iOS 和 Android 的账号命令返回聊天的两种结构。
#[cfg_attr(
    not(any(target_os = "ios", target_os = "android")),
    allow(unused_imports)
)]
pub(crate) use msime_client_services::account::page::{ChatModelsResponse, ChatResponse};
