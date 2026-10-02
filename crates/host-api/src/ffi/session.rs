//! Session lifetime and the voice capture cycle that belongs to one session.
//!
//! Part of the C ABI; see the parent module for what these shims guarantee.

use crate::*;

/// # Safety
/// `options` must point to `length` readable bytes for this call. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_create(options: *const u8, length: usize) -> *mut c_char {
    response(|| {
        if options.is_null() || length > HOST_OPTIONS_DOCUMENT_LIMIT {
            return Err("invalid options buffer".into());
        }
        // SAFETY: guaranteed by the C caller's documented buffer contract.
        let bytes = unsafe { std::slice::from_raw_parts(options, length) };
        let options = serde_json::from_slice(bytes)
            .ok()
            .and_then(HostOptions::from_document)
            .ok_or("invalid options document")?;
        if options.api_version != 1 {
            return Err("unsupported host API version".into());
        }
        options.preferences.validate().map_err(|e| e.to_string())?;
        let page_size = options.preferences.candidate_page_size;
        let applied = options.preferences.clone();
        let ai_provider_cache = {
            let ai = &applied.ai_assistant;
            ai.enabled.then(|| AiAssistantProviderConfig {
                enabled: true,
                provider: ai.provider.clone(),
                model: ai.model.clone(),
                endpoint: ai.endpoint.clone(),
                candidate_limit: ai.candidate_limit,
                prompt_id: ai.prompt_id.clone(),
                prompt_custom_1: ai.prompt_custom_1.clone(),
                prompt_custom_2: ai.prompt_custom_2.clone(),
                prompt_custom_3: ai.prompt_custom_3.clone(),
            })
        };
        // Taken before the options are consumed, and kept separate from the engine's own paths.
        let sentence_model_path = options.sentence_model.clone();
        let settled_model_path = options.settled_model.clone();
        let phrase_preedit = options.phrase_preedit.unwrap_or(false);
        let plugin_roots = key_sound::PluginRoots::new(
            options.preferences_directory.as_deref(),
            options.sound_packs.as_deref(),
            &options.resources,
        );
        let mut options = options.into_engine_options();
        let plugin_tables = plugin_tables::PluginTables::stamp(
            plugin_roots.installed.as_deref(),
            &options,
            &applied.plugins,
        );
        plugin_tables.fill(
            &plugin_tables::PluginTables::default(),
            plugin_roots.installed.as_deref(),
            &mut options,
        );
        let sound = key_sound::SessionSound::new(key_sound::SoundSettings::new(
            &applied.plugins,
            &plugin_roots,
        ));
        let dictionary_access = DictionaryAccess::try_session(
            std::path::Path::new(&options.user_data),
            std::path::Path::new(&options.dictionaries),
        )
        .map_err(|_| "dictionary access unavailable".to_owned())?
        .ok_or_else(|| "dictionary maintenance busy".to_owned())?;
        let mut engine = Session::new(&options).map_err(|e| e.to_string())?;
        // Gate on the scheme actually run, as `apply_pending` does, so a preferred scheme that fell back to Quanpin starts in the same mode a rebuild would give it.
        let default_nine_key = SchemeType::from_u8(options.scheme)
            .is_some_and(SchemeType::nine_key)
            && matches!(applied.touch_keyboard_layout, TouchKeyboardLayout::NineKey);
        if default_nine_key {
            engine
                .set_nine_key_enabled(true)
                .map_err(|e| e.to_string())?;
        }
        // Places in `@` mode start off in every engine, so the session carries the preference over itself, as `apply_pending` does on a rebuild.
        engine
            .set_mention_places(applied.local_modes.mention_places)
            .map_err(|e| e.to_string())?;
        // 默认输入状态 says which state a new focus session starts in, and the
        // host applies it as its own English passthrough - letters go straight
        // to the document, with no session involved. It is not the Engine's
        // dedicated English mode, which keeps a session and answers with
        // English word candidates. Seeding one from the other left a session
        // that could never reach Chinese: the host's toggle only flips
        // passthrough, so the "Chinese" half of it was English candidates, and
        // nothing on the way back clears a mode the user never turned on.
        // Dedicated English starts off and is only ever set by the menu row or
        // the hotkey that owns it.
        let mut runtime =
            Runtime::new_with_touch_layout(engine, page_size, applied.touch_keyboard_layout)
                .map_err(|e| e.to_string())?;
        runtime.set_phrase_preedit(phrase_preedit);
        runtime.set_reranker(
            sentence_model(&options.resources, sentence_model_path.as_deref()).map(Reranker::new),
        );

        // The settled model is optional and independent: a resource set that ships only the small
        // one behaves exactly as before, and one that ships both gets the fast model per keystroke
        // and the large one when typing stops.
        runtime.set_settled_reranker(
            sentence_model_settled(&options.resources, settled_model_path.as_deref())
                .map(Reranker::new),
        );
        // Loaded whenever it is installed, run only while the desktop model preference is on; `apply_pending` follows later changes of the switch.
        runtime.set_settled_rerank_enabled(applied.sentence_association.neural_desktop);

        let view = runtime.view();
        let output = serde_json::to_value(&view).map_err(|e| e.to_string())?;
        SESSIONS.with(|sessions| {
            sessions.borrow_mut().insert(
                view.session,
                HostSession {
                    runtime,
                    options,
                    applied,
                    requested: None,
                    preferences_pending: false,
                    punctuation_override: None,
                    paired_punctuation_override: None,
                    punctuation_lock_override: None,
                    english_mode: false,
                    page_size_override: None,
                    nine_key_override: None,
                    ai_credential: None,
                    ai_provider_cache,
                    voice: VoiceSessionState::default(),
                    pending_selections: Default::default(),
                    plugin_roots,
                    sound,
                    plugin_tables,
                    _dictionary_access: dictionary_access,
                },
            )
        });
        Ok(output)
    })
}

#[no_mangle]
pub extern "C" fn msime_client_focus(handle: u64, focused: bool) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            // Focus-out is where a host's field ends, so selections counted in it are written here rather than waiting for a batch to fill. The same goes for the engine's delayed context learning: the iOS keyboard extension and the Android IME process can be killed without ever destroying their sessions, and the engine no longer writes that queue at process exit.
            if !focused {
                session.flush_selections();
                msime_engine::flush_personal_learning();
            } else {
                session.refresh_plugin_tables()?;
                // The settings page may have imported the pack in use again, or removed it, since this session last looked.
                session.sound.restamp();
            }
            let result = session.runtime.focus(focused).map_err(|e| e.to_string())?;
            let result = session.complete_transition(result);
            serde_json::to_value(result).map_err(|e| e.to_string())
        })
    })
}

#[no_mangle]
pub extern "C" fn msime_client_reset_cache(handle: u64) -> *mut c_char {
    dispatch(handle, Action::ResetCache)
}

#[no_mangle]
pub extern "C" fn msime_client_voice_start(handle: u64) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            let generation = session.voice.start();
            if generation == 0 {
                return Err("voice generation exhausted".into());
            }
            Ok(json!(generation))
        })
    })
}

#[no_mangle]
pub extern "C" fn msime_client_voice_cancel(handle: u64) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            session.voice.cancel();
            Ok(Value::Null)
        })
    })
}

/// Capture a bounded PCM16-compatible sample buffer from the default input device. The returned JSON contains only the samples for this call; callers must transport them immediately and must not log or persist them.
#[no_mangle]
pub extern "C" fn msime_client_voice_capture(milliseconds: u32) -> *mut c_char {
    response(|| {
        if !(1..=60_000).contains(&milliseconds) {
            return Err("invalid voice capture duration".into());
        }
        let samples = crate::voice_capture::capture_audio(milliseconds);
        if samples.is_empty() {
            return Err("voice capture unavailable".into());
        }
        Ok(
            json!({ "sample_rate": crate::voice_capture::SAMPLE_RATE, "channels": 1, "samples": samples }),
        )
    })
}

/// Apply asynchronous ASR text only for the active voice token.
///
/// # Safety
/// `text` must point to a readable UTF-8 buffer of `length` bytes and must not
/// be null. The buffer is not retained.
#[no_mangle]
pub unsafe extern "C" fn msime_client_voice_apply(
    handle: u64,
    generation: u64,
    text: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if text.is_null() || length > 65536 {
            return Err("invalid voice text buffer".into());
        }
        let text = std::str::from_utf8(unsafe { std::slice::from_raw_parts(text, length) })
            .map_err(|_| "voice text is not UTF-8")?;
        with_session(handle, |session| {
            Ok(session
                .voice
                .apply(generation, text)
                .map(Value::String)
                .unwrap_or(Value::Null))
        })
    })
}
