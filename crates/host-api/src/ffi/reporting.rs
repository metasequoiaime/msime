//! Usage reporting and notices for native hosts: the telemetry queue, the notice feed and the Markdown renderer of `msime_client_core::telemetry` and `msime_client_core::notices`.
//!
//! Part of the C ABI; see the parent module for what these shims guarantee. Every request is a small JSON document so a host builds it with the JSON library it already has, and a field added later does not change a signature.

use super::with_bounded_bytes;
use crate::*;
use msime_client_core::account::BackendAccountClient;
use msime_client_core::notices::{fetch_notices, markdown_to_html, NoticeChannel, NoticeStore};
use msime_client_core::telemetry::{TelemetryApp, TelemetryStore};
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

/// Starts a reporting session at host start. With reporting on: closes the previous session, queues its crash records and today's `active`, writes the new session marker and returns `{enabled:true, crash_record_path, previous_session_crashed, crashes}`. With reporting off: clears the queue, marker and crash records and returns `{enabled:false}`. No network I/O.
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

/// Ends the running session normally (`{directory}`), queueing its `session` event for the next flush. Value is false when no session was running. No network I/O, so it can run on a shutdown path.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_telemetry_end(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: guaranteed by the documented caller contract.
        let request: DirectoryRequest = unsafe { document(request, length, MAX_REQUEST_BYTES)? };
        TelemetryStore::new(absolute_directory(&request.directory)?)
            .end_session()
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
}

/// Writes the running session's crash record (`{directory, message, stack}`) from a crash handler that may allocate, such as a C++ terminate handler. Takes no lock and does no network I/O; the record becomes a `crash` event on the next start. Value is false when no session is running or this session already has a record. Signal handlers write the record file returned by `msime_client_telemetry_begin` directly instead.
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
        TelemetryStore::new(absolute_directory(&request.directory)?)
            .record_crash(&request.message, &request.stack)
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
