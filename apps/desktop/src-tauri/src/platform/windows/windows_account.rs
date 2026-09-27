//! Windows desktop account session storage.
//!
//! Session tokens stay in the per-user Windows Credential Manager. The commands themselves live in [`crate::platform::desktop::desktop_account`], and the React surface only receives the same redacted DTOs as the mobile hosts.

use crate::platform::desktop::desktop_account;
use msime_client_core::account::{AccountError, AccountSessionStorage, SavedAccountSession};

#[derive(Clone, Copy)]
pub(crate) struct WindowsAccountStorage;

impl AccountSessionStorage for WindowsAccountStorage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        msime_host_windows::load_account_session()
            .map_err(|_| AccountError::Storage)?
            .map(|value| serde_json::from_str(&value).map_err(|_| AccountError::Storage))
            .transpose()
    }

    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        let value = serde_json::to_string(session).map_err(|_| AccountError::Storage)?;
        msime_host_windows::save_account_session(Some(&value)).map_err(|_| AccountError::Storage)
    }

    fn clear(&self) -> Result<(), AccountError> {
        msime_host_windows::save_account_session(None).map_err(|_| AccountError::Storage)
    }
}

/// Registers the shared desktop account state around the Credential Manager store.
pub fn setup(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    desktop_account::manage(app, WindowsAccountStorage)
}
