use msime_client_core::host_surface::{PanelSurface, SurfaceRoute};
use msime_host_macos::cloud_clipboard::CloudClipboardSession;
use serde_json::Value;

pub(crate) struct CloudState(Option<CloudClipboardSession>);
impl CloudState {
    pub(crate) fn from_environment() -> Result<Self, &'static str> {
        super::native_cloud_session_from_environment(
            "MSIME_CLIENT_CLOUD_CLIPBOARD_SESSION",
            "Invalid native cloud session",
        )
        .map(Self)
    }
    pub(crate) fn request(
        &self,
        label: &str,
        action: &Value,
    ) -> Option<Result<Value, crate::CommandError>> {
        self.0.as_ref().map(|session| {
            if label != "cloud-clipboard-panel" {
                return Err(crate::CommandError {
                    code: "unavailable",
                });
            }
            session
                .request(action)
                .map_err(super::cloud_clipboard_error)
        })
    }
}

pub(crate) fn startup_panel(route: Option<SurfaceRoute>) -> Option<PanelSurface> {
    super::startup_panel_for_route(route, SurfaceRoute::CloudClipboard)
}
pub(crate) fn prepare_windows(
    windows: &mut [tauri::utils::config::WindowConfig],
    route: Option<SurfaceRoute>,
) {
    super::prepare_windows_for_route(windows, route, SurfaceRoute::CloudClipboard);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cloud_startup_is_separate_from_input_sessions() {
        let route = Some(SurfaceRoute::CloudClipboard);
        assert_eq!(startup_panel(route).unwrap().label, "cloud-clipboard-panel");
        assert!(super::super::macos_panel_session::startup_panel(route).is_none());
        let mut windows = vec![tauri::utils::config::WindowConfig {
            label: "main".into(),
            visible: true,
            focus: true,
            ..Default::default()
        }];
        prepare_windows(&mut windows, route);
        assert!(!windows[0].visible && !windows[0].focus);
        assert!(startup_panel(Some(SurfaceRoute::Emoji)).is_none());
    }
}
