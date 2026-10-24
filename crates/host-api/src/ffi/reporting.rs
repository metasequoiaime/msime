//! 面向原生宿主的使用情况上报、公告和检查更新：`msime_client_services::telemetry` 与 `msime_client_services::notices` 提供的上报队列、公告源和 Markdown 渲染，以及 `msime_client_services::update_check`。
//!
//! Part of the C ABI; see the parent module for what these shims guarantee. Every request is a small JSON document so a host builds it with the JSON library it already has, and a field added later does not change a signature.

use super::with_bounded_bytes;
use crate::*;
use msime_client_services::account::BackendAccountClient;
use msime_client_services::notices::{fetch_notices, markdown_to_html, NoticeChannel, NoticeStore};
use msime_client_services::telemetry::{TelemetryApp, TelemetryStore};
use msime_client_services::update_check::{check_for_update, UpdateCheckRequest};
use std::time::SystemTime;

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
    /// The host's own `usage_reporting` switch, for hosts that keep preferences of their own.
    #[serde(default)]
    enabled: Option<bool>,
    /// The shared preferences directory (`preferences.json`), read for `usage_reporting` when `enabled` is absent.
    #[serde(default)]
    preferences_directory: Option<String>,
}

impl TelemetryRequest {
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
        let store = TelemetryStore::new(absolute_directory(&self.directory)?);
        let app =
            TelemetryApp::new(&self.platform, &self.version).map_err(|error| error.to_string())?;
        Ok((store, app))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DirectoryRequest {
    directory: String,
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
        let request: TelemetryRequest = unsafe { document(request, length, MAX_REQUEST_BYTES)? };
        let (store, app) = request.open()?;
        match request.consent()? {
            Some(true) => {
                let start = store
                    .begin_session(&app, SystemTime::now())
                    .map_err(|error| error.to_string())?;
                let mut value = serde_json::to_value(start).map_err(|error| error.to_string())?;
                value["enabled"] = Value::Bool(true);
                Ok(value)
            }
            Some(false) => {
                store.clear().map_err(|error| error.to_string())?;
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
}

/// 从可以分配内存的崩溃处理（如 C++ terminate handler）写会话的崩溃记录（`{directory, message, stack, session?}`）。带 `session` 时写这个会话的记录；不带时写标记里记着的会话的记录，而标记可能已属于同目录里别的进程。不加锁，不联网；记录在下次启动时成为 `crash` 事件。没有会话在运行或这个会话已有记录时值为 false。信号处理函数改为直接写 `msime_client_telemetry_begin` 返回的记录文件。
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
        match request.session.as_deref() {
            Some(session) => store.record_crash_for(session, &request.message, &request.stack),
            None => store.record_crash(&request.message, &request.stack),
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
        let request: TelemetryRequest = unsafe { document(request, length, MAX_REQUEST_BYTES)? };
        let (store, app) = request.open()?;
        match request.consent()? {
            Some(true) => {
                let client = BackendAccountClient::new().map_err(|error| error.to_string())?;
                let report = store
                    .flush_with(&app, &client, SystemTime::now())
                    .map_err(|error| error.to_string())?;
                let mut value = serde_json::to_value(report).map_err(|error| error.to_string())?;
                value["enabled"] = Value::Bool(true);
                Ok(value)
            }
            Some(false) => {
                store.clear().map_err(|error| error.to_string())?;
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
        let request: DirectoryRequest = unsafe { document(request, length, MAX_REQUEST_BYTES)? };
        TelemetryStore::new(absolute_directory(&request.directory)?)
            .clear()
            .map(|()| Value::Bool(true))
            .map_err(|error| error.to_string())
    })
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

/// 把宿主所在平台最新的已发布版本与当前运行的版本号比较，结果为 `{status:"available"|"current", update:{version:{display,parts}, release_url, installer_name, installer_sha256, signed}}`；该平台还没有发布时为 `{status:"none"}`。请求为 `{platform, current_version, edition?, arch?}`。会在网络上阻塞最多十秒，不要在 UI 线程或输入线程上调用。
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
