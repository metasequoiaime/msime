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

/// 测试没通过的原因，只到设置页能说清该改哪一项的粒度：握手的 HTTP 状态码和错误帧里的数字错误码可以给用户看，服务端的错误文字不给。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    /// 握手没有升级成 WebSocket，服务端回了这个 HTTP 状态码（密钥无效是 401）。
    Rejected(u16),
    /// 连上之后服务端报错：错误帧（带它的错误码）或结果里带了非零 `code`、`error`。
    Service(Option<i64>),
    /// 地址解析、TCP、TLS 失败，或者在最终结果之前连接断了。
    Connection,
    /// 超过五秒没握手成功，或者十五秒内没等到最终结果。
    Timeout,
    /// 收到的不是豆包协议的帧，或者帧数、字节数超过上限。
    InvalidResponse,
}

pub trait Transport {
    /// Ok requires a valid terminal ASR response, not merely a connection.
    fn probe(&self, request: &ProbeRequest) -> Result<(), Failure>;
}

pub struct WebSocketTransport;

impl Transport for WebSocketTransport {
    fn probe(&self, request: &ProbeRequest) -> Result<(), Failure> {
        // Called on the desktop's blocking worker, never on its UI executor.
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return Err(Failure::Connection);
        };
        let result = runtime.block_on(async {
            tokio::time::timeout(TOTAL_TIMEOUT, connect_and_probe(request))
                .await
                .unwrap_or(Err(Failure::Timeout))
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

async fn connect_and_probe(probe: &ProbeRequest) -> Result<(), Failure> {
    let Ok(mut request) = probe.endpoint.as_str().into_client_request() else {
        return Err(Failure::Connection);
    };
    for (name, value) in &probe.headers {
        let Ok(mut value) = value.parse::<tokio_tungstenite::tungstenite::http::HeaderValue>()
        else {
            return Err(Failure::Connection);
        };
        value.set_sensitive(true);
        request.headers_mut().insert(*name, value);
    }
    // 与上游默认实现相同，只信任内置 Mozilla 根证书，不使用系统根证书或自定义校验器。
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let connector = tokio_tungstenite::Connector::Rustls(std::sync::Arc::new(
        rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    ));
    // 不跟随重定向，凭据留在所选地址；五秒截止时间包含 DNS、TCP、TLS 与升级。
    let mut socket = match tokio::time::timeout(
        Duration::from_secs(5),
        tokio_tungstenite::connect_async_tls_with_config(
            request,
            Some(socket_config()),
            false,
            Some(connector),
        ),
    )
    .await
    {
        Ok(Ok((socket, _))) => socket,
        // 只取状态码；握手响应的正文（豆包写的是 `{"error":"Invalid X-Api-Key"}` 这样的文字）不读也不转发。
        Ok(Err(tokio_tungstenite::tungstenite::Error::Http(response))) => {
            return Err(Failure::Rejected(response.status().as_u16()))
        }
        Ok(Err(_)) => return Err(Failure::Connection),
        Err(_) => return Err(Failure::Timeout),
    };
    let result = exchange(&mut socket).await;
    // Completion is already established; best-effort close cannot turn a
    // valid final response into a failure or keep the settings request alive.
    let _ = tokio::time::timeout(Duration::from_millis(100), socket.close(None)).await;
    result
}

async fn exchange<S: AsyncRead + AsyncWrite + Unpin>(
    socket: &mut WebSocketStream<S>,
) -> Result<(), Failure> {
    let packets = [
        doubao_frame::start_frame(true, true, false, ""),
        doubao_frame::audio_frame(2, &vec![0; 16000 * 2], true),
    ];
    for packet in packets {
        if socket.send(Message::Binary(packet.into())).await.is_err() {
            return Err(Failure::Connection);
        }
    }
    let mut received = 0usize;
    for _ in 0..MAX_MESSAGES {
        let Some(Ok(message)) = socket.next().await else {
            return Err(Failure::Connection);
        };
        received = received.saturating_add(message.len());
        if received > MAX_BYTES {
            return Err(Failure::InvalidResponse);
        }
        match message {
            Message::Binary(frame) => {
                if response_state(&frame)? {
                    return Ok(());
                }
            }
            Message::Ping(_) => {
                // Tungstenite queued the matching pong while reading.
                if socket.flush().await.is_err() {
                    return Err(Failure::Connection);
                }
            }
            Message::Pong(_) => {}
            Message::Close(_) => return Err(Failure::Connection),
            _ => return Err(Failure::InvalidResponse),
        }
    }
    Err(Failure::InvalidResponse)
}

/// `Ok(true)` 是最终结果，`Ok(false)` 是中间结果。
fn response_state(frame: &[u8]) -> Result<bool, Failure> {
    // Error frames and unsupported flags are failures regardless of their
    // message text. Only the numeric code of an error frame reaches the UI.
    if frame.len() >= 2 && frame[1] >> 4 == 0x0f {
        return Err(Failure::Service(
            doubao_frame::decode_error_code(frame).map(i64::from),
        ));
    }
    if frame.len() < 4 || frame[1] & 0x0f > 3 || frame[3] != 0 {
        return Err(Failure::InvalidResponse);
    }
    let (last, _, payload) =
        doubao_frame::decode_json_frame(frame).ok_or(Failure::InvalidResponse)?;
    let body: Value = serde_json::from_slice(&payload).map_err(|_| Failure::InvalidResponse)?;
    if !body.is_object() {
        return Err(Failure::InvalidResponse);
    }
    if body.get("error").is_some_and(|v| !v.is_null()) {
        return Err(Failure::Service(None));
    }
    // Some gateways wrap the recognizer result. Silence may legitimately
    // produce an empty object or no transcript; malformed JSON is not success.
    for candidate in [&body, body.get("payload_msg").unwrap_or(&Value::Null)] {
        if let Some(code) = candidate.get("code") {
            if code.as_i64() != Some(0) {
                return Err(Failure::Service(code.as_i64()));
            }
        }
    }
    Ok(last)
}

/// 设置页上的一句话：说出是哪一步没通过、该去改什么。
fn failure_message(failure: Failure) -> String {
    match failure {
        Failure::Rejected(401) => {
            "豆包认证失败（HTTP 401），请确认密钥来自豆包语音控制台，且鉴权方式与之对应。".into()
        }
        Failure::Rejected(403) => {
            "豆包拒绝访问（HTTP 403），请检查密钥的权限，以及资源 ID 对应的服务是否已开通。".into()
        }
        Failure::Rejected(429) => {
            "豆包限流或额度不足（HTTP 429），请稍后重试或检查资源用量。".into()
        }
        Failure::Rejected(status @ 500..=599) => {
            format!("豆包服务暂时不可用（HTTP {status}），请稍后重试。")
        }
        Failure::Rejected(status) => {
            format!("豆包拒绝了连接（HTTP {status}），请检查接口地址和资源 ID。")
        }
        // 《大模型流式语音识别 API》的错误码表：450xxxxx 是请求本身的问题（参数无效、空音频、音频格式），550xxxxx 是服务内部错误，其中 55000031 是服务过载。后者改配置没有用。
        Failure::Service(Some(code @ 55_000_000..=55_099_999)) => {
            format!("豆包服务暂时不可用（错误码 {code}），请稍后重试。")
        }
        Failure::Service(Some(code)) => {
            format!("豆包返回错误码 {code}，请检查资源 ID 和识别选项。")
        }
        Failure::Service(None) => "豆包返回了错误，请检查资源 ID 和识别选项。".into(),
        Failure::Connection => "无法连接豆包或连接中断，请检查接口地址和网络。".into(),
        Failure::Timeout => "豆包测试超时，请检查网络后重试。".into(),
        Failure::InvalidResponse => "豆包的响应无法识别，请确认接口地址是豆包流式识别接口。".into(),
    }
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
        && reqwest::Url::parse(endpoint)
            .ok()
            .is_some_and(|url| crate::endpoint::valid_endpoint_shape(endpoint, &url, "wss"));
    if !valid {
        return ProbeResult {
            ok: false,
            message: "请填写有效的 WSS 接口、资源 ID 及对应鉴权方式的凭据。".into(),
        };
    }
    match transport.probe(&ProbeRequest {
        endpoint: endpoint.to_owned(),
        headers: headers.unwrap_or_default(),
    }) {
        Ok(()) => ProbeResult {
            ok: true,
            message: "连接成功，豆包语音识别凭据有效。".into(),
        },
        Err(failure) => ProbeResult {
            ok: false,
            message: failure_message(failure),
        },
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
            fn probe(&self, request: &ProbeRequest) -> Result<(), Failure> {
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
                if self.legacy {
                    Ok(())
                } else {
                    Err(Failure::Rejected(401))
                }
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
            fn probe(&self, _: &ProbeRequest) -> Result<(), Failure> {
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
            assert_eq!(response_state(&frame), Ok(true));
        }
        for (body, failure) in [
            (b"null".as_slice(), Failure::InvalidResponse),
            (b"[]", Failure::InvalidResponse),
            (b"invalid", Failure::InvalidResponse),
            (br#"{"code":401}"#, Failure::Service(Some(401))),
            (
                br#"{"error":"synthetic-diagnostic"}"#,
                Failure::Service(None),
            ),
            (
                br#"{"payload_msg":{"code":403}}"#,
                Failure::Service(Some(403)),
            ),
        ] {
            let frame = doubao_frame::encode_json_frame(9, 3, -2, body);
            assert_eq!(response_state(&frame), Err(failure));
        }
        // 错误帧只取错误码（这里是 45000001，请求参数无效），后面的 UTF-8 错误文字不解析。
        let mut error = vec![0x11, 0xf0, 0x00, 0];
        error.extend_from_slice(&45_000_001_i32.to_be_bytes());
        error.extend_from_slice(&20_u32.to_be_bytes());
        error.extend_from_slice(b"synthetic-diagnostic");
        assert_eq!(
            response_state(&error),
            Err(Failure::Service(Some(45_000_001)))
        );
        assert_eq!(response_state(&[0x11, 0xf0]), Err(Failure::Service(None)));
        assert_eq!(
            response_state(&doubao_frame::encode_json_frame(9, 1, 1, b"{}")),
            Ok(false)
        );
    }

    // Actual WebSocket framing over an in-memory duplex: no external service,
    // DNS, microphone, credentials or transcript logging in regression tests.
    async fn fixture(messages: Vec<Message>) -> Result<(), Failure> {
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
                assert_eq!(
                    fixture(vec![
                        Message::Ping(vec![1].into()),
                        response(1, b"{}"),
                        response(3, b"{}")
                    ])
                    .await,
                    Ok(())
                );
                // upgrade / write alone is not success
                assert!(fixture(vec![]).await.is_err());
                assert_eq!(
                    fixture(vec![response(1, b"{}"), Message::Close(None)]).await,
                    Err(Failure::Connection)
                );
                assert_eq!(
                    fixture(vec![response(3, b"not-json")]).await,
                    Err(Failure::InvalidResponse)
                );
                assert_eq!(
                    fixture(vec![Message::Text("synthetic-diagnostic".into())]).await,
                    Err(Failure::InvalidResponse)
                );
                assert_eq!(
                    fixture(vec![response(1, b"{}"); MAX_MESSAGES]).await,
                    Err(Failure::InvalidResponse)
                );
                assert!(fixture(vec![response(3, &vec![b'a'; MAX_BYTES + 1])])
                    .await
                    .is_err());
                assert!(
                    fixture(vec![Message::Binary(vec![0; MAX_BYTES + 1].into())])
                        .await
                        .is_err()
                );
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
            assert_eq!(connect_and_probe(&ProbeRequest { endpoint: format!("ws://{address}/asr"),
                headers: vec![("x-api-key", "synthetic-key".into())] }).await, Err(Failure::Rejected(302)));
            server.await.unwrap();
        });
    }

    // 豆包对无效密钥在握手时回 401 和一段 JSON 文字（实测 `{"error":"Invalid X-Api-Key"}`）：设置页要说出是认证没通过，而不是笼统的「未成功或超时」，也不转述那段文字。
    #[test]
    fn rejected_handshake_reports_its_status_without_the_body() {
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
                let body = r#"{"error":"synthetic-diagnostic"}"#;
                stream.write_all(format!("HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            });
            let failure = connect_and_probe(&ProbeRequest { endpoint: format!("ws://{address}/asr"),
                headers: vec![("x-api-key", "synthetic-key".into())] }).await.unwrap_err();
            server.await.unwrap();
            assert_eq!(failure, Failure::Rejected(401));
        });
    }

    #[test]
    fn each_failure_names_its_cause_and_never_echoes_input() {
        struct Failing(Failure);
        impl Transport for Failing {
            fn probe(&self, _: &ProbeRequest) -> Result<(), Failure> {
                Err(self.0)
            }
        }
        for (failure, explanation) in [
            (Failure::Rejected(401), "豆包认证失败（HTTP 401）"),
            (Failure::Rejected(403), "豆包拒绝访问（HTTP 403）"),
            (Failure::Rejected(429), "豆包限流或额度不足（HTTP 429）"),
            (Failure::Rejected(404), "豆包拒绝了连接（HTTP 404）"),
            (
                Failure::Rejected(503),
                "豆包服务暂时不可用（HTTP 503），请稍后重试",
            ),
            (
                Failure::Service(Some(45_000_001)),
                "豆包返回错误码 45000001",
            ),
            (
                Failure::Service(Some(55_000_031)),
                "豆包服务暂时不可用（错误码 55000031），请稍后重试",
            ),
            (
                Failure::Service(Some(55_000_002)),
                "豆包服务暂时不可用（错误码 55000002），请稍后重试",
            ),
            (Failure::Service(None), "豆包返回了错误"),
            (Failure::Connection, "无法连接豆包"),
            (Failure::Timeout, "豆包测试超时"),
            (Failure::InvalidResponse, "豆包的响应无法识别"),
        ] {
            let result = test(&config(), &Failing(failure));
            assert!(!result.ok);
            assert!(
                result.message.starts_with(explanation),
                "{failure:?}: {}",
                result.message
            );
            assert!(result.message.len() < 256);
            for private in [
                "synthetic-key",
                "fixture.invalid",
                "fixture-resource",
                "stale-app",
            ] {
                assert!(!result.message.contains(private));
            }
        }
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
                assert_eq!(result, Err(Failure::Timeout));
                assert!(started.elapsed() >= TOTAL_TIMEOUT);
                assert!(started.elapsed() < Duration::from_secs(20));
                server.abort();
            });
    }
}
