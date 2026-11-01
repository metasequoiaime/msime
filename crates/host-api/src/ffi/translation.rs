//! Translation and gloss: provider requests, response parsing, and applying the result.
//!
//! Part of the C ABI; see the parent module for what these shims guarantee.

use crate::*;
use msime_client_core::is_bounded_text;

fn traditional_retry_inputs(
    candidates: &[(String, u8)],
    glosses: &[String],
) -> (Vec<(String, u8)>, Vec<usize>) {
    let mut retry = Vec::with_capacity(candidates.len());
    let mut retry_index = Vec::with_capacity(candidates.len());
    for (index, ((text, source), gloss)) in candidates.iter().zip(glosses).enumerate() {
        if !gloss.is_empty() {
            continue;
        }
        let simplified =
            msime_client_core::chinese_conversion::traditional_to_simplified_characters(text);
        if simplified != *text {
            retry.push((simplified, *source));
            retry_index.push(index);
        }
    }
    (retry, retry_index)
}

/// Plan eligible visible candidates using shared script filters. No I/O.
/// # Safety
/// `request` must reference `length` readable bytes for this call.
#[no_mangle]
pub unsafe extern "C" fn msime_client_custom_translation_plan(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Candidate {
            text: String,
            source: u8,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            target_language: String,
            candidates: Vec<Candidate>,
        }
        let request: Request = unsafe {
            with_bounded_bytes(
                request,
                length,
                65536,
                "invalid translation plan buffer",
                |bytes| {
                    serde_json::from_slice(bytes).map_err(|_| "invalid translation plan".into())
                },
            )?
        };
        // 一次翻译一页候选，上限与每页候选数相同。
        if request.candidates.len()
            > usize::from(msime_client_core::preferences::MAX_CANDIDATE_PAGE_SIZE)
            || request.target_language == "zh"
            || !msime_client_core::translation::is_supported_translation_language(
                &request.target_language,
            )
        {
            return Err("invalid translation plan parameters".into());
        }
        let mut results = Vec::with_capacity(request.candidates.len());
        for candidate in request.candidates {
            // 普通候选包括键盘模型的整句（11）；特殊行、AI 占位和未知来源不进入翻译。
            if !matches!(candidate.source, 0..=5 | 8..=9 | 11)
                || !msime_client_core::translation::is_valid_source_text(&candidate.text)
            {
                continue;
            }
            let (source, target, key) =
                if msime_client_core::translation::is_cloud_translatable_english(&candidate.text) {
                    ("en", "zh", candidate.text.to_ascii_lowercase())
                } else if msime_client_core::translation::is_cloud_translatable_chinese(
                    &candidate.text,
                ) {
                    (
                        "zh",
                        request.target_language.as_str(),
                        candidate.text.clone(),
                    )
                } else {
                    continue;
                };
            let item = json!({"text":candidate.text,"key":key,"source_language":source,"target_language":target});
            if !results.contains(&item) {
                results.push(item);
            }
        }
        Ok(json!(results))
    })
}

/// Build a pure AI HTTP descriptor containing credentials. Never log it.
/// # Safety
/// `request` references `length` readable JSON bytes. No buffers are retained.
#[no_mangle]
pub unsafe extern "C" fn msime_client_ai_http_request(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            config: msime_client_core::preferences::AiAssistantPreferences,
            input: msime_client_providers::ai::AiSuggestionRequest,
        }
        let request: Request = unsafe {
            with_bounded_bytes(
                request,
                length,
                65536,
                "invalid AI request buffer",
                |bytes| {
                    serde_json::from_slice(bytes).map_err(|_| "invalid AI request document".into())
                },
            )?
        };
        msime_client_providers::ai::chat_completion_http_request(&request.config, &request.input)
            .map(|value| value.unwrap_or(Value::Null))
            .map_err(|e| e.to_string())
    })
}

/// 构造带凭据的 AI 翻译描述器。调用方只能在候选翻译已通过宿主功能和路由门禁后调用。
/// # Safety
/// `request` 引用 `length` 个可读 JSON 字节；函数不会保留任何缓冲区。
#[no_mangle]
pub unsafe extern "C" fn msime_client_ai_translation_http_request(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            config: msime_client_core::preferences::AiAssistantPreferences,
            input: msime_client_providers::ai::AiTranslationRequest,
        }
        let request: Request = unsafe {
            with_bounded_bytes(
                request,
                length,
                65536,
                "invalid AI translation request buffer",
                |bytes| {
                    serde_json::from_slice(bytes)
                        .map_err(|_| "invalid AI translation request document".into())
                },
            )?
        };
        msime_client_providers::ai::chat_translation_http_request(&request.config, &request.input)
            .map(|value| value.unwrap_or(Value::Null))
            .map_err(|e| e.to_string())
    })
}

/// 从 Chat Completions 响应中解析一个严格的 JSON 翻译结果，失败时返回空值。
/// # Safety
/// `body` 引用 `length` 个可读字节；函数不会保留任何缓冲区。
#[no_mangle]
pub unsafe extern "C" fn msime_client_parse_ai_translation_response(
    body: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if body.is_null() || length > 1048576 {
            return Err("invalid AI translation response buffer".into());
        }
        let bytes = unsafe { std::slice::from_raw_parts(body, length) };
        Ok(
            msime_client_providers::ai::parse_chat_translation_response(bytes)
                .map(Value::String)
                .unwrap_or(Value::Null),
        )
    })
}

/// 判断 Chat Completions 翻译回复是服务失败还是合法的空译文。
/// # Safety
/// `body` 引用 `length` 个可读字节；函数不会保留任何缓冲区。
#[no_mangle]
pub unsafe extern "C" fn msime_client_ai_translation_reply_failed(
    body: *const u8,
    length: usize,
) -> bool {
    if body.is_null() || length > 1_048_576 {
        return true;
    }
    msime_client_providers::ai::chat_translation_response_failed(unsafe {
        std::slice::from_raw_parts(body, length)
    })
}

/// Parse a successful AI HTTP response into a bounded string array, or null.
/// # Safety
/// `body` references `length` readable bytes. No buffers are retained.
#[no_mangle]
pub unsafe extern "C" fn msime_client_parse_ai_response(
    body: *const u8,
    length: usize,
    limit: u8,
) -> *mut c_char {
    response(|| {
        if body.is_null() || length > 1048576 || !(1..=10).contains(&limit) {
            return Err("invalid AI response buffer".into());
        }
        Ok(msime_client_providers::ai::parse_chat_completion_response(
            unsafe { std::slice::from_raw_parts(body, length) },
            limit,
        )
        .map(|response| {
            json!(response
                .candidates
                .into_iter()
                .map(|candidate| candidate.text)
                .collect::<Vec<_>>())
        })
        .unwrap_or(Value::Null))
    })
}

/// Read/write private learned glosses on a host-owned IO worker. Never log inputs.
/// # Safety
/// `request` must reference `length` readable bytes for this call.
#[no_mangle]
pub unsafe extern "C" fn msime_client_learned_translation_request(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| unsafe {
        with_bounded_bytes(
            request,
            length,
            65536,
            "invalid learned translation buffer",
            |bytes| learned_translation::execute(bytes).map_err(str::to_owned),
        )
    })
}

/// Build a signed Tencent TMT descriptor with an exact UTF-8 payload. No I/O.
/// # Safety
/// `request` must reference `length` readable bytes for this call.
#[no_mangle]
pub unsafe extern "C" fn msime_client_tencent_translation_http_request(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| unsafe {
        with_bounded_bytes(
            request,
            length,
            65536,
            "invalid Tencent request buffer",
            |bytes| tencent_translation::descriptor(bytes).map_err(String::from),
        )
    })
}

/// Build a NiuTrans v2 form descriptor. No network or credential persistence.
/// # Safety
/// `request` must reference `length` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn msime_client_niutrans_translation_http_request(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| unsafe {
        with_bounded_bytes(
            request,
            length,
            65536,
            "invalid NiuTrans request buffer",
            |bytes| niutrans_translation::descriptor(bytes).map_err(String::from),
        )
    })
}

/// Parse a bounded response preserving batch positions (unusable slots are null).
/// # Safety
/// `body` must reference `length` readable bytes for this call.
#[no_mangle]
pub unsafe extern "C" fn msime_client_parse_tencent_translation_response(
    body: *const u8,
    length: usize,
    expected: usize,
) -> *mut c_char {
    response(|| {
        if body.is_null()
            || length > 1048576
            || !(1..=usize::from(msime_client_core::preferences::MAX_CANDIDATE_PAGE_SIZE))
                .contains(&expected)
        {
            return Err("invalid Tencent response buffer".into());
        }
        Ok(tencent_translation::parse(
            unsafe { std::slice::from_raw_parts(body, length) },
            expected,
        )
        .unwrap_or(Value::Null))
    })
}

/// Parse one bounded NiuTrans response into a formatted gloss, or null.
/// # Safety
/// `body` must reference `length` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn msime_client_parse_niutrans_translation_response(
    body: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if body.is_null() || length > 1048576 {
            return Err("invalid NiuTrans response buffer".into());
        }
        Ok(
            niutrans_translation::parse(unsafe { std::slice::from_raw_parts(body, length) })
                .unwrap_or(Value::Null),
        )
    })
}

/// Format one gloss a host produced itself (Apple's on-device model, say) the way provider replies are formatted: whitespace runs collapse to one space, the ends are trimmed, and a gloss that is empty afterwards or carries a control character becomes null. A newline left in would otherwise start a new row and push the next target language's gloss out of place. No I/O.
/// # Safety
/// `text` must reference `length` readable bytes for this call.
#[no_mangle]
pub unsafe extern "C" fn msime_client_format_translation_gloss(
    text: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if text.is_null() || length > 65536 {
            return Err("invalid translation gloss buffer".into());
        }
        let text = std::str::from_utf8(unsafe { std::slice::from_raw_parts(text, length) })
            .map_err(|_| "invalid translation gloss text")?;
        Ok(
            msime_client_core::translation::format_translation_gloss(text)
                .map(Value::String)
                .unwrap_or(Value::Null),
        )
    })
}

/// 为宿主自己的 HTTP 传输构造自定义翻译请求：DeepL 官方 API 地址用 DeepL 的格式，其他地址用 DeepLX 兼容格式。不做 I/O。
/// # Safety
/// `request` must reference `length` readable bytes for this call.
#[no_mangle]
pub unsafe extern "C" fn msime_client_custom_translation_http_request(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length > 16384 {
            return Err("invalid custom translation request buffer".into());
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            config: msime_input_runtime::TranslationProviderConfig,
            text: String,
            source_language: String,
            target_language: String,
        }
        let Request {
            mut config,
            text,
            source_language,
            target_language,
        }: Request = serde_json::from_slice(unsafe { std::slice::from_raw_parts(request, length) })
            .map_err(|_| "invalid custom translation request")?;
        // Match the Windows source: settings pasted from a password manager
        // are trimmed before endpoint and credential validation.
        config.endpoint = config.endpoint.trim().to_owned();
        config.api_key = config.api_key.trim().to_owned();
        if !config.enabled {
            return Ok(Value::Null);
        }
        let valid_language = |value: &str| {
            !value.is_empty()
                && value.len() <= 16
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphabetic() || byte == b'-')
        };
        if !msime_client_core::translation::is_supported_endpoint(&config.endpoint)
            || !is_bounded_text(&config.api_key, 4096)
            || text.is_empty()
            || !is_bounded_text(&text, 160)
            || !msime_client_core::translation::is_valid_source_text(&text)
            || !valid_language(&source_language)
            || !valid_language(&target_language)
        {
            return Err("invalid custom translation parameters".into());
        }
        // DeepL 官方地址与 DeepLX 兼容服务的请求格式不同，由共享构造函数决定；设置页的连接测试用的也是它。
        let request = msime_client_core::translation::custom_translation_request(
            &config.endpoint,
            &config.api_key,
            &text,
            &source_language,
            &target_language,
        )
        .ok_or("invalid custom translation parameters")?;
        let headers: serde_json::Map<String, Value> = request
            .headers
            .into_iter()
            .map(|(name, value)| (name, Value::String(value)))
            .collect();
        Ok(json!({
            "url": request.url,
            "method": "POST",
            "headers": headers,
            "body": request.body,
            "timeout_ms": 2500,
            "max_response_bytes": 1048576,
        }))
    })
}

/// Parse a bounded provider document; malformed/no-result documents return null.
/// # Safety
/// `body` must reference `length` readable bytes for this call.
#[no_mangle]
pub unsafe extern "C" fn msime_client_parse_custom_translation_response(
    body: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if body.is_null() || length > 1048576 {
            return Err("invalid custom translation response buffer".into());
        }
        let bytes = unsafe { std::slice::from_raw_parts(body, length) };
        let result = std::str::from_utf8(bytes)
            .ok()
            .and_then(msime_client_core::translation::parse_translation_response)
            .and_then(|text| msime_client_core::translation::format_translation_gloss(&text))
            .filter(|text| !text.is_empty() && text.len() <= 4096);
        Ok(result.map(Value::String).unwrap_or(Value::Null))
    })
}

/// Whether a DeepLX-compatible reply reports a failure rather than an answer. A failed reply is asked again; only an answer, empty or not, may be negative-cached. An oversized or null buffer is a failure.
/// # Safety
/// `body` must reference `length` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn msime_client_custom_translation_reply_failed(
    body: *const u8,
    length: usize,
) -> bool {
    if body.is_null() || length > 1048576 {
        return true;
    }
    let bytes = unsafe { std::slice::from_raw_parts(body, length) };
    std::str::from_utf8(bytes)
        .map(msime_client_core::translation::translation_response_failed)
        .unwrap_or(true)
}

/// Whether a NiuTrans reply reports a failure (rate limit, credentials, malformed body) rather than an answer. A failed reply is asked again; only an answer, empty or not, may be negative-cached.
/// # Safety
/// `body` must reference `length` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn msime_client_niutrans_translation_reply_failed(
    body: *const u8,
    length: usize,
) -> bool {
    unsafe { niutrans_reply_failure_from_raw(body, length, niutrans_translation::failed) }
}

/// 在构造借用切片前拒绝空指针和超长回复，避免 FFI 调用方的长度契约变成未定义行为。
unsafe fn niutrans_reply_failure_from_raw(
    body: *const u8,
    length: usize,
    parse: impl FnOnce(&[u8]) -> bool,
) -> bool {
    if body.is_null() || length > 1_048_576 {
        return true;
    }
    parse(unsafe { std::slice::from_raw_parts(body, length) })
}

/// 同代次非空候选释义合并，空数组主动清空；宿主异步无结果时跳过调用。
/// 缓冲区为 `{text, translation}` 数组，本接口不保留它。
///
/// # Safety
/// `translations` must point to a readable UTF-8 buffer of `length` bytes and
/// must not be null. The buffer is not retained after this call.
#[no_mangle]
pub unsafe extern "C" fn msime_client_apply_translations(
    handle: u64,
    generation: u64,
    translations: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if translations.is_null() || length > 1_048_576 {
            return Err("invalid translation buffer".into());
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Translation {
            text: String,
            translation: String,
            /// 来自翻译查询 `gloss_query` 的离线释义，不受「仅当前高亮」限制；缺省为假。
            #[serde(default)]
            gloss: bool,
        }
        let bytes = unsafe { std::slice::from_raw_parts(translations, length) };
        let values: Vec<Translation> =
            serde_json::from_slice(bytes).map_err(|_| "translations must be a UTF-8 JSON array")?;
        if values.len() > 4096
            || values.iter().any(|item| {
                !is_bounded_text(&item.text, 4096) || !is_bounded_text(&item.translation, 4096)
            })
        {
            return Err("translation entries exceed limits".into());
        }
        with_session(handle, |session| {
            let pending = session.lifecycle_begin(lifecycle::Call::SideChannel);
            let applied = session.runtime.apply_translation_answers(
                generation,
                values
                    .into_iter()
                    .map(|item| (item.text, item.translation, item.gloss)),
            );
            let view = session.runtime.view();
            session.lifecycle_view(pending, &view);
            Ok(json!({"applied": applied, "view": view}))
        })
    })
}

/// Save short English-target glosses through Engine into a user-owned overlay.
/// # Safety
/// Both pointers must reference readable buffers of their declared lengths.
#[no_mangle]
pub unsafe extern "C" fn msime_client_translation_gloss_save(
    request: *const u8,
    request_length: usize,
    user_data: *const u8,
    user_data_length: usize,
) -> *mut c_char {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        target_language: String,
        translations: Vec<msime_input_runtime::TranslationResult>,
    }
    response(|| {
        if request.is_null()
            || user_data.is_null()
            || request_length > 131_072
            || user_data_length > 4096
        {
            return Err("invalid translation persistence buffer".into());
        }
        let request: Request =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(request, request_length) })
                .map_err(|_| "invalid translation persistence request")?;
        let user_data =
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(user_data, user_data_length) })
                .map_err(|_| "invalid user data path")?;
        if !std::path::Path::new(user_data).is_absolute()
            || !std::path::Path::new(user_data).is_dir()
        {
            return Err("user data requires an existing absolute directory".into());
        }
        if request.translations.len()
            > usize::from(msime_client_core::preferences::MAX_CANDIDATE_PAGE_SIZE)
            || request
                .translations
                .iter()
                .any(|item| !is_bounded_text(&item.text, 4096) || item.translation.len() > 4096)
        {
            return Err("translation persistence entries exceed limits".into());
        }
        let mut saved = 0;
        if request.target_language == "en" {
            // The overlay lives in the user directory, so it waits behind dictionary maintenance and a data directory move like a session does; a save refused here is only a cache entry lost.
            let _access = DictionaryAccess::try_session(
                std::path::Path::new(user_data),
                std::path::Path::new(user_data),
            )
            .map_err(|_| "dictionary access unavailable")?
            .ok_or("dictionary maintenance busy")?;
            use msime_client_core::translation::{
                format_translation_gloss, is_cloud_translatable_chinese,
                is_cloud_translatable_english, should_persist_translation,
            };
            for item in request.translations {
                let english = is_cloud_translatable_english(&item.text);
                if !msime_client_core::translation::is_valid_source_text(&item.text)
                    || (!english && !is_cloud_translatable_chinese(&item.text))
                {
                    continue;
                }
                let Some(gloss) = format_translation_gloss(&item.translation) else {
                    continue;
                };
                if !should_persist_translation(&item.text, &gloss) {
                    continue;
                }
                let key = if english {
                    item.text.to_ascii_lowercase()
                } else {
                    item.text
                };
                if msime_engine::host::save_candidate_gloss(user_data, !english, &key, &gloss) {
                    saved += 1;
                }
            }
        }
        Ok(json!({"saved":saved}))
    })
}

/// Resolve copied candidates against the packaged offline English dictionary.
/// This owns no session state and is safe to call on a host worker thread. The
/// copied generation is echoed so the host can apply only to the originating view.
///
/// # Safety
/// Both pointers must reference readable buffers for their stated lengths and
/// remain valid for this call. The buffers are not retained.
#[no_mangle]
pub unsafe extern "C" fn msime_client_candidate_gloss_request(
    request: *const u8,
    request_length: usize,
    resources: *const u8,
    resources_length: usize,
) -> *mut c_char {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        generation: u64,
        #[serde(default)]
        user_data: Option<String>,
        #[serde(default)]
        target_language: Option<String>,
        /// The `state_root` of the translation query, when an offline dictionary came from the downloaded pack.
        #[serde(default)]
        state_root: Option<String>,
        /// 当前 Engine 资源目录较旧时可选的随包补充资源目录。
        #[serde(default)]
        supplementary_resources: Option<String>,
        candidates: Vec<GlossCandidate>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct GlossCandidate {
        text: String,
        source: u8,
    }
    response(|| {
        if request.is_null()
            || resources.is_null()
            || request_length > 262_144
            || resources_length > 4096
        {
            return Err("invalid candidate gloss buffer".into());
        }
        let request: Request =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(request, request_length) })
                .map_err(|_| "invalid candidate gloss request")?;
        let Request {
            generation,
            user_data,
            target_language,
            state_root,
            supplementary_resources,
            candidates: raw_candidates,
        } = request;
        if state_root.as_deref().is_some_and(|state_root| {
            state_root.len() > 4096 || !std::path::Path::new(state_root).is_absolute()
        }) {
            return Err("state root must be absolute".into());
        }
        if raw_candidates.len() > 4096
            || raw_candidates.iter().any(|candidate| {
                candidate.text.is_empty() || !is_bounded_text(&candidate.text, 4096)
            })
        {
            return Err("candidate gloss entries exceed limits".into());
        }
        let resources = super::parse_absolute_path(
            unsafe { std::slice::from_raw_parts(resources, resources_length) },
            "resources path is not UTF-8",
            "resources path must be absolute",
        )?;
        let supplementary_resources = supplementary_resources
            .as_deref()
            .map(|path| {
                if path.len() > 4096 {
                    return Err("supplementary resources path is too long".into());
                }
                super::parse_absolute_path(
                    path.as_bytes(),
                    "supplementary resources path is not UTF-8",
                    "supplementary resources path must be absolute",
                )
            })
            .transpose()?;
        let mut candidates = Vec::with_capacity(raw_candidates.len());
        candidates.extend(
            raw_candidates
                .into_iter()
                .map(|candidate| (candidate.text, candidate.source)),
        );
        let user_data = user_data.as_deref().unwrap_or("");
        if !user_data.is_empty()
            && (user_data.len() > 4096 || !std::path::Path::new(user_data).is_absolute())
        {
            return Err("user data path must be absolute".into());
        }
        let lookup = |candidates: &[(String, u8)]| -> Result<Vec<String>, String> {
            Ok(match target_language.as_deref() {
                None | Some("en") => {
                    let mut glosses = msime_engine::host::candidate_glosses_with_user(
                        resources, user_data, candidates,
                    )
                    .map_err(|_| "candidate gloss dictionary unavailable")?;
                    // What the user's own glossary contributed is what differs from the packaged answer; that stays.
                    let learned = if user_data.is_empty() {
                        vec![false; glosses.len()]
                    } else {
                        msime_engine::host::candidate_glosses(resources, candidates)
                            .map(|packaged| {
                                glosses
                                    .iter()
                                    .zip(&packaged)
                                    .map(|(gloss, packaged)| gloss != packaged)
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_else(|_| vec![true; glosses.len()])
                    };
                    crate::supplementary_glosses::prefer(
                        std::path::Path::new(resources),
                        supplementary_resources.map(std::path::Path::new),
                        candidates,
                        &mut glosses,
                        &learned,
                    );
                    glosses
                }
                // Another language reads only its offline dictionary: the learned store and custom_translations.txt hold English. A dictionary that is not installed answers nothing, so the host keeps whatever the online path brings.
                Some(language) if crate::OFFLINE_GLOSS_LANGUAGES.contains(&language) => {
                    let Some(database) = crate::offline_glosses_file(
                        std::path::Path::new(resources),
                        state_root.as_deref().map(std::path::Path::new),
                        language,
                    ) else {
                        return Ok(vec![String::new(); candidates.len()]);
                    };
                    let database = database
                        .to_str()
                        .ok_or("candidate gloss dictionary unavailable")?;
                    msime_engine::host::candidate_target_glosses(database, language, candidates)
                        .map_err(|_| "candidate gloss dictionary unavailable")?
                }
                Some(_) => return Err("invalid candidate gloss request".into()),
            })
        };
        let mut glosses = lookup(&candidates)?;
        // Sources that never receive glosses keep an empty slot so the reply
        // remains position-safe; hosts can still apply ordinary rows with the
        // same text without displaying a gloss on these special rows.
        for (candidate, gloss) in candidates.iter().zip(&mut glosses) {
            if msime_client_core::translation::source_gets_no_gloss(candidate.1) {
                gloss.clear();
            }
        }
        // The gloss tables are keyed by Simplified text, so a Traditional candidate - a Korean Hanja such as 韓, or any candidate under Traditional output - finds nothing under its own spelling. Ask again under the Simplified characters for the ones that missed; the reply still names the candidate as shown.
        if glosses.len() == candidates.len() {
            let (retry, retry_index) = traditional_retry_inputs(&candidates, &glosses);
            if !retry.is_empty() {
                let found = lookup(&retry)?;
                if found.len() == retry.len() {
                    for (index, gloss) in retry_index.into_iter().zip(found) {
                        glosses[index] = gloss;
                    }
                }
            }
        }
        if glosses.len() != candidates.len() {
            return Err("candidate gloss response mismatch".into());
        }
        candidates
            .iter()
            .zip(&glosses)
            .try_fold(0_usize, |total, ((text, _), gloss)| {
                if gloss.len() > 4096 {
                    return None;
                }
                total.checked_add(text.len())?.checked_add(gloss.len())
            })
            .filter(|total| *total <= 900_000)
            .ok_or("candidate gloss response exceeds limits")?;
        let translated_count = glosses.iter().filter(|gloss| !gloss.is_empty()).count();
        let mut translations = Vec::with_capacity(translated_count);
        translations.extend(candidates.into_iter().zip(glosses).filter_map(
            |((text, _), translation)| {
                (!translation.is_empty()).then(|| {
                    json!({
                        "text": text,
                        "translation": translation,
                    })
                })
            },
        ));
        Ok(json!({
            "generation": generation,
            "translations": translations,
        }))
    })
}

/// 从离线表查询英文候选、英文释义或法日西俄德韩释义行的读音，活动资源旁缺表时可回退到显式补充目录。
/// 不持有会话状态，可在宿主工作线程调用；返回代次供宿主匹配当前视图。
///
/// # Safety
/// 两个指针必须在调用期间指向各自长度的可读缓冲区；本函数不保留缓冲区。
#[no_mangle]
pub unsafe extern "C" fn msime_client_pronunciation_request(
    request: *const u8,
    request_length: usize,
    resources: *const u8,
    resources_length: usize,
) -> *mut c_char {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        generation: u64,
        items: Vec<Item>,
        #[serde(default)]
        supplementary_resources: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Item {
        text: String,
        language: String,
    }
    response(|| {
        if request.is_null()
            || resources.is_null()
            || request_length > 262_144
            || resources_length > 4096
        {
            return Err("invalid pronunciation buffer".into());
        }
        let request: Request =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(request, request_length) })
                .map_err(|_| "invalid pronunciation request")?;
        if request.items.len() > 4096
            || request.items.iter().any(|item| {
                item.text.is_empty()
                    || item.text.len() > 4096
                    || item.text.chars().any(char::is_control)
            })
        {
            return Err("pronunciation entries exceed limits".into());
        }
        if request
            .items
            .iter()
            .any(|item| !msime_engine::host::PHONETIC_LANGUAGES.contains(&item.language.as_str()))
        {
            return Err("invalid pronunciation request".into());
        }
        let resources =
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(resources, resources_length) })
                .map_err(|_| "resources path is not UTF-8")?;
        if !std::path::Path::new(resources).is_absolute() {
            return Err("resources path must be absolute".into());
        }
        let supplementary_resources = request
            .supplementary_resources
            .as_deref()
            .map(|path| {
                if path.len() > 4096 {
                    return Err("supplementary resources path is too long".into());
                }
                super::parse_absolute_path(
                    path.as_bytes(),
                    "supplementary resources path is not UTF-8",
                    "supplementary resources path must be absolute",
                )
            })
            .transpose()?;
        let resources = std::path::Path::new(resources);
        let supplementary_resources = supplementary_resources.map(std::path::Path::new);
        // 每种语言各查一次自己的表，回答按请求里的顺序排；某种语言没装表时它的项没有读音，宿主继续显示原有释义。
        let mut readings = vec![String::new(); request.items.len()];
        for language in msime_engine::host::PHONETIC_LANGUAGES {
            let positions = request
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.language == language)
                .map(|(position, _)| position)
                .collect::<Vec<_>>();
            if positions.is_empty() {
                continue;
            }
            let database = if language == "en" {
                crate::pronunciation::english_database_beside(resources, supplementary_resources)
            } else {
                crate::pronunciation::target_database_beside(
                    resources,
                    supplementary_resources,
                    language,
                )
            };
            let Some(database) = database else {
                continue;
            };
            let database = database
                .to_str()
                .ok_or("pronunciation dictionary unavailable")?;
            let texts = positions
                .iter()
                .map(|&position| request.items[position].text.clone())
                .collect::<Vec<_>>();
            let answers = if language == "en" {
                crate::pronunciation::english_pronunciations(database, &texts)?
            } else {
                crate::pronunciation::target_pronunciations(database, language, &texts)?
            };
            for (position, answer) in positions.into_iter().zip(answers) {
                readings[position] = answer;
            }
        }
        let pronunciations = request
            .items
            .into_iter()
            .zip(readings)
            .filter(|(_, pronunciation)| !pronunciation.is_empty())
            .map(|(item, pronunciation)| {
                json!({"text": item.text, "language": item.language, "pronunciation": pronunciation})
            })
            .collect::<Vec<_>>();
        Ok(json!({"generation": request.generation, "pronunciations": pronunciations}))
    })
}

/// Break copied Chinese candidates that no dictionary has as a whole into words with their English, from the local
/// character and word tables. Owns no session state and runs on a host worker thread; the generation is echoed.
///
/// # Safety
/// Both pointers must reference readable buffers for their stated lengths and
/// remain valid for this call. The buffers are not retained.
#[no_mangle]
pub unsafe extern "C" fn msime_client_gloss_breakdown_request(
    request: *const u8,
    request_length: usize,
    resources: *const u8,
    resources_length: usize,
) -> *mut c_char {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        generation: u64,
        texts: Vec<String>,
    }
    response(|| {
        if request.is_null()
            || resources.is_null()
            || request_length > 65_536
            || resources_length > 4096
        {
            return Err("invalid breakdown buffer".into());
        }
        let request: Request =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(request, request_length) })
                .map_err(|_| "invalid breakdown request")?;
        if request.texts.len() > 64
            || request.texts.iter().any(|text| {
                text.is_empty() || text.len() > 4096 || text.chars().any(char::is_control)
            })
        {
            return Err("breakdown entries exceed limits".into());
        }
        let resources =
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(resources, resources_length) })
                .map_err(|_| "resources path is not UTF-8")?;
        if !std::path::Path::new(resources).is_absolute() {
            return Err("resources path must be absolute".into());
        }
        let texts = request
            .texts
            .into_iter()
            .filter(|text| crate::word_breakdown::eligible(text))
            .collect::<Vec<_>>();
        let breakdowns =
            crate::word_breakdown::known_pieces(std::path::Path::new(resources), &texts)
                .map(|known| {
                    texts
                        .iter()
                        .filter_map(|text| {
                            let pieces = crate::word_breakdown::segment(text, &known);
                            crate::word_breakdown::render(&pieces)
                                .map(|breakdown| json!({"text": text, "breakdown": breakdown}))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
        Ok(json!({"generation": request.generation, "breakdowns": breakdowns}))
    })
}

/// 查询固定英文词库；`correction: true` 时返回原文与唯一纠错。此接口不创建或改变输入会话，可在宿主工作线程调用。
///
/// `completions: true` 只能与 `correction: true` 同用，返回手机候选栏的合并展示：有纠错时为原文与纠错；原文已是完整的词（词库精确命中、固定词形或受保护的网络缩写）时只有原文；否则原文后接至多 `limit - 1` 个前缀补全，原文首字母大写时补全的首字母随之大写。这种请求的响应多一个 `corrected`，当且仅当第二行是纠错时为真；不带 `completions` 的两种请求响应形状不变。
///
/// # Safety
/// 请求与资源指针须在所给长度内可读；函数返回后不保留缓冲区。
#[no_mangle]
pub unsafe extern "C" fn msime_client_english_completions_request(
    request: *const u8,
    request_length: usize,
    resources: *const u8,
    resources_length: usize,
) -> *mut c_char {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        prefix: String,
        limit: u8,
        #[serde(default)]
        correction: bool,
        #[serde(default)]
        completions: bool,
    }
    response(|| {
        if request.is_null()
            || resources.is_null()
            || request_length > 16_384
            || resources_length > 4_096
        {
            return Err("invalid English completion buffer".into());
        }
        let request: Request =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(request, request_length) })
                .map_err(|_| "invalid English completion request")?;
        if request.completions && !request.correction {
            return Err("invalid English completion request".into());
        }
        if !(1..=32).contains(&request.limit)
            || request.prefix.is_empty()
            || request.prefix.len() > 128
            || !request
                .prefix
                .bytes()
                .all(|byte| byte.is_ascii_alphabetic() || (request.correction && byte == b'\''))
        {
            return Err("invalid English completion prefix".into());
        }
        let resources =
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(resources, resources_length) })
                .map_err(|_| "resources path is not UTF-8")?;
        if !Path::new(resources).is_absolute() {
            return Err("resources path must be absolute".into());
        }
        if request.completions {
            let (items, corrected) = msime_engine::host::english_correction_display(
                resources,
                &request.prefix,
                usize::from(request.limit),
            )
            .map_err(|_| "English completion dictionary unavailable")?;
            return Ok(json!({"prefix": request.prefix, "items": items, "corrected": corrected}));
        }
        let items = if request.correction {
            msime_engine::host::english_corrections(resources, &request.prefix)
                .map(|items| items.into_iter().take(usize::from(request.limit)).collect())
        } else {
            msime_engine::host::english_completions(
                resources,
                &request.prefix,
                usize::from(request.limit),
            )
        }
        .map_err(|_| "English completion dictionary unavailable")?;
        Ok(json!({"prefix": request.prefix, "items": items}))
    })
}

#[cfg(test)]
mod gloss_limit_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traditional_retry_inputs_reserve_candidate_capacity() {
        let candidates = vec![("學".to_owned(), 1), ("你好".to_owned(), 2)];
        let glosses = vec![String::new(), String::new()];
        let (retry, indexes) = traditional_retry_inputs(&candidates, &glosses);
        assert_eq!(retry.len(), 1);
        assert_eq!(indexes, [0]);
        assert!(retry.capacity() >= candidates.len());
        assert!(indexes.capacity() >= candidates.len());
    }

    #[test]
    fn niutrans_reply_failure_checks_length_before_borrowing() {
        let pointer = std::ptr::NonNull::<u8>::dangling().as_ptr();
        assert!(unsafe {
            niutrans_reply_failure_from_raw(pointer, 1_048_577, |_| {
                panic!("oversized reply must not be borrowed")
            })
        });
    }
}

/// `msime_client_account_gloss` 的请求：组请求体，或解读宿主收到的响应。
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum AccountGlossRequest {
    Request {
        texts: Vec<String>,
        source_lang: String,
        target_lang: String,
    },
    Parse {
        texts: Vec<String>,
        status: u16,
        body: String,
        #[serde(default)]
        retry_after: Option<String>,
    },
}

/// 账号释义的 sans-IO 一半，给在按键路径上自己发 HTTP 的宿主（Windows 的 curl、Linux）：`request` 校验后给出 `POST /v1/translate` 的请求体，`parse` 解读收到的响应。规则与 `msime_client_account` 的 `translate` 相同。请求、返回值和错误码见头文件。
/// # Safety
/// `request` 指向 `length` 个可读的 UTF-8 JSON 字节，NULL 被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_account_gloss(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    use msime_client_services::account::{
        translate_request_body, translate_response, AccountError, AccountTranslateRequest,
        TRANSLATE_PATH,
    };
    super::account::account_failure_response(|| {
        // SAFETY: 由调用方契约保证；长度先经过上限检查。
        let request: AccountGlossRequest = unsafe {
            super::with_bounded_bytes(
                request,
                length,
                4 * 1024 * 1024,
                "account_invalid",
                |bytes| serde_json::from_slice(bytes).map_err(|_| "account_invalid".to_owned()),
            )
        }?;
        match request {
            AccountGlossRequest::Request {
                texts,
                source_lang,
                target_lang,
            } => {
                let body = translate_request_body(&AccountTranslateRequest {
                    texts,
                    source_lang,
                    target_lang,
                })?;
                let body =
                    String::from_utf8(body).map_err(|_| AccountError::Invalid.code().to_owned())?;
                Ok(json!({ "method": "POST", "path": TRANSLATE_PATH, "body": body }))
            }
            AccountGlossRequest::Parse {
                texts,
                status,
                body,
                retry_after,
            } => {
                let translations =
                    translate_response(&texts, status, body.as_bytes(), retry_after.as_deref())?;
                serde_json::to_value(translations)
                    .map_err(|_| AccountError::Unavailable.code().to_owned().into())
            }
        }
    })
}
