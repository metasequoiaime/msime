//! Keystrokes, punctuation, mode switches and candidate selection - the composing surface.
//!
//! Part of the C ABI; see the parent module for what these shims guarantee.

use crate::*;

/// Native presentation override; changes wait for the current composition to end.
#[no_mangle]
pub extern "C" fn msime_client_set_candidate_page_size(handle: u64, size: u8) -> *mut c_char {
    response(|| {
        if !(1..=9).contains(&size) {
            return Err("candidate page size must be between 1 and 9".into());
        }
        with_session(handle, |session| {
            if session.runtime.is_idle() {
                session
                    .runtime
                    .set_page_size(size)
                    .map_err(|e| e.to_string())?;
            }
            session.page_size_override = Some(size);
            let view = session.runtime.view();
            Ok(json!({"deferred": view.page_size != usize::from(size), "view": view}))
        })
    })
}

/// Override the live host punctuation mode without persisting preferences.
#[no_mangle]
pub extern "C" fn msime_client_set_chinese_punctuation(handle: u64, enabled: bool) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            let lock = session
                .punctuation_lock_override
                .unwrap_or_else(|| punctuation_lock_code(session.applied.punctuation_lock));
            session
                .runtime
                .set_chinese_punctuation_enabled(engine_chinese_punctuation(enabled, lock))
                .map_err(|e| e.to_string())?;
            session.punctuation_override = Some(enabled);
            serialized_runtime_view(session)
        })
    })
}

#[no_mangle]
pub extern "C" fn msime_client_set_paired_punctuation(handle: u64, enabled: bool) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            session
                .runtime
                .set_paired_punctuation_enabled(enabled)
                .map_err(|e| e.to_string())?;
            session.paired_punctuation_override = Some(enabled);
            serialized_runtime_view(session)
        })
    })
}

#[no_mangle]
pub extern "C" fn msime_client_set_punctuation_lock(handle: u64, lock: u8) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            session
                .runtime
                .set_punctuation_lock(lock)
                .map_err(|e| e.to_string())?;
            session.punctuation_lock_override = Some(lock);
            let engine_enabled = session.live_engine_chinese_punctuation();
            session
                .runtime
                .set_chinese_punctuation_enabled(engine_enabled)
                .map_err(|e| e.to_string())?;
            serialized_runtime_view(session)
        })
    })
}

#[no_mangle]
pub extern "C" fn msime_client_set_english_mode(handle: u64, enabled: bool) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            session
                .runtime
                .set_dedicated_english(enabled)
                .map_err(|e| e.to_string())?;
            session.english_mode = enabled;
            serialized_runtime_view(session)
        })
    })
}

/// 标出隐私会话：隐私模式或不允许学习的输入框。只影响打字统计（选词位置和上屏效率不计），学习仍由偏好里的 `learning` 决定。
#[no_mangle]
pub extern "C" fn msime_client_set_private_session(handle: u64, enabled: bool) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            session.statistics_private = enabled;
            Ok(Value::Bool(enabled))
        })
    })
}

/// 在组字空闲后开启引擎负责的九键数字处理：全拼九宫格，或注音九键。
#[no_mangle]
pub extern "C" fn msime_client_set_nine_key_mode(handle: u64, enabled: bool) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            session
                .runtime
                .set_nine_key_enabled(enabled)
                .map_err(|e| e.to_string())?;
            session.nine_key_override = Some(enabled);
            serialized_runtime_view(session)
        })
    })
}

#[no_mangle]
pub extern "C" fn msime_client_set_character_width(handle: u64, fullwidth: bool) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            session.runtime.set_character_width(if fullwidth {
                CharacterWidth::Fullwidth
            } else {
                CharacterWidth::Halfwidth
            });
            serialized_runtime_view(session)
        })
    })
}

#[no_mangle]
pub extern "C" fn msime_client_character(handle: u64, ascii: u8, shift: bool) -> *mut c_char {
    dispatch(
        handle,
        Action::Character {
            value: ascii,
            shift,
        },
    )
}

#[no_mangle]
pub extern "C" fn msime_client_command(handle: u64, command: u32) -> *mut c_char {
    let action = match command {
        0 => Action::Command(Command::Backspace),
        1 => Action::SelectHighlighted,
        2 => Action::Command(Command::CommitRaw),
        3 => Action::Command(Command::Cancel),
        4 => Action::Command(Command::MoveLeft),
        5 => Action::Command(Command::MoveRight),
        6 => Action::Command(Command::MoveHome),
        7 => Action::Command(Command::MoveEnd),
        8 => Action::Command(Command::DeleteForward),
        9 => Action::Finish,
        10 => Action::Command(Command::CycleKanaVariant),
        11 => Action::Command(Command::CommitReading),
        12 => Action::SegmentBackspace,
        13 => Action::SegmentMoveLeft,
        14 => Action::SegmentMoveRight,
        15 => Action::Command(Command::CommitRawWithoutLearning),
        16 => Action::Command(Command::ConvertHanja),
        100 => Action::NextPage,
        101 => Action::PreviousPage,
        102 => Action::NextCandidate,
        103 => Action::PreviousCandidate,
        104 => Action::FirstCandidate,
        105 => Action::LastCandidate,
        _ => return response(|| Err("unknown input command".into())),
    };
    dispatch(handle, action)
}

/// Explicit native punctuation route, even when a local mode consumes characters.
#[no_mangle]
pub extern "C" fn msime_client_punctuation(handle: u64, ascii: u8) -> *mut c_char {
    dispatch(handle, Action::Punctuation(ascii))
}

/// Resolve punctuation using the platform editor's immediately preceding
/// Unicode scalar. Zero means that no preceding scalar is available. Only the
/// scalar value crosses the host boundary; document text is never retained.
#[no_mangle]
pub extern "C" fn msime_client_punctuation_with_context(
    handle: u64,
    ascii: u8,
    preceding: u32,
) -> *mut c_char {
    if !ascii.is_ascii_punctuation() {
        return response(|| Err("invalid punctuation".into()));
    }
    let preceding = if preceding == 0 {
        None
    } else {
        match char::from_u32(preceding) {
            Some(value) => Some(value),
            None => return response(|| Err("invalid preceding character".into())),
        }
    };
    let action = SESSIONS.with(|sessions| {
        let sessions = sessions
            .try_borrow()
            .map_err(|_| "reentrant host call".to_owned())?;
        let session = sessions
            .get(&handle)
            .ok_or_else(|| "unknown session or wrong thread".to_owned())?;
        let lock = match session.punctuation_lock_override {
            Some(1) => msime_client_core::preferences::PunctuationLock::Chinese,
            Some(2) => msime_client_core::preferences::PunctuationLock::English,
            Some(_) => msime_client_core::preferences::PunctuationLock::Follow,
            None => session.applied.punctuation_lock,
        };
        let route = punctuation_route(PunctuationContext {
            character: ascii,
            preceding,
            host_context_available: session
                .runtime
                .punctuation_host_context_available(session.english_mode),
            has_composition: !session.runtime.is_idle(),
            chinese_punctuation: session
                .punctuation_override
                .unwrap_or(session.applied.chinese_punctuation),
            smart_punctuation: session.applied.smart_punctuation,
            direct_digit: session.applied.smart_punctuation_direct_digit,
            direct_letter: session.applied.smart_punctuation_direct_letter,
            lock,
        });
        Ok(match route {
            PunctuationRoute::Engine => Action::Punctuation(ascii),
            PunctuationRoute::Ascii => Action::PunctuationAscii(ascii),
        })
    });
    match action {
        Ok(action) => dispatch(handle, action),
        Err(error) => response(|| Err(error)),
    }
}

/// Whether the commit just made arms either smart-punctuation follow-up gesture.
///
/// Both answers are pure, but the switches that gate them live in the applied preferences, so the
/// host asks the session rather than keeping a second copy of them. The host holds the returned
/// snapshots: they belong to its editor, not to Engine, and a session rebuilt while the keyboard
/// was away must not carry a gesture across the gap.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_smart_punctuation_arm(
    handle: u64,
    request: *const u8,
    length: usize,
) -> *mut c_char {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        ascii: u8,
        commit: String,
        timestamp_ms: u64,
        editor_generation: u64,
        auto_closed_pair: bool,
    }
    response(|| {
        if request.is_null() || length > 4096 {
            return Err("invalid smart punctuation request".into());
        }
        // SAFETY: guaranteed by the documented caller contract; size checked above.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let value: Request =
            serde_json::from_slice(bytes).map_err(|_| "invalid smart punctuation request")?;
        SESSIONS.with(|sessions| {
            let sessions = sessions
                .try_borrow()
                .map_err(|_| "reentrant host call".to_owned())?;
            let session = sessions
                .get(&handle)
                .ok_or_else(|| "unknown session or wrong thread".to_owned())?;
            let smart = session.applied.smart_punctuation;
            // 重复按键手势把 ASCII 标点换成中文标点。韩文、越南文和藏文只写 ASCII 标点，注音的标点键用来拼注音符号，所以这几个方案里从不启用；日文照旧启用。
            let smart_scheme = !matches!(
                SchemeType::from_u8(session.runtime.scheme()),
                Some(
                    SchemeType::Korean
                        | SchemeType::Zhuyin
                        | SchemeType::Vietnamese
                        | SchemeType::Tibetan
                )
            );
            let repeat = msime_client_core::punctuation::arm_repeat(
                value.ascii,
                &value.commit,
                value.timestamp_ms,
                value.editor_generation,
            )
            .filter(|_| smart && session.applied.smart_punctuation_repeat && smart_scheme);
            let space = msime_client_core::punctuation::arm_space_convert(
                &value.commit,
                value.auto_closed_pair,
                smart,
                session.applied.smart_punctuation_space_convert,
                value.editor_generation,
            );
            Ok(json!({
                "repeat": repeat.map(|armed| json!({
                    "ascii": armed.ascii,
                    "committed": armed.committed.to_string(),
                    "timestamp_ms": armed.timestamp_ms,
                    "editor_generation": armed.editor_generation,
                })),
                "space": space.map(|armed| json!({
                    "chinese": armed.chinese.to_string(),
                    "ascii": armed.ascii,
                    "editor_generation": armed.editor_generation,
                })),
            }))
        })
    })
}

/// What a key press should do, given what the host has armed.
///
/// `preceding` is what the editor actually holds before the caret, read at the moment of the
/// press. Both decisions re-read it and decline when it disagrees with the arming, so a stale
/// snapshot can never rewrite the wrong character.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_smart_punctuation_decide(
    handle: u64,
    request: *const u8,
    length: usize,
) -> *mut c_char {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ArmedRepeat {
        ascii: u8,
        committed: String,
        timestamp_ms: u64,
        editor_generation: u64,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ArmedSpace {
        chinese: String,
        ascii: u8,
        editor_generation: u64,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        character: u8,
        preceding: Option<String>,
        timestamp_ms: u64,
        editor_generation: u64,
        repeat: Option<ArmedRepeat>,
        space: Option<ArmedSpace>,
    }
    response(|| {
        if request.is_null() || length > 4096 {
            return Err("invalid smart punctuation request".into());
        }
        // SAFETY: guaranteed by the documented caller contract; size checked above.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let value: Request =
            serde_json::from_slice(bytes).map_err(|_| "invalid smart punctuation request")?;
        let single = |text: &str| -> Result<char, String> {
            let mut characters = text.chars();
            let first = characters.next().ok_or_else(|| "empty scalar".to_owned())?;
            if characters.next().is_some() {
                return Err("expected a single scalar".into());
            }
            Ok(first)
        };
        let preceding = match value.preceding.as_deref() {
            None | Some("") => None,
            Some(text) => Some(single(text)?),
        };
        let repeat_snapshot = match value.repeat {
            None => None,
            Some(armed) => Some(msime_client_core::punctuation::RepeatSnapshot {
                ascii: armed.ascii,
                committed: single(&armed.committed)?,
                timestamp_ms: armed.timestamp_ms,
                editor_generation: armed.editor_generation,
            }),
        };
        let space_snapshot = match value.space {
            None => None,
            Some(armed) => Some(msime_client_core::punctuation::SpaceConvertSnapshot {
                chinese: single(&armed.chinese)?,
                ascii: armed.ascii,
                editor_generation: armed.editor_generation,
            }),
        };
        SESSIONS.with(|sessions| {
            let sessions = sessions
                .try_borrow()
                .map_err(|_| "reentrant host call".to_owned())?;
            let session = sessions
                .get(&handle)
                .ok_or_else(|| "unknown session or wrong thread".to_owned())?;
            let candidate_count = repeat_snapshot
                .as_ref()
                .map(|_| session.runtime.candidate_page_len())
                .unwrap_or(0);
            let replace = msime_client_core::punctuation::should_replace_repeat(
                repeat_snapshot,
                msime_client_core::punctuation::RepeatContext {
                    ascii: value.character,
                    preceding,
                    timestamp_ms: value.timestamp_ms,
                    editor_generation: value.editor_generation,
                    smart_punctuation: session.applied.smart_punctuation,
                    repeat_enabled: session.applied.smart_punctuation_repeat,
                    has_composition: !session.runtime.is_idle(),
                    candidate_count,
                },
            );
            let space = msime_client_core::punctuation::decide_space_convert(
                space_snapshot,
                value.character,
                preceding,
                !session.runtime.is_idle(),
                value.editor_generation,
            );
            Ok(json!({
                "replace_with": replace.map(|mark| mark.to_string()),
                "space_ascii": space,
            }))
        })
    })
}

/// Notify Engine that a host-emitted paired closing mark completed the opening.
#[no_mangle]
pub extern "C" fn msime_client_balance_paired_punctuation_after_auto_close(
    handle: u64,
    opening: u8,
) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            session
                .runtime
                .balance_paired_punctuation_after_auto_close(opening)
                .map_err(|e| e.to_string())?;
            serialized_runtime_view(session)
        })
    })
}

/// Finish the highlighted composition and append a literal ASCII punctuation
/// mark. This is kept separate from Engine punctuation so a platform host can
/// apply its own surrounding-text policy without changing the shared table.
#[no_mangle]
pub extern "C" fn msime_client_punctuation_ascii(handle: u64, ascii: u8) -> *mut c_char {
    dispatch(handle, Action::PunctuationAscii(ascii))
}

/// Re-rank the visible candidates with the settled model, after the host's typing pause elapses.
///
/// The host owns the clock. It already runs a settle timer for cloud candidates, and it is the
/// only side that knows whether a keystroke arrived while this was being decided — the runtime
/// would have to guess. Call it when the composition has been unchanged for the pause, and not
/// while keys are still arriving.
///
/// Answers `{"moved": false}` when the order did not change, or `{"moved": true, "view": ...}`
/// after a reorder. The false case is common and lets hosts leave the candidate window alone
/// without serializing a view they will discard.
#[no_mangle]
pub extern "C" fn msime_client_rerank_settled(handle: u64) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            let moved = session.runtime.rerank_settled();
            if moved {
                let view = session.runtime.view();
                Ok(serde_json::json!({"moved": true, "view": view}))
            } else {
                Ok(serde_json::json!({"moved": false}))
            }
        })
    })
}

#[no_mangle]
pub extern "C" fn msime_client_select(handle: u64, generation: u64, index: usize) -> *mut c_char {
    dispatch(
        handle,
        Action::Select(CandidateId {
            session: handle,
            generation,
            index,
        }),
    )
}

/// Select any candidate returned by `msime_client_all_candidates` for the exact
/// session generation. Regular `msime_client_select` remains page-bounded.
#[no_mangle]
pub extern "C" fn msime_client_select_any_candidate(
    handle: u64,
    generation: u64,
    index: usize,
) -> *mut c_char {
    dispatch(
        handle,
        Action::SelectAnyCandidate(CandidateId {
            session: handle,
            generation,
            index,
        }),
    )
}

#[no_mangle]
pub extern "C" fn msime_client_pin_candidate(
    handle: u64,
    generation: u64,
    index: usize,
) -> *mut c_char {
    dispatch(
        handle,
        Action::PinCandidate(CandidateId {
            session: handle,
            generation,
            index,
        }),
    )
}

#[no_mangle]
pub extern "C" fn msime_client_remove_candidate(
    handle: u64,
    generation: u64,
    index: usize,
) -> *mut c_char {
    dispatch(
        handle,
        Action::RemoveCandidate(CandidateId {
            session: handle,
            generation,
            index,
        }),
    )
}

#[no_mangle]
pub extern "C" fn msime_client_fix_candidate_position(
    handle: u64,
    generation: u64,
    index: usize,
    position: u8,
) -> *mut c_char {
    if !(1..=5).contains(&position) {
        return response(|| Err("candidate position must be between 1 and 5".into()));
    }
    dispatch(
        handle,
        Action::FixCandidatePosition(
            CandidateId {
                session: handle,
                generation,
                index,
            },
            position,
        ),
    )
}

#[no_mangle]
pub extern "C" fn msime_client_clear_candidate_position(
    handle: u64,
    generation: u64,
    index: usize,
) -> *mut c_char {
    dispatch(
        handle,
        Action::ClearCandidatePosition(CandidateId {
            session: handle,
            generation,
            index,
        }),
    )
}

/// Select one spelling from View.nine_key_spellings for the exact view generation.
#[no_mangle]
pub extern "C" fn msime_client_choose_nine_key_spelling(
    handle: u64,
    generation: u64,
    index: usize,
) -> *mut c_char {
    dispatch(
        handle,
        Action::ChooseNineKeySpelling(NineKeySpellingId {
            session: handle,
            generation,
            index,
        }),
    )
}

/// Copy every cached Engine candidate only when a host opens an expanded panel.
#[no_mangle]
pub extern "C" fn msime_client_all_candidates(handle: u64) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            serde_json::to_value(session.runtime.all_candidates()).map_err(|e| e.to_string())
        })
    })
}

/// Return bounded, lower-case English completions for the word immediately before the cursor.
/// This query is read-only and does not touch the Engine session's composition state.
///
/// # Safety
/// `prefix` must point to `prefix_length` readable UTF-8 bytes. The buffer is not retained.
#[no_mangle]
pub unsafe extern "C" fn msime_client_english_completions(
    handle: u64,
    prefix: *const u8,
    prefix_length: usize,
    limit: usize,
) -> *mut c_char {
    response(|| {
        if prefix.is_null() || !(1..=64).contains(&prefix_length) || !(1..=32).contains(&limit) {
            return Err("invalid English completion buffer".into());
        }
        let bytes = unsafe { std::slice::from_raw_parts(prefix, prefix_length) };
        let prefix = std::str::from_utf8(bytes).map_err(|_| "invalid English completion prefix")?;
        if !msime_client_core::is_ascii_alphabetic(prefix) {
            return Err("invalid English completion prefix".into());
        }
        with_session(handle, |session| {
            let words = msime_engine::host::english_completions(
                &session.options.dictionaries,
                prefix,
                limit,
            )
            .map_err(|error| error.to_string())?;
            Ok(json!({"completions": words}))
        })
    })
}
