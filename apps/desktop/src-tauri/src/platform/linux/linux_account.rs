//! Linux desktop account session storage.
//!
//! The commands themselves are the same ones the Windows and macOS shells register, shared in [`crate::platform::desktop::desktop_account`]; only where the session file lives differs. Linux has no single secret service every target desktop is guaranteed to run, so this host keeps the session in an owner-only file inside the shared state directory - the same rule the Linux provider services already state for their credential files, and the same 0700 directory `msime-linux-prepare` publishes.
//!
//! The React surface receives the same redacted DTOs as every other host: the tokens never leave this process.

use crate::platform::desktop::{desktop_account, desktop_cloud_dictionary};
use msime_client_services::account::{AccountSessionFileLayout, FileAccountSessionStorage};
use std::path::Path;
use tauri::Manager;

/// Registers the shared desktop account state around the session file in `directory`, the resolved shared state directory.
///
/// 同时登记云词库面板经这个会话做完整快照（下载预览、应用到本机、备份与恢复）用的临时目录，与 Windows 和 macOS 一样放在设置应用自己的本地数据目录下。没有配置 provider socket 时，云词库面板就经这个会话完成请求。
pub fn setup(app: &tauri::AppHandle, directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(desktop_cloud_dictionary::CloudDictionaryState::new(
        app.path()
            .app_local_data_dir()?
            .join("dictionary-snapshots"),
    ));
    desktop_account::manage(
        app,
        FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Native),
    )
}
