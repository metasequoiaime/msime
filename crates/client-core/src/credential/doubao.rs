//! Doubao credential probe: synthetic PCM only, no recording or transcript output.
use crate::credential::probe::ProbeResult;
use crate::voice::doubao_frame;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::{
    tungstenite::{client::IntoClientRequest, protocol::WebSocketConfig, Message},
    WebSocketStream,
};

const MAX_BYTES: usize = 1_048_576;
const MAX_MESSAGES: usize = 64;
const TOTAL_TIMEOUT: Duration = Duration::from_secs(15);

// Deliberately no Debug implementation: this request contains credentials.
pub struct ProbeRequest {
    pub endpoint: String,
    pub headers: Vec<(&'static str, String)>,
}

pub trait Transport {
    /// True requires a valid terminal ASR response, not merely a connection.
    fn probe(&self, request: &ProbeRequest) -> bool;
}

pub struct WebSocketTransport;

impl Transport for WebSocketTransport {
    fn probe(&self, request: &ProbeRequest) -> bool {
        // Called on the desktop's blocking worker, never on its UI executor.
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return false;
        };
        let result = runtime.block_on(async {
            tokio::time::timeout(TOTAL_TIMEOUT, connect_and_probe(request))
                .await
                .unwrap_or(false)
        });
        // DNS may use a blocking resolver. Do not wait indefinitely for it at
        // runtime teardown after the request deadline has already expired.
        runtime.shutdown_timeout(Duration::ZERO);
        result
    }
}

fn socket_config() -> WebSocketConfig {
    WebSocketConfig::default()
        .max_message_size(Some(MAX_BYTES))
        .max_frame_size(Some(MAX_BYTES))
}

async fn connect_and_probe(probe: &ProbeRequest) -> bool {
    let Ok(mut request) = probe.endpoint.as_str().into_client_request() else {
        return false;
    };
    for (name, value) in &probe.headers {
        let Ok(mut value) = value.parse::<tokio_tungstenite::tungstenite::http::HeaderValue>()
        else {
            return false;
        };
        value.set_sensitive(true);
        request.headers_mut().insert(*name, value);
    }
    // connect_async does not follow redirects: credentials stay at the chosen
    // endpoint. The five-second deadline includes DNS, TCP, TLS and upgrade.
    let Ok(Ok((mut socket, _))) = tokio::time::timeout(
        Duration::from_secs(5),
        tokio_tungstenite::connect_async_with_config(request, Some(socket_config()), false),
    )
    .await
    else {
        return false;
    };
    let ok = exchange(&mut socket).await;
    // Completion is already established; best-effort close cannot turn a
    // valid final response into a failure or keep the settings request alive.
    let _ = tokio::time::timeout(Duration::from_millis(100), socket.close(None)).await;
    ok
}

async fn exchange<S: AsyncRead + AsyncWrite + Unpin>(socket: &mut WebSocketStream<S>) -> bool {
    let packets = [
        doubao_frame::start_frame(true, true, false, ""),
        doubao_frame::audio_frame(2, &vec![0; 16000 * 2], true),
    ];
    for packet in packets {
        if socket.send(Message::Binary(packet.into())).await.is_err() {
            return false;
        }
    }
    let mut received = 0usize;
    for _ in 0..MAX_MESSAGES {
        let Some(Ok(message)) = socket.next().await else {
            return false;
        };
        received = received.saturating_add(message.len());
        if received > MAX_BYTES {
            return false;
        }
        match message {
            Message::Binary(frame) => match response_state(&frame) {
                Some(true) => return true,
                Some(false) => {}
                None => return false,
            },
            Message::Ping(_) => {
                // Tungstenite queued the matching pong while reading.
                if socket.flush().await.is_err() {
                    return false;
                }
            }
            Message::Pong(_) => {}
            _ => return false,
        }
    }
    false
}

fn response_state(frame: &[u8]) -> Option<bool> {
    // Error frames and unsupported flags are failures regardless of their
    // message text. Never deserialize or expose service diagnostics to UI.
    if frame.len() < 4 || frame[1] & 0x0f > 3 || frame[3] != 0 {
        return None;
    }
    let (last, _, payload) = doubao_frame::decode_json_frame(frame)?;
    let body: Value = serde_json::from_slice(&payload).ok()?;
    if !body.is_object() || body.get("error").is_some_and(|v| !v.is_null()) {
        return None;
    }
    // Some gateways wrap the recognizer result. Silence may legitimately
    // produce an empty object or no transcript; malformed JSON is not success.
    for candidate in [&body, body.get("payload_msg").unwrap_or(&Value::Null)] {
        if let Some(code) = candidate.get("code") {
            if code.as_i64() != Some(0) {
                return None;
            }
        }
    }
    Some(last)
}

pub fn test(config: &Value, transport: &impl Transport) -> ProbeResult {
    let get = |key| config.get(key).and_then(Value::as_str).unwrap_or("").trim();
    let headers = crate::credential::doubao_auth::headers(
        get("auth_mode"),
        get("app_id"),
        get("token"),
        get("resource_id"),
    );
    let endpoint = get("endpoint");
    let valid = get("provider") == "doubao"
        && matches!(get("auth_mode"), "api_key" | "legacy")
        && headers.is_some()
        && endpoint.len() <= 2048
        && reqwest::Url::parse(endpoint).ok().is_some_and(|url| {
            url.scheme() == "wss"
                && endpoint.split_once("://").is_some_and(|(_, authority)| {
                    authority
                        .as_bytes()
                        .first()
                        .is_some_and(|byte| *byte != b'/')
                })
                && url.host_str().is_some_and(|host| !host.is_empty())
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none()
        });
    if !valid {
        return ProbeResult {
            ok: false,
            message: "请填写有效的 WSS 接口、资源 ID 及对应鉴权方式的凭据。".into(),
        };
    }
    let ok = transport.probe(&ProbeRequest {
        endpoint: endpoint.to_owned(),
        headers: headers.unwrap_or_default(),
    });
    ProbeResult {
        ok,
        message: if ok {
            "连接成功，豆包语音识别凭据有效。"
        } else {
            "豆包测试未成功或超时，请检查凭据、资源权限、接口地址和网络。"
        }
        .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{cell::Cell, io::Read};
    use tokio_tungstenite::tungstenite::protocol::Role;

    fn config() -> Value {
        json!({"provider":"doubao", "auth_mode":"api_key", "app_id":"stale-app",
            "token":"synthetic-key", "resource_id":"fixture-resource",
            "endpoint":"wss://fixture.invalid/asr"})
    }

    #[test]
    fn auth_modes_are_exclusive_and_failures_are_sanitized() {
        struct Fake {
            legacy: bool,
            called: Cell<bool>,
        }
        impl Transport for Fake {
            fn probe(&self, request: &ProbeRequest) -> bool {
                self.called.set(true);
                assert_eq!(request.endpoint, "wss://fixture.invalid/asr");
                let headers: std::collections::HashMap<_, _> = request
                    .headers
                    .iter()
                    .map(|(name, value)| (*name, value.as_str()))
                    .collect();
                assert_eq!(headers["x-api-resource-id"], "fixture-resource");
                assert!(uuid::Uuid::parse_str(headers["x-api-request-id"]).is_ok());
                if self.legacy {
                    assert_eq!(headers["x-api-app-key"], "stale-app");
                    assert_eq!(headers["x-api-access-key"], "synthetic-key");
                    assert!(!headers.contains_key("x-api-key"));
                } else {
                    assert_eq!(headers["x-api-key"], "synthetic-key");
                    assert!(!headers.contains_key("x-api-app-key"));
                    assert!(!headers.contains_key("x-api-access-key"));
                }
                self.legacy
            }
        }
        for legacy in [false, true] {
            let mut config = config();
            config["auth_mode"] = json!(if legacy { "legacy" } else { "api_key" });
            let transport = Fake {
                legacy,
                called: Cell::new(false),
            };
            let result = test(&config, &transport);
            assert!(transport.called.get());
            assert_eq!(result.ok, legacy);
            assert!(!result.message.contains("synthetic-key"));
            assert!(!result.message.contains("fixture.invalid"));
        }
    }

    #[test]
    fn invalid_credentials_and_endpoints_never_connect() {
        struct Never;
        impl Transport for Never {
            fn probe(&self, _: &ProbeRequest) -> bool {
                panic!("unexpected connection")
            }
        }
        for (field, value) in [
            ("provider", "openai"),
            ("auth_mode", "unknown"),
            ("token", ""),
            ("token", "***"),
            ("token", "<stored>"),
            ("token", "a\r\nb"),
            ("resource_id", ""),
            ("resource_id", "a\r\nb"),
            ("endpoint", "ws://fixture.invalid/asr"),
            ("endpoint", "https://fixture.invalid/asr"),
            ("endpoint", "wss://user:pass@fixture.invalid/asr"),
            ("endpoint", "wss:///asr"),
            ("endpoint", "wss://fixture.invalid/asr#fragment"),
        ] {
            let mut config = config();
            config[field] = json!(value);
            assert!(!test(&config, &Never).ok);
        }
        let mut config = config();
        config["auth_mode"] = json!("legacy");
        config["app_id"] = json!("");
        assert!(!test(&config, &Never).ok);
        config["token"] = json!("x".repeat(8193));
        assert!(!test(&config, &Never).ok);
    }

    fn response(flags: u8, body: &[u8]) -> Message {
        Message::Binary(doubao_frame::encode_json_frame(9, flags, -2, body).into())
    }

    #[test]
    fn terminal_response_requires_valid_json_and_no_service_error() {
        for body in [
            b"{}".as_slice(),
            br#"{"result":{"text":""}}"#,
            br#"{"result":[]}"#,
        ] {
            let frame = doubao_frame::encode_json_frame(9, 3, -2, body);
            assert_eq!(response_state(&frame), Some(true));
        }
        for body in [
            b"null".as_slice(),
            b"[]",
            b"invalid",
            br#"{"code":401}"#,
            br#"{"error":"synthetic-diagnostic"}"#,
            br#"{"payload_msg":{"code":403}}"#,
        ] {
            let frame = doubao_frame::encode_json_frame(9, 3, -2, body);
            assert_eq!(response_state(&frame), None);
        }
        assert_eq!(
            response_state(&[0x11, 0xf0, 0x11, 0, 0, 0, 0, 7, 0, 0, 0, 0]),
            None
        );
        assert_eq!(
            response_state(&doubao_frame::encode_json_frame(9, 1, 1, b"{}")),
            Some(false)
        );
    }

    // Actual WebSocket framing over an in-memory duplex: no external service,
    // DNS, microphone, credentials or transcript logging in regression tests.
    async fn fixture(messages: Vec<Message>) -> bool {
        let (client, server) = tokio::io::duplex(128 * 1024);
        let mut client =
            WebSocketStream::from_raw_socket(client, Role::Client, Some(socket_config())).await;
        let mut server = WebSocketStream::from_raw_socket(server, Role::Server, None).await;
        let server = tokio::spawn(async move {
            for (index, header) in [[0x11, 0x11, 0x11, 0], [0x11, 0x23, 0x11, 0]]
                .iter()
                .enumerate()
            {
                let Message::Binary(packet) = server.next().await.unwrap().unwrap() else {
                    panic!("expected binary")
                };
                assert_eq!(&packet[..4], header);
                let mut payload = Vec::new();
                flate2::read::GzDecoder::new(&packet[12..])
                    .read_to_end(&mut payload)
                    .unwrap();
                if index == 0 {
                    let body: Value = serde_json::from_slice(&payload).unwrap();
                    assert_eq!(body["audio"]["rate"], 16000);
                    assert_eq!(body["request"]["enable_ddc"], false);
                } else {
                    assert_eq!(&packet[4..8], &(-2_i32).to_be_bytes());
                    assert_eq!(payload, vec![0; 32000]);
                }
            }
            for message in messages {
                if server.send(message).await.is_err() {
                    break;
                }
            }
            let _ = server.close(None).await;
            while let Some(Ok(message)) = server.next().await {
                if matches!(message, Message::Close(_)) {
                    break;
                }
            }
        });
        let result = tokio::time::timeout(Duration::from_secs(2), exchange(&mut client))
            .await
            .unwrap();
        drop(client);
        server.await.unwrap();
        result
    }

    #[test]
    fn websocket_probe_handles_partial_final_close_errors_and_limits() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                assert!(
                    fixture(vec![
                        Message::Ping(vec![1].into()),
                        response(1, b"{}"),
                        response(3, b"{}")
                    ])
                    .await
                );
                assert!(!fixture(vec![]).await); // upgrade / write alone is not success
                assert!(!fixture(vec![response(1, b"{}"), Message::Close(None)]).await);
                assert!(!fixture(vec![response(3, b"not-json")]).await);
                assert!(!fixture(vec![Message::Text("synthetic-diagnostic".into())]).await);
                assert!(!fixture(vec![response(1, b"{}"); MAX_MESSAGES]).await);
                assert!(!fixture(vec![response(3, &vec![b'a'; MAX_BYTES + 1])]).await);
                assert!(!fixture(vec![Message::Binary(vec![0; MAX_BYTES + 1].into())]).await);
            });
    }

    #[test]
    fn websocket_handshake_uses_headers_and_does_not_follow_redirects() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    request.push(stream.read_u8().await.unwrap());
                    assert!(request.len() < 8192);
                }
                let request = String::from_utf8(request).unwrap().to_ascii_lowercase();
                assert!(request.contains("x-api-key: synthetic-key\r\n"));
                assert!(!request.contains("x-api-app-key:"));
                stream.write_all(format!("HTTP/1.1 302 Found\r\nLocation: ws://{address}/redirected\r\nContent-Length: 0\r\n\r\n").as_bytes()).await.unwrap();
                assert!(tokio::time::timeout(Duration::from_millis(200), listener.accept()).await.is_err());
            });
            assert!(!connect_and_probe(&ProbeRequest { endpoint: format!("ws://{address}/asr"),
                headers: vec![("x-api-key", "synthetic-key".into())] }).await);
            server.await.unwrap();
        });
    }

    #[test]
    fn websocket_transport_deadline_covers_a_stalled_recognizer() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let address = listener.local_addr().unwrap();
                let server = tokio::spawn(async move {
                    let (stream, _) = listener.accept().await.unwrap();
                    let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
                    // Accept both packets, but never send a terminal result.
                    assert!(socket.next().await.unwrap().is_ok());
                    assert!(socket.next().await.unwrap().is_ok());
                    tokio::time::sleep(Duration::from_secs(30)).await;
                });
                let started = std::time::Instant::now();
                let result = tokio::task::spawn_blocking(move || {
                    WebSocketTransport.probe(&ProbeRequest {
                        endpoint: format!("ws://{address}/asr"),
                        headers: vec![],
                    })
                })
                .await
                .unwrap();
                assert!(!result);
                assert!(started.elapsed() >= TOTAL_TIMEOUT);
                assert!(started.elapsed() < Duration::from_secs(20));
                server.abort();
            });
    }
}
