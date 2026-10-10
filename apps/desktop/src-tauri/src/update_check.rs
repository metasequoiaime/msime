//! The settings page's update check (`msime_client_core::update_check`).
//!
//! The check runs here rather than as a `fetch` in the webview so every host asks the same Rust code which release is newer and which package belongs to this edition and architecture; the native Windows settings window and the HarmonyOS page reach the same function through `msime_client_update_check`.

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

/// The newest published release of `platform` compared with `current_version`. Blocks on GitHub for up to ten seconds, on the blocking pool.
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
