//! Voice provider wire formats and the streaming recognition exchange.
//!
//! Part of the C ABI; see the parent module for what these shims guarantee.

use crate::*;
use msime_client_core::{is_bounded_chars_without_nul, is_bounded_text};

/// 设置页的语音接口地址提示；与运行时共用 URL 校验，不读写偏好或联网。
///
/// # Safety
/// `endpoint` 必须指向本次调用期间可读的 `length` 个字节；空引用或超限会被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_endpoint_issue(
    endpoint: *const u8,
    length: usize,
    websocket: bool,
) -> *mut c_char {
    response(|| unsafe {
        with_bounded_bytes(
            endpoint,
            length,
            65_536,
            "invalid voice endpoint",
            |bytes| {
                let endpoint =
                    std::str::from_utf8(bytes).map_err(|_| "invalid voice endpoint".to_owned())?;
                let issue = msime_client_providers::voice::provider_settings::endpoint_issue(
                    endpoint, websocket,
                );
                Ok(json!({"accepted": issue.is_none(), "issue": issue}))
            },
        )
    })
}

/// 设置页保存语音凭据前的共享预检；不回显凭据。
///
/// # Safety
/// `provider` 与 `value` 必须分别指向本次调用期间可读的 `length` 个字节。
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_credential_issue(
    provider: *const u8,
    provider_length: usize,
    value: *const u8,
    value_length: usize,
) -> *mut c_char {
    response(|| unsafe {
        with_bounded_bytes(
            provider,
            provider_length,
            256,
            "invalid voice credential",
            |bytes| {
                let provider = std::str::from_utf8(bytes)
                    .map_err(|_| "invalid voice credential".to_owned())?;
                with_bounded_bytes(
                    value,
                    value_length,
                    16 * 1024,
                    "invalid voice credential",
                    |bytes| {
                        let value = std::str::from_utf8(bytes)
                            .map_err(|_| "invalid voice credential".to_owned())?;
                        let accepted =
                            msime_client_providers::voice::provider::valid_voice_credential(
                                provider, value,
                            );
                        Ok(json!({"accepted": accepted}))
                    },
                )
            },
        )
    })
}

/// 生成 Android 流式 WebSocket 使用的规范化传输描述；不返回原始地址或凭据。
///
/// # Safety
/// `endpoint` 必须指向本次调用期间可读的 `length` 个字节；空引用或超限会被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_websocket_endpoint(
    endpoint: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| unsafe {
        with_bounded_bytes(
            endpoint,
            length,
            65_536,
            "invalid voice websocket endpoint",
            |bytes| {
                let endpoint = std::str::from_utf8(bytes)
                    .map_err(|_| "invalid voice websocket endpoint".to_owned())?;
                let descriptor =
                    msime_client_providers::voice::provider::websocket_endpoint_descriptor(
                        endpoint,
                    )
                    .ok_or_else(|| "invalid voice websocket endpoint".to_owned())?;
                serde_json::to_value(descriptor)
                    .map_err(|_| "invalid voice websocket endpoint".to_owned())
            },
        )
    })
}

/// 语音设置的公开服务目录；纯函数，不访问偏好或设备。返回值用标准 JSON 信封封装。
#[no_mangle]
pub extern "C" fn msime_client_voice_provider_catalog() -> *mut c_char {
    response(|| {
        use msime_client_providers::voice::provider_catalog::{
            ASR_PROVIDERS, DOUBAO_STREAM_ENDPOINTS, POLISH_PROVIDERS,
        };
        Ok(json!({
            "asr": ASR_PROVIDERS, "polish": POLISH_PROVIDERS,
            "doubao_stream_endpoints": DOUBAO_STREAM_ENDPOINTS
        }))
    })
}

/// 设置页识别服务行的状态文案；纯计算，不读取偏好或设备状态。
///
/// # Safety
/// `provider` 必须指向本次调用期间可读的 UTF-8 字节；空服务用 `system` 或空串表示。
#[no_mangle]
pub unsafe extern "C" fn msime_client_mobile_voice_status(
    provider: *const u8,
    provider_length: usize,
    resolved: bool,
    platform_available: bool,
    model_ready: bool,
    runtime_ready: bool,
) -> *mut c_char {
    response(|| unsafe {
        with_bounded_bytes(
            provider,
            provider_length,
            256,
            "invalid voice status provider",
            |bytes| {
                let provider = std::str::from_utf8(bytes)
                    .map_err(|_| "invalid voice status provider".to_owned())?;
                let status = if provider == "local" {
                    msime_client_providers::voice::provider::mobile_voice_local_status(
                        model_ready,
                        runtime_ready,
                    )
                } else {
                    msime_client_providers::voice::provider::mobile_voice_provider_status(
                        provider,
                        resolved,
                        platform_available,
                    )
                };
                Ok(json!({"status": status}))
            },
        )
    })
}

/// 设置页润色行的状态文案；纯计算，不读取偏好或设备状态。
#[no_mangle]
pub extern "C" fn msime_client_mobile_voice_polish_status(
    enabled: bool,
    configured: bool,
    has_token: bool,
) -> *mut c_char {
    response(|| {
        let status = msime_client_providers::voice::provider::mobile_voice_polish_status(
            enabled, configured, has_token,
        );
        Ok(json!({"status": status}))
    })
}

/// 设置页语音服务分类和豆包鉴权模式；纯计算，不读取偏好或设备状态。
///
/// # Safety
/// `provider` 与 `auth_mode` 必须分别指向本次调用期间可读的 UTF-8 字节。
#[no_mangle]
pub unsafe extern "C" fn msime_client_mobile_voice_policy(
    provider: *const u8,
    provider_length: usize,
    auth_mode: *const u8,
    auth_mode_length: usize,
) -> *mut c_char {
    response(|| unsafe {
        with_bounded_bytes(
            provider,
            provider_length,
            256,
            "invalid voice provider policy",
            |provider_bytes| {
                with_bounded_bytes(
                    auth_mode,
                    auth_mode_length,
                    256,
                    "invalid voice provider policy",
                    |auth_mode_bytes| {
                        let provider = std::str::from_utf8(provider_bytes)
                            .map_err(|_| "invalid voice provider policy".to_owned())?;
                        let auth_mode = std::str::from_utf8(auth_mode_bytes)
                            .map_err(|_| "invalid voice provider policy".to_owned())?;
                        let policy =
                            msime_client_providers::voice::provider::mobile_voice_provider_policy(
                                provider, auth_mode,
                            );
                        Ok(json!({
                            "network": policy.network,
                            "legacy_auth": policy.legacy_auth,
                        }))
                    },
                )
            },
        )
    })
}

#[derive(serde::Deserialize)]
struct VoiceProviderUpdateRequest {
    service: msime_client_providers::voice::provider_settings::ProviderService,
    provider: String,
    current: msime_client_providers::voice::provider_settings::ProviderFields,
    #[serde(default)]
    field_policy: msime_client_providers::voice::provider_settings::ProviderFieldPolicy,
}

/// 计算语音设置的换服务补丁；请求与返回均含凭据，不能记录到日志。
///
/// # Safety
/// `request` 必须指向本次调用期间可读的 `length` 个字节。
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_provider_update(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        let query: VoiceProviderUpdateRequest = local_voice_request(request, length, 1 << 20)?;
        Ok(json!(
            msime_client_providers::voice::provider_settings::provider_update(
                query.service,
                &query.provider,
                &query.current,
                query.field_policy,
            )
        ))
    })
}

/// Decode one Doubao v1 response frame for Apple hosts. The returned payload
/// is UTF-8 JSON text; no frame bytes or credentials are retained.
///
/// # Safety
/// `frame` must reference a readable buffer for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn msime_client_doubao_decode_frame(
    frame: *const u8,
    frame_length: usize,
) -> *mut c_char {
    response(|| {
        if frame.is_null() || frame_length == 0 || frame_length > 1_048_576 {
            return Err("invalid Doubao frame buffer".into());
        }
        let bytes = unsafe { std::slice::from_raw_parts(frame, frame_length) };
        if let Some((last, _sequence, payload)) = decode_json_frame(bytes) {
            let text = String::from_utf8(payload).map_err(|_| "Doubao payload is not UTF-8")?;
            return Ok(json!({ "last": last, "payload": text }));
        }
        if let Some(code) = decode_error_code(bytes) {
            return Ok(json!({ "error_code": code }));
        }
        Err("invalid Doubao response frame".into())
    })
}

unsafe fn write_doubao_frame(
    frame: Vec<u8>,
    output: *mut u8,
    output_capacity: usize,
    output_length: *mut usize,
) -> bool {
    // A null, zero-capacity output is the sizing probe used by mobile bindings. Report the
    // required length before returning false so the caller can allocate exactly one frame. A null
    // output with nonzero capacity is still an invalid destination and must not be accepted.
    if output_length.is_null() || (output.is_null() && output_capacity != 0) {
        return false;
    }
    *output_length = frame.len();
    if output_capacity < frame.len() {
        return false;
    }
    std::ptr::copy_nonoverlapping(frame.as_ptr(), output, frame.len());
    true
}

/// Build a Doubao start request into caller-owned storage.
///
/// # Safety
/// `boosting_table_id` must point to `boosting_table_id_length` readable bytes when the length is
/// nonzero. `output` must point to `output_capacity` writable bytes and `output_length` must point
/// to a writable `usize`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_doubao_start_frame(
    enable_itn: bool,
    enable_punc: bool,
    enable_ddc: bool,
    boosting_table_id: *const u8,
    boosting_table_id_length: usize,
    output: *mut u8,
    output_capacity: usize,
    output_length: *mut usize,
) -> bool {
    if boosting_table_id_length > 4096
        || (boosting_table_id.is_null() && boosting_table_id_length != 0)
    {
        return false;
    }
    let boosting = if boosting_table_id_length == 0 {
        ""
    } else {
        let bytes =
            unsafe { std::slice::from_raw_parts(boosting_table_id, boosting_table_id_length) };
        match std::str::from_utf8(bytes) {
            Ok(value) => value,
            Err(_) => return false,
        }
    };
    unsafe {
        write_doubao_frame(
            start_frame(enable_itn, enable_punc, enable_ddc, boosting),
            output,
            output_capacity,
            output_length,
        )
    }
}

/// Build a Doubao PCM or final audio frame into caller-owned storage.
///
/// # Safety
/// `pcm` must point to `pcm_length` readable bytes when the length is nonzero. `output` must point
/// to `output_capacity` writable bytes and `output_length` must point to a writable `usize`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_doubao_audio_frame(
    sequence: i32,
    pcm: *const u8,
    pcm_length: usize,
    final_chunk: bool,
    output: *mut u8,
    output_capacity: usize,
    output_length: *mut usize,
) -> bool {
    // `i32::abs()` panics for its sole unrepresentable input. Reject it at
    // the C boundary so hostile sequence values cannot abort the host process.
    if sequence == i32::MIN || pcm_length > 1_048_576 || (pcm.is_null() && pcm_length != 0) {
        return false;
    }
    let bytes = if pcm_length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(pcm, pcm_length) }
    };
    unsafe {
        write_doubao_frame(
            audio_frame(sequence, bytes, final_chunk),
            output,
            output_capacity,
            output_length,
        )
    }
}

/// Run one bounded voice capture/ASR request through a user-owned Unix socket.
/// The socket service owns microphone access, credentials and network policy.
/// The query is a bounded JSON object containing `language`, `generation`,
/// and optional non-sensitive voice behavior `options`.
///
/// # Safety
/// All pointers must reference readable buffers of the stated lengths for
/// the duration of this call; the buffers are not retained.
#[cfg(unix)]
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_provider_request(
    query: *const u8,
    query_length: usize,
    socket_path: *const u8,
    socket_length: usize,
) -> *mut c_char {
    response(|| {
        if query.is_null() || socket_path.is_null() || query_length > 16_384 || socket_length > 4096
        {
            return Err("invalid voice provider buffer".into());
        }
        #[derive(Deserialize)]
        struct VoiceQuery {
            language: String,
            generation: u64,
            #[serde(default)]
            options: Value,
        }
        let query = serde_json::from_slice::<VoiceQuery>(unsafe {
            std::slice::from_raw_parts(query, query_length)
        })
        .map_err(|_| "invalid voice query document")?;
        let path = super::parse_absolute_socket_path(unsafe {
            std::slice::from_raw_parts(socket_path, socket_length)
        })?;
        Ok(UnixSocketProvider::new(path)
            .voice_with_options(&query.language, query.generation, &query.options)
            .map(|text| json!({"text": text}))
            .unwrap_or(Value::Null))
    })
}

/// Stream bounded interim/final voice provider updates from a user-owned
/// Unix socket. The callback is invoked synchronously on the calling thread.
///
/// # Safety
/// Buffers must remain readable for the duration of this call. The callback
/// must remain valid and must copy the text before returning.
#[cfg(unix)]
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_provider_stream(
    query: *const u8,
    query_length: usize,
    socket_path: *const u8,
    socket_length: usize,
    callback: Option<unsafe extern "C" fn(*const u8, usize, bool, *mut c_void)>,
    context: *mut c_void,
) -> *mut c_char {
    unsafe {
        msime_client_voice_provider_stream_events(
            query,
            query_length,
            socket_path,
            socket_length,
            callback,
            None,
            context,
        )
    }
}

/// Stream voice text and optional phase notifications (0 recording, 1 recognizing, 2 polishing).
///
/// # Safety
/// Buffers and callbacks must remain valid for this synchronous call. Callbacks must not unwind.
#[cfg(unix)]
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_provider_stream_events(
    query: *const u8,
    query_length: usize,
    socket_path: *const u8,
    socket_length: usize,
    callback: Option<unsafe extern "C" fn(*const u8, usize, bool, *mut c_void)>,
    status_callback: Option<unsafe extern "C" fn(u8, *mut c_void)>,
    context: *mut c_void,
) -> *mut c_char {
    unsafe {
        msime_client_voice_provider_stream_feedback(
            query,
            query_length,
            socket_path,
            socket_length,
            callback,
            status_callback,
            None,
            context,
        )
    }
}

/// Stream voice text, phases and optional normalized microphone levels.
///
/// # Safety
/// Buffers and callbacks must remain valid for this synchronous call. Callbacks must not unwind.
#[cfg(unix)]
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_provider_stream_feedback(
    query: *const u8,
    query_length: usize,
    socket_path: *const u8,
    socket_length: usize,
    callback: Option<unsafe extern "C" fn(*const u8, usize, bool, *mut c_void)>,
    status_callback: Option<unsafe extern "C" fn(u8, *mut c_void)>,
    level_callback: Option<unsafe extern "C" fn(f32, *mut c_void)>,
    context: *mut c_void,
) -> *mut c_char {
    unsafe {
        msime_client_voice_provider_stream_feedback_cancelled(
            query,
            query_length,
            socket_path,
            socket_length,
            callback,
            status_callback,
            level_callback,
            None,
            context,
        )
    }
}

/// Stream voice text, phases and levels while polling a host-owned cancellation callback.
///
/// # Safety
/// Buffers and callbacks must remain valid for the duration of this synchronous call. Callbacks
/// must not unwind; the cancellation callback must be safe to call from the stream worker.
#[cfg(unix)]
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_provider_stream_feedback_cancelled(
    query: *const u8,
    query_length: usize,
    socket_path: *const u8,
    socket_length: usize,
    callback: Option<unsafe extern "C" fn(*const u8, usize, bool, *mut c_void)>,
    status_callback: Option<unsafe extern "C" fn(u8, *mut c_void)>,
    level_callback: Option<unsafe extern "C" fn(f32, *mut c_void)>,
    cancellation_callback: Option<unsafe extern "C" fn(*mut c_void) -> bool>,
    context: *mut c_void,
) -> *mut c_char {
    response(|| {
        if query.is_null() || socket_path.is_null() || query_length > 16_384 || socket_length > 4096
        {
            return Err("invalid voice provider buffer".into());
        }
        #[derive(Deserialize)]
        struct VoiceQuery {
            language: String,
            generation: u64,
            #[serde(default)]
            options: Value,
        }
        let query = serde_json::from_slice::<VoiceQuery>(unsafe {
            std::slice::from_raw_parts(query, query_length)
        })
        .map_err(|_| "invalid voice query document")?;
        let path = super::parse_absolute_socket_path(unsafe {
            std::slice::from_raw_parts(socket_path, socket_length)
        })?;
        let mut update = |text: &str, final_result: bool| {
            if let Some(callback) = callback {
                unsafe {
                    callback(text.as_ptr(), text.len(), final_result, context);
                }
            }
        };
        let mut status = |phase: &str| {
            if let Some(callback) = status_callback {
                let value = match phase {
                    "recording" => 0,
                    "recognizing" => 1,
                    "polishing" => 2,
                    _ => return,
                };
                unsafe {
                    callback(value, context);
                }
            }
        };
        let mut level = |value: f32| {
            if let Some(callback) = level_callback {
                unsafe {
                    callback(value, context);
                }
            }
        };
        let mut cancellation =
            || cancellation_callback.is_some_and(|callback| unsafe { callback(context) });
        let value = UnixSocketProvider::new(path)
            .voice_stream_with_options_diagnosed_with_callback(
                &query.language,
                query.generation,
                &query.options,
                cancellation_callback.map(|_| &mut cancellation as &mut dyn FnMut() -> bool),
                &mut update,
                if status_callback.is_some() {
                    Some(&mut status)
                } else {
                    None
                },
                if level_callback.is_some() {
                    Some(&mut level)
                } else {
                    None
                },
            );
        match value {
            Ok(text) => Ok(json!({"text": text})),
            // A named missing dependency is the one provider failure reported as an error, so hosts can show what to install; older callers see it as any other failed call.
            Err(Some(detail)) => Err(format!("voice_dependency_missing:{detail}")),
            Err(None) => Ok(Value::Null),
        }
    })
}

/// Ask a user-owned Unix socket to stop voice capture for one generation.
///
/// # Safety
/// `socket_path` must reference a readable UTF-8 buffer for this call.
#[cfg(unix)]
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_provider_cancel(
    socket_path: *const u8,
    socket_length: usize,
    generation: u64,
) -> *mut c_char {
    response(|| {
        if socket_path.is_null() || socket_length > 4096 {
            return Err("invalid voice provider socket buffer".into());
        }
        let path = super::parse_absolute_socket_path(unsafe {
            std::slice::from_raw_parts(socket_path, socket_length)
        })?;
        Ok(json!(UnixSocketProvider::new(path).voice_cancel(generation)))
    })
}

/// Ask a user-owned voice socket to finish capture and return its final stream
/// result. The streaming connection remains responsible for delivering text.
///
/// # Safety
/// `socket_path` must reference a readable UTF-8 buffer for this call.
#[cfg(unix)]
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_provider_stop(
    socket_path: *const u8,
    socket_length: usize,
    generation: u64,
) -> *mut c_char {
    response(|| {
        if socket_path.is_null() || socket_length > 4096 {
            return Err("invalid voice provider socket buffer".into());
        }
        let path = super::parse_absolute_socket_path(unsafe {
            std::slice::from_raw_parts(socket_path, socket_length)
        })?;
        Ok(json!(UnixSocketProvider::new(path).voice_stop(generation)))
    })
}

// ---- on-device models and hotwords ----

/// Largest request any of the local-model and hotword calls accepts. The hotword correction request carries a transcript and up to a few hundred hotwords.
const LOCAL_VOICE_REQUEST_LIMIT: usize = 1 << 20;
/// Keep the pinyin matcher bounded even when a caller invokes the C ABI directly instead of using
/// the mobile request validator, which applies these same per-field limits.
const MAX_VOICE_CORRECTION_TEXT_CHARS: usize = 10_000;
const MAX_VOICE_CORRECTION_HOTWORDS: usize = 1_000;
const MAX_VOICE_CORRECTION_HOTWORD_TEXT_BYTES: usize = 256;
const MAX_VOICE_CORRECTION_HOTWORD_PINYIN_BYTES: usize = 1_024;

fn local_voice_request<T: serde::de::DeserializeOwned>(
    request: *const u8,
    length: usize,
    limit: usize,
) -> Result<T, String> {
    if request.is_null() || length == 0 || length > limit {
        return Err("invalid voice request buffer".into());
    }
    // SAFETY: the caller contract of every entry point using this guarantees `length` readable bytes; null and size are checked above.
    let bytes = unsafe { std::slice::from_raw_parts(request, length) };
    serde_json::from_slice(bytes).map_err(|_| "invalid voice request".to_owned())
}

fn local_model_root(root: &str) -> Result<&Path, String> {
    if !is_bounded_text(root, 4096) {
        return Err("invalid local model root".into());
    }
    let path = Path::new(root);
    if !path.is_absolute() {
        return Err("invalid local model root".into());
    }
    Ok(path)
}

/// 一次「从本地文件安装」最多带几个文件。目录里的模型最多要两个（压缩包和一个附加文件），留出用户多选几个无关文件的余地。
const MAX_LOCAL_MODEL_IMPORT_FILES: usize = 16;

/// Cancellation flags of the installs running in this process, by model id.
fn local_model_installs() -> &'static Mutex<HashMap<String, Arc<std::sync::atomic::AtomicBool>>> {
    static INSTALLS: OnceLock<Mutex<HashMap<String, Arc<std::sync::atomic::AtomicBool>>>> =
        OnceLock::new();
    INSTALLS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Hotwords for on-device recognition from the user's own pinyin dictionary words.
///
/// Request `{"options": HostOptions, "limit": 200}`, the same HostOptions `msime_client_dictionary` takes. Response `{"hotwords":[{"text","pinyin"}]}`, highest-weighted words first. Reads through the dictionary list action, so it fails with "dictionary maintenance busy" while a maintenance writer holds the store.
///
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_hotwords(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        struct HotwordsRequest {
            options: Value,
            #[serde(default)]
            limit: Option<usize>,
        }
        let request: HotwordsRequest =
            local_voice_request(request, length, HOST_OPTIONS_DOCUMENT_LIMIT)?;
        let limit = request
            .limit
            .unwrap_or(msime_client_providers::voice::hotwords::DEFAULT_HOTWORD_LIMIT)
            .min(1_000);
        // Enough rows that the heaviest words can be picked even from a large dictionary, without reading the whole store for every voice session.
        let hotwords = msime_client_providers::voice::hotwords::hotwords_from_dictionary_pages(
            limit,
            1_000,
            5_000,
            |offset, page_size| {
                let page = crate::dictionary_request_json(
                    &serde_json::to_vec(&json!({
                        "options": request.options,
                        "action": {"operation": "list", "offset": offset, "limit": page_size, "kind": "pinyin", "user_only": true},
                    }))
                    .map_err(|_| "invalid voice request")?,
                )?;
                let entries = page["entries"].as_array().cloned().unwrap_or_default();
                let mut hotword_entries = Vec::with_capacity(entries.len());
                hotword_entries.extend(entries.iter().filter_map(|entry| {
                    Some((
                        entry["value"].as_str()?.to_owned(),
                        entry["key"].as_str()?.to_owned(),
                        entry["weight"].as_i64().unwrap_or(0),
                    ))
                }));
                Ok::<Option<msime_client_providers::voice::hotwords::DictionaryHotwordPage>, String>(
                    Some(
                        msime_client_providers::voice::hotwords::DictionaryHotwordPage {
                            entries: hotword_entries,
                            has_more: page["has_more"].as_bool() == Some(true),
                        },
                    ),
                )
            },
        )?;
        Ok(json!({ "hotwords": hotwords }))
    })
}

/// Apply hotwords to a final transcript by pinyin similarity, for models whose manifest says `"hotwords": "pinyin"`. The transcript is limited to 10,000 Unicode scalar values, the list to 1,000 entries, and each entry to 256 bytes of text and 1,024 bytes of pinyin.
///
/// Request `{"text": "...", "hotwords": [{"text","pinyin"}]}` (at most 1 MiB); response `{"text": "..."}`. Pure; no state is read.
///
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_hotword_correct(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        struct CorrectRequest {
            text: String,
            #[serde(default)]
            hotwords: Vec<msime_client_providers::voice::hotwords::Hotword>,
        }
        let request: CorrectRequest =
            local_voice_request(request, length, LOCAL_VOICE_REQUEST_LIMIT)?;
        if !is_bounded_chars_without_nul(&request.text, MAX_VOICE_CORRECTION_TEXT_CHARS)
            || request.hotwords.len() > MAX_VOICE_CORRECTION_HOTWORDS
            || request.hotwords.iter().any(|hotword| {
                hotword.text.trim().is_empty()
                    || !is_bounded_text(&hotword.text, MAX_VOICE_CORRECTION_HOTWORD_TEXT_BYTES)
                    || !is_bounded_text(&hotword.pinyin, MAX_VOICE_CORRECTION_HOTWORD_PINYIN_BYTES)
            })
        {
            return Err("invalid voice hotword correction request".into());
        }
        Ok(json!({
            "text": msime_client_providers::voice::hotwords::correct(&request.text, &request.hotwords),
        }))
    })
}

/// 本地模型的最终文本里，把汉字写的数字改成数字（二零零二年 → 2002年）。
///
/// 请求 `{"text": "...", "kind": "<msime-model.json 的 kind>"}`，响应 `{"text": "..."}`。只有不自带 ITN 的模型（`voice::numbers::applies_to_kind`）会被改写，其他 kind 原样返回，所以宿主对每个本地模型的最终文本都可以调用它。纯函数；文本上限与 `msime_client_voice_hotword_correct` 相同。
///
/// # Safety
/// `request` 必须指向 `length` 个可读字节；null 会被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_normalize_numbers(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        struct NumbersRequest {
            text: String,
            kind: String,
        }
        let request: NumbersRequest =
            local_voice_request(request, length, LOCAL_VOICE_REQUEST_LIMIT)?;
        if !is_bounded_chars_without_nul(&request.text, MAX_VOICE_CORRECTION_TEXT_CHARS)
            || !is_bounded_text(&request.kind, 64)
        {
            return Err("invalid voice number normalization request".into());
        }
        let text = if msime_client_providers::voice::numbers::applies_to_kind(&request.kind) {
            msime_client_providers::voice::numbers::convert(&request.text)
        } else {
            request.text
        };
        Ok(json!({ "text": text }))
    })
}

/// The on-device model catalog with what is installed under a root.
///
/// Request `{"root": "<absolute dir>"}`; response `{"models": [LocalModelStatus], "default": "<id>"}`. A model's `path` is what `voice_input.asr_model_path` is set to when the user picks it.
///
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_local_models(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        struct ModelsRequest {
            root: String,
        }
        let request: ModelsRequest = local_voice_request(request, length, 16_384)?;
        let root = local_model_root(&request.root)?;
        Ok(json!({
            "models": msime_client_providers::voice::local_models::list(root),
            "default": msime_client_providers::voice::local_models::default_model_id(),
        }))
    })
}

/// 下载、校验并安装一个目录模型。阻塞到结束，必须在工作线程上调用。
///
/// 请求 `{"root": "<absolute dir>", "id": "<catalog id>", "mirror": ""}`，响应 `{"path": "<root>/<id>"}`。`progress` 可为空，在调用线程上收到 `{"id","stage","downloaded","total"}` JSON，阶段为 download、verify、extract 或 done，缓冲区只在回调期间有效。登记后、访问文件或网络前先报告 download（导入为 import）的零字节进度，回调可重入 `msime_client_voice_local_model_cancel` 取消本次安装。取消入口可从任何线程调用，安装随后报 "local_model_cancelled"。每个 id 同时只允许一次安装，重复调用报 "local_model_install_running"。其余失败使用 "local_model_*" 错误码（network、http_status、size_mismatch、checksum_mismatch、unsafe_archive、missing_file、io、invalid_mirror、unknown）。
///
/// 请求带 `"files": ["<absolute path>", ...]`（至多 16 个）时不联网，改用用户自己下载好的这些文件安装（`msime_client_providers::voice::local_models::import`）：按长度和 SHA-256 认文件，不看文件名，`mirror` 不用；进度里原来的 download 阶段报成 import；缺文件时失败为 "local_model_import_missing: <上游文件名>"，所选文件打不开或读出错时为 "local_model_import_unreadable: <说明>"。和下载共用同一个按 id 的登记，所以取消、互斥和删除时的拒绝都一样。
///
/// # Safety
/// `request` 必须指向 `length` 字节的可读缓冲区。`progress` 必须在整个调用期间有效，在返回前复制缓冲区，且不得展开栈。
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_local_model_install(
    request: *const u8,
    length: usize,
    progress: Option<unsafe extern "C" fn(*const u8, usize, *mut std::ffi::c_void)>,
    context: *mut std::ffi::c_void,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        struct InstallRequest {
            root: String,
            id: String,
            #[serde(default)]
            mirror: String,
            #[serde(default)]
            files: Option<Vec<String>>,
        }
        let request: InstallRequest = local_voice_request(request, length, 16_384)?;
        let root = local_model_root(&request.root)?;
        let files = match &request.files {
            Some(files) => {
                if files.len() > MAX_LOCAL_MODEL_IMPORT_FILES
                    || files
                        .iter()
                        .any(|file| !is_bounded_text(file, 4096) || !Path::new(file).is_absolute())
                {
                    return Err("invalid local model import".into());
                }
                Some(files.iter().map(PathBuf::from).collect::<Vec<_>>())
            }
            None => None,
        };
        let model = msime_client_providers::voice::local_models::catalog()
            .models
            .iter()
            .find(|model| model.id == request.id)
            .ok_or("local_model_unknown")?;
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let mut installs = local_model_installs()
                .lock()
                .map_err(|_| "internal runtime failure")?;
            if installs.contains_key(&request.id) {
                return Err("local_model_install_running".into());
            }
            installs.insert(request.id.clone(), cancel.clone());
        }
        struct Registered<'a>(&'a str);
        impl Drop for Registered<'_> {
            fn drop(&mut self) {
                if let Ok(mut installs) = local_model_installs().lock() {
                    installs.remove(self.0);
                }
            }
        }
        let _registered = Registered(&request.id);
        let mut report = |event: msime_client_providers::voice::local_models::InstallProgress| {
            if let Some(callback) = progress {
                let text = json!({
                    "id": request.id,
                    "stage": event.stage,
                    "downloaded": event.downloaded,
                    "total": event.total,
                })
                .to_string();
                unsafe {
                    callback(text.as_ptr(), text.len(), context);
                }
            }
        };
        // 登记锁已释放，宿主可在首次回调中补发登记前的取消，避免先进入阻塞 I/O。
        report(
            msime_client_providers::voice::local_models::InstallProgress {
                stage: if files.is_some() {
                    "import"
                } else {
                    "download"
                },
                downloaded: 0,
                total: model.archive.size,
            },
        );
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("local_model_cancelled".into());
        }
        let path = match &files {
            Some(files) => msime_client_providers::voice::local_models::import(
                root,
                &request.id,
                files,
                &mut report,
                &cancel,
            ),
            None => msime_client_providers::voice::local_models::install(
                root,
                &request.id,
                &request.mirror,
                &mut report,
                &cancel,
            ),
        }
        .map_err(|error| error.to_string())?;
        Ok(json!({ "path": path.to_string_lossy() }))
    })
}

/// Stop a running install. Request `{"id": "<catalog id>"}` cancels that model's install; a null request, or one without `id`, cancels every install in this process. Response value: whether anything was running. The install call itself returns once it notices, between chunks.
///
/// # Safety
/// `request` must be null or point to `length` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_local_model_cancel(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        struct CancelRequest {
            #[serde(default)]
            id: Option<String>,
        }
        let id = if request.is_null() || length == 0 {
            None
        } else {
            local_voice_request::<CancelRequest>(request, length, 16_384)?.id
        };
        let installs = local_model_installs()
            .lock()
            .map_err(|_| "internal runtime failure")?;
        let mut cancelled = false;
        for (running, flag) in installs.iter() {
            if id.as_ref().is_none_or(|id| id == running) {
                flag.store(true, std::sync::atomic::Ordering::Relaxed);
                cancelled = true;
            }
        }
        Ok(json!(cancelled))
    })
}

/// Delete an installed model. Request `{"root": "<absolute dir>", "id": "<catalog id>"}`; only catalog ids are accepted. Removing a model that is not installed succeeds. Response value: null.
///
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_local_model_remove(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        struct RemoveRequest {
            root: String,
            id: String,
        }
        let request: RemoveRequest = local_voice_request(request, length, 16_384)?;
        let root = local_model_root(&request.root)?;
        // Hold the registry across the removal: releasing it after the check would let an
        // install of the same id register and have its staging directory swept mid-download.
        let installs = local_model_installs()
            .lock()
            .map_err(|_| "internal runtime failure")?;
        if installs.contains_key(&request.id) {
            return Err("local_model_install_running".into());
        }
        let removed = msime_client_providers::voice::local_models::remove(root, &request.id);
        drop(installs);
        removed.map_err(|error| error.to_string())?;
        Ok(Value::Null)
    })
}

/// 偏好库会不会接受本地识别的两个字段，与 `Preferences::validate` 的判断相同：`asr_model_mirror` 按 [`msime_client_core::preferences::valid_model_mirror`]，`asr_model_path` 按 [`msime_client_core::preferences::valid_model_path`]。原生设置窗口边输入边保存，只把这里认可的值写进设置，免得一个半截的镜像地址让整份偏好保存失败。
///
/// Request `{"asr_model_mirror"?: string, "asr_model_path"?: string}`（至多 64 KiB）；value 只含请求里给出的键，各为一个布尔值。纯函数，不读任何状态。
///
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_model_setting_check(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        struct CheckRequest {
            #[serde(default)]
            asr_model_mirror: Option<String>,
            #[serde(default)]
            asr_model_path: Option<String>,
        }
        let request: CheckRequest = local_voice_request(request, length, 64 * 1024)?;
        let mut value = serde_json::Map::new();
        if let Some(mirror) = &request.asr_model_mirror {
            value.insert(
                "asr_model_mirror".into(),
                Value::Bool(msime_client_core::preferences::valid_model_mirror(mirror)),
            );
        }
        if let Some(path) = &request.asr_model_path {
            value.insert(
                "asr_model_path".into(),
                Value::Bool(msime_client_core::preferences::valid_model_path(path)),
            );
        }
        Ok(Value::Object(value))
    })
}
