//! 设置页的检查更新（`msime_client_services::update_check`）。
//!
//! 检查放在这里而不是在 webview 里 `fetch`，是为了让每个宿主都问同一份 Rust 代码：哪个发布更新、哪个安装包属于本版本和本架构；原生 Windows 设置窗口和 HarmonyOS 页面经由 `msime_client_update_check` 调到同一个函数。

use crate::CommandError;
use msime_client_services::update_check::{
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

/// 只为带完整更新配置的 macOS 安装包提供原生更新入口。
#[tauri::command]
pub fn application_update_supported() -> bool {
    #[cfg(target_os = "macos")]
    {
        msime_host_macos::settings_update_supported()
    }
    #[cfg(not(target_os = "macos"))]
    false
}

/// Sparkle 的标准界面负责下载、验签、替换应用和重启，始终运行在 AppKit 主线程。
#[tauri::command]
pub async fn application_update_check(app: tauri::AppHandle) -> Result<(), CommandError> {
    #[cfg(target_os = "macos")]
    {
        let (sender, receiver) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let _ = sender.send(msime_host_macos::check_settings_update());
        })
        .map_err(|_| CommandError {
            code: "application_update_unavailable",
        })?;
        tauri::async_runtime::spawn_blocking(move || receiver.recv().unwrap_or(false))
            .await
            .unwrap_or(false)
            .then_some(())
            .ok_or(CommandError {
                code: "application_update_unavailable",
            })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err(CommandError {
            code: "application_update_unavailable",
        })
    }
}
