//! Requests the host performs on the library's behalf: cloud, online, handwriting and emoji.
//!
//! Part of the C ABI; see the parent module for what these shims guarantee.

use crate::*;
use msime_client_core::is_bounded_text;

#[cfg(any(unix, test))]
fn online_candidate_response(candidates: Vec<(String, u8)>) -> Value {
    let mut rows = Vec::with_capacity(candidates.len());
    for (text, source) in candidates {
        rows.push(json!({"text": text, "source": source}));
    }
    json!({"candidates": rows})
}

#[no_mangle]
pub extern "C" fn msime_client_view(handle: u64) -> *mut c_char {
    response(|| with_session(handle, |session| serialized_runtime_view(session)))
}

#[no_mangle]
pub extern "C" fn msime_client_online_query(handle: u64) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            let Some(query) = session.runtime.online_query().map_err(|e| e.to_string())? else {
                return Ok(Value::Null);
            };
            let mut value = serde_json::to_value(query).map_err(|e| e.to_string())?;
            value["cloud_candidates"] = Value::Bool(session.cloud_candidates_enabled());
            value["ai_assistant"] =
                serde_json::to_value(session.ai_provider_config()).map_err(|e| e.to_string())?;
            Ok(value)
        })
    })
}

/// Build a validated AI HTTP descriptor for a copied OnlineQuery.
/// Credentials stay inside the host session; only the returned descriptor is
/// consumed by the platform transport worker and the query must still match
/// the current AI preferences.
/// # Safety
/// `query` references `query_length` readable UTF-8 JSON bytes. No buffers are retained.
#[no_mangle]
pub unsafe extern "C" fn msime_client_ai_request_for_query(
    handle: u64,
    query: *const u8,
    query_length: usize,
) -> *mut c_char {
    response(|| {
        if query.is_null() || query_length > 16384 {
            return Err("invalid AI query buffer".into());
        }
        let query = serde_json::from_slice::<OnlineQuery>(unsafe {
            std::slice::from_raw_parts(query, query_length)
        })
        .map_err(|_| "invalid online query document")?;
        with_session(handle, |session| {
            if !query.ai_eligible || !session.ai_query_is_current(&query) {
                return Ok(Value::Null);
            }
            let preferences = session
                .requested
                .as_ref()
                .map(|snapshot| &snapshot.preferences)
                .unwrap_or(&session.applied);
            // The limit has to come from the same config the descriptor is
            // built from: chat_completion_http_request rejects a request whose
            // limit disagrees with its config, and the query document's copy
            // can lag the pending preferences this call is meant to follow.
            let mut config = preferences.ai_assistant.clone();
            if let Some(credential) = &session.ai_credential {
                if credential.endpoint == config.endpoint {
                    if let Some(origin) =
                        msime_client_core::ai::endpoint::credential_origin(&config.endpoint)
                    {
                        config.tokens.insert(origin, credential.token.clone());
                    }
                }
            }
            let request = AiSuggestionRequest {
                segmented_pinyin: query.pinyin_segments,
                context: query.ai_context,
                candidate_limit: config.candidate_limit,
            };
            msime_client_core::ai::chat_completion_http_request(&config, &request)
                .map(|value| value.unwrap_or(Value::Null))
                .map_err(|error| error.to_string())
        })
    })
}

/// Hand the session an AI provider credential kept outside the preferences. It overrides tokens only for the current endpoint, lives only as long as the session, and is never persisted or reported back. An empty token clears it.
/// # Safety
/// `token` references `token_length` readable UTF-8 bytes; null is accepted only with a zero length. No buffers are retained.
#[no_mangle]
pub unsafe extern "C" fn msime_client_set_ai_credential(
    handle: u64,
    token: *const u8,
    token_length: usize,
) -> *mut c_char {
    response(|| {
        if (token.is_null() && token_length != 0) || token_length > 4096 {
            return Err("invalid AI credential buffer".into());
        }
        let token = if token_length == 0 {
            None
        } else {
            let text =
                std::str::from_utf8(unsafe { std::slice::from_raw_parts(token, token_length) })
                    .map_err(|_| "invalid AI credential")?;
            if !is_bounded_text(text, 4096) {
                return Err("invalid AI credential".into());
            }
            Some(text.to_owned())
        };
        with_session(handle, |session| {
            session.ai_credential = token.map(|token| {
                let preferences = session
                    .requested
                    .as_ref()
                    .map(|snapshot| &snapshot.preferences)
                    .unwrap_or(&session.applied);
                SessionAiCredential {
                    endpoint: preferences.ai_assistant.endpoint.clone(),
                    token,
                }
            });
            Ok(Value::Bool(true))
        })
    })
}

/// The online translation service a query names, with the configuration of the services it may reach. `account_allowed` says whether the hosted account may be chosen at all: candidate glosses reach it only with candidate translation on, and the `/fy` request, which the account cannot answer, asks with it allowed so an explicit account choice is still recognised and never falls back to another service.
struct TranslationServices {
    provider: TranslationService,
    translation_account: bool,
    custom_translation: Option<Value>,
    tencent_tmt: Option<Value>,
    niutrans: Option<Value>,
}

fn selected_translation_services(
    preferences: &msime_client_core::preferences::Preferences,
    account_allowed: bool,
) -> Result<TranslationServices, &'static str> {
    let custom_translation = &preferences.custom_translation;
    let tencent = &preferences.tencent_tmt;
    // The MSIME account gloss endpoint (api.msime.app) is used only when the user explicitly chose it and no service of their own takes precedence. Tencent counts only with usable secrets, because its default `enabled: true` is not a user choice.
    let translation_account = account_allowed
        && preferences.translation_account
        && !preferences.niutrans.enabled
        && !custom_translation.enabled
        && !(tencent.enabled
            && msime_client_core::translation::usable_credential(&tencent.secret_id)
            && msime_client_core::translation::usable_credential(&tencent.secret_key));
    // The selected service, derived from the enable flags alone so an incomplete NiuTrans or custom configuration stays selected instead of reading as Tencent. A host whose Tencent secret lives outside preferences (Linux keeps it in the provider's own file) relies on this to honour 关闭.
    let provider = if translation_account {
        TranslationService::Account
    } else if preferences.niutrans.enabled {
        TranslationService::NiuTrans
    } else if custom_translation.enabled {
        TranslationService::Custom
    } else if tencent.enabled {
        TranslationService::Tencent
    } else {
        TranslationService::Off
    };
    // Selecting custom translation must never silently fall back to TMT.
    let tencent_tmt = (!custom_translation.enabled
        && !preferences.niutrans.enabled
        && tencent.enabled
        && msime_client_core::translation::usable_credential(&tencent.secret_id)
        && msime_client_core::translation::usable_credential(&tencent.secret_key))
    .then(|| serde_json::to_value(tencent))
    .transpose()
    .map_err(|_| "invalid Tencent translation configuration")?;
    let custom_translation = (custom_translation.enabled
        && !preferences.niutrans.enabled
        && !custom_translation.endpoint.is_empty())
    .then(|| {
        json!({
            "enabled": true,
            "endpoint": &custom_translation.endpoint,
            "api_key": &custom_translation.api_key,
        })
    });
    let niutrans = (preferences.niutrans.enabled
        && msime_client_core::translation::usable_credential(&preferences.niutrans.app_id)
        && msime_client_core::translation::usable_credential(&preferences.niutrans.apikey))
    .then(|| serde_json::to_value(&preferences.niutrans))
    .transpose()
    .map_err(|_| "invalid NiuTrans translation configuration")?;
    Ok(TranslationServices {
        provider,
        translation_account,
        custom_translation,
        tencent_tmt,
        niutrans,
    })
}

/// Return the visible candidate texts that may receive asynchronous translations.
/// The generation must be echoed to `msime_client_apply_translations`.
#[no_mangle]
pub extern "C" fn msime_client_translation_query(handle: u64) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            // Provider settings can change while Engine preferences wait for
            // composition to finish. Query with the newest validated settings.
            let preferences = session
                .requested
                .as_ref()
                .map(|snapshot| &snapshot.preferences)
                .unwrap_or(&session.applied);
            // The offline English gloss rides this same query, so it has to be
            // reachable with online translation off: it is a packaged
            // dictionary lookup and never leaves the machine.
            let mut target_languages = vec![preferences.translation_target_language];
            if let Some(secondary) = preferences.translation_secondary_language {
                if !target_languages.contains(&secondary) {
                    target_languages.push(secondary);
                }
            }
            let english_gloss = preferences.candidate_english_gloss
                && target_languages.iter().any(|language| {
                    matches!(
                        language,
                        msime_client_core::preferences::TranslationTargetLanguage::En
                    )
                });
            let persist_english_translation = preferences.candidate_translations
                && matches!(
                    preferences.translation_target_language,
                    msime_client_core::preferences::TranslationTargetLanguage::En
                );
            // Non-English targets with an offline dictionary installed beside the resources or in the downloaded offline-glosses pack, in preference order. The same switches as macOS's English fallback reach them: the offline gloss switch, or candidate translation, whose online answer replaces the offline one when it arrives. Never read from the user directory, so no user path is needed for them.
            let resources = std::path::Path::new(&session.options.resources);
            let mut from_pack = false;
            let offline_gloss_languages = if preferences.candidate_translations
                || preferences.candidate_english_gloss
            {
                let mut languages = Vec::with_capacity(target_languages.len());
                languages.extend(target_languages.iter().filter_map(|language| {
                    let language = serde_json::to_value(language).ok()?;
                    let code = language.as_str()?;
                    crate::offline_glosses_file(resources, session.state_root.as_deref(), code)?;
                    from_pack |= crate::offline_glosses_beside(resources, code).is_none();
                    Some(code.to_owned())
                }));
                languages
            } else {
                Vec::new()
            };
            // `/fy` asks the selected service whatever the gloss switches say: it is the user's explicit request, a single English text translated into Chinese that comes back as a row which commits it. Nothing else rides it, so no offline dictionary is consulted and nothing is persisted.
            if let Some(command) = session.runtime.command_translation() {
                // The hosted account only glosses Chinese candidates, so `/fy` needs a service of the user's own.
                let services = selected_translation_services(preferences, true)?;
                if matches!(
                    services.provider,
                    TranslationService::Off | TranslationService::Account
                ) {
                    return Ok(Value::Null);
                }
                return Ok(json!({
                    "generation": command.generation,
                    "sentence": true,
                    "target_language": "zh",
                    "target_languages": ["zh"],
                    "candidates": [{"text": command.text, "online_gloss": false}],
                    "provider": services.provider,
                    "translation_account": false,
                    "custom_translation": services.custom_translation,
                    "tencent_tmt": services.tencent_tmt,
                    "niutrans": services.niutrans,
                    "english_gloss": false,
                    "resources": Value::Null,
                    "user_data": Value::Null,
                }));
            }
            if !preferences.candidate_translations
                && !english_gloss
                && offline_gloss_languages.is_empty()
            {
                return Ok(Value::Null);
            }
            let Some(candidates_view) = session.runtime.translation_candidates() else {
                return Ok(Value::Null);
            };
            // 只有显示释义的方案才请求释义：Windows 不为日文候选请求释义，而韩文的汉字候选和中文一样带释义：훈음 由宿主自己画出，与这里的回答无关，翻译或释义画在它下面一行。按引擎当前的本地模式判断，临时日文组字同样不请求。网址模式也不请求：网址可能带着私密路径和参数，不能发给翻译服务。
            if !SchemeType::from_u8(candidates_view.scheme).is_some_and(SchemeType::shows_glosses)
                || matches!(
                    candidates_view.local_mode.as_str(),
                    "temporary_japanese" | "url"
                )
            {
                return Ok(Value::Null);
            }
            // Whether the online gloss endpoint may be asked about each candidate,
            // answered once here so every host applies the same rule. Only Chinese
            // candidates qualify: a gloss model has nothing to say about "cun",
            // "123", "OpenAI", a punctuation candidate or an emoji, and asking
            // spends the account's bounded quota to put noise under candidates
            // that should carry no gloss - a pinyin buffer candidate also hands
            // the user's raw keystrokes to a remote service.
            //
            // The gloss path only. A user's own translator detects the direction
            // per candidate, so English candidates still reach it, and the offline
            // dictionary answers for every candidate because it never leaves the
            // machine.
            //
            // Emoji and kaomoji sources never qualify, whatever their text: many kaomoji carry Han characters ("(*Φ皿Φ*)") and would otherwise queue behind real words on the serial on-device model and spend account quota, as Windows' BuildTranslationQuery already refuses.
            let mut candidates = Vec::with_capacity(candidates_view.candidates.len());
            candidates.extend(candidates_view.candidates.iter().map(|candidate| {
                json!({
                    "text": candidate.text,
                    "online_gloss":
                        !msime_client_core::translation::is_emoji_or_kaomoji_source(
                            candidate.source,
                        ) && msime_client_core::translation::is_cloud_translatable_chinese(
                            &candidate.text,
                        ),
                })
            }));
            let services =
                selected_translation_services(preferences, preferences.candidate_translations)?;
            let mut query = json!({
                "generation": candidates_view.generation,
                "target_language": serde_json::to_value(preferences.translation_target_language)
                    .map_err(|e| e.to_string())?,
                "target_languages": target_languages
                    .iter()
                    .map(|language| serde_json::to_value(language).map_err(|e| e.to_string()))
                    .collect::<Result<Vec<_>, _>>()?,
                "candidates": candidates,
                "provider": services.provider,
                "translation_account": services.translation_account,
                "custom_translation": services.custom_translation,
                "tencent_tmt": services.tencent_tmt,
                "niutrans": services.niutrans,
                "english_gloss": english_gloss,
                // The packaged resource path is only needed for offline
                // lookup. The user path is also needed by a background host
                // worker to persist successful English-target translations.
                "resources": (english_gloss || !offline_gloss_languages.is_empty())
                    .then(|| session.options.resources.clone()),
                "user_data": (english_gloss || persist_english_translation)
                    .then(|| session.options.user_data.clone()),
            });
            // Omitted rather than empty, so a host with no offline dictionary installed sees the query it always did.
            if !offline_gloss_languages.is_empty() {
                query["offline_gloss_languages"] = json!(offline_gloss_languages);
            }
            // Likewise present only when on: the host then pronounces the gloss lines it draws, English through msime_client_pronunciation_request.
            if preferences.candidate_pronunciation {
                query["candidate_pronunciation"] = json!(true);
            }
            // Only when a dictionary comes from the downloaded pack: the host passes it back to msime_client_candidate_gloss_request, which looks there after the resources.
            if from_pack {
                if let Some(state_root) =
                    session.state_root.as_deref().and_then(|path| path.to_str())
                {
                    query["state_root"] = json!(state_root);
                }
            }
            Ok(query)
        })
    })
}

/// Build the default HTTPS cloud URL for a copied eligible query. The host
/// performs the request and later calls `msime_client_apply_online_candidate`.
///
/// # Safety
/// `query` must point to a readable UTF-8 JSON buffer of `query_length` bytes,
/// or be null only when `query_length` is zero. The buffer is not retained.
#[no_mangle]
pub unsafe extern "C" fn msime_client_cloud_request_url(
    query: *const u8,
    query_length: usize,
) -> *mut c_char {
    response(|| {
        if query.is_null() || query_length > 16384 {
            return Err("invalid cloud query buffer".into());
        }
        let query = serde_json::from_slice::<OnlineQuery>(unsafe {
            std::slice::from_raw_parts(query, query_length)
        })
        .map_err(|_| "invalid online query document")?;
        let url = msime_input_runtime::cloud_request_url(&query)
            .ok_or_else(|| "cloud query is not eligible".to_owned())?;
        Ok(json!(url))
    })
}

/// Query a user-owned Unix-socket provider off the session thread.
/// Returns null when the provider has no candidate or is unavailable.
///
/// # Safety
/// The caller must provide readable buffers of the stated lengths, or null pointers only with
/// zero lengths; buffers are read for the duration of this call and never retained.
#[cfg(unix)]
#[no_mangle]
pub unsafe extern "C" fn msime_client_online_provider_request(
    query: *const u8,
    query_length: usize,
    socket_path: *const u8,
    socket_length: usize,
) -> *mut c_char {
    response(|| {
        if query.is_null() || socket_path.is_null() || query_length > 16384 || socket_length > 4096
        {
            return Err("invalid online provider buffer".into());
        }
        let query = serde_json::from_slice::<OnlineQuery>(unsafe {
            std::slice::from_raw_parts(query, query_length)
        })
        .map_err(|_| "invalid online query document")?;
        let path = super::parse_absolute_socket_path(unsafe {
            std::slice::from_raw_parts(socket_path, socket_length)
        })?;
        Ok(UnixSocketProvider::new(path)
            .query_candidates(query)
            .map(online_candidate_response)
            .unwrap_or(Value::Null))
    })
}

/// Forward one account-backed dictionary operation to a user-owned Linux
/// provider. The request is validated before it crosses the Unix socket.
///
/// # Safety
/// The caller must provide non-null readable buffers of the stated lengths. The buffers are read
/// only for the duration of this call and are never retained.
#[cfg(unix)]
#[no_mangle]
pub unsafe extern "C" fn msime_client_cloud_dictionary_provider_request(
    request: *const u8,
    request_length: usize,
    socket_path: *const u8,
    socket_length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null()
            || socket_path.is_null()
            || request_length > 65_536
            || socket_length > 4096
        {
            return Err("invalid cloud dictionary provider buffer".into());
        }
        let request_bytes = unsafe { std::slice::from_raw_parts(request, request_length) };
        let parsed =
            serde_json::from_slice::<cloud_dictionary::CloudDictionaryRequest>(request_bytes)
                .map_err(|_| "invalid cloud dictionary request")?;
        cloud_dictionary::validate_cloud_request(&parsed).map_err(|error| error.to_owned())?;
        let path = super::parse_absolute_socket_path(unsafe {
            std::slice::from_raw_parts(socket_path, socket_length)
        })?;
        let request = serde_json::from_slice::<serde_json::Value>(request_bytes)
            .map_err(|_| "invalid cloud dictionary request")?;
        msime_input_runtime::UnixSocketProvider::new(path)
            .cloud_dictionary(request)
            .ok_or_else(|| "cloud dictionary provider unavailable".to_owned())
    })
}

/// Forward one validated account-backed cloud clipboard operation to a
/// user-owned Linux provider.
///
/// # Safety
/// The caller must provide non-null readable buffers of the stated lengths. The buffers are read
/// only for the duration of this call and are never retained.
#[cfg(unix)]
#[no_mangle]
pub unsafe extern "C" fn msime_client_cloud_clipboard_provider_request(
    request: *const u8,
    request_length: usize,
    socket_path: *const u8,
    socket_length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null()
            || socket_path.is_null()
            || request_length > 65_536
            || socket_length > 4096
        {
            return Err("invalid cloud clipboard provider buffer".into());
        }
        let request_bytes = unsafe { std::slice::from_raw_parts(request, request_length) };
        let request = serde_json::from_slice::<serde_json::Value>(request_bytes)
            .map_err(|_| "invalid cloud clipboard request")?;
        cloud_clipboard::validate_request(&request).map_err(|error| error.to_owned())?;
        let path = super::parse_absolute_socket_path(unsafe {
            std::slice::from_raw_parts(socket_path, socket_length)
        })?;
        msime_input_runtime::UnixSocketProvider::new(path)
            .cloud_clipboard(request)
            .ok_or_else(|| "cloud clipboard provider unavailable".to_owned())
    })
}

/// Query a user-owned Unix-socket translation provider.
///
/// # Safety
/// Both buffers must be non-null readable UTF-8 buffers for the stated lengths;
/// they are copied for the duration of this call and never retained.
#[cfg(unix)]
#[no_mangle]
pub unsafe extern "C" fn msime_client_translation_provider_request(
    query: *const u8,
    query_length: usize,
    socket_path: *const u8,
    socket_length: usize,
) -> *mut c_char {
    response(|| {
        if query.is_null() || socket_path.is_null() || query_length > 16384 || socket_length > 4096
        {
            return Err("invalid translation provider buffer".into());
        }
        let query = serde_json::from_slice::<TranslationQuery>(unsafe {
            std::slice::from_raw_parts(query, query_length)
        })
        .map_err(|_| "invalid translation query document")?;
        let path = super::parse_absolute_socket_path(unsafe {
            std::slice::from_raw_parts(socket_path, socket_length)
        })?;
        Ok(UnixSocketProvider::new(path)
            .translate(query)
            .map(|items| json!({"translations": items}))
            .unwrap_or(Value::Null))
    })
}

/// Query a user-owned Linux handwriting recognizer over a Unix socket.
/// The request is a bounded JSON HandwritingQuery; the response is
/// `{candidates:[...]}` or null when the recognizer is unavailable.
///
/// # Safety
/// All pointers must reference readable buffers of the stated lengths for
/// the duration of this call; the buffers are not retained.
#[cfg(unix)]
#[no_mangle]
pub unsafe extern "C" fn msime_client_handwriting_provider_request(
    query: *const u8,
    query_length: usize,
    socket_path: *const u8,
    socket_length: usize,
) -> *mut c_char {
    response(|| {
        if query.is_null()
            || socket_path.is_null()
            || query_length > 262_144
            || socket_length > 4096
        {
            return Err("invalid handwriting provider buffer".into());
        }
        let query = serde_json::from_slice::<HandwritingQuery>(unsafe {
            std::slice::from_raw_parts(query, query_length)
        })
        .map_err(|_| "invalid handwriting query document")?;
        let path = super::parse_absolute_socket_path(unsafe {
            std::slice::from_raw_parts(socket_path, socket_length)
        })?;
        Ok(UnixSocketProvider::new(path)
            .handwriting(query)
            .map(|candidates| json!({"candidates": candidates}))
            .unwrap_or(Value::Null))
    })
}

/// Run the Engine's optional offline handwriting recognizer against a trusted
/// packaged model. The model path is supplied by the native host, never by a
/// webview or remote provider.
///
/// # Safety
/// The caller must provide non-null readable buffers of the stated lengths. The buffers are read
/// only for the duration of this call and are never retained.
#[cfg(unix)]
#[no_mangle]
pub unsafe extern "C" fn msime_client_handwriting_local_request(
    query: *const u8,
    query_length: usize,
    model_path: *const u8,
    model_length: usize,
) -> *mut c_char {
    response(|| {
        if query.is_null() || model_path.is_null() || query_length > 262_144 || model_length > 4096
        {
            return Err("invalid local handwriting buffer".into());
        }
        let query = serde_json::from_slice::<HandwritingQuery>(unsafe {
            std::slice::from_raw_parts(query, query_length)
        })
        .map_err(|_| "invalid handwriting query document")?;
        let model =
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(model_path, model_length) })
                .map_err(|_| "model path is not UTF-8")?;
        if !std::path::Path::new(model).is_absolute() {
            return Err("model path must be absolute".into());
        }
        let candidates = engine_handwriting_candidates(model, &query, 1.0, 1.0)?;
        Ok(json!({"candidates": candidates}))
    })
}

/// Query a user-owned Linux emoji catalog over a Unix socket.
/// The response is `{items:[{text,annotation}]}` or null when unavailable.
///
/// # Safety
/// All pointers must reference readable buffers of the stated lengths for
/// the duration of this call; the buffers are not retained.
#[cfg(unix)]
#[no_mangle]
pub unsafe extern "C" fn msime_client_emoji_provider_request(
    query: *const u8,
    query_length: usize,
    socket_path: *const u8,
    socket_length: usize,
) -> *mut c_char {
    response(|| {
        if query.is_null() || socket_path.is_null() || query_length > 16_384 || socket_length > 4096
        {
            return Err("invalid emoji provider buffer".into());
        }
        let query = serde_json::from_slice::<EmojiPanelQuery>(unsafe {
            std::slice::from_raw_parts(query, query_length)
        })
        .map_err(|_| "invalid emoji query document")?;
        let path = super::parse_absolute_socket_path(unsafe {
            std::slice::from_raw_parts(socket_path, socket_length)
        })?;
        Ok(UnixSocketProvider::new(path)
            .emoji(query)
            .map(|items| json!({"items": items}))
            .unwrap_or(Value::Null))
    })
}

#[cfg(unix)]
#[derive(Deserialize)]
pub(crate) struct EmojiCatalogQuery {
    #[serde(flatten)]
    pub(crate) panel: EmojiPanelQuery,
    #[serde(default)]
    pub(crate) offset: usize,
    #[serde(default)]
    pub(crate) group: String,
    #[serde(default)]
    pub(crate) list_groups: bool,
    #[serde(default)]
    pub(crate) list_symbol_groups: bool,
    #[serde(default)]
    pub(crate) parent: String,
    #[serde(default)]
    pub(crate) cursor: bool,
    /// 插件目录的绝对路径，`list_plugin_symbol_groups` 从这里读符号集。
    #[serde(default)]
    pub(crate) plugins: Option<String>,
    #[serde(default)]
    pub(crate) list_plugin_symbol_groups: bool,
}

/// Query the local verified `msime-others.db` Emoji catalog without a provider socket.
/// Success contains `{items:[{text,annotation,group}]}` in the response envelope.
/// With `cursor:true`, also returns `next_offset` and `complete`, preserves
/// duplicate entries, and advances past invalid rows without treating them as EOF.
/// Unavailable or unreadable catalogs return an error, not an empty item list.
///
/// # Safety
/// All pointers must reference readable buffers of the stated lengths for
/// the duration of this call; the buffers are not retained.
#[cfg(unix)]
#[no_mangle]
pub unsafe extern "C" fn msime_client_emoji_catalog_request(
    query: *const u8,
    query_length: usize,
    resources: *const u8,
    resources_length: usize,
) -> *mut c_char {
    response(|| {
        if query.is_null()
            || resources.is_null()
            || query_length > 16_384
            || resources_length > 4096
        {
            return Err("invalid local emoji buffer".into());
        }
        let query = serde_json::from_slice::<EmojiCatalogQuery>(unsafe {
            std::slice::from_raw_parts(query, query_length)
        })
        .map_err(|_| "invalid emoji query document")?;
        let resources = super::parse_absolute_path(
            unsafe { std::slice::from_raw_parts(resources, resources_length) },
            "resources path is not UTF-8",
            "resources path must be absolute",
        )?;
        if query.offset > i64::MAX as usize || query.panel.limit == 0 {
            return Err("invalid emoji page".into());
        }
        if query.list_groups {
            let groups = msime_engine::host::emoji_catalog_groups(resources, &query.panel.category)
                .map_err(|_| "local emoji catalog unavailable")?;
            return Ok(json!({"groups": groups}));
        }
        if query.list_plugin_symbol_groups {
            // 符号集插件不依赖 msime-others.db：目录不可用时内置符号读不出来，插件组照样给。没传插件目录时没有插件组。
            let groups = match query.plugins.as_deref() {
                None => Vec::new(),
                Some(plugins) if std::path::Path::new(plugins).is_absolute() => {
                    crate::plugin_symbol_groups(std::path::Path::new(plugins))
                }
                Some(_) => return Err("plugins path must be absolute".into()),
            };
            return Ok(json!({ "plugin_symbol_groups": groups }));
        }
        if query.list_symbol_groups {
            let groups = msime_engine::host::emoji_symbol_groups(resources)
                .map_err(|_| "local emoji catalog unavailable")?;
            let mut symbol_groups = Vec::with_capacity(groups.len());
            symbol_groups.extend(
                groups
                    .into_iter()
                    .map(|group| json!({"parent":group.parent,"title":group.title})),
            );
            return Ok(json!({"symbol_groups": symbol_groups}));
        }
        if !query.parent.is_empty() && query.panel.category != "symbols" {
            return Err("parent filter requires symbols catalog".into());
        }
        if query.cursor {
            let slice = msime_engine::host::emoji_catalog_slice(
                resources,
                &query.panel.search,
                &query.panel.category,
                &query.group,
                query.offset,
                u16::from(query.panel.limit),
                &query.parent,
            )
            .map_err(|_| "local emoji catalog unavailable")?;
            let next_offset = slice.next_offset;
            let complete = slice.complete;
            let mut items = Vec::with_capacity(slice.items.len());
            items.extend(slice.items.into_iter().map(|item| {
                json!({
                    "text": item.text, "annotation": item.annotation, "group": item.group,
                })
            }));
            return Ok(json!({
                "items": items,
                "next_offset": next_offset,
                "complete": complete,
            }));
        }
        let items = msime_engine::host::emoji_catalog_filtered_page(
            resources,
            &query.panel.search,
            &query.panel.category,
            &query.group,
            query.offset,
            u16::from(query.panel.limit),
            &query.parent,
        )
        .map_err(|_| "local emoji catalog unavailable")?;
        let mut rendered_items = Vec::with_capacity(items.len());
        rendered_items.extend(items.into_iter().map(|item| {
            json!({
                "text": item.text,
                "annotation": item.annotation,
                "group": item.group,
            })
        }));
        Ok(json!({
            "items": rendered_items
        }))
    })
}

#[cfg(test)]
mod tests {
    use super::online_candidate_response;
    use serde_json::json;

    #[test]
    fn online_candidate_response_carries_only_the_batch() {
        let value =
            online_candidate_response(vec![("first".to_owned(), 0), ("second".to_owned(), 1)]);

        assert_eq!(
            value,
            json!({"candidates": [
                {"text": "first", "source": 0},
                {"text": "second", "source": 1},
            ]})
        );
    }
}
