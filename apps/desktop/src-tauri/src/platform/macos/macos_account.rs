//! macOS desktop account session storage backed by the Swift backend's Keychain item.
//!
//! The commands themselves live in [`crate::platform::desktop::desktop_account`].

use crate::platform::desktop::desktop_account;
use msime_client_core::account::{
    AccountError, AccountSessionStorage, AccountTokens, SavedAccountSession,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct SwiftSavedSession {
    tokens: AccountTokens,
    #[serde(rename = "expiresAt")]
    expires_at: f64,
}

#[derive(Clone, Copy)]
pub(crate) struct MacosAccountStorage;

impl AccountSessionStorage for MacosAccountStorage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        let Some(bytes) = msime_host_macos::account_load().map_err(|_| AccountError::Storage)?
        else {
            return Ok(None);
        };
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| AccountError::Storage)?;
        if value.get("expiresAt").is_some() {
            let saved: SwiftSavedSession =
                serde_json::from_value(value).map_err(|_| AccountError::Storage)?;
            let expires_at_unix_ms = (saved.expires_at + 978_307_200.0) * 1000.0;
            if !expires_at_unix_ms.is_finite() || expires_at_unix_ms < 0.0 {
                return Err(AccountError::Storage);
            }
            return Ok(Some(SavedAccountSession {
                tokens: saved.tokens,
                expires_at_unix_ms: expires_at_unix_ms.round() as u64,
            }));
        }
        serde_json::from_value(value)
            .map(Some)
            .map_err(|_| AccountError::Storage)
    }

    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        let expires_at = session.expires_at_unix_ms as f64 / 1000.0 - 978_307_200.0;
        let value = serde_json::json!({ "tokens": session.tokens, "expiresAt": expires_at });
        let bytes = serde_json::to_vec(&value).map_err(|_| AccountError::Storage)?;
        msime_host_macos::account_save(&bytes).map_err(|_| AccountError::Storage)
    }

    fn clear(&self) -> Result<(), AccountError> {
        msime_host_macos::account_clear().map_err(|_| AccountError::Storage)
    }
}

/// Registers the shared desktop account state around the Keychain-backed store.
pub fn setup(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    desktop_account::manage(app, MacosAccountStorage)
}
