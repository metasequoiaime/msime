//! 词库同步的 WebDAV 传输对着一个真实的 WebDAV 服务端（`dav-server` 的内存文件系统，经 axum 跑在 127.0.0.1 上）：两台设备经词库自己的密钥串配对、批准、互传词条；错误的应用密码、超大响应、两台设备同时抢序号、网盘上的日志被截尾、服务端暂时不可用之后恢复；凭据和词条不出现在网盘、本机状态和调试输出里。
//!
//! 与 `webdav_flow.rs` 一样放在参考中继的 crate 里，因为这里已经有 axum、tokio 和维护者同意的 `dav-server`。外层的锁让请求一个一个处理（`dav-server` 的 MOVE 先查后改），相当于一个独占创建可靠的网盘；协议在不可靠网盘上的行为由 `client-core` 的模拟测试覆盖。

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Router;
use base64::Engine as _;
use dav_server::{fakels::FakeLs, memfs::MemFs, DavHandler};
use msime_client_core::cloud::dictionary_sync::{
    LocalChange, LocalDictionary, LocalError, LocalOutcome, Options, Trigger, WordKey, WordKind,
};
use msime_client_services::dictionary_webdav::{probe, DictionarySync, RunResult, STATE_FILE};
use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const USER: &str = "synthetic@example.test";
const PASSWORD: &str = "synthetic-app-password";

#[derive(Clone)]
struct Dav {
    handler: DavHandler,
    gate: Arc<tokio::sync::Mutex<()>>,
    authorization: String,
    /// 服务端暂时不可用：每个请求答 503，不带 `Retry-After`。
    down: Arc<AtomicBool>,
}

async fn handle(State(dav): State<Dav>, request: Request) -> Response {
    if dav.down.load(Ordering::SeqCst) {
        return StatusCode::BAD_GATEWAY.into_response();
    }
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
    dav: Dav,
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
        down: Arc::default(),
    };
    let app_dav = dav.clone();
    runtime.spawn(async move {
        let app = Router::new().fallback(handle).with_state(app_dav);
        let _ = axum::serve(listener, app).await;
    });
    Server {
        address,
        dav,
        runtime: Some(runtime),
    }
}

impl Server {
    fn url(&self) -> String {
        format!("http://{}/dav/", self.address)
    }

    /// 绕过鉴权直接对文件系统发一个请求，测试用来种文件、删文件、列目录。
    fn raw(&self, method: &str, path: &str, depth: Option<&str>, body: Vec<u8>) -> (u16, Vec<u8>) {
        let runtime = self.runtime.as_ref().unwrap();
        let handler = self.dav.handler.clone();
        let mut builder = Request::builder()
            .method(Method::from_bytes(method.as_bytes()).unwrap())
            .uri(format!("/dav/{path}"));
        if let Some(depth) = depth {
            builder = builder.header("Depth", depth);
        }
        let request = builder.body(Body::from(body)).unwrap();
        runtime.block_on(async move {
            let response = handler.handle(request).await;
            let status = response.status().as_u16();
            let bytes = axum::body::to_bytes(Body::new(response.into_body()), usize::MAX)
                .await
                .unwrap()
                .to_vec();
            (status, bytes)
        })
    }

    /// 一个集合里的条目：名字（`href` 的最后一段）和是不是集合（`href` 以 `/` 结尾）。
    fn entries(&self, dir: &str) -> Vec<(String, bool)> {
        let (status, body) = self.raw("PROPFIND", dir, Some("1"), Vec::new());
        if status != 207 {
            return Vec::new();
        }
        let text = String::from_utf8(body).unwrap();
        let own = format!("/dav/{dir}");
        let mut out = Vec::new();
        for chunk in text.split("href>").skip(1).step_by(2) {
            let href = chunk.split('<').next().unwrap_or_default();
            let path = href.find("/dav/").map_or(href, |start| &href[start..]);
            if path == own || path.trim_end_matches('/') == own.trim_end_matches('/') {
                continue;
            }
            let collection = path.ends_with('/');
            let name = path
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or_default();
            if !name.is_empty() {
                out.push((name.to_owned(), collection));
            }
        }
        out
    }

    fn list(&self, dir: &str) -> Vec<String> {
        self.entries(dir)
            .into_iter()
            .map(|(name, _)| name)
            .collect()
    }

    /// 网盘上的全部文件和它们的内容。
    fn files(&self, dir: &str) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        for (name, collection) in self.entries(dir) {
            let path = format!("{dir}{name}");
            if collection {
                out.extend(self.files(&format!("{path}/")));
            } else {
                let (status, body) = self.raw("GET", &path, None, Vec::new());
                assert_eq!(status, 200, "{path}");
                out.push((path, body));
            }
        }
        out
    }

    /// 当前 epoch 目录里最大的事件文件名。
    fn last_event(&self) -> Option<String> {
        let mut epochs: Vec<String> = self
            .list("msime-dictionary/v1/")
            .into_iter()
            .filter(|name| name.starts_with("epoch-"))
            .collect();
        epochs.sort();
        let epoch = epochs.pop()?;
        let mut events: Vec<String> = self
            .list(&format!("msime-dictionary/v1/{epoch}/"))
            .into_iter()
            .filter(|name| name.starts_with("e-"))
            .collect();
        events.sort();
        events
            .pop()
            .map(|event| format!("msime-dictionary/v1/{epoch}/{event}"))
    }
}

/// 内存里的「我的词条」。
#[derive(Default)]
struct Words(BTreeSet<WordKey>);

impl LocalDictionary for Words {
    fn words(&mut self) -> Result<BTreeSet<WordKey>, LocalError> {
        Ok(self.0.clone())
    }

    fn apply(&mut self, changes: &[LocalChange]) -> Result<Vec<LocalOutcome>, LocalError> {
        Ok(changes
            .iter()
            .map(|change| {
                if change.add {
                    self.0.insert(change.key.clone());
                } else {
                    self.0.remove(&change.key);
                }
                LocalOutcome::Applied
            })
            .collect())
    }
}

fn word(code: &str, text: &str) -> WordKey {
    WordKey::new(WordKind::Pinyin, code, text).unwrap()
}

fn device(server: &Server, directory: &Path, name: &str) -> DictionarySync {
    DictionarySync::new(&server.url(), USER, PASSWORD, directory, name).unwrap()
}

fn manual(sync: &DictionarySync, words: &mut Words) -> RunResult {
    sync.run(words, Trigger::Manual, Options::default())
        .unwrap()
}

/// 密钥串的虚拟中继在两秒内直接用本机缓存；两台设备交替操作时等一下，让对方的写入被读到。
fn settle() {
    std::thread::sleep(std::time::Duration::from_millis(2100));
}

/// 两台设备配对：A 是第一台，B 登记后由 A 批准。
fn paired(
    server: &Server,
    a_dir: &Path,
    b_dir: &Path,
    a: &mut Words,
    b: &mut Words,
) -> (DictionarySync, DictionarySync) {
    let first = device(server, a_dir, "MacBook");
    let second = device(server, b_dir, "Pixel");
    let run = manual(&first, a);
    assert!(run.view.approved, "the first device creates the key");
    let run = manual(&second, b);
    assert!(!run.view.approved, "a new device waits for approval");
    settle();
    let devices = first.devices().unwrap();
    let pending = devices
        .devices
        .iter()
        .find(|device| !device.approved)
        .expect("B is listed as pending");
    assert_eq!(Some(pending.device_id.clone()), run.view.device_id);
    assert_eq!(
        Some(pending.code.clone()),
        run.view.code,
        "both sides show the same code"
    );
    first.approve(&pending.device_id).unwrap();
    settle();
    (first, second)
}

#[test]
fn two_devices_pair_and_sync_their_words_through_a_real_webdav_server() {
    let server = start();
    let (a_dir, b_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut a = Words(BTreeSet::from([
        word("shui'shan", "水杉"),
        word("ni'hao", "你好"),
    ]));
    let mut b = Words(BTreeSet::from([word("zai'jian", "再见")]));
    let (first, second) = paired(&server, a_dir.path(), b_dir.path(), &mut a, &mut b);
    let run = manual(&second, &mut b);
    assert!(run.view.approved && run.view.joined, "{:?}", run.view);
    run.report.unwrap();
    manual(&first, &mut a).report.unwrap();
    assert_eq!(a.0, b.0);
    assert_eq!(a.0.len(), 3);
    // B 改编码、A 删词，各同步一次之后两边一样。
    b.0.remove(&word("shui'shan", "水杉"));
    b.0.insert(word("ss", "水杉"));
    manual(&second, &mut b).report.unwrap();
    a.0.remove(&word("ni'hao", "你好"));
    manual(&first, &mut a).report.unwrap();
    manual(&second, &mut b).report.unwrap();
    assert_eq!(a.0, b.0);
    assert_eq!(
        a.0,
        BTreeSet::from([word("ss", "水杉"), word("zai'jian", "再见")])
    );
    // 网盘上的布局：词库日志和密钥串各在自己的目录下，与云剪贴板的 msime-clipboard/ 不相干。
    let root = server.list("");
    assert_eq!(root, vec!["msime-dictionary".to_owned()]);
    let mut inside = server.list("msime-dictionary/");
    inside.sort();
    assert_eq!(inside, vec!["keyring".to_owned(), "v1".to_owned()]);
    assert_eq!(
        server.list("msime-dictionary/keyring/"),
        vec!["msime-clipboard".to_owned()]
    );
}

#[test]
fn a_pending_device_is_listed_once_however_often_its_processes_ask() {
    let server = start();
    let (a_dir, b_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let first = device(&server, a_dir.path(), "MacBook");
    manual(&first, &mut Words::default());
    // 同一个安装的两个进程同时第一次使用：共用一个目录，整次同步串行，只登记一台设备。
    let path = b_dir.path().to_path_buf();
    let url = server.url();
    let racers: Vec<_> = (0..2)
        .map(|_| {
            let path = path.clone();
            let url = url.clone();
            std::thread::spawn(move || {
                let sync = DictionarySync::new(&url, USER, PASSWORD, &path, "Pixel").unwrap();
                sync.run(&mut Words::default(), Trigger::Manual, Options::default())
                    .unwrap()
                    .view
                    .device_id
            })
        })
        .collect();
    let ids: Vec<_> = racers
        .into_iter()
        .map(|racer| racer.join().unwrap())
        .collect();
    assert_eq!(ids[0], ids[1]);
    let second = device(&server, b_dir.path(), "Pixel");
    for _ in 0..3 {
        manual(&second, &mut Words::default());
    }
    settle();
    let devices = first.devices().unwrap();
    assert_eq!(devices.devices.len(), 2, "{devices:?}");
    assert_eq!(
        devices
            .devices
            .iter()
            .filter(|device| !device.approved)
            .count(),
        1
    );
}

#[test]
fn a_wrong_app_password_is_unauthorized_and_the_probe_says_so_too() {
    let server = start();
    let directory = tempfile::tempdir().unwrap();
    probe(&server.url(), USER, PASSWORD, Some(directory.path())).unwrap();
    assert_eq!(
        probe(&server.url(), USER, "not-the-password", None)
            .unwrap_err()
            .code(),
        "dictionary_sync_unauthorized"
    );
    let wrong = DictionarySync::new(
        &server.url(),
        USER,
        "not-the-password",
        directory.path(),
        "x",
    )
    .unwrap();
    let run = wrong
        .run(&mut Words::default(), Trigger::Manual, Options::default())
        .unwrap();
    assert_eq!(
        run.report.unwrap_err().code(),
        "dictionary_sync_unauthorized"
    );
    assert_eq!(run.view.device_id, None, "nothing was registered");
    // 测试连接的探测目录用完就删掉，只留下密钥串要用的目录。
    assert_eq!(
        server.list("msime-dictionary/keyring/msime-clipboard/"),
        Vec::<String>::new()
    );
    // 连不上的地址是另一个码。
    assert_eq!(
        probe("http://127.0.0.1:9/dav/", USER, PASSWORD, None)
            .unwrap_err()
            .code(),
        "dictionary_sync_unavailable"
    );
}

#[test]
fn an_oversized_event_is_reported_as_too_large_without_reading_it_all() {
    let server = start();
    let directory = tempfile::tempdir().unwrap();
    let sync = device(&server, directory.path(), "MacBook");
    let mut words = Words(BTreeSet::from([word("ma", "妈")]));
    manual(&sync, &mut words).report.unwrap();
    let last = server.last_event().unwrap();
    let (prefix, name) = last.rsplit_once("/e-").unwrap();
    let next: u64 = name.trim_end_matches(".json").parse::<u64>().unwrap() + 1;
    let planted = format!("{prefix}/e-{next:012}.json");
    let (status, _) = server.raw("PUT", &planted, None, vec![b' '; 600 * 1024]);
    assert!((200..300).contains(&status));
    let run = manual(&sync, &mut words);
    assert_eq!(run.report.unwrap_err().code(), "dictionary_sync_too_large");
    assert_eq!(
        run.view.last_error.as_deref(),
        Some("dictionary_sync_too_large")
    );
}

#[test]
fn two_devices_racing_for_the_same_sequence_number_both_get_their_words_in() {
    let server = start();
    let (a_dir, b_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut a = Words::default();
    let mut b = Words::default();
    let (first, second) = paired(&server, a_dir.path(), b_dir.path(), &mut a, &mut b);
    manual(&second, &mut b).report.unwrap();
    manual(&first, &mut a).report.unwrap();
    let first = Arc::new(first);
    let second = Arc::new(second);
    for round in 0..3u32 {
        a.0.insert(word("ma", &format!("甲{round}")));
        b.0.insert(word("ba", &format!("乙{round}")));
        let (first_clone, second_clone) = (first.clone(), second.clone());
        let mut a_words = std::mem::take(&mut a);
        let mut b_words = std::mem::take(&mut b);
        let left = std::thread::spawn(move || {
            let run = manual(&first_clone, &mut a_words);
            (run.report.is_ok(), a_words)
        });
        let right = std::thread::spawn(move || {
            let run = manual(&second_clone, &mut b_words);
            (run.report.is_ok(), b_words)
        });
        let (left_ok, left_words) = left.join().unwrap();
        let (right_ok, right_words) = right.join().unwrap();
        assert!(left_ok && right_ok, "round {round}");
        a = left_words;
        b = right_words;
    }
    for _ in 0..2 {
        manual(&first, &mut a).report.unwrap();
        manual(&second, &mut b).report.unwrap();
    }
    assert_eq!(a.0, b.0);
    assert_eq!(a.0.len(), 6);
}

#[test]
fn a_truncated_log_is_refused_until_the_user_trusts_the_share_again() {
    let server = start();
    let directory = tempfile::tempdir().unwrap();
    let sync = device(&server, directory.path(), "MacBook");
    let mut words = Words(BTreeSet::from([word("ma", "妈")]));
    manual(&sync, &mut words).report.unwrap();
    words.0.insert(word("ba", "爸"));
    manual(&sync, &mut words).report.unwrap();
    // 有人删掉了本机读过的最后一个事件。
    let last = server.last_event().unwrap();
    let (status, _) = server.raw("DELETE", &last, None, Vec::new());
    assert!((200..300).contains(&status));
    let run = manual(&sync, &mut words);
    assert_eq!(run.report.unwrap_err().code(), "dictionary_sync_rollback");
    // 普通探测不解除防回退；只有用户确认恢复才从网盘现状重新开始。
    let action = serde_json::json!({"operation": "probe", "url": server.url(), "username": USER, "password": PASSWORD});
    assert_eq!(host_probe(directory.path(), action.clone())["ok"], true);
    assert_eq!(
        manual(&sync, &mut words).report.unwrap_err().code(),
        "dictionary_sync_rollback"
    );
    let mut confirmed = action;
    confirmed["trust_remote"] = serde_json::json!(true);
    assert_eq!(host_probe(directory.path(), confirmed)["ok"], true);
    let run = manual(&sync, &mut words);
    run.report.unwrap();
    assert!(run.view.joined);
    assert_eq!(words.0.len(), 2);
}

#[allow(unsafe_code)]
fn host_probe(directory: &Path, action: serde_json::Value) -> serde_json::Value {
    let body =
        serde_json::json!({"options": {}, "directory": directory, "action": action}).to_string();
    // SAFETY: 活字符串提供完整字节区间，响应立即复制并通过配对接口释放。
    unsafe {
        let pointer = msime_host_api::msime_client_dictionary_sync(body.as_ptr(), body.len());
        assert!(!pointer.is_null());
        let text = std::ffi::CStr::from_ptr(pointer)
            .to_str()
            .unwrap()
            .to_owned();
        msime_host_api::msime_client_string_free(pointer);
        serde_json::from_str(&text).unwrap()
    }
}

/// 普通设置探测不能忘掉尚未发送的删除；这里走宿主实际使用的 C ABI。
#[test]
fn a_host_connection_probe_keeps_pending_deletions() {
    let server = start();
    let directory = tempfile::tempdir().unwrap();
    let sync = device(&server, directory.path(), "Synthetic device");
    let mut words = Words(BTreeSet::from([word("ma", "妈")]));
    manual(&sync, &mut words).report.unwrap();
    words.0.clear();
    sync.run(&mut words, Trigger::LocalChange, Options::default())
        .unwrap()
        .report
        .unwrap();
    assert_eq!(sync.status().unwrap().pending_upload, 1);
    let response = host_probe(
        directory.path(),
        serde_json::json!({"operation": "probe", "url": server.url(), "username": USER, "password": PASSWORD}),
    );
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(sync.status().unwrap().pending_upload, 1);
    manual(&sync, &mut words).report.unwrap();
    assert!(words.0.is_empty(), "测试连接不能让已删除的词从网盘回来");
}

#[test]
fn changes_made_while_the_server_is_down_go_out_once_it_is_back() {
    let server = start();
    let (a_dir, b_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut a = Words::default();
    let mut b = Words::default();
    let (first, second) = paired(&server, a_dir.path(), b_dir.path(), &mut a, &mut b);
    manual(&second, &mut b).report.unwrap();
    server.dav.down.store(true, Ordering::SeqCst);
    a.0.insert(word("duan'wang", "断网"));
    // 改词之后马上记下来，不联网。
    let run = first
        .run(&mut a, Trigger::LocalChange, Options::default())
        .unwrap();
    assert!(!run.network);
    assert_eq!(run.view.pending_upload, 1);
    let run = manual(&first, &mut a);
    assert_eq!(
        run.report.unwrap_err().code(),
        "dictionary_sync_unavailable"
    );
    assert_eq!(run.view.pending_upload, 1, "kept for later");
    server.dav.down.store(false, Ordering::SeqCst);
    settle();
    let run = manual(&first, &mut a);
    run.report.unwrap();
    assert_eq!(run.view.pending_upload, 0);
    manual(&second, &mut b).report.unwrap();
    assert!(b.0.contains(&word("duan'wang", "断网")));
}

#[test]
fn credentials_and_words_never_reach_the_share_and_credentials_never_reach_local_state() {
    let server = start();
    let directory = tempfile::tempdir().unwrap();
    let sync = device(&server, directory.path(), "MacBook");
    let mut words = Words(BTreeSet::from([word("mi'mi", "秘密词条")]));
    manual(&sync, &mut words).report.unwrap();
    let files = server.files("msime-dictionary/");
    assert!(files.len() > 3, "{}", files.len());
    for (path, bytes) in files {
        let text = String::from_utf8_lossy(&bytes);
        for needle in [USER, PASSWORD, "秘密词条", "mi'mi"] {
            assert!(!text.contains(needle), "{path} contains {needle}");
        }
    }
    for entry in std::fs::read_dir(directory.path()).unwrap() {
        let path = entry.unwrap().path();
        let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        assert!(!text.contains(PASSWORD), "{path:?}");
        assert!(!text.contains(USER), "{path:?}");
    }
    assert!(directory.path().join(STATE_FILE).exists());
    let debug = format!("{sync:?} {:?}", sync.status().unwrap());
    assert!(!debug.contains(PASSWORD) && !debug.contains(USER) && !debug.contains("秘密词条"));
}

#[test]
fn oversized_local_changes_keep_state_readable_and_allow_recovery() {
    let server = start();
    let dir = tempfile::tempdir().unwrap();
    let sync = device(&server, dir.path(), "Synthetic");
    let original = word("ma", "合成词");
    let mut words = Words(BTreeSet::from([original.clone()]));
    manual(&sync, &mut words).report.unwrap();
    let before = std::fs::read(dir.path().join(STATE_FILE)).unwrap();
    let mut remote_before = server.files("msime-dictionary/");
    remote_before.sort();
    for index in 0..10_000 {
        words.0.insert(
            WordKey::new(WordKind::Quick, &format!("q{index}"), &"s".repeat(4096)).unwrap(),
        );
    }
    let run = sync
        .run(&mut words, Trigger::LocalChange, Options::default())
        .unwrap();
    assert!(!run.network);
    assert_eq!(run.report.unwrap_err().code(), "dictionary_sync_too_large");
    assert_eq!(std::fs::read(dir.path().join(STATE_FILE)).unwrap(), before);
    sync.status().unwrap();
    let mut remote_after = server.files("msime-dictionary/");
    remote_after.sort();
    assert_eq!(remote_after, remote_before);
    words.0 = BTreeSet::from([
        original,
        WordKey::new(WordKind::Quick, "small", "合成恢复词").unwrap(),
    ]);
    sync.run(&mut words, Trigger::LocalChange, Options::default())
        .unwrap()
        .report
        .unwrap();
    manual(&sync, &mut words).report.unwrap();
    assert_eq!(sync.status().unwrap().pending_upload, 0);
}
