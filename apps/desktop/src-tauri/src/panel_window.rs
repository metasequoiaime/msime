//! Creating, positioning and closing the floating panel windows.
//!
//! Every panel is a borderless always-on-top webview whose placement follows the
//! caret the host reported. The keyboard panel is the one window that must never
//! take focus, because taking it would end the text client's composition.

#[cfg(target_os = "linux")]
use crate::panel_input::panel_position;
#[cfg(target_os = "windows")]
use crate::panel_input::windows_panel_position;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use crate::panel_input::{
    forget_cloud_clipboard_input_target, remember_opening_panel_target, CLOUD_CLIPBOARD_PANEL,
};
#[cfg(target_os = "macos")]
use crate::platform::macos::macos_keyboard;
#[cfg(target_os = "macos")]
use crate::platform::macos::macos_panel_session;
use crate::voice::cancel_voice;
use crate::{DictionaryHostOptions, HostActionError, PanelInputState};
use msime_client_core::host_surface::{PanelPlacement, PanelSurface, SurfaceRoute};
use tauri::Manager;
#[cfg(not(mobile))]
use tauri::{WebviewUrl, WebviewWindowBuilder};

pub(crate) fn panel_accepts_focus(label: &str) -> bool {
    label != "keyboard-panel"
}

/// The screen keyboard's height for the shared `touch_keyboard_height_adjustment`, the same `base + adjustment` (clamped to -12..=48) the settings preview draws, so the window matches what the slider showed.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn keyboard_panel_height(base: f64, adjustment: i8) -> f64 {
    base + f64::from(adjustment.clamp(-12, 48))
}

/// A panel's height on Windows, where the keyboard panel follows the saved height adjustment. Every other panel, and a document that cannot be read, keeps the surface's own height.
#[cfg(target_os = "windows")]
pub(crate) fn windows_panel_height(app: &tauri::AppHandle, label: &str, height: f64) -> f64 {
    if label != "keyboard-panel" {
        return height;
    }
    app.try_state::<std::sync::Arc<msime_client_core::preferences::PreferencesStore>>()
        .and_then(|store| store.load().ok())
        .map_or(height, |snapshot| {
            keyboard_panel_height(
                height,
                snapshot.preferences.touch_keyboard_height_adjustment,
            )
        })
}

#[cfg(target_os = "linux")]
pub(crate) fn visible_panel_position(
    window: &tauri::WebviewWindow,
    position: tauri::Position,
    width: f64,
    height: f64,
) -> tauri::Position {
    // X11 geometry is physical. Do not reinterpret Sway's logical coordinates
    // using a monitor scale factor from a different coordinate space.
    let tauri::Position::Physical(point) = position else {
        return position;
    };
    let Ok(monitors) = window.available_monitors() else {
        return position;
    };
    let x = f64::from(point.x);
    let y = f64::from(point.y);
    // panel_position used the logical requested width. Recover the editor's
    // physical center before choosing a monitor and applying its scale.
    let center_x = x + width / 2.0;
    let distance = |monitor: &tauri::Monitor| {
        let area = monitor.work_area();
        let left = f64::from(area.position.x);
        let top = f64::from(area.position.y);
        let dx = center_x - center_x.clamp(left, left + f64::from(area.size.width));
        let dy = y - y.clamp(top, top + f64::from(area.size.height));
        dx * dx + dy * dy
    };
    let Some(monitor) = monitors
        .iter()
        .filter(|monitor| monitor.work_area().size.width > 0 && monitor.work_area().size.height > 0)
        .min_by(|left, right| distance(left).total_cmp(&distance(right)))
    else {
        return position;
    };
    let scale = monitor.scale_factor();
    if !scale.is_finite() || scale <= 0.0 {
        return position;
    }
    let x = center_x - width * scale / 2.0;
    let area = monitor.work_area();
    let left = f64::from(area.position.x);
    let top = f64::from(area.position.y);
    let right = left + (f64::from(area.size.width) - width * scale).max(0.0);
    let bottom = top + (f64::from(area.size.height) - height * scale).max(0.0);
    tauri::Position::Physical(tauri::PhysicalPosition::new(
        x.clamp(left, right).round() as i32,
        y.clamp(top, bottom).round() as i32,
    ))
}

pub(crate) fn open_panel_window(
    app: &tauri::AppHandle,
    label: &'static str,
    route: &'static str,
    title: &'static str,
    width: f64,
    height: f64,
    position: Option<tauri::Position>,
) -> Result<(), HostActionError> {
    #[cfg(mobile)]
    {
        let _ = (app, label, route, title, width, height, position);
        Err(HostActionError {
            code: "unavailable",
        })
    }
    #[cfg(not(mobile))]
    {
        let accepts_focus = panel_accepts_focus(label);
        if let Some(window) = app.get_webview_window(label) {
            #[cfg(target_os = "macos")]
            if label == "keyboard-panel" {
                return macos_keyboard::show(&window).map_err(|_| HostActionError {
                    code: "unavailable",
                });
            }
            // The window is reused, so a height saved since it was created is applied here; the minimum goes first because it was created at the old height.
            #[cfg(target_os = "windows")]
            if label == "keyboard-panel" {
                let size = tauri::LogicalSize::new(width, height);
                let _ = window.set_min_size(Some(size));
                let _ = window.set_size(size);
            }
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            if let Some(position) = position {
                #[cfg(target_os = "linux")]
                let position = visible_panel_position(&window, position, width, height);
                let _ = window.set_position(position);
            }
            window
                .show()
                .and_then(|_| {
                    if accepts_focus {
                        window.set_focus()
                    } else {
                        Ok(())
                    }
                })
                .map_err(|_| HostActionError {
                    code: "unavailable",
                })?;
            return Ok(());
        }
        let builder = WebviewWindowBuilder::new(
            app,
            label,
            WebviewUrl::App(format!("index.html?panel={route}").into()),
        )
        .title(title);
        // A panel is shown as soon as it is positioned, which is well before its page paints. The
        // settings window carries the system theme, so the panel opens in the colour it is about
        // to paint rather than in the platform's white.
        let theme = app
            .get_webview_window("main")
            .and_then(|window| window.theme().ok());
        let window = builder
            .background_color(crate::chrome_background(theme))
            .inner_size(width, height)
            .visible(false)
            .focused(false)
            .focusable(accepts_focus)
            .min_inner_size(width, height)
            .resizable(false)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .build()
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?;
        if let Some(position) = position {
            #[cfg(target_os = "linux")]
            let position = visible_panel_position(&window, position, width, height);
            let _ = window.set_position(position);
        }
        #[cfg(target_os = "macos")]
        if label == "keyboard-panel" {
            return macos_keyboard::show(&window).map_err(|_| HostActionError {
                code: "unavailable",
            });
        }
        window
            .show()
            .and_then(|_| {
                if accepts_focus {
                    window.set_focus()
                } else {
                    Ok(())
                }
            })
            .map_err(|_| HostActionError {
                code: "unavailable",
            })
    }
}

/// This host's window geometry for a panel route, from the shared route table.
fn panel_surface(route: SurfaceRoute) -> Result<PanelSurface, HostActionError> {
    route
        .panel_for(crate::host_platform())
        .ok_or(HostActionError {
            code: "unavailable",
        })
}

/// 背单词 has no launch route, so its geometry lives here rather than in the shared route table.
pub(crate) const VOCABULARY_PANEL: PanelSurface = PanelSurface {
    label: "vocabulary-panel",
    query: "vocabulary",
    title: "水杉背单词",
    width: 560,
    height: 680,
    placement: PanelPlacement::BottomCenter,
};

/// Opens a panel route's window on this host. See [`open_surface_panel`].
pub(crate) fn open_route_panel(
    app: &tauri::AppHandle,
    state: &tauri::State<'_, PanelInputState>,
    route: SurfaceRoute,
) -> Result<(), HostActionError> {
    // 设置页的按钮和菜单都走这里：本版本没有的面板（不提供手写的版本里的手写面板）一律不开。
    if !crate::edition_offers_route(crate::package_edition(), route) {
        return Err(HostActionError {
            code: "unavailable",
        });
    }
    open_surface_panel(app, state, panel_surface(route)?)
}

/// Opens a panel window where this host places it. Linux and Windows first remember the window that owns the caret, because the panel never becomes the input target itself and synthetic input has to reach that window later; Windows also applies the keyboard panel's saved height.
pub(crate) fn open_surface_panel(
    app: &tauri::AppHandle,
    state: &tauri::State<'_, PanelInputState>,
    surface: PanelSurface,
) -> Result<(), HostActionError> {
    let (width, height) = (f64::from(surface.width), f64::from(surface.height));
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let _ = state;
    #[cfg(target_os = "linux")]
    let position = {
        remember_opening_panel_target(app, state, surface.label);
        panel_position(state, surface.label, width, height)
    };
    #[cfg(target_os = "windows")]
    let height = windows_panel_height(app, surface.label, height);
    #[cfg(target_os = "windows")]
    let position = {
        remember_opening_panel_target(app, state, surface.label);
        windows_panel_position(width, height, surface.placement)
    };
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let position = None;
    open_panel_window(
        app,
        surface.label,
        surface.query,
        surface.title,
        width,
        height,
        position,
    )
}

#[tauri::command]
pub(crate) fn open_keyboard_panel(
    app: tauri::AppHandle,
    state: tauri::State<'_, PanelInputState>,
) -> Result<(), HostActionError> {
    open_route_panel(&app, &state, SurfaceRoute::Keyboard)
}

#[tauri::command]
pub(crate) fn open_handwriting_panel(
    app: tauri::AppHandle,
    state: tauri::State<'_, PanelInputState>,
) -> Result<(), HostActionError> {
    open_route_panel(&app, &state, SurfaceRoute::Handwriting)
}

#[tauri::command]
pub(crate) fn open_emoji_panel(
    app: tauri::AppHandle,
    options: tauri::State<'_, DictionaryHostOptions>,
    input: tauri::State<'_, PanelInputState>,
) -> Result<(), HostActionError> {
    let _ = &options;
    open_route_panel(&app, &input, SurfaceRoute::Emoji)
}

#[tauri::command]
pub(crate) fn open_voice_panel(
    app: tauri::AppHandle,
    state: tauri::State<'_, PanelInputState>,
) -> Result<(), HostActionError> {
    #[cfg(target_os = "macos")]
    if !app
        .state::<macos_panel_session::PanelState>()
        .can_open_voice_panel()
    {
        // A standalone Tauri keyboard has no authenticated IMK target. Do not open a panel which could recognize text but can never submit it.
        return Err(HostActionError {
            code: "unavailable",
        });
    }
    open_route_panel(&app, &state, SurfaceRoute::Voice)
}

#[tauri::command]
pub(crate) fn open_cloud_clipboard_panel(
    app: tauri::AppHandle,
    state: tauri::State<'_, PanelInputState>,
) -> Result<(), HostActionError> {
    open_route_panel(&app, &state, SurfaceRoute::CloudClipboard)
}

/// 背单词, summoned from wherever the user is typing.
///
/// A panel rather than a page in the settings window: a review session is ten minutes a day, and settings is a drawer people open to flip one switch and leave. It is not the candidate window either — recalling a word before its meaning appears cannot share attention with composing a sentence, and both Linux hosts have already spoken for their auxiliary row.
#[tauri::command]
pub(crate) fn open_vocabulary_panel(
    app: tauri::AppHandle,
    state: tauri::State<'_, PanelInputState>,
) -> Result<(), HostActionError> {
    open_surface_panel(&app, &state, VOCABULARY_PANEL)
}

#[tauri::command]
pub(crate) fn open_cloud_dictionary_panel(
    app: tauri::AppHandle,
    state: tauri::State<'_, PanelInputState>,
) -> Result<(), HostActionError> {
    open_route_panel(&app, &state, SurfaceRoute::CloudDictionary)
}

/// Every label an `open_*_panel` command creates. `close_panel` refuses anything
/// else, so a new panel belongs here too or its page can never close itself.
pub(crate) const CLOSABLE_PANELS: &[&str] = &[
    "keyboard-panel",
    "handwriting-panel",
    "emoji-panel",
    "clipboard-panel",
    "voice-panel",
    "vocabulary-panel",
    "cloud-clipboard-panel",
    "cloud-dictionary-panel",
];

#[tauri::command]
pub(crate) fn close_panel(
    app: tauri::AppHandle,
    label: String,
    state: tauri::State<'_, PanelInputState>,
) -> Result<(), HostActionError> {
    if !CLOSABLE_PANELS.contains(&label.as_str()) {
        return Err(HostActionError {
            code: "invalid_panel",
        });
    }
    if label == "voice-panel" {
        let _ = cancel_voice(app.clone(), None);
    }
    #[cfg(target_os = "macos")]
    if matches!(label.as_str(), "emoji-panel" | "handwriting-panel") {
        let window = app.get_webview_window(&label).ok_or(HostActionError {
            code: "unavailable",
        })?;
        return macos_panel_session::close(&app, window);
    }
    #[cfg(target_os = "macos")]
    if label == "keyboard-panel" {
        let window = app.get_webview_window(&label).ok_or(HostActionError {
            code: "unavailable",
        })?;
        return macos_keyboard::close(&window).map_err(|_| HostActionError {
            code: "unavailable",
        });
    }
    let result = app
        .get_webview_window(&label)
        .ok_or(HostActionError {
            code: "unavailable",
        })?
        .close()
        .map_err(|_| HostActionError {
            code: "unavailable",
        });
    if result.is_ok() {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        if label == CLOUD_CLIPBOARD_PANEL {
            forget_cloud_clipboard_input_target(&app);
        }
        if let Ok(mut target) = state.0.lock() {
            #[cfg(target_os = "linux")]
            {
                target.remove(&label);
            }
            #[cfg(not(target_os = "linux"))]
            {
                *target = None;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::keyboard_panel_height;

    #[test]
    fn keyboard_panel_height_follows_the_settings_preview() {
        // The preview draws 400 + clamp(adjustment, -12, 48); the window has to open at that height.
        assert_eq!(keyboard_panel_height(400.0, 0), 400.0);
        assert_eq!(keyboard_panel_height(400.0, 48), 448.0);
        assert_eq!(keyboard_panel_height(400.0, -12), 388.0);
        assert_eq!(keyboard_panel_height(400.0, 100), 448.0);
        assert_eq!(keyboard_panel_height(400.0, -100), 388.0);
    }
}
