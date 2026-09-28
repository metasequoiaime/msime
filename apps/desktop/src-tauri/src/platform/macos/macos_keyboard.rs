use msime_client_core::host_surface::{PanelSurface, SurfaceRoute};
use tauri::utils::config::WindowConfig;
#[cfg(target_os = "macos")]
use tauri::Manager;

#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(MSIMEKeyboardPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
}

#[cfg(target_os = "macos")]
pub(crate) fn show(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    let panel = prepare(window)?;
    if !window.is_visible().unwrap_or(false) {
        center_on_monitor(window)?;
    }
    panel.order_front_regardless();
    Ok(())
}

fn bottom_center_position(area: (i32, i32, u32, u32), panel: (u32, u32)) -> (i32, i32) {
    let x = i64::from(area.0) + ((i64::from(area.2) - i64::from(panel.0)).max(0) / 2);
    let y = i64::from(area.1) + (i64::from(area.3) - i64::from(panel.1)).max(0);
    (
        x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    )
}

#[cfg(target_os = "macos")]
fn center_on_monitor(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    let monitor = window.current_monitor()?.or(window.primary_monitor()?);
    let Some(monitor) = monitor else {
        return Ok(());
    };
    let area = monitor.work_area();
    let size = window.outer_size()?;
    let (x, y) = bottom_center_position(
        (
            area.position.x,
            area.position.y,
            area.size.width,
            area.size.height,
        ),
        (size.width, size.height),
    );
    window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        x, y,
    )))?;
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn prepare(
    window: &tauri::WebviewWindow,
) -> tauri::Result<tauri_nspanel::PanelHandle<tauri::Wry>> {
    use tauri_nspanel::{ManagerExt, WebviewWindowExt};
    let content = if window.get_webview_panel(window.label()).is_err() {
        Some(
            msime_host_macos::detach_window_content(window.ns_window()? as usize).ok_or_else(
                || tauri::Error::Io(std::io::Error::other("Cannot prepare keyboard view")),
            )?,
        )
    } else {
        None
    };
    let panel = match window.get_webview_panel(window.label()) {
        Ok(panel) => panel,
        Err(_) => window.to_panel::<MSIMEKeyboardPanel>()?,
    };
    panel.set_style_mask(tauri_nspanel::panel::NSWindowStyleMask::NonactivatingPanel);
    panel.set_hides_on_deactivate(false);
    panel.set_becomes_key_only_if_needed(true);
    drop(content);
    Ok(panel)
}

#[cfg(target_os = "macos")]
pub(crate) fn restore(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use tauri_nspanel::ManagerExt;
    if let Ok(panel) = window.get_webview_panel(window.label()) {
        let content = msime_host_macos::detach_window_content(window.ns_window()? as usize)
            .ok_or_else(|| {
                tauri::Error::Io(std::io::Error::other("Cannot restore keyboard view"))
            })?;
        let restored = panel.to_window();
        drop(content);
        if restored.is_none() {
            return Err(tauri::Error::Io(std::io::Error::other(
                "Cannot restore keyboard window",
            )));
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn close(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    restore(window)?;
    // Conversion clears the native delegate. Destroy through Tauri so its
    // window manager is notified and the same label can be opened again.
    window.destroy()
}

/// Enable routes as their native input/service bridges are migrated. The
/// keyboard can use live foreground targeting without retaining an IMK client.
pub(crate) fn startup_panel(route: Option<SurfaceRoute>) -> Option<PanelSurface> {
    route
        .filter(|route| *route == SurfaceRoute::Keyboard)?
        .panel()
}

/// Runs before Tauri constructs any windows, so the hidden settings window
/// cannot steal the editor's focus during a keyboard-only launch.
pub(crate) fn prepare_windows(windows: &mut [WindowConfig], route: Option<SurfaceRoute>) {
    super::prepare_windows_for_panel(windows, startup_panel(route));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottom_center_position_respects_work_area_origin_and_small_monitors() {
        assert_eq!(
            bottom_center_position((0, 0, 1728, 1117), (1100, 400)),
            (314, 717)
        );
        assert_eq!(
            bottom_center_position((-1920, 40, 1280, 800), (1100, 400)),
            (-1830, 440)
        );
        assert_eq!(
            bottom_center_position((20, -10, 500, 200), (800, 400)),
            (20, -10)
        );
    }

    #[test]
    fn keyboard_launch_hides_settings_before_creation_and_uses_shared_surface() {
        let mut windows = vec![WindowConfig::default()];
        windows[0].label = "main".into();
        windows[0].visible = true;
        windows[0].focus = true;
        prepare_windows(&mut windows, Some(SurfaceRoute::Keyboard));
        assert!(!windows[0].visible && !windows[0].focus);
        let panel = startup_panel(Some(SurfaceRoute::Keyboard)).unwrap();
        assert_eq!(panel.label, "keyboard-panel");
        assert_eq!(panel.query, "keyboard");
        assert!(!crate::panel_window::panel_accepts_focus(panel.label));
    }

    #[test]
    fn ordinary_settings_launch_keeps_its_existing_visibility() {
        for route in [None, SurfaceRoute::parse("settings:appearance").ok()] {
            let mut windows = vec![WindowConfig::default()];
            windows[0].label = "main".into();
            windows[0].visible = true;
            windows[0].focus = true;
            prepare_windows(&mut windows, route);
            assert!(windows[0].visible && windows[0].focus);
            assert!(startup_panel(route).is_none());
        }
    }
}
