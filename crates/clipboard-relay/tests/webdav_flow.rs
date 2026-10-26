//! 云剪贴板的 WebDAV 后端对着一个真实的 WebDAV 服务端（`dav-server` 的内存文件系统，经 axum 跑在 127.0.0.1 上）跑与 `flow.rs` 相同的完整流程：两台 `RelayClipboard` 设备经虚拟中继建钥、批准、互传、去重、搜索、置顶、保留期、删除、移除后换钥、重新批准、重置、清空、关闭开关。
//!
//! 这个测试放在参考中继的 crate 里，因为这里已经有 axum 和 tokio，跑 `dav-server` 不用再给客户端的 crate 加 HTTP 服务端依赖。`dav-server` 的 MOVE 是先查后改（目标在不在、再改名），并发时不保证只有一个成功；外层用一把锁让请求一个一个处理，相当于一个独占创建可靠的网盘。不加锁时的行为由客户端单元测试里的假网盘覆盖。

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Router;
use base64::Engine as _;
use dav_server::{fakels::FakeLs, memfs::MemFs, DavHandler};
use msime_client_core::cloud::clipboard_e2e::{self, Operation};
use msime_client_services::account::AccountClipboardE2eStatus;
use msime_client_services::clipboard_relay::{
    RelayClipboard, RelayConfig, RelayError, RelayTransport,
};
use msime_client_services::clipboard_webdav::{probe, WEBDAV_STATE_FILE};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

const USER: &str = "synthetic@example.test";
const PASSWORD: &str = "synthetic-app-password";

#[derive(Clone)]
struct Dav {
    handler: DavHandler,
    gate: Arc<tokio::sync::Mutex<()>>,
    authorization: String,
}

async fn handle(State(dav): State<Dav>, request: Request) -> Response {
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        == Some(dav.authorization.as_str());
    if !authorized {
        return (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Basic realm=\"dav\"")],
        )
            .into_response();
    }
    let _serial = dav.gate.lock().await;
    let (parts, body) = request.into_parts();
    dav.handler
        .handle(Request::from_parts(parts, body))
        .await
        .map(Body::new)
        .into_response()
}

struct Server {
    address: SocketAddr,
    runtime: Option<tokio::runtime::Runtime>,
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

fn start() -> Server {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_io()
        .build()
        .unwrap();
    let listener = runtime
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .unwrap();
    let address = listener.local_addr().unwrap();
    let dav = Dav {
        handler: DavHandler::builder()
            .filesystem(MemFs::new())
            .locksystem(FakeLs::new())
            .strip_prefix("/dav")
            .build_handler(),
        gate: Arc::default(),
        authorization: format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{USER}:{PASSWORD}"))
        ),
    };
    runtime.spawn(async move {
        let app = Router::new().fallback(handle).with_state(dav);
        let _ = axum::serve(listener, app).await;
    });
    Server {
        address,
        runtime: Some(runtime),
    }
}

fn url(server: &Server) -> String {
    format!("http://{}/dav/", server.address)
}

fn config(server: &Server) -> RelayConfig {
    RelayConfig::webdav(&url(server), USER, PASSWORD).unwrap()
}

fn device(server: &Server, directory: &Path, name: &str) -> RelayClipboard {
    RelayClipboard::new(config(server), directory, name).unwrap()
}

fn texts(device: &RelayClipboard, search: &str) -> Vec<String> {
    device
        .list(search)
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.text)
        .collect()
}

/// 本机缓存在两秒内直接复用；流程里两台设备交替操作，每步之间等一下，让对方的写入被读到。
fn settle() {
    std::thread::sleep(std::time::Duration::from_millis(2100));
}

#[test]
fn the_probe_passes_a_real_webdav_server_and_refuses_a_wrong_password() {
    let server = start();
    probe(&config(&server)).unwrap();
    let wrong = RelayConfig::webdav(&url(&server), USER, "not-the-password").unwrap();
    assert_eq!(probe(&wrong), Err(RelayError::Unauthorized));
    assert_eq!(
        RelayError::Unauthorized.code(),
        "relay_unauthorized",
        "错误的应用密码与中继令牌错误报同一个码，宿主不会当成水杉账号未登录"
    );
}

#[test]
fn devices_pair_share_and_manage_a_clipboard_on_a_webdav_server() {
    let root = tempfile::tempdir().unwrap();
    let server = start();
    let laptop = device(&server, &root.path().join("laptop"), "合成笔记本");
    let phone = device(&server, &root.path().join("phone"), "合成手机");

    let page = laptop.list("").unwrap();
    assert!(page.enabled);
    assert_eq!(page.e2e.status, AccountClipboardE2eStatus::Ready);

    let pending = phone.list("").unwrap();
    assert_eq!(pending.e2e.status, AccountClipboardE2eStatus::Pending);
    assert_eq!(
        phone.add("合成上传").unwrap_err().code(),
        "clipboard_device_pending"
    );
    settle();
    let phone_entry = laptop
        .devices()
        .unwrap()
        .devices
        .into_iter()
        .find(|entry| !entry.current)
        .unwrap();
    assert!(!phone_entry.approved);
    assert_eq!(phone_entry.name, "合成手机");
    assert_eq!(phone_entry.code, pending.e2e.code);

    let approved = laptop.approve(&phone_entry.device_id).unwrap();
    assert!(approved.devices.iter().all(|entry| entry.approved));
    settle();
    assert_eq!(
        phone.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Ready
    );
    laptop.add("合成的第一条").unwrap();
    phone.add("second synthetic line\n第二行").unwrap();
    settle();
    assert_eq!(
        texts(&laptop, ""),
        ["second synthetic line\n第二行", "合成的第一条"]
    );
    phone.add("合成的第一条").unwrap();
    assert_eq!(
        texts(&phone, ""),
        ["合成的第一条", "second synthetic line\n第二行"]
    );
    assert_eq!(texts(&phone, "SECOND"), ["second synthetic line\n第二行"]);

    settle();
    let older = laptop.list("").unwrap().items.last().unwrap().clone();
    phone.set_pinned(&older.id, true).unwrap();
    settle();
    let page = laptop.list("").unwrap();
    assert_eq!(page.items[0].id, older.id);
    assert!(page.items[0].pinned);
    laptop.set_retention(7).unwrap();
    settle();
    assert_eq!(phone.list("").unwrap().retention_days, 7);
    phone.delete(&older.id).unwrap();
    settle();
    assert_eq!(texts(&laptop, ""), ["合成的第一条"]);
    assert_eq!(phone.delete(&older.id).unwrap_err().code(), "not_found");

    // 移除手机：换钥，条目清空，手机回到待批准；再批准一次。
    let removed = laptop.remove(&phone_entry.device_id).unwrap();
    assert_eq!(removed.devices.len(), 1);
    settle();
    assert_eq!(
        phone.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Pending
    );
    settle();
    let again = laptop
        .devices()
        .unwrap()
        .devices
        .into_iter()
        .find(|entry| !entry.current)
        .unwrap();
    assert_eq!(again.device_id, phone_entry.device_id);
    laptop.approve(&again.device_id).unwrap();
    settle();
    phone.add("移除后的一条").unwrap();
    settle();
    assert_eq!(texts(&laptop, ""), ["移除后的一条"]);

    // 手机重置，笔记本要重新批准。
    let reset = phone.reset().unwrap();
    settle();
    assert_eq!(
        laptop.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Pending
    );
    let laptop_id = reset
        .devices
        .iter()
        .find(|entry| !entry.current)
        .unwrap()
        .device_id
        .clone();
    phone.approve(&laptop_id).unwrap();
    settle();

    // 清空只删条目。
    laptop.add("清空前").unwrap();
    phone.clear().unwrap();
    settle();
    let page = laptop.list("").unwrap();
    assert!(page.items.is_empty());
    assert_eq!(page.e2e.status, AccountClipboardE2eStatus::Ready);

    // 关闭开关删掉一切；再打开时第一台设备重新建钥。
    laptop.set_enabled(false).unwrap();
    settle();
    let page = phone.list("").unwrap();
    assert!(!page.enabled);
    assert_eq!(page.e2e.status, AccountClipboardE2eStatus::Disabled);
    laptop.set_enabled(true).unwrap();
    assert_eq!(
        laptop.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Ready
    );
    settle();
    assert_eq!(
        phone.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Pending
    );
    // 本机只有密钥串、日志缓存和锁文件。
    let mut names: Vec<String> = std::fs::read_dir(root.path().join("laptop"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "cloud-clipboard-relay-keystore",
            "cloud-clipboard-relay.lock",
            WEBDAV_STATE_FILE,
        ]
    );
}

/// 自己跑驱动、自己保存密钥串的宿主（Android）只借 `RelayTransport::with_directory` 发请求：它与用 `RelayClipboard` 的设备互相批准、互传。
#[test]
fn a_host_that_keeps_its_own_keystore_pairs_through_the_transport_alone() {
    let root = tempfile::tempdir().unwrap();
    let server = start();
    let laptop = device(&server, &root.path().join("laptop"), "合成笔记本");
    laptop.list("").unwrap();

    assert!(RelayTransport::new(config(&server)).is_err());
    let transport =
        RelayTransport::with_directory(config(&server), root.path().join("phone-cache")).unwrap();
    let mut keystore: Option<String> = None;
    let mut run = |operation: Operation| {
        let saved = keystore.clone();
        let mut latest = saved.clone();
        let result = clipboard_e2e::run(
            operation,
            saved.as_deref(),
            "合成手机",
            |request| transport.send(&request.method, &request.path, request.body.as_deref()),
            |value| {
                latest = Some(value.to_owned());
                Ok::<(), RelayError>(())
            },
            || RelayError::Unavailable,
        )
        .unwrap();
        keystore = latest;
        result
    };
    let pending = run(Operation::List {
        search: String::new(),
    })
    .unwrap();
    assert_eq!(pending["e2e"]["status"], "pending");
    settle();
    let entry = laptop
        .devices()
        .unwrap()
        .devices
        .into_iter()
        .find(|entry| !entry.current)
        .unwrap();
    assert_eq!(pending["e2e"]["code"], entry.code.as_str());
    laptop.approve(&entry.device_id).unwrap();
    laptop.add("合成的第一条").unwrap();
    settle();
    let page = run(Operation::List {
        search: String::new(),
    })
    .unwrap();
    assert_eq!(page["items"][0]["text"], "合成的第一条");
    run(Operation::Add {
        text: "合成的第二条".into(),
    })
    .unwrap();
    settle();
    assert!(texts(&laptop, "").contains(&"合成的第二条".to_owned()));
    // 宿主那一侧的目录里只有日志缓存和锁，没有密钥串。
    let mut names: Vec<String> = std::fs::read_dir(root.path().join("phone-cache"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["cloud-clipboard-relay.lock", WEBDAV_STATE_FILE]);
}
