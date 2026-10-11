//! Anonymous usage reporting to `POST https://api.msime.app/v1/telemetry/events`.
//!
//! Every host records into one directory it owns and flushes from a background thread; nothing here touches the network except [`TelemetryStore::flush`]. What is recorded:
//!
//! - `active`: at most once per UTC day per installation, with the deterministic id `active-<install_id>-<yyyymmdd>`, so a duplicate costs nothing.
//! - `session`: one per host process lifetime that ended through [`TelemetryStore::end_session`]. It is queued at the end and sent on a later flush.
//! - `session_crash`: a session that left its marker behind *and* a crash record. A marker alone (the system killed the process, logout, shutdown, low memory) is not a crash and produces no event.
//! - `crash`: a crash record the host's crash handler wrote to disk, turned into an event on the next start. The handler only writes a file ([`TelemetryStore::record_crash`], or a raw write to the path [`TelemetryStore::begin_session`] returned, which is what an async-signal handler can do); it never sends.
//!
//! Clients never send `download`: release downloads are counted on the server and installs by `active`.
//!
//! Every event carries `install_id`, an anonymous random id generated once per installation and stored in this directory. It is never derived from the hardware, the account or user data. Crash messages and stacks have every directory part of a path removed before they are queued, so a user or folder name cannot leave the machine.
//!
//! The queue keeps at most [`MAX_QUEUED_EVENTS`] events. A `400`-class rejection drops an event for good; `429`, `5xx` and network failures keep it for a later flush with the same id, and a `Retry-After` delay is honoured across processes.

use crate::account::{AccountError, BackendAccountClient};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

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
pub(crate) const MAX_CRASH_RECORD_BYTES: u64 = 64 * 1024;
/// Crash records turned into events per start; a host that crashes in a loop cannot fill the queue with one start's worth of records.
const MAX_CRASH_RECORDS_PER_START: usize = 8;
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
        })
    }

    pub fn platform(&self) -> &'static str {
        self.platform
    }

    pub fn version(&self) -> &str {
        &self.version
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TelemetryError {
    #[error("invalid telemetry input")]
    Invalid,
    #[error("telemetry storage is unavailable")]
    Storage,
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
    /// Whether the previous session ended in a crash it left a record of (a `session_crash` was queued).
    pub previous_session_crashed: bool,
    /// Crash records queued as `crash` events.
    pub crashes: usize,
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
    #[serde(default)]
    install_id: String,
    /// `yyyymmdd` (UTC) of the last queued `active`.
    #[serde(default)]
    active_day: String,
    #[serde(default)]
    retry_after_unix_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SessionMarker {
    id: String,
    platform: String,
    version: String,
    started_at_unix_ms: u64,
}

/// The telemetry files of one host, all under one directory the host owns.
#[derive(Clone, Debug)]
pub struct TelemetryStore {
    directory: PathBuf,
}

impl TelemetryStore {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// The anonymous installation id, generated and stored on first use.
    pub fn install_id(&self) -> Result<String, TelemetryError> {
        let _lock = self.lock()?;
        let mut state = self.read_state();
        let id = self.ensure_install_id(&mut state)?;
        Ok(id)
    }

    /// Starts a session: closes the previous one (a `session_crash` when it left a crash record, nothing when it only left its marker), queues every crash record as a `crash`, queues today's `active` and writes a new marker. Does no network I/O.
    pub fn begin_session(
        &self,
        app: &TelemetryApp,
        now: SystemTime,
    ) -> Result<SessionStart, TelemetryError> {
        let _lock = self.lock()?;
        let mut state = self.read_state();
        let install_id = self.ensure_install_id(&mut state)?;
        let mut queue = self.read_queue()?;
        let previous = self.read_marker();
        let crashes_directory = self.directory.join(CRASH_DIRECTORY);
        crate::storage::create_directory_and_check(&crashes_directory)?;
        let mut previous_session_crashed = false;
        if let Some(previous) = &previous {
            let record = crashes_directory.join(format!("{}.{CRASH_EXTENSION}", previous.id));
            if is_regular_file(&record) {
                previous_session_crashed = true;
                push(
                    &mut queue,
                    TelemetryEvent {
                        id: format!("session-{}", previous.id),
                        kind: TelemetryKind::SessionCrash,
                        platform: previous.platform.clone(),
                        version: previous.version.clone(),
                        message: String::new(),
                        stack: String::new(),
                        artifact: String::new(),
                        channel: String::new(),
                        install_id: install_id.clone(),
                    },
                );
            }
        }
        let crashes = self.ingest_crash_records(
            &crashes_directory,
            app,
            previous.as_ref(),
            &install_id,
            &mut queue,
        )?;
        push_active(&mut state, &mut queue, app, &install_id, now);
        let marker = SessionMarker {
            id: Uuid::new_v4().hyphenated().to_string(),
            platform: app.platform.to_owned(),
            version: app.version.clone(),
            started_at_unix_ms: unix_ms(now),
        };
        self.write_queue(&queue)?;
        self.write_state(&state)?;
        self.write_json(SESSION_FILE, &marker)?;
        Ok(SessionStart {
            crash_record_path: crashes_directory.join(format!("{}.{CRASH_EXTENSION}", marker.id)),
            previous_session_crashed,
            crashes,
        })
    }

    /// Ends the running session normally: queues its `session` event and removes the marker. Returns false when no session was running (reporting was off when it began, or it was already ended). Does no network I/O, so it is safe on a shutdown path.
    pub fn end_session(&self) -> Result<bool, TelemetryError> {
        let _lock = self.lock()?;
        let Some(marker) = self.read_marker() else {
            return Ok(false);
        };
        let mut state = self.read_state();
        let install_id = self.ensure_install_id(&mut state)?;
        let mut queue = self.read_queue()?;
        push(
            &mut queue,
            TelemetryEvent {
                id: format!("session-{}", marker.id),
                kind: TelemetryKind::Session,
                platform: marker.platform,
                version: marker.version,
                message: String::new(),
                stack: String::new(),
                artifact: String::new(),
                channel: String::new(),
                install_id,
            },
        );
        self.write_queue(&queue)?;
        remove_file(&self.directory.join(SESSION_FILE))?;
        Ok(true)
    }

    /// Writes the running session's crash record. Meant for a crash handler that may allocate (a C++ terminate handler, an uncaught-exception hook): it takes no lock and only creates one file. Returns false when no session is running, or when this session already has a record (the first one is kept: a terminate handler that aborts would otherwise have its summary replaced by the SIGABRT handler's). The message and stack are cleaned when the record is queued on the next start, not here.
    pub fn record_crash(&self, message: &str, stack: &str) -> Result<bool, TelemetryError> {
        let Some(marker) = self.read_marker() else {
            return Ok(false);
        };
        let directory = self.directory.join(CRASH_DIRECTORY);
        crate::storage::create_directory_and_check(&directory)?;
        let path = directory.join(format!("{}.{CRASH_EXTENSION}", marker.id));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = match options.open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        let message = message.lines().next().unwrap_or_default();
        file.write_all(message.as_bytes())?;
        file.write_all(b"\n")?;
        file.write_all(stack.as_bytes())?;
        file.sync_all()?;
        Ok(true)
    }

    /// Queues today's `active` unless it was already queued today. Long-running hosts call this (or [`Self::flush_with`]) periodically so a process that runs across midnight still counts the new day.
    pub fn record_active(&self, app: &TelemetryApp, now: SystemTime) -> Result<(), TelemetryError> {
        let _lock = self.lock()?;
        let mut state = self.read_state();
        let install_id = self.ensure_install_id(&mut state)?;
        let mut queue = self.read_queue()?;
        if push_active(&mut state, &mut queue, app, &install_id, now) {
            self.write_queue(&queue)?;
            self.write_state(&state)?;
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
        let pending = {
            let _lock = self.lock()?;
            let state = self.read_state();
            if state.retry_after_unix_ms > unix_ms(now) {
                return Ok(FlushReport {
                    remaining: self.read_queue()?.len(),
                    deferred: true,
                    ..FlushReport::default()
                });
            }
            self.read_queue()?
        };
        let mut done = Vec::with_capacity(pending.len());
        let mut report = FlushReport::default();
        let mut retry_after = None;
        for event in &pending {
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
        let _lock = self.lock()?;
        let mut state = self.read_state();
        let mut queue = self.read_queue()?;
        queue.retain(|event| !done.contains(&event.id));
        report.remaining = queue.len();
        if !done.is_empty() {
            self.write_queue(&queue)?;
        }
        let now_ms = unix_ms(now);
        let deadline = match retry_after {
            Some(delay) => {
                // 请求在途期间，另一个 flush 可能已经记录了更长的退避时间。
                let proposed =
                    now_ms.saturating_add(u64::try_from(delay.as_millis()).unwrap_or(u64::MAX));
                state.retry_after_unix_ms.max(proposed)
            }
            None if state.retry_after_unix_ms > now_ms => state.retry_after_unix_ms,
            None => 0,
        };
        if state.retry_after_unix_ms != deadline {
            state.retry_after_unix_ms = deadline;
            self.write_state(&state)?;
        }
        Ok(report)
    }

    /// Reporting was turned off: drops the queue, the session marker and every crash record. The installation id stays, so turning reporting back on does not count the installation twice.
    pub fn clear(&self) -> Result<(), TelemetryError> {
        let _lock = self.lock()?;
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
        if !state.active_day.is_empty() || state.retry_after_unix_ms != 0 {
            state.active_day.clear();
            state.retry_after_unix_ms = 0;
            if !state.install_id.is_empty() {
                self.write_state(&state)?;
            }
        }
        Ok(())
    }

    /// The queued events, oldest first.
    pub fn queued(&self) -> Result<Vec<TelemetryEvent>, TelemetryError> {
        let _lock = self.lock()?;
        self.read_queue()
    }

    fn lock(&self) -> Result<File, TelemetryError> {
        if !self.directory.is_absolute()
            || !crate::storage::create_directory_and_check(&self.directory)?
        {
            return Err(TelemetryError::Storage);
        }
        let file = crate::file_lock::open_private_lock_file(self.directory.join(LOCK_FILE))?;
        crate::file_lock::exclusive(&file)?;
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
        read_small_json(&self.directory.join(STATE_FILE)).unwrap_or_default()
    }

    fn write_state(&self, state: &TelemetryState) -> Result<(), TelemetryError> {
        self.write_json(STATE_FILE, state)
    }

    fn read_marker(&self) -> Option<SessionMarker> {
        read_small_json::<SessionMarker>(&self.directory.join(SESSION_FILE)).filter(|marker| {
            Uuid::parse_str(&marker.id).is_ok()
                && canonical_platform(&marker.platform) == Some(marker.platform.as_str())
                && crate::community::valid_text(&marker.version, 1, 64, false)
        })
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
            temporary
                .persist(self.directory.join(name))
                .map_err(|_| TelemetryError::Storage)?;
            Ok(())
        }
    }

    fn ingest_crash_records(
        &self,
        directory: &Path,
        app: &TelemetryApp,
        previous: Option<&SessionMarker>,
        install_id: &str,
        queue: &mut Vec<TelemetryEvent>,
    ) -> Result<usize, TelemetryError> {
        let mut records = crash_record_paths(directory)?;
        records.sort();
        let mut queued = 0;
        for path in records {
            if queued < MAX_CRASH_RECORDS_PER_START {
                if let Some(event) = crash_event(&path, app, previous, install_id) {
                    push(queue, event);
                    queued += 1;
                }
            }
            remove_file(&path)?;
        }
        Ok(queued)
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

fn crash_event(
    path: &Path,
    app: &TelemetryApp,
    previous: Option<&SessionMarker>,
    install_id: &str,
) -> Option<TelemetryEvent> {
    // A record is read up to the limit and the rest ignored rather than refused: a stack that ran long still has its useful top.
    let bytes = read_crash_file(path)?;
    let text = String::from_utf8_lossy(&bytes);
    let (message, stack) = text.split_once('\n').unwrap_or((&text, ""));
    let mut message = clean_message(message);
    if message.trim().is_empty() {
        message = "unknown crash".to_owned();
    }
    let stem = path.file_stem()?.to_str()?;
    let (platform, version) = match previous.filter(|marker| marker.id == stem) {
        Some(marker) => (marker.platform.clone(), marker.version.clone()),
        None => (app.platform.to_owned(), app.version.clone()),
    };
    let id = if Uuid::parse_str(stem).is_ok() {
        format!("crash-{stem}")
    } else {
        format!("crash-{}", Uuid::new_v4().hyphenated())
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
    event.is_valid().then_some(event)
}

fn read_crash_file(path: &Path) -> Option<Vec<u8>> {
    let file = open_regular_file(path)?;
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
    Some(bytes)
}

/// Appends `event` unless an event with its id is queued already, then drops the oldest events beyond [`MAX_QUEUED_EVENTS`].
fn push(queue: &mut Vec<TelemetryEvent>, event: TelemetryEvent) {
    if queue.iter().any(|queued| queued.id == event.id) {
        return;
    }
    queue.push(event);
    if queue.len() > MAX_QUEUED_EVENTS {
        let excess = queue.len() - MAX_QUEUED_EVENTS;
        queue.drain(..excess);
    }
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
    if state.active_day == day {
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

fn read_small_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let file = open_regular_file(path)?;
    let bytes = crate::bounded_io::read_bounded(file, MAX_SMALL_FILE_BYTES).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
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
