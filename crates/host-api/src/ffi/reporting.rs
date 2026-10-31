//! 面向原生宿主的使用情况上报、公告和检查更新：`msime_client_services::telemetry` 与 `msime_client_services::notices` 提供的上报队列、公告源和 Markdown 渲染，以及 `msime_client_services::update_check`。
//!
//! Part of the C ABI; see the parent module for what these shims guarantee. Every request is a small JSON document so a host builds it with the JSON library it already has, and a field added later does not change a signature.

use super::with_bounded_bytes;
use crate::*;
use msime_client_services::account::BackendAccountClient;
use msime_client_services::notices::{fetch_notices, markdown_to_html, NoticeChannel, NoticeStore};
use msime_client_services::telemetry::{
    backend_sender, clear_if_consent_disabled, CrashContext, DiagnosedCrash, TelemetryApp,
    TelemetryStore,
};
use msime_client_services::update_check::background::{pending_update, run_background_check};
use msime_client_services::update_check::{check_for_update, UpdateCheckRequest};
use std::time::SystemTime;

mod consent;
pub use consent::msime_client_telemetry_consent;

const MAX_REQUEST_BYTES: usize = 16 * 1024;
/// A crash request carries a message and a stack the host has not trimmed yet.
const MAX_CRASH_REQUEST_BYTES: usize = 256 * 1024;
const MAX_MARKDOWN_BYTES: usize = 256 * 1024;

unsafe fn document<T: serde::de::DeserializeOwned>(
    pointer: *const u8,
    length: usize,
    maximum: usize,
) -> Result<T, String> {
    // SAFETY: forwarded from the documented caller contract.
    unsafe {
        with_bounded_bytes(
            pointer,
            length,
            maximum,
            "invalid request buffer",
            |bytes| {
                serde_json::from_slice(bytes).map_err(|_| "invalid request document".to_owned())
            },
        )
    }
}

fn absolute_directory(value: &str) -> Result<&Path, String> {
    let path = Path::new(value);
    if !path.is_absolute() {
        return Err("directory must be absolute".into());
    }
    Ok(path)
}

/// Which hosts are reporting, and whether the user lets them.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TelemetryRequest {
    directory: String,
    platform: String,
    version: String,
    /// 独立应用和辅助进程只准备崩溃采集，不建立输入会话。
    #[serde(default)]
    capture_only: bool,
    #[serde(default = "default_telemetry_component")]
    component: String,
    /// The host's own `usage_reporting` switch, for hosts that keep preferences of their own.
    #[serde(default)]
    enabled: Option<bool>,
    /// The shared preferences directory (`preferences.json`), read for `usage_reporting` when `enabled` is absent.
    #[serde(default)]
    preferences_directory: Option<String>,
    /// 单写方发布的只读授权目录，与 preferences_directory 互斥。
    #[serde(default)]
    authority_directory: Option<String>,
    /// 可选的持续同意身份；给出时必须使用共享偏好，不能由 enabled 绕过。
    #[serde(default)]
    consent_epoch: Option<String>,
    /// 验收工具的安全握手；正式构建按未知字段拒绝，避免误载正式库后发送合成事件。
    #[cfg(feature = "telemetry-loopback")]
    #[serde(default)]
    test_origin: Option<String>,
}

fn default_telemetry_component() -> String {
    "input".to_owned()
}

impl TelemetryRequest {
    /// 真实偏好入口自动取得当前代次；显式旧代次仍必须被拒绝，不能追认为当前请求。
    fn bind_shared_consent(&mut self, store: &TelemetryStore) -> Result<bool, String> {
        if self.consent_epoch.is_some() {
            self.epoch_source()?;
            return Ok(true);
        }
        // 桌面旧调用自动绑定；移动宿主显式传入它们准备时取得的代次。
        if self.enabled.is_none()
            && matches!(
                msime_client_services::telemetry::canonical_platform(&self.platform),
                Some("macos" | "windows" | "linux")
            )
        {
            let directory = self
                .preferences_directory
                .as_deref()
                .ok_or("enabled or preferences_directory is required")?;
            let preferences = PreferencesStore::new(absolute_directory(directory)?);
            match preferences
                .usage_reporting_consent()
                .map_err(|error| error.to_string())?
            {
                Some(consent) => self.consent_epoch = Some(consent.epoch.to_string()),
                None => {
                    clear_if_consent_disabled(&preferences, store)?;
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn epoch_source(&self) -> Result<consent::ConsentSource, String> {
        if self.enabled.is_some() {
            return Err("consent_epoch cannot be combined with enabled".into());
        }
        consent::ConsentSource::new(
            self.preferences_directory.as_deref(),
            self.authority_directory.as_deref(),
        )
    }
    /// The user's choice: `Some(false)` clears what is queued, `None` (preferences unreadable) neither reports nor clears.
    fn consent(&self) -> Result<Option<bool>, String> {
        if let Some(enabled) = self.enabled {
            return Ok(Some(enabled));
        }
        let directory = self
            .preferences_directory
            .as_deref()
            .ok_or("enabled or preferences_directory is required")?;
        let store = PreferencesStore::new(absolute_directory(directory)?);
        Ok(store
            .load()
            .ok()
            .map(|snapshot| snapshot.preferences.usage_reporting))
    }

    fn open(&self) -> Result<(TelemetryStore, TelemetryApp), String> {
        #[cfg(feature = "telemetry-loopback")]
        if let Some(expected) = self.test_origin.as_deref() {
            if msime_client_services::telemetry::isolated_origin()
                .map_err(|error| error.to_string())?
                != expected
            {
                return Err("isolated telemetry origin mismatch".into());
            }
        }
        if self.authority_directory.is_some() && self.consent_epoch.is_none() {
            return Err("authority_directory requires consent_epoch".into());
        }
        let store = TelemetryStore::new(absolute_directory(&self.directory)?);
        let app = TelemetryApp::new(&self.platform, &self.version)
            .and_then(|app| app.with_component(&self.component))
            .map_err(|error| error.to_string())?;
        Ok((store, app))
    }
}

/// Starts a reporting session at host start. With reporting on: closes the previous session, queues its crash records and today's `active`, writes the new session marker and returns `{enabled:true, crash_record_path, session_id, previous_session_crashed, crashes}`. With reporting off: clears the queue, marker and crash records and returns `{enabled:false}`. No network I/O.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_telemetry_begin(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: guaranteed by the documented caller contract.
        let mut request: TelemetryRequest =
            unsafe { document(request, length, MAX_REQUEST_BYTES)? };
        let (store, app) = request.open()?;
        if !request.bind_shared_consent(&store)? {
            return Ok(json!({"enabled": false}));
        }
        if let Some(epoch) = request.consent_epoch.as_deref() {
            let start =
                consent::with_epoch(&request.epoch_source()?, epoch, &store, |store, _| {
                    if request.capture_only {
                        store.prepare_crash_capture(&app, SystemTime::now())
                    } else {
                        store.begin_session(&app, SystemTime::now())
                    }
                    .map_err(|error| error.to_string())
                })?;
            return match start {
                Some(start) => {
                    let mut value =
                        serde_json::to_value(start).map_err(|error| error.to_string())?;
                    value["enabled"] = Value::Bool(true);
                    Ok(value)
                }
                None => Ok(json!({"enabled": false})),
            };
        }
        match request.consent()? {
            Some(true) => {
                let start = if request.capture_only {
                    store.prepare_crash_capture(&app, SystemTime::now())
                } else {
                    store.begin_session(&app, SystemTime::now())
                }
                .map_err(|error| error.to_string())?;
                let mut value = serde_json::to_value(start).map_err(|error| error.to_string())?;
                value["enabled"] = Value::Bool(true);
                Ok(value)
            }
            Some(false) => {
                if let Some(directory) = request.preferences_directory.as_deref() {
                    clear_if_consent_disabled(
                        &PreferencesStore::new(absolute_directory(directory)?),
                        &store,
                    )?;
                } else {
                    store.clear().map_err(|error| error.to_string())?;
                }
                Ok(json!({ "enabled": false }))
            }
            None => Ok(json!({ "enabled": false })),
        }
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EndRequest {
    directory: String,
    /// begin 返回的 `session_id`。给了就只结束这个会话；不给则结束标记里记着的会话（旧宿主的行为）。
    #[serde(default)]
    session: Option<String>,
}

/// 正常结束会话（`{directory, session?}`），排进它的 `session` 事件，下次 flush 时发送。带 `session`（begin 返回的 `session_id`）时只结束这个会话，同目录里别的进程的会话不受影响；不带时结束标记里记着的会话。没有对应的会话在运行时值为 false。不联网，可以在退出路径上调用。
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_telemetry_end(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: guaranteed by the documented caller contract.
        let request: EndRequest = unsafe { document(request, length, MAX_REQUEST_BYTES)? };
        let store = TelemetryStore::new(absolute_directory(&request.directory)?);
        match request.session.as_deref() {
            Some(session) => store.end_session_by_id(session),
            None => store.end_session(),
        }
        .map(Value::Bool)
        .map_err(|error| error.to_string())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CrashRequest {
    directory: String,
    message: String,
    #[serde(default)]
    stack: String,
    /// begin 返回的 `session_id`。给了就写这个会话的记录，不看标记现在属于谁。
    #[serde(default)]
    session: Option<String>,
    /// 系统事后报告的崩溃（iOS 的 MetricKit 诊断）所属的进程号：按它认领该进程未结束的会话，见 `TelemetryStore::record_diagnosed_crash`。
    #[serde(default)]
    pid: Option<u32>,
    /// 系统事后报告的强制结束（SIGKILL），不属于任何会话：只报 `crash`，不报 `session_crash`。
    #[serde(default)]
    outside_session: bool,
    /// begin 返回的预计算记录头，供独立进程的异常回调保存原始构建身份。
    #[serde(default)]
    crash_record_header: Option<String>,
    /// 系统诊断的稳定身份；只在事后补录时使用，不含系统原始描述。
    #[serde(default)]
    diagnostic_id: Option<String>,
    /// 系统保留的原始捕获 UUID；与现场记录共用文件和事件身份，仅允许完整授权核验的补录。
    #[serde(default)]
    diagnostic_capture: Option<String>,
    /// 仅系统事后诊断使用；普通故障处理器沿用记录头，不读偏好。
    #[serde(default)]
    consent_epoch: Option<String>,
    #[serde(default)]
    preferences_directory: Option<String>,
    /// 单写方发布的只读授权目录，与 preferences_directory 互斥。
    #[serde(default)]
    authority_directory: Option<String>,
    #[serde(default)]
    diagnostic_begin_unix_ms: Option<u64>,
    #[serde(default)]
    diagnostic_end_unix_ms: Option<u64>,
}

/// 写会话的崩溃记录（`{directory, message, stack, session?, pid?, outside_session?}`），记录在下次启动时成为 `crash` 事件。三种用法互斥：
///
/// - 可以分配内存的崩溃处理（如 C++ terminate handler）：带 `session` 时写这个会话的记录；都不带时写标记里记着的会话的记录，而标记可能已属于同目录里别的进程。不加锁。没有会话在运行或这个会话已有记录时值为 false。信号处理函数改为直接写 `msime_client_telemetry_begin` 返回的记录文件。
/// - 系统事后报告的崩溃带 `pid`：写成该进程未结束会话的记录（下次启动时连同 `session_crash` 补报）；该进程这次崩溃已被它自己的处理程序记下时不写，值为 false；没有未结束的会话时写成单独的记录，只报 `crash`。
/// - 系统事后报告的强制结束带 `outside_session: true`：总是写成单独的记录，只报 `crash`。
///
/// 后两种会加锁，不能在崩溃处理程序里调用。都不联网。
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_telemetry_record_crash(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: guaranteed by the documented caller contract.
        let request: CrashRequest = unsafe { document(request, length, MAX_CRASH_REQUEST_BYTES)? };
        let store = TelemetryStore::new(absolute_directory(&request.directory)?);
        let context = request
            .crash_record_header
            .as_deref()
            .map(|header| CrashContext::from_header(header).ok_or("invalid crash record header"))
            .transpose()?;
        let (message, stack) = (&request.message, &request.stack);
        if let Some(epoch) = request.consent_epoch.as_deref() {
            if request.session.is_some() || request.pid.is_some() == request.outside_session {
                return Err("consent_epoch requires a system diagnostic".into());
            }
            let source = consent::ConsentSource::new(
                request.preferences_directory.as_deref(),
                request.authority_directory.as_deref(),
            )?;
            let begin = request
                .diagnostic_begin_unix_ms
                .ok_or("diagnostic_begin_unix_ms is required")?;
            let end = request
                .diagnostic_end_unix_ms
                .ok_or("diagnostic_end_unix_ms is required")?;
            let recorded = consent::with_epoch(&source, epoch, &store, |store, consent| {
                let now = SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|error| error.to_string())?
                    .as_millis();
                if begin < consent.since_unix_ms || end < begin || u128::from(end) > now {
                    return Ok(false);
                }
                let mut context = context
                    .clone()
                    .ok_or("system diagnostic requires crash_record_header")?;
                if context
                    .consent_epoch
                    .is_some_and(|epoch| epoch != consent.epoch)
                {
                    return Ok(false);
                }
                context.consent_epoch = Some(consent.epoch);
                if let Some(capture) = request.diagnostic_capture.as_deref() {
                    if request.pid.is_some()
                        || !request.outside_session
                        || request.diagnostic_id.is_some()
                    {
                        return Err(
                            "diagnostic_capture requires an exclusive capture identity".into()
                        );
                    }
                    return store
                        .record_diagnosed_capture(capture, &context, message, stack)
                        .map_err(|error| error.to_string());
                }
                store
                    .record_diagnosed_crash_with_context(
                        request.pid,
                        Some(&context),
                        request.diagnostic_id.as_deref(),
                        message,
                        stack,
                    )
                    .map(|recorded| recorded != DiagnosedCrash::Duplicate)
                    .map_err(|error| error.to_string())
            })?;
            return Ok(Value::Bool(recorded.unwrap_or(false)));
        }
        if request.diagnostic_capture.is_some()
            || request.preferences_directory.is_some()
            || request.authority_directory.is_some()
            || request.diagnostic_begin_unix_ms.is_some()
            || request.diagnostic_end_unix_ms.is_some()
        {
            return Err("diagnostic consent fields require consent_epoch".into());
        }
        match (
            request.session.as_deref(),
            request.pid,
            request.outside_session,
        ) {
            (Some(session), None, false) if request.diagnostic_id.is_none() => {
                match context.as_ref() {
                    Some(context) => {
                        store.record_crash_with_context(session, Some(context), message, stack)
                    }
                    None => store.record_crash_for(session, message, stack),
                }
            }
            (None, None, false) if context.is_none() && request.diagnostic_id.is_none() => {
                store.record_crash(message, stack)
            }
            (None, pid, outside) if pid.is_some() != outside => store
                .record_diagnosed_crash_with_context(
                    pid,
                    context.as_ref(),
                    request.diagnostic_id.as_deref(),
                    message,
                    stack,
                )
                .map(|recorded| recorded != DiagnosedCrash::Duplicate),
            _ => return Err("session, pid and outside_session are exclusive".to_owned()),
        }
        .map(Value::Bool)
        .map_err(|error| error.to_string())
    })
}

/// Queues today's `active` and sends the queue to `https://api.msime.app/v1/telemetry/events` without credentials. Takes the same request as `msime_client_telemetry_begin`; with reporting off it clears instead and sends nothing. Value: `{enabled, sent, dropped, remaining, deferred}`. Blocks on the network: call from a background thread, at start and every few hours in a long-running host.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_telemetry_flush(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: guaranteed by the documented caller contract.
        let mut request: TelemetryRequest =
            unsafe { document(request, length, MAX_REQUEST_BYTES)? };
        let (store, app) = request.open()?;
        if !request.bind_shared_consent(&store)? {
            return Ok(json!({"enabled": false}));
        }
        if let Some(epoch) = request.consent_epoch.as_deref() {
            let source = request.epoch_source()?;
            let bound = consent::with_epoch(&source, epoch, &store, |store, _| {
                store
                    .record_active(&app, SystemTime::now())
                    .map_err(|error| error.to_string())?;
                Ok(store.clone())
            })?;
            let Some(store) = bound else {
                return Ok(json!({"enabled": false}));
            };
            let client = backend_sender().map_err(|error| error.to_string())?;
            let sender = consent::EpochSender {
                source,
                epoch,
                sender: &client,
            };
            let report = store
                .flush(&sender, SystemTime::now())
                .map_err(|error| error.to_string())?;
            let mut value = serde_json::to_value(report).map_err(|error| error.to_string())?;
            value["enabled"] = Value::Bool(true);
            return Ok(value);
        }
        match request.consent()? {
            Some(true) => {
                let client = backend_sender().map_err(|error| error.to_string())?;
                let report = store
                    .flush_with(&app, &client, SystemTime::now())
                    .map_err(|error| error.to_string())?;
                let mut value = serde_json::to_value(report).map_err(|error| error.to_string())?;
                value["enabled"] = Value::Bool(true);
                Ok(value)
            }
            Some(false) => {
                if let Some(directory) = request.preferences_directory.as_deref() {
                    clear_if_consent_disabled(
                        &PreferencesStore::new(absolute_directory(directory)?),
                        &store,
                    )?;
                } else {
                    store.clear().map_err(|error| error.to_string())?;
                }
                Ok(json!({ "enabled": false }))
            }
            None => Ok(json!({ "enabled": false })),
        }
    })
}

/// Clears the telemetry queue, session marker and crash records (`{directory}`), for a host whose user just turned usage reporting off.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_telemetry_clear(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: guaranteed by the documented caller contract.
        let request: ClearRequest = unsafe { document(request, length, MAX_REQUEST_BYTES)? };
        if request.preferences_directory.is_some() || request.authority_directory.is_some() {
            let source = consent::ConsentSource::new(
                request.preferences_directory.as_deref(),
                request.authority_directory.as_deref(),
            )?;
            return source
                .clear_if_disabled(&TelemetryStore::new(absolute_directory(
                    &request.directory,
                )?))
                .map(Value::Bool);
        }
        TelemetryStore::new(absolute_directory(&request.directory)?)
            .clear()
            .map(|()| Value::Bool(true))
            .map_err(|error| error.to_string())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClearRequest {
    directory: String,
    #[serde(default)]
    preferences_directory: Option<String>,
    /// 单写方发布的只读授权目录，与 preferences_directory 互斥。
    #[serde(default)]
    authority_directory: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoticesRequest {
    directory: String,
    platform: String,
    #[serde(default = "app_channel")]
    channel: NoticeChannel,
}

fn app_channel() -> NoticeChannel {
    NoticeChannel::App
}

/// The live notices for this host that the user has not dismissed, newest first: `{items:[{id,title,body,html,targets,channels,published_at}]}`, where `html` is the body rendered by `msime_client_markdown_to_html`. Request: `{directory, platform, channel?}` (`channel` defaults to `app`). Uses the copy cached under `directory` when the feed was requested less than a minute ago, and that copy when the request fails. Blocks on the network: call off the input thread, when the settings window or app home opens.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_notices(request: *const u8, length: usize) -> *mut c_char {
    response(|| {
        // SAFETY: guaranteed by the documented caller contract.
        let request: NoticesRequest = unsafe { document(request, length, MAX_REQUEST_BYTES)? };
        let store = NoticeStore::new(absolute_directory(&request.directory)?);
        let items = store
            .current(
                request.channel,
                &request.platform,
                SystemTime::now(),
                || {
                    fetch_notices(
                        &BackendAccountClient::new()?,
                        request.channel,
                        &request.platform,
                    )
                },
            )
            .map_err(|error| error.to_string())?;
        let items: Vec<Value> = items
            .into_iter()
            .map(|notice| {
                json!({
                    "id": notice.id,
                    "title": notice.title,
                    "html": markdown_to_html(&notice.body),
                    "body": notice.body,
                    "targets": notice.targets,
                    "channels": notice.channels,
                    "published_at": notice.published_at,
                })
            })
            .collect();
        Ok(json!({ "items": items }))
    })
}

/// 把宿主所在平台在请求通道里最新的构建与当前运行的构建比较，结果为 `{status:"available"|"current", update:{version:{display,parts}, release_url, installer_name, installer_sha256, signed, channel, build_run}}`；该平台还没有发布时为 `{status:"none"}`。请求为 `{platform, current_version, edition?, arch?, channel?}`，`channel` 是 `stable`（缺省）、`beta` 或 `nightly`。会在网络上阻塞，每个请求最多十秒，不要在 UI 线程或输入线程上调用。
/// # Safety
/// `request` 指向 `length` 个可读的 UTF-8 JSON 字节。传入空指针会被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_update_check(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: 由文档写明的调用方契约保证。
        let request: UpdateCheckRequest = unsafe { document(request, length, MAX_REQUEST_BYTES)? };
        let check = check_for_update(&request).map_err(|error| error.to_string())?;
        serde_json::to_value(check).map_err(|error| error.to_string())
    })
}

/// 后台检查和读缓存的请求：`msime_client_update_check` 的请求加上宿主的状态目录 `directory`（绝对路径，记录 `update-check.json` 放在这里）。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BackgroundUpdateRequest {
    directory: String,
    platform: String,
    current_version: String,
    #[serde(default)]
    edition: Option<String>,
    #[serde(default)]
    arch: Option<String>,
    #[serde(default)]
    channel: Option<String>,
}

impl BackgroundUpdateRequest {
    fn split(self) -> (String, UpdateCheckRequest) {
        (
            self.directory,
            UpdateCheckRequest {
                platform: self.platform,
                current_version: self.current_version,
                edition: self.edition,
                arch: self.arch,
                channel: self.channel,
            },
        )
    }
}

/// 后台检查一次更新（`msime_client_services::update_check::background::run_background_check`）：距上次成功检查不满一天、距上次失败不满六小时、另一个进程正在检查、开发构建或设置了 `MSIME_DISABLE_UPDATE_CHECK` 时不联网。请求为 `{directory, platform, current_version, edition?, arch?, channel?}`，`directory` 是宿主状态目录的绝对路径，同一宿主的读缓存请求要用同一个目录。结果为 `{status:"disabled"|"not_due"|"unavailable"|"checked", notify: update|null}`，`notify` 的形状同 `msime_client_update_check` 的 `update`，是这次该提醒、而且已经记成提醒过的构建。没到时间时只读一个小文件，可以每小时调用；到时间时会在网络上阻塞，不要在 UI 线程或输入线程上调用。
/// # Safety
/// `request` 指向 `length` 个可读的 UTF-8 JSON 字节。传入空指针会被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_update_background_check(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: 由文档写明的调用方契约保证。
        let request: BackgroundUpdateRequest =
            unsafe { document(request, length, MAX_REQUEST_BYTES)? };
        let (directory, request) = request.split();
        let outcome = run_background_check(absolute_directory(&directory)?, &request)
            .map_err(|error| error.to_string())?;
        serde_json::to_value(outcome).map_err(|error| error.to_string())
    })
}

/// 后台检查已经找到、仍比当前运行的构建新的版本，关于页打开时先显示它：`{status:"available", update}`，没有时为 null。请求与 `msime_client_update_background_check` 相同；记录是按别的通道检查的时也为 null。只读记录，不联网、不拿锁。
/// # Safety
/// `request` 指向 `length` 个可读的 UTF-8 JSON 字节。传入空指针会被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_update_pending(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: 由文档写明的调用方契约保证。
        let request: BackgroundUpdateRequest =
            unsafe { document(request, length, MAX_REQUEST_BYTES)? };
        let (directory, request) = request.split();
        let pending = pending_update(absolute_directory(&directory)?, &request)
            .map_err(|error| error.to_string())?;
        serde_json::to_value(pending).map_err(|error| error.to_string())
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DismissRequest {
    directory: String,
    id: String,
}

/// Remembers under `directory` that the user dismissed notice `id` (`{directory, id}`), so `msime_client_notices` no longer lists it.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_notice_dismiss(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: guaranteed by the documented caller contract.
        let request: DismissRequest = unsafe { document(request, length, MAX_REQUEST_BYTES)? };
        NoticeStore::new(absolute_directory(&request.directory)?)
            .dismiss(&request.id)
            .map(|()| Value::Bool(true))
            .map_err(|error| error.to_string())
    })
}

/// Renders simple Markdown (a notice body) to HTML for a rich-text view: raw HTML in the source is escaped, only http, https and mailto links are kept, and images become links instead of being loaded. Value is the HTML string.
/// # Safety
/// `text` points to `length` readable UTF-8 bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_markdown_to_html(
    text: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: guaranteed by the documented caller contract.
        unsafe {
            with_bounded_bytes(
                text,
                length,
                MAX_MARKDOWN_BYTES,
                "invalid text buffer",
                |bytes| {
                    let text = std::str::from_utf8(bytes).map_err(|_| "invalid text encoding")?;
                    Ok(Value::String(markdown_to_html(text)))
                },
            )
        }
    })
}
