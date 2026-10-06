//! The console's app notices for the settings window (`GET /v1/notices?channel=app`).
//!
//! The settings window asks for the feed when it opens; the input method process never does. Fetching happens here rather than in the webview because the API refuses browser origins it does not list, and the cache and dismissed ids live in `notices.json` in the shared state directory, through [`NoticeStore`], so a refetch never happens more often than the server's one-minute cache allows. The page renders each body's Markdown itself, with raw HTML disabled.

use crate::CommandError;
use msime_client_core::account::BackendAccountClient;
use msime_client_core::notices::{fetch_notices, Notice, NoticeChannel, NoticeError, NoticeStore};
use std::path::PathBuf;
use std::time::SystemTime;

/// The directory `notices.json` lives in.
pub(crate) struct NoticesState(pub(crate) PathBuf);

/// The canonical platform id the feed is asked for.
pub(crate) const PLATFORM: &str = if cfg!(target_os = "windows") {
    "windows"
} else if cfg!(target_os = "macos") {
    "macos"
} else if cfg!(target_os = "android") {
    "android"
} else if cfg!(target_os = "ios") {
    "ios"
} else {
    "linux"
};

fn notice_error(error: NoticeError) -> CommandError {
    CommandError {
        code: match error {
            NoticeError::Invalid => "notice_invalid",
            NoticeError::Storage => "notice_storage",
        },
    }
}

fn store(state: &tauri::State<'_, NoticesState>) -> NoticeStore {
    NoticeStore::new(state.0.clone())
}

/// The live notices the user has not dismissed, newest first. A failed fetch falls back to the cached feed, so the page only sees an error when the cache cannot be written.
#[tauri::command]
pub async fn notices_list(
    state: tauri::State<'_, NoticesState>,
) -> Result<Vec<Notice>, CommandError> {
    let store = store(&state);
    tauri::async_runtime::spawn_blocking(move || {
        store.current(NoticeChannel::App, PLATFORM, SystemTime::now(), || {
            let client = BackendAccountClient::new()?;
            fetch_notices(&client, NoticeChannel::App, PLATFORM)
        })
    })
    .await
    .map_err(|_| CommandError {
        code: "notice_storage",
    })?
    .map_err(notice_error)
}

/// Remembers that the user closed notice `id`, so it is not shown again on this device.
#[tauri::command]
pub async fn notice_dismiss(
    state: tauri::State<'_, NoticesState>,
    id: String,
) -> Result<(), CommandError> {
    let store = store(&state);
    tauri::async_runtime::spawn_blocking(move || store.dismiss(&id))
        .await
        .map_err(|_| CommandError {
            code: "notice_storage",
        })?
        .map_err(notice_error)
}
