//! Key sounds, commit sounds, background music, sound and music pack files, and the 插件 page's pack store for native hosts.
//!
//! Part of the C ABI; see the parent module for what these shims guarantee. The three sound calls sit on the key path, so they answer a plain bool instead of a JSON document the host would have to free: whether a request was queued. They never block, decode or touch the disk; `key_sound` says why.

use crate::*;
use key_sound::{KeyClass, PluginRoots, SessionSound};
use msime_client_core::plugins::mentions::{MentionEntry, MentionStore};
use msime_client_core::plugins::{self, PluginFailure};

/// Run `action` on the session's sound settings. False for an unknown handle, a wrong thread or a reentrant call, and after a panic, which must not cross the C boundary.
fn with_sound(handle: u64, action: impl FnOnce(&SessionSound) -> bool) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        SESSIONS.with(|sessions| {
            let Ok(sessions) = sessions.try_borrow() else {
                return false;
            };
            sessions
                .get(&handle)
                .is_some_and(|session| action(&session.sound))
        })
    }))
    .unwrap_or(false)
}

/// Queue the sound of one key press. `key_class` is 0 for any other key, 1 space, 2 enter, 3 backspace; anything else queues nothing.
#[no_mangle]
pub extern "C" fn msime_client_key_sound(handle: u64, key_class: u32) -> bool {
    let Some(class) = KeyClass::from_code(key_class) else {
        return false;
    };
    with_sound(handle, |sound| key_sound::key(sound, class))
}

/// The typing effect of one key or commit, packed into one integer; `include/msime_client.h` documents the event codes and the bits. 0 for an unknown handle, a wrong thread or a reentrant call, and after a panic, which must not cross the C boundary.
#[no_mangle]
pub extern "C" fn msime_client_typing_effect(handle: u64, event: u32) -> u32 {
    catch_unwind(AssertUnwindSafe(|| {
        SESSIONS.with(|sessions| {
            let Ok(sessions) = sessions.try_borrow() else {
                return 0;
            };
            sessions.get(&handle).map_or(0, |session| {
                key_sound::typing_effect(&session.sound, event, std::time::Instant::now())
            })
        })
    }))
    .unwrap_or(0)
}

/// The session's resolved typing effect: the selected effect pack, or the preferences' style and intensity without one, as `{pack, issue, style, intensity, colors, duration_ms, particles, combo_counter}`. Read it when the preferences change or a field gains focus, not per key: `msime_client_typing_effect` stays the key-path call.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub extern "C" fn msime_client_typing_effect_settings(handle: u64) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            Ok(key_sound::effect_settings(&session.sound))
        })
    })
}

/// Queue the sound of a commit: the key pack's commit sample, the melody's next note when it advances on commits, or both.
#[no_mangle]
pub extern "C" fn msime_client_commit_sound(handle: u64) -> bool {
    with_sound(handle, key_sound::commit)
}

/// Whether background music may play now: true while the input method is active in a field that is not a secure one, false otherwise.
#[no_mangle]
pub extern "C" fn msime_client_music_set_active(handle: u64, active: bool) -> bool {
    with_sound(handle, |sound| key_sound::music_active(sound, active))
}

/// The request both pack calls take: the state root holding `plugins/`, the bundle's built-in sound packs, and the pack's id.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PackRequest {
    state_root: Option<String>,
    sound_packs: Option<String>,
    pack: String,
}

/// Read a `PackRequest` and resolve its roots. `invalid` and `relative` are the call's own errors for a request it cannot read and for a path that is not absolute.
/// # Safety
/// As the calling export: `request` points to `length` readable bytes, or is null.
unsafe fn pack_request(
    request: *const u8,
    length: usize,
    invalid: &'static str,
    relative: &'static str,
) -> Result<(PluginRoots, String), String> {
    // SAFETY: guaranteed by the documented caller contract; the length is bounded first.
    let request: PackRequest = unsafe {
        with_bounded_bytes(request, length, 65_536, invalid, |bytes| {
            serde_json::from_slice(bytes).map_err(|_| invalid.to_owned())
        })
    }?;
    let absolute = |path: &Option<String>| path.as_deref().is_none_or(absolute_path);
    if !absolute(&request.state_root) || !absolute(&request.sound_packs) {
        return Err(relative.into());
    }
    let roots = PluginRoots::new(
        request.state_root.as_deref(),
        request.sound_packs.as_deref(),
        "",
    );
    Ok((roots, request.pack))
}

/// The validated files of one sound pack, for a host that plays packs itself.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_key_sound_pack(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: forwarded from this function's own contract.
        let (roots, pack) = unsafe {
            pack_request(
                request,
                length,
                "invalid sound pack request",
                "sound pack paths must be absolute",
            )
        }?;
        key_sound::pack_files(&roots, &pack)
    })
}

/// The validated tracks of one music pack, for a host that streams music itself.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_music_pack(request: *const u8, length: usize) -> *mut c_char {
    response(|| {
        // SAFETY: forwarded from this function's own contract.
        let (roots, pack) = unsafe {
            pack_request(
                request,
                length,
                "invalid music pack request",
                "music pack paths must be absolute",
            )
        }?;
        key_sound::music_pack_files(&roots, &pack)
    })
}

/// Largest `msime_client_plugins` request: a full name list at its document bound, with room for JSON escaping and the two paths.
const MAX_PLUGINS_REQUEST_BYTES: usize = 2 * 1024 * 1024;

/// The 插件 page's pack store and @ name list, for a settings host other than the desktop shell. Every rule and every failure code is client-core's, the same ones the desktop shell answers its page with.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_plugins(request: *const u8, length: usize) -> *mut c_char {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        state_root: String,
        sound_packs: Option<String>,
        action: PluginAction,
    }
    #[derive(Deserialize)]
    #[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
    enum PluginAction {
        Catalog,
        Import { source: String },
        Remove { kind: String, id: String },
        LoadMentions,
        SaveMentions { entries: Vec<MentionEntry> },
    }
    plugin_response(|| {
        let invalid = || PluginFailure::code("invalid");
        // SAFETY: guaranteed by the documented caller contract; the length is bounded first.
        let request: Request = unsafe {
            with_bounded_bytes(request, length, MAX_PLUGINS_REQUEST_BYTES, "", |bytes| {
                serde_json::from_slice(bytes).map_err(|_| String::new())
            })
        }
        .map_err(|_| invalid())?;
        if !absolute_path(&request.state_root)
            || !request.sound_packs.as_deref().is_none_or(absolute_path)
        {
            return Err(invalid());
        }
        let roots = PluginRoots::new(
            Some(&request.state_root),
            request.sound_packs.as_deref(),
            "",
        );
        let root = roots.installed.ok_or_else(invalid)?;
        let value = match request.action {
            PluginAction::Catalog => {
                serde_json::to_value(plugins::scan(&root, roots.builtin_sounds.as_deref()))
            }
            PluginAction::Import { source } => {
                if !absolute_path(&source) {
                    return Err(invalid());
                }
                serde_json::to_value(plugins::import(Path::new(&source), &root)?)
            }
            PluginAction::Remove { kind, id } => {
                plugins::remove_named(&root, &kind, &id)?;
                Ok(Value::Null)
            }
            PluginAction::LoadMentions => serde_json::to_value(MentionStore::new(root).load()?),
            PluginAction::SaveMentions { entries } => {
                MentionStore::new(root).save(&entries)?;
                Ok(Value::Null)
            }
        };
        value.map_err(|_| PluginFailure::code("storage"))
    })
}

/// `response` with the page's failure record: `error` is the code, and `detail`, when client-core gave one, the rule that was broken.
fn plugin_response(operation: impl FnOnce() -> Result<Value, PluginFailure>) -> *mut c_char {
    let value = match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(value)) => json!({ "ok": true, "value": value }),
        Ok(Err(PluginFailure {
            code,
            detail: Some(detail),
        })) => json!({ "ok": false, "error": code, "detail": detail }),
        Ok(Err(PluginFailure { code, detail: None })) => json!({ "ok": false, "error": code }),
        Err(_) => json!({ "ok": false, "error": "internal runtime failure" }),
    };
    // JSON escapes embedded NUL bytes, so this cannot contain an interior NUL.
    CString::new(value.to_string())
        .expect("JSON contains no NUL")
        .into_raw()
}
