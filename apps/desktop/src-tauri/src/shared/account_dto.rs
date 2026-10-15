//! webview 读取的脱敏账号应答，由 iOS 宿主和三个桌面宿主共用。
//!
//! 状态、用户、登录方式、验证挑战和资料这几种结构定义在 [`msime_client_services::account::page`]，因为原生 Windows 设置窗口经 C ABI（`msime_client_account`）返回的也是同样的结构，设置同步的字段表（`PreferenceSchemaResponse`）同理。下面的聊天结构只由 iOS 和 Android 的账号命令返回。

#[cfg_attr(
    not(any(target_os = "ios", target_os = "android")),
    allow(unused_imports)
)]
pub(crate) use msime_client_services::account::page::providers_response_without_apple;
pub(crate) use msime_client_services::account::page::{
    providers_response, ChallengeResponse, ProfileResponse, ProvidersResponse, StatusResponse,
};
// 设置同步只接在 Windows、iOS 和 Android 的账号会话上。
#[cfg_attr(
    not(any(target_os = "windows", target_os = "ios", target_os = "android")),
    allow(unused_imports)
)]
pub(crate) use msime_client_services::account::page::PreferenceSchemaResponse;
use msime_client_services::account::AccountChatModels;
use serde::Serialize;

// 只有 iOS 和 Android 的账号命令返回它。
#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatModelResponse {
    id: String,
}

// 只有 iOS 和 Android 的账号命令返回它。
#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatModelsResponse {
    data: Vec<ChatModelResponse>,
    default_model: String,
}

impl From<AccountChatModels> for ChatModelsResponse {
    fn from(models: AccountChatModels) -> Self {
        Self {
            data: models
                .data
                .into_iter()
                .map(|model| ChatModelResponse { id: model.id })
                .collect(),
            default_model: models.default_model,
        }
    }
}

// 只有 iOS 和 Android 的账号命令返回它。
#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub(crate) content: String,
}
