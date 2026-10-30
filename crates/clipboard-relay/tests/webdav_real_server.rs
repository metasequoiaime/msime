//! 对真实网盘（例如坚果云）跑云剪贴板和词库同步的端到端流程，由维护者手动开启。
//!
//! `MSIME_TEST_WEBDAV_URL`、`MSIME_TEST_WEBDAV_USERNAME`、`MSIME_TEST_WEBDAV_PASSWORD` 三个环境变量都设置了才联网；都没设置时跳过（测试照常通过，CI 里就是这样），只设置了一部分时报错，免得以为跑过了。做法见 `docs/cloud-clipboard-webdav.md` 和 `docs/dictionary-webdav-sync.md` 的「对真实网盘跑端到端测试」。
//!
//! 每次在给定地址下的固定目录 `msime-e2e/` 里新建一个 `run-<毫秒>-<随机数>/`，两个流程都以它为基地址，跑完删掉它。坚果云不许删根目录下建出来的文件夹，所以根目录下只会出现 `msime-e2e/` 这一个，它会一直留着。
//!
//! 坚果云免费账号每 30 分钟 600 个请求，与其他第三方应用共用，所以这里只挑有代表性的步骤，并在最后打印实际发出的 HTTP 请求数（经产品同一条 WebDAV 通道计数）。同一套流程也对着本机的 `dav-server` 跑，CI 据此守着流程本身和请求数的上限。
//!
//! 应用密码和 `Authorization` 头不出现在任何输出里，账号也隐去：输出和 panic 信息都经 [`redact`] 过滤。

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Router;
use base64::Engine as _;
use dav_server::{fakels::FakeLs, memfs::MemFs, DavHandler};
use msime_client_core::cloud::dictionary_sync::{
    LocalChange, LocalDictionary, LocalError, LocalOutcome, Options, Trigger, WordKey, WordKind,
};
use msime_client_services::account::{AccountClipboardE2eStatus, AccountClipboardPage};
use msime_client_services::clipboard_relay::{RelayClipboard, RelayConfig, RelayError};
use msime_client_services::clipboard_webdav::{
    self,
    test_support::{requests_sent, RawDav},
};
use msime_client_services::dictionary_webdav::{
    self, DictionarySync, DictionarySyncError, RunResult,
};
use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const URL_VAR: &str = "MSIME_TEST_WEBDAV_URL";
const USERNAME_VAR: &str = "MSIME_TEST_WEBDAV_USERNAME";
const PASSWORD_VAR: &str = "MSIME_TEST_WEBDAV_PASSWORD";
/// 每次运行的目录都建在这下面；根目录下不建别的东西。
const PARENT: &str = "msime-e2e/";
/// 一次运行最多发这么多请求，是坚果云 30 分钟额度（600）的四成；本机 `dav-server` 上现在约 180 个。同一套流程在本机超过它就失败，流程变重时先在 CI 里红，而不是在维护者的网盘额度上红。
const REQUEST_CEILING: u64 = 240;
const MASK: &str = "***";

// ---- 凭据与隐去 ----

/// 把 `text` 里出现的每个秘密（原样或经百分号编码，ASCII 不分大小写）换成 `***`。长的先换，一个秘密包含另一个时也不会漏出片段。
fn redact(text: &str, secrets: &[&str]) -> String {
    let mut needles: Vec<String> = Vec::new();
    for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
        for form in [
            secret.to_string(),
            percent_encoded(secret, false),
            percent_encoded(secret, true),
        ] {
            let form = form.to_ascii_lowercase();
            if !needles.contains(&form) {
                needles.push(form);
            }
        }
    }
    needles.sort_by_key(|needle| std::cmp::Reverse(needle.len()));
    let mut out = text.to_owned();
    for needle in needles {
        let mut result = String::with_capacity(out.len());
        let lower = out.to_ascii_lowercase();
        let mut rest = 0;
        // ASCII 小写不改变字节位置，在小写副本里找到的区间就是原文里的区间。
        while let Some(found) = lower[rest..].find(&needle) {
            let start = rest + found;
            result.push_str(&out[rest..start]);
            result.push_str(MASK);
            rest = start + needle.len();
        }
        result.push_str(&out[rest..]);
        out = result;
    }
    out
}

/// 秘密出现在地址里时的样子（例如 Nextcloud 路径里的账号 `a%40b.c`）。
fn percent_encoded(secret: &str, lower_hex: bool) -> String {
    secret
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                char::from(byte).to_string()
            } else if lower_hex {
                format!("%{byte:02x}")
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

/// 要跑的网盘。`Debug` 只给出隐去账号后的地址。
struct Target {
    url: String,
    username: String,
    password: String,
}

impl std::fmt::Debug for Target {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Target")
            .field("url", &self.redact(&self.url))
            .field("username", &MASK)
            .field("password", &MASK)
            .finish()
    }
}

impl Target {
    /// 三个环境变量都有时是 `Some`，都没有时是 `None`；只有一部分时报出缺哪几个（只报变量名）。
    fn from_env() -> Result<Option<Self>, String> {
        let read = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        let (url, username, password) = (read(URL_VAR), read(USERNAME_VAR), read(PASSWORD_VAR));
        match (url, username, password) {
            (Some(url), Some(username), Some(password)) => Ok(Some(Self {
                url,
                username,
                password,
            })),
            (None, None, None) => Ok(None),
            (url, username, password) => {
                let missing: Vec<&str> = [
                    (URL_VAR, url.is_none()),
                    (USERNAME_VAR, username.is_none()),
                    (PASSWORD_VAR, password.is_none()),
                ]
                .into_iter()
                .filter_map(|(name, missing)| missing.then_some(name))
                .collect();
                Err(format!(
                    "只设置了一部分对真实网盘测试的环境变量，还缺 {}",
                    missing.join("、")
                ))
            }
        }
    }

    /// 隐去应用密码、账号和它们拼成的 Basic 鉴权值。
    fn redact(&self, text: &str) -> String {
        redact(text, &self.secrets().each_ref().map(String::as_str))
    }

    fn secrets(&self) -> [String; 3] {
        let basic = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", self.username, self.password));
        [self.password.clone(), self.username.clone(), basic]
    }

    fn root(&self) -> Result<RelayConfig, RelayError> {
        RelayConfig::webdav(&self.url, &self.username, &self.password)
    }
}

// ---- 每一步的错误 ----

/// 本次运行碰到了限流：之后的流程不再开始。
static RATE_LIMITED: AtomicBool = AtomicBool::new(false);

trait Coded {
    fn error_code(&self) -> String;
}

impl Coded for RelayError {
    fn error_code(&self) -> String {
        self.code().to_owned()
    }
}

impl Coded for DictionarySyncError {
    fn error_code(&self) -> String {
        self.code().to_owned()
    }
}

fn is_rate_limited(code: &str) -> bool {
    code == "webdav_rate_limited" || code == "dictionary_sync_rate_limited"
}

/// 一步的结果：失败时以步骤名和错误码 panic（错误码里没有凭据）；限流时记下来并说明为什么停下。
#[track_caller]
fn ok<T, E: Coded>(step: &str, result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => fail(step, &error.error_code()),
    }
}

#[track_caller]
fn fail(step: &str, code: &str) -> ! {
    if is_rate_limited(code) {
        RATE_LIMITED.store(true, Ordering::SeqCst);
        panic!(
            "{step}：被限流（{code}）。网盘答了 429 或 503 且按 Retry-After 退避三次后仍然如此，或者本机词库同步的额度（每台设备 30 分钟手动 60 个请求）用完了。坚果云免费账号每 30 分钟 600 个请求、与其他应用共用；本次不再发请求，过半小时再跑"
        );
    }
    panic!("{step}：{code}");
}

/// 本机缓存在两秒内直接复用；两台设备交替操作时等一下，让对方的写入被读到。
fn settle() {
    std::thread::sleep(std::time::Duration::from_millis(2100));
}

// ---- 云剪贴板 ----

fn texts(page: &AccountClipboardPage) -> Vec<&str> {
    page.items.iter().map(|item| item.text.as_str()).collect()
}

/// 测试连接、第一台设备建钥、第二台登记并被批准、一台上传另一台读到、置顶、删除。
fn clipboard_flow(base: &str, target: &Target, root: &Path) {
    let config = || {
        ok(
            "云剪贴板配置",
            RelayConfig::webdav(base, &target.username, &target.password),
        )
    };
    ok("云剪贴板测试连接", clipboard_webdav::probe(&config()));
    let laptop = ok(
        "建第一台设备",
        RelayClipboard::new(config(), root.join("laptop"), "合成笔记本"),
    );
    let page = ok("第一台设备登记", laptop.list(""));
    assert_eq!(
        page.e2e.status,
        AccountClipboardE2eStatus::Ready,
        "第一台设备建钥"
    );
    let phone = ok(
        "建第二台设备",
        RelayClipboard::new(config(), root.join("phone"), "合成手机"),
    );
    let pending = ok("第二台设备登记", phone.list(""));
    assert_eq!(
        pending.e2e.status,
        AccountClipboardE2eStatus::Pending,
        "第二台设备等待批准"
    );
    settle();
    let entry = ok("第一台设备列出设备", laptop.devices())
        .devices
        .into_iter()
        .find(|entry| !entry.current)
        .expect("第二台设备出现在第一台的设备列表里");
    assert!(!entry.approved);
    assert_eq!(entry.name, "合成手机");
    assert_eq!(entry.code, pending.e2e.code, "两边的核对码相同");
    ok("批准第二台设备", laptop.approve(&entry.device_id));
    settle();
    let page = ok("第二台设备获准后读取", phone.list(""));
    assert_eq!(
        page.e2e.status,
        AccountClipboardE2eStatus::Ready,
        "第二台设备获准"
    );

    ok("第一台设备上传", laptop.add("合成的端到端条目"));
    settle();
    let page = ok("第二台设备读取", phone.list(""));
    assert_eq!(texts(&page), ["合成的端到端条目"], "第二台设备读到上传");
    let id = page.items[0].id.clone();
    ok("第二台设备置顶", phone.set_pinned(&id, true));
    settle();
    let page = ok("第一台设备读取置顶", laptop.list(""));
    assert_eq!(page.items.len(), 1);
    assert!(
        page.items[0].id == id && page.items[0].pinned,
        "第一台设备看到置顶"
    );
    ok("第一台设备删除", laptop.delete(&id));
    settle();
    let page = ok("第二台设备读取删除", phone.list(""));
    assert!(page.items.is_empty(), "第二台设备看到删除");
}

// ---- 词库同步 ----

/// 内存里的「我的词条」。
#[derive(Default)]
struct Words(BTreeSet<WordKey>);

impl LocalDictionary for Words {
    fn words(&mut self) -> Result<BTreeSet<WordKey>, LocalError> {
        Ok(self.0.clone())
    }

    fn apply(&mut self, changes: &[LocalChange]) -> Result<Vec<LocalOutcome>, LocalError> {
        for change in changes {
            if change.add {
                self.0.insert(change.key.clone());
            } else {
                self.0.remove(&change.key);
            }
        }
        Ok(vec![LocalOutcome::Applied; changes.len()])
    }
}

fn word(code: &str, text: &str) -> WordKey {
    WordKey::new(WordKind::Pinyin, code, text).unwrap()
}

/// 手动同步一次；同步本身的错误只在限流时算失败（等待批准的设备答 `clipboard_device_pending` 是预期的）。
fn manual(step: &str, sync: &DictionarySync, words: &mut Words) -> RunResult {
    let run = ok(step, sync.run(words, Trigger::Manual, Options::default()));
    if let Err(error) = &run.report {
        if is_rate_limited(error.code()) {
            fail(step, error.code());
        }
    }
    run
}

/// 手动同步一次，必须成功。
fn synced(step: &str, sync: &DictionarySync, words: &mut Words) -> RunResult {
    let run = manual(step, sync, words);
    if let Err(error) = &run.report {
        fail(step, error.code());
    }
    run
}

/// 测试连接、设备甲加词、设备乙登记并获准后收到这些词、乙删一个词甲也删掉、甲乙同时发布一次。
fn dictionary_flow(base: &str, target: &Target, root: &Path) {
    let (username, password) = (target.username.as_str(), target.password.as_str());
    ok(
        "词库测试连接",
        dictionary_webdav::probe(base, username, password, None),
    );
    let first = ok(
        "建设备甲",
        DictionarySync::new(base, username, password, root.join("a"), "合成设备甲"),
    );
    let second = ok(
        "建设备乙",
        DictionarySync::new(base, username, password, root.join("b"), "合成设备乙"),
    );
    let mut a = Words(BTreeSet::from([
        word("shui'shan", "水杉"),
        word("ni'hao", "你好"),
    ]));
    let mut b = Words::default();
    let run = synced("设备甲第一次同步", &first, &mut a);
    assert!(run.view.approved, "第一台设备生成密钥");

    let run = manual("设备乙登记", &second, &mut b);
    assert!(!run.view.approved, "新设备等待批准");
    settle();
    let devices = ok("设备甲列出设备", first.devices());
    let pending = devices
        .devices
        .iter()
        .find(|device| !device.approved)
        .expect("设备乙在设备甲的列表里等待批准");
    assert_eq!(Some(&pending.device_id), run.view.device_id.as_ref());
    assert_eq!(
        Some(&pending.code),
        run.view.code.as_ref(),
        "两边的核对码相同"
    );
    ok("设备甲批准设备乙", first.approve(&pending.device_id));
    settle();
    let run = synced("设备乙加入", &second, &mut b);
    assert!(run.view.approved && run.view.joined, "设备乙获准并加入");
    assert_eq!(b.0, a.0, "设备乙收到设备甲的词");

    b.0.remove(&word("ni'hao", "你好"));
    synced("设备乙删词后同步", &second, &mut b);
    synced("设备甲拉取删词", &first, &mut a);
    assert_eq!(
        a.0,
        BTreeSet::from([word("shui'shan", "水杉")]),
        "设备甲看到删除"
    );

    // 两台设备同时发布，抢同一个序号；输的一方重读再写，两边的词都要进去。
    a.0.insert(word("jia", "合成甲"));
    b.0.insert(word("yi", "合成乙"));
    std::thread::scope(|scope| {
        let left = scope.spawn(|| {
            synced("设备甲同时发布", &first, &mut a);
        });
        let right = scope.spawn(|| {
            synced("设备乙同时发布", &second, &mut b);
        });
        for joined in [left.join(), right.join()] {
            if let Err(payload) = joined {
                resume_unwind(payload);
            }
        }
    });
    synced("设备甲收尾", &first, &mut a);
    synced("设备乙收尾", &second, &mut b);
    assert_eq!(a.0, b.0, "两台设备收敛");
    assert_eq!(
        a.0,
        BTreeSet::from([
            word("shui'shan", "水杉"),
            word("jia", "合成甲"),
            word("yi", "合成乙"),
        ])
    );
}

// ---- 一次运行 ----

/// 一次运行的结果。所有文字都已经隐去凭据。
#[derive(Default)]
struct Summary {
    lines: Vec<String>,
    failures: Vec<String>,
    leftovers: Vec<String>,
    total: u64,
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
        })
        .unwrap_or_else(|| "panic".to_owned())
}

/// 一个流程：本次的基地址、网盘和本机目录。
type Flow = fn(&str, &Target, &Path);

/// 一次运行的目录名：毫秒时间加随机数，每次都是新的。
fn run_name() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let mut bytes = [0u8; 4];
    getrandom::fill(&mut bytes).unwrap();
    format!("run-{millis}-{}", hex::encode(bytes))
}

/// 在 `msime-e2e/` 下新建本次的目录，依次跑两个流程（限流之后不再开始下一个），最后删掉本次的目录并核对。
fn run_end_to_end(target: &Target) -> Summary {
    RATE_LIMITED.store(false, Ordering::SeqCst);
    let mut summary = Summary::default();
    let start = requests_sent();
    let mut mark = start;
    let mut spent = |summary: &mut Summary, phase: &str, outcome: &str| {
        let now = requests_sent();
        summary
            .lines
            .push(format!("{phase}：{} 个请求，{outcome}", now - mark));
        mark = now;
    };
    let raw = match target.root().and_then(|root| RawDav::new(&root)) {
        Ok(raw) => raw,
        Err(error) => {
            summary
                .failures
                .push(format!("地址、账号或应用密码不合规（{}）", error.code()));
            return summary;
        }
    };
    let run_dir = format!("{PARENT}{}/", run_name());
    summary.lines.push(format!(
        "网盘：{}（账号已隐去），本次目录：{run_dir}",
        target.redact(&target.url)
    ));
    // `msime-e2e/` 已经存在时 MKCOL 答 405 算成功；有的网盘答别的码，再列一次确认它在。
    let parent = raw.mkcol(PARENT).or_else(|error| match raw.list(PARENT) {
        Ok(Some(_)) => Ok(false),
        _ => Err(error),
    });
    let created = parent.and_then(|_| raw.mkcol(&run_dir));
    match created {
        Ok(true) => spent(&mut summary, "准备", "通过"),
        Ok(false) => {
            spent(&mut summary, "准备", "失败");
            summary
                .failures
                .push(format!("本次目录 {run_dir} 已经存在，没有动它"));
            summary.total = requests_sent() - start;
            return summary;
        }
        Err(error) => {
            spent(&mut summary, "准备", "失败");
            let code = error.code();
            let hint = if is_rate_limited(code) {
                "，被限流，过半小时再跑"
            } else if code == "relay_unauthorized" {
                "，账号或应用密码不对"
            } else {
                ""
            };
            summary
                .failures
                .push(format!("建测试目录失败（{code}）{hint}"));
            summary.total = requests_sent() - start;
            return summary;
        }
    }
    let base = format!("{}/{run_dir}", target.url.trim_end_matches('/'));
    let local = tempfile::tempdir().unwrap();
    let flows: [(&str, Flow); 2] = [("云剪贴板", clipboard_flow), ("词库同步", dictionary_flow)];
    for (name, flow) in flows {
        if RATE_LIMITED.load(Ordering::SeqCst) {
            summary.lines.push(format!("{name}：前面被限流，没有开始"));
            continue;
        }
        let directory = local.path().join(name);
        let outcome = catch_unwind(AssertUnwindSafe(|| flow(&base, target, &directory)));
        match outcome {
            Ok(()) => spent(&mut summary, name, "通过"),
            Err(payload) => {
                spent(&mut summary, name, "失败");
                summary.failures.push(format!(
                    "{name}：{}",
                    target.redact(&panic_text(payload.as_ref()))
                ));
            }
        }
    }
    summary.leftovers = clean_up(&raw, &run_dir);
    let outcome = if summary.leftovers.is_empty() {
        "没有残留".to_owned()
    } else {
        format!("留下了 {}", summary.leftovers.join("、"))
    };
    spent(&mut summary, "清理", &outcome);
    summary.total = requests_sent() - start;
    summary
}

/// 删掉本次的目录，再列一次确认它没了。删不掉或确认不了时报出留下了什么（尽力而为，不重试）。
fn clean_up(raw: &RawDav, run_dir: &str) -> Vec<String> {
    let own = run_dir
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default();
    let deleted = raw.delete(run_dir);
    match raw.list(run_dir) {
        Ok(None) => Vec::new(),
        Ok(Some(names)) => {
            let inside: Vec<String> = names.into_iter().filter(|name| name != own).collect();
            let why = match deleted {
                Ok(true) => "删除答成功但还列得出".to_owned(),
                Ok(false) => "网盘拒绝删除".to_owned(),
                Err(error) => format!("删除失败（{}）", error.code()),
            };
            vec![format!("{run_dir}（{why}，里面有：{}）", inside.join("、"))]
        }
        Err(error) => vec![format!("{run_dir}（确认不了是否删掉：{}）", error.code())],
    }
}

/// 计数是进程级的：联网的测试一个一个跑，各自的请求数才不会混在一起。
static NETWORK: Mutex<()> = Mutex::new(());

fn report(summary: &Summary) {
    for line in &summary.lines {
        println!("[webdav-e2e] {line}");
    }
    println!("[webdav-e2e] 合计：{} 个 HTTP 请求", summary.total);
}

#[test]
fn the_real_server_flows_run_when_the_environment_names_a_server() {
    let target = match Target::from_env() {
        Ok(Some(target)) => target,
        Ok(None) => {
            println!("[webdav-e2e] 跳过：没有设置 {URL_VAR}、{USERNAME_VAR}、{PASSWORD_VAR}");
            return;
        }
        Err(message) => panic!("{message}"),
    };
    let _serial = NETWORK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // 断言和 panic 的信息先经隐去再打印；默认的钩子会原样打印。
    let secrets = target.secrets();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!(
            "{}",
            redact(&info.to_string(), &secrets.each_ref().map(String::as_str))
        );
    }));
    let summary = run_end_to_end(&target);
    report(&summary);
    let mut problems = summary.failures.clone();
    problems.extend(
        summary
            .leftovers
            .iter()
            .map(|leftover| format!("网盘上留下了 {leftover}，请手动删除")),
    );
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert!(
        summary.total <= REQUEST_CEILING,
        "这次用了 {} 个请求，超过上限 {REQUEST_CEILING}",
        summary.total
    );
}

// ---- 同一套流程对着本机的 dav-server ----

const LOCAL_USER: &str = "synthetic@example.test";
const LOCAL_PASSWORD: &str = "synthetic-app-password";

#[derive(Clone)]
struct Dav {
    handler: DavHandler,
    gate: Arc<tokio::sync::Mutex<()>>,
    authorization: String,
    requests: Arc<AtomicUsize>,
}

async fn handle(State(dav): State<Dav>, request: Request) -> Response {
    dav.requests.fetch_add(1, Ordering::SeqCst);
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
    // `dav-server` 的 MOVE 先查后改，一个一个处理才相当于独占创建可靠的网盘。
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
    requests: Arc<AtomicUsize>,
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
    let requests = Arc::<AtomicUsize>::default();
    let dav = Dav {
        handler: DavHandler::builder()
            .filesystem(MemFs::new())
            .locksystem(FakeLs::new())
            .strip_prefix("/dav")
            .build_handler(),
        gate: Arc::default(),
        authorization: format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD
                .encode(format!("{LOCAL_USER}:{LOCAL_PASSWORD}"))
        ),
        requests: requests.clone(),
    };
    runtime.spawn(async move {
        let app = Router::new().fallback(handle).with_state(dav);
        let _ = axum::serve(listener, app).await;
    });
    Server {
        address,
        requests,
        runtime: Some(runtime),
    }
}

#[test]
fn the_real_server_flows_pass_against_a_local_dav_server_within_the_request_ceiling() {
    let _serial = NETWORK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let server = start();
    let target = Target {
        url: format!("http://{}/dav/", server.address),
        username: LOCAL_USER.to_owned(),
        password: LOCAL_PASSWORD.to_owned(),
    };
    let summary = run_end_to_end(&target);
    report(&summary);
    assert!(summary.failures.is_empty(), "{:?}", summary.failures);
    assert!(summary.leftovers.is_empty(), "{:?}", summary.leftovers);
    assert_eq!(
        summary.total,
        server.requests.load(Ordering::SeqCst) as u64,
        "客户端的计数与服务端收到的请求数一致"
    );
    assert!(
        summary.total <= REQUEST_CEILING,
        "流程用了 {} 个请求，超过上限 {REQUEST_CEILING}",
        summary.total
    );
    // 第二次运行用新的目录，`msime-e2e/` 已经在也照常通过，根目录下只有它。
    let again = run_end_to_end(&target);
    assert!(again.failures.is_empty(), "{:?}", again.failures);
    let root = RawDav::new(&target.root().unwrap()).unwrap();
    let mut names = root.list("").unwrap().unwrap();
    names.retain(|name| name != "dav");
    assert_eq!(names, [PARENT.trim_end_matches('/')]);
    let mut inside = root.list(PARENT).unwrap().unwrap();
    inside.retain(|name| name != "msime-e2e");
    assert!(inside.is_empty(), "每次运行的目录都删掉了：{inside:?}");
}

#[test]
fn credentials_are_redacted_from_output_and_panic_messages() {
    let target = Target {
        url: "https://dav.example.test/remote.php/dav/files/Someone%40Example.test/".to_owned(),
        username: "someone@example.test".to_owned(),
        password: "synthetic-secret-pass".to_owned(),
    };
    let basic = base64::engine::general_purpose::STANDARD
        .encode("someone@example.test:synthetic-secret-pass");
    let text = format!(
        "user someone@example.test pass synthetic-secret-pass upper SOMEONE@EXAMPLE.TEST url {} header Authorization: Basic {basic}",
        target.url
    );
    let redacted = target.redact(&text);
    for leaked in [
        "someone",
        "Someone",
        "SOMEONE",
        "synthetic-secret-pass",
        &basic,
    ] {
        assert!(!redacted.contains(leaked), "{redacted}");
    }
    assert_eq!(
        redacted,
        "user *** pass *** upper *** url https://dav.example.test/remote.php/dav/files/***/ header Authorization: Basic ***"
    );
    // 一个秘密包含另一个时，长的先换，不会漏出剩下的片段。
    assert_eq!(redact("abc-secret", &["abc", "abc-secret"]), "***");
    // 空的秘密什么也不换，非 ASCII 文字原样保留。
    assert_eq!(redact("合成文字", &["", "x"]), "合成文字");
    let debug = format!("{target:?}");
    assert!(
        !debug.contains("someone") && !debug.contains("Someone"),
        "{debug}"
    );
    assert!(!debug.contains("synthetic-secret-pass"), "{debug}");
    // 流程里失败的那一步带出的信息同样经过隐去。
    let payload: Box<dyn std::any::Any + Send> =
        Box::new("步骤失败：someone@example.test synthetic-secret-pass".to_owned());
    assert_eq!(
        target.redact(&panic_text(payload.as_ref())),
        "步骤失败：*** ***"
    );
}
