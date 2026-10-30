//! 鸿蒙设置应用的华为账号登录经 C 入口走的两步：`huawei_challenge` 申请带 nonce 的 challenge，`huawei_login` 交回 Account Kit 给的 ID token。本线程的 [`TEST_ORIGIN`] 把目录模式的客户端连到本机回环上按顺序回答的假服务端，核对线上的请求、登录后会话写进目录，以及 nonce 只交给宿主。

use super::*;
use std::ffi::CStr;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

fn call(request: Value) -> Value {
    let bytes = request.to_string();
    // SAFETY: 缓冲区在调用期间有效，返回的字符串读完释放。
    unsafe {
        let raw = msime_client_account(bytes.as_ptr(), bytes.len());
        let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
        msime_client_string_free(raw);
        serde_json::from_str(&text).unwrap()
    }
}

/// 收到的一个请求：请求行和 JSON 正文。
struct Seen {
    line: String,
    body: Value,
}

fn json_response(status: &str, body: Value) -> String {
    let body = body.to_string();
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// 依次回答 `responses` 里的每一个，每个连接一个，收到的请求按顺序交回；读满 `Content-Length` 的正文才回。
fn serve(responses: Vec<String>) -> (String, mpsc::Receiver<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (sender, seen) = mpsc::channel();
    thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            let head = loop {
                if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                    break end + 4;
                }
                let read = stream.read(&mut buffer).unwrap();
                assert_ne!(read, 0);
                request.extend_from_slice(&buffer[..read]);
            };
            let text = String::from_utf8(request[..head].to_vec()).unwrap();
            let length: usize = text
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().unwrap())
                })
                .unwrap_or(0);
            while request.len() < head + length {
                let read = stream.read(&mut buffer).unwrap();
                assert_ne!(read, 0);
                request.extend_from_slice(&buffer[..read]);
            }
            sender
                .send(Seen {
                    line: text.lines().next().unwrap().to_owned(),
                    body: serde_json::from_slice(&request[head..]).unwrap(),
                })
                .unwrap();
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (origin, seen)
}

/// 在本线程上把目录模式的客户端连到 `origin`，离开作用域时恢复。
struct LoopbackOrigin;

impl LoopbackOrigin {
    fn set(origin: &str) -> Self {
        TEST_ORIGIN.with_borrow_mut(|slot| *slot = Some(origin.to_owned()));
        Self
    }
}

impl Drop for LoopbackOrigin {
    fn drop(&mut self) {
        TEST_ORIGIN.with_borrow_mut(|slot| *slot = None);
    }
}

const ID_TOKEN: &str = "eyJhbGciOiJQUzI1NiJ9.eyJzdWIiOiJ1bmlvbi1pZCJ9.c2lnbmF0dXJl";

#[test]
fn a_huawei_sign_in_asks_for_a_nonce_then_trades_the_id_token_for_a_session() {
    let challenge_id = "c".repeat(64);
    let nonce = "d".repeat(64);
    let (origin, seen) = serve(vec![
        json_response(
            "201 Created",
            json!({ "challenge_id": challenge_id, "expires_in": 300, "nonce": nonce }),
        ),
        json_response(
            "200 OK",
            json!({
                "access_token": "a".repeat(64),
                "refresh_token": "b".repeat(64),
                "token_type": "Bearer",
                "expires_in": 900,
                "user": { "id": "synthetic-huawei-user", "display_name": "合成用户", "created_at": "2026-01-01T00:00:00Z" }
            }),
        ),
    ]);
    let _loopback = LoopbackOrigin::set(&origin);
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("account");
    std::fs::create_dir(&directory).unwrap();

    let challenge =
        call(json!({ "directory": directory, "action": { "operation": "huawei_challenge" } }));
    assert_eq!(
        challenge,
        json!({ "ok": true, "value": { "challengeId": challenge_id, "expiresIn": 300, "nonce": nonce } })
    );
    let signed_in = call(json!({
        "directory": directory,
        "action": { "operation": "huawei_login", "challenge_id": challenge_id, "id_token": ID_TOKEN },
    }));
    assert_eq!(signed_in["ok"], true, "{signed_in}");
    assert_eq!(signed_in["value"]["user"]["id"], "synthetic-huawei-user");
    // 令牌不交给宿主，会话写进目录，之后的 status 只读本地文件。
    assert!(!signed_in.to_string().contains(&"a".repeat(64)));
    assert_eq!(
        call(json!({ "directory": directory, "action": { "operation": "status" } }))["value"]
            ["user"]["id"],
        "synthetic-huawei-user"
    );

    let seen: Vec<Seen> = seen.try_iter().collect();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].line, "POST /v1/auth/challenges HTTP/1.1");
    assert_eq!(
        seen[0].body,
        json!({ "provider": "huawei", "target": "", "purpose": "login" })
    );
    assert_eq!(seen[1].line, "POST /v1/auth/login HTTP/1.1");
    assert_eq!(
        seen[1].body,
        json!({ "challenge_id": challenge_id, "credential": ID_TOKEN })
    );
}

#[test]
fn a_huawei_challenge_without_a_nonce_is_not_handed_to_the_host() {
    let (origin, _seen) = serve(vec![json_response(
        "201 Created",
        json!({ "challenge_id": "c".repeat(64), "expires_in": 300 }),
    )]);
    let _loopback = LoopbackOrigin::set(&origin);
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        call(
            json!({ "directory": root.path().join("account"), "action": { "operation": "huawei_challenge" } })
        ),
        json!({ "ok": false, "error": "account_unavailable" })
    );
}
