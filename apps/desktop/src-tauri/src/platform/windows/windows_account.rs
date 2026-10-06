//! Windows desktop account session storage.
//!
//! The commands themselves live in [`crate::platform::desktop::desktop_account`]. The session is an `account-session.json` in this user's local application data directory, which the profile's ACL keeps from other accounts; the input method's state directory is not used, because the installer can place that one machine-wide.

use crate::platform::desktop::desktop_account;
use msime_client_core::account::{AccountSessionFileLayout, FileAccountSessionStorage};
use tauri::Manager;

/// Registers the shared desktop account state around the per-user session file.
pub fn setup(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let directory = app.path().app_local_data_dir()?;
    desktop_account::manage(
        app,
        FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Native),
    )
}
