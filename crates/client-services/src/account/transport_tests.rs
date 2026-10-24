//! #8201 新接口的传输测试（请求的读取和响应的拼装也给 `chat_stream` 的测试用）：`BackendAccountClient::loopback` 连本机 `TcpListener` 假服务器，回放服务端的响应形状（msime-cloud `origin/main`），核对发出去的方法、路径、凭据和正文，以及失败时的状态码、服务端错误码和 `Retry-After`。

use super::*;
use crate::diagnostics::upload::{prepare_upload, DiagnosticUpload};
use crate::diagnostics::DiagnosticBundle;
use serde_json::{json, Value};
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};

fn token(character: char) -> String {
    character.to_string().repeat(64)
}

/// 一个完整的 HTTP/1.1 响应，带 `Content-Length` 和 `Connection: close`，客户端不会复用连接。
pub(super) fn http(status: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        response.push_str(&format!("{name}: {value}\r\n"));
    }
    response.push_str("\r\n");
    let mut bytes = response.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

pub(super) fn json_response(status: &str, body: Value) -> Vec<u8> {
    http(
        status,
        &[("Content-Type", "application/json")],
        body.to_string().as_bytes(),
    )
}

pub(super) fn error_response(status: &str, code: &str, headers: &[(&str, &str)]) -> Vec<u8> {
    let mut all = vec![("Content-Type", "application/json")];
    all.extend_from_slice(headers);
    http(
        status,
        &all,
        json!({ "error": { "code": code, "message": code } })
            .to_string()
            .as_bytes(),
    )
}

/// 一个收到的请求：请求行和头（头名转成小写）以及正文。
pub(super) struct Received {
    pub(super) head: String,
    pub(super) body: Vec<u8>,
}

impl Received {
    pub(super) fn line(&self) -> &str {
        self.head.lines().next().unwrap()
    }

    pub(super) fn header(&self, name: &str) -> Option<&str> {
        self.head.lines().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name).then(|| value.trim())
        })
    }

    pub(super) fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

/// 依次回答 `responses` 里的每一个，每个连接一个；收到的请求按顺序交回。
fn serve(responses: Vec<Vec<u8>>) -> (String, Receiver<Received>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (sent, received) = mpsc::channel();
    std::thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            sent.send(request).unwrap();
            std::io::Write::write_all(&mut stream, &response).unwrap();
        }
    });
    (origin, received)
}

pub(super) fn read_request(stream: &mut std::net::TcpStream) -> Received {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8192];
    let head_end = loop {
        let read = stream.read(&mut buffer).unwrap();
        assert_ne!(read, 0, "request ended before its headers");
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break end;
        }
    };
    let head = String::from_utf8(bytes[..head_end].to_vec()).unwrap();
    let mut body = bytes[head_end + 4..].to_vec();
    let received = Received {
        head,
        body: Vec::new(),
    };
    if let Some(length) = received.header("content-length") {
        let length: usize = length.parse().unwrap();
        while body.len() < length {
            let read = stream.read(&mut buffer).unwrap();
            assert_ne!(read, 0, "request ended before its body");
            body.extend_from_slice(&buffer[..read]);
        }
    } else if received
        .header("transfer-encoding")
        .is_some_and(|value| value.eq_ignore_ascii_case("chunked"))
    {
        while !body.ends_with(b"0\r\n\r\n") {
            let read = stream.read(&mut buffer).unwrap();
            assert_ne!(read, 0, "request ended before its last chunk");
            body.extend_from_slice(&buffer[..read]);
        }
    }
    Received { body, ..received }
}

fn client(origin: &str) -> BackendAccountClient {
    BackendAccountClient::loopback(origin).unwrap()
}

#[test]
fn dictionary_add_posts_to_the_collection_route() {
    let entry = json!({
        "id": "a".repeat(64), "kind": "english", "code": "fixture",
        "word": "synthetic", "weight": 1, "revision": 1
    });
    let (origin, received) = serve(vec![json_response(
        "201 Created",
        json!({"revision": 1, "previous": null, "replacement": entry}),
    )]);
    let result = client(&origin)
        .add_dictionary(
            DictionaryKind::English,
            "fixture",
            "synthetic",
            1,
            &token('a'),
        )
        .unwrap();
    assert_eq!(result.revision, 1);
    let request = received.recv().unwrap();
    assert_eq!(
        request.line(),
        "POST /v1/users/me/dictionaries/english HTTP/1.1"
    );
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {}", token('a')).as_str())
    );
    assert_eq!(
        request.json(),
        json!({"code": "fixture", "word": "synthetic", "weight": 1})
    );
}

#[test]
fn translate_posts_the_validated_body_and_normalizes_the_answer() {
    let (origin, received) = serve(vec![
        json_response(
            "200 OK",
            json!({ "code": 200, "data": ["Metasequoia\n", "水杉"] }),
        ),
        error_response(
            "429 Too Many Requests",
            "rate_limit_exceeded",
            &[("Retry-After", "30")],
        ),
    ]);
    let client = client(&origin);
    let request = AccountTranslateRequest {
        texts: vec!["水杉".into(), "水杉".into()],
        source_lang: "zh".into(),
        target_lang: "en".into(),
    };
    let translations = client
        .translate(&request, Duration::from_secs(10), &token('a'))
        .unwrap();
    assert_eq!(
        translations.translations,
        [Some("Metasequoia".to_owned()), None]
    );
    let sent = received.recv().unwrap();
    assert_eq!(sent.line(), "POST /v1/translate HTTP/1.1");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {}", token('a')).as_str())
    );
    assert_eq!(
        sent.json(),
        json!({ "texts": ["水杉", "水杉"], "source_lang": "ZH", "target_lang": "EN" })
    );
    let limited = client
        .translate(&request, Duration::from_secs(10), &token('a'))
        .unwrap_err();
    assert_eq!(limited.code(), "account_rate_limited");
    assert_eq!(limited.retry_after_seconds, Some(30));
    assert_eq!(limited.status, Some(429));
}

#[test]
fn phrases_round_trip_and_a_stale_revision_is_a_conflict() {
    let list = json!({ "revision": 3, "phrases": [{ "id": "p1", "text": "你好", "group": "", "position": 0 }] });
    let (origin, received) = serve(vec![
        json_response("200 OK", list.clone()),
        json_response(
            "200 OK",
            json!({ "revision": 4, "phrases": list["phrases"] }),
        ),
        error_response("409 Conflict", "revision_conflict", &[]),
        // 服务端回了不合规的列表：不可用，不交给页面。
        json_response(
            "200 OK",
            json!({ "revision": 1, "phrases": [{ "id": "bad id", "text": "x", "group": "", "position": 0 }] }),
        ),
    ]);
    let client = client(&origin);
    let loaded = client.phrases(&token('a')).unwrap();
    assert_eq!(loaded.revision, 3);
    assert_eq!(
        received.recv().unwrap().line(),
        "GET /v1/users/me/phrases HTTP/1.1"
    );
    let saved = client.put_phrases(&loaded, &token('a')).unwrap();
    assert_eq!(saved.revision, 4);
    let sent = received.recv().unwrap();
    assert_eq!(sent.line(), "PUT /v1/users/me/phrases HTTP/1.1");
    assert_eq!(sent.json(), list);
    assert_eq!(
        client.put_phrases(&loaded, &token('a')).unwrap_err().code(),
        "account_conflict"
    );
    assert_eq!(
        client.phrases(&token('a')).unwrap_err().code(),
        "account_unavailable"
    );
}

#[test]
fn devices_are_listed_and_revoked_by_server_id() {
    let id = "ab".repeat(32);
    let (origin, received) = serve(vec![
        json_response(
            "200 OK",
            json!({ "sessions": [{
                "id": id,
                "platform": "web",
                "name": "Chrome · Windows",
                "app_version": "",
                "created_at": "2026-10-01T00:00:00Z",
                "last_active": "2026-10-16T00:00:00Z",
                "current": false,
            }] }),
        ),
        http("204 No Content", &[], b""),
        error_response("404 Not Found", "session_not_found", &[]),
    ]);
    let client = client(&origin);
    let devices = client.devices(&token('a')).unwrap();
    assert_eq!(devices.sessions[0].name, "Chrome · Windows");
    assert_eq!(
        received.recv().unwrap().line(),
        "GET /v1/users/me/sessions HTTP/1.1"
    );
    client.revoke_device(&id, &token('a')).unwrap();
    assert_eq!(
        received.recv().unwrap().line(),
        format!("DELETE /v1/users/me/sessions/{id} HTTP/1.1")
    );
    assert_eq!(
        client.revoke_device(&id, &token('a')).unwrap_err().code(),
        "account_not_found"
    );
}

#[test]
fn data_summary_and_delete_follow_the_server_contract() {
    let (origin, received) = serve(vec![
        json_response(
            "200 OK",
            json!({ "bytes": 30, "sections": [{ "id": "preferences", "bytes": 10, "items": 1 }, { "id": "avatar", "bytes": 0, "items": 1 }] }),
        ),
        http("204 No Content", &[], b""),
        error_response("403 Forbidden", "recent_login_required", &[]),
    ]);
    let client = client(&origin);
    let summary = client.data_summary(&token('a')).unwrap();
    assert_eq!(summary.sections.len(), 2);
    assert_eq!(
        received.recv().unwrap().line(),
        "GET /v1/users/me/data HTTP/1.1"
    );
    let sections = vec!["phrases".to_owned(), "voice".to_owned()];
    client.data_delete(&sections, &token('a')).unwrap();
    let sent = received.recv().unwrap();
    assert_eq!(sent.line(), "DELETE /v1/users/me/data HTTP/1.1");
    assert_eq!(sent.json(), json!({ "sections": ["phrases", "voice"] }));
    let fresh = client.data_delete(&sections, &token('a')).unwrap_err();
    assert_eq!(fresh.code(), "account_recent_login_required");
    // 本地就拒绝的分区不发请求：假服务器只准备了三个响应。
    assert_eq!(
        client
            .data_delete(&["community".to_owned()], &token('a'))
            .unwrap_err()
            .code(),
        "account_invalid"
    );
}

#[test]
fn clipboard_pin_retention_and_clear_follow_the_server_contract() {
    let id = "0123456789abcdef".repeat(4);
    // msime-cloud `clipboard.go`：置顶和保留期成功时回 204，清空也是 204；置顶的条目不在时回 404 `not_found`。
    let (origin, received) = serve(vec![
        http("204 No Content", &[], b""),
        http("204 No Content", &[], b""),
        http("204 No Content", &[], b""),
        error_response("404 Not Found", "not_found", &[]),
    ]);
    let client = client(&origin);
    client.pin_clipboard(&id, true, &token('a')).unwrap();
    let sent = received.recv().unwrap();
    assert_eq!(
        sent.line(),
        format!("PUT /v1/users/me/clipboard/{id}/pin HTTP/1.1")
    );
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {}", token('a')).as_str())
    );
    assert_eq!(sent.json(), json!({ "pinned": true }));
    client.set_clipboard_retention(0, &token('a')).unwrap();
    let sent = received.recv().unwrap();
    assert_eq!(sent.line(), "PUT /v1/users/me/clipboard/retention HTTP/1.1");
    assert_eq!(sent.json(), json!({ "days": 0 }));
    client.delete_clipboard(None, &token('a')).unwrap();
    let sent = received.recv().unwrap();
    assert_eq!(sent.line(), "DELETE /v1/users/me/clipboard HTTP/1.1");
    assert!(sent.body.is_empty());
    assert_eq!(
        client.pin_clipboard(&id, false, &token('a')).unwrap_err(),
        AccountError::NotFound
    );
    assert_eq!(received.recv().unwrap().json(), json!({ "pinned": false }));
    // 本地就拒绝的编号和天数不发请求：假服务器的四个响应已经用完，真发出去会是 `Unavailable`。
    for bad in [
        id.to_uppercase(),
        id[..63].to_owned(),
        "../retention".to_owned(),
    ] {
        assert_eq!(
            client.pin_clipboard(&bad, true, &token('a')).unwrap_err(),
            AccountError::Invalid,
            "{bad}"
        );
    }
    for days in [2, 3, 31, 365] {
        assert_eq!(
            client
                .set_clipboard_retention(days, &token('a'))
                .unwrap_err(),
            AccountError::Invalid,
            "{days}"
        );
    }
}

fn zip_bytes() -> Vec<u8> {
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    archive
        .start_file("profile.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    std::io::Write::write_all(&mut archive, br#"{"id":"synthetic-user"}"#).unwrap();
    archive.finish().unwrap().into_inner()
}

#[test]
fn the_export_is_streamed_to_the_destination_only_when_it_is_a_whole_zip() {
    let archive = zip_bytes();
    let zip_headers = [
        ("Content-Type", "application/zip"),
        (
            "Content-Disposition",
            r#"attachment; filename="msime-data-2026-10-16.zip""#,
        ),
    ];
    let (origin, received) = serve(vec![
        http("200 OK", &zip_headers, &archive),
        // 服务端写到一半断了：中央目录读不出来。
        http("200 OK", &zip_headers, &archive[..archive.len() / 2]),
        error_response(
            "429 Too Many Requests",
            "rate_limit_exceeded",
            &[("Retry-After", "3600")],
        ),
    ]);
    let client = client(&origin);
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("export.zip");
    let exported = client.data_export(&destination, &token('a')).unwrap();
    assert_eq!(exported.filename, "msime-data-2026-10-16.zip");
    assert_eq!(exported.bytes, archive.len() as u64);
    assert_eq!(std::fs::read(&destination).unwrap(), archive);
    let sent = received.recv().unwrap();
    assert_eq!(sent.line(), "GET /v1/users/me/data/export HTTP/1.1");
    assert_eq!(sent.header("accept"), Some("application/zip"));

    // 截断的包不覆盖目标位置上原有的文件，也不留下临时文件。
    assert_eq!(
        client
            .data_export(&destination, &token('a'))
            .unwrap_err()
            .code(),
        "account_unavailable"
    );
    assert_eq!(std::fs::read(&destination).unwrap(), archive);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);

    let limited = client.data_export(&destination, &token('a')).unwrap_err();
    assert_eq!(limited.code(), "account_rate_limited");
    assert_eq!(limited.retry_after_seconds, Some(3600));
}

#[test]
fn feedback_is_sent_as_multipart_with_its_screenshots() {
    let directory = tempfile::tempdir().unwrap();
    let screenshot = directory.path().join("shot.png");
    let mut png = std::io::Cursor::new(Vec::new());
    image::GrayImage::new(4, 3)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    std::fs::write(&screenshot, png.get_ref()).unwrap();
    let feedback = prepare_feedback(&AccountFeedbackInput {
        kind: "bug".into(),
        text: "合成反馈".into(),
        platform: "android".into(),
        app_version: "0.9.0".into(),
        edition: "full".into(),
        screenshots: vec![screenshot],
        ..Default::default()
    })
    .unwrap();
    let (origin, received) = serve(vec![
        json_response(
            "201 Created",
            json!({ "id": "6f9619ff-8b86-d011-b42d-00c04fc964ff", "status": "received" }),
        ),
        error_response("413 Payload Too Large", "feedback_too_large", &[]),
    ]);
    let client = client(&origin);
    let receipt = client.feedback(&feedback, &token('a')).unwrap();
    assert_eq!(receipt.status, "received");
    let sent = received.recv().unwrap();
    assert_eq!(sent.line(), "POST /v1/feedback HTTP/1.1");
    assert!(sent
        .header("content-type")
        .unwrap()
        .starts_with("multipart/form-data; boundary="));
    let body = String::from_utf8_lossy(&sent.body);
    assert!(body.contains(r#"name="payload""#));
    assert!(body.contains(r#"name="screenshots"; filename="screenshot-1.png""#));
    assert!(body.contains("Content-Type: image/png"));
    assert!(body.contains(r#""text":"合成反馈""#));
    assert_eq!(
        client.feedback(&feedback, &token('a')).unwrap_err().code(),
        "account_too_large"
    );
}

#[test]
fn the_download_link_posts_the_platform_and_reads_the_masked_address() {
    // msime-cloud `download_link.go`：成功回 202 `{sent_to}`，没有已验证邮箱回 409 `no_verified_email`，邮件没配置回 503 `provider_disabled`，限流回 429。
    let (origin, received) = serve(vec![
        json_response("202 Accepted", json!({ "sent_to": "u***@example.test" })),
        json_response(
            "202 Accepted",
            json!({ "sent_to": "u***@example.test\u{7}" }),
        ),
        error_response("409 Conflict", "no_verified_email", &[]),
        error_response("503 Service Unavailable", "provider_disabled", &[]),
        error_response(
            "429 Too Many Requests",
            "rate_limit_exceeded",
            &[("Retry-After", "3600")],
        ),
    ]);
    let client = client(&origin);
    assert_eq!(
        client.download_link("harmony-pc", &token('a')).unwrap(),
        AccountDownloadLink {
            sent_to: Some("u***@example.test".into())
        }
    );
    let sent = received.recv().unwrap();
    assert_eq!(sent.line(), "POST /v1/users/me/download-link HTTP/1.1");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {}", token('a')).as_str())
    );
    assert_eq!(sent.header("content-type"), Some("application/json"));
    assert_eq!(sent.json(), json!({ "platform": "harmony-pc" }));
    // 邮件已经发出，收件地址不合规时只是不显示它。
    assert_eq!(
        client.download_link("ios", &token('a')).unwrap(),
        AccountDownloadLink { sent_to: None }
    );
    assert_eq!(
        received.recv().unwrap().json(),
        json!({ "platform": "ios" })
    );
    assert_eq!(
        client.download_link("ios", &token('a')).unwrap_err().code(),
        "account_no_verified_email"
    );
    assert_eq!(
        client.download_link("ios", &token('a')).unwrap_err().code(),
        "account_unavailable"
    );
    let limited = client.download_link("ios", &token('a')).unwrap_err();
    assert_eq!(limited.code(), "account_rate_limited");
    assert_eq!(limited.retry_after_seconds, Some(3600));
    // 服务端不认的平台不发请求：假服务器的五个响应已经用完。
    for platform in ["web", "Windows", "harmony_pc", ""] {
        assert_eq!(
            client
                .download_link(platform, &token('a'))
                .unwrap_err()
                .code(),
            "account_invalid",
            "{platform}"
        );
    }
}

#[test]
fn a_voice_contribution_is_sent_as_multipart_with_its_wav() {
    let wav = synthetic_wav(1, 16_000, 16, 32_000 * 2);
    let contribution = prepare_voice_contribution(&AccountVoiceContributionInput {
        language: "zh-CN".into(),
        provider: "system".into(),
        transcript: " 合成识别文本 ".into(),
        app_version: "0.9.0".into(),
        audio: wav.clone(),
    })
    .unwrap();
    let id = "0123456789abcdef".repeat(4);
    // msime-cloud `voice_contributions.go`：成功回 201 `{id}`（64 位小写十六进制），限流回 429 带 `Retry-After: 3600`，设备令牌回 401。
    let (origin, received) = serve(vec![
        json_response("201 Created", json!({ "id": id })),
        json_response("201 Created", json!({ "id": "not-an-id" })),
        error_response(
            "429 Too Many Requests",
            "rate_limit_exceeded",
            &[("Retry-After", "3600")],
        ),
        error_response("413 Payload Too Large", "audio_too_large", &[]),
    ]);
    let client = client(&origin);
    assert_eq!(
        client
            .voice_contribution(&contribution, &token('a'))
            .unwrap(),
        AccountVoiceContributionReceipt { id: id.clone() }
    );
    let sent = received.recv().unwrap();
    assert_eq!(sent.line(), "POST /v1/voice/contributions HTTP/1.1");
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {}", token('a')).as_str())
    );
    assert!(sent
        .header("content-type")
        .unwrap()
        .starts_with("multipart/form-data; boundary="));
    let body = String::from_utf8_lossy(&sent.body);
    assert!(body.contains(r#"name="payload""#));
    assert!(body.contains("Content-Type: application/json"));
    assert!(body.contains(r#""duration_ms":2000"#));
    assert!(body.contains(r#""transcript":"合成识别文本""#));
    assert!(body.contains(r#"name="audio"; filename="voice.wav""#));
    assert!(body.contains("Content-Type: audio/wav"));
    assert!(sent
        .body
        .windows(wav.len())
        .any(|window| window == wav.as_slice()));
    assert_eq!(
        client
            .voice_contribution(&contribution, &token('a'))
            .unwrap_err()
            .code(),
        "account_unavailable"
    );
    let limited = client
        .voice_contribution(&contribution, &token('a'))
        .unwrap_err();
    assert_eq!(limited.code(), "account_rate_limited");
    assert_eq!(limited.retry_after_seconds, Some(3600));
    assert_eq!(
        client
            .voice_contribution(&contribution, &token('a'))
            .unwrap_err()
            .code(),
        "account_too_large"
    );
}

fn upload() -> DiagnosticUpload {
    let bundle = DiagnosticBundle {
        sections: Some(
            json!({ "input_events": [{ "t_ms": 1, "kind": "commit" }] })
                .as_object()
                .unwrap()
                .clone(),
        ),
        ..Default::default()
    };
    prepare_upload("android", "0.9.0", "one_day", bundle).unwrap()
}

#[test]
fn diagnostics_upload_state_token_and_delete_reach_their_routes() {
    let id = "0123456789abcdef01234567";
    let (origin, received) = serve(vec![
        json_response(
            "201 Created",
            json!({ "id": id, "mcp_url": format!("https://api.msime.app/mcp/s/{id}"), "token": "msk_synthetic", "expires_at": "2026-10-17T00:00:00Z" }),
        ),
        json_response(
            "200 OK",
            json!({
                "snapshot": { "id": id, "created_at": "2026-10-16T00:00:00Z", "expires_at": "2026-10-17T00:00:00Z", "bytes": 64, "sections": ["input_events"], "token_hint": "etic" },
                "accesses": [{ "at": "2026-10-16T01:00:00Z", "tool": "list_sections", "arguments": {}, "result_count": 1, "bytes": 12 }],
            }),
        ),
        json_response("200 OK", json!({ "token": "msk_replacement" })),
        http("204 No Content", &[], b""),
        error_response("400 Bad Request", "invalid_input_events", &[]),
    ]);
    let client = client(&origin);
    let upload = upload();
    let receipt = client.diagnostics_upload(&upload, &token('a')).unwrap();
    assert_eq!(receipt.token, "msk_synthetic");
    let sent = received.recv().unwrap();
    assert_eq!(sent.line(), "POST /v1/users/me/diagnostics HTTP/1.1");
    assert_eq!(
        sent.json(),
        json!({ "platform": "android", "app_version": "0.9.0", "ttl": "one_day", "sections": { "input_events": [{ "t_ms": 1, "kind": "commit" }] } })
    );
    let state = client.diagnostics_state(&token('a')).unwrap();
    assert_eq!(state.accesses.len(), 1);
    assert_eq!(
        received.recv().unwrap().line(),
        "GET /v1/users/me/diagnostics HTTP/1.1"
    );
    assert_eq!(
        client.diagnostics_regenerate_token(&token('a')).unwrap(),
        "msk_replacement"
    );
    assert_eq!(
        received.recv().unwrap().line(),
        "POST /v1/users/me/diagnostics/token HTTP/1.1"
    );
    client.diagnostics_delete(&token('a')).unwrap();
    assert_eq!(
        received.recv().unwrap().line(),
        "DELETE /v1/users/me/diagnostics HTTP/1.1"
    );
    assert_eq!(
        client
            .diagnostics_upload(&upload, &token('a'))
            .unwrap_err()
            .code(),
        "account_invalid"
    );
}

#[test]
fn failure_codes_follow_the_documented_order() {
    let failure = |status: u16, code: &str| {
        AccountFailure::from_http_response(
            status,
            json!({ "error": { "code": code } }).to_string().as_bytes(),
            None,
        )
        .code()
    };
    assert_eq!(
        failure(403, "recent_login_required"),
        "account_recent_login_required"
    );
    assert_eq!(failure(413, "anything"), "account_too_large");
    assert_eq!(failure(400, "screenshot_too_large"), "account_too_large");
    assert_eq!(failure(404, "diagnostics_not_found"), "account_not_found");
    assert_eq!(failure(429, "rate_limit_exceeded"), "account_rate_limited");
    assert_eq!(failure(503, "server_busy"), "account_rate_limited");
    assert_eq!(failure(503, "skin_jobs_busy"), "account_unavailable");
    assert_eq!(failure(403, "account_banned"), "account_banned");
    assert_eq!(failure(400, "invalid_json"), "account_invalid");
    assert_eq!(
        failure(409, "no_verified_email"),
        "account_no_verified_email"
    );
    assert_eq!(failure(409, "revision_conflict"), "account_conflict");
    assert_eq!(AccountFailure::too_large().code(), "account_too_large");
    assert_eq!(
        AccountFailure::identity_changed().code(),
        "account_identity_changed"
    );
    // 已有的映射不变：AccountError 自己的码里没有这几个新码。
    assert_eq!(AccountError::Forbidden.code(), "account_unauthorized");
    assert_eq!(AccountError::NotFound.code(), "account_unavailable");
}

// ---- 云同步用的带细节的接口 ----

#[test]
fn sync_preference_requests_keep_the_conflict_code_and_retry_after() {
    let (origin, received) = serve(vec![
        json_response(
            "200 OK",
            json!({
                "fields": { "input.schema": { "type": "string" } },
                "maximum_bytes": 1_048_576,
                "update_mode": "replace",
                "revision_required": true,
            }),
        ),
        json_response(
            "200 OK",
            json!({ "revision": 4, "settings": { "input.schema": "wubi" } }),
        ),
        error_response("409 Conflict", "revision_conflict", &[]),
        error_response(
            "429 Too Many Requests",
            "rate_limit_exceeded",
            &[("Retry-After", "60")],
        ),
    ]);
    let client = BackendAccountClient::loopback(&origin).unwrap();
    let schema = client.preference_schema_detailed(&token('a')).unwrap();
    assert!(schema.fields.contains_key("input.schema"));
    assert_eq!(
        received.recv().unwrap().line(),
        "GET /v1/users/me/preferences/schema HTTP/1.1"
    );
    let cloud = client.preferences_detailed(&token('a')).unwrap();
    assert_eq!(cloud.revision, 4);
    received.recv().unwrap();
    let conflict = client
        .put_preferences_detailed(&cloud, &token('a'))
        .unwrap_err();
    assert_eq!(conflict.status, Some(409));
    assert_eq!(conflict.server_code.as_deref(), Some("revision_conflict"));
    let sent = received.recv().unwrap();
    assert_eq!(sent.line(), "PUT /v1/users/me/preferences HTTP/1.1");
    assert_eq!(sent.json()["revision"], 4);
    let limited = client
        .put_preferences_detailed(&cloud, &token('a'))
        .unwrap_err();
    assert_eq!(limited.retry_after_seconds, Some(60));
    assert_eq!(limited.code(), "account_rate_limited");
}

#[test]
fn the_dictionary_probe_reads_one_change_and_checks_the_cursor() {
    let (origin, received) = serve(vec![
        json_response(
            "200 OK",
            json!({ "changes": [{ "revision": 8, "reset": true, "previous": null, "replacement": null }], "next": 8, "has_more": false }),
        ),
        json_response(
            "200 OK",
            json!({ "changes": [], "next": 5, "has_more": false }),
        ),
        // 没有改动却往前跳的 next 不合规。
        json_response(
            "200 OK",
            json!({ "changes": [], "next": 6, "has_more": false }),
        ),
    ]);
    let client = BackendAccountClient::loopback(&origin).unwrap();
    let probe = client.dictionary_probe(5, &token('a')).unwrap();
    assert_eq!(
        probe,
        AccountDictionaryProbe {
            changed: true,
            revision: 8,
            reset: true,
        }
    );
    assert_eq!(
        received.recv().unwrap().line(),
        "GET /v1/users/me/dictionary/changes?after=5&limit=1 HTTP/1.1"
    );
    let unchanged = client.dictionary_probe(5, &token('a')).unwrap();
    assert!(!unchanged.changed && !unchanged.reset);
    assert_eq!(unchanged.revision, 5);
    assert_eq!(
        client.dictionary_probe(5, &token('a')).unwrap_err().code(),
        "account_unavailable"
    );
    assert_eq!(
        client.dictionary_probe(-1, &token('a')).unwrap_err().code(),
        "account_invalid"
    );
}

#[test]
fn the_snapshot_upload_reports_the_server_code() {
    let (origin, received) = serve(vec![
        error_response("409 Conflict", "revision_conflict", &[]),
        error_response(
            "503 Service Unavailable",
            "snapshot_restore_busy",
            &[("Retry-After", "5")],
        ),
        json_response("200 OK", json!({ "revision": 10, "reset": true })),
    ]);
    let directory = tempfile::tempdir().unwrap();
    let snapshot = directory.path().join("upload.ndjson");
    std::fs::write(&snapshot, b"{\"type\":\"header\"}\n").unwrap();
    let client = BackendAccountClient::loopback(&origin).unwrap();
    let conflict = client
        .dictionary_snapshot_upload(&snapshot, 9, &token('a'))
        .unwrap_err();
    assert_eq!(conflict.server_code.as_deref(), Some("revision_conflict"));
    assert_eq!(
        received.recv().unwrap().line(),
        "PUT /v1/users/me/dictionary/snapshot?revision=9 HTTP/1.1"
    );
    let busy = client
        .dictionary_snapshot_upload(&snapshot, 9, &token('a'))
        .unwrap_err();
    assert_eq!(busy.retry_after_seconds, Some(5));
    received.recv().unwrap();
    let restored = client
        .dictionary_snapshot_upload(&snapshot, 9, &token('a'))
        .unwrap();
    assert_eq!(restored.revision, 10);
    // 正文就是快照文件本身，流式发出。
    assert_eq!(received.recv().unwrap().body, b"{\"type\":\"header\"}\n");
}

#[test]
fn the_snapshot_download_reports_the_server_code() {
    let (origin, _received) = serve(vec![
        error_response(
            "429 Too Many Requests",
            "rate_limit_exceeded",
            &[("Retry-After", "30")],
        ),
        http(
            "200 OK",
            &[("Content-Type", "application/x-ndjson")],
            b"{\"type\":\"header\"}\n",
        ),
    ]);
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("download.ndjson");
    let client = BackendAccountClient::loopback(&origin).unwrap();
    let limited = client
        .dictionary_snapshot_download(&destination, &token('a'))
        .unwrap_err();
    assert_eq!(limited.retry_after_seconds, Some(30));
    assert_eq!(limited.status, Some(429));
    let bytes = client
        .dictionary_snapshot_download(&destination, &token('a'))
        .unwrap();
    assert_eq!(bytes, 18);
    assert_eq!(
        std::fs::read(&destination).unwrap(),
        b"{\"type\":\"header\"}\n"
    );
}

// ---- 宿主自己弹登录界面的登录 ----

/// `/v1/auth/login` 与 `/v1/auth/apple/web/login` 共用的成功响应。
fn session_tokens() -> Value {
    json!({
        "access_token": token('a'),
        "refresh_token": token('b'),
        "token_type": "Bearer",
        "expires_in": 900,
        "user": { "id": "fixture-user", "display_name": "Fixture", "created_at": "2026-01-01T00:00:00Z" }
    })
}

#[test]
fn a_google_challenge_without_a_target_asks_for_the_id_token_flow() {
    let (origin, received) = serve(vec![json_response(
        "201 Created",
        json!({ "challenge_id": "c".repeat(64), "expires_in": 300, "nonce": "fixture-nonce" }),
    )]);
    let challenge = client(&origin).challenge("google", "").unwrap();
    assert_eq!(challenge.nonce.as_deref(), Some("fixture-nonce"));
    let sent = received.recv().unwrap();
    assert_eq!(sent.line(), "POST /v1/auth/challenges HTTP/1.1");
    assert_eq!(sent.header("authorization"), None);
    assert_eq!(
        sent.json(),
        json!({ "provider": "google", "target": "", "purpose": "login" })
    );
    // 其余 provider 的目标仍按原来的规则检查。
    assert_eq!(
        client(&origin).challenge("email", "").unwrap_err(),
        AccountError::Invalid
    );
}

#[test]
fn an_apple_web_sign_in_posts_the_digest_then_the_grant_and_verifier() {
    let authorization_url = "https://appleid.apple.com/auth/authorize?client_id=app.msime.signin&state=fixture-state&nonce=fixture-nonce";
    let (origin, received) = serve(vec![
        json_response(
            "201 Created",
            json!({ "authorization_url": authorization_url, "expires_in": 300 }),
        ),
        json_response("200 OK", session_tokens()),
        error_response("400 Bad Request", "invalid_grant", &[]),
        json_response(
            "201 Created",
            json!({ "authorization_url": "https://evil.test/auth/authorize?state=x", "expires_in": 300 }),
        ),
    ]);
    let client = client(&origin);
    let code_challenge = apple_web::code_challenge(&"v".repeat(43));
    let challenge = client
        .apple_web_challenge(&code_challenge, "app.msime.android.wubi")
        .unwrap();
    assert_eq!(challenge.authorization_url, authorization_url);
    assert_eq!(challenge.expires_in, 300);
    let sent = received.recv().unwrap();
    assert_eq!(sent.line(), "POST /v1/auth/apple/web HTTP/1.1");
    assert_eq!(sent.header("authorization"), None);
    assert_eq!(
        sent.json(),
        json!({ "code_challenge": code_challenge, "code_challenge_method": "S256", "app": "app.msime.android.wubi", "purpose": "login" })
    );

    let grant = "g".repeat(42) + "A";
    let verifier = "v".repeat(43);
    let tokens = client.apple_web_login(&grant, &verifier).unwrap();
    assert_eq!(tokens.user.id, "fixture-user");
    let sent = received.recv().unwrap();
    assert_eq!(sent.line(), "POST /v1/auth/apple/web/login HTTP/1.1");
    assert_eq!(sent.header("authorization"), None);
    assert_eq!(
        sent.json(),
        json!({ "grant": grant, "code_verifier": verifier })
    );
    // 服务端不认这个 grant（不认识、用过或过期）时是 account_invalid，宿主据此保留等待中的流程。
    assert_eq!(
        client.apple_web_login(&grant, &verifier).err(),
        Some(AccountError::Invalid)
    );
    received.recv().unwrap();
    // 不是 Apple 授权页的链接不交给宿主打开。
    assert_eq!(
        client
            .apple_web_challenge(&code_challenge, "app.msime.android")
            .unwrap_err(),
        AccountError::Unavailable
    );
}

#[test]
fn apple_web_requests_are_checked_before_they_are_sent() {
    // 没有假服务器：这些请求都不该发出去。
    let client = client("http://127.0.0.1:9");
    let code_challenge = apple_web::code_challenge(&"v".repeat(43));
    assert_eq!(
        client
            .apple_web_challenge("not-a-digest", "app.msime.android")
            .unwrap_err(),
        AccountError::Invalid
    );
    assert_eq!(
        client
            .apple_web_challenge(&code_challenge, "App.Msime")
            .unwrap_err(),
        AccountError::Invalid
    );
    assert_eq!(
        client
            .apple_web_login(&("g".repeat(42) + "h"), &"v".repeat(43))
            .err(),
        Some(AccountError::Invalid)
    );
    assert_eq!(
        client
            .apple_web_login(&("g".repeat(42) + "A"), "short")
            .err(),
        Some(AccountError::Invalid)
    );
}

// ---- 断网与服务不可用、对话和 AI 皮肤的失败细节、带细节的注销 ----

/// 读完请求就挂断、什么也不回：`serve` 收到空响应时就是这样，客户端拿不到响应头。
fn hang_up() -> Vec<u8> {
    Vec::new()
}

/// 内存里的会话存储，开始时有一份未过期的合成登录会话（访问令牌是 `a`）。
#[derive(Clone)]
struct SignedIn(Arc<Mutex<Option<SavedAccountSession>>>);

impl SignedIn {
    fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        Self(Arc::new(Mutex::new(Some(SavedAccountSession {
            tokens: AccountTokens {
                access_token: token('a'),
                refresh_token: token('b'),
                token_type: "Bearer".into(),
                expires_in: 3600,
                user: AccountUser {
                    id: "fixture-user".into(),
                    display_name: "Fixture".into(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    email: None,
                    avatar_url: None,
                },
            },
            expires_at_unix_ms: now + 3_600_000,
            session_id: Some(uuid::Uuid::new_v4()),
        }))))
    }

    fn signed_in(&self) -> bool {
        self.0.lock().unwrap().is_some()
    }
}

impl AccountSessionStorage for SignedIn {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        *self.0.lock().unwrap() = Some(session.clone());
        Ok(())
    }
    fn clear(&self) -> Result<(), AccountError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

#[test]
fn a_request_without_a_response_is_a_network_failure_only_on_the_detailed_paths() {
    let (origin, received) = serve(vec![
        hang_up(),
        hang_up(),
        error_response("503 Service Unavailable", "job_unavailable", &[]),
    ]);
    let client = client(&origin);
    let failure = client.phrases(&token('a')).unwrap_err();
    assert_eq!(failure.code(), "account_network");
    assert!(failure.network_failure);
    assert_eq!(failure.status, None);
    // 经 `error` 取码的已有接口看到的码不变。
    assert_eq!(failure.error, AccountError::Unavailable);
    assert_eq!(
        client.chat_models(&token('a')).unwrap_err(),
        AccountError::Unavailable
    );
    // 服务端回了 503 不是断网。
    let failure = client.chat_models_detailed(&token('a')).unwrap_err();
    assert_eq!(failure.code(), "account_unavailable");
    assert!(!failure.network_failure);
    assert_eq!(failure.status, Some(503));
    assert_eq!(received.iter().count(), 3);
}

#[test]
fn chat_failures_keep_retry_after_and_tell_503_from_a_dropped_connection() {
    let messages = [AccountChatMessage {
        role: "user".into(),
        content: "合成的问题".into(),
    }];
    let (origin, received) = serve(vec![
        error_response(
            "429 Too Many Requests",
            "rate_limit_exceeded",
            &[("Retry-After", "60")],
        ),
        error_response(
            "429 Too Many Requests",
            "rate_limit_exceeded",
            &[("Retry-After", "60")],
        ),
        error_response(
            "503 Service Unavailable",
            "server_busy",
            &[("Retry-After", "1")],
        ),
        error_response("503 Service Unavailable", "upstream_unavailable", &[]),
        hang_up(),
        error_response(
            "429 Too Many Requests",
            "rate_limit_exceeded",
            &[("Retry-After", "30")],
        ),
    ]);
    let client = client(&origin);
    let chat = || {
        client
            .chat_detailed(&messages, "synthetic-model", &token('a'))
            .unwrap_err()
    };
    let limited = chat();
    assert_eq!(
        (limited.code(), limited.retry_after_seconds),
        ("account_rate_limited", Some(60))
    );
    // 已有的 `chat` 照旧只有 AccountError。
    assert_eq!(
        client
            .chat(&messages, "synthetic-model", &token('a'))
            .unwrap_err(),
        AccountError::RateLimited
    );
    let busy = chat();
    assert_eq!(
        (busy.code(), busy.retry_after_seconds),
        ("account_rate_limited", Some(1))
    );
    assert_eq!(chat().code(), "account_unavailable");
    assert_eq!(chat().code(), "account_network");
    let models = client.chat_models_detailed(&token('a')).unwrap_err();
    assert_eq!(
        (models.code(), models.retry_after_seconds),
        ("account_rate_limited", Some(30))
    );
    let sent: Vec<_> = received.iter().collect();
    assert_eq!(sent[0].line(), "POST /v1/chat/completions HTTP/1.1");
    assert_eq!(sent[0].json()["stream"], false);
    assert_eq!(sent[5].line(), "GET /v1/models HTTP/1.1");
}

#[test]
fn only_the_detailed_account_deletion_reports_recent_login_required() {
    let (origin, received) = serve(vec![
        error_response("403 Forbidden", "recent_login_required", &[]),
        error_response("403 Forbidden", "recent_login_required", &[]),
        error_response("403 Forbidden", "recent_login_required", &[]),
        http("204 No Content", &[], b""),
    ]);
    let client = client(&origin);
    assert_eq!(
        client
            .delete_account_detailed(&token('a'))
            .unwrap_err()
            .code(),
        "account_recent_login_required"
    );
    // 已有的 `delete` 不变：仍是 account_unauthorized。
    let old = client.delete_account(&token('a')).unwrap_err();
    assert_eq!(
        (old.clone(), old.code()),
        (AccountError::Forbidden, "account_unauthorized")
    );
    // 会话层：需要最近登录时本地会话留着，注销成功后清掉。
    let storage = SignedIn::new();
    let session = BackendAccountSession::new(client, storage.clone());
    assert_eq!(
        session.delete_account_detailed().unwrap_err().code(),
        "account_recent_login_required"
    );
    assert!(storage.signed_in());
    session.delete_account_detailed().unwrap();
    assert!(!storage.signed_in());
    let sent: Vec<_> = received.iter().collect();
    assert_eq!(sent.len(), 4);
    for request in &sent {
        assert_eq!(request.line(), "DELETE /v1/users/me HTTP/1.1");
        assert_eq!(
            request.header("authorization"),
            Some(format!("Bearer {}", token('a')).as_str())
        );
    }
}

#[test]
fn ai_skin_routes_keep_retry_after_and_the_network_flag() {
    use crate::skin::ai::AiSkinApi;
    let id = "0123456789abcdef".repeat(3);
    let (origin, received) = serve(vec![
        error_response(
            "429 Too Many Requests",
            "rate_limit_exceeded",
            &[("Retry-After", "7200")],
        ),
        error_response("404 Not Found", "skin_job_not_found", &[]),
        hang_up(),
        error_response(
            "503 Service Unavailable",
            "skin_jobs_busy",
            &[("Retry-After", "5")],
        ),
    ]);
    let client = client(&origin);
    let limited = client
        .create_skin_artwork("合成的插画提示词", &token('a'))
        .unwrap_err();
    assert_eq!(
        (limited.code(), limited.retry_after_seconds),
        ("account_rate_limited", Some(7200))
    );
    assert_eq!(
        client
            .get_skin_artwork(&id, &token('a'))
            .unwrap_err()
            .code(),
        "account_not_found"
    );
    assert_eq!(
        client
            .delete_skin_artwork(&id, &token('a'))
            .unwrap_err()
            .code(),
        "account_network"
    );
    let busy = client.ai_models(&token('a')).unwrap_err();
    assert_eq!(
        (busy.code(), busy.retry_after_seconds),
        ("account_unavailable", Some(5))
    );
    let lines: Vec<_> = received
        .iter()
        .map(|request| request.line().to_owned())
        .collect();
    assert_eq!(
        lines,
        [
            "POST /v1/skins/jobs HTTP/1.1".to_owned(),
            format!("GET /v1/skins/jobs/{id} HTTP/1.1"),
            format!("DELETE /v1/skins/jobs/{id} HTTP/1.1"),
            "GET /v1/models HTTP/1.1".to_owned(),
        ]
    );
}

#[test]
fn the_ai_skin_service_keeps_the_detail_only_for_the_detailed_entry_points() {
    use crate::skin::ai::{AiSkinError, AiSkinFailure, BackendAiSkinService};
    use std::sync::atomic::AtomicBool;
    let limited = || {
        error_response(
            "429 Too Many Requests",
            "rate_limit_exceeded",
            &[("Retry-After", "3600")],
        )
    };
    let (origin, received) = serve(vec![limited(), limited(), hang_up()]);
    let service = BackendAiSkinService::new(
        client(&origin),
        Arc::new(BackendAccountSession::new(client(&origin), SignedIn::new())),
    );
    let cancelled = AtomicBool::new(false);
    match service
        .generate_detailed("合成的提示词", &cancelled, |_| {})
        .unwrap_err()
    {
        AiSkinFailure::Account(failure) => assert_eq!(
            (failure.code(), failure.retry_after_seconds),
            ("account_rate_limited", Some(3600))
        ),
        other => panic!("{other:?}"),
    }
    // Tauri 的 `ai_skin_generate` 用的 `generate` 照旧只有 AccountError。
    assert_eq!(
        service
            .generate("合成的提示词", &cancelled, |_| {})
            .unwrap_err(),
        AiSkinError::Account(AccountError::RateLimited)
    );
    match service.create_artwork_job("合成的插画提示词").unwrap_err() {
        AiSkinFailure::Account(failure) => assert_eq!(failure.code(), "account_network"),
        other => panic!("{other:?}"),
    }
    assert_eq!(received.iter().count(), 3);
}

#[test]
fn feedback_voice_and_export_report_a_dropped_connection_as_account_network() {
    let feedback = prepare_feedback(&AccountFeedbackInput {
        kind: "bug".into(),
        text: "合成反馈".into(),
        platform: "android".into(),
        app_version: "0.9.0".into(),
        edition: "full".into(),
        ..Default::default()
    })
    .unwrap();
    let contribution = prepare_voice_contribution(&AccountVoiceContributionInput {
        language: "zh-CN".into(),
        provider: "system".into(),
        transcript: "合成识别文本".into(),
        app_version: "0.9.0".into(),
        audio: synthetic_wav(1, 16_000, 16, 32_000),
    })
    .unwrap();
    // 三个都自己 send()，不经 request_detailed：服务端读完请求就挂断，一个字节也不回。
    let (origin, received) = serve(vec![hang_up(), hang_up(), hang_up()]);
    let client = client(&origin);
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("export.zip");
    std::fs::write(&destination, b"older export").unwrap();
    let failures = [
        client.feedback(&feedback, &token('a')).unwrap_err(),
        client
            .voice_contribution(&contribution, &token('a'))
            .unwrap_err(),
        client.data_export(&destination, &token('a')).unwrap_err(),
    ];
    for failure in &failures {
        assert_eq!(failure.code(), "account_network", "{failure:?}");
        assert!(failure.network_failure);
        assert_eq!(failure.status, None);
        // 经 `error` 取码的已有接口看到的仍是 Unavailable。
        assert_eq!(failure.error, AccountError::Unavailable);
    }
    // 导出没拿到响应：目标位置上原有的文件不动，也不留临时文件。
    assert_eq!(std::fs::read(&destination).unwrap(), b"older export");
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    let lines: Vec<String> = received
        .iter()
        .map(|request| request.line().to_owned())
        .collect();
    assert_eq!(
        lines,
        [
            "POST /v1/feedback HTTP/1.1",
            "POST /v1/voice/contributions HTTP/1.1",
            "GET /v1/users/me/data/export HTTP/1.1",
        ]
    );
}

#[test]
fn send_errors_that_are_not_a_lost_connection_are_not_account_network() {
    // 建不出请求：根本没有发出去，是请求不合法。
    let builder = reqwest::blocking::Client::new()
        .get("not an absolute url")
        .send()
        .unwrap_err();
    assert!(builder.is_builder(), "{builder:?}");
    let failure = AccountFailure::from_send_error(&builder);
    assert_eq!(failure.code(), "account_invalid");
    assert!(!failure.network_failure);

    // 重定向被策略拒绝：服务端已经回了响应，是不可用而不是断网。账号客户端不跟随重定向，这里用一个一次也不许跟随的客户端造出这种错误。
    let (origin, received) = serve(vec![
        http("302 Found", &[("Location", "/elsewhere")], b""),
        hang_up(),
    ]);
    let redirect = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(0))
        .build()
        .unwrap()
        .get(format!("{origin}/start"))
        .send()
        .unwrap_err();
    assert!(redirect.is_redirect(), "{redirect:?}");
    let failure = AccountFailure::from_send_error(&redirect);
    assert_eq!(failure.code(), "account_unavailable");
    assert!(!failure.network_failure);

    // 真的没拿到响应仍是断网。
    let dropped = reqwest::blocking::Client::new()
        .get(format!("{origin}/again"))
        .send()
        .unwrap_err();
    assert_eq!(
        AccountFailure::from_send_error(&dropped).code(),
        "account_network"
    );
    assert_eq!(received.iter().count(), 2);
}
