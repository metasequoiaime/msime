//! 注销账号以后删本机匿名账号的语音样本（#8201 的决定 12 后续 2），经 C 入口走已有的 `delete`：本线程的 [`TEST_ORIGIN`] 把目录模式的客户端连到本机回环上按顺序回答的假服务端，核对两个请求各带哪个账号的令牌、删语音样本失败时注销照样成功，以及 `voice_delete` 不登录也能用。

use super::*;
use msime_client_services::account::{
    AccountSessionStorage, AccountTokens, AccountUser, AnonymousSessionStorage,
    SavedAccountSession, ACCOUNT_SESSION_FILE,
};
use std::ffi::CStr;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

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

/// 一份未过期的合成会话，访问令牌是 64 个 `access`。
fn saved(user: &str, access: char) -> SavedAccountSession {
    SavedAccountSession {
        tokens: AccountTokens {
            access_token: access.to_string().repeat(64),
            refresh_token: "b".repeat(64),
            token_type: "Bearer".into(),
            expires_in: 3600,
            user: AccountUser {
                id: user.into(),
                display_name: "合成用户".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                email: None,
                avatar_url: None,
            },
        },
        expires_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 3_600_000,
        session_id: Some(msime_client_core::uuid::Uuid::new_v4()),
    }
}

/// 收到的一个请求：请求行、`Authorization` 和正文。
struct Seen {
    line: String,
    authorization: String,
    body: String,
}

/// 依次回答 `responses` 里的每一个，每个连接一个，收到的请求按顺序交回；读满 `Content-Length` 的正文才回。
fn serve(responses: Vec<&'static str>) -> (String, mpsc::Receiver<Seen>) {
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
            let header = |wanted: &str| {
                text.lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case(wanted)
                            .then(|| value.trim().to_owned())
                    })
                    .unwrap_or_default()
            };
            let length: usize = header("content-length").parse().unwrap_or(0);
            while request.len() < head + length {
                let read = stream.read(&mut buffer).unwrap();
                assert_ne!(read, 0);
                request.extend_from_slice(&buffer[..read]);
            }
            sender
                .send(Seen {
                    line: text.lines().next().unwrap().to_owned(),
                    authorization: header("authorization"),
                    body: String::from_utf8(request[head..].to_vec()).unwrap(),
                })
                .unwrap();
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (origin, seen)
}

const NO_CONTENT: &str = "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n";
const RATE_LIMITED: &str = "HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\nRetry-After: 3600\r\nContent-Length: 65\r\nConnection: close\r\n\r\n{\"error\":{\"code\":\"rate_limit_exceeded\",\"message\":\"rate limited\"}}";

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

/// 一个登录着、本机也有匿名会话的账号目录。
fn signed_in_directory(root: &Path, name: &str) -> PathBuf {
    let directory = root.join(name);
    std::fs::create_dir(&directory).unwrap();
    FileAccountSessionStorage::new(&directory, AccountSessionFileLayout::Native)
        .save(&saved("synthetic-user", 'a'))
        .unwrap();
    AnonymousSessionStorage::new(&directory)
        .save(&saved("synthetic-anonymous", 'c'))
        .unwrap();
    directory
}

#[test]
fn deleting_the_account_then_deletes_the_anonymous_voice_samples() {
    assert_eq!(RATE_LIMITED.split("\r\n\r\n").nth(1).unwrap().len(), 65);
    let (origin, seen) = serve(vec![NO_CONTENT, NO_CONTENT, NO_CONTENT, RATE_LIMITED]);
    let _loopback = LoopbackOrigin::set(&origin);
    let root = tempfile::tempdir().unwrap();

    let deleted = signed_in_directory(root.path(), "deleted");
    assert_eq!(
        call(json!({ "directory": deleted, "action": { "operation": "delete" } })),
        json!({ "ok": true, "value": { "voice_samples": { "ok": true } } })
    );
    assert!(!deleted.join(ACCOUNT_SESSION_FILE).exists());

    // 删语音样本被限流：注销照样成功、本地会话照样清掉，失败和秒数只在 voice_samples 里。
    let limited = signed_in_directory(root.path(), "limited");
    assert_eq!(
        call(json!({ "directory": limited, "action": { "operation": "delete" } })),
        json!({ "ok": true, "value": { "voice_samples": { "ok": false, "error": "account_rate_limited", "retry_after_seconds": 3600 } } })
    );
    assert!(!limited.join(ACCOUNT_SESSION_FILE).exists());

    let seen: Vec<Seen> = seen.try_iter().collect();
    let account = format!("Bearer {}", "a".repeat(64));
    let anonymous = format!("Bearer {}", "c".repeat(64));
    let requests: Vec<(&str, &str)> = seen
        .iter()
        .map(|request| (request.line.as_str(), request.authorization.as_str()))
        .collect();
    let expected = [
        ("DELETE /v1/users/me HTTP/1.1", account.as_str()),
        ("DELETE /v1/users/me/data HTTP/1.1", anonymous.as_str()),
    ];
    assert_eq!(requests, [expected, expected].concat());
    for request in seen.iter().filter(|request| request.line.contains("/data")) {
        assert_eq!(request.body, r#"{"sections":["voice"]}"#);
    }
}

#[test]
fn voice_samples_are_deleted_without_signing_in() {
    let (origin, seen) = serve(vec![NO_CONTENT]);
    let _loopback = LoopbackOrigin::set(&origin);
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("anonymous");
    std::fs::create_dir(&directory).unwrap();
    AnonymousSessionStorage::new(&directory)
        .save(&saved("synthetic-anonymous", 'c'))
        .unwrap();
    assert_eq!(
        call(json!({ "directory": directory, "action": { "operation": "voice_delete" } })),
        json!({ "ok": true, "value": null })
    );
    // 调用已经返回：请求要么已经到了，要么根本没发。
    let request = seen.try_recv().expect("voice_delete sent no request");
    assert_eq!(request.line, "DELETE /v1/users/me/data HTTP/1.1");
    assert_eq!(request.authorization, format!("Bearer {}", "c".repeat(64)));

    // 目录还不存在：没有匿名会话也没有身份，没有样本可删，不联网。
    assert_eq!(
        call(
            json!({ "directory": root.path().join("missing"), "action": { "operation": "voice_delete" } })
        ),
        json!({ "ok": true, "value": null })
    );
    assert!(seen.try_recv().is_err());
}
