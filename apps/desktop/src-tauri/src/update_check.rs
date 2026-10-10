//! 设置页的检查更新（`msime_client_core::update_check`）。
//!
//! 检查放在这里而不是在 webview 里 `fetch`，是为了让每个宿主都问同一份 Rust 代码：哪个发布更新、哪个安装包属于本版本和本架构；原生 Windows 设置窗口和 HarmonyOS 页面经由 `msime_client_update_check` 调到同一个函数。

use crate::CommandError;
use msime_client_core::update_check::{
    check_for_update, UpdateCheck, UpdateCheckError, UpdateCheckRequest,
};

fn update_check_error(error: UpdateCheckError) -> CommandError {
    CommandError {
        code: match error {
            UpdateCheckError::Invalid => "update_check_invalid",
            UpdateCheckError::Unavailable => "update_check_unavailable",
        },
    }
}

/// 把 `platform` 最新的已发布版本与 `current_version` 比较。在阻塞线程池上等待 GitHub，最多十秒。
#[tauri::command]
pub async fn update_check(
    platform: String,
    current_version: String,
    edition: Option<String>,
    arch: Option<String>,
) -> Result<UpdateCheck, CommandError> {
    let request = UpdateCheckRequest {
        platform,
        current_version,
        edition,
        arch,
    };
    tauri::async_runtime::spawn_blocking(move || check_for_update(&request))
        .await
        .map_err(|_| CommandError {
            code: "update_check_unavailable",
        })?
        .map_err(update_check_error)
}
