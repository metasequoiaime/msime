//! 云剪贴板的 WebDAV 后端：在本机扮演一个「虚拟中继」。
//!
//! 端到端加密驱动照旧发中继的那 12 个接口（`docs/cloud-clipboard-e2e.md`），[`VirtualRelay::send`] 不把它们发给任何服务端，而是读写用户网盘上的一份只追加的事件日志来回答，语义与参考中继相同（日志核心是不碰网络的 `msime_client_core::cloud::clipboard_log`）。协议、加密、密钥串、设备批准和核对码都不变，网盘只见到密文。存储布局是一份契约，见 `docs/cloud-clipboard-webdav.md`：
//!
//! ```text
//! <基地址>/msime-clipboard/v1/format.json
//! <基地址>/msime-clipboard/v1/epoch-NNNNNN/snapshot.json
//! <基地址>/msime-clipboard/v1/epoch-NNNNNN/e-NNNNNNNNNNNN.json
//! <基地址>/msime-clipboard/v1/tmp/<时间>-<随机数>.json
//! ```
//!
//! 一个事件就是一个不可变的文件，序号是唯一的文件名，谁先创建出下一个序号谁的写入算数。独占创建不用 `PUT If-None-Match: *`（坚果云不理会这个头，并发的 PUT 全都「成功」并互相覆盖），而是先 PUT 到一个唯一的临时名，再 `MOVE` 到目标并带 `Overwrite: F`：坚果云和 sabre/dav（Nextcloud）都只让一个 MOVE 成功，其余答 409 或 412。
//!
//! 没有后台轮询：面板打开、刷新和复制时才同步。一次读取是一个 PROPFIND（当前 epoch，Depth 1）加上新事件的 GET，算出的状态和读到的位置缓存在本机（[`WEBDAV_STATE_FILE`]，只有密文和公开数据）。网盘答 429 或 503 时按 `Retry-After` 退避，仍不行就是 `webdav_rate_limited`。
//!
//! 能写网盘的人（包括已经被移除、但还知道应用密码的设备）可以删掉事件，比如删掉一次换钥，让旧密钥复活。本机记住读到的位置、链头和密钥代数，下一个事件必须接在链头后面，位置和代数也不许变小，否则是 `webdav_rollback`。移除设备之后要换应用密码，见文档。

use crate::account::{AccountSessionFileLayout, FileAccountSessionStorage};
use crate::clipboard_relay::{lock_directory, RelayConfig, RelayError};
use crate::cloud::clipboard_e2e::HttpResponse;
use crate::cloud::clipboard_log::{parse_request, Draft, Ledger, LogError, Plan, Response};
use reqwest::blocking::Client;
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

/// 本机缓存的文件名：读到的日志位置、由它算出的状态（只有密文和公开数据）和待清理的旧 epoch。仅本人可读写，原子替换写入。
pub const WEBDAV_STATE_FILE: &str = "cloud-clipboard-webdav-state";
/// 50 条最长的密文、20 台设备和封装加起来不到 1 MiB。
const MAX_STATE_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_LISTING_BYTES: u64 = 2 * 1024 * 1024;
/// 一个事件最大的是一条 16384 个字符的密文或 20 个封装。
const MAX_EVENT_BYTES: u64 = 128 * 1024;
const MAX_SNAPSHOT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_SMALL_BYTES: u64 = 4096;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// 这么久之内读过的缓存直接用：驱动的一个操作要连着发两三个读请求，不必每个都去网盘。
const FRESH_MS: u64 = 2_000;
/// 写入时抢不到序号就重读再试，最多这么多次。
const MAX_WRITE_ATTEMPTS: usize = 6;
/// 一次同步最多跨几个 epoch。
const MAX_SYNC_ROUNDS: usize = 8;
const MAX_RATE_RETRIES: u32 = 3;
/// `Retry-After` 比这还长就不等了，直接报 `webdav_rate_limited`：输入法和设置页的请求不能挂几分钟。
const MAX_RATE_WAIT: Duration = Duration::from_secs(8);
/// 旧 epoch 在新快照写好之后再留这么久才删，给正在读它的设备一点时间。
const OLD_EPOCH_GRACE_SECS: i64 = 120;
/// 临时文件放了这么久还在，说明写入的设备在 PUT 和 MOVE 之间中断了。
const STALE_TMP_SECS: i64 = 600;
/// 「测试连接」里同时抢同一个名字的写入数。
const PROBE_RACERS: usize = 4;

const ROOT: &str = "msime-clipboard/";
const V1: &str = "msime-clipboard/v1/";
const TMP: &str = "msime-clipboard/v1/tmp/";
const FORMAT_FILE: &str = "msime-clipboard/v1/format.json";
const FORMAT_NAME: &str = "format.json";
const SNAPSHOT_NAME: &str = "snapshot.json";

fn epoch_dir(epoch: u64) -> String {
    format!("{V1}epoch-{epoch:06}/")
}

fn event_path(epoch: u64, seq: u64) -> String {
    format!("{}e-{seq:012}.json", epoch_dir(epoch))
}

fn snapshot_path(epoch: u64) -> String {
    format!("{}{SNAPSHOT_NAME}", epoch_dir(epoch))
}

fn format_bytes() -> Vec<u8> {
    json!({ "format": crate::cloud::clipboard_log::FORMAT, "version": crate::cloud::clipboard_log::FORMAT_VERSION })
        .to_string()
        .into_bytes()
}

fn digits(value: &str, length: usize) -> Option<u64> {
    (value.len() == length && value.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| value.parse().ok())
        .flatten()
}

fn epoch_name(name: &str) -> Option<u64> {
    digits(name.strip_prefix("epoch-")?, 6)
}

fn event_name(name: &str) -> Option<u64> {
    digits(name.strip_prefix("e-")?.strip_suffix(".json")?, 12)
}

/// 临时文件名里的写入时间（Unix 秒）。
fn tmp_name(name: &str) -> Option<i64> {
    let (time, random) = name.strip_suffix(".json")?.split_once('-')?;
    (crate::cloud::clipboard_log::is_lower_hex(random, 32)
        && !time.is_empty()
        && time.len() <= 12
        && time.bytes().all(|byte| byte.is_ascii_digit()))
    .then(|| time.parse().ok())
    .flatten()
}

fn random_hex<const N: usize>() -> Result<String, RelayError> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|_| RelayError::Unavailable)?;
    Ok(hex::encode(bytes))
}

// ---- 时钟 ----

/// 时钟和等待。测试换成假的，退避和宽限期因此不用真的等。
pub(crate) trait Environment: Send + Sync {
    fn now_ms(&self) -> u64;
    fn sleep(&self, duration: Duration);
    fn now(&self) -> i64 {
        (self.now_ms() / 1000) as i64
    }
}

struct SystemEnvironment;

impl Environment for SystemEnvironment {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or_default()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

// ---- 一个 WebDAV 请求 ----

/// 发给网盘的一个请求。`path` 相对于用户选的基地址，集合以 `/` 结尾；`destination` 是 MOVE 的目标（同样是相对路径），一律带 `Overwrite: F`。
pub(crate) struct DavRequest<'a> {
    pub method: &'static str,
    pub path: &'a str,
    pub depth: Option<&'static str>,
    pub destination: Option<&'a str>,
    pub body: Option<&'a [u8]>,
    /// 成功响应正文的上限。
    pub limit: u64,
}

pub(crate) struct DavResponse {
    pub status: u16,
    /// `Retry-After` 的秒数；没有或者不是整数时为 `None`。
    pub retry_after: Option<u64>,
    pub body: Vec<u8>,
}

/// 把一个请求发出去，状态码原样交回。真实的实现是 [`HttpDav`]，测试里是内存里的假网盘。
pub(crate) trait DavHttp: Send + Sync {
    fn send(&self, request: &DavRequest<'_>) -> Result<DavResponse, RelayError>;
}

/// 经 reqwest 的阻塞客户端发 WebDAV 请求：HTTP Basic 鉴权、不跟随重定向、30 秒超时，只发往配置的基地址之下。
pub(crate) struct HttpDav {
    base: Url,
    prefix: String,
    username: String,
    password: String,
    client: Client,
}

impl HttpDav {
    pub(crate) fn new(config: &RelayConfig) -> Result<Self, RelayError> {
        let (username, password) = config.webdav_credentials().ok_or(RelayError::Invalid)?;
        let base = Url::parse(&format!("{}/", config.url())).map_err(|_| RelayError::Invalid)?;
        let client = crate::proxy::configure_blocking_for(Client::builder(), config.base())
            .redirect(reqwest::redirect::Policy::none())
            .timeout(REQUEST_TIMEOUT)
            .user_agent(format!("MSIME/{}", std::env::consts::OS))
            .build()
            .map_err(|_| RelayError::Unavailable)?;
        Ok(Self {
            prefix: base.path().to_owned(),
            base,
            username: username.to_owned(),
            password: password.to_owned(),
            client,
        })
    }

    /// 基地址拼上一个本模块生成的相对路径；只有小写字母、数字、`-`、`.`、`_` 和 `/`，拼完再确认没有跑出基地址。
    fn url(&self, path: &str) -> Result<Url, RelayError> {
        let plain = path.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-._/".contains(&byte)
        });
        if !plain || path.starts_with('/') || path.contains("//") || path.contains("..") {
            return Err(RelayError::Invalid);
        }
        let url = self.base.join(path).map_err(|_| RelayError::Invalid)?;
        if url.scheme() != self.base.scheme()
            || url.host_str() != self.base.host_str()
            || url.port_or_known_default() != self.base.port_or_known_default()
            || !url.path().starts_with(&self.prefix)
        {
            return Err(RelayError::Invalid);
        }
        Ok(url)
    }
}

impl DavHttp for HttpDav {
    fn send(&self, request: &DavRequest<'_>) -> Result<DavResponse, RelayError> {
        let method =
            Method::from_bytes(request.method.as_bytes()).map_err(|_| RelayError::Invalid)?;
        let mut builder = self
            .client
            .request(method, self.url(request.path)?)
            .basic_auth(&self.username, Some(&self.password));
        if let Some(depth) = request.depth {
            builder = builder.header("Depth", depth);
        }
        if let Some(destination) = request.destination {
            builder = builder
                .header("Destination", self.url(destination)?.as_str())
                .header("Overwrite", "F");
        }
        if let Some(body) = request.body {
            let content_type = if request.method == "PROPFIND" {
                "application/xml; charset=utf-8"
            } else {
                "application/json"
            };
            builder = builder
                .header(reqwest::header::CONTENT_TYPE, content_type)
                .body(body.to_vec());
        }
        let response = builder.send().map_err(|_| RelayError::Unavailable)?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse().ok());
        let limit = if (200..300).contains(&status) {
            request.limit
        } else {
            MAX_SMALL_BYTES
        };
        if response
            .content_length()
            .is_some_and(|length| length > limit && (200..300).contains(&status))
        {
            return Err(RelayError::Unavailable);
        }
        let body = crate::bounded_io::read_bounded(response, limit).unwrap_or_default();
        Ok(DavResponse {
            status,
            retry_after,
            body,
        })
    }
}

// ---- 有语义的操作 ----

/// PROPFIND 正文：只要资源类型。
const PROPFIND_BODY: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:"><d:prop><d:resourcetype/></d:prop></d:propfind>"#;

/// 读 207 多状态响应，得出每个 `href` 的最后一段（去掉末尾的 `/`）。命名空间前缀各家不同（`d:`、`D:`、`ns0:`），只看本地名。
pub(crate) fn listing_names(body: &[u8]) -> Option<Vec<String>> {
    use quick_xml::events::Event;
    let text = std::str::from_utf8(body).ok()?;
    let mut reader = quick_xml::Reader::from_str(text);
    let mut names = Vec::new();
    let mut href: Option<String> = None;
    loop {
        match reader.read_event().ok()? {
            Event::Start(element) if element.local_name().as_ref() == "href" => {
                href = Some(String::new());
            }
            Event::Text(content) => {
                if let Some(href) = href.as_mut() {
                    href.push_str(&content);
                }
            }
            // 实体引用（`&amp;` 之类）只会出现在别人的文件名里；记一个本模块的名字里不会有的字符，这条就认不出了。
            Event::GeneralRef(_) => {
                if let Some(href) = href.as_mut() {
                    href.push('&');
                }
            }
            Event::End(element) if element.local_name().as_ref() == "href" => {
                if let Some(href) = href.take() {
                    let path = href.split(['?', '#']).next().unwrap_or_default();
                    if let Some(name) = path.trim_end_matches('/').rsplit('/').next() {
                        if !name.is_empty() {
                            names.push(name.to_owned());
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Some(names)
}

/// 在 [`DavHttp`] 上的几个有语义的操作：401 是 [`RelayError::Unauthorized`]，429 和 503 按 `Retry-After` 退避后重试。
pub(crate) struct Dav<'a> {
    http: &'a dyn DavHttp,
    env: &'a dyn Environment,
}

impl<'a> Dav<'a> {
    pub(crate) fn new(http: &'a dyn DavHttp, env: &'a dyn Environment) -> Self {
        Self { http, env }
    }

    fn call(&self, request: &DavRequest<'_>) -> Result<DavResponse, RelayError> {
        let mut attempt = 0;
        loop {
            let response = self.http.send(request)?;
            match response.status {
                401 => return Err(RelayError::Unauthorized),
                429 | 503 => {
                    if attempt >= MAX_RATE_RETRIES {
                        return Err(RelayError::RateLimited);
                    }
                    let wait = Duration::from_secs(response.retry_after.unwrap_or(1 << attempt));
                    if wait > MAX_RATE_WAIT {
                        return Err(RelayError::RateLimited);
                    }
                    self.env.sleep(wait);
                    attempt += 1;
                }
                _ => return Ok(response),
            }
        }
    }

    fn request<'r>(method: &'static str, path: &'r str) -> DavRequest<'r> {
        DavRequest {
            method,
            path,
            depth: None,
            destination: None,
            body: None,
            limit: MAX_SMALL_BYTES,
        }
    }

    /// 一个集合里的条目名；集合不存在时为 `None`。
    pub(crate) fn list(&self, dir: &str) -> Result<Option<Vec<String>>, RelayError> {
        let response = self.call(&DavRequest {
            depth: Some("1"),
            body: Some(PROPFIND_BODY),
            limit: MAX_LISTING_BYTES,
            ..Self::request("PROPFIND", dir)
        })?;
        match response.status {
            207 => listing_names(&response.body)
                .map(Some)
                .ok_or(RelayError::Unavailable),
            404 => Ok(None),
            _ => Err(RelayError::Unavailable),
        }
    }

    /// 一个文件的内容；不存在时为 `None`。
    pub(crate) fn get(&self, path: &str, limit: u64) -> Result<Option<Vec<u8>>, RelayError> {
        let response = self.call(&DavRequest {
            limit,
            ..Self::request("GET", path)
        })?;
        match response.status {
            200 => Ok(Some(response.body)),
            404 => Ok(None),
            _ => Err(RelayError::Unavailable),
        }
    }

    /// 建一个集合。已经存在（405）也算成功；返回是不是这一次新建的。
    pub(crate) fn mkcol(&self, dir: &str) -> Result<bool, RelayError> {
        match self.call(&Self::request("MKCOL", dir))?.status {
            200 | 201 | 204 => Ok(true),
            405 => Ok(false),
            _ => Err(RelayError::Unavailable),
        }
    }

    /// 删除一个文件或集合。不存在也算成功；网盘拒绝时（例如坚果云不许删根目录下的同步文件夹）返回 `false`。
    pub(crate) fn delete(&self, path: &str) -> Result<bool, RelayError> {
        Ok(matches!(
            self.call(&Self::request("DELETE", path))?.status,
            200 | 202 | 204 | 404
        ))
    }

    fn put(&self, path: &str, bytes: &[u8]) -> Result<u16, RelayError> {
        Ok(self
            .call(&DavRequest {
                body: Some(bytes),
                ..Self::request("PUT", path)
            })?
            .status)
    }

    /// 独占创建 `target`：先写到 `tmp_dir` 里一个唯一的临时名，再 `MOVE` 过去并带 `Overwrite: F`。抢到时为 `true`；目标已经存在（412，坚果云答 409）或者目标所在的目录没了（409）时为 `false`，临时文件随即删掉。
    pub(crate) fn create_in(
        &self,
        tmp_dir: &str,
        target: &str,
        bytes: &[u8],
    ) -> Result<bool, RelayError> {
        let tmp = format!("{tmp_dir}{}-{}.json", self.env.now(), random_hex::<16>()?);
        let mut status = self.put(&tmp, bytes)?;
        if status == 409 && tmp_dir == TMP {
            // 临时目录被人删了：重建一次。
            self.mkcol(TMP)?;
            status = self.put(&tmp, bytes)?;
        }
        if !matches!(status, 200 | 201 | 204) {
            return Err(RelayError::Unavailable);
        }
        let moved = self.call(&DavRequest {
            destination: Some(target),
            ..Self::request("MOVE", &tmp)
        });
        let status = match moved {
            Ok(response) => response.status,
            Err(error) => {
                let _ = self.delete(&tmp);
                return Err(error);
            }
        };
        match status {
            200 | 201 | 204 => Ok(true),
            409 | 412 => {
                let _ = self.delete(&tmp);
                Ok(false)
            }
            _ => {
                let _ = self.delete(&tmp);
                Err(RelayError::Unavailable)
            }
        }
    }

    pub(crate) fn create(&self, target: &str, bytes: &[u8]) -> Result<bool, RelayError> {
        self.create_in(TMP, target, bytes)
    }
}

// ---- 本机缓存 ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cleanup {
    epoch: u64,
    not_before: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredLog {
    relay: String,
    space: String,
    ledger: Ledger,
    synced_at_ms: u64,
    #[serde(default)]
    cleanup: Vec<Cleanup>,
}

// ---- 虚拟中继 ----

/// 用 WebDAV 网盘回答中继接口的本机「中继」。每个方法都会联网，不要在界面线程或输入线程上调用。
pub struct VirtualRelay {
    config: RelayConfig,
    http: Box<dyn DavHttp>,
    env: Box<dyn Environment>,
    directory: PathBuf,
    locks: bool,
}

impl VirtualRelay {
    /// `directory` 放本机缓存；`locks` 为真时每个请求持有 `cloud-clipboard-relay.lock`（单独借它发请求的宿主），为假时由调用方在整个操作期间持有（[`crate::clipboard_relay::RelayClipboard`]）。
    pub(crate) fn new(
        config: RelayConfig,
        directory: PathBuf,
        locks: bool,
    ) -> Result<Self, RelayError> {
        let http = HttpDav::new(&config)?;
        Ok(Self {
            config,
            http: Box::new(http),
            env: Box::new(SystemEnvironment),
            directory,
            locks,
        })
    }

    #[cfg(test)]
    pub(crate) fn with_parts(
        config: RelayConfig,
        directory: PathBuf,
        http: Box<dyn DavHttp>,
        env: Box<dyn Environment>,
        locks: bool,
    ) -> Self {
        Self {
            config,
            http,
            env,
            directory,
            locks,
        }
    }

    pub(crate) fn config(&self) -> &RelayConfig {
        &self.config
    }

    fn storage(&self) -> FileAccountSessionStorage {
        FileAccountSessionStorage::new(&self.directory, AccountSessionFileLayout::Native)
    }

    fn load(&self) -> Result<Option<StoredLog>, RelayError> {
        let Some(bytes) = self
            .storage()
            .read_private(WEBDAV_STATE_FILE, MAX_STATE_FILE_BYTES)
            .map_err(|_| RelayError::Storage)?
        else {
            return Ok(None);
        };
        let Ok(stored) = serde_json::from_slice::<StoredLog>(&bytes) else {
            return Ok(None);
        };
        Ok(
            (stored.relay == self.config.url() && stored.space == self.config.space())
                .then_some(stored),
        )
    }

    fn save(&self, stored: &StoredLog) -> Result<(), RelayError> {
        let bytes = serde_json::to_vec(stored).map_err(|_| RelayError::Storage)?;
        self.storage()
            .create_directory()
            .map_err(|_| RelayError::Storage)?;
        self.storage()
            .write_private(WEBDAV_STATE_FILE, &bytes)
            .map_err(|_| RelayError::Storage)
    }

    /// 回答一个中继请求，状态码和正文与参考中继相同。路径、方法和正文已经由 [`crate::clipboard_relay::RelayTransport::send`] 核对过。网盘拒绝账号或密码时是 [`RelayError::Unauthorized`]，日志被回退时是 [`RelayError::Rollback`]，限流是 [`RelayError::RateLimited`]，其余连不上、读不懂是 [`RelayError::Unavailable`]。会联网。
    pub fn send(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<HttpResponse, RelayError> {
        let body = body.unwrap_or_default().as_bytes();
        // 解析不了、不合规的请求在本机答，不碰网盘。
        let request = match parse_request(method, path, body) {
            Ok(request) => request,
            Err(response) => return Ok(http_response(response)),
        };
        let _lock = if self.locks {
            Some(lock_directory(&self.directory)?)
        } else {
            None
        };
        let mut session = Session {
            relay: self,
            dav: Dav::new(self.http.as_ref(), self.env.as_ref()),
            stored: self.load()?,
            synced: false,
            dirty: false,
        };
        let outcome = session.handle(request.is_write(), method, path, body);
        if outcome.is_ok() {
            session.maintain();
        }
        if session.dirty {
            if let Some(stored) = &session.stored {
                self.save(stored)?;
            }
        }
        outcome.map(http_response)
    }
}

fn http_response(response: Response) -> HttpResponse {
    HttpResponse {
        status: response.status,
        body: response.body.unwrap_or_default(),
    }
}

/// 一次 [`VirtualRelay::send`] 的工作状态。
struct Session<'a> {
    relay: &'a VirtualRelay,
    dav: Dav<'a>,
    stored: Option<StoredLog>,
    /// 这一次请求里已经和网盘对过。
    synced: bool,
    dirty: bool,
}

impl Session<'_> {
    fn now(&self) -> i64 {
        self.relay.env.now()
    }

    fn fresh(&self) -> bool {
        self.stored.as_ref().is_some_and(|stored| {
            self.relay.env.now_ms().saturating_sub(stored.synced_at_ms) < FRESH_MS
        })
    }

    fn ledger(&self) -> Result<&Ledger, RelayError> {
        self.stored
            .as_ref()
            .map(|stored| &stored.ledger)
            .ok_or(RelayError::Unavailable)
    }

    fn handle(
        &mut self,
        write: bool,
        method: &str,
        path: &str,
        body: &[u8],
    ) -> Result<Response, RelayError> {
        if !write {
            if !self.fresh() {
                self.sync()?;
            }
            let draft = Draft {
                now: self.now(),
                nonce: String::new(),
                item_id: String::new(),
            };
            return match self.ledger()?.plan(method, path, body, &draft) {
                Plan::Reply(response) => Ok(response),
                Plan::Append { .. } => Err(RelayError::Unavailable),
            };
        }
        if self.stored.is_none() {
            self.sync()?;
        }
        for _ in 0..MAX_WRITE_ATTEMPTS {
            let ledger = self.ledger()?.clone();
            if ledger.sealed {
                self.sync()?;
                continue;
            }
            let draft = Draft::random(self.now()).map_err(|_| RelayError::Unavailable)?;
            match ledger.plan(method, path, body, &draft) {
                // 不写入的回答（被拒绝、或者不改变状态）只在这一次刚读过网盘时才算数：缓存可能落后，例如另一台设备刚换过钥。
                Plan::Reply(response) if self.synced => return Ok(response),
                Plan::Reply(_) => self.sync()?,
                Plan::Append { bytes, response } => {
                    let target = event_path(ledger.epoch, ledger.seq + 1);
                    if self.dav.create(&target, &bytes)? {
                        let stored = self.stored.as_mut().ok_or(RelayError::Unavailable)?;
                        stored
                            .ledger
                            .apply(&bytes)
                            .map_err(|_| RelayError::Unavailable)?;
                        self.dirty = true;
                        return Ok(response);
                    }
                    // 另一台设备先写了这个序号：读进来，在新的状态上再处理一次。
                    self.sync()?;
                }
            }
        }
        Err(RelayError::Unavailable)
    }

    fn schedule_cleanup(&mut self, epoch: u64, not_before: i64) {
        if let Some(stored) = self.stored.as_mut() {
            if !stored.cleanup.iter().any(|entry| entry.epoch == epoch) {
                stored.cleanup.push(Cleanup { epoch, not_before });
            }
        }
    }

    fn store(&mut self, ledger: Ledger, cleanup: Vec<u64>) {
        let now_ms = self.relay.env.now_ms();
        let not_before = self.now() + OLD_EPOCH_GRACE_SECS;
        match self.stored.as_mut() {
            Some(stored) => {
                stored.ledger = ledger;
                stored.synced_at_ms = now_ms;
            }
            None => {
                self.stored = Some(StoredLog {
                    relay: self.relay.config.url().to_owned(),
                    space: self.relay.config.space(),
                    ledger,
                    synced_at_ms: now_ms,
                    cleanup: Vec::new(),
                })
            }
        }
        for epoch in cleanup {
            self.schedule_cleanup(epoch, not_before);
        }
        self.synced = true;
        self.dirty = true;
    }

    /// 读网盘，把本机的位置推到最新：当前 epoch 里的新事件按序号接上；遇到封口就进入下一个 epoch（它的目录或快照还没写好时由本机补上）；当前 epoch 已经被删掉时从最新的快照重新开始，并核对位置没有倒退。
    fn sync(&mut self) -> Result<(), RelayError> {
        let mut cleanup = Vec::new();
        let mut ledger = match self.stored.as_ref() {
            Some(stored) => stored.ledger.clone(),
            None => self.bootstrap(None, &mut cleanup)?,
        };
        let mut entered = false;
        let mut created = false;
        for _ in 0..MAX_SYNC_ROUNDS * 2 {
            if ledger.sealed {
                let previous = ledger.epoch;
                ledger = ledger.next_epoch().ok_or(RelayError::Unavailable)?;
                cleanup.push(previous);
                entered = true;
                created = false;
                continue;
            }
            let dir = epoch_dir(ledger.epoch);
            let names = match self.dav.list(&dir)? {
                Some(names) => names,
                None if entered && !created => {
                    // 封口的设备还没建好下一个 epoch：内容由日志唯一决定，本机照样写得出来。
                    self.dav.mkcol(&dir)?;
                    self.dav
                        .create(&snapshot_path(ledger.epoch), &ledger.snapshot())?;
                    created = true;
                    continue;
                }
                None => {
                    let previous = ledger.clone();
                    ledger = self.bootstrap(Some(&previous), &mut cleanup)?;
                    entered = false;
                    continue;
                }
            };
            if entered && ledger.events == 0 && !names.iter().any(|name| name == SNAPSHOT_NAME) {
                self.dav
                    .create(&snapshot_path(ledger.epoch), &ledger.snapshot())?;
            }
            entered = false;
            let seqs: BTreeSet<u64> = names.iter().filter_map(|name| event_name(name)).collect();
            while seqs.contains(&(ledger.seq + 1)) {
                // 列出来却还读不到的事件当作还没写好，下次再读。
                let Some(bytes) = self
                    .dav
                    .get(&event_path(ledger.epoch, ledger.seq + 1), MAX_EVENT_BYTES)?
                else {
                    break;
                };
                ledger.apply(&bytes).map_err(|error| match error {
                    LogError::Chain => RelayError::Rollback,
                    _ => RelayError::Unavailable,
                })?;
                if ledger.sealed {
                    break;
                }
            }
            if ledger.sealed {
                continue;
            }
            self.store(ledger, cleanup);
            return Ok(());
        }
        Err(RelayError::Unavailable)
    }

    /// 从网盘上最新的快照开始。`previous` 是本机原来的位置（它所在的 epoch 已经没了）：网盘上的位置、密钥代数不许比它小，序号相同时链头也要相同。网盘上还没有这份日志时建好目录和第一份快照；但本机已经读到过事件时不重建，那是有人把日志删了。
    fn bootstrap(
        &mut self,
        previous: Option<&Ledger>,
        cleanup: &mut Vec<u64>,
    ) -> Result<Ledger, RelayError> {
        let seen = previous.is_some_and(|ledger| ledger.seq > 0 || ledger.epoch > 0);
        let names = match self.dav.list(V1)? {
            Some(names) => names,
            None if seen => return Err(RelayError::Rollback),
            None => {
                self.dav.mkcol(ROOT)?;
                self.dav.mkcol(V1)?;
                self.dav.list(V1)?.ok_or(RelayError::Unavailable)?
            }
        };
        if !names.iter().any(|name| name == "tmp") {
            self.dav.mkcol(TMP)?;
        }
        if names.iter().any(|name| name == FORMAT_NAME) {
            if previous.is_none() {
                #[derive(Deserialize)]
                struct Format {
                    format: String,
                    version: u32,
                }
                let bytes = self
                    .dav
                    .get(FORMAT_FILE, MAX_SMALL_BYTES)?
                    .ok_or(RelayError::Unavailable)?;
                let format: Format =
                    serde_json::from_slice(&bytes).map_err(|_| RelayError::Unsupported)?;
                if format.format != crate::cloud::clipboard_log::FORMAT
                    || format.version != crate::cloud::clipboard_log::FORMAT_VERSION
                {
                    return Err(RelayError::Unsupported);
                }
            }
        } else {
            self.dav.create(FORMAT_FILE, &format_bytes())?;
        }
        let mut epochs: Vec<u64> = names.iter().filter_map(|name| epoch_name(name)).collect();
        epochs.sort_unstable_by(|a, b| b.cmp(a));
        if epochs.is_empty() {
            if seen {
                return Err(RelayError::Rollback);
            }
            let genesis = Ledger::genesis();
            self.dav.mkcol(&epoch_dir(0))?;
            self.dav.create(&snapshot_path(0), &genesis.snapshot())?;
            epochs.push(0);
        }
        for &epoch in &epochs {
            let Some(bytes) = self.dav.get(&snapshot_path(epoch), MAX_SNAPSHOT_BYTES)? else {
                // 封口的设备建了目录还没写快照：从上一个 epoch 读到封口，再由本机补写。
                continue;
            };
            let ledger =
                Ledger::from_snapshot(epoch, &bytes).map_err(|_| RelayError::Unavailable)?;
            if let Some(previous) = previous {
                if ledger.epoch < previous.epoch
                    || ledger.seq < previous.seq
                    || ledger.state.generation < previous.state.generation
                    || (ledger.seq == previous.seq && ledger.head != previous.head)
                {
                    return Err(RelayError::Rollback);
                }
            }
            cleanup.extend(epochs.iter().copied().filter(|older| *older < epoch));
            return Ok(ledger);
        }
        Err(RelayError::Unavailable)
    }

    /// 写完之后顺手做的事，失败不影响这一次的回答：该压缩时压缩，旧 epoch 过了宽限期就删掉。
    fn maintain(&mut self) {
        let now = self.now();
        if self
            .stored
            .as_ref()
            .is_some_and(|stored| stored.ledger.compaction_due(now))
        {
            let _ = self.compact();
        }
        let _ = self.cleanup(now);
    }

    /// 压缩：一个 `seal` 事件抢下一个序号关闭这个 epoch，建下一个 epoch 的目录，写它的快照。抢不到说明另一台设备刚写过，下次再说。
    fn compact(&mut self) -> Result<(), RelayError> {
        let mut ledger = self.ledger()?.clone();
        let seal = ledger.seal(&Draft::random(self.now()).map_err(|_| RelayError::Unavailable)?);
        if !self
            .dav
            .create(&event_path(ledger.epoch, ledger.seq + 1), &seal)?
        {
            return Ok(());
        }
        ledger.apply(&seal).map_err(|_| RelayError::Unavailable)?;
        // 先记下封口，后面的步骤失败时下一次同步会接着做完。
        if let Some(stored) = self.stored.as_mut() {
            stored.ledger = ledger.clone();
        }
        self.dirty = true;
        let next = ledger.next_epoch().ok_or(RelayError::Unavailable)?;
        self.dav.mkcol(&epoch_dir(next.epoch))?;
        self.dav
            .create(&snapshot_path(next.epoch), &next.snapshot())?;
        let not_before = self.now() + OLD_EPOCH_GRACE_SECS;
        if let Some(stored) = self.stored.as_mut() {
            stored.ledger = next;
        }
        self.schedule_cleanup(ledger.epoch, not_before);
        Ok(())
    }

    /// 删掉过了宽限期的旧 epoch，顺带清掉中断的写入留下的临时文件。
    fn cleanup(&mut self, now: i64) -> Result<(), RelayError> {
        let Some(stored) = self.stored.as_ref() else {
            return Ok(());
        };
        let current = stored.ledger.epoch;
        let due: Vec<u64> = stored
            .cleanup
            .iter()
            .filter(|entry| entry.not_before <= now && entry.epoch < current)
            .map(|entry| entry.epoch)
            .collect();
        if due.is_empty() {
            return Ok(());
        }
        for epoch in due {
            // 网盘拒绝删除时也不再试：留着的旧 epoch 不影响读写。
            self.dav.delete(&epoch_dir(epoch))?;
            if let Some(stored) = self.stored.as_mut() {
                stored.cleanup.retain(|entry| entry.epoch != epoch);
            }
            self.dirty = true;
        }
        if let Some(names) = self.dav.list(TMP)? {
            for name in names {
                if tmp_name(&name).is_some_and(|written| written < now - STALE_TMP_SECS) {
                    self.dav.delete(&format!("{TMP}{name}"))?;
                }
            }
        }
        Ok(())
    }
}

// ---- 测试连接 ----

/// 「测试连接」：在用户选的文件夹里试一遍日志要用的网盘行为，通过了才让保存。建一个探测用的子目录；独占创建一次要成功、同名再来一次要失败、内容是第一次的；刚写的文件马上列得出来；几个写入同时抢一个名字，只有一个成功、内容是它的；最后删掉探测目录。不合格是 [`RelayError::Unsupported`]，账号或密码不对是 [`RelayError::Unauthorized`]。这只证明网盘有这些语义，证明不了任何负载下都是原子的。会联网，大约二十个请求。
pub fn probe(config: &RelayConfig) -> Result<(), RelayError> {
    let http = HttpDav::new(config)?;
    probe_with(&http, &SystemEnvironment)
}

/// [`probe`]，通过之后再删掉 `directory` 里的日志缓存（[`WEBDAV_STATE_FILE`]）：用户在设置里重新确认了这个网盘，防回退从网盘现在的样子重新开始。用户自己清理过网盘上的文件夹、本机因此报 `webdav_rollback` 时，这是恢复的办法。
pub fn probe_and_trust(
    config: &RelayConfig,
    directory: Option<&std::path::Path>,
) -> Result<(), RelayError> {
    if directory.is_some_and(|directory| !directory.is_absolute()) {
        return Err(RelayError::Invalid);
    }
    let http = HttpDav::new(config)?;
    trust_after(probe_with(&http, &SystemEnvironment), directory)
}

/// 测试连接通过了才删缓存；没通过时本机记得的位置原样留着。
fn trust_after(
    probed: Result<(), RelayError>,
    directory: Option<&std::path::Path>,
) -> Result<(), RelayError> {
    probed?;
    let Some(directory) = directory else {
        return Ok(());
    };
    if !directory.exists() {
        return Ok(());
    }
    let _lock = lock_directory(directory)?;
    match std::fs::remove_file(directory.join(WEBDAV_STATE_FILE)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(RelayError::Storage),
    }
}

pub(crate) fn probe_with(http: &dyn DavHttp, env: &dyn Environment) -> Result<(), RelayError> {
    let recording = Recording {
        inner: http,
        unreachable: std::sync::atomic::AtomicBool::new(false),
    };
    let dav = Dav::new(&recording, env);
    let result = (|| {
        dav.mkcol(ROOT)?;
        let dir = format!("{ROOT}probe-{}/", random_hex::<8>()?);
        if !dav.mkcol(&dir)? {
            return Err(RelayError::Unsupported);
        }
        let result = probe_inside(&dav, &dir);
        let _ = dav.delete(&dir);
        result
    })();
    // 网盘答了、但答得不对（例如 MKCOL 答 403、MOVE 答 500）是网盘不合格；根本没连上才是连不上。
    result.map_err(|error| match error {
        RelayError::Unavailable
            if !recording
                .unreachable
                .load(std::sync::atomic::Ordering::SeqCst) =>
        {
            RelayError::Unsupported
        }
        error => error,
    })
}

/// 记下有没有请求根本没发出去或没拿到响应，好把「连不上」和「网盘答得不对」分开。
struct Recording<'a> {
    inner: &'a dyn DavHttp,
    unreachable: std::sync::atomic::AtomicBool,
}

impl DavHttp for Recording<'_> {
    fn send(&self, request: &DavRequest<'_>) -> Result<DavResponse, RelayError> {
        let response = self.inner.send(request);
        if response.is_err() {
            self.unreachable
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        response
    }
}

fn probe_inside(dav: &Dav<'_>, dir: &str) -> Result<(), RelayError> {
    let target = format!("{dir}once.json");
    if !dav.create_in(dir, &target, b"first")? {
        return Err(RelayError::Unsupported);
    }
    // 同名再创建一次必须失败，目标还是第一次的内容（坚果云不理会 `If-None-Match`，就是在这里露馅的那种网盘）。
    if dav.create_in(dir, &target, b"second")? {
        return Err(RelayError::Unsupported);
    }
    if dav.get(&target, MAX_SMALL_BYTES)?.as_deref() != Some(b"first".as_slice()) {
        return Err(RelayError::Unsupported);
    }
    let names = dav.list(dir)?.ok_or(RelayError::Unsupported)?;
    if !names.iter().any(|name| name == "once.json") {
        return Err(RelayError::Unsupported);
    }
    let race = format!("{dir}race.json");
    let outcomes: Vec<Result<bool, RelayError>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..PROBE_RACERS)
            .map(|index| {
                let race = race.as_str();
                scope.spawn(move || dav.create_in(dir, race, format!("racer-{index}").as_bytes()))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap_or(Err(RelayError::Unavailable)))
            .collect()
    });
    let mut winners = Vec::new();
    for (index, outcome) in outcomes.into_iter().enumerate() {
        if outcome? {
            winners.push(index);
        }
    }
    let [winner] = winners.as_slice() else {
        return Err(RelayError::Unsupported);
    };
    if dav.get(&race, MAX_SMALL_BYTES)? != Some(format!("racer-{winner}").into_bytes()) {
        return Err(RelayError::Unsupported);
    }
    Ok(())
}

#[cfg(test)]
#[path = "clipboard_webdav_tests.rs"]
mod tests;
