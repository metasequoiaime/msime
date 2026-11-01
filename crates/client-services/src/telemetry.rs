//! Anonymous usage reporting to `POST https://api.msime.app/v1/telemetry/events`.
//!
//! Every host records into one directory it owns and flushes from a background thread; nothing here touches the network except [`TelemetryStore::flush`]. What is recorded:
//!
//! - `active`: at most once per UTC day per installation, with the deterministic id `active-<install_id>-<yyyymmdd>`, so a duplicate costs nothing.
//! - `session`: one per host process lifetime that ended through [`TelemetryStore::end_session`]. It is queued at the end and sent on a later flush.
//! - `session_crash`：一个开始过、没有正常结束、并且留下了自己崩溃记录的会话。只留下标记（进程被系统杀掉、注销、关机、内存不足）不算崩溃，不产生事件。同一目录可能同时有几个宿主进程各自的会话（例如同一用户的两个图形会话），所以这里按会话 id 认领崩溃记录，而不是只看最后写下的那个标记，见 `.agents/notes/implemented/bug-fix/2026-10-24-telemetry-session-ownership.md`。
//! - `crash`: a crash record the host's crash handler wrote to disk, turned into an event on the next start. The handler only writes a file ([`TelemetryStore::record_crash`], or a raw write to the path [`TelemetryStore::begin_session`] returned, which is what an async-signal handler can do); it never sends.
//! - 系统事后报告的崩溃（iOS 的 MetricKit 诊断，在 App 下次打开时才送到）经 [`TelemetryStore::record_diagnosed_crash`] 按进程号认领：进程当时有未结束的会话，就写成那个会话的崩溃记录（`crash` 加 `session_crash`）；那个会话已经由进程自己的崩溃处理程序记过，就不再写第二份；没有会话的（App 自身、键盘收起后被挂起的进程）只报 `crash`。系统强制结束（SIGKILL：看门狗、内存上限、挂起时持有共享容器的文件锁）按上面的规则不算会话崩溃，由宿主不带进程号调用，只报 `crash`。见 `.agents/notes/implemented/bug-fix/2026-10-25-ios-crash-diagnostic-session-attribution.md`。
//!
//! Clients never send `download`: release downloads are counted on the server and installs by `active`.
//!
//! Every event carries `install_id`, an anonymous random id generated once per installation and stored in this directory. It is never derived from the hardware, the account or user data. Crash messages and stacks have every directory part of a path removed before they are queued, so a user or folder name cannot leave the machine.
//!
//! The queue keeps at most [`MAX_QUEUED_EVENTS`] events. A `400`-class rejection drops an event for good; `429`, `5xx` and network failures keep it for a later flush with the same id, and a `Retry-After` delay is honoured across processes.

use crate::account::{AccountError, BackendAccountClient};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
#[cfg(not(unix))]
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

mod consent;
pub use consent::{
    clear_if_authority_disabled, clear_if_consent_disabled, with_consent_epoch, ConsentEpochSender,
};
mod identity;
pub use identity::CrashContext;
mod transport;
pub use transport::backend_sender;
#[cfg(feature = "telemetry-loopback")]
pub use transport::isolated_origin;
#[cfg(test)]
mod consent_tests;

/// The queue, a JSON array of events.
pub const QUEUE_FILE: &str = "telemetry.json";
/// The installation id, the last day `active` was queued, and any `Retry-After` deadline.
pub const STATE_FILE: &str = "telemetry-state.json";
/// The marker of the running session.
pub const SESSION_FILE: &str = "telemetry-session.json";
/// Crash records, one `<session id>.crash` file per crashed session.
pub const CRASH_DIRECTORY: &str = "telemetry-crashes";
pub(crate) const CRASH_EXTENSION: &str = "crash";
const LOCK_FILE: &str = "telemetry.lock";

/// Most events kept for delivery; the oldest are dropped first.
pub const MAX_QUEUED_EVENTS: usize = 64;

fn telemetry_queue_capacity(input_len: usize) -> usize {
    input_len.min(MAX_QUEUED_EVENTS)
}

/// The server's limit on a crash message, in Unicode scalar values.
pub const MAX_MESSAGE_CHARS: usize = 1000;
/// The server's limit on a crash stack, in Unicode scalar values.
pub const MAX_STACK_CHARS: usize = 16_000;
/// The client's limit on the UTF-8 size of a stack, so that a whole event stays well inside the server's 32 KiB body limit.
pub const MAX_STACK_BYTES: usize = 12 * 1024;
/// The server's request body limit.
pub const MAX_EVENT_BYTES: usize = 32 * 1024;

const EVENTS_PATH: &str = "/v1/telemetry/events";
const SEND_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_QUEUE_BYTES: u64 = 1 << 20;
const MAX_SMALL_FILE_BYTES: u64 = 16 * 1024;
// 16 个最大构建头和 128 条收据一起保存时可超过单会话标记的 16 KiB；状态独立限长。
const MAX_STATE_FILE_BYTES: u64 = 32 * 1024;
pub(crate) const MAX_CRASH_RECORD_BYTES: u64 = 64 * 1024;
/// Crash records turned into events per start; a host that crashes in a loop cannot fill the queue with one start's worth of records.
const MAX_CRASH_RECORDS_PER_START: usize = 8;
/// 读不出的崩溃记录保留这么久，覆盖短期降级后再升级的情况，之后删除。
const STALE_UNREADABLE_CRASH: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// 记住的未结束会话上限。超过时淘汰最旧的：被淘汰的会话之后再留下崩溃记录，只报 `crash`，不再报 `session_crash`。
const MAX_OPEN_SESSIONS: usize = 16;
/// 记住的、已经补报过 `session_crash` 的进程号上限，用来认出系统事后为同一次崩溃再送来的诊断。
const MAX_CRASHED_PROCESSES: usize = 16;
/// 最近已持久入队的故障 UUID，独立于发送队列保留，覆盖 Android 每次读取的 16 项退出历史及收件重试。
const MAX_RECEIVED_CRASHES: usize = 128;
/// A `Retry-After` beyond this is treated as this, so a malformed header cannot silence reporting for good.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(24 * 60 * 60);
/// The delay after a `429` without a usable `Retry-After`.
const DEFAULT_RETRY_AFTER: Duration = Duration::from_secs(60);

/// The canonical platform ids the server groups by.
pub const PLATFORMS: [&str; 6] = ["windows", "macos", "linux", "android", "ios", "harmony"];

/// The canonical id for `value`, accepting the aliases the server also maps (`win`, `mac`, `darwin`, `ipados`, `harmonyos`, `ohos`).
pub fn canonical_platform(value: &str) -> Option<&'static str> {
    Some(match value.trim().to_ascii_lowercase().as_str() {
        "windows" | "win" => "windows",
        "macos" | "mac" | "darwin" => "macos",
        "linux" => "linux",
        "android" => "android",
        "ios" | "ipados" => "ios",
        "harmony" | "harmonyos" | "ohos" => "harmony",
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryKind {
    Active,
    Session,
    SessionCrash,
    Crash,
}

/// One event as the server accepts it. Empty optional fields are left out of the body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetryEvent {
    pub id: String,
    pub kind: TelemetryKind,
    pub platform: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub message: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub stack: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub artifact: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub channel: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub install_id: String,
}

impl TelemetryEvent {
    /// The server's acceptance rules, applied before queueing so an event that would only ever be rejected never waits in the queue.
    pub fn is_valid(&self) -> bool {
        let single_line = |value: &str, minimum: usize, maximum: usize| {
            crate::community::valid_text(value, minimum, maximum, false)
                && value.trim().chars().count() >= minimum
        };
        let crash = self.kind == TelemetryKind::Crash;
        single_line(&self.id, 16, 128)
            && canonical_platform(&self.platform) == Some(self.platform.as_str())
            && single_line(&self.version, 1, 64)
            && (!crash || !self.message.trim().is_empty())
            && (crash || (self.message.is_empty() && self.stack.is_empty()))
            && crate::community::valid_text(&self.message, 0, MAX_MESSAGE_CHARS, true)
            && crate::community::valid_text(&self.stack, 0, MAX_STACK_CHARS, true)
            && (self.artifact.is_empty() || single_line(&self.artifact, 1, 64))
            && (self.channel.is_empty() || valid_channel(&self.channel))
            && (self.install_id.is_empty() || valid_install_id(&self.install_id))
            && (self.kind != TelemetryKind::Active || !self.install_id.is_empty())
    }
}

fn valid_channel(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=32).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_' || *byte == b'-'
        })
}

/// 16 to 64 characters of `[A-Za-z0-9_-]`, the server's rule for `install_id`.
pub fn valid_install_id(value: &str) -> bool {
    (16..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

/// The host recording events: its canonical platform and real version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TelemetryApp {
    platform: &'static str,
    version: String,
    component: String,
}

impl TelemetryApp {
    pub fn new(platform: &str, version: &str) -> Result<Self, TelemetryError> {
        let platform = canonical_platform(platform).ok_or(TelemetryError::Invalid)?;
        let version = version.trim();
        if !crate::community::valid_text(version, 1, 64, false) {
            return Err(TelemetryError::Invalid);
        }
        Ok(Self {
            platform,
            version: version.to_owned(),
            component: "input".to_owned(),
        })
    }

    pub fn platform(&self) -> &'static str {
        self.platform
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn with_component(mut self, component: &str) -> Result<Self, TelemetryError> {
        CrashContext::new(&self, component)?;
        self.component = component.to_owned();
        Ok(self)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TelemetryError {
    #[error("invalid telemetry input")]
    Invalid,
    #[error("telemetry storage is unavailable")]
    Storage,
    #[error("telemetry consent changed")]
    ConsentChanged,
}

impl From<std::io::Error> for TelemetryError {
    fn from(_: std::io::Error) -> Self {
        Self::Storage
    }
}

/// What a host learns when its session starts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SessionStart {
    /// The file a crash handler writes this session's crash record to: UTF-8 text, the first line the exception or signal summary, the following lines the stack. The directory already exists, so an async-signal handler only needs `open(path, O_WRONLY|O_CREAT|O_EXCL, 0600)` and `write`. An existing record is never replaced.
    pub crash_record_path: PathBuf,
    /// 采集前计算的版本化记录头。信号处理函数直接写这些字节，再写摘要和调用栈，不在崩溃现场序列化。
    pub crash_record_header: String,
    /// 这次开始的会话 id。宿主用它结束自己的会话（[`TelemetryStore::end_session_by_id`]）和写自己的崩溃记录（[`TelemetryStore::record_crash_for`]），不受同目录里别的进程后来写下的标记影响。
    pub session_id: String,
    /// 这次启动是否为之前的会话补报了 `session_crash`（它们留下了崩溃记录）。
    pub previous_session_crashed: bool,
    /// Crash records queued as `crash` events.
    pub crashes: usize,
}

/// 系统事后报告的一次崩溃（[`TelemetryStore::record_diagnosed_crash`]）记到了哪里。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosedCrash {
    /// 写成了该进程未结束会话的崩溃记录：下次开始会话时报 `crash` 和这个会话的 `session_crash`。
    Session,
    /// 不属于任何未结束的会话，写成单独的记录：下次开始会话时只报 `crash`。
    Outside,
    /// 该进程这次崩溃已经由它自己的崩溃处理程序记下（或已经报过），没有再写。
    Duplicate,
}

/// The result of one delivery attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// The server took the event (`202`, also for an id it already had).
    Accepted,
    /// The server refused the event itself (`400`): it would be refused again, so it is dropped.
    Rejected,
    /// Rate limited or the service is down: keep the event and wait at least this long.
    RetryAfter(Option<Duration>),
    /// No answer or an unexpected one: keep the event for a later flush.
    Failed,
}

/// Sends one serialized event. [`BackendAccountClient`] is the production sender; the trait lets the queue logic be tested without a server.
pub trait TelemetrySender {
    fn send(&self, body: &[u8]) -> Delivery;
}

impl TelemetrySender for BackendAccountClient {
    fn send(&self, body: &[u8]) -> Delivery {
        match self.post_for_status(EVENTS_PATH, body.to_vec(), MAX_EVENT_BYTES, SEND_TIMEOUT) {
            Ok((status, retry_after)) => delivery_for_status(status, retry_after),
            Err(AccountError::Invalid) => Delivery::Rejected,
            Err(_) => Delivery::Failed,
        }
    }
}

pub(crate) fn delivery_for_status(status: StatusCode, retry_after: Option<Duration>) -> Delivery {
    match status.as_u16() {
        200..=299 => Delivery::Accepted,
        400 | 413 | 415 | 422 => Delivery::Rejected,
        429 | 503 => Delivery::RetryAfter(retry_after),
        _ => Delivery::Failed,
    }
}

/// What a flush did.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct FlushReport {
    pub sent: usize,
    pub dropped: usize,
    pub remaining: usize,
    /// True when an earlier `Retry-After` was still running and nothing was attempted.
    pub deferred: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TelemetryState {
    /// 绑定这份本地队列的持续同意代次；旧版文档没有此字段。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    consent_epoch: Option<Uuid>,
    /// 已接入持久授权的目录在撤回后仍拒绝未绑定的正常入口。
    #[serde(default)]
    consent_required: bool,
    #[serde(default)]
    install_id: String,
    /// `yyyymmdd` (UTC) of the last queued `active`.
    #[serde(default)]
    active_day: String,
    #[serde(default)]
    retry_after_unix_ms: u64,
    /// 已经开始、既没有正常结束也没有因崩溃记录被关闭的会话，旧的在前。其中可能有仍在运行的别的进程的会话，也有被杀掉、永远不会再有下文的会话；两者从文件上分不出来，所以只按数量（[`MAX_OPEN_SESSIONS`]）淘汰最旧的。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    open_sessions: Vec<SessionMarker>,
    /// 最近因崩溃记录补报过 `session_crash` 的会话所在的进程号，旧的在前，最多 [`MAX_CRASHED_PROCESSES`] 个。系统事后送来同一进程的诊断时据此认出它已经报过。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    crashed_processes: Vec<u32>,
    /// 仅本机保存；入队和此收据均写盘后才删除原记录，撤回时一起清空。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    received_crashes: Vec<Uuid>,
}

impl TelemetryState {
    /// 远于最大退避窗口的截止时间来自旧时钟或异常状态，不能持续挡住队列。
    fn retry_deadline(&self, now_ms: u64) -> Option<u64> {
        let remaining = self.retry_after_unix_ms.checked_sub(now_ms)?;
        (remaining > 0 && Duration::from_millis(remaining) <= MAX_RETRY_AFTER)
            .then_some(self.retry_after_unix_ms)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SessionMarker {
    id: String,
    platform: String,
    version: String,
    started_at_unix_ms: u64,
    /// 开始这个会话的进程号，只留在本机，用来把系统事后报告的崩溃对回会话。旧版本写下的标记没有这一项。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    context: Option<CrashContext>,
}

impl SessionMarker {
    fn is_valid(&self) -> bool {
        Uuid::parse_str(&self.id).is_ok()
            && canonical_platform(&self.platform) == Some(self.platform.as_str())
            && crate::community::valid_text(&self.version, 1, 64, false)
            && self.context.as_ref().is_none_or(CrashContext::is_valid)
    }
}

/// The telemetry files of one host, all under one directory the host owns.
#[derive(Clone, Debug)]
pub struct TelemetryStore {
    directory: PathBuf,
    consent_epoch: Option<Uuid>,
}

/// 只有队列、会话状态和新标记都持久化后才确认删除原记录；失败重试沿用相同事件 id。
#[derive(Default)]
struct CollectedCrashes {
    records: Vec<PathBuf>,
    sessions: Vec<String>,
    crashes: usize,
    session_crashed: bool,
}

impl CollectedCrashes {
    fn acknowledge(&self, store: &TelemetryStore) -> Result<(), TelemetryError> {
        // 补报时没有新会话替换旧标记；先清除已关闭的那个，避免下次补报把它重新认作未结束会话。
        if store
            .read_marker()
            .is_some_and(|marker| self.sessions.contains(&marker.id))
        {
            remove_file(&store.directory.join(SESSION_FILE))?;
        }
        for path in &self.records {
            remove_file(path)?;
        }
        Ok(())
    }
}

impl TelemetryStore {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            consent_epoch: None,
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// 调用方在偏好锁内确认代次后激活；锁顺序是偏好、队列，不能反过来。
    /// 新代次清理旧报告但保留匿名安装 ID，返回的存储拒绝之后的旧代次操作。
    pub fn activate_consent_epoch(&self, epoch: Uuid) -> Result<Self, TelemetryError> {
        if epoch.get_version() != Some(uuid::Version::Random)
            || epoch.get_variant() != uuid::Variant::RFC4122
        {
            return Err(TelemetryError::Invalid);
        }
        let unbound = Self::new(self.directory.clone());
        let _lock = unbound.lock()?;
        let mut state = unbound.read_state();
        if state.consent_epoch != Some(epoch) {
            unbound.clear_locked()?;
            state = unbound.read_state();
        }
        if state.consent_epoch != Some(epoch) || !state.consent_required {
            state.consent_epoch = Some(epoch);
            state.consent_required = true;
            unbound.write_state(&state)?;
        }
        Ok(Self {
            directory: self.directory.clone(),
            consent_epoch: Some(epoch),
        })
    }

    fn crash_context(&self, app: &TelemetryApp) -> Result<CrashContext, TelemetryError> {
        let mut context = CrashContext::new(app, &app.component)?;
        context.consent_epoch = self.consent_epoch;
        Ok(context)
    }

    /// The anonymous installation id, generated and stored on first use.
    pub fn install_id(&self) -> Result<String, TelemetryError> {
        let _lock = self.lock()?;
        let mut state = self.read_state();
        let id = self.ensure_install_id(&mut state)?;
        Ok(id)
    }

    /// 开始一个会话：为每个留下了崩溃记录的未结束会话补报 `session_crash`（只留下标记的不算），把所有崩溃记录排成 `crash`，排进当天的 `active`，写下新会话的标记并把它记为未结束。不联网。
    pub fn begin_session(
        &self,
        app: &TelemetryApp,
        now: SystemTime,
    ) -> Result<SessionStart, TelemetryError> {
        self.begin_session_in_process(app, now, std::process::id())
    }

    pub(crate) fn begin_session_in_process(
        &self,
        app: &TelemetryApp,
        now: SystemTime,
        pid: u32,
    ) -> Result<SessionStart, TelemetryError> {
        let _lock = self.lock_normal_operation()?;
        let mut state = self.read_state();
        let install_id = self.ensure_install_id(&mut state)?;
        let mut queue = self.read_queue()?;
        let crashes_directory = self.directory.join(CRASH_DIRECTORY);
        crate::storage::create_directory_and_check(&crashes_directory)?;
        // 旧版本只写标记、不记未结束列表；标记里的会话照样算一个未结束会话。
        if let Some(previous) = self.read_marker() {
            if !state
                .open_sessions
                .iter()
                .any(|session| session.id == previous.id)
            {
                state.open_sessions.push(previous);
            }
        }
        let collected = self.ingest_crash_records(
            &crashes_directory,
            app,
            &mut state,
            &install_id,
            &mut queue,
        )?;
        push_active(&mut state, &mut queue, app, &install_id, now);
        let marker = SessionMarker {
            id: Uuid::new_v4().hyphenated().to_string(),
            platform: app.platform.to_owned(),
            version: app.version.clone(),
            started_at_unix_ms: unix_ms(now),
            pid: Some(pid),
            context: Some(self.crash_context(app)?),
        };
        state.open_sessions.push(marker.clone());
        if state.open_sessions.len() > MAX_OPEN_SESSIONS {
            state
                .open_sessions
                .drain(..state.open_sessions.len() - MAX_OPEN_SESSIONS);
        }
        self.write_queue(&queue)?;
        self.write_state(&state)?;
        self.write_json(SESSION_FILE, &marker)?;
        collected.acknowledge(self)?;
        Ok(SessionStart {
            crash_record_path: crashes_directory.join(format!("{}.{CRASH_EXTENSION}", marker.id)),
            crash_record_header: marker
                .context
                .as_ref()
                .expect("new marker has context")
                .record_header(),
            session_id: marker.id,
            previous_session_crashed: collected.session_crashed,
            crashes: collected.crashes,
        })
    }

    /// 为独立应用、设置或辅助进程准备采集位置，不建立输入会话，也不改写其它进程的会话标记。
    pub fn prepare_crash_capture(
        &self,
        app: &TelemetryApp,
        now: SystemTime,
    ) -> Result<SessionStart, TelemetryError> {
        self.record_active(app, now)?;
        let id = Uuid::new_v4().hyphenated().to_string();
        Ok(SessionStart {
            crash_record_path: self
                .directory
                .join(CRASH_DIRECTORY)
                .join(format!("{id}.{CRASH_EXTENSION}")),
            crash_record_header: self.crash_context(app)?.record_header(),
            session_id: id,
            previous_session_crashed: false,
            crashes: 0,
        })
    }

    /// 正常结束标记里记着的会话：排进它的 `session` 事件并删掉标记。没有会话在运行时返回 false（开始时上报是关的，或已经结束过）。不联网，可以在退出路径上调用。同一目录里可能有别的进程后来开始的会话，标记就不再是自己的了；知道自己会话 id 的宿主应改用 [`Self::end_session_by_id`]。
    pub fn end_session(&self) -> Result<bool, TelemetryError> {
        self.end(None)
    }

    /// 正常结束 `id` 这个会话：排进它的 `session` 事件，把它移出未结束列表；标记只在仍是它的时候才删，别的进程后来开始的会话不受影响。`id` 不是未结束的会话时返回 false。
    pub fn end_session_by_id(&self, id: &str) -> Result<bool, TelemetryError> {
        self.end(Some(id))
    }

    fn end(&self, id: Option<&str>) -> Result<bool, TelemetryError> {
        let _lock = if id.is_some() {
            self.lock()?
        } else {
            self.lock_normal_operation()?
        };
        let marker = self.read_marker();
        let Some(id) = id
            .map(str::to_owned)
            .or_else(|| marker.as_ref().map(|marker| marker.id.clone()))
        else {
            return Ok(false);
        };
        let mut state = self.read_state();
        let open = state
            .open_sessions
            .iter()
            .position(|session| session.id == id);
        let session = match open {
            Some(index) => state.open_sessions.remove(index),
            None => match marker.as_ref().filter(|marker| marker.id == id) {
                Some(marker) => marker.clone(),
                None => return Ok(false),
            },
        };
        let install_id = self.ensure_install_id(&mut state)?;
        let mut queue = self.read_queue()?;
        push(
            &mut queue,
            TelemetryEvent {
                id: format!("session-{}", session.id),
                kind: TelemetryKind::Session,
                platform: session.platform,
                version: session.version,
                message: String::new(),
                stack: String::new(),
                artifact: String::new(),
                channel: String::new(),
                install_id,
            },
        );
        self.write_queue(&queue)?;
        if open.is_some() {
            self.write_state(&state)?;
        }
        if marker.is_some_and(|marker| marker.id == id) {
            remove_file(&self.directory.join(SESSION_FILE))?;
        }
        Ok(true)
    }

    /// 为标记里记着的会话写崩溃记录，供可以分配内存的崩溃处理（C++ terminate handler、未捕获异常钩子）使用：不加锁，只建一个文件。没有会话在运行，或这个会话已经有记录时返回 false（保留第一份：terminate handler 随后 abort，SIGABRT 处理函数不能把它的摘要换掉）。消息和栈在下次启动排队时清理，这里不处理。标记可能已经属于同目录里别的进程，知道自己会话 id 的宿主应改用 [`Self::record_crash_for`]。
    pub fn record_crash(&self, message: &str, stack: &str) -> Result<bool, TelemetryError> {
        let Some(marker) = self.read_marker() else {
            return Ok(false);
        };
        self.record_crash_for(&marker.id, message, stack)
    }

    /// 为 `session` 这个会话写崩溃记录，写到 [`SessionStart::crash_record_path`] 那个文件，不看标记现在属于谁；其余同 [`Self::record_crash`]。`session` 不是会话 id 时返回 [`TelemetryError::Invalid`]。
    pub fn record_crash_for(
        &self,
        session: &str,
        message: &str,
        stack: &str,
    ) -> Result<bool, TelemetryError> {
        let context = self
            .read_state()
            .open_sessions
            .into_iter()
            .chain(self.read_marker())
            .find(|marker| marker.id == session)
            .and_then(|marker| marker.context);
        self.record_crash_with_context(session, context.as_ref(), message, stack)
    }

    /// 将预先固定的构建身份写入记录；不加锁，可以用于允许分配内存的异常回调。
    pub fn record_crash_with_context(
        &self,
        session: &str,
        context: Option<&CrashContext>,
        message: &str,
        stack: &str,
    ) -> Result<bool, TelemetryError> {
        if context.is_some_and(|context| !context.is_valid()) {
            return Err(TelemetryError::Invalid);
        }
        if self.consent_epoch.is_some()
            && context.and_then(|context| context.consent_epoch) != self.consent_epoch
        {
            return Err(TelemetryError::ConsentChanged);
        }
        let session = Uuid::parse_str(session).map_err(|_| TelemetryError::Invalid)?;
        let directory = self.directory.join(CRASH_DIRECTORY);
        crate::storage::create_directory_and_check(&directory)?;
        let name = format!("{}.{CRASH_EXTENSION}", session.hyphenated());
        let mut contents = context.map(CrashContext::record_header).unwrap_or_default();
        contents.push_str(message.lines().next().unwrap_or_default());
        contents.push('\n');
        contents.push_str(stack);
        // 完整写盘后才原子发布；另一个进程补报时不能读走半份异常记录。
        #[cfg(unix)]
        {
            let directory = crate::storage::open_private_directory(&directory)?;
            Ok(crate::storage::write_private_file_at_noclobber(
                &directory,
                std::ffi::OsStr::new(&name),
                contents.as_bytes(),
            )?)
        }
        #[cfg(not(unix))]
        {
            let mut file = tempfile::NamedTempFile::new_in(&directory)?;
            file.write_all(contents.as_bytes())?;
            file.as_file().sync_all()?;
            match file.persist_noclobber(directory.join(name)) {
                Ok(_) => Ok(true),
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
                Err(error) => Err(error.error.into()),
            }
        }
    }

    /// 系统退出与现场故障共用捕获 UUID；收据在队列发送后仍可识别重复，不能用于故障处理器。
    pub fn record_diagnosed_capture(
        &self,
        capture: &str,
        context: &CrashContext,
        message: &str,
        stack: &str,
    ) -> Result<bool, TelemetryError> {
        let _lock = self.lock_normal_operation()?;
        let id = Uuid::parse_str(capture).map_err(|_| TelemetryError::Invalid)?;
        if self.read_state().received_crashes.contains(&id) {
            return Ok(false);
        }
        self.record_crash_with_context(capture, Some(context), message, stack)
    }

    /// 记下系统事后报告的一次崩溃（iOS 的 MetricKit 诊断）。`pid` 是崩溃进程的进程号：它有未结束的会话时写成最近开始的那个会话的崩溃记录；这个进程的崩溃已经由它自己的崩溃处理程序记下（记录还在，或已经补报过 `session_crash`）时什么都不写，一次崩溃只报一次；都不是时写成不属于任何会话的记录。`pid` 为 `None` 用于系统强制结束（SIGKILL）：按会话语义它不算会话崩溃，总是写成单独的记录。不联网；会加锁，不能在崩溃处理程序里调用。
    pub fn record_diagnosed_crash(
        &self,
        pid: Option<u32>,
        message: &str,
        stack: &str,
    ) -> Result<DiagnosedCrash, TelemetryError> {
        self.record_diagnosed_crash_with_context(pid, None, None, message, stack)
    }

    /// 系统延迟诊断可提供原构建身份；未结束会话里的身份比补录的诊断信息优先。
    pub fn record_diagnosed_crash_with_context(
        &self,
        pid: Option<u32>,
        context: Option<&CrashContext>,
        diagnostic_id: Option<&str>,
        message: &str,
        stack: &str,
    ) -> Result<DiagnosedCrash, TelemetryError> {
        let _lock = self.lock_normal_operation()?;
        let state = self.read_state();
        if let Some(pid) = pid {
            if state.crashed_processes.contains(&pid) {
                return Ok(DiagnosedCrash::Duplicate);
            }
            let mut open = state.open_sessions;
            if let Some(marker) = self.read_marker() {
                if !open.iter().any(|session| session.id == marker.id) {
                    open.push(marker);
                }
            }
            if let Some(session) = open
                .iter()
                .filter(|session| session.pid == Some(pid))
                .max_by_key(|session| session.started_at_unix_ms)
            {
                return Ok(if self.record_crash_for(&session.id, message, stack)? {
                    DiagnosedCrash::Session
                } else {
                    DiagnosedCrash::Duplicate
                });
            }
        }
        let id = if let Some(identity) = diagnostic_id {
            if identity.is_empty() || identity.len() > 256 {
                return Err(TelemetryError::Invalid);
            }
            let hash = Sha256::digest(identity.as_bytes());
            Uuid::from_bytes(hash[..16].try_into().expect("SHA-256 has 32 bytes"))
        } else {
            Uuid::new_v4()
        };
        if state.received_crashes.contains(&id) {
            return Ok(DiagnosedCrash::Duplicate);
        }
        Ok(
            if self.record_crash_with_context(
                &id.hyphenated().to_string(),
                context,
                message,
                stack,
            )? {
                DiagnosedCrash::Outside
            } else {
                DiagnosedCrash::Duplicate
            },
        )
    }

    /// 补收本地崩溃记录，再排进当天的 `active`。不开始新会话，设置应用和长时间运行的宿主也能补报，不必等待键盘再次启动。
    pub fn record_active(&self, app: &TelemetryApp, now: SystemTime) -> Result<(), TelemetryError> {
        self.record_activity(app, Some(now))
    }

    /// 只补收原有崩溃，不把代收的宿主算作今天活跃；调用方仍须先核对实际授权代次。
    pub fn collect_crashes(&self, app: &TelemetryApp) -> Result<(), TelemetryError> {
        self.record_activity(app, None)
    }

    fn record_activity(
        &self,
        app: &TelemetryApp,
        now: Option<SystemTime>,
    ) -> Result<(), TelemetryError> {
        let _lock = self.lock_normal_operation()?;
        let mut state = self.read_state();
        let install_id = self.ensure_install_id(&mut state)?;
        let mut queue = self.read_queue()?;
        let directory = self.directory.join(CRASH_DIRECTORY);
        crate::storage::create_directory_and_check(&directory)?;
        if let Some(previous) = self.read_marker() {
            if !state
                .open_sessions
                .iter()
                .any(|session| session.id == previous.id)
            {
                state.open_sessions.push(previous);
            }
        }
        let collected =
            self.ingest_crash_records(&directory, app, &mut state, &install_id, &mut queue)?;
        let active =
            now.is_some_and(|now| push_active(&mut state, &mut queue, app, &install_id, now));
        if active || !collected.records.is_empty() {
            self.write_queue(&queue)?;
            self.write_state(&state)?;
            collected.acknowledge(self)?;
        }
        Ok(())
    }

    /// Queues today's `active` and sends the queue, oldest first. Blocks on the network: call from a background thread.
    pub fn flush_with(
        &self,
        app: &TelemetryApp,
        sender: &impl TelemetrySender,
        now: SystemTime,
    ) -> Result<FlushReport, TelemetryError> {
        self.record_active(app, now)?;
        self.flush(sender, now)
    }

    /// Sends the queue, oldest first, stopping at the first event the server could not take now. The lock is not held while a request is in flight, so recording from another thread or process never waits on the network.
    pub fn flush(
        &self,
        sender: &impl TelemetrySender,
        now: SystemTime,
    ) -> Result<FlushReport, TelemetryError> {
        let (pending, observed_retry_after) = {
            let _lock = self.lock_normal_operation()?;
            let state = self.read_state();
            if state.retry_deadline(unix_ms(now)).is_some() {
                return Ok(FlushReport {
                    remaining: self.read_queue()?.len(),
                    deferred: true,
                    ..FlushReport::default()
                });
            }
            (self.read_queue()?, state.retry_after_unix_ms)
        };
        let mut done = Vec::with_capacity(pending.len());
        let mut report = FlushReport::default();
        let mut retry_after = None;
        for event in &pending {
            // 旧发送快照不能在另一进程换代后继续发送；网络期间不持队列锁。
            {
                let _lock = self.lock_normal_operation()?;
            }
            let body = serde_json::to_vec(event).map_err(|_| TelemetryError::Invalid)?;
            let delivery = if body.len() > MAX_EVENT_BYTES {
                Delivery::Rejected
            } else {
                sender.send(&body)
            };
            match delivery {
                Delivery::Accepted => {
                    report.sent += 1;
                    done.push(event.id.clone());
                }
                Delivery::Rejected => {
                    report.dropped += 1;
                    done.push(event.id.clone());
                }
                Delivery::RetryAfter(delay) => {
                    retry_after = Some(delay.unwrap_or(DEFAULT_RETRY_AFTER).min(MAX_RETRY_AFTER));
                    break;
                }
                Delivery::Failed => break,
            }
        }
        let _lock = self.lock_normal_operation()?;
        let mut state = self.read_state();
        let mut queue = self.read_queue()?;
        queue.retain(|event| !done.contains(&event.id));
        report.remaining = queue.len();
        if !done.is_empty() {
            self.write_queue(&queue)?;
        }
        let now_ms = unix_ms(now);
        // 开始时已确认观察到的旧值过期或异常；在途期间新写入的未来截止时间可能基于更晚的时钟，不能再用本次较早的 now 判上限。
        let existing_deadline = if state.retry_after_unix_ms != observed_retry_after
            && state.retry_after_unix_ms > now_ms
        {
            state.retry_after_unix_ms
        } else {
            0
        };
        let deadline = match retry_after {
            Some(delay) => {
                // 请求在途期间，另一个 flush 可能已经记录了更长的退避时间。
                let proposed =
                    now_ms.saturating_add(u64::try_from(delay.as_millis()).unwrap_or(u64::MAX));
                existing_deadline.max(proposed)
            }
            None => existing_deadline,
        };
        if state.retry_after_unix_ms != deadline {
            state.retry_after_unix_ms = deadline;
            self.write_state(&state)?;
        }
        Ok(report)
    }

    /// Reporting was turned off: drops the queue, the session marker and every crash record. The installation id stays, so turning reporting back on does not count the installation twice.
    pub fn clear(&self) -> Result<(), TelemetryError> {
        let _lock = self.lock_normal_operation()?;
        self.clear_locked()
    }

    fn clear_locked(&self) -> Result<(), TelemetryError> {
        remove_file(&self.directory.join(QUEUE_FILE))?;
        remove_file(&self.directory.join(SESSION_FILE))?;
        #[cfg(unix)]
        {
            let directory = crate::storage::open_private_directory(&self.directory)?;
            match crate::storage::remove_private_tree_at(
                &directory,
                std::ffi::OsStr::new(CRASH_DIRECTORY),
            ) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        #[cfg(not(unix))]
        {
            let crashes = self.directory.join(CRASH_DIRECTORY);
            match fs::remove_dir_all(&crashes) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        let mut state = self.read_state();
        if !state.active_day.is_empty()
            || state.retry_after_unix_ms != 0
            || !state.open_sessions.is_empty()
            || !state.crashed_processes.is_empty()
            || !state.received_crashes.is_empty()
            || state.consent_epoch.is_some()
        {
            state.active_day.clear();
            state.retry_after_unix_ms = 0;
            state.open_sessions.clear();
            state.crashed_processes.clear();
            state.received_crashes.clear();
            state.consent_epoch = None;
            if !state.install_id.is_empty() || state.consent_required {
                self.write_state(&state)?;
            } else {
                // 未接入授权且没有安装身份的空状态无需保留。
                remove_file(&self.directory.join(STATE_FILE))?;
            }
        }
        Ok(())
    }

    /// The queued events, oldest first.
    pub fn queued(&self) -> Result<Vec<TelemetryEvent>, TelemetryError> {
        let _lock = self.lock()?;
        self.read_queue()
    }

    fn lock_normal_operation(&self) -> Result<crate::file_lock::ScopedFileLock, TelemetryError> {
        let lock = self.lock()?;
        if self.consent_epoch.is_none() && {
            let state = self.read_state();
            state.consent_required || state.consent_epoch.is_some()
        } {
            return Err(TelemetryError::ConsentChanged);
        }
        Ok(lock)
    }

    fn lock(&self) -> Result<crate::file_lock::ScopedFileLock, TelemetryError> {
        if !self.directory.is_absolute()
            || !crate::storage::create_directory_and_check(&self.directory)?
        {
            return Err(TelemetryError::Storage);
        }
        let file = crate::file_lock::open_private_lock_file(self.directory.join(LOCK_FILE))?;
        let file = crate::file_lock::ScopedFileLock::exclusive(file)?;
        if self.consent_epoch.is_some() && self.read_state().consent_epoch != self.consent_epoch {
            return Err(TelemetryError::ConsentChanged);
        }
        Ok(file)
    }

    fn ensure_install_id(&self, state: &mut TelemetryState) -> Result<String, TelemetryError> {
        if !valid_install_id(&state.install_id) {
            state.install_id = Uuid::new_v4().simple().to_string();
            state.active_day.clear();
            self.write_state(state)?;
        }
        Ok(state.install_id.clone())
    }

    fn read_state(&self) -> TelemetryState {
        let mut state: TelemetryState =
            read_bounded_json(&self.directory.join(STATE_FILE), MAX_STATE_FILE_BYTES)
                .unwrap_or_default();
        state.open_sessions.retain(SessionMarker::is_valid);
        let excess = state
            .received_crashes
            .len()
            .saturating_sub(MAX_RECEIVED_CRASHES);
        state.received_crashes.drain(..excess);
        state
    }

    fn write_state(&self, state: &TelemetryState) -> Result<(), TelemetryError> {
        self.write_json(STATE_FILE, state)
    }

    fn read_marker(&self) -> Option<SessionMarker> {
        read_bounded_json::<SessionMarker>(&self.directory.join(SESSION_FILE), MAX_SMALL_FILE_BYTES)
            .filter(SessionMarker::is_valid)
    }

    /// Reads the queue and, when reading it dropped anything, writes the result back at once.
    fn read_queue(&self) -> Result<Vec<TelemetryEvent>, TelemetryError> {
        let (queue, changed) = self.parse_queue();
        if changed {
            self.write_queue(&queue)?;
        }
        Ok(queue)
    }

    /// Parses the queue, keeping only events the server would accept: an event that fails to parse or validate is dropped.
    fn parse_queue(&self) -> (Vec<TelemetryEvent>, bool) {
        let path = self.directory.join(QUEUE_FILE);
        let Some(file) = open_regular_file(&path) else {
            return (Vec::new(), false);
        };
        let Ok(bytes) = crate::bounded_io::read_bounded(file, MAX_QUEUE_BYTES) else {
            return (Vec::new(), true);
        };
        let Ok(serde_json::Value::Array(values)) = serde_json::from_slice(&bytes) else {
            return (Vec::new(), true);
        };
        let mut queue = Vec::with_capacity(telemetry_queue_capacity(values.len()));
        let mut changed = false;
        for value in values {
            let Ok(original) = serde_json::from_value::<TelemetryEvent>(value) else {
                changed = true;
                continue;
            };
            let mut event = original.clone();
            if event.kind == TelemetryKind::Crash {
                event.message = clean_message(&event.message);
                event.stack = clean_stack(&event.stack);
            }
            changed |= event != original;
            if event.is_valid() {
                push(&mut queue, event);
            } else {
                changed = true;
            }
        }
        (queue, changed)
    }

    fn write_queue(&self, queue: &[TelemetryEvent]) -> Result<(), TelemetryError> {
        if queue.is_empty() {
            return remove_file(&self.directory.join(QUEUE_FILE));
        }
        self.write_json(QUEUE_FILE, &queue)
    }

    fn write_json(&self, name: &str, value: &impl Serialize) -> Result<(), TelemetryError> {
        let bytes = serde_json::to_vec(value).map_err(|_| TelemetryError::Storage)?;
        #[cfg(unix)]
        {
            let directory = crate::storage::open_private_directory(&self.directory)?;
            crate::storage::write_private_file_at(&directory, std::ffi::OsStr::new(name), &bytes)?;
            Ok(())
        }
        #[cfg(not(unix))]
        {
            let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)?;
            temporary.write_all(&bytes)?;
            temporary.as_file().sync_all()?;
            crate::storage::persist_replacing(temporary, self.directory.join(name))
                .map_err(|_| TelemetryError::Storage)?;
            Ok(())
        }
    }

    fn ingest_crash_records(
        &self,
        directory: &Path,
        app: &TelemetryApp,
        state: &mut TelemetryState,
        install_id: &str,
        queue: &mut Vec<TelemetryEvent>,
    ) -> Result<CollectedCrashes, TelemetryError> {
        let mut records = crash_record_paths(directory)?;
        records.sort();
        let mut collected = CollectedCrashes::default();
        for path in records {
            if collected.crashes >= MAX_CRASH_RECORDS_PER_START {
                break;
            }
            let event = match crash_event(
                &path,
                app,
                &state.open_sessions,
                install_id,
                self.consent_epoch,
            ) {
                Ok(event) => event,
                Err(CrashRecordSkip::Revoked) => {
                    collected.records.push(path);
                    continue;
                }
                Err(CrashRecordSkip::Unavailable) => {
                    // 写入中的记录只在崩溃进程写入期间持锁；空文件、写坏或降级后读不懂的记录过期后删除，避免永久滞留和反复扫描。
                    if crash_record_is_stale(&path) {
                        collected.records.push(path);
                    }
                    continue;
                }
            };
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            let id = Uuid::parse_str(stem).unwrap_or_else(|_| {
                // 旧系统诊断可使用非 UUID 文件名；收据仍绑定原始文件身份。
                let hash = Sha256::digest(stem.as_bytes());
                Uuid::from_bytes(hash[..16].try_into().expect("SHA-256 has 32 bytes"))
            });
            if state.received_crashes.contains(&id) {
                collected.records.push(path);
                continue;
            }
            let session_index = state
                .open_sessions
                .iter()
                .position(|session| session.id == stem);
            let needed = usize::from(!queue.iter().any(|queued| queued.id == event.id))
                + usize::from(
                    session_index.is_some()
                        && !queue
                            .iter()
                            .any(|queued| queued.id == format!("session-{stem}")),
                );
            let available = MAX_QUEUED_EVENTS.saturating_sub(queue.len())
                + queue.iter().filter(|event| !is_crash_event(event)).count();
            // 队列全是待发崩溃时保留原记录，不能确认删除尚未容纳的诊断。
            if needed > available {
                break;
            }
            if let Some(index) = session_index {
                let session = state.open_sessions.remove(index);
                collected.sessions.push(session.id.clone());
                collected.session_crashed = true;
                if let Some(pid) = session.pid {
                    state.crashed_processes.retain(|known| *known != pid);
                    state.crashed_processes.push(pid);
                }
                push(
                    queue,
                    TelemetryEvent {
                        id: format!("session-{}", session.id),
                        kind: TelemetryKind::SessionCrash,
                        platform: session.platform,
                        version: session.version,
                        message: String::new(),
                        stack: String::new(),
                        artifact: String::new(),
                        channel: String::new(),
                        install_id: install_id.to_owned(),
                    },
                );
            }
            push(queue, event);
            state.received_crashes.push(id);
            let excess = state
                .received_crashes
                .len()
                .saturating_sub(MAX_RECEIVED_CRASHES);
            state.received_crashes.drain(..excess);
            collected.crashes += 1;
            collected.records.push(path);
        }
        if state.crashed_processes.len() > MAX_CRASHED_PROCESSES {
            state
                .crashed_processes
                .drain(..state.crashed_processes.len() - MAX_CRASHED_PROCESSES);
        }
        Ok(collected)
    }
}

fn crash_record_paths(directory: &Path) -> Result<Vec<PathBuf>, TelemetryError> {
    let mut records = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some(CRASH_EXTENSION)
        {
            if records.is_empty() {
                records.reserve_exact(MAX_CRASH_RECORDS_PER_START);
            }
            records.push(path);
        }
    }
    Ok(records)
}

fn crash_record_is_stale(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age > STALE_UNREADABLE_CRASH)
}

enum CrashRecordSkip {
    Revoked,
    Unavailable,
}

fn crash_event(
    path: &Path,
    app: &TelemetryApp,
    sessions: &[SessionMarker],
    install_id: &str,
    consent_epoch: Option<Uuid>,
) -> Result<TelemetryEvent, CrashRecordSkip> {
    // 只读额度内的前段，过长的栈仍保留有用的顶部。
    let bytes = read_crash_file(path).ok_or(CrashRecordSkip::Unavailable)?;
    let text = String::from_utf8_lossy(&bytes);
    let (context, text) = if text.starts_with(identity::RECORD_PREFIX) {
        let (header, rest) = text.split_once('\n').ok_or(CrashRecordSkip::Unavailable)?;
        // 新格式的身份无效时保留原文件，不能悄悄归到补报时的版本。
        (
            Some(CrashContext::from_header(header).ok_or(CrashRecordSkip::Unavailable)?),
            rest,
        )
    } else {
        (None, text.as_ref())
    };
    let (message, stack) = text.split_once('\n').unwrap_or((text, ""));
    let mut message = clean_message(message);
    if message.trim().is_empty() {
        message = "unknown crash".to_owned();
    }
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or(CrashRecordSkip::Unavailable)?;
    let context = context.or_else(|| {
        sessions
            .iter()
            .find(|marker| marker.id == stem)
            .and_then(|marker| marker.context.clone())
    });
    if consent_epoch.is_some()
        && context.as_ref().and_then(|context| context.consent_epoch) != consent_epoch
    {
        return Err(CrashRecordSkip::Revoked);
    }
    let (platform, version) = match context.as_ref() {
        Some(context) => (context.platform.clone(), context.version.clone()),
        None => match sessions.iter().find(|marker| marker.id == stem) {
            Some(marker) => (marker.platform.clone(), marker.version.clone()),
            None => (app.platform.to_owned(), app.version.clone()),
        },
    };
    if let Some(context) = context {
        message = context.message(&message);
    }
    let id = if Uuid::parse_str(stem).is_ok() {
        format!("crash-{stem}")
    } else {
        // 系统诊断的文件名未必是 UUID；相同文件重试入队时也必须得到相同 id。
        format!("crash-{}", hex::encode(Sha256::digest(stem.as_bytes())))
    };
    let event = TelemetryEvent {
        id,
        kind: TelemetryKind::Crash,
        platform,
        version,
        message,
        stack: clean_stack(stack),
        artifact: String::new(),
        channel: String::new(),
        install_id: install_id.to_owned(),
    };
    event
        .is_valid()
        .then_some(event)
        .ok_or(CrashRecordSkip::Unavailable)
}

fn read_crash_file(path: &Path) -> Option<Vec<u8>> {
    let file = open_regular_file(path)?;
    // 原生信号写入先锁住暂存文件再发布文件名；不能读走仍在展开栈的半份记录。
    // Windows 原生写入以不共享的 CreateFile 打开，open_regular_file 本身就会拒绝并发读取。
    #[cfg(unix)]
    let lock = crate::file_lock::ScopedFileLock::try_exclusive(file).ok()??;
    #[cfg(unix)]
    let file = lock.file();
    let capacity = file
        .metadata()
        .ok()
        .map(|metadata| metadata.len().min(MAX_CRASH_RECORD_BYTES))
        .and_then(|size| usize::try_from(size).ok())
        .unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity);
    std::io::Read::read_to_end(
        &mut std::io::Read::take(file, MAX_CRASH_RECORD_BYTES),
        &mut bytes,
    )
    .ok()?;
    (!bytes.is_empty()).then_some(bytes)
}

fn is_crash_event(event: &TelemetryEvent) -> bool {
    matches!(
        event.kind,
        TelemetryKind::Crash | TelemetryKind::SessionCrash
    )
}

/// 有界队列优先保留崩溃；普通统计不能挤掉已经接收的诊断。
fn push(queue: &mut Vec<TelemetryEvent>, event: TelemetryEvent) {
    if queue.iter().any(|queued| queued.id == event.id) {
        return;
    }
    if queue.len() >= MAX_QUEUED_EVENTS {
        let Some(index) = queue.iter().position(|queued| !is_crash_event(queued)) else {
            return;
        };
        queue.remove(index);
    }
    queue.push(event);
}

/// Queues `active` for the UTC day of `now` unless the state says it was queued for that day. Returns whether anything changed.
fn push_active(
    state: &mut TelemetryState,
    queue: &mut Vec<TelemetryEvent>,
    app: &TelemetryApp,
    install_id: &str,
    now: SystemTime,
) -> bool {
    let day = utc_day(now);
    if state.active_day == day
        || (queue.len() >= MAX_QUEUED_EVENTS && queue.iter().all(is_crash_event))
    {
        return false;
    }
    push(
        queue,
        TelemetryEvent {
            id: format!("active-{install_id}-{day}"),
            kind: TelemetryKind::Active,
            platform: app.platform.to_owned(),
            version: app.version.clone(),
            message: String::new(),
            stack: String::new(),
            artifact: String::new(),
            channel: String::new(),
            install_id: install_id.to_owned(),
        },
    );
    state.active_day = day;
    true
}

/// `yyyymmdd` of `now` in UTC.
pub fn utc_day(now: SystemTime) -> String {
    let seconds = now
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let date = time::OffsetDateTime::from_unix_timestamp(i64::try_from(seconds).unwrap_or(0))
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
        .date();
    format!(
        "{:04}{:02}{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

fn unix_ms(now: SystemTime) -> u64 {
    now.duration_since(UNIX_EPOCH).map_or(0, |duration| {
        u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
    })
}

fn read_bounded_json<T: serde::de::DeserializeOwned>(path: &Path, maximum: u64) -> Option<T> {
    let file = open_regular_file(path)?;
    let bytes = crate::bounded_io::read_bounded(file, maximum).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn open_regular_file(path: &Path) -> Option<File> {
    crate::storage::open_private_file_in(path).ok()
}

fn remove_file(path: &Path) -> Result<(), TelemetryError> {
    match crate::storage::remove_private_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Keeps the text the server accepts in a multi-line field: CRLF and lone CR become LF, and every other control character except tab is removed.
fn clean_text(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .collect()
}

/// A crash message as it is sent: paths reduced to file names, controls removed, at most [`MAX_MESSAGE_CHARS`].
pub fn clean_message(value: &str) -> String {
    let cleaned = clean_text(value);
    let stripped: Vec<String> = cleaned.lines().map(strip_directories).collect();
    let joined = stripped.join("\n");
    joined.trim().chars().take(MAX_MESSAGE_CHARS).collect()
}

/// A crash stack as it is sent: paths reduced to file names, controls removed, cut at a line boundary to at most [`MAX_STACK_CHARS`] characters and [`MAX_STACK_BYTES`] bytes. A first line longer than that on its own is cut inside the line.
pub fn clean_stack(value: &str) -> String {
    let cleaned = clean_text(value);
    let mut stack = String::new();
    let mut characters = 0;
    for line in cleaned.lines().map(strip_directories) {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let separator = usize::from(!stack.is_empty());
        let line_characters = line.chars().count();
        if stack.len() + separator + line.len() > MAX_STACK_BYTES
            || characters + separator + line_characters > MAX_STACK_CHARS
        {
            if stack.is_empty() {
                for character in line.chars() {
                    if stack.len() + character.len_utf8() > MAX_STACK_BYTES
                        || characters + 1 > MAX_STACK_CHARS
                    {
                        break;
                    }
                    stack.push(character);
                    characters += 1;
                }
            }
            break;
        }
        if separator == 1 {
            stack.push('\n');
        }
        stack.push_str(line);
        characters += separator + line_characters;
    }
    stack
}

/// Removes the directory part of every path in `line`, keeping the file name: `C:\Users\Name\AppData\Local\MSIME\msime.dll+0x1a2b` becomes `msime.dll+0x1a2b` and `/home/name/.local/lib/libmsime.so(+0x1f)` becomes `libmsime.so(+0x1f)`. A path starts at a `/`, `\`, `~/` or drive letter at the start of the line or after whitespace, an opening bracket or quote, `=`, `,`, `;` or `:` (so the path of a `file:///Users/name/...` URL in an exception reason is caught too), and runs, spaces included, up to the next such start; its directory part ends at the last separator in that run. Text after the file name that itself contains a separator is cut with the path: losing part of a frame is preferable to sending a folder name.
pub fn strip_directories(line: &str) -> String {
    let characters: Vec<char> = line.chars().collect();
    let starts: Vec<usize> = (0..characters.len())
        .filter(|&index| path_starts_at(&characters, index))
        .collect();
    if starts.is_empty() {
        return line.to_owned();
    }
    let mut removed = vec![false; characters.len()];
    for (position, &start) in starts.iter().enumerate() {
        let end = starts
            .get(position + 1)
            .copied()
            .unwrap_or(characters.len());
        if let Some(last) = (start..end)
            .rev()
            .find(|&index| is_separator(characters[index]))
        {
            for flag in &mut removed[start..=last] {
                *flag = true;
            }
        }
    }
    characters
        .iter()
        .zip(removed)
        .filter(|(_, removed)| !removed)
        .map(|(character, _)| *character)
        .collect()
}

fn is_separator(character: char) -> bool {
    matches!(character, '/' | '\\')
}

fn path_starts_at(characters: &[char], index: usize) -> bool {
    let previous = index.checked_sub(1).map(|before| characters[before]);
    // After a colon only a separator starts a path (`file:///Users/...`, `error:/home/...`), and not the one of a drive letter, whose path already started at the letter.
    if previous == Some(':') {
        return is_separator(characters[index])
            && !(index >= 2
                && characters[index - 2].is_ascii_alphabetic()
                && path_starts_at(characters, index - 2));
    }
    let at_boundary = previous.is_none_or(|before| {
        before.is_whitespace()
            || matches!(before, '(' | '[' | '{' | '<' | '\'' | '"' | '=' | ',' | ';')
    });
    if !at_boundary {
        return false;
    }
    let next = |offset: usize| characters.get(index + offset).copied();
    match characters[index] {
        '/' | '\\' => true,
        '~' => next(1).is_some_and(is_separator),
        letter if letter.is_ascii_alphabetic() => {
            next(1) == Some(':') && next(2).is_some_and(is_separator)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
