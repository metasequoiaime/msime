//! 回环 HTTP 验证真实请求字节、固定路径、状态保留和传输边界。

use super::*;
use crate::account::transport_tests::{http, json_response, read_request, Received};
use serde_json::json;
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};

const DEVICE: &str = "0123456789abcdef0123456789abcdef";
const TOKEN: &str = "synthetic-learning-token";

fn serve(responses: Vec<Vec<u8>>) -> (BackendAccountClient, Receiver<Received>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (sent, received) = mpsc::channel();
    std::thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let _ = sent.send(read_request(&mut stream));
            // 超限响应被客户端中断时，服务端写入可以失败。
            let _ = std::io::Write::write_all(&mut stream, &response);
        }
    });
    (BackendAccountClient::loopback(&origin).unwrap(), received)
}

#[test]
fn control_routes_send_json_and_bearer_without_rewriting() {
    let routes = [
        ("GET", format!("{LEARNING_PATH}?device_id={DEVICE}"), None),
        (
            "POST",
            format!("{LEARNING_PATH}/key"),
            Some("{\"key\":\"fixture\"}".to_owned()),
        ),
        (
            "POST",
            format!("{LEARNING_PATH}/rekey"),
            Some("{}".to_owned()),
        ),
        (
            "PUT",
            format!("{LEARNING_PATH}/devices/{DEVICE}"),
            Some("{}".to_owned()),
        ),
        (
            "PUT",
            format!("{LEARNING_PATH}/wraps/{DEVICE}"),
            Some("{}".to_owned()),
        ),
        ("DELETE", format!("{LEARNING_PATH}/devices/{DEVICE}"), None),
    ];
    let (client, received) = serve(
        routes
            .iter()
            .map(|_| json_response("200 OK", json!({})))
            .collect(),
    );
    for (method, path, body) in routes {
        let result = client
            .learning_e2e(
                &HttpRequest {
                    method: method.into(),
                    path: path.clone(),
                    body: body.clone(),
                },
                TOKEN,
            )
            .unwrap();
        assert_eq!(result.status, 200);
        let request = received.recv().unwrap();
        assert_eq!(request.line(), format!("{method} {path} HTTP/1.1"));
        assert_eq!(
            request.header("authorization"),
            Some("Bearer synthetic-learning-token")
        );
        assert_eq!(request.body, body.unwrap_or_default().as_bytes());
        if !request.body.is_empty() {
            assert_eq!(request.header("content-type"), Some("application/json"));
        }
    }
}

#[test]
fn invalid_control_paths_and_oversized_bodies_fail_before_network() {
    let client = BackendAccountClient::loopback("http://127.0.0.1:1").unwrap();
    for (method, path) in [
        (
            "GET",
            "https://elsewhere.invalid/v1/users/me/learning".into(),
        ),
        ("GET", format!("//elsewhere.invalid{LEARNING_PATH}")),
        ("GET", format!("{LEARNING_PATH}?device_id={DEVICE}&other=1")),
        (
            "GET",
            format!("{LEARNING_PATH}?device_id={DEVICE}#fragment"),
        ),
        (
            "GET",
            format!("{LEARNING_PATH}?device_id={}", DEVICE.to_uppercase()),
        ),
        ("PUT", format!("{LEARNING_PATH}/devices/../key")),
        ("PUT", format!("{LEARNING_PATH}/devices/%2e%2e%2fkey")),
        ("DELETE", format!("{LEARNING_PATH}/wraps/{DEVICE}")),
        ("PATCH", format!("{LEARNING_PATH}/key")),
        ("POST", "/v1/users/me/clipboard/keys".into()),
    ] {
        let request = HttpRequest {
            method: method.into(),
            path,
            body: None,
        };
        assert_eq!(
            client.learning_e2e(&request, TOKEN),
            Err(AccountError::Invalid)
        );
    }
    let request = HttpRequest {
        method: "POST".into(),
        path: format!("{LEARNING_PATH}/key"),
        body: Some("x".repeat(MAX_METADATA_BYTES + 1)),
    };
    assert_eq!(
        client.learning_e2e(&request, TOKEN),
        Err(AccountError::Invalid)
    );
}

#[test]
fn blobs_are_transmitted_as_raw_bytes_with_one_content_type() {
    let bytes: Vec<u8> = (0..=255).collect();
    let (client, received) = serve(vec![
        http("204 No Content", &[], &[]),
        http(
            "200 OK",
            &[("Content-Type", "application/octet-stream")],
            &bytes,
        ),
        json_response("200 OK", json!({"blobs": []})),
        http("204 No Content", &[], &[]),
    ]);
    assert_eq!(
        client
            .learning_blob(
                &BlobRequest::Upload {
                    device_id: DEVICE,
                    generation: 7,
                    envelope: &bytes
                },
                TOKEN
            )
            .unwrap()
            .status,
        204
    );
    let request = received.recv().unwrap();
    assert_eq!(
        request.line(),
        format!("PUT {LEARNING_PATH}/blobs/{DEVICE}?revision=7 HTTP/1.1")
    );
    assert_eq!(
        request.header("authorization"),
        Some("Bearer synthetic-learning-token")
    );
    assert_eq!(
        request.header("content-type"),
        Some("application/octet-stream")
    );
    assert_eq!(
        request
            .head
            .lines()
            .filter(|line| line.to_ascii_lowercase().starts_with("content-type:"))
            .count(),
        1
    );
    assert_eq!(request.body, bytes);
    assert_eq!(
        client
            .learning_blob(&BlobRequest::Download { device_id: DEVICE }, TOKEN)
            .unwrap()
            .body,
        bytes
    );
    assert_eq!(
        received.recv().unwrap().line(),
        format!("GET {LEARNING_PATH}/blobs/{DEVICE} HTTP/1.1")
    );
    client
        .learning_blob(&BlobRequest::Metadata { device_id: DEVICE }, TOKEN)
        .unwrap();
    assert_eq!(
        received.recv().unwrap().line(),
        format!("GET {LEARNING_PATH}?device_id={DEVICE} HTTP/1.1")
    );
    client
        .learning_blob(&BlobRequest::Delete { device_id: DEVICE }, TOKEN)
        .unwrap();
    assert_eq!(
        received.recv().unwrap().line(),
        format!("DELETE {LEARNING_PATH}/blobs/{DEVICE} HTTP/1.1")
    );
}

#[test]
fn http_failures_and_retry_after_are_kept_for_the_sync_round() {
    for (status, code) in [
        (401, "unauthorized"),
        (409, "learning_revision_conflict"),
        (413, "learning_quota"),
        (429, "rate_limit_exceeded"),
        (503, "unavailable"),
    ] {
        let body = json!({"error": {"code": code}}).to_string();
        let response = http(
            &format!("{status} Synthetic"),
            &[("Retry-After", "37")],
            body.as_bytes(),
        );
        let (client, _) = serve(vec![response.clone(), response]);
        let control = client
            .learning_e2e(
                &HttpRequest {
                    method: "GET".into(),
                    path: format!("{LEARNING_PATH}?device_id={DEVICE}"),
                    body: None,
                },
                TOKEN,
            )
            .unwrap();
        assert_eq!((control.status, control.body), (status, body.clone()));
        let blob = client
            .learning_blob(&BlobRequest::Download { device_id: DEVICE }, TOKEN)
            .unwrap();
        assert_eq!(
            (blob.status, blob.body, blob.retry_after_seconds),
            (status, body.into_bytes(), Some(37))
        );
    }
}

#[test]
fn invalid_blob_parameters_and_limits_fail_before_network() {
    let client = BackendAccountClient::loopback("http://127.0.0.1:1").unwrap();
    let bytes = vec![0; HEADER_BYTES + 16];
    let oversized = vec![0; MAX_ENVELOPE_BYTES + 1];
    for request in [
        BlobRequest::Metadata {
            device_id: "../key",
        },
        BlobRequest::Download { device_id: "wrong" },
        BlobRequest::Delete {
            device_id: "0123456789abcdef0123456789abcdef?x=1",
        },
        BlobRequest::Upload {
            device_id: DEVICE,
            generation: 0,
            envelope: &bytes,
        },
        BlobRequest::Upload {
            device_id: DEVICE,
            generation: MAX_GENERATION + 1,
            envelope: &bytes,
        },
        BlobRequest::Upload {
            device_id: DEVICE,
            generation: 1,
            envelope: &bytes[..HEADER_BYTES],
        },
        BlobRequest::Upload {
            device_id: DEVICE,
            generation: 1,
            envelope: &oversized,
        },
    ] {
        assert!(matches!(
            client.learning_blob(&request, TOKEN),
            Err(AccountError::Invalid)
        ));
    }
}

#[test]
fn oversized_success_responses_are_rejected() {
    for (request, limit) in [
        (
            BlobRequest::Metadata { device_id: DEVICE },
            MAX_METADATA_BYTES,
        ),
        (
            BlobRequest::Download { device_id: DEVICE },
            MAX_ENVELOPE_BYTES,
        ),
    ] {
        let (client, _) = serve(vec![http("200 OK", &[], &vec![0; limit + 1])]);
        assert!(matches!(
            client.learning_blob(&request, TOKEN),
            Err(AccountError::Unavailable)
        ));
    }
    let (client, _) = serve(vec![http("200 OK", &[], &vec![0; MAX_METADATA_BYTES + 1])]);
    assert_eq!(
        client.learning_e2e(
            &HttpRequest {
                method: "GET".into(),
                path: format!("{LEARNING_PATH}?device_id={DEVICE}"),
                body: None
            },
            TOKEN
        ),
        Err(AccountError::Unavailable)
    );
}
