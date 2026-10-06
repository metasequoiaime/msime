//! Exercise the registered command on the compiled desktop platform, not a UI mock.
use crate::{test_api_credential, RuntimeOptionsState};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn invoke(service: &str, config: Value) -> Value {
    let app = tauri::test::mock_builder()
        .manage(RuntimeOptionsState {
            path: None,
            document: Arc::new(Mutex::new(json!({}))),
            skins: None,
        })
        .invoke_handler(tauri::generate_handler![test_api_credential])
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("mock application");
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock window");
    tauri::test::get_ipc_response(
        &window,
        tauri::webview::InvokeRequest {
            cmd: "test_api_credential".into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: if cfg!(target_os = "windows") {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .unwrap(),
            body: tauri::ipc::InvokeBody::Json(json!({"service":service,"config":config})),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    )
    .expect("credential command must resolve, not return unavailable")
    .deserialize()
    .expect("structured credential result")
}

#[test]
fn desktop_credential_command_routes_all_services_without_a_provider_socket() {
    // Invalid drafts must reach each shared validator without issuing a request.
    for (service, config, message) in [
        ("ai.assistant", json!({}), "请先填写有效的 API Key。"),
        ("voice.polish", json!({}), "请先填写有效的 API Key。"),
        (
            "voice.asr",
            json!({"provider":"openai"}),
            "请先填写有效的 API Key。",
        ),
        (
            "voice.asr",
            json!({"provider":"siliconflow"}),
            "请先填写有效的 API Key。",
        ),
        (
            "voice.asr",
            json!({"provider":"groq"}),
            "请先填写有效的 API Key。",
        ),
        // Doubao answers with its own message rather than the shared one: it needs a wss endpoint and a
        // resource ID as well as a credential, and "fill in an API Key" would send the user looking for
        // the wrong box.
        (
            "voice.asr",
            json!({"provider":"doubao"}),
            "请填写有效的 WSS 接口、资源 ID 及对应鉴权方式的凭据。",
        ),
    ] {
        assert_eq!(
            invoke(service, config),
            json!({"ok":false,"message":message})
        );
    }
    for (service, config) in [
        ("voice.asr", json!({"provider":"doubao"})),
        ("voice.asr", json!({"provider":"system"})),
        ("translation.tencent", json!({})),
        ("translation.niutrans", json!({})),
        (
            "translation.custom",
            json!({"endpoint":"file:///synthetic"}),
        ),
        ("unknown", json!({})),
    ] {
        let result = invoke(service, config);
        assert_eq!(result["ok"], false);
        assert!(!result["message"].as_str().unwrap().is_empty());
    }
}

#[test]
fn desktop_credential_command_uses_bounded_http_and_sanitizes_provider_output() {
    for (status, body, ok) in [
        (
            "200 OK",
            r#"{"code":200,"data":"synthetic-provider-output"}"#.to_owned(),
            true,
        ),
        (
            "200 OK",
            r#"{"message":"synthetic-provider-output"}"#.to_owned(),
            false,
        ),
        (
            "401 Unauthorized",
            "synthetic-provider-output".into(),
            false,
        ),
        ("302 Found", "synthetic-provider-output".into(), false),
        ("200 OK", "x".repeat(256 * 1024 + 1), false),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let redirect = TcpListener::bind("127.0.0.1:0").unwrap();
        redirect.set_nonblocking(true).unwrap();
        let location = format!(
            "http://{}/must-not-receive-key",
            redirect.local_addr().unwrap()
        );
        let server = std::thread::spawn(move || {
            listener.set_nonblocking(true).unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(std::time::Instant::now() < deadline, "no probe request");
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => panic!("fixture accept failed"),
                }
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0; 1024];
                let count = socket.read(&mut chunk).unwrap();
                assert!(count > 0 && request.len() + count <= 16 * 1024);
                request.extend_from_slice(&chunk[..count]);
                if let Some(offset) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..offset]).to_lowercase();
                    let length: usize = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if request.len() >= offset + 4 + length {
                        assert!(headers.starts_with("post /translate http/1.1"));
                        assert!(headers.contains("authorization: bearer synthetic-key"));
                        let payload: Value =
                            serde_json::from_slice(&request[offset + 4..]).unwrap();
                        assert_eq!(
                            payload,
                            json!({"text":"测试","source_lang":"ZH","target_lang":"EN"})
                        );
                        break;
                    }
                }
            }
            let response = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nLocation: {location}\r\nConnection: close\r\n\r\n{body}", body.len());
            // The bounded reader is allowed to close early on an oversized response.
            let _ = socket.write_all(response.as_bytes());
        });
        let result = invoke(
            "translation.custom",
            json!({
                "endpoint":format!("http://{address}/translate"), "api_key":"synthetic-key",
            }),
        );
        server.join().unwrap();
        assert_eq!(result["ok"], ok);
        assert!(!result.to_string().contains("synthetic-provider-output"));
        assert!(!result.to_string().contains("synthetic-key"));
        assert_eq!(
            redirect.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}
