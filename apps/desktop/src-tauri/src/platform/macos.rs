//! macOS host integration.
//!
//! The four modules gated on `test` as well as the target hold logic the host
//! tests exercise on any machine; the rest link against AppKit and only build
//! for macOS.

use msime_client_core::host_surface::{PanelSurface, SurfaceRoute};
use msime_host_macos::cloud_clipboard::{CloudClipboardError, CloudClipboardSession};
use tauri::utils::config::WindowConfig;

pub(crate) fn cloud_clipboard_error(error: CloudClipboardError) -> crate::CommandError {
    crate::CommandError {
        code: match error {
            CloudClipboardError::Invalid => "invalid",
            CloudClipboardError::Unavailable => "unavailable",
            CloudClipboardError::OutcomeUnknown => "outcome_unknown",
            CloudClipboardError::Conflict => "conflict",
        },
    }
}

pub(crate) fn native_cloud_session_from_environment(
    variable: &str,
    invalid_message: &'static str,
) -> Result<Option<CloudClipboardSession>, &'static str> {
    match std::env::var(variable) {
        Ok(value) => CloudClipboardSession::parse(&value)
            .map(Some)
            .map_err(|_| invalid_message),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(_) => Err(invalid_message),
    }
}

pub(crate) fn startup_panel_for_route(
    route: Option<SurfaceRoute>,
    expected: SurfaceRoute,
) -> Option<PanelSurface> {
    route.filter(|route| *route == expected)?.panel()
}

pub(crate) fn prepare_windows_for_route(
    windows: &mut [WindowConfig],
    route: Option<SurfaceRoute>,
    expected: SurfaceRoute,
) {
    prepare_windows_for_panel(windows, startup_panel_for_route(route, expected));
}

pub(crate) fn prepare_windows_for_panel(windows: &mut [WindowConfig], panel: Option<PanelSurface>) {
    if panel.is_some() {
        for window in windows.iter_mut().filter(|window| window.label == "main") {
            window.visible = false;
            window.focus = false;
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) mod macos_account;
#[cfg(target_os = "macos")]
pub(crate) mod macos_cloud_clipboard;
#[cfg(target_os = "macos")]
pub(crate) mod macos_cloud_dictionary;
#[cfg(any(target_os = "macos", test))]
pub(crate) mod macos_data_directory;
#[cfg(any(target_os = "macos", test))]
pub(crate) mod macos_handwriting;
#[cfg(any(target_os = "macos", test))]
pub(crate) mod macos_input_source;
#[cfg(any(target_os = "macos", test))]
pub(crate) mod macos_keyboard;
#[cfg(any(target_os = "macos", test))]
pub(crate) mod macos_launch;
#[cfg(target_os = "macos")]
pub(crate) mod macos_panel_session;
