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
use msime_client_core::cloud::dictionary_sync::schedule::{MIN_PUSH_INTERVAL_MS, OPEN_PULL_MIN_MS};
use msime_client_core::cloud::dictionary_sync::{
    LocalChange, LocalDictionary, LocalError, LocalOutcome, Options, Trigger, WordKey, WordKind,
};
use msime_client_services::dictionary_webdav::{probe, DictionarySync, RunResult, STATE_FILE};
use msime_client_services::dictionary_webdav_scheduler::{
    fingerprint, Round, Scheduler, SchedulerHost, Timing, Watched, BACKGROUND_LOCK_WAIT,
};
use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const USER: &str = "synthetic@example.test";
const PASSWORD: &str = "synthetic-app-password";

#[derive(Clone)]
struct Dav {
    handler: DavHandler,
    gate: Arc<tokio::sync::Mutex<()>>,
    authorization: String,
    /// 服务端暂时不可用：每个请求答 503，不带 `Retry-After`。
    down: Arc<AtomicBool>,
    requests: Arc<AtomicUsize>,
    /// 每个请求的方法和路径，按到达顺序。
    log: Arc<Mutex<Vec<String>>>,
    rate_limited: Arc<AtomicBool>,
    dictionary_retries: Arc<AtomicUsize>,
    registration_blocked: Arc<AtomicBool>,
}

async fn handle(State(dav): State<Dav>, request: Request) -> Response {
    dav.requests.fetch_add(1, Ordering::SeqCst);
    dav.log
        .lock()
        .unwrap()
        .push(format!("{} {}", request.method(), request.uri().path()));
    if dav.registration_blocked.load(Ordering::SeqCst)
        && request.method() == "MOVE"
        && request
            .headers()
            .get("destination")
            .is_some_and(|destination| {
                destination
                    .to_str()
                    .is_ok_and(|path| path.contains("/keyring/") && path.contains("/e-"))
            })
    {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    if dav.rate_limited.load(Ordering::SeqCst) {
        return (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, "0")]).into_response();
    }
    if request.uri().path().contains("/msime-dictionary/v1/")
        && dav
            .dictionary_retries
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            })
            .is_ok()
    {
        return (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, "0")]).into_response();
    }
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
        requests: Arc::default(),
        log: Arc::default(),
        rate_limited: Arc::default(),
        dictionary_retries: Arc::default(),
        registration_blocked: Arc::default(),
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

    /// 客户端发来过多少个请求。
    fn requests(&self) -> usize {
        self.dav.requests.load(Ordering::SeqCst)
    }

    /// 到现在为止的请求，一行一个，断言失败时打印出来。
    fn log(&self) -> String {
        self.dav.log.lock().unwrap().join("\n")
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

fn expire_request_window(directory: &Path) {
    let path = directory.join(STATE_FILE);
    let mut stored: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    // 多轮收敛与回退恢复用例模拟阶段之间额度窗口已过期；硬限额由专门回归覆盖。
    for list in ["requests", "automatic_requests"] {
        if let Some(requests) = stored["state"]["schedule"][list].as_array_mut() {
            for request in requests {
                *request = serde_json::json!(0);
            }
        }
    }
    std::fs::write(&path, serde_json::to_vec(&stored).unwrap()).unwrap();
}

#[test]
fn failed_registration_keeps_the_internal_identity_without_publishing_it() {
    for cached in [false, true] {
        let server = start();
        let directory = tempfile::tempdir().unwrap();
        let sync = device(&server, directory.path(), "Synthetic desktop");
        let mut words = Words::default();
        server.dav.rate_limited.store(!cached, Ordering::SeqCst);
        server
            .dav
            .registration_blocked
            .store(cached, Ordering::SeqCst);
        let failed = manual(&sync, &mut words);
        assert!(failed.report.is_err());
        assert!(failed.view.device_id.is_none());
        assert!(directory
            .path()
            .join("cloud-clipboard-relay-keystore")
            .is_file());
        assert_eq!(
            directory
                .path()
                .join("cloud-clipboard-webdav-state")
                .exists(),
            cached
        );
        let status = sync.status().unwrap();
        assert!(status.device_id.is_none(), "未登记的内部身份不能开放给宿主");
        assert!(!status.approved);
        let offline = sync
            .run(&mut words, Trigger::LocalChange, Options::default())
            .unwrap();
        assert!(!offline.network);
        assert!(offline.view.device_id.is_none());
        assert_eq!(offline.view.last_error, status.last_error);
        server.dav.rate_limited.store(false, Ordering::SeqCst);
        server
            .dav
            .registration_blocked
            .store(false, Ordering::SeqCst);
        let registered = manual(&sync, &mut words);
        assert!(registered.report.is_ok(), "{:?}", registered.report);
        assert!(registered.view.device_id.is_some());
        assert_eq!(sync.status().unwrap().device_id, registered.view.device_id);
    }
}

#[test]
fn http_registration_failures_keep_the_budget_without_a_previous_identity() {
    for previous_identity in [false, true] {
        let server = start();
        let directory = tempfile::tempdir().unwrap();
        let sync = device(&server, directory.path(), "Synthetic desktop");
        let mut words = Words::default();
        let mut used = 0;
        if previous_identity {
            assert!(manual(&sync, &mut words).report.is_ok());
            used = server.dav.requests.load(Ordering::SeqCst);
            std::fs::remove_file(directory.path().join("cloud-clipboard-relay-keystore")).unwrap();
            std::fs::remove_file(directory.path().join("cloud-clipboard-webdav-state")).unwrap();
        }
        server.dav.rate_limited.store(true, Ordering::SeqCst);
        for _ in 0..18 {
            let run = manual(&sync, &mut words);
            assert_eq!(
                run.report.unwrap_err().code(),
                "dictionary_sync_rate_limited"
            );
        }
        let actual = server.dav.requests.load(Ordering::SeqCst);
        assert_eq!(
            actual, 60,
            "首次注册失败或身份丢失都不能重新领取滚动窗口额度（先前已用 {used}）"
        );
        let stored: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.path().join(STATE_FILE)).unwrap())
                .unwrap();
        assert_eq!(
            stored["state"]["schedule"]["requests"]
                .as_array()
                .unwrap()
                .len(),
            actual
        );
    }
}

#[test]
fn http_retries_cannot_exceed_the_remaining_automatic_or_manual_budget() {
    for (trigger, limit) in [(Trigger::Opened, 40), (Trigger::Manual, 60)] {
        let server = start();
        let directory = tempfile::tempdir().unwrap();
        let sync = device(&server, directory.path(), "Synthetic desktop");
        let mut words = Words::default();
        assert!(manual(&sync, &mut words).report.is_ok());
        let path = directory.path().join(STATE_FILE);
        let mut stored: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        stored["state"]["schedule"]["requests"] = serde_json::json!(vec![now; limit - 3]);
        if trigger != Trigger::Manual {
            // 自动同步的硬限额看的是自动同步自己发过的请求。
            stored["state"]["schedule"]["automatic_requests"] =
                serde_json::json!(vec![now; limit - 3]);
        }
        stored["state"]["schedule"]["last_pull_ms"] = serde_json::json!(0);
        std::fs::write(&path, serde_json::to_vec(&stored).unwrap()).unwrap();
        settle();
        server.dav.requests.store(0, Ordering::SeqCst);
        server.dav.rate_limited.store(true, Ordering::SeqCst);
        let run = sync.run(&mut words, trigger, Options::default()).unwrap();
        assert_eq!(
            run.report.unwrap_err().code(),
            "dictionary_sync_rate_limited"
        );
        assert_eq!(
            server.dav.requests.load(Ordering::SeqCst),
            3,
            "HTTP 重试也必须在发出之前扣额度",
        );
        let stored: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            stored["state"]["schedule"]["requests"]
                .as_array()
                .unwrap()
                .len(),
            limit
        );
    }
}

#[test]
fn http_compaction_cleanup_is_charged_before_the_final_state_save() {
    let server = start();
    let directory = tempfile::tempdir().unwrap();
    let sync = device(&server, directory.path(), "Synthetic desktop");
    let mut words = Words(BTreeSet::from([word("ni'hao", "合成词条")]));
    assert!(manual(&sync, &mut words).report.is_ok());
    let path = directory.path().join(STATE_FILE);
    let mut stored: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    // 只把本机压缩计数置到阈值，远端事件和摘要仍由真实同步生成。
    stored["state"]["ledger"]["events"] = serde_json::json!(64);
    stored["state"]["schedule"]["requests"] = serde_json::json!([]);
    stored["state"]["schedule"]["automatic_requests"] = serde_json::json!([]);
    std::fs::write(&path, serde_json::to_vec(&stored).unwrap()).unwrap();
    let stale = "msime-dictionary/v1/tmp/1-00000000000000000000000000000000.json";
    assert_eq!(server.raw("PUT", stale, None, b"synthetic".to_vec()).0, 201);
    server.dav.requests.store(0, Ordering::SeqCst);
    let run = manual(&sync, &mut words);
    assert!(run.report.unwrap().compacted);
    assert_eq!(server.raw("GET", stale, None, Vec::new()).0, 404);
    let stored: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        stored["state"]["schedule"]["requests"]
            .as_array()
            .unwrap()
            .len(),
        server.dav.requests.load(Ordering::SeqCst),
        "清理的列目录和删除也要进入同一份额度记录",
    );
}

#[test]
fn http_requests_are_all_charged_to_the_sync_budget() {
    for retries in [0, 2] {
        let server = start();
        server
            .dav
            .dictionary_retries
            .store(retries, Ordering::SeqCst);
        let directory = tempfile::tempdir().unwrap();
        let sync = device(&server, directory.path(), "Synthetic desktop");
        let mut words = Words(BTreeSet::from([word("ni'hao", "合成词条")]));
        let run = manual(&sync, &mut words);
        assert!(run.report.is_ok(), "{:?}", run.report);
        let stored: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.path().join(STATE_FILE)).unwrap())
                .unwrap();
        assert_eq!(
            stored["state"]["schedule"]["requests"]
                .as_array()
                .unwrap()
                .len(),
            server.dav.requests.load(Ordering::SeqCst),
            "实际 HTTP 请求都必须计入滚动窗口，包括临时目录创建和词库重试",
        );
        assert_eq!(server.dav.dictionary_retries.load(Ordering::SeqCst), 0);
    }
}

/// 调度记着的时间都往前挪 `by` 毫秒，相当于过了这么久；请求记录不动，配置时的请求仍在同一个额度窗口里。
fn elapse_schedule(directory: &Path, by: u64) {
    let path = directory.join(STATE_FILE);
    let mut stored: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let schedule = &mut stored["state"]["schedule"];
    for field in [
        "last_change_ms",
        "dirty_since_ms",
        "last_push_ms",
        "last_pull_ms",
        "last_success_ms",
        "retry_at_ms",
    ] {
        if let Some(at) = schedule[field].as_u64().filter(|at| *at > 0) {
            schedule[field] = serde_json::json!(at.saturating_sub(by));
        }
    }
    std::fs::write(&path, serde_json::to_vec(&stored).unwrap()).unwrap();
}

/// 加了一个词之后：`local_change` 马上记下、不联网；过了五分钟的发送间隔，定时到点时发出去；再过一分钟，回到 App 时的 `opened` 照常拉取。全程仍在配置的那个额度窗口里。
fn next_change_goes_out_automatically(
    server: &Server,
    directory: &Path,
    sync: &DictionarySync,
    words: &mut Words,
    what: &str,
) {
    words.0.insert(word("he'cheng", "合成新词"));
    let recorded = sync
        .run(words, Trigger::LocalChange, Options::default())
        .unwrap();
    assert!(!recorded.network, "{what}");
    assert_eq!(recorded.view.pending_upload, 1, "{what}");
    assert_eq!(recorded.view.budget_wait_until, None, "{what}");
    elapse_schedule(directory, MIN_PUSH_INTERVAL_MS);
    let before = server.requests();
    let sent = sync
        .run(words, Trigger::Periodic, Options::default())
        .unwrap();
    assert!(
        sent.network,
        "{what}: automatic sync held back for {} ms",
        sent.next_check_ms
    );
    assert_eq!(sent.report.unwrap().published_ops, 1, "{what}");
    assert_eq!(sent.view.pending_upload, 0, "{what}");
    eprintln!(
        "{what}: the automatic send took {} requests",
        server.requests() - before
    );
    elapse_schedule(directory, OPEN_PULL_MIN_MS);
    let opened = sync
        .run(words, Trigger::Opened, Options::default())
        .unwrap();
    assert!(opened.network, "{what}: opened held back");
    opened.report.unwrap();
}

/// 第一台设备配置时最多这么多个请求：密钥串 23 个（建布局、登记、生成密钥），词库 12 个（建布局、写格式文件和第一份检查点、发出本机的词）。
const FIRST_DEVICE_SETUP_REQUESTS: usize = 35;

/// 坚果云真机（2026-10-28，iOS）：第一台设备保存配置后的第一次同步用掉了全部 40 个自动额度，之后半小时里改词、定时和回到 App 都不联网，「上次同步」停在配置时。配置时的请求属于用户的操作，不能挤掉之后的自动同步。iOS 保存后以 `manual` 跑第一次，共享设置页和 Win7 以 `opened`，两种都要覆盖。
#[test]
fn first_device_setup_leaves_the_automatic_budget_for_the_next_change() {
    for setup in [Trigger::Manual, Trigger::Opened] {
        let server = start();
        let directory = tempfile::tempdir().unwrap();
        let sync = device(&server, directory.path(), "Synthetic phone");
        let mut words = Words(BTreeSet::from([word("ni'hao", "合成词条")]));
        let first = sync.run(&mut words, setup, Options::default()).unwrap();
        assert!(first.network, "{setup:?}");
        first.report.unwrap();
        assert!(first.view.approved && first.view.joined, "{:?}", first.view);
        assert_eq!(first.view.pending_upload, 0, "{:?}", first.view);
        assert_eq!(first.view.budget_wait_until, None);
        assert!(
            server.requests() <= FIRST_DEVICE_SETUP_REQUESTS,
            "{setup:?}: first-device setup sent {} requests:\n{}",
            server.requests(),
            server.log()
        );
        eprintln!(
            "{setup:?}: first-device setup sent {} requests",
            server.requests()
        );
        next_change_goes_out_automatically(
            &server,
            directory.path(),
            &sync,
            &mut words,
            &format!("first device, setup by {setup:?}"),
        );
    }
}

/// 第二台设备登记、获准之后加入：加入时读检查点、日志并发出本机的词，这些同样不占之后的自动额度。
#[test]
fn second_device_join_leaves_the_automatic_budget_for_the_next_change() {
    let server = start();
    let (a_dir, b_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut a = Words(BTreeSet::from([
        word("shui'shan", "水杉"),
        word("ni'hao", "你好"),
    ]));
    let mut b = Words(BTreeSet::from([word("zai'jian", "再见")]));
    let first = device(&server, a_dir.path(), "MacBook");
    let second = device(&server, b_dir.path(), "Pixel");
    manual(&first, &mut a).report.unwrap();
    let before = server.requests();
    let registered = second
        .run(&mut b, Trigger::Opened, Options::default())
        .unwrap();
    assert!(!registered.view.approved);
    let registration = server.requests() - before;
    settle();
    let pending = first
        .devices()
        .unwrap()
        .devices
        .into_iter()
        .find(|device| !device.approved)
        .unwrap();
    first.approve(&pending.device_id).unwrap();
    settle();
    elapse_schedule(b_dir.path(), OPEN_PULL_MIN_MS);
    let before = server.requests();
    let joined = second
        .run(&mut b, Trigger::Opened, Options::default())
        .unwrap();
    assert!(joined.network);
    joined.report.unwrap();
    assert!(
        joined.view.approved && joined.view.joined,
        "{:?}",
        joined.view
    );
    assert_eq!(b.0.len(), 3);
    // 加入的那一次就把本机的词发了出去，不等下一轮。
    assert_eq!(joined.view.pending_upload, 0, "{:?}", joined.view);
    eprintln!(
        "second device: registration {registration} requests, join {} requests",
        server.requests() - before
    );
    next_change_goes_out_automatically(&server, b_dir.path(), &second, &mut b, "second device");
}

/// 自动额度用完时，自动同步不联网，状态里说什么时候继续（`budget_wait_until`），而不是看上去卡住了；「立即同步」照常。
#[test]
fn an_exhausted_automatic_budget_is_reported_with_the_time_sync_resumes() {
    let server = start();
    let directory = tempfile::tempdir().unwrap();
    let sync = device(&server, directory.path(), "Synthetic phone");
    let mut words = Words(BTreeSet::from([word("ni'hao", "合成词条")]));
    manual(&sync, &mut words).report.unwrap();
    let path = directory.path().join(STATE_FILE);
    let mut stored: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let schedule = &mut stored["state"]["schedule"];
    // 这个窗口里自动同步已经发了 40 个请求，总额度还剩 20 个给「立即同步」。
    schedule["automatic_requests"] = serde_json::json!(vec![now; 40]);
    schedule["requests"] = serde_json::json!(vec![now; 40]);
    std::fs::write(&path, serde_json::to_vec(&stored).unwrap()).unwrap();
    elapse_schedule(directory.path(), OPEN_PULL_MIN_MS);
    let before = server.requests();
    let held = sync
        .run(&mut words, Trigger::Opened, Options::default())
        .unwrap();
    assert!(!held.network);
    assert_eq!(server.requests(), before);
    let resumes = held.view.budget_wait_until.expect("the wait is reported");
    let window = 30 * 60 * 1000;
    assert!(
        (now + window..now + window + 60_000).contains(&resumes),
        "{resumes} vs {now}"
    );
    assert_eq!(sync.status().unwrap().budget_wait_until, Some(resumes));
    let sent = manual(&sync, &mut words);
    assert!(sent.network);
    sent.report.unwrap();
    assert_eq!(sent.view.budget_wait_until, Some(resumes));
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
        expire_request_window(a_dir.path());
        expire_request_window(b_dir.path());
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
        expire_request_window(a_dir.path());
        expire_request_window(b_dir.path());
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
    let state_path = directory.path().join(STATE_FILE);
    let before: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
    let mut confirmed = action;
    confirmed["trust_remote"] = serde_json::json!(true);
    assert_eq!(host_probe(directory.path(), confirmed)["ok"], true);
    let after: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
    assert_eq!(
        after["state"]["schedule"]["requests"], before["state"]["schedule"]["requests"],
        "重新信任网盘不能返还已经使用的额度",
    );
    expire_request_window(directory.path());
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

/// 一个安装里的「我的词条」：设置应用和输入法两个进程读写的是同一份 Engine 日志库。`writes` 是文件戳的替身，每写一次加一。
#[derive(Clone, Default)]
struct SharedWords(Arc<Mutex<(BTreeSet<WordKey>, u64)>>);

impl SharedWords {
    fn insert(&self, key: WordKey) {
        let mut shared = self.0.lock().unwrap();
        shared.0.insert(key);
        shared.1 += 1;
    }

    fn snapshot(&self) -> BTreeSet<WordKey> {
        self.0.lock().unwrap().0.clone()
    }

    fn writes(&self) -> u64 {
        self.0.lock().unwrap().1
    }
}

impl LocalDictionary for SharedWords {
    fn words(&mut self) -> Result<BTreeSet<WordKey>, LocalError> {
        Ok(self.snapshot())
    }

    fn apply(&mut self, changes: &[LocalChange]) -> Result<Vec<LocalOutcome>, LocalError> {
        let mut shared = self.0.lock().unwrap();
        for change in changes {
            if change.add {
                shared.0.insert(change.key.clone());
            } else {
                shared.0.remove(&change.key);
            }
        }
        shared.1 += 1;
        Ok(vec![LocalOutcome::Applied; changes.len()])
    }
}

const IDENTITY_MISSING: &str = "clipboard_identity_missing";

#[test]
fn the_input_method_borrows_the_settings_apps_identity_and_never_registers_its_own() {
    let server = start();
    let directory = tempfile::tempdir().unwrap();
    let local = SharedWords::default();
    local.insert(word("shui'shan", "水杉"));
    let ime = device(&server, directory.path(), "MacBook").existing_identity_only();
    // 设置应用还没配置过：输入法什么触发都不联网、不生成身份、不记录本机改动。
    for trigger in [
        Trigger::Periodic,
        Trigger::LocalChange,
        Trigger::Opened,
        Trigger::Manual,
    ] {
        let run = ime
            .run(&mut local.clone(), trigger, Options::default())
            .unwrap();
        assert!(!run.network, "{trigger:?}");
        assert_eq!(run.report.unwrap_err().code(), IDENTITY_MISSING);
    }
    assert_eq!(ime.devices().unwrap_err().code(), IDENTITY_MISSING);
    assert_eq!(
        ime.reset(&mut local.clone()).unwrap_err().code(),
        IDENTITY_MISSING
    );
    assert_eq!(server.requests(), 0, "the input method sent requests");
    assert!(!directory
        .path()
        .join("cloud-clipboard-relay-keystore")
        .exists());
    assert!(!directory.path().join(STATE_FILE).exists());
    assert_eq!(ime.status().unwrap().device_id, None);
    // 设置应用第一次配置：生成身份、加入。之后输入法用的是同一个身份。
    let settings = device(&server, directory.path(), "MacBook");
    let run = settings
        .run(&mut local.clone(), Trigger::Opened, Options::default())
        .unwrap();
    run.report.unwrap();
    assert!(run.view.approved);
    let id = run.view.device_id.unwrap();
    local.insert(word("ni'hao", "你好"));
    let run = ime
        .run(&mut local.clone(), Trigger::Manual, Options::default())
        .unwrap();
    let report = run.report.unwrap();
    assert_eq!(report.published_ops, 1, "{report:?}");
    assert_eq!(run.view.device_id.as_deref(), Some(id.as_str()));
    settle();
    let devices = settings.devices().unwrap();
    assert_eq!(devices.devices.len(), 1, "{devices:?}");
    assert_eq!(devices.devices[0].device_id, id);
}

/// 一台已经配好的设备：设置应用生成身份并加入，返回它的同步。
fn configured(server: &Server, directory: &Path, local: &SharedWords) -> DictionarySync {
    let settings = device(server, directory, "MacBook");
    settings
        .run(&mut local.clone(), Trigger::Opened, Options::default())
        .unwrap()
        .report
        .unwrap();
    settings
}

#[test]
fn the_input_method_and_the_settings_app_sync_concurrently_from_one_directory() {
    let server = start();
    let (directory, other) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let local = SharedWords::default();
    local.insert(word("shui'shan", "水杉"));
    let settings = configured(&server, directory.path(), &local);
    // 两个进程同时同步，同时有人改词：整次同步在目录里的锁下串行，谁也不会读到对方写了一半的状态。输入法这边用 manual 只是为了让每一次都真的联网。
    let racers: Vec<_> = ["sheding", "shurufa"]
        .into_iter()
        .map(|process| {
            let (url, path, local) = (server.url(), directory.path().to_path_buf(), local.clone());
            std::thread::spawn(move || {
                let sync = DictionarySync::new(&url, USER, PASSWORD, &path, "MacBook").unwrap();
                let ime = process == "shurufa";
                let sync = if ime {
                    sync.existing_identity_only()
                } else {
                    sync
                };
                for (round, letter) in ["a", "b"].into_iter().enumerate() {
                    local.insert(word(&format!("{process}'{letter}"), "合成词"));
                    let trigger = match (ime, round) {
                        (true, 0) => Trigger::LocalChange,
                        _ => Trigger::Manual,
                    };
                    sync.run(&mut local.clone(), trigger, Options::default())
                        .unwrap()
                        .report
                        .unwrap();
                }
            })
        })
        .collect();
    for racer in racers {
        racer.join().unwrap();
    }
    let run = settings
        .run(&mut local.clone(), Trigger::Manual, Options::default())
        .unwrap();
    run.report.unwrap();
    assert_eq!(run.view.pending_upload, 0);
    // 另一台设备加入后拿到两个进程记下的全部词，这台设备在密钥串里只有一条。
    let mut pixel = Words::default();
    let second = device(&server, other.path(), "Pixel");
    manual(&second, &mut pixel);
    settle();
    let devices = settings.devices().unwrap();
    assert_eq!(devices.devices.len(), 2, "{devices:?}");
    let pending = devices
        .devices
        .iter()
        .find(|device| !device.approved)
        .unwrap();
    settings.approve(&pending.device_id).unwrap();
    settle();
    manual(&second, &mut pixel).report.unwrap();
    assert_eq!(pixel.0, local.snapshot());
    assert_eq!(pixel.0.len(), 5);
}

/// 后台调度每一次同步的触发方式和错误码（成功时为 `None`）。
type RunLog = Arc<Mutex<Vec<(Trigger, Option<String>)>>>;

/// 后台调度的宿主：同步是只用已有身份的那一份，本机词库是共享的那一份，每一次同步的结果都记下来。
struct ImeHost {
    sync: DictionarySync,
    local: SharedWords,
    log: RunLog,
}

impl SchedulerHost for ImeHost {
    fn run(&mut self, trigger: Trigger) -> Option<Round> {
        let mut local = self.local.clone();
        let mut watched = Watched::new(&mut local);
        let result = self.sync.run(&mut watched, trigger, Options::default());
        let seen = watched.seen();
        let error = match &result {
            Ok(run) => run
                .report
                .as_ref()
                .err()
                .map(|error| error.code().to_owned()),
            Err(error) => Some(error.code().to_owned()),
        };
        self.log.lock().unwrap().push((trigger, error));
        Some(Round { result, seen })
    }

    fn fingerprint(&mut self) -> Option<u64> {
        Some(fingerprint(&self.local.snapshot()))
    }

    fn stamp(&mut self) -> Option<u64> {
        Some(self.local.writes())
    }
}

fn wait_until(what: &str, condition: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !condition() {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting until {what}"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn the_background_scheduler_waits_for_the_settings_app_and_then_records_new_words() {
    let server = start();
    let directory = tempfile::tempdir().unwrap();
    let local = SharedWords::default();
    local.insert(word("shui'shan", "水杉"));
    let log = Arc::new(Mutex::new(Vec::new()));
    let host = ImeHost {
        sync: device(&server, directory.path(), "MacBook")
            .existing_identity_only()
            .lock_wait(BACKGROUND_LOCK_WAIT),
        local: local.clone(),
        log: Arc::clone(&log),
    };
    let timing = Timing {
        poll: std::time::Duration::from_millis(20),
        max_delay: std::time::Duration::from_millis(200),
        idle: std::time::Duration::from_millis(50),
        retry: std::time::Duration::from_millis(50),
    };
    let scheduler = Scheduler::start_with(host, timing).unwrap();
    // 还没有身份：只是定时再问，一个请求也不发，改词也不去读。
    wait_until("it asked twice", || log.lock().unwrap().len() >= 2);
    local.insert(word("zao'ci", "造词"));
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(server.requests(), 0);
    assert!(log
        .lock()
        .unwrap()
        .iter()
        .all(|(trigger, error)| *trigger == Trigger::Periodic
            && error.as_deref() == Some(IDENTITY_MISSING)));
    // 设置应用配好之后，下一次询问就用上它的身份，从那时的词开始看。
    let settings = configured(&server, directory.path(), &local);
    wait_until("the scheduler ran with the identity", || {
        log.lock()
            .unwrap()
            .last()
            .is_some_and(|(_, error)| error.is_none())
    });
    assert_eq!(settings.status().unwrap().pending_upload, 0);
    // 打字时造了一个词：调一次 local_change，词记进待发送（发送要等防抖）。等同步整个跑完再看：核对一记下就写盘，那时同步还没返回。
    local.insert(word("xin'ci", "新词"));
    wait_until("local_change ran", || {
        log.lock()
            .unwrap()
            .iter()
            .any(|(trigger, error)| *trigger == Trigger::LocalChange && error.is_none())
    });
    assert!(scheduler.stop(None));
    let runs = log.lock().unwrap().clone();
    assert_eq!(
        runs.iter()
            .filter(|(trigger, _)| *trigger == Trigger::LocalChange)
            .count(),
        1,
        "{runs:?}"
    );
    assert_eq!(settings.status().unwrap().pending_upload, 1);
    let devices = settings.devices().unwrap();
    assert_eq!(devices.devices.len(), 1, "{devices:?}");
}
