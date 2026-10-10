//! The clipboard history panel: reading the host clipboard, keeping the bounded
//! store in step with it, and writing a chosen entry back.
//!
//! The store is only ever consulted when the user has the feature switched on,
//! so a disabled history never touches the clipboard at all.

#[cfg(target_os = "android")]
use crate::platform::android::android_account;
#[cfg(target_os = "linux")]
use crate::platform::linux::linux_clipboard;
use crate::{
    clipboard_history_uses_preference, host_platform, ClipboardHistoryState, HostActionError,
};
use msime_client_core::clipboard::ClipboardHistoryEntry;
#[cfg(target_os = "linux")]
use msime_client_core::clipboard::ClipboardHistoryStore;
use msime_client_core::preferences::PreferencesStore;
#[cfg(target_os = "ios")]
use msime_tauri_mobile_platform::MobilePlatform;
use std::sync::Arc;
#[cfg(target_os = "linux")]
use std::sync::Mutex;

pub(crate) fn clipboard_enabled(
    store: &std::sync::Arc<PreferencesStore>,
) -> Result<bool, HostActionError> {
    store
        .load()
        .map(|snapshot| snapshot.preferences.clipboard_history)
        .map_err(|_| HostActionError {
            code: "unavailable",
        })
}

#[cfg(target_os = "linux")]
/// `Ok(None)` means a password manager marked the copy secret, so there is
/// nothing to capture.
pub(crate) fn linux_clipboard_text() -> Result<Option<String>, HostActionError> {
    type Arguments = &'static [&'static str];
    let mut commands: Vec<(&str, Arguments, Option<Arguments>)> = Vec::with_capacity(3);
    // 合成器不提供 data-control 时 `wl-paste` 要抢焦点才能读，改经 XWayland 的 X11 剪贴板读取，见 `wayland_paste_is_background`。
    if std::env::var_os("WAYLAND_DISPLAY").is_some_and(|value| !value.is_empty())
        && linux_clipboard::wayland_paste_is_background()
    {
        commands.push((
            "wl-paste",
            &["--no-newline", "--type", "text"],
            Some(&["--list-types"]),
        ));
    }
    if std::env::var_os("DISPLAY").is_some_and(|value| !value.is_empty()) {
        commands.push((
            "xclip",
            &["-selection", "clipboard", "-o"],
            Some(&["-selection", "clipboard", "-o", "-t", "TARGETS"]),
        ));
        // xsel cannot list targets, so it is unfiltered. It is only reached
        // when xclip could not read the clipboard at all.
        commands.push(("xsel", &["--clipboard", "--output"], None));
    }
    // Check the targets before reading so a secret never enters this process.
    // The clipboard can change in between; the next poll corrects that. A
    // listing that fails is not treated as secret, or history would stop
    // working with tools that cannot list.
    // Preserve source line endings; wl-paste suppresses its own separator.
    for (program, arguments, targets) in commands {
        if targets.is_some_and(|targets| linux_clipboard::offers_secret(program, targets)) {
            return Ok(None);
        }
        if let Some(text) = linux_clipboard::read_text(program, arguments) {
            return Ok(Some(text));
        }
    }
    Err(HostActionError {
        code: "unavailable",
    })
}

#[cfg(target_os = "linux")]
pub(crate) fn write_linux_clipboard(text: &str) -> bool {
    if std::env::var_os("WAYLAND_DISPLAY").is_some_and(|value| !value.is_empty())
        && linux_clipboard::write_text("wl-copy", &["--type", "text/plain;charset=utf-8"], text)
    {
        return true;
    }
    if std::env::var_os("DISPLAY").is_some_and(|value| !value.is_empty()) {
        return linux_clipboard::write_text("xclip", &["-selection", "clipboard"], text)
            || linux_clipboard::write_text("xsel", &["--clipboard", "--input"], text);
    }
    false
}

#[cfg(target_os = "linux")]
pub(crate) fn start_linux_clipboard_monitor(
    history: Arc<Mutex<ClipboardHistoryStore>>,
    preferences: Arc<PreferencesStore>,
) {
    let _ = std::thread::Builder::new()
        .name("msime-clipboard-monitor".to_owned())
        .spawn(move || {
            let mut last_text = None;
            loop {
                let enabled = preferences
                    .load()
                    .map(|snapshot| snapshot.preferences.clipboard_history)
                    .unwrap_or(false);
                if !enabled {
                    last_text = None;
                } else if let Ok(Some(text)) = linux_clipboard_text() {
                    if last_text.as_deref() != Some(text.as_str()) {
                        match preferences.capture_clipboard_text(text.clone()) {
                            Ok(true) => {
                                if let Ok(mut store) = history.lock() {
                                    let _ = store.load();
                                }
                                last_text = Some(text);
                            }
                            Ok(false) => last_text = None,
                            Err(_) => {}
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(750));
            }
        });
}

#[tauri::command]
pub(crate) async fn list_clipboard_history(
    state: tauri::State<'_, ClipboardHistoryState>,
    store: tauri::State<'_, std::sync::Arc<PreferencesStore>>,
) -> Result<Vec<ClipboardHistoryEntry>, HostActionError> {
    let state = state.inner().clone();
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || list_clipboard_history_blocking(&state, &store))
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
}

pub(crate) fn list_clipboard_history_blocking(
    state: &ClipboardHistoryState,
    store: &Arc<PreferencesStore>,
) -> Result<Vec<ClipboardHistoryEntry>, HostActionError> {
    if clipboard_history_uses_preference(host_platform()) && !clipboard_enabled(store)? {
        return Ok(Vec::new());
    }
    let mut history = state.0.lock().map_err(|_| HostActionError {
        code: "unavailable",
    })?;
    history.load().map_err(|_| HostActionError {
        code: "unavailable",
    })?;
    Ok(history.entries().to_vec())
}

#[tauri::command]
pub(crate) async fn remove_clipboard_history(
    text: String,
    state: tauri::State<'_, ClipboardHistoryState>,
) -> Result<(), HostActionError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .0
            .lock()
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?
            .remove(&text)
            .map(|_| ())
            .map_err(|_| HostActionError {
                code: "unavailable",
            })
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })?
}

#[tauri::command]
pub(crate) async fn set_clipboard_history_pinned(
    text: String,
    pinned: bool,
    state: tauri::State<'_, ClipboardHistoryState>,
) -> Result<(), HostActionError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .0
            .lock()
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?
            .set_pinned(&text, pinned)
            .map(|_| ())
            .map_err(|_| HostActionError {
                code: "unavailable",
            })
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })?
}

#[tauri::command]
pub(crate) async fn clear_clipboard_history(
    state: tauri::State<'_, ClipboardHistoryState>,
) -> Result<(), HostActionError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || clear_clipboard_history_blocking(&state))
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
}

pub(crate) fn clear_clipboard_history_blocking(
    state: &ClipboardHistoryState,
) -> Result<(), HostActionError> {
    state
        .0
        .lock()
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
        .clear()
        .map_err(|_| HostActionError {
            code: "unavailable",
        })
}

#[tauri::command]
pub(crate) async fn sync_clipboard_history(
    state: tauri::State<'_, ClipboardHistoryState>,
    store: tauri::State<'_, std::sync::Arc<PreferencesStore>>,
) -> Result<Vec<ClipboardHistoryEntry>, HostActionError> {
    let state = state.inner().clone();
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || sync_clipboard_history_blocking(&state, &store))
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
}

pub(crate) fn sync_clipboard_history_blocking(
    state: &ClipboardHistoryState,
    store: &Arc<PreferencesStore>,
) -> Result<Vec<ClipboardHistoryEntry>, HostActionError> {
    if !clipboard_enabled(store)? {
        return Err(HostActionError { code: "disabled" });
    }
    // `None` is a clipboard with nothing capturable, such as a password
    // manager's secret copy: the panel still gets the current history.
    // The native macOS read is the monitor's: bounded, and it refuses concealed,
    // transient and password-manager pasteboards, which pbpaste would not.
    #[cfg(target_os = "macos")]
    let text = msime_host_macos::clipboard_snapshot(true)
        .ok_or(HostActionError {
            code: "unavailable",
        })?
        .text;
    #[cfg(target_os = "linux")]
    let text = linux_clipboard_text()?;
    #[cfg(target_os = "windows")]
    let text = Some(
        msime_host_windows::read_clipboard_text()
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?
            .trim_end_matches(['\r', '\n'])
            .to_owned(),
    );
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let text: Option<String> = Err(HostActionError {
        code: "unavailable",
    })?;
    if let Some(text) = text {
        store
            .capture_clipboard_text(text)
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?;
    }
    let mut history = state.0.lock().map_err(|_| HostActionError {
        code: "unavailable",
    })?;
    history.load().map_err(|_| HostActionError {
        code: "unavailable",
    })?;
    Ok(history.entries().to_vec())
}

pub(crate) async fn copy_text_impl(
    text: String,
    state: tauri::State<'_, ClipboardHistoryState>,
    store: tauri::State<'_, std::sync::Arc<PreferencesStore>>,
    #[cfg(target_os = "android")] account: tauri::State<'_, android_account::AccountState>,
    #[cfg(target_os = "ios")] platform: tauri::State<'_, MobilePlatform<tauri::Wry>>,
) -> Result<(), HostActionError> {
    // Clipboard writes are shared by Emoji, handwriting and voice panels. Keep
    // the same bounded text contract as paste/submit so a panel cannot make an
    // unbounded system-clipboard or history update on any desktop host.
    if !clipboard_text_is_valid(&text) {
        return Err(HostActionError {
            code: "invalid_text",
        });
    }
    let state = state.inner().clone();
    let store = store.inner().clone();
    #[cfg(target_os = "android")]
    {
        if text.is_empty()
            || !msime_client_core::is_bounded_utf16(&text, 4000)
            || text.contains('\0')
        {
            return Err(HostActionError {
                code: "invalid_text",
            });
        }
        let plugin = account.platform.clone();
        let clipboard_text = text.clone();
        tauri::async_runtime::spawn_blocking(move || {
            plugin
                .run_mobile_plugin::<()>("copyText", serde_json::json!({ "text": clipboard_text }))
                .map_err(|_| HostActionError {
                    code: "unavailable",
                })
        })
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })??;
        if clipboard_enabled(&store)? {
            store
                .capture_clipboard_text(text)
                .map_err(|_| HostActionError {
                    code: "unavailable",
                })?;
            state
                .0
                .lock()
                .map_err(|_| HostActionError {
                    code: "unavailable",
                })?
                .load()
                .map_err(|_| HostActionError {
                    code: "unavailable",
                })?;
        }
        return Ok(());
    }
    #[cfg(target_os = "ios")]
    {
        let platform = platform.inner().clone();
        let clipboard_text = text.clone();
        tauri::async_runtime::spawn_blocking(move || {
            platform
                .copy_text(&clipboard_text)
                .map_err(|_| HostActionError {
                    code: "unavailable",
                })
        })
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })??;
        if clipboard_enabled(&store)? {
            store
                .capture_clipboard_text(text)
                .map_err(|_| HostActionError {
                    code: "unavailable",
                })?;
            state
                .0
                .lock()
                .map_err(|_| HostActionError {
                    code: "unavailable",
                })?
                .load()
                .map_err(|_| HostActionError {
                    code: "unavailable",
                })?;
        }
        return Ok(());
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        tauri::async_runtime::spawn_blocking(move || copy_text_blocking(text, &state, &store))
            .await
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?
    }
}

pub(crate) fn clipboard_text_is_valid(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= msime_client_core::clipboard::MAX_TEXT_BYTES
        && !text.contains('\0')
}

#[cfg(target_os = "android")]
#[tauri::command]
pub(crate) async fn copy_text(
    text: String,
    state: tauri::State<'_, ClipboardHistoryState>,
    store: tauri::State<'_, std::sync::Arc<PreferencesStore>>,
    account: tauri::State<'_, android_account::AccountState>,
) -> Result<(), HostActionError> {
    copy_text_impl(text, state, store, account).await
}

#[cfg(target_os = "ios")]
#[tauri::command]
pub(crate) async fn copy_text(
    text: String,
    state: tauri::State<'_, ClipboardHistoryState>,
    store: tauri::State<'_, std::sync::Arc<PreferencesStore>>,
    platform: tauri::State<'_, MobilePlatform<tauri::Wry>>,
) -> Result<(), HostActionError> {
    copy_text_impl(text, state, store, platform).await
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tauri::command]
pub(crate) async fn copy_text(
    text: String,
    state: tauri::State<'_, ClipboardHistoryState>,
    store: tauri::State<'_, std::sync::Arc<PreferencesStore>>,
) -> Result<(), HostActionError> {
    copy_text_impl(text, state, store).await
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) fn copy_text_blocking(
    text: String,
    state: &ClipboardHistoryState,
    store: &Arc<PreferencesStore>,
) -> Result<(), HostActionError> {
    let enabled = clipboard_enabled(store)?;
    #[cfg(target_os = "macos")]
    let result = {
        use std::io::Write;
        let mut child = std::process::Command::new("pbcopy")
            // A Finder-launched app has no LANG, and pbcopy would then decode
            // the UTF-8 input in the legacy encoding and garble CJK text.
            .env("LC_ALL", "en_US.UTF-8")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?;
        // stdin is dropped inside the closure, before the wait.
        let written = child
            .stdin
            .take()
            .is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
        if !written {
            let _ = child.kill();
        }
        let exited = child.wait().is_ok_and(|status| status.success());
        written && exited
    };
    #[cfg(target_os = "linux")]
    let result = write_linux_clipboard(&text);
    #[cfg(target_os = "windows")]
    let result = msime_host_windows::write_clipboard_text(&text);
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let result = false;
    if !result {
        return Err(HostActionError {
            code: "unavailable",
        });
    }
    if enabled {
        store
            .capture_clipboard_text(text)
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?;
        state
            .0
            .lock()
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?
            .load()
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?;
    }
    Ok(())
}
