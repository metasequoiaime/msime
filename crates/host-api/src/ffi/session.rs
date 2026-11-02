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
        create_session(bytes, SessionUse::Input)
    })
}

/// 会话是给谁用的。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionUse {
    /// 宿主的输入会话。
    Input,
    /// 只用来看一串按键给出哪些候选（`candidate_preview`）：不向设置页报告词库代次已经载入（输入法本身可能还在用旧的那一代），不记打字统计。
    Preview,
}

/// `msime_client_create` 的本体，返回新会话的 view。
pub(crate) fn create_session(bytes: &[u8], usage: SessionUse) -> Result<Value, String> {
    let options = serde_json::from_slice(bytes)
        .ok()
        .and_then(HostOptions::from_document)
        .ok_or("invalid options document")?;
    if options.api_version != 1 {
        return Err("unsupported host API version".into());
    }
    options.preferences.validate().map_err(|e| e.to_string())?;
    let page_size = host_page_size(options.preferences.candidate_page_size);
    let applied = options.preferences.clone();
    let ai_provider_cache = {
        let ai = &applied.ai_assistant;
        ai.enabled.then(|| AiAssistantProviderConfig {
            enabled: true,
            provider: ai.provider.clone(),
            model: ai.model.clone(),
            endpoint: ai.endpoint.clone(),
            candidate_limit: host_ai_candidate_limit(ai.candidate_limit),
            thinking: ai.thinking.clone(),
            prompt_id: ai.prompt_id.clone(),
            prompt_custom_1: ai.prompt_custom_1.clone(),
            prompt_custom_2: ai.prompt_custom_2.clone(),
            prompt_custom_3: ai.prompt_custom_3.clone(),
        })
    };
    // Taken before the options are consumed, and kept separate from the engine's own paths.
    let sentence_model_path = options.sentence_model.clone();
    let recorded_settled_model = options.settled_model.clone();
    let phrase_preedit = options.phrase_preedit.unwrap_or(false);
    let statistics_recorded_elsewhere = options.selection_statistics == Some(false);
    // 聚焦时据此重新查找按需下载的资源包，见 `HostSession::refresh_resource_packs`。
    let state_root = absolute_state_root(options.preferences_directory.as_deref());
    let recorded_language_dictionaries = options
        .language_dictionaries
        .as_deref()
        .map(std::path::PathBuf::from);
    let plugin_roots = key_sound::PluginRoots::new(
        options.preferences_directory.as_deref(),
        options.sound_packs.as_deref(),
        options.plugin_packs.as_deref(),
        &options.resources,
    );
    let edition = options.edition();
    let quick_phrase_candidates_requested = options.quick_phrase_candidates.unwrap_or(false);
    let korean_romaja_requested = options.korean_romaja.unwrap_or(false);
    let mut options = options.into_engine_options();
    let plugin_tables = plugin_tables::PluginTables::stamp(
        plugin_roots.installed.as_deref(),
        plugin_roots.builtin_packs.as_deref(),
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
    // 在 Engine 打开资源文件之前占用下载的词库代次，会话活着时设置宿主不会把它当旧版本删掉。随包资源、鸿蒙键盘的镜像或者锁文件建不了时没有凭证，不影响输入。
    let generation_lease = state_root.as_deref().and_then(|state| {
        msime_client_providers::dictionary_updates::lease(
            state,
            std::path::Path::new(&options.resources),
        )
    });
    let mut engine = Session::new(&options).map_err(|e| e.to_string())?;
    // 设置宿主据此判断下载的词库代次是否已经生效。写不进状态目录（例如 iOS 键盘没有完全访问）时设置页退回原来的判断，不影响输入，所以不让会话因此失败。
    if let Some(state) = state_root.as_deref().filter(|_| usage == SessionUse::Input) {
        let _ = msime_client_providers::dictionary_updates::record_loaded(
            state,
            std::path::Path::new(&options.resources),
        );
    }
    // Gate on the scheme actually run, as `apply_pending` does, so a preferred scheme that fell back to Quanpin starts in the same mode a rebuild would give it.
    let default_key_grid = layout_key_grid(SchemeType::from_u8(options.scheme), &applied);
    if default_key_grid.is_some() {
        engine
            .set_key_grid(default_key_grid)
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
    runtime.set_english_first_page_count(applied.mixed_input.first_page_count);
    runtime
        .set_english_online_candidates_enabled(applied.english_online_candidates)
        .map_err(|e| e.to_string())?;
    runtime
        .set_cloud_candidates_enabled(applied.cloud_candidates)
        .map_err(|e| e.to_string())?;
    runtime.set_translation_scope_highlighted(crate::translation_scope_highlighted(&applied));
    runtime.set_phrase_preedit(phrase_preedit);
    runtime.set_backspace_reverts_selection(applied.backspace_reverts_selection);
    runtime.set_escape_keeps_selected_word(applied.escape_keeps_selected_word);
    runtime.set_left_right_navigation(applied.navigation.left_right);
    runtime.set_vertical_candidates(crate::host_vertical_candidates(applied.candidate_layout));
    runtime.set_wubi_auto_commit_unique(applied.wubi_auto_commit_unique);
    runtime.set_wubi_fifth_code_top_commit(applied.wubi_fifth_code_top_commit);
    runtime.set_wubi_candidate_selection(applied.wubi_candidate_selection.clone());
    runtime.set_second_third_candidate(applied.second_third_candidate);
    runtime.set_punctuation_marks(msime_client_core::punctuation::punctuation_marks(&applied));
    runtime.set_multilingual_output(
        applied.traditional_chinese_output,
        applied.traditional_chinese_variant,
    );
    runtime.set_candidate_scorer(keyboard_reranker(
        keyboard_reranker_enabled(&applied.sentence_association, edition),
        &options.resources,
        options.helpcode_fallback_resources.as_deref(),
        sentence_model_path.as_deref(),
    ));

    // The settled model is optional and independent: a resource set that ships only the small
    // one behaves exactly as before, and one that ships both gets the fast model per keystroke
    // and the large one when typing stops.
    // 随包的模型优先，没有时用已下载的 `settled-model` 资源包；会话打开后才装好的由 `refresh_resource_packs` 接上。
    let settled_model = settled_model_file(
        &options.resources,
        recorded_settled_model.as_deref(),
        state_root.as_deref(),
    );
    let settled_reranker = settled_model
        .as_deref()
        .and_then(std::path::Path::to_str)
        .and_then(|path| sentence_model_settled(&options.resources, Some(path)))
        .map(Reranker::new);
    let settled_model_attached = settled_reranker.is_some();
    runtime.set_settled_reranker(settled_reranker);
    // Loaded whenever it is installed, run only while the desktop model preference is on; `apply_pending` follows later changes of the switch.
    runtime.set_settled_rerank_enabled(applied.sentence_association.neural_desktop);
    // 开关开着的输入会话写一行模型接上没有，诊断日志据此分得清没装、读不了和宿主没给状态目录（#10341）。
    if usage == SessionUse::Input && applied.sentence_association.neural_desktop {
        crate::report_settled_model(
            &options.resources,
            state_root.as_deref(),
            settled_model.as_deref(),
            settled_model_attached,
        );
    }

    let view = runtime.view();
    let output = serde_json::to_value(&view).map_err(|e| e.to_string())?;
    SESSIONS.with(|sessions| {
        sessions.borrow_mut().insert(
            view.session,
            HostSession {
                runtime,
                options,
                quick_phrase_candidates_requested,
                korean_romaja_requested,
                edition,
                applied,
                requested: None,
                requested_settled: false,
                preferences_pending: false,
                punctuation_override: None,
                paired_punctuation_override: None,
                punctuation_lock_override: None,
                english_mode: false,
                caps_lock: false,
                page_size_override: None,
                key_grid_override: None,
                statistics_private: false,
                preview: usage == SessionUse::Preview,
                statistics_recorded_elsewhere,
                ai_credential: None,
                ai_provider_cache,
                voice: VoiceSessionState::default(),
                pending_selections: Default::default(),
                pending_efficiency: Default::default(),
                statistics_enabled: None,
                plugin_roots,
                sound,
                plugin_tables,
                state_root,
                recorded_language_dictionaries,
                resources_pending: false,
                recorded_sentence_model: sentence_model_path,
                recorded_settled_model,
                settled_model,
                settled_model_attached,
                settled_model_loading: None,
                lifecycle: None,
                _dictionary_access: dictionary_access,
                _generation_lease: generation_lease,
            },
        )
    });
    Ok(output)
}

#[no_mangle]
pub extern "C" fn msime_client_focus(handle: u64, focused: bool) -> *mut c_char {
    response(|| focus_session(handle, focused))
}

/// 写出本进程攒着的打字统计上屏（`record` 的批次）和会话交出的选词位置（`flush_selections`）：`inline` 时在当前线程上写完再返回，否则交给后台线程。
///
/// 失焦和会话销毁都要写：移动宿主的键盘扩展和输入法进程可能在收起后被系统直接杀掉，桌面宿主停下不打字以后最后一批也要等到下一次上屏满间隔才写，设置页一直看不到。宿主另在统计工作线程上调 `flush` 时这里再写一次也只是空转。
pub(crate) fn flush_typing_statistics(inline: bool) {
    if inline {
        msime_client_services::typing_statistics::flush_pending_commits();
    } else {
        msime_client_services::typing_statistics::flush_pending_commits_in_background();
    }
}

/// 失焦时在当前线程上写统计。只有 Windows：它的失焦在服务进程的 IPC 线程上。macOS 的失焦在 IMK 主线程、Linux 的在 GLib 主循环、移动宿主的可能在 UI 线程上，而它们的 `record` 在别的线程上跑：在这里写会把整份读写和 fsync 放到主线程，还可能等统计线程手里那次写的 fsync，所以交给后台线程。
const FLUSHES_TYPING_STATISTICS_INLINE_ON_FOCUS_OUT: bool = cfg!(target_os = "windows");

/// 销毁会话时在当前线程上写统计。桌面宿主销毁会话不频繁，之后进程可能就退出了，后台线程来不及写完；移动宿主（Android 每换一个输入框就销毁重建一次会话，鸿蒙在 ArkTS 里同步销毁）的销毁在 UI 线程上，交给后台线程。
pub(crate) const FLUSHES_TYPING_STATISTICS_INLINE_ON_DESTROY: bool = !cfg!(any(
    target_os = "android",
    target_env = "ohos",
    target_os = "ios"
));

/// `msime_client_focus` 的本体。
pub(crate) fn focus_session(handle: u64, focused: bool) -> Result<Value, String> {
    with_session(handle, |session| {
        // 在换插件表之前采样：换表会刷新打开着的 `/` 列表，这也算这次 focus 的变化。
        let pending = session.lifecycle_begin(lifecycle::Call::Focus(focused));
        // Focus-out is where a host's field ends, so selections counted in it are handed to the process's statistics batch here rather than waiting for a batch to fill, and `flush_typing_statistics` writes that batch right after. The same goes for the engine's delayed context learning: the iOS keyboard extension and the Android IME process can be killed without ever destroying their sessions, and the engine no longer writes that queue at process exit.
        if !focused {
            session.flush_selections();
            flush_typing_statistics(FLUSHES_TYPING_STATISTICS_INLINE_ON_FOCUS_OUT);
            msime_engine::flush_personal_learning();
        } else {
            session.refresh_plugin_tables();
            // 脚本插件：在本次 focus 的任何事件之前丢弃上一个字段的实例，授权文件或包变了就重读。
            session.lifecycle_focus_in();
            // 设置应用可能刚下载好日文、粤拼、注音、笔画的资源包或落定重排模型；Engine 重建和模型加载在随后的 `complete_transition` 里空闲时进行。
            session.refresh_resource_packs();
            // The settings page may have imported the pack in use again, or removed it, since this session last looked.
            session.sound.restamp();
        }
        let result = session.runtime.focus(focused).map_err(|e| e.to_string());
        // 失焦先取消组字再刷新，刷新失败时组字已经没了：照样发 end、hidden 和 focus_out。
        let result = session.lifecycle_on_error(pending, result)?;
        let facts = session.runtime.facts();
        let mut result = session.complete_transition(result);
        // 失焦上屏不可改写、也不出声；focus_in/focus_out 排在组字结束之后，所以 focus_in 也在上面的重读之后。focus 的回复不带 `feedback`。
        session.lifecycle_after(pending, facts, &mut result);
        serde_json::to_value(result).map_err(|e| e.to_string())
    })
}

#[no_mangle]
pub extern "C" fn msime_client_reset_cache(handle: u64) -> *mut c_char {
    crate::plugin_tables::release_cached_tables();
    dispatch(handle, Action::ResetCache)
}

/// Releases process-level model caches without requiring a live session handle. This is for host
/// memory-pressure callbacks after an input session has already been destroyed.
#[no_mangle]
pub extern "C" fn msime_client_release_process_cache() {
    release_japanese_dictionary_cache();
    release_idle_personal_models();
    crate::plugin_tables::release_cached_tables();
    release_gloss_caches();
}

/// 关掉调用线程缓存的候选释义词库连接和读好的自定义译文，见 `msime_engine::host::release_gloss_caches`。日文词典这类重新加载很贵的模型不动，所以宿主可以在每次键盘收起时调用。
#[no_mangle]
pub extern "C" fn msime_client_release_gloss_caches() {
    release_gloss_caches();
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

/// The microphones the settings page offers under 「可用录音设备」, the listing the Tauri settings shell's `list_voice_capture_devices` gives on Windows: value `[{backend:"windows", id, label}]`, `id` being `wasapi:` and the WASAPI endpoint id as four lowercase hex digits per UTF-16 unit, at most 128 entries, `[]` when no capture endpoint is available. Other targets fail with "voice capture devices unavailable": their hosts list devices their own way. Enumerates through COM: call off the UI thread. Ids and labels may identify the user's hardware: never log them.
#[no_mangle]
pub extern "C" fn msime_client_voice_capture_devices() -> *mut c_char {
    response(|| {
        #[cfg(target_os = "windows")]
        {
            Ok(Value::Array(crate::voice_capture_device_values(
                "windows",
                crate::voice_capture::capture_devices(),
            )))
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err("voice capture devices unavailable".into())
        }
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
