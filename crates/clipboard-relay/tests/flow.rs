//! 在 127.0.0.1 上起参考中继，用真正的客户端（`msime_client_services::clipboard_relay`，经端到端加密驱动）跑完整流程。每台「设备」是一个独立的密钥串目录。

use msime_client_services::account::AccountClipboardE2eStatus;
use msime_client_services::clipboard_relay::{RelayClipboard, RelayConfig, RelayError};
use msime_clipboard_relay::{store::RelayStore, Relay, Server};
use std::io::{Read, Write};
use std::path::Path;

const TOKEN: &str = "synthetic-relay-token-0001";
const OTHER_TOKEN: &str = "synthetic-relay-token-0002";

fn start(data: &Path) -> Server {
    start_at(data, "127.0.0.1:0".parse().unwrap()).unwrap()
}

fn start_at(data: &Path, address: std::net::SocketAddr) -> std::io::Result<Server> {
    let store = RelayStore::open(data).unwrap();
    let relay = Relay::new(store, &[TOKEN.to_owned(), OTHER_TOKEN.to_owned()]);
    Server::start(address, relay)
}

fn device(server: &Server, directory: &Path, token: &str, name: &str) -> RelayClipboard {
    let url = format!("http://{}/", server.address());
    RelayClipboard::new(RelayConfig::new(&url, token).unwrap(), directory, name).unwrap()
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

#[test]
fn devices_pair_share_and_manage_a_clipboard_through_the_relay() {
    let root = tempfile::tempdir().unwrap();
    let server = start(&root.path().join("relay.db"));
    let laptop = device(&server, &root.path().join("laptop"), TOKEN, "合成笔记本");
    let phone = device(&server, &root.path().join("phone"), TOKEN, "合成手机");

    // 第一台建钥，马上可用。
    let page = laptop.list("").unwrap();
    assert!(page.enabled);
    assert_eq!(page.e2e.status, AccountClipboardE2eStatus::Ready);
    assert!(page.items.is_empty());

    // 第二台登记后等待批准，不能上传。
    let pending = phone.list("").unwrap();
    assert_eq!(pending.e2e.status, AccountClipboardE2eStatus::Pending);
    assert_eq!(
        phone.add("合成上传").unwrap_err().code(),
        "clipboard_device_pending"
    );
    let laptop_view = laptop.devices().unwrap();
    assert_eq!(laptop_view.devices.len(), 2);
    let phone_entry = laptop_view
        .devices
        .iter()
        .find(|entry| !entry.current)
        .unwrap()
        .clone();
    assert!(!phone_entry.approved);
    assert_eq!(phone_entry.name, "合成手机");
    // 两边显示的核对码一致。
    assert_eq!(phone_entry.code, pending.e2e.code);

    // 批准之后互传。
    let approved = laptop.approve(&phone_entry.device_id).unwrap();
    assert!(approved.devices.iter().all(|entry| entry.approved));
    assert_eq!(
        phone.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Ready
    );
    laptop.add("合成的第一条").unwrap();
    phone.add("second synthetic line\n第二行").unwrap();
    assert_eq!(
        texts(&laptop, ""),
        ["second synthetic line\n第二行", "合成的第一条"]
    );
    // 相同内容只保留一条，并移到最前；搜索在本机解密后进行。
    phone.add("合成的第一条").unwrap();
    assert_eq!(
        texts(&phone, ""),
        ["合成的第一条", "second synthetic line\n第二行"]
    );
    assert_eq!(texts(&phone, "SECOND"), ["second synthetic line\n第二行"]);

    // 置顶、保留期、删除一条。
    let items = laptop.list("").unwrap().items;
    let older = items.last().unwrap().clone();
    phone.set_pinned(&older.id, true).unwrap();
    let page = laptop.list("").unwrap();
    assert_eq!(page.items[0].id, older.id);
    assert!(page.items[0].pinned);
    laptop.set_retention(7).unwrap();
    assert_eq!(phone.list("").unwrap().retention_days, 7);
    assert_eq!(laptop.set_retention(5), Err(RelayError::Invalid));
    phone.delete(&older.id).unwrap();
    assert_eq!(texts(&laptop, ""), ["合成的第一条"]);
    assert_eq!(phone.delete(&older.id).unwrap_err().code(), "not_found");

    // 移除手机：换钥，条目清空，手机回到待批准。
    let removed = laptop.remove(&phone_entry.device_id).unwrap();
    assert_eq!(removed.devices.len(), 1);
    assert!(laptop.list("").unwrap().items.is_empty());
    assert_eq!(
        phone.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Pending
    );
    let phone_again = laptop
        .devices()
        .unwrap()
        .devices
        .into_iter()
        .find(|entry| !entry.current)
        .unwrap();
    assert_eq!(phone_again.device_id, phone_entry.device_id);
    laptop.approve(&phone_again.device_id).unwrap();
    phone.add("移除后的一条").unwrap();
    assert_eq!(texts(&laptop, ""), ["移除后的一条"]);

    // 手机重置：只有它自己持有新密钥，笔记本要重新批准。
    let reset = phone.reset().unwrap();
    assert_eq!(reset.status, AccountClipboardE2eStatus::Ready);
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

    // 清空只删条目，设备和密钥都在。
    laptop.add("清空前").unwrap();
    phone.clear().unwrap();
    let page = laptop.list("").unwrap();
    assert!(page.items.is_empty());
    assert_eq!(page.e2e.status, AccountClipboardE2eStatus::Ready);

    // 关闭开关删掉一切；再打开时第一台设备重新建钥。
    laptop.set_enabled(false).unwrap();
    let page = phone.list("").unwrap();
    assert!(!page.enabled);
    assert_eq!(page.e2e.status, AccountClipboardE2eStatus::Disabled);
    assert_eq!(
        phone.add("关闭时上传").unwrap_err().code(),
        "clipboard_sync_disabled"
    );
    laptop.set_enabled(true).unwrap();
    assert_eq!(
        laptop.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Ready
    );
    assert_eq!(
        phone.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Pending
    );
}

#[test]
fn a_wrong_token_is_refused_and_two_tokens_never_see_each_other() {
    let root = tempfile::tempdir().unwrap();
    let server = start(&root.path().join("relay.db"));
    let stranger = device(
        &server,
        &root.path().join("stranger"),
        "synthetic-relay-token-9999",
        "合成",
    );
    assert_eq!(stranger.list("").unwrap_err(), RelayError::Unauthorized);
    assert_eq!(stranger.list("").unwrap_err().code(), "relay_unauthorized");
    assert_eq!(stranger.clear().unwrap_err().code(), "relay_unauthorized");

    let first = device(&server, &root.path().join("first"), TOKEN, "合成甲");
    let second = device(&server, &root.path().join("second"), OTHER_TOKEN, "合成乙");
    first.list("").unwrap();
    first.add("只属于第一个空间").unwrap();
    // 另一个令牌是另一个空间：它自己建钥，看不到第一个空间的设备和条目。
    let page = second.list("").unwrap();
    assert_eq!(page.e2e.status, AccountClipboardE2eStatus::Ready);
    assert!(page.items.is_empty());
    assert_eq!(second.devices().unwrap().devices.len(), 1);
    second.clear().unwrap();
    assert_eq!(texts(&first, ""), ["只属于第一个空间"]);
}

#[test]
fn data_survives_a_restart() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("relay.db");
    let laptop_directory = root.path().join("laptop");
    let address = {
        let server = start(&data);
        let laptop = device(&server, &laptop_directory, TOKEN, "合成");
        laptop.list("").unwrap();
        laptop.add("重启前写入").unwrap();
        server.address()
    };
    // 本机的密钥串绑定中继地址（含端口），重启后要在同一个地址上服务，设备才认得出是同一个中继。旧的监听套接字在后台关闭，端口空出来之前重试。
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let server = loop {
        match start_at(&data, address) {
            Ok(server) => break server,
            Err(error) if std::time::Instant::now() < deadline => {
                assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse, "{error}");
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(error) => panic!("{error}"),
        }
    };
    let laptop = device(&server, &laptop_directory, TOKEN, "合成");
    let page = laptop.list("").unwrap();
    assert_eq!(page.e2e.status, AccountClipboardE2eStatus::Ready);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].text, "重启前写入");
}

/// 发一个原始的 HTTP/1.1 请求，返回状态码。
fn raw_status(server: &Server, request: &[u8]) -> u16 {
    let mut stream = std::net::TcpStream::connect(server.address()).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    // 中继可能在读完请求体之前就回答并关闭连接，这时写入失败不算错。
    let _ = stream.write_all(request);
    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    let text = String::from_utf8_lossy(&response);
    text.split_whitespace()
        .nth(1)
        .and_then(|status| status.parse().ok())
        .unwrap_or(0)
}

#[test]
fn oversized_or_unauthenticated_requests_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let server = start(&root.path().join("relay.db"));
    let body = vec![b'x'; 100 * 1024];
    let mut request = format!(
        "POST /v1/users/me/clipboard HTTP/1.1\r\nHost: relay\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    request.extend_from_slice(&body);
    assert_eq!(raw_status(&server, &request), 413);

    let unauthenticated =
        b"GET /v1/users/me/clipboard HTTP/1.1\r\nHost: relay\r\nConnection: close\r\n\r\n";
    assert_eq!(raw_status(&server, unauthenticated), 401);
}

/// 自己跑驱动、自己保存密钥串的宿主（Android 把密钥串交给 AndroidKeyStore 加密）只借 `RelayTransport` 发请求：它与用 `RelayClipboard` 的设备互相批准、互传，密钥串只在宿主自己手里，客户端这边不落任何文件。
#[test]
fn a_host_that_keeps_its_own_keystore_pairs_through_the_transport_alone() {
    use msime_client_core::cloud::clipboard_e2e::{self, HttpResponse, Operation};
    use msime_client_services::clipboard_relay::RelayTransport;

    let root = tempfile::tempdir().unwrap();
    let server = start(&root.path().join("relay.db"));
    let laptop = device(&server, &root.path().join("laptop"), TOKEN, "合成笔记本");
    assert_eq!(
        laptop.list("").unwrap().e2e.status,
        AccountClipboardE2eStatus::Ready
    );

    let url = format!("http://{}/", server.address());
    let transport = RelayTransport::new(RelayConfig::new(&url, TOKEN).unwrap()).unwrap();
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

    // 登记后等待批准，笔记本上看得到它、核对码一致。
    let pending = run(Operation::List {
        search: String::new(),
    })
    .unwrap();
    assert_eq!(pending["e2e"]["status"], "pending");
    let entry = laptop
        .devices()
        .unwrap()
        .devices
        .into_iter()
        .find(|entry| !entry.current)
        .unwrap();
    assert_eq!(entry.name, "合成手机");
    assert_eq!(pending["e2e"]["code"], entry.code.as_str());
    laptop.approve(&entry.device_id).unwrap();

    // 批准之后互传。
    laptop.add("合成的第一条").unwrap();
    let page = run(Operation::List {
        search: String::new(),
    })
    .unwrap();
    assert_eq!(page["e2e"]["status"], "ready");
    assert_eq!(page["items"][0]["text"], "合成的第一条");
    run(Operation::Add {
        text: "合成的第二条".into(),
    })
    .unwrap();
    assert!(texts(&laptop, "").contains(&"合成的第二条".to_owned()));

    // 状态码原样交回；路径不在云剪贴板接口下、令牌被拒都不交给驱动。
    let missing: HttpResponse = transport
        .send(
            "DELETE",
            "/v1/users/me/clipboard/devices/ffffffffffffffffffffffffffffffff",
            None,
        )
        .unwrap();
    assert_eq!(missing.status, 404);
    assert_eq!(
        transport
            .send("GET", "/v1/users/me/profile", None)
            .unwrap_err(),
        RelayError::Invalid
    );
    let refused =
        RelayTransport::new(RelayConfig::new(&url, "synthetic-relay-token-unknown").unwrap())
            .unwrap();
    assert_eq!(
        refused
            .send("GET", "/v1/users/me/clipboard/keys", None)
            .unwrap_err(),
        RelayError::Unauthorized
    );
    // 中继侧之外，只有笔记本那台 `RelayClipboard` 设备的目录。
    let mut entries: Vec<_> = std::fs::read_dir(root.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| !name.starts_with("relay.db"))
        .collect();
    entries.sort();
    assert_eq!(entries, ["laptop"]);
}
