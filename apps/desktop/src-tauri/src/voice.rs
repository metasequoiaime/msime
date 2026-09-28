//! One voice recognition session, from the provider configuration the user chose
//! to the transcript the panel shows.
//!
//! The audio never reaches this crate: capture and the provider exchange happen
//! behind an injected transport, and what crosses this boundary is the request,
//! the bounded progress updates, and the final text.

use crate::*;

pub(crate) mod local_models;

#[derive(serde::Deserialize)]
pub(crate) struct VoiceRecognitionRequest {
    pub(crate) language: String,
    pub(crate) request_id: String,
}

#[derive(serde::Serialize)]
pub(crate) struct VoiceRecognitionResult {
    pub(crate) text: String,
}

// Resolved in the shared layer so the Android keyboard, which never goes through this shell,
// reads the same answer through the C ABI rather than growing a second implementation.
#[cfg(any(target_os = "ios", target_os = "android", test))]
pub(crate) use msime_client_core::voice::provider::{
    mobile_voice_polish_configuration, mobile_voice_provider_configuration,
};

#[cfg(any(unix, windows))]
#[derive(serde::Serialize, Clone)]
pub(crate) struct VoiceRecognitionUpdate {
    pub(crate) text: String,
    pub(crate) request_id: String,
    #[serde(rename = "final")]
    pub(crate) final_result: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) level: Option<f32>,
}

#[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
pub(crate) fn utf8_prefix(value: &str, max_bytes: usize) -> &str {
    let mut end = value.len().min(max_bytes);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

#[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
pub(crate) fn voice_provider_options(document: &Value) -> Result<Value, HostActionError> {
    let Some(voice) = document
        .get("preferences")
        .and_then(|value| value.get("voice_input"))
        .and_then(Value::as_object)
    else {
        return Ok(Value::Object(Default::default()));
    };
    let mut options = serde_json::Map::new();
    for key in [
        "sound_enabled",
        "start_sound",
        "end_sound",
        "mute_system_audio",
        "polish_enabled",
        "polish_text",
        "doubao_enable_itn",
        "doubao_enable_punc",
        "doubao_enable_ddc",
        "stream_inline_preedit",
    ] {
        if let Some(value) = voice.get(key).filter(|value| value.is_boolean()) {
            options.insert(key.to_owned(), value.clone());
        }
    }
    for key in [
        "capture_backend",
        "capture_device",
        "commit_mode",
        "asr_provider",
        "doubao_auth_mode",
        "asr_model",
        "asr_resource_id",
        "polish_provider",
        "polish_model",
        "polish_prompt_id",
        "doubao_boosting_table_id",
    ] {
        if let Some(value) = voice.get(key).and_then(Value::as_str) {
            if key == "doubao_auth_mode" && !matches!(value, "api_key" | "legacy") {
                continue;
            }
            options.insert(
                key.to_owned(),
                Value::String(utf8_prefix(value, 512).to_owned()),
            );
        }
    }
    // A path is not cut like the names above: a truncated path names a different file. One too long for any host is left out and the provider reports the model missing.
    if let Some(path) = voice
        .get("asr_model_path")
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty() && msime_client_core::is_bounded_text(path, 4096))
    {
        options.insert("asr_model_path".to_owned(), Value::String(path.to_owned()));
    }
    let preset = voice
        .get("polish_prompt_id")
        .and_then(Value::as_str)
        .unwrap_or("cleanup");
    let prompt_key = match preset {
        "custom" | "custom_1" => Some("polish_prompt_custom_1"),
        "custom_2" => Some("polish_prompt_custom_2"),
        "custom_3" => Some("polish_prompt_custom_3"),
        _ => None,
    };
    if let Some(key) = prompt_key {
        let mut prompt = voice.get(key).and_then(Value::as_str).unwrap_or("");
        if prompt.is_empty() && key == "polish_prompt_custom_1" {
            prompt = voice
                .get("polish_prompt")
                .and_then(Value::as_str)
                .unwrap_or("");
        }
        if prompt.len() > 8192 {
            return Err(HostActionError {
                code: "invalid_voice",
            });
        }
        if !prompt.is_empty() {
            options.insert(key.to_owned(), Value::String(prompt.to_owned()));
        }
    }
    Ok(Value::Object(options))
}

// Resolve on each request so services started after the panel remain discoverable.

#[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
pub(crate) fn resolve_voice_provider_socket(
    document: &serde_json::Value,
) -> Option<std::path::PathBuf> {
    document
        .get("voice_provider_socket")
        .and_then(serde_json::Value::as_str)
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("MSIME_VOICE_PROVIDER_SOCKET")
                .map(std::path::PathBuf::from)
                .filter(|path| path.is_absolute())
        })
        .or_else(|| discover_session_provider("voice.sock"))
}

/// The most the provider options may serialise to. The provider socket refuses a request over 16 KiB, and the envelope around the options (version, kind, language, generation, events) stays well under the remaining 512 bytes.
#[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
pub(crate) const PROVIDER_OPTIONS_BUDGET: usize = 16_384 - 512;

/// The user's dictionary words for a mobile `local` session; none for a network provider, whose request may not carry them.
#[cfg(any(target_os = "ios", target_os = "android"))]
fn mobile_session_hotwords(
    configuration: &msime_client_core::voice::provider::MobileVoiceProviderConfiguration,
    dictionary: &DictionaryHostOptions,
) -> Vec<msime_tauri_mobile_platform::MobileVoiceHotword> {
    if configuration.provider != "local" {
        return Vec::new();
    }
    local_models::session_hotwords(dictionary)
        .into_iter()
        .map(|hotword| msime_tauri_mobile_platform::MobileVoiceHotword {
            text: hotword.text,
            pinyin: hotword.pinyin,
        })
        .collect()
}

pub(crate) fn refresh_voice_preferences(
    mut document: Value,
    store: &PreferencesStore,
) -> Result<Value, HostActionError> {
    let preferences = store.load().map_err(|_| HostActionError {
        code: "unavailable",
    })?;
    document["preferences"] =
        serde_json::to_value(preferences.preferences).map_err(|_| HostActionError {
            code: "unavailable",
        })?;
    Ok(document)
}

#[tauri::command]
pub(crate) async fn recognize_voice(
    app: tauri::AppHandle,
    request: VoiceRecognitionRequest,
    runtime: tauri::State<'_, RuntimeOptionsState>,
    store: tauri::State<'_, Arc<PreferencesStore>>,
    dictionary: tauri::State<'_, DictionaryHostOptions>,
) -> Result<VoiceRecognitionResult, HostActionError> {
    #[cfg(windows)]
    let _ = (&runtime, &store, &dictionary);
    #[cfg(target_os = "ios")]
    let _ = &runtime;
    #[cfg(target_os = "android")]
    let _ = (&runtime, &store);
    #[cfg(not(any(unix, windows)))]
    let _ = (&app, &runtime, &store, &dictionary);
    if request.request_id.is_empty()
        || request.request_id.len() > 64
        || !request
            .request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || request.language.is_empty()
        || request.language.len() > 64
        || request.language.chars().any(char::is_control)
    {
        return Err(HostActionError {
            code: "invalid_voice",
        });
    }
    #[cfg(windows)]
    {
        windows_voice::recognize(app, request).await
    }
    #[cfg(target_os = "android")]
    {
        let platform = app
            .state::<AndroidVoicePlatform<tauri::Wry>>()
            .inner()
            .clone();
        // A configured transcription provider is used when there is one, and its absence is not an
        // error: this host has a platform recognizer that works with no account at all, and that
        // stays the default. `unsupported_voice` is what an unset or unknown provider produces, and
        // a provider whose credentials do not validate is dropped the same way.
        let store = store.inner().clone();
        let provider_store = store.clone();
        let dictionary = dictionary.inner().clone();
        let provider = tauri::async_runtime::spawn_blocking(move || {
            let snapshot = provider_store.load().ok()?;
            let configuration = mobile_voice_provider_configuration(&snapshot.preferences)?;
            let hotwords = mobile_session_hotwords(&configuration, &dictionary);
            Some((configuration, hotwords))
        })
        .await
        .ok()
        .flatten()
        .map(
            |(configuration, hotwords)| MobileVoiceTranscriptionRequest {
                request_id: request.request_id.clone(),
                provider: configuration.provider,
                endpoint: configuration.endpoint,
                model: configuration.model,
                token: configuration.token,
                headers: configuration
                    .headers
                    .into_iter()
                    .map(|header| MobileVoiceRequestHeader {
                        name: header.name,
                        value: header.value,
                    })
                    .collect(),
                enable_itn: configuration.enable_itn,
                enable_punctuation: configuration.enable_punctuation,
                enable_ddc: configuration.enable_ddc,
                boosting_table_id: configuration.boosting_table_id,
                model_path: configuration.model_path,
                hotwords,
            },
        );
        let polish_store = store.clone();
        let polish = tauri::async_runtime::spawn_blocking(move || {
            let snapshot = polish_store.load().ok()?;
            mobile_voice_polish_configuration(&snapshot.preferences)
        })
        .await
        .ok()
        .flatten()
        .map(|configuration| AndroidVoicePolishRequest {
            endpoint: configuration.endpoint,
            model: configuration.model,
            token: configuration.token,
            prompt_id: configuration.prompt_id,
            prompt_legacy: configuration.prompt_legacy,
            prompt_custom_1: configuration.prompt_custom_1,
            prompt_custom_2: configuration.prompt_custom_2,
            prompt_custom_3: configuration.prompt_custom_3,
        });
        let text = platform
            .recognize_voice(&request.request_id, &request.language, provider, polish)
            .await
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?;
        return Ok(VoiceRecognitionResult { text });
    }
    #[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
    {
        let runtime = runtime.inner().clone();
        let store = store.inner().clone();
        let dictionary = dictionary.inner().clone();
        let document = tauri::async_runtime::spawn_blocking(move || {
            let document = runtime.snapshot().map_err(|_| HostActionError {
                code: "unavailable",
            })?;
            // The shared preference store is also written by IBus, the macOS
            // native settings bridge, and other settings windows; new
            // recordings must use those saved settings instead of the
            // launch-time HostOptions snapshot. This matters on macOS where
            // the native IMK process and the Tauri panel can remain alive
            // while a settings window changes the active provider.
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            let document = refresh_voice_preferences(document, &store)?;
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            let _ = store;
            let mut provider_options = voice_provider_options(&document)?;
            if provider_options.get("asr_provider").and_then(Value::as_str) == Some("local") {
                local_models::add_hotwords_within(
                    &mut provider_options,
                    &local_models::session_hotwords(&dictionary),
                    PROVIDER_OPTIONS_BUDGET,
                );
            }
            Ok::<_, HostActionError>((document, provider_options))
        })
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })??;
        let (document, provider_options) = document;
        let path = resolve_voice_provider_socket(&document).ok_or(HostActionError {
            code: "unavailable",
        })?;
        let sessions = app.state::<voice_sessions::VoiceSessions>();
        let session = sessions
            .begin(request.request_id, path)
            .ok_or(HostActionError { code: "busy" })?;
        let generation = session.generation;
        let language = request.language;
        let worker_app = app.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let mut update = |text: &str, final_result: bool| {
                if session.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                let _ = worker_app.emit(
                    "voice-update",
                    VoiceRecognitionUpdate {
                        text: text.to_owned(),
                        request_id: session.request_id.clone(),
                        final_result,
                        phase: None,
                        level: None,
                    },
                );
            };
            let mut status = |phase: &str| {
                if session.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                let _ = worker_app.emit(
                    "voice-update",
                    VoiceRecognitionUpdate {
                        text: String::new(),
                        request_id: session.request_id.clone(),
                        final_result: false,
                        phase: Some(phase.to_owned()),
                        level: None,
                    },
                );
            };
            let mut level = |level: f32| {
                if session.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                let _ = worker_app.emit(
                    "voice-update",
                    VoiceRecognitionUpdate {
                        text: String::new(),
                        request_id: session.request_id.clone(),
                        final_result: false,
                        phase: None,
                        level: Some(level),
                    },
                );
            };
            UnixSocketProvider::new(session.path.clone()).voice_stream_with_options_feedback(
                &language,
                generation,
                &provider_options,
                Some(&session.cancelled),
                &mut update,
                Some(&mut status),
                Some(&mut level),
            )
        })
        .await;
        sessions.finish(generation);
        let text = result
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?
            .ok_or(HostActionError {
                code: "unavailable",
            })?;
        Ok(VoiceRecognitionResult { text })
    }
    #[cfg(target_os = "ios")]
    {
        let store = store.inner().clone();
        let dictionary = dictionary.inner().clone();
        let (configuration, hotwords) = tauri::async_runtime::spawn_blocking(move || {
            let snapshot = store.load().map_err(|_| HostActionError {
                code: "unavailable",
            })?;
            // `None` is "no usable provider configured", which on this host is a failure: the iOS
            // keyboard extension has no platform recogniser to fall back to.
            let configuration = mobile_voice_provider_configuration(&snapshot.preferences).ok_or(
                HostActionError {
                    code: "unsupported_voice",
                },
            )?;
            // The iOS plugin only uploads to a provider; it has no on-device recogniser yet, so a
            // local model is unsupported here rather than a generic plugin rejection.
            if configuration.provider == "local" {
                return Err(HostActionError {
                    code: "unsupported_voice",
                });
            }
            let hotwords = mobile_session_hotwords(&configuration, &dictionary);
            Ok::<_, HostActionError>((configuration, hotwords))
        })
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })??;
        let request_id = request.request_id;
        let _ = app.emit(
            "voice-update",
            VoiceRecognitionUpdate {
                text: String::new(),
                request_id: request_id.clone(),
                final_result: false,
                phase: Some("recording".into()),
                level: None,
            },
        );
        let platform = app.state::<MobilePlatform<tauri::Wry>>().inner().clone();
        let response = platform
            .recognize_voice(MobileVoiceTranscriptionRequest {
                request_id,
                provider: configuration.provider,
                endpoint: configuration.endpoint,
                model: configuration.model,
                token: configuration.token,
                headers: configuration
                    .headers
                    .into_iter()
                    .map(|header| MobileVoiceRequestHeader {
                        name: header.name,
                        value: header.value,
                    })
                    .collect(),
                enable_itn: configuration.enable_itn,
                enable_punctuation: configuration.enable_punctuation,
                enable_ddc: configuration.enable_ddc,
                boosting_table_id: configuration.boosting_table_id,
                model_path: configuration.model_path,
                hotwords,
            })
            .await
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?;
        Ok(VoiceRecognitionResult {
            text: response.text,
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (request, runtime);
        Err(HostActionError {
            code: "unavailable",
        })
    }
}

#[tauri::command]
pub(crate) fn stop_voice(app: tauri::AppHandle, request_id: String) -> Result<(), HostActionError> {
    #[cfg(target_os = "ios")]
    {
        app.state::<MobilePlatform<tauri::Wry>>()
            .stop_voice(&request_id)
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?;
        let _ = app.emit(
            "voice-update",
            VoiceRecognitionUpdate {
                text: String::new(),
                request_id,
                final_result: false,
                phase: Some("recognizing".into()),
                level: None,
            },
        );
        Ok(())
    }
    #[cfg(target_os = "android")]
    {
        app.state::<AndroidVoicePlatform<tauri::Wry>>()
            .stop_voice(&request_id)
            .map_err(|_| HostActionError {
                code: "unavailable",
            })
    }
    #[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
    {
        let sessions = app.state::<voice_sessions::VoiceSessions>();
        // Keep the session alive for the provider's final transcription, but
        // publish the stop state before sending the control message. This is
        // the same ownership boundary as the Windows controller: a late
        // result belongs to this generation, while a successor recording
        // cannot be admitted until the worker finishes.
        let Some(session) = sessions.stop(&request_id) else {
            return Ok(());
        };
        if UnixSocketProvider::new(session.path).voice_stop(session.generation) {
            return Ok(());
        }
        Err(HostActionError {
            code: "unavailable",
        })
    }
    #[cfg(not(unix))]
    {
        let sessions = app.state::<voice_sessions::VoiceSessions>();
        let _ = sessions.stop(&request_id);
        Ok(())
    }
}

#[tauri::command]
pub(crate) fn cancel_voice(
    app: tauri::AppHandle,
    request_id: Option<String>,
) -> Result<(), HostActionError> {
    #[cfg(target_os = "ios")]
    {
        app.state::<MobilePlatform<tauri::Wry>>()
            .cancel_voice(request_id.as_deref())
            .map_err(|_| HostActionError {
                code: "unavailable",
            })
    }
    #[cfg(target_os = "android")]
    {
        app.state::<AndroidVoicePlatform<tauri::Wry>>()
            .cancel_voice(request_id.as_deref())
            .map_err(|_| HostActionError {
                code: "unavailable",
            })
    }
    #[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
    {
        let sessions = app.state::<voice_sessions::VoiceSessions>();
        let Some(session) = sessions.cancel(request_id.as_deref()) else {
            return Ok(());
        };
        if UnixSocketProvider::new(session.path).voice_cancel(session.generation) {
            return Ok(());
        }
        Err(HostActionError {
            code: "unavailable",
        })
    }
    #[cfg(not(unix))]
    {
        // The Windows worker observes cancellation during I/O, drains it and
        // closes its connection; the Server cancels only that review session.
        let sessions = app.state::<voice_sessions::VoiceSessions>();
        let _ = sessions.cancel(request_id.as_deref());
        Ok(())
    }
}
