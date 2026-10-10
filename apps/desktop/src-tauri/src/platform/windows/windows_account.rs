//! Windows 桌面账号会话的存放位置。
//!
//! 命令本身在 [`crate::platform::desktop::desktop_account`]。会话是本 Windows 用户 `%LOCALAPPDATA%\<本版本的用户目录>\account` 下的 `account-session.json`（full 是 `%LOCALAPPDATA%\MSIME\account`），也就是 Server 注册本机匿名账号的目录（`server_main.cpp` 的 `anonymous_account_directory`）。输入法进程经 host C ABI `msime_client_account_access_token` 读同一份文件、拿同一把 `account-refresh.lock`，与 macOS 上设置应用和输入法共用 Application Support 里那份会话是同一个做法，两个进程不会用对方已经用掉的刷新令牌。目录在用户配置文件里，只有本用户能访问；安装器可能放在全机共享位置的状态目录不存放凭据。
//!
//! 以前的版本把会话放在设置应用自己的 `%LOCALAPPDATA%\<tauri identifier>`。启动时新目录还没有会话而旧目录有，就把旧文件搬过来，用户不用重新登录。

use crate::platform::desktop::{desktop_account, desktop_cloud_dictionary};
use msime_client_core::account::{AccountSessionFileLayout, FileAccountSessionStorage};
use tauri::Manager;

/// 在 Server 也读的那份本用户会话文件上注册桌面账号状态，以及经这个会话服务云词库面板的状态。
pub fn setup(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let identity = msime_client_core::edition::Edition::windows_package_identity()?;
    let directory = app
        .path()
        .local_data_dir()?
        .join(&identity.user_data_directory)
        .join("account");
    let local = app.path().app_local_data_dir()?;
    desktop_account::adopt_legacy_session(&local, &directory);
    app.manage(desktop_cloud_dictionary::CloudDictionaryState::new(
        local.join("dictionary-snapshots"),
    ));
    desktop_account::manage(
        app,
        FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Native),
    )
}
