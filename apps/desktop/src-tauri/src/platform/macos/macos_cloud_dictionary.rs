use msime_client_core::host_surface::{PanelSurface, SurfaceRoute};
use msime_host_macos::cloud_clipboard::{CloudClipboardError, CloudClipboardSession};
use serde_json::Value;

pub(crate) struct DictionaryState(Option<CloudClipboardSession>);
impl DictionaryState {
    pub(crate) fn from_environment() -> Result<Self, &'static str> {
        super::native_cloud_session_from_environment(
            "MSIME_CLIENT_CLOUD_DICTIONARY_SESSION",
            "Invalid native dictionary session",
        )
        .map(Self)
    }
    pub(crate) fn request(
        &self,
        label: &str,
        action: &Value,
    ) -> Option<Result<Value, crate::CommandError>> {
        self.0.as_ref().map(|session| {
            if label != "cloud-dictionary-panel" {
                return Err(super::cloud_clipboard_error(
                    CloudClipboardError::Unavailable,
                ));
            }
            let mut result = session
                .request_dictionary(action)
                .map_err(super::cloud_clipboard_error)?;
            if action["operation"] == "export" {
                let name = format!("dictionary-{}.tsv", action["kind"].as_str().unwrap_or(""));
                let path = result["export_file"]["path"].as_str().ok_or_else(|| {
                    super::cloud_clipboard_error(CloudClipboardError::Unavailable)
                })?;
                let bytes = result["export_file"]["bytes"].as_u64().ok_or_else(|| {
                    super::cloud_clipboard_error(CloudClipboardError::Unavailable)
                })?;
                let text = msime_host_macos::cloud_dictionary::read_export(
                    std::path::Path::new(path),
                    &name,
                    bytes,
                )
                .map_err(super::cloud_clipboard_error)?;
                // The private descriptor is never returned to JavaScript.
                result = serde_json::json!({"text":text,"filename":name});
            }
            Ok(result)
        })
    }
}
pub(crate) fn startup_panel(route: Option<SurfaceRoute>) -> Option<PanelSurface> {
    super::startup_panel_for_route(route, SurfaceRoute::CloudDictionary)
}
pub(crate) fn prepare_windows(
    windows: &mut [tauri::utils::config::WindowConfig],
    route: Option<SurfaceRoute>,
) {
    super::prepare_windows_for_route(windows, route, SurfaceRoute::CloudDictionary);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_export_descriptor_is_consumed_before_returning_to_the_webview() {
        use std::io::{Read, Write};
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::UnixListener;
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary
            .path()
            .join("msime-export-00000000-0000-0000-0000-000000000000");
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let file = root.join("dictionary-pinyin.tsv");
        std::fs::write(&file, "synthetic").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        let path = temporary.path().join("rpc.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let state = DictionaryState(Some(
            CloudClipboardSession::parse(
                &serde_json::json!({"version":1,"path":path,"host_pid":std::process::id()})
                    .to_string(),
            )
            .unwrap(),
        ));
        let action = serde_json::json!({"operation":"export","kind":"pinyin","format":"standard"});
        assert_eq!(
            state
                .request("cloud-clipboard-panel", &action)
                .unwrap()
                .unwrap_err()
                .code,
            "unavailable"
        );
        let worker = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            socket.read_to_end(&mut request).unwrap();
            let response = serde_json::to_vec(
                &serde_json::json!({"ok":true,"value":{"export_file":{"path":file,"bytes":9}}}),
            )
            .unwrap();
            socket
                .write_all(&(response.len() as u32).to_be_bytes())
                .unwrap();
            socket.write_all(&response).unwrap();
        });
        let result = state
            .request("cloud-dictionary-panel", &action)
            .unwrap()
            .unwrap_or_else(|_| panic!("synthetic export failed"));
        assert_eq!(
            result,
            serde_json::json!({"filename":"dictionary-pinyin.tsv","text":"synthetic"})
        );
        worker.join().unwrap();
    }

    #[test]
    fn dictionary_startup_does_not_own_clipboard_or_input_sessions() {
        let route = Some(SurfaceRoute::CloudDictionary);
        assert_eq!(
            startup_panel(route).unwrap().label,
            "cloud-dictionary-panel"
        );
        assert!(startup_panel(Some(SurfaceRoute::CloudClipboard)).is_none());
        assert!(super::super::macos_panel_session::startup_panel(route).is_none());
        let mut windows = vec![tauri::utils::config::WindowConfig {
            label: "main".into(),
            visible: true,
            focus: true,
            ..Default::default()
        }];
        prepare_windows(&mut windows, route);
        assert!(!windows[0].visible && !windows[0].focus);
    }
}
