//! The desktop clipboard history the 剪贴板 settings page manages: `clipboard_history.json` in the preferences directory, kept by [`ClipboardHistoryStore`] and captured only while the `clipboard_history` preference is on.
//!
//! Tauri 设置外壳（Windows、macOS、Linux）的共享剪贴板页在历史开关保存为开启后调用一次 `sync_clipboard_history`；Windows 原生设置窗口经 `msime_client_desktop_clipboard_history` 在同样的时机发送 `sync`。两者都通过 [`capture_clipboard`] 采集，统一保证历史关闭时不读剪贴板；Windows 上都经 [`windows_clipboard_text`] 读取。移动端历史（`msime_client_mobile_clipboard_history`）是 App Group 下的另一个文件，这里不处理。

use msime_client_core::clipboard::{ClipboardHistoryEntry, ClipboardHistoryStore};
use msime_client_core::preferences::PreferencesStore;

/// Why a history operation did not complete. The codes are the ones the shared settings page already receives from the Tauri shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardHistoryError {
    /// The history is switched off, so nothing is read or captured.
    Disabled,
    /// The preferences, the history file or the clipboard could not be read or written.
    Unavailable,
}

impl ClipboardHistoryError {
    pub fn code(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Whether the saved preferences have the history switched on.
pub fn enabled(preferences: &PreferencesStore) -> Result<bool, ClipboardHistoryError> {
    preferences
        .load()
        .map(|snapshot| snapshot.preferences.clipboard_history)
        .map_err(|_| ClipboardHistoryError::Unavailable)
}

/// The saved entries, newest and pinned first, re-read from disk so another process's captures show.
pub fn entries(
    history: &mut ClipboardHistoryStore,
) -> Result<Vec<ClipboardHistoryEntry>, ClipboardHistoryError> {
    history
        .load()
        .map_err(|_| ClipboardHistoryError::Unavailable)?;
    Ok(history.entries().to_vec())
}

/// 从系统剪贴板同步: only while the history is on, reads the clipboard through `read` and captures what it returns under the preferences lock. `Ok(None)` from `read` is a clipboard with nothing capturable, such as a password manager's secret copy; that is not an error, the page still gets the current history.
pub fn capture_clipboard<E>(
    preferences: &PreferencesStore,
    read: impl FnOnce() -> Result<Option<String>, E>,
) -> Result<(), ClipboardHistoryError> {
    if !enabled(preferences)? {
        return Err(ClipboardHistoryError::Disabled);
    }
    if let Some(text) = read().map_err(|_| ClipboardHistoryError::Unavailable)? {
        preferences
            .capture_clipboard_text(text)
            .map_err(|_| ClipboardHistoryError::Unavailable)?;
    }
    Ok(())
}

/// The Windows clipboard text as the history keeps it: the trailing line break a copied line carries is dropped. The read refuses a clipboard that carries the privacy markers password managers set (`ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory=0` and the rest), the same list the Server's capture skips, and answers `Ok(None)` for it.
#[cfg(windows)]
pub fn windows_clipboard_text() -> Result<Option<String>, ()> {
    msime_host_windows::read_clipboard_text()
        .map(|text| text.map(|text| text.trim_end_matches(['\r', '\n']).to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_core::preferences::Preferences;

    fn store(directory: &std::path::Path, enabled: bool) -> PreferencesStore {
        let store = PreferencesStore::new(directory);
        store
            .save(
                0,
                Preferences {
                    clipboard_history: enabled,
                    ..Preferences::default()
                },
            )
            .unwrap();
        store
    }

    #[test]
    fn a_disabled_history_never_reads_the_clipboard() {
        let directory = tempfile::tempdir().unwrap();
        let preferences = store(directory.path(), false);
        let result = capture_clipboard(&preferences, || -> Result<Option<String>, ()> {
            panic!("the clipboard was read while the history is off")
        });
        assert_eq!(result, Err(ClipboardHistoryError::Disabled));
        assert!(!directory.path().join("clipboard_history.json").exists());
    }

    #[test]
    fn an_enabled_history_captures_the_clipboard_text_and_lists_it() {
        let directory = tempfile::tempdir().unwrap();
        let preferences = store(directory.path(), true);
        capture_clipboard(&preferences, || Ok::<_, ()>(Some("合成剪贴板".to_owned()))).unwrap();
        let mut history =
            ClipboardHistoryStore::open(directory.path().join("clipboard_history.json"));
        let listed = entries(&mut history).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].text, "合成剪贴板");
        assert!(!listed[0].pinned);
    }

    #[test]
    fn a_secret_clipboard_is_skipped_and_a_failed_read_is_unavailable() {
        let directory = tempfile::tempdir().unwrap();
        let preferences = store(directory.path(), true);
        capture_clipboard(&preferences, || Ok::<_, ()>(None)).unwrap();
        let mut history =
            ClipboardHistoryStore::open(directory.path().join("clipboard_history.json"));
        assert!(entries(&mut history).unwrap().is_empty());
        assert_eq!(
            capture_clipboard(&preferences, || Err::<Option<String>, _>(())),
            Err(ClipboardHistoryError::Unavailable)
        );
    }
}
