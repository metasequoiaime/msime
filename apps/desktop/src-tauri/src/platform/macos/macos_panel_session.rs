use crate::HostActionError;
use msime_client_core::host_surface::{PanelSurface, SurfaceRoute};
use msime_host_macos::panel_session::{PanelSession, SessionError};
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};
use tauri::Manager;

pub(crate) struct PanelState {
    session: Option<Arc<PanelSession>>,
    // Serialize input and close so a hidden/closing view cannot submit later.
    lifecycle: Arc<AtomicU8>,
}

impl PanelState {
    pub(crate) fn from_environment() -> Result<Self, &'static str> {
        let value = std::env::var("MSIME_CLIENT_PANEL_SESSION");
        let session = match value {
            Ok(value) => Some(Arc::new(
                PanelSession::parse(&value).map_err(|_| "Invalid native panel session")?,
            )),
            Err(std::env::VarError::NotPresent) => None,
            Err(_) => return Err("Invalid native panel session"),
        };
        Ok(Self {
            session,
            lifecycle: Arc::new(AtomicU8::new(0)),
        })
    }

    pub(crate) fn can_open_voice_panel(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|session| !session.accepts_clipboard() && !session.is_used())
    }
}

pub(crate) fn startup_panel(route: Option<SurfaceRoute>) -> Option<PanelSurface> {
    route
        .filter(|route| {
            matches!(
                route,
                SurfaceRoute::Emoji | SurfaceRoute::Handwriting | SurfaceRoute::Voice
            )
        })?
        .panel()
}

fn startup_panel_for_session(
    route: Option<SurfaceRoute>,
    voice_session_available: bool,
) -> Option<PanelSurface> {
    let panel = startup_panel(route)?;
    if route == Some(SurfaceRoute::Voice) && !voice_session_available {
        return None;
    }
    Some(panel)
}

fn voice_session_available(route: Option<SurfaceRoute>) -> bool {
    if route != Some(SurfaceRoute::Voice) {
        return true;
    }
    PanelState::from_environment()
        .ok()
        .is_some_and(|state| state.can_open_voice_panel())
}

pub(crate) fn startup_panel_for_launch(route: Option<SurfaceRoute>) -> Option<PanelSurface> {
    startup_panel_for_session(route, voice_session_available(route))
}

/// Whether the settings window stays hidden, given a decision about the voice session.
///
/// Separate from `prepare_windows` because that one asks the environment, and a voice route hides the
/// window only when a session is actually reachable - so a test that called it was really testing whether
/// the machine it ran on had one.
fn prepare_windows_for_session(
    windows: &mut [tauri::utils::config::WindowConfig],
    route: Option<SurfaceRoute>,
    voice_session_available: bool,
) {
    super::prepare_windows_for_panel(
        windows,
        startup_panel_for_session(route, voice_session_available),
    );
}

pub(crate) fn prepare_windows(
    windows: &mut [tauri::utils::config::WindowConfig],
    route: Option<SurfaceRoute>,
) {
    prepare_windows_for_session(windows, route, voice_session_available(route));
}

struct SubmissionGuard(Arc<AtomicU8>);
impl Drop for SubmissionGuard {
    fn drop(&mut self) {
        self.0.store(0, Ordering::Release);
    }
}

pub(crate) async fn submit(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    text: String,
) -> Result<(), HostActionError> {
    submit_with_mode(app, window, text, false).await
}

pub(crate) async fn submit_clipboard(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    text: String,
) -> Result<(), HostActionError> {
    submit_with_mode(app, window, text, true).await
}

async fn submit_with_mode(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    text: String,
    clipboard: bool,
) -> Result<(), HostActionError> {
    if !owns_input_panel(crate::requested_surface_route(), window.label()) {
        return Err(error(SessionError::Unavailable));
    }
    let state = app.state::<PanelState>();
    let session = state
        .session
        .clone()
        .ok_or_else(|| error(SessionError::Unavailable))?;
    validate_submission(&session, clipboard, &text).map_err(error)?;
    if session.is_used()
        || state
            .lifecycle
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return Err(error(SessionError::Rejected));
    }
    let _guard = SubmissionGuard(Arc::clone(&state.lifecycle));
    let (send, received) = std::sync::mpsc::sync_channel(1);
    let target = Arc::clone(&session);
    let panel = window.clone();
    app.run_on_main_thread(move || {
        // Do not hide the UI when the user has already chosen a third app.
        let activated = target.activate_target();
        let hidden = activated && panel.hide().is_ok();
        let _ = send.send(hidden);
    })
    .map_err(|_| error(SessionError::Unavailable))?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        if !received.recv().unwrap_or(false) {
            return (false, Err(SessionError::Unavailable));
        }
        (
            true,
            if clipboard {
                session.submit_clipboard(&text)
            } else {
                session.submit_candidate(&text)
            },
        )
    })
    .await
    .map_err(|_| error(SessionError::Unavailable))?;
    match result.1 {
        Ok(()) => window
            .destroy()
            .map_err(|_| error(SessionError::Unavailable)),
        Err(failure) => {
            if result.0 {
                let _ = window.show();
            }
            Err(error(failure))
        }
    }
}

pub(crate) fn close(
    app: &tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<(), HostActionError> {
    if !owns_input_panel(crate::requested_surface_route(), window.label()) {
        return window
            .destroy()
            .map_err(|_| error(SessionError::Unavailable));
    }
    let state = app.state::<PanelState>();
    if state
        .lifecycle
        .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(error(SessionError::Rejected));
    }
    let session = state.session.clone();
    let lifecycle = Arc::clone(&state.lifecycle);
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = SubmissionGuard(lifecycle);
        if let Some(session) = session {
            session.cancel();
        }
        let _ = window.destroy();
    });
    Ok(())
}

fn error(error: SessionError) -> HostActionError {
    HostActionError {
        code: match error {
            SessionError::Invalid => "invalid_text",
            SessionError::Unavailable => "unavailable",
            SessionError::Rejected => "input_session_expired",
            SessionError::OutcomeUnknown => "input_outcome_unknown",
        },
    }
}

fn owns_input_panel(route: Option<SurfaceRoute>, label: &str) -> bool {
    startup_panel(route)
        .or_else(|| super::macos_cloud_clipboard::startup_panel(route))
        .is_some_and(|panel| panel.label == label)
}

fn validate_submission(
    session: &PanelSession,
    clipboard: bool,
    text: &str,
) -> Result<(), SessionError> {
    if clipboard && !session.accepts_clipboard() {
        return Err(SessionError::Rejected);
    }
    if clipboard {
        msime_host_macos::panel_session::validate_clipboard_text(text)
    } else {
        msime_client_core::panels::validate_candidate(text).map_err(|_| SessionError::Invalid)
    }
}

pub(crate) fn can_submit_clipboard(app: &tauri::AppHandle, label: &str) -> bool {
    let state = app.state::<PanelState>();
    crate::requested_surface_route() == Some(SurfaceRoute::CloudClipboard)
        && owns_input_panel(crate::requested_surface_route(), label)
        && state.lifecycle.load(Ordering::Acquire) == 0
        && state
            .session
            .as_ref()
            .is_some_and(|session| session.accepts_clipboard() && !session.is_used())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipboard_route_and_session_type_are_both_required_for_multiline_submission() {
        let make = |clipboard| {
            PanelSession::parse(&serde_json::json!({"version":1,"path":"/tmp/synthetic.sock","host_pid":1,"target_pid":2,"target_started":42,"clipboard":clipboard}).to_string()).unwrap()
        };
        let clipboard = make(true);
        let candidate = make(false);
        let text = "中".repeat(3997) + "\n\r\t";
        assert!(owns_input_panel(
            Some(SurfaceRoute::CloudClipboard),
            "cloud-clipboard-panel"
        ));
        for label in ["emoji-panel", "handwriting-panel", "main"] {
            assert!(!owns_input_panel(Some(SurfaceRoute::CloudClipboard), label));
        }
        assert!(!owns_input_panel(
            Some(SurfaceRoute::Emoji),
            "cloud-clipboard-panel"
        ));
        assert_eq!(validate_submission(&clipboard, true, &text), Ok(()));
        assert_eq!(
            validate_submission(&candidate, true, &text),
            Err(SessionError::Rejected)
        );
        assert_eq!(validate_submission(&clipboard, false, "synthetic"), Ok(()));
        assert_eq!(
            validate_submission(&candidate, false, &text),
            Err(SessionError::Invalid)
        );
        assert_eq!(validate_submission(&candidate, false, "合成"), Ok(()));
    }

    #[test]
    fn handwriting_uses_shared_startup_and_cannot_consume_another_panels_session() {
        let route = Some(SurfaceRoute::Handwriting);
        assert_eq!(startup_panel(route).unwrap().label, "handwriting-panel");
        assert!(owns_input_panel(route, "handwriting-panel"));
        assert!(!owns_input_panel(route, "emoji-panel"));
        assert!(!owns_input_panel(
            Some(SurfaceRoute::Emoji),
            "handwriting-panel"
        ));
        assert!(!owns_input_panel(None, "handwriting-panel"));
        let mut windows = vec![tauri::utils::config::WindowConfig {
            label: "main".into(),
            visible: true,
            focus: true,
            ..Default::default()
        }];
        prepare_windows(&mut windows, route);
        assert!(!windows[0].visible && !windows[0].focus);
    }
    #[test]
    fn emoji_startup_uses_shared_route_and_hides_only_settings() {
        let route = SurfaceRoute::parse("emoji").ok();
        let mut windows = vec![tauri::utils::config::WindowConfig {
            label: "main".into(),
            visible: true,
            ..Default::default()
        }];
        prepare_windows(&mut windows, route);
        assert!(!windows[0].visible);
        assert_eq!(startup_panel(route).unwrap().label, "emoji-panel");
        assert!(startup_panel(Some(SurfaceRoute::Keyboard)).is_none());
        assert!(startup_panel(SurfaceRoute::parse("settings:input").ok()).is_none());
    }
    #[test]
    fn voice_startup_uses_shared_route_and_hides_only_settings() {
        let route = SurfaceRoute::parse("voice").ok();
        let settings_window = || {
            vec![tauri::utils::config::WindowConfig {
                label: "main".into(),
                visible: true,
                focus: true,
                ..Default::default()
            }]
        };

        let mut windows = settings_window();
        prepare_windows_for_session(&mut windows, route, true);
        assert!(!windows[0].visible && !windows[0].focus);

        // Without a session the panel never opens, so hiding the settings window would leave the user
        // looking at nothing: the voice route is the one launch that can decline to take over.
        let mut windows = settings_window();
        prepare_windows_for_session(&mut windows, route, false);
        assert!(windows[0].visible && windows[0].focus);

        assert_eq!(startup_panel(route).unwrap().label, "voice-panel");
        assert!(super::super::macos_cloud_clipboard::startup_panel(route).is_none());
    }

    #[test]
    fn voice_launch_requires_an_authenticated_session_but_other_panels_do_not() {
        assert!(startup_panel_for_session(Some(SurfaceRoute::Voice), true).is_some());
        assert!(startup_panel_for_session(Some(SurfaceRoute::Voice), false).is_none());
        assert!(startup_panel_for_session(Some(SurfaceRoute::Emoji), false).is_some());
        assert!(startup_panel_for_session(Some(SurfaceRoute::Handwriting), false).is_some());
    }
    #[test]
    fn close_and_submit_have_an_exclusive_lifecycle() {
        let state = Arc::new(AtomicU8::new(0));
        assert!(state
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok());
        assert!(state
            .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
            .is_err());
        drop(SubmissionGuard(Arc::clone(&state)));
        assert!(state
            .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
            .is_ok());
        assert!(state
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_err());
    }
}
