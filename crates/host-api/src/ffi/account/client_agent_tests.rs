//! 目录模式的 `client_agent`（#8201 的决定 1）：带 `directory` 的请求可以自报 User-Agent，校验与注入存储的 `client_agent` 相同；不带时与以前相同。这里核对请求的形状、会话对象怎么按 `client_agent` 分开，以及登录、绑定、匿名账号的注册和退回匿名账号的请求在线上真的带着它（本线程的 [`TEST_ORIGIN`] 把目录模式的客户端连到本机假服务端）；`with_user_agent` 本身由 client-services 的测试核对。

use super::super::chat_stream::msime_client_account_chat_stream;
use super::super::session_store::tests::{leaked_store, registered, unregister};
use super::*;
use msime_client_services::account::{
    AccountSessionStorage, AccountTokens, AccountUser, AnonymousSessionStorage, SavedAccountSession,
};
use std::ffi::CStr;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

const AGENT: &str = "msime-windows/1.4.0 (Surface Pro; Windows 11; edition=full)";
const OTHER_AGENT: &str = "msime-linux/1.4.0";

fn read(raw: *mut c_char) -> Value {
    // SAFETY: 导出函数返回的字符串，读完释放。
    unsafe {
        let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
        msime_client_string_free(raw);
        serde_json::from_str(&text).unwrap()
    }
}

fn call(request: Value) -> Value {
    let bytes = request.to_string();
    // SAFETY: 缓冲区在调用期间有效。
    read(unsafe { msime_client_account(bytes.as_ptr(), bytes.len()) })
}

fn chat_stream(request: Value) -> Value {
    let bytes = request.to_string();
    // SAFETY: 缓冲区在调用期间有效，没有增量回调。
    read(unsafe {
        msime_client_account_chat_stream(bytes.as_ptr(), bytes.len(), None, std::ptr::null_mut())
    })
}

/// 一份未过期的合成会话。
fn saved(user: &str) -> SavedAccountSession {
    SavedAccountSession {
        tokens: AccountTokens {
            access_token: "a".repeat(64),
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

#[test]
fn a_directory_request_may_name_its_client_agent() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("account");
    std::fs::create_dir(&directory).unwrap();
    FileAccountSessionStorage::new(&directory, AccountSessionFileLayout::Native)
        .save(&saved("synthetic-user"))
        .unwrap();
    // 同一份会话文件，带不带 client_agent 读到的都是它。
    for agent in [Some(AGENT), None] {
        let mut request = json!({ "directory": directory, "action": { "operation": "status" } });
        if let Some(agent) = agent {
            request["client_agent"] = json!(agent);
        }
        assert_eq!(call(request)["value"]["user"]["id"], "synthetic-user");
    }
    // 匿名账号已经注册过：带 client_agent 的 ensure_anonymous 只读一次会话文件，不联网。
    AnonymousSessionStorage::new(&directory)
        .save(&saved("synthetic-anonymous"))
        .unwrap();
    assert_eq!(
        call(json!({
            "directory": directory,
            "client_agent": AGENT,
            "action": { "operation": "ensure_anonymous" },
        })),
        json!({ "ok": true, "value": true })
    );
    // 新操作也收它：没有登录会话时与不带时一样是 account_unauthorized。
    let empty = root.path().join("empty");
    assert_eq!(
        call(json!({
            "directory": empty,
            "client_agent": AGENT,
            "action": { "operation": "account_delete" },
        })),
        json!({ "ok": false, "error": "account_unauthorized" })
    );
    // 流式对话的会话字段与 msime_client_account 相同。
    assert_eq!(
        chat_stream(json!({
            "directory": empty,
            "client_agent": AGENT,
            "request_id": "client-agent-test",
            "messages": [{ "role": "user", "content": "合成的问题" }],
            "model": "synthetic-model",
        })),
        json!({ "ok": false, "error": "account_unauthorized" })
    );
}

#[test]
fn each_client_agent_has_its_own_session_object_for_a_directory() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path();
    let default = session_for(directory, None).unwrap();
    let named = session_with_agent(directory, None, Some(AGENT)).unwrap();
    let other = session_with_agent(directory, None, Some(OTHER_AGENT)).unwrap();
    assert!(!Arc::ptr_eq(&default, &named));
    assert!(!Arc::ptr_eq(&named, &other));
    // 同一目录、同一个 client_agent 是同一个对象，google_login 和 google_cancel 落在它上面。
    assert!(Arc::ptr_eq(
        &named,
        &session_with_agent(directory, None, Some(AGENT)).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &default,
        &session_with_agent(directory, None, None).unwrap()
    ));
    let anonymous = anonymous_session_with_agent(directory, Some(AGENT)).unwrap();
    assert!(!Arc::ptr_eq(
        &anonymous,
        &anonymous_session_for(directory).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &anonymous,
        &anonymous_session_with_agent(directory, Some(AGENT)).unwrap()
    ));
}

#[test]
fn a_client_agent_must_have_the_shape_the_server_reads_and_needs_a_directory() {
    let root = tempfile::tempdir().unwrap();
    for agent in [
        "MSIME/Android",
        "msime-web/1.0",
        "msime-windows/",
        "msime-windows/1.0 (nested (parens))",
        "msime-windows/1.0 (\u{4e2d})",
        "msime-windows/1.0\t(tab)",
    ]
    .into_iter()
    .map(str::to_owned)
    .chain([format!("msime-windows/1.0 ({})", "x".repeat(256))])
    {
        assert_eq!(
            call(json!({
                "directory": root.path(),
                "client_agent": agent,
                "action": { "operation": "status" },
            })),
            json!({ "ok": false, "error": "account_invalid" }),
            "{agent}"
        );
        assert_eq!(
            chat_stream(json!({
                "directory": root.path(),
                "client_agent": agent,
                "request_id": "client-agent-invalid",
                "messages": [{ "role": "user", "content": "合成的问题" }],
                "model": "synthetic-model",
            }))["error"],
            "account_invalid",
            "{agent}"
        );
    }
    // 注入存储的 User-Agent 是注册时给的那个，请求里不能再给。
    let store = leaked_store();
    let id = registered(store, 1, 0);
    assert_eq!(
        call(json!({
            "session_store": id,
            "client_agent": AGENT,
            "action": { "operation": "status" },
        })),
        json!({ "ok": false, "error": "account_invalid" })
    );
    assert_eq!(
        chat_stream(json!({
            "session_store": id,
            "client_agent": AGENT,
            "request_id": "client-agent-store",
            "messages": [{ "role": "user", "content": "合成的问题" }],
            "model": "synthetic-model",
        }))["error"],
        "account_invalid"
    );
    // 不带时与以前一样。
    assert_eq!(
        call(json!({ "session_store": id, "action": { "operation": "status" } })),
        json!({ "ok": true, "value": { "user": null } })
    );
    unregister(id);
}

/// 本机回环上的假服务端：每个请求记下路径和 User-Agent，一律回 503 并关掉连接。只读请求头和 `Content-Length` 那么长的请求体，读满才回，免得关连接时还有没读的数据、对端看到的是连接被重置。
struct FakeServer {
    origin: String,
    seen: mpsc::Receiver<(String, String)>,
    thread: thread::JoinHandle<()>,
}

impl FakeServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (sender, seen) = mpsc::channel();
        let thread = thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut request = Vec::new();
                let mut buffer = [0_u8; 4096];
                let head = loop {
                    if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                        break end + 4;
                    }
                    let read = stream.read(&mut buffer).unwrap();
                    if read == 0 {
                        // 测试连上来什么都不发就关掉：结束。
                        return;
                    }
                    request.extend_from_slice(&buffer[..read]);
                };
                let text = String::from_utf8(request[..head].to_vec()).unwrap();
                let header = |wanted: &str| {
                    text.lines().find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case(wanted)
                            .then(|| value.trim().to_owned())
                    })
                };
                let length: usize =
                    header("content-length").map_or(0, |value| value.parse().unwrap());
                while request.len() < head + length {
                    let read = stream.read(&mut buffer).unwrap();
                    assert_ne!(read, 0);
                    request.extend_from_slice(&buffer[..read]);
                }
                let path = text.split(' ').nth(1).unwrap().to_owned();
                sender
                    .send((path, header("user-agent").unwrap_or_default()))
                    .unwrap();
                stream
                    .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .unwrap();
            }
        });
        Self {
            origin,
            seen,
            thread,
        }
    }

    /// 停下服务端，交出它收到的每个请求的（路径，User-Agent）。
    fn finish(self) -> Vec<(String, String)> {
        drop(TcpStream::connect(self.origin.trim_start_matches("http://")).unwrap());
        self.thread.join().unwrap();
        self.seen.try_iter().collect()
    }
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

#[test]
fn directory_requests_go_out_with_the_host_agent() {
    let server = FakeServer::start();
    let loopback = LoopbackOrigin::set(&server.origin);
    let root = tempfile::tempdir().unwrap();
    let failed = |request: Value| {
        let response = call(request.clone());
        assert_eq!(response["ok"], false, "{request}");
    };

    // 登录：申请验证码和交回验证码。
    let signing_in = root.path().join("sign-in");
    failed(json!({
        "directory": signing_in,
        "client_agent": AGENT,
        "action": { "operation": "request_code", "provider": "email", "target": "person@example.test" },
    }));
    failed(json!({
        "directory": signing_in,
        "client_agent": AGENT,
        "action": { "operation": "login", "challenge_id": "c".repeat(64), "code": "123456" },
    }));

    // 绑定：已登录的目录完成 purpose:"link"。
    let linking = root.path().join("link");
    std::fs::create_dir(&linking).unwrap();
    FileAccountSessionStorage::new(&linking, AccountSessionFileLayout::Native)
        .save(&saved("synthetic-user"))
        .unwrap();
    failed(json!({
        "directory": linking,
        "client_agent": AGENT,
        "action": { "operation": "login", "challenge_id": "c".repeat(64), "code": "123456", "purpose": "link" },
    }));

    // 匿名账号的注册。
    failed(json!({
        "directory": root.path().join("anonymous"),
        "client_agent": AGENT,
        "action": { "operation": "ensure_anonymous" },
    }));

    // 没有登录、退回匿名账号的新操作。
    let fallback = root.path().join("fallback");
    std::fs::create_dir(&fallback).unwrap();
    AnonymousSessionStorage::new(&fallback)
        .save(&saved("synthetic-anonymous"))
        .unwrap();
    failed(json!({
        "directory": fallback,
        "client_agent": AGENT,
        "action": { "operation": "chat_models" },
    }));

    drop(loopback);
    let seen = server.finish();
    let paths: Vec<&str> = seen.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "/v1/auth/challenges",
            "/v1/auth/login",
            "/v1/auth/login",
            // 匿名账号的注册也从申请 challenge 开始。
            "/v1/auth/challenges",
            "/v1/models",
        ]
    );
    for (path, agent) in &seen {
        assert_eq!(agent, AGENT, "{path}");
    }
}

fn ensure_anonymous(request: &[u8]) -> Value {
    // SAFETY: 缓冲区在调用期间有效。
    read(unsafe {
        msime_client_ensure_anonymous_account_with_agent(request.as_ptr(), request.len())
    })
}

#[test]
fn the_anonymous_registration_entry_takes_the_host_agent() {
    let server = FakeServer::start();
    let loopback = LoopbackOrigin::set(&server.origin);
    let root = tempfile::tempdir().unwrap();
    // Windows Server 注册匿名账号的入口带上 client_agent：注册请求以它作 User-Agent。
    let registering = root.path().join("registering");
    let response = ensure_anonymous(
        json!({ "directory": registering, "client_agent": AGENT })
            .to_string()
            .as_bytes(),
    );
    // 假服务端一律回 503。
    assert_eq!(
        response,
        json!({ "ok": false, "error": "account_unavailable" })
    );
    // 身份已经生成并留着，下次启动再试时是同一个账号。
    assert!(registering
        .join(msime_client_services::account::ANONYMOUS_ACCOUNT_FILE)
        .is_file());

    // 已经注册过：带不带 client_agent 都只读一次会话文件，不联网（回环一直连着假服务端，联网了就会记下第二个请求）。
    let registered = root.path().join("registered");
    std::fs::create_dir(&registered).unwrap();
    AnonymousSessionStorage::new(&registered)
        .save(&saved("synthetic-anonymous"))
        .unwrap();
    for request in [
        json!({ "directory": registered, "client_agent": AGENT }),
        json!({ "directory": registered }),
    ] {
        assert_eq!(
            ensure_anonymous(request.to_string().as_bytes()),
            json!({ "ok": true, "value": true }),
            "{request}"
        );
    }

    // 校验与 msime_client_account 的 client_agent 相同，请求不合法时不碰目录。
    let untouched = root.path().join("untouched");
    for request in [
        json!({ "directory": untouched, "client_agent": "MSIME/Android" }),
        json!({ "directory": untouched, "client_agent": "msime-web/1.0" }),
        json!({ "directory": "relative/account", "client_agent": AGENT }),
        json!({ "directory": untouched, "client_agent": AGENT, "extra": true }),
        json!({ "client_agent": AGENT }),
    ] {
        assert_eq!(
            ensure_anonymous(request.to_string().as_bytes()),
            json!({ "ok": false, "error": "account_invalid" }),
            "{request}"
        );
    }
    assert_eq!(
        ensure_anonymous(untouched.to_string_lossy().as_bytes()),
        json!({ "ok": false, "error": "account_invalid" })
    );
    assert_eq!(
        // SAFETY: 空指针被拒绝，不读。
        read(unsafe { msime_client_ensure_anonymous_account_with_agent(std::ptr::null(), 2) }),
        json!({ "ok": false, "error": "account_invalid" })
    );
    assert!(!untouched.exists());
    drop(loopback);
    // 只有第一次真的注册过一次。
    assert_eq!(
        server.finish(),
        [("/v1/auth/challenges".to_owned(), AGENT.to_owned())]
    );
}
