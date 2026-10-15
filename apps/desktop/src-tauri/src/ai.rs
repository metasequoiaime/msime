//! 自带接入点的 AI 助手在设置页上的两个请求：列出模型，以及一次性的润色测试。
//!
//! 校验、请求和响应解析都在 client-providers 的 `ai::settings` 里，原生 Windows 设置窗口经 `msime_client_ai_models` 和 `msime_client_ai_test` 调用它们；这些命令只把阻塞调用移出异步运行时，并把它的错误转成错误码。
//!
//! 只有提供这项功能的两个桌面宿主编译这个模块。Android 和 iOS 在各自的平台模块里应答同样的两个命令，因为那些请求走平台自己的 HTTP 栈，而不是 reqwest。

#[cfg(not(target_os = "linux"))]
use crate::CommandError;
#[cfg(not(target_os = "linux"))]
use msime_client_providers::ai::settings::AiSettingsError;

#[cfg(not(target_os = "linux"))]
fn command_error(error: AiSettingsError) -> CommandError {
    CommandError { code: error.code() }
}

// Linux routes these two through the provider socket instead - see the
// same-named commands in lib.rs - and `#[tauri::command]` declares a crate-level
// `macro_rules! __cmd__<name>`, so two commands sharing a name collide however
// separate their modules are. Gate the definitions the way their registration is
// already gated rather than leaving both to exist on Linux.
#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub(crate) async fn ai_models(
    endpoint: String,
    token: String,
) -> Result<Vec<String>, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        msime_client_providers::ai::settings::fetch_models(&endpoint, &token).map_err(command_error)
    })
    .await
    .map_err(|_| command_error(AiSettingsError::ModelsUnavailable))?
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub(crate) async fn ai_test(
    endpoint: String,
    model: String,
    prompt: String,
    token: String,
    text: String,
) -> Result<String, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        msime_client_providers::ai::settings::test_polish(&endpoint, &model, &prompt, &token, &text)
            .map_err(command_error)
    })
    .await
    .map_err(|_| command_error(AiSettingsError::TestUnavailable))?
}
