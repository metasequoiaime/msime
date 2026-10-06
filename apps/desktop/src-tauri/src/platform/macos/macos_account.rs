//! macOS desktop account session storage.
//!
//! The commands themselves live in [`crate::platform::desktop::desktop_account`]. The session is `account-session.json` in the input method's own Application Support directory, in the Swift backend's layout: the input method signs in and refreshes through `shared/backend` against the same file and the same lock, so the two processes never refresh from a token the other has already spent.

use crate::platform::desktop::desktop_account;
use crate::platform::macos::macos_launch;
use msime_client_core::account::{AccountSessionFileLayout, FileAccountSessionStorage};

/// Registers the shared desktop account state around the session file the input method also uses.
pub fn setup(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let directory = macos_launch::native_locator_root()?;
    desktop_account::manage(
        app,
        FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Apple),
    )
}
