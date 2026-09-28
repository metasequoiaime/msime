//! Run with `cargo run -p msime-desktop --example macos_keyboard_window_check`.
//! Creates hidden synthetic windows only. Never requests permissions or posts input.
#[cfg(all(target_os = "macos", not(test)))]
#[allow(dead_code)]
#[path = "../src/platform/macos/macos_keyboard.rs"]
mod macos_keyboard;

#[cfg(all(target_os = "macos", not(test)))]
fn startup_panel_for_route(
    route: Option<msime_client_core::host_surface::SurfaceRoute>,
    expected: msime_client_core::host_surface::SurfaceRoute,
) -> Option<msime_client_core::host_surface::PanelSurface> {
    route.filter(|route| *route == expected)?.panel()
}

#[cfg(all(target_os = "macos", not(test)))]
fn prepare_windows_for_route(
    windows: &mut [tauri::utils::config::WindowConfig],
    route: Option<msime_client_core::host_surface::SurfaceRoute>,
    expected: msime_client_core::host_surface::SurfaceRoute,
) {
    if startup_panel_for_route(route, expected).is_some() {
        for window in windows.iter_mut().filter(|window| window.label == "main") {
            window.visible = false;
            window.focus = false;
        }
    }
}

#[cfg(all(target_os = "macos", not(test)))]
fn check_window(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_nspanel::ManagerExt;
    let window = tauri::WebviewWindowBuilder::new(
        app,
        "keyboard-panel",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .visible(false)
    .focused(false)
    .focusable(false)
    .decorations(false)
    .resizable(false)
    .always_on_top(true)
    .build()?;
    let panel = macos_keyboard::prepare(&window)?;
    if panel.can_become_key_window()
        || panel.can_become_main_window()
        || panel.hides_on_deactivate()
        || !panel.is_floating_panel()
        || panel.is_visible()
        || !panel.becomes_key_only_if_needed()
        || !panel
            .as_panel()
            .styleMask()
            .contains(tauri_nspanel::panel::NSWindowStyleMask::NonactivatingPanel)
    {
        return Err("synthetic native panel configuration mismatch".into());
    }
    macos_keyboard::close(&window)?;
    if app.get_webview_panel("keyboard-panel").is_ok() {
        return Err("synthetic panel remained registered after restoration".into());
    }
    Ok(())
}

#[cfg(all(target_os = "macos", not(test)))]
fn main() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("synthetic native check panic: {info}")
    }));
    let mut target = msime_host_macos::capture_launch_target();
    let mut completed = 0;
    tauri::Builder::default()
        .plugin(tauri_nspanel::init())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("synthetic keyboard app must build")
        .run(move |app, event| {
            let run = match event {
                tauri::RunEvent::Ready => {
                    if let Some(target) = target.take() {
                        let _ = msime_host_macos::restore_launch_target(target);
                    }
                    true
                }
                tauri::RunEvent::WindowEvent {
                    label,
                    event: tauri::WindowEvent::Destroyed,
                    ..
                } if label == "keyboard-panel" => {
                    completed += 1;
                    if completed == 2 {
                        println!("native keyboard panel create/restore/close/reopen: passed");
                        app.exit(0);
                        false
                    } else {
                        true
                    }
                }
                tauri::RunEvent::ExitRequested {
                    api, code: None, ..
                } => {
                    api.prevent_exit();
                    false
                }
                _ => false,
            };
            if run {
                if let Err(error) = check_window(app) {
                    eprintln!("synthetic keyboard check failed: {error}");
                    app.exit(1);
                }
            }
        });
}

#[cfg(any(not(target_os = "macos"), test))]
fn main() {}
