//! The clipboard history panel: reading the host clipboard, keeping the bounded
//! store in step with it, and writing a chosen entry back.
//!
//! The store is only ever consulted when the user has the feature switched on,
//! so a disabled history never touches the clipboard at all.

#[cfg(target_os = "android")]
use crate::platform::android::android_account;
#[cfg(target_os = "linux")]
use crate::platform::linux::{linux_clipboard, linux_process};
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
/// `Ok(None)` means there is no text to capture: a password manager marked the copy secret, or a reader ran but the clipboard is empty or holds no text (an image, files, nothing copied since login). `Err` means no reader program for this session is installed, see [`clipboard_without_text`].
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
    let reader_installed = commands
        .iter()
        .any(|(program, ..)| linux_process::program_available(program));
    for (program, arguments, targets) in commands {
        if targets.is_some_and(|targets| linux_clipboard::offers_secret(program, targets)) {
            return Ok(None);
        }
        if let Some(text) = linux_clipboard::read_text(program, arguments) {
            return Ok(Some(text));
        }
    }
    clipboard_without_text(reader_installed)
}

/// 本会话可用的读取程序都没读到文本时的结果。`wl-paste`、`xclip` 在剪贴板为空或只有图片、文件时也以非零状态退出，与没装它们在 `read_text` 看来一样，所以按有没有装来分：装了其中任何一个，读取本身是通的，只是这时没有文本，按没有内容处理（`Ok(None)`），采集照常；一个都没装（或会话里没有能用的显示）才是这个进程读不了剪贴板，面板的状态行据此按 `unreadable_clipboard_status` 说明。
#[cfg(target_os = "linux")]
pub(crate) fn clipboard_without_text(
    reader_installed: bool,
) -> Result<Option<String>, HostActionError> {
    if reader_installed {
        Ok(None)
    } else {
        Err(HostActionError {
            code: "unavailable",
        })
    }
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

/// 剪贴板采集最近一轮的结果，给面板的状态行用，对应 macOS 原生剪贴板页 `MacClipboardMonitor.Status` 的四种状态；Linux 多出读不到系统剪贴板的三种，按该装什么来分（见 `unreadable_clipboard_status`）。
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ClipboardCaptureStatus {
    Disabled,
    Monitoring,
    Unavailable,
    /// 这个会话里没有能读的剪贴板：没有显示，或只有不提供 data-control 的 Wayland 合成器而没有 XWayland，装什么程序都没用。
    ReadUnavailable,
    /// 有 X11 显示（含 XWayland），装上 xclip 或 xsel 就能读。
    ReadUnavailableX11,
    /// 只有 Wayland 显示且没装 wl-paste，装上 wl-clipboard 就能读。
    ReadUnavailableWayland,
    SaveFailed,
}

#[cfg(target_os = "linux")]
impl ClipboardCaptureStatus {
    const fn code(self) -> u8 {
        match self {
            Self::Disabled => 1,
            Self::Monitoring => 2,
            Self::Unavailable => 3,
            Self::ReadUnavailable => 4,
            Self::SaveFailed => 5,
            Self::ReadUnavailableX11 => 6,
            Self::ReadUnavailableWayland => 7,
        }
    }

    fn from_code(code: u8) -> Option<Self> {
        [
            Self::Disabled,
            Self::Monitoring,
            Self::Unavailable,
            Self::ReadUnavailable,
            Self::ReadUnavailableX11,
            Self::ReadUnavailableWayland,
            Self::SaveFailed,
        ]
        .into_iter()
        .find(|status| status.code() == code)
    }
}

/// 监视线程写、`clipboard_capture_status` 读的最近状态；0 表示还没跑完第一轮。
#[cfg(target_os = "linux")]
#[derive(Clone, Default)]
pub(crate) struct ClipboardCaptureState(Arc<std::sync::atomic::AtomicU8>);

#[cfg(target_os = "linux")]
impl ClipboardCaptureState {
    /// 记下这一轮的状态，返回它是否与上一轮不同。
    pub(crate) fn update(&self, status: ClipboardCaptureStatus) -> bool {
        self.0
            .swap(status.code(), std::sync::atomic::Ordering::AcqRel)
            != status.code()
    }

    pub(crate) fn current(&self) -> Option<ClipboardCaptureStatus> {
        ClipboardCaptureStatus::from_code(self.0.load(std::sync::atomic::Ordering::Acquire))
    }
}

/// 一轮采集的状态：偏好读不出来时暂停；关闭历史时停用；没有能读剪贴板的程序时报读取不可用（监视线程随后按 `unreadable_clipboard_status` 细分）；剪贴板里没有文本或是密码管理器标了保密的内容（`Ok(None)`）也算在正常采集；保存失败下一轮重试。
#[cfg(target_os = "linux")]
pub(crate) fn clipboard_capture_round(
    preferences_readable: bool,
    enabled: bool,
    read: &Result<Option<String>, HostActionError>,
    saved: Option<bool>,
) -> ClipboardCaptureStatus {
    if !preferences_readable {
        ClipboardCaptureStatus::Unavailable
    } else if !enabled {
        ClipboardCaptureStatus::Disabled
    } else if read.is_err() {
        ClipboardCaptureStatus::ReadUnavailable
    } else if saved == Some(false) {
        ClipboardCaptureStatus::SaveFailed
    } else {
        ClipboardCaptureStatus::Monitoring
    }
}

/// 设置程序自己读不到剪贴板时状态行怎么说。剪贴板历史在 Linux 上主要由用户服务 `<package>-clipboard.service`（`msime-linux-clipboard-monitor`）采集：它用随包的 `msime-linux-clipboard-watch-x11` 或 `wl-paste --watch`，不需要 xclip、xsel，首次配置会启用它。它在运行时采集照常，状态行说正在采集，不提示去装用不上的工具。服务没在运行时才按会话说该装什么：有 X11 显示（含 XWayland）时 xclip 或 xsel 一定能用；只有 Wayland 显示且没装 wl-paste 时是 wl-clipboard；其余情况（没有显示，或只有 Wayland 显示、装了 wl-paste 但合成器不提供 data-control，见 `wayland_paste_is_background`）装什么都读不到。
#[cfg(target_os = "linux")]
pub(crate) fn unreadable_clipboard_status(
    capture_service_active: bool,
    x11_display: bool,
    wayland_display: bool,
    wl_paste_installed: bool,
) -> ClipboardCaptureStatus {
    if capture_service_active {
        ClipboardCaptureStatus::Monitoring
    } else if x11_display {
        ClipboardCaptureStatus::ReadUnavailableX11
    } else if wayland_display && !wl_paste_installed {
        ClipboardCaptureStatus::ReadUnavailableWayland
    } else {
        ClipboardCaptureStatus::ReadUnavailable
    }
}

/// 隔多久再问一次 systemd 剪贴板服务是否在运行；只在这个进程读不到剪贴板时才问。
#[cfg(target_os = "linux")]
const CAPTURE_SERVICE_RECHECK: std::time::Duration = std::time::Duration::from_secs(30);

/// 本版本的剪贴板用户服务（full 是 `msime-linux-clipboard.service`）是否在运行。
#[cfg(target_os = "linux")]
fn capture_service_active() -> bool {
    let unit = msime_client_core::edition::Edition::linux_package_identity_or_full()
        .user_unit("clipboard.service");
    linux_process::run_status(
        "systemctl",
        &["--user", "--quiet", "is-active", &unit],
        std::time::Duration::from_secs(2),
    )
}

/// 本进程读不到剪贴板时的状态行，见 `unreadable_clipboard_status`。服务状态最多每 `CAPTURE_SERVICE_RECHECK` 问一次。
#[cfg(target_os = "linux")]
fn session_unreadable_status(
    service: &mut Option<(std::time::Instant, bool)>,
) -> ClipboardCaptureStatus {
    let active = match *service {
        Some((checked, active)) if checked.elapsed() < CAPTURE_SERVICE_RECHECK => active,
        _ => {
            let active = capture_service_active();
            *service = Some((std::time::Instant::now(), active));
            active
        }
    };
    let set = |name: &str| std::env::var_os(name).is_some_and(|value| !value.is_empty());
    unreadable_clipboard_status(
        active,
        set("DISPLAY"),
        set("WAYLAND_DISPLAY"),
        linux_process::program_available("wl-paste"),
    )
}

#[cfg(target_os = "linux")]
pub(crate) fn start_linux_clipboard_monitor(
    history: Arc<Mutex<ClipboardHistoryStore>>,
    preferences: Arc<PreferencesStore>,
    capture: ClipboardCaptureState,
    on_status_changed: impl Fn() + Send + 'static,
) {
    let _ = std::thread::Builder::new()
        .name("msime-clipboard-monitor".to_owned())
        .spawn(move || {
            let mut last_text = None;
            // 上次问 systemd 剪贴板服务的时间和结果，见 `session_unreadable_status`。
            let mut service = None;
            loop {
                let snapshot = preferences.load().ok();
                let enabled = snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.preferences.clipboard_history);
                if let Some(snapshot) = snapshot.as_ref() {
                    if let Ok(mut store) = history.lock() {
                        store.set_limit(snapshot.preferences.clipboard_history_limit);
                    }
                }
                let mut read = Ok(None);
                // `Some(false)` 是这一轮保存失败；没有新内容要保存时为 `None`。
                let mut saved = None;
                if !enabled {
                    last_text = None;
                } else {
                    read = linux_clipboard_text();
                    if let Ok(Some(text)) = &read {
                        if last_text.as_deref() != Some(text.as_str()) {
                            match preferences.capture_clipboard_text(text.clone()) {
                                Ok(true) => {
                                    if let Ok(mut store) = history.lock() {
                                        let _ = store.load();
                                    }
                                    last_text = Some(text.clone());
                                }
                                Ok(false) => last_text = None,
                                Err(_) => saved = Some(false),
                            }
                        }
                    }
                }
                let status =
                    match clipboard_capture_round(snapshot.is_some(), enabled, &read, saved) {
                        ClipboardCaptureStatus::ReadUnavailable => {
                            session_unreadable_status(&mut service)
                        }
                        status => status,
                    };
                if capture.update(status) {
                    on_status_changed();
                }
                std::thread::sleep(std::time::Duration::from_millis(750));
            }
        });
}

/// 剪贴板面板状态行要显示的采集状态；监视线程还没跑完第一轮时为 `None`。
#[cfg(target_os = "linux")]
#[tauri::command]
pub(crate) fn clipboard_capture_status(
    state: tauri::State<'_, ClipboardCaptureState>,
) -> Option<ClipboardCaptureStatus> {
    state.current()
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
    let limit = store
        .load()
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
        .preferences
        .clipboard_history_limit;
    history.set_limit(limit);
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
    // `None` is a clipboard with nothing capturable, such as a password
    // manager's secret copy: the panel still gets the current history.
    // The native macOS read is the monitor's: bounded, and it refuses concealed,
    // transient and password-manager pasteboards, which pbpaste would not.
    #[cfg(target_os = "macos")]
    let read = || {
        msime_host_macos::clipboard_snapshot(true)
            .map(|snapshot| snapshot.text)
            .ok_or(())
    };
    #[cfg(target_os = "linux")]
    let read = linux_clipboard_text;
    // Windows 的读取同样拒收带 ExcludeClipboardContentFromMonitorProcessing、CanIncludeInClipboardHistory=0 等隐私标记的剪贴板，与 Server 的采集用同一份名单；原生设置窗口的同步走同一个函数。
    #[cfg(target_os = "windows")]
    let read = msime_host_api::clipboard_history::windows_clipboard_text;
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let read = || Err::<Option<String>, ()>(());
    msime_host_api::clipboard_history::capture_clipboard(store, read)
        .map_err(|error| HostActionError { code: error.code() })?;
    let mut history = state.0.lock().map_err(|_| HostActionError {
        code: "unavailable",
    })?;
    let limit = store
        .load()
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
        .preferences
        .clipboard_history_limit;
    history.set_limit(limit);
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
        Ok(())
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
        Ok(())
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
