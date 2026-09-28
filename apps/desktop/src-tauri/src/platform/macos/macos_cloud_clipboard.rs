use msime_client_core::host_surface::{PanelSurface, SurfaceRoute};
use msime_host_macos::cloud_clipboard::{CloudClipboardError, CloudClipboardSession};
use serde_json::Value;

pub(crate) struct CloudState(Option<CloudClipboardSession>);
impl CloudState {
    pub(crate) fn from_environment() -> Result<Self, &'static str> {
        match std::env::var("MSIME_CLIENT_CLOUD_CLIPBOARD_SESSION") {
            Ok(value) => CloudClipboardSession::parse(&value)
                .map(|session| Self(Some(session)))
                .map_err(|_| "Invalid native cloud session"),
            Err(std::env::VarError::NotPresent) => Ok(Self(None)),
            Err(_) => Err("Invalid native cloud session"),
        }
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
                .map_err(|error| crate::CommandError {
                    code: match error {
                        CloudClipboardError::Invalid => "invalid",
                        CloudClipboardError::Unavailable => "unavailable",
                        CloudClipboardError::OutcomeUnknown => "outcome_unknown",
                        CloudClipboardError::Conflict => "conflict",
                    },
                })
        })
    }
}

pub(crate) fn startup_panel(route: Option<SurfaceRoute>) -> Option<PanelSurface> {
    route
        .filter(|route| *route == SurfaceRoute::CloudClipboard)?
        .panel()
}
pub(crate) fn prepare_windows(
    windows: &mut [tauri::utils::config::WindowConfig],
    route: Option<SurfaceRoute>,
) {
    super::prepare_windows_for_panel(windows, startup_panel(route));
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
