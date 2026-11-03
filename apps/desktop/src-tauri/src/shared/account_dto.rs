//! webview 读取的脱敏账号应答，由三个桌面宿主共用。
//!
//! 状态、用户、登录方式、验证挑战和资料这几种结构定义在 [`msime_client_services::account::page`]，因为原生 Windows 设置窗口经 C ABI（`msime_client_account`）返回的也是同样的结构，设置同步的字段表（`PreferenceSchemaResponse`）同理。

pub(crate) use msime_client_services::account::page::providers_response;
pub(crate) use msime_client_services::account::page::{
    ChallengeResponse, ProfileResponse, ProvidersResponse, StatusResponse,
};
// 设置同步只接在 Windows 和 Linux 的账号会话上。
#[cfg_attr(
    not(any(target_os = "windows", target_os = "linux")),
    allow(unused_imports)
)]
pub(crate) use msime_client_services::account::page::PreferenceSchemaResponse;
