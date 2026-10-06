//! Linux desktop account session storage.
//!
//! The commands themselves are the same ones the Windows and macOS shells register, shared in [`crate::platform::desktop::desktop_account`]; only where the session file lives differs. Linux has no single secret service every target desktop is guaranteed to run, so this host keeps the session in an owner-only file inside the shared state directory - the same rule the Linux provider services already state for their credential files, and the same 0700 directory `msime-linux-prepare` publishes.
//!
//! The React surface receives the same redacted DTOs as every other host: the tokens never leave this process.

use crate::platform::desktop::desktop_account;
use msime_client_core::account::{AccountSessionFileLayout, FileAccountSessionStorage};
use std::path::Path;

/// Registers the shared desktop account state around the session file in `directory`, the resolved shared state directory.
pub fn setup(app: &tauri::AppHandle, directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    desktop_account::manage(
        app,
        FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Native),
    )
}
