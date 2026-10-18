//! macOS desktop account session storage.
//!
//! The commands themselves live in [`crate::platform::desktop::desktop_account`]. The session is `account-session.json` in the input method's own Application Support directory, in the Swift backend's layout: the input method signs in and refreshes through `shared/backend` against the same file and the same lock, so the two processes never refresh from a token the other has already spent.

use crate::platform::desktop::{desktop_account, desktop_cloud_dictionary};
use crate::platform::macos::macos_launch;
use msime_client_services::account::{AccountSessionFileLayout, FileAccountSessionStorage};
use tauri::Manager;

/// 注册输入法共用的账号会话，以及设置进程自己的云词库快照预览状态。
pub fn setup(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let directory = macos_launch::native_locator_root()?;
    app.manage(desktop_cloud_dictionary::CloudDictionaryState::new(
        app.path()
            .app_local_data_dir()?
            .join("dictionary-snapshots"),
    ));
    desktop_account::manage(
        app,
        FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Apple),
    )
}
