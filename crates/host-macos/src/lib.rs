//! macOS platform operations for shared clients; no desktop or Engine dependency.

// The workspace denies unsafe code; the AppKit and Objective-C entry points this wraps are C functions.
// The exemption is stated here rather than left implicit by opting out of
// the workspace lint table, which would also silently drop every other lint
// the workspace adds later.
#![allow(unsafe_code)]

use msime_client_core::panels::KeyboardInputRequest;
#[cfg(target_os = "macos")]
pub mod cloud_clipboard;
#[cfg(target_os = "macos")]
pub mod cloud_dictionary;
#[cfg(target_os = "macos")]
pub mod panel_session;

#[cfg(target_os = "macos")]
pub(crate) const SESSION_JSON_MAX_BYTES: usize = 2048;

#[cfg(target_os = "macos")]
pub(crate) fn valid_session_socket_path(path: &str) -> bool {
    path.len() < 104 && std::path::Path::new(path).is_absolute() && !path.contains('\0')
}

/// Keeps WebKit detached while a desktop adapter changes a window's class.
/// Main-thread-only; dropping restores the view and its window observations.
#[cfg(target_os = "macos")]
pub struct DetachedWindowContent {
    token: usize,
    _main_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

#[cfg(target_os = "macos")]
pub fn detach_window_content(window_address: usize) -> Option<DetachedWindowContent> {
    unsafe extern "C" {
        fn msime_macos_detach_window_content(address: usize) -> usize;
    }
    // SAFETY: native code resolves this address against NSApp's live windows;
    // arbitrary caller addresses are never dereferenced.
    let token = unsafe { msime_macos_detach_window_content(window_address) };
    (token != 0).then_some(DetachedWindowContent {
        token,
        _main_thread: std::marker::PhantomData,
    })
}

#[cfg(target_os = "macos")]
impl Drop for DetachedWindowContent {
    fn drop(&mut self) {
        unsafe extern "C" {
            fn msime_macos_restore_window_content(token: usize);
        }
        // SAFETY: private, uniquely-owned native token; !Send keeps drop on main.
        unsafe { msime_macos_restore_window_content(self.token) };
    }
}

/// A process identity pinned by PID and launch time. The same identity is used
/// for startup focus restoration and for non-activating panel key delivery.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
pub struct LaunchTarget {
    pid: i32,
    launched: f64,
}

#[cfg(target_os = "macos")]
pub fn capture_launch_target() -> Option<LaunchTarget> {
    unsafe extern "C" {
        fn msime_macos_capture_launch_target(launched: *mut f64) -> i32;
    }
    let mut launched = 0.0;
    // SAFETY: native code writes one double to this valid local output pointer.
    let pid = unsafe { msime_macos_capture_launch_target(&mut launched) };
    (pid > 0 && launched > 0.0).then_some(LaunchTarget { pid, launched })
}

/// Undo only this shell's startup activation; never take focus from a third app.
#[cfg(target_os = "macos")]
pub fn restore_launch_target(target: LaunchTarget) -> bool {
    unsafe extern "C" {
        fn msime_macos_restore_launch_target(pid: i32, launched: f64) -> bool;
    }
    // SAFETY: scalar ABI; native side validates thread, process identity and focus.
    unsafe { msime_macos_restore_launch_target(target.pid, target.launched) }
}

/// Enumerate input-capable CoreAudio devices using their stable UIDs. The
/// callback runs synchronously on the caller's thread and never opens a
/// device, so this is safe to use from a Tauri blocking task.
#[cfg(target_os = "macos")]
pub fn voice_capture_devices() -> Vec<(String, String)> {
    use std::ffi::{c_char, c_void, CStr};

    extern "C" fn collect(
        uid: *const c_char,
        name: *const c_char,
        _is_default: bool,
        context: *mut c_void,
    ) {
        if uid.is_null() || name.is_null() || context.is_null() {
            return;
        }
        let (uid, name) = unsafe { (CStr::from_ptr(uid), CStr::from_ptr(name)) };
        let (Ok(uid), Ok(name)) = (uid.to_str(), name.to_str()) else {
            return;
        };
        if uid.is_empty()
            || name.is_empty()
            || uid.len() > 512
            || name.len() > 512
            || msime_client_core::has_disallowed_control_with_options(uid, false)
            || msime_client_core::has_disallowed_control_with_options(name, false)
        {
            return;
        }
        // SAFETY: the native enumerator invokes this callback synchronously
        // with the exact Vec pointer passed below and never stores it.
        let devices = unsafe { &mut *(context.cast::<Vec<(String, String)>>()) };
        devices.push((uid.to_owned(), name.to_owned()));
    }

    unsafe extern "C" {
        fn msime_macos_list_voice_capture_devices(
            callback: extern "C" fn(*const c_char, *const c_char, bool, *mut c_void),
            context: *mut c_void,
        );
    }

    let mut devices = Vec::new();
    // SAFETY: `collect` has the ABI and lifetime required by the native
    // callback; the context points to a live Vec for the duration of the call.
    unsafe {
        msime_macos_list_voice_capture_devices(
            collect,
            (&mut devices as *mut Vec<(String, String)>).cast::<c_void>(),
        );
    }
    devices
}

#[cfg(target_os = "macos")]
pub fn uninstall_input_source(
    bundle: &std::path::Path,
    user_data: &std::path::Path,
    remove_user_data: bool,
) -> Result<(), &'static str> {
    use std::ffi::CString;
    let home = std::env::var_os("HOME").ok_or("home unavailable")?;
    if !std::path::PathBuf::from(&home).is_absolute()
        || !bundle.is_absolute()
        || !user_data.is_absolute()
    {
        return Err("invalid uninstall path");
    }
    let bundle =
        CString::new(bundle.to_string_lossy().as_bytes()).map_err(|_| "invalid uninstall path")?;
    let user_data = CString::new(user_data.to_string_lossy().as_bytes())
        .map_err(|_| "invalid uninstall path")?;
    // The preferences the input method leaves behind are keyed on its bundle identifier; removing any other domain leaves the real one on disk and reports a clean uninstall.
    let domain = CString::new("app.msime.inputmethod.MetasequoiaIME").unwrap();
    unsafe extern "C" {
        fn msime_macos_uninstall_input_source(
            bundle: *const std::ffi::c_char,
            user_data: *const std::ffi::c_char,
            preferences_domain: *const std::ffi::c_char,
            remove_user_data: bool,
        ) -> bool;
    }
    let ok = unsafe {
        msime_macos_uninstall_input_source(
            bundle.as_ptr(),
            user_data.as_ptr(),
            domain.as_ptr(),
            remove_user_data,
        )
    };
    ok.then_some(()).ok_or("uninstall failed")
}

/// Wake the separate IMK process so it releases its dictionary sessions now, after the caller has written the quiesce lease beside the dictionary lock. IMK only lets go while that lease is live, so a stray notification drops nothing. The notification carries no input, credentials, or paths.
#[cfg(target_os = "macos")]
pub fn quiesce_input_sessions() {
    unsafe extern "C" {
        fn msime_macos_quiesce_input_sessions();
    }
    // SAFETY: the native function has no arguments and retains no state.
    unsafe { msime_macos_quiesce_input_sessions() };
}

/// Tell the separate IMK process that the private aggregate-statistics opt-in changed.
///
/// Only the boolean crosses the process boundary. No committed text, path, or statistic is placed
/// in the distributed notification.
#[cfg(target_os = "macos")]
pub fn notify_typing_statistics_enabled(enabled: bool) {
    unsafe extern "C" {
        fn msime_macos_notify_typing_statistics_enabled(enabled: bool);
    }
    // SAFETY: scalar ABI; the native function posts one per-user notification and retains no
    // caller-owned state.
    unsafe { msime_macos_notify_typing_statistics_enabled(enabled) };
}

/// Read the account session shared with the Swift backend Keychain store.
#[cfg(target_os = "macos")]
pub fn account_load() -> Result<Option<Vec<u8>>, &'static str> {
    let mut bytes = vec![0_u8; 1024 * 1024];
    let mut length = 0_usize;
    unsafe extern "C" {
        fn msime_macos_account_load(buffer: *mut u8, capacity: usize, length: *mut usize) -> i32;
    }
    let status = unsafe { msime_macos_account_load(bytes.as_mut_ptr(), bytes.len(), &mut length) };
    if status < 0 || length > bytes.len() {
        return Err("account keychain unavailable");
    }
    Ok((status == 1).then(|| {
        bytes.truncate(length);
        bytes
    }))
}

#[cfg(target_os = "macos")]
pub fn account_save(bytes: &[u8]) -> Result<(), &'static str> {
    if bytes.is_empty() || bytes.len() > 1024 * 1024 {
        return Err("account session too large");
    }
    unsafe extern "C" {
        fn msime_macos_account_save(bytes: *const u8, length: usize) -> bool;
    }
    unsafe { msime_macos_account_save(bytes.as_ptr(), bytes.len()) }
        .then_some(())
        .ok_or("account keychain unavailable")
}

#[cfg(target_os = "macos")]
pub fn account_clear() -> Result<(), &'static str> {
    unsafe extern "C" {
        fn msime_macos_account_clear() -> bool;
    }
    unsafe { msime_macos_account_clear() }
        .then_some(())
        .ok_or("account keychain unavailable")
}

#[cfg(not(target_os = "macos"))]
pub fn account_load() -> Result<Option<Vec<u8>>, &'static str> {
    Ok(None)
}

#[cfg(not(target_os = "macos"))]
pub fn account_save(_: &[u8]) -> Result<(), &'static str> {
    Err("account unavailable")
}

#[cfg(not(target_os = "macos"))]
pub fn account_clear() -> Result<(), &'static str> {
    Err("account unavailable")
}

#[cfg(not(target_os = "macos"))]
pub fn quiesce_input_sessions() {}

#[cfg(not(target_os = "macos"))]
pub fn uninstall_input_source(
    _: &std::path::Path,
    _: &std::path::Path,
    _: bool,
) -> Result<(), &'static str> {
    Err("uninstall unavailable")
}

#[cfg(not(target_os = "macos"))]
pub fn voice_capture_devices() -> Vec<(String, String)> {
    Vec::new()
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardSnapshot {
    pub change_count: i64,
    pub text: Option<String>,
}

#[cfg(target_os = "macos")]
pub fn clipboard_snapshot(include_text: bool) -> Option<ClipboardSnapshot> {
    let mut bytes = vec![0_u8; msime_client_core::clipboard::MAX_TEXT_BYTES];
    let mut length = 0_usize;
    let mut has_text = false;
    let mut change_count = 0_i64;
    unsafe extern "C" {
        fn msime_macos_read_clipboard(
            buffer: *mut u8,
            capacity: usize,
            read_text: bool,
            length: *mut usize,
            has_text: *mut bool,
            change_count: *mut i64,
        ) -> bool;
    }
    // SAFETY: the native function fills only the provided bounded buffer and
    // scalar outputs, and never retains any pointer after returning.
    let available = unsafe {
        msime_macos_read_clipboard(
            bytes.as_mut_ptr(),
            bytes.len(),
            include_text,
            &mut length,
            &mut has_text,
            &mut change_count,
        )
    };
    if !available || length > bytes.len() {
        return None;
    }
    let text = has_text
        .then(|| String::from_utf8(bytes[..length].to_vec()).ok())
        .flatten();
    Some(ClipboardSnapshot { change_count, text })
}

#[cfg(all(test, target_os = "macos"))]
mod clipboard_type_tests {
    use std::ffi::CString;

    unsafe extern "C" {
        /// Pure predicate `msime_macos_read_clipboard` applies to `pasteboard.types`: true only when the list carries plain text and none of the nspasteboard.org or password-manager marker types.
        fn msime_macos_clipboard_types_capturable(
            types: *const *const std::ffi::c_char,
            count: usize,
        ) -> bool;
    }

    unsafe extern "C" {
        /// Same normalization `msime_macos_read_clipboard` applies to the pasteboard string before copying it out.
        fn msime_macos_normalize_clipboard_text(
            input: *const u8,
            input_length: usize,
            buffer: *mut u8,
            capacity: usize,
            length: *mut usize,
        ) -> bool;
    }

    fn normalized(text: &str) -> Option<String> {
        let mut buffer = vec![0_u8; msime_client_core::clipboard::MAX_TEXT_BYTES];
        let mut length = 0_usize;
        // SAFETY: both buffers stay valid for the call and the native function writes at most `buffer.len()` bytes.
        let ok = unsafe {
            msime_macos_normalize_clipboard_text(
                text.as_ptr(),
                text.len(),
                buffer.as_mut_ptr(),
                buffer.len(),
                &mut length,
            )
        };
        ok.then(|| String::from_utf8(buffer[..length].to_vec()).unwrap())
    }

    #[test]
    fn over_long_copies_are_truncated_like_the_shared_history() {
        let long = "x".repeat(4001);
        assert_eq!(normalized(&long).as_deref(), Some(&long[..4000]));
        // A surrogate pair straddling the 4000-unit cut is dropped whole rather than split.
        let straddle = format!("{}\u{1F600}tail", "x".repeat(3999));
        assert_eq!(normalized(&straddle).as_deref(), Some(&straddle[..3999]));
        // Three-byte characters at the limit still fit the 12000-byte buffer.
        let wide = "\u{4E2D}".repeat(5000);
        let expected = "\u{4E2D}".repeat(4000);
        assert_eq!(normalized(&wide).as_deref(), Some(expected.as_str()));
        assert_eq!(normalized("copied\r\0\r").as_deref(), Some("copied"));
        assert_eq!(
            normalized("line\r\nnext\n").as_deref(),
            Some("line\r\nnext\n")
        );
        assert_eq!(normalized("").as_deref(), Some(""));
        for sample in [
            long.as_str(),
            straddle.as_str(),
            wide.as_str(),
            "copied\r\0\r",
        ] {
            assert_eq!(
                normalized(sample).unwrap(),
                msime_client_core::clipboard::normalize_text(sample)
            );
        }
    }

    fn capturable(types: &[&str]) -> bool {
        let owned: Vec<CString> = types.iter().map(|t| CString::new(*t).unwrap()).collect();
        let pointers: Vec<*const std::ffi::c_char> = owned.iter().map(|t| t.as_ptr()).collect();
        // SAFETY: the pointers stay valid for the call and the native predicate only reads them.
        unsafe { msime_macos_clipboard_types_capturable(pointers.as_ptr(), pointers.len()) }
    }

    #[test]
    fn marker_and_password_manager_types_are_not_capturable() {
        const STRING: &str = "public.utf8-plain-text";
        assert!(capturable(&[STRING]));
        assert!(capturable(&["public.html", STRING]));
        for marker in [
            "org.nspasteboard.ConcealedType",
            "com.agilebits.onepassword",
            "org.nspasteboard.TransientType",
            "org.nspasteboard.AutoGeneratedType",
            "de.petermaurer.TransientPasteboardType",
            "com.typeit4me.clipping",
            "Pasteboard generator type",
        ] {
            assert!(!capturable(&[STRING, marker]), "{marker}");
            assert!(!capturable(&[marker, STRING]), "{marker}");
        }
        assert!(!capturable(&["public.png"]));
        assert!(!capturable(&[]));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyStroke {
    pub code: u16,
    /// CoreGraphics Shift, Control, Option, Command flags.
    pub flags: u64,
}

/// Translate the shared virtual-key contract to macOS ANSI hardware codes.
/// Unknown PC-only keys fail closed instead of accidentally typing another key.
pub fn keyboard_stroke(request: &KeyboardInputRequest) -> Option<KeyStroke> {
    request.validate().ok()?;
    let code = match request.virtual_key {
        0x41 => 0,
        0x53 => 1,
        0x44 => 2,
        0x46 => 3,
        0x48 => 4,
        0x47 => 5,
        0x5a => 6,
        0x58 => 7,
        0x43 => 8,
        0x56 => 9,
        0x42 => 11,
        0x51 => 12,
        0x57 => 13,
        0x45 => 14,
        0x52 => 15,
        0x59 => 16,
        0x54 => 17,
        0x31 => 18,
        0x32 => 19,
        0x33 => 20,
        0x34 => 21,
        0x36 => 22,
        0x35 => 23,
        0xbb => 24,
        0x39 => 25,
        0x37 => 26,
        0xbd => 27,
        0x38 => 28,
        0x30 => 29,
        0xdd => 30,
        0x4f => 31,
        0x55 => 32,
        0xdb => 33,
        0x49 => 34,
        0x50 => 35,
        0x0d => 36,
        0x4c => 37,
        0x4a => 38,
        0xde => 39,
        0x4b => 40,
        0xba => 41,
        0xdc => 42,
        0xbc => 43,
        0xbf => 44,
        0x4e => 45,
        0x4d => 46,
        0xbe => 47,
        0x09 => 48,
        0x20 => 49,
        0xc0 => 50,
        0x08 => 51,
        0x1b => 53,
        0x6e => 65,
        0x6a => 67,
        0x6b => 69,
        0x90 => 71,
        0x6f => 75,
        0x6d => 78,
        0x60 => 82,
        0x61 => 83,
        0x62 => 84,
        0x63 => 85,
        0x64 => 86,
        0x65 => 87,
        0x66 => 88,
        0x67 => 89,
        0x68 => 91,
        0x69 => 92,
        0x74 => 96,
        0x75 => 97,
        0x76 => 98,
        0x72 => 99,
        0x77 => 100,
        0x78 => 101,
        0x7a => 103,
        0x79 => 109,
        0x7b => 111,
        0x24 => 115,
        0x21 => 116,
        0x2e => 117,
        0x73 => 118,
        0x23 => 119,
        0x71 => 120,
        0x22 => 121,
        0x70 => 122,
        0x25 => 123,
        0x27 => 124,
        0x28 => 125,
        0x26 => 126,
        _ => return None,
    };
    let sticky = request.include_sticky_modifiers;
    let flags = (u64::from(sticky && request.shift) << 17)
        | (u64::from(sticky && request.modifiers.ctrl) << 18)
        | (u64::from(sticky && request.modifiers.alt) << 19)
        | (u64::from(sticky && request.modifiers.win) << 20);
    Some(KeyStroke { code, flags })
}

/// Must be called on the AppKit main thread. No permission prompt or focus change.
#[cfg(target_os = "macos")]
pub fn send_keyboard_key(request: &KeyboardInputRequest) -> bool {
    let Some(stroke) = keyboard_stroke(request) else {
        return false;
    };
    unsafe extern "C" {
        fn msime_macos_send_keyboard_key(code: u16, flags: u64) -> bool;
    }
    // SAFETY: scalar-only ABI. Native code checks main thread, permissions,
    // external foreground ownership and event allocation before posting.
    unsafe { msime_macos_send_keyboard_key(stroke.code, stroke.flags) }
}

/// Ask the user for a file and return its path, or `None` if they cancelled.
///
/// A local speech model is identified to the recognizer by path, and a web view's file input hands back
/// contents rather than a path - so the shared settings page cannot do this itself and asks the host.
/// Must run on the main thread; the native side refuses anywhere else rather than showing a panel from a
/// worker, which AppKit does not support.
#[cfg(target_os = "macos")]
pub fn pick_file() -> Option<String> {
    unsafe extern "C" {
        fn msime_macos_pick_file() -> *mut std::os::raw::c_char;
        fn msime_macos_free_picked_path(path: *mut std::os::raw::c_char);
    }
    // SAFETY: the native side returns either null or a strdup'd UTF-8 path that this owns and frees.
    let raw = unsafe { msime_macos_pick_file() };
    if raw.is_null() {
        return None;
    }
    let path = unsafe { std::ffi::CStr::from_ptr(raw) }
        .to_str()
        .ok()
        .map(str::to_owned);
    unsafe { msime_macos_free_picked_path(raw) };
    path
}

/// Ask the user for a directory and return its absolute path, or `None` when cancelled.
/// Must run on the AppKit main thread.
#[cfg(target_os = "macos")]
pub fn pick_directory() -> Option<String> {
    unsafe extern "C" {
        fn msime_macos_pick_directory() -> *mut std::os::raw::c_char;
        fn msime_macos_free_picked_path(path: *mut std::os::raw::c_char);
    }
    // SAFETY: the native side returns either null or a strdup'd UTF-8 path that this owns and frees.
    let raw = unsafe { msime_macos_pick_directory() };
    if raw.is_null() {
        return None;
    }
    let path = unsafe { std::ffi::CStr::from_ptr(raw) }
        .to_str()
        .ok()
        .map(str::to_owned);
    unsafe { msime_macos_free_picked_path(raw) };
    path
}

/// Stop every running instance of the separate InputMethodKit bundle before its state is moved.
/// Must run on the AppKit main thread.
#[cfg(target_os = "macos")]
pub fn stop_input_method() -> bool {
    unsafe extern "C" {
        fn msime_macos_stop_input_method() -> bool;
    }
    // SAFETY: no pointers cross the boundary; native code validates the calling thread.
    unsafe { msime_macos_stop_input_method() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_core::panels::KeyboardModifiers;

    fn request(key: u16) -> KeyboardInputRequest {
        KeyboardInputRequest {
            virtual_key: key,
            shift: true,
            modifiers: KeyboardModifiers {
                ctrl: true,
                alt: true,
                win: true,
            },
            include_sticky_modifiers: true,
        }
    }

    #[test]
    fn ansi_letters_digits_punctuation_and_navigation_are_mapped() {
        for (key, code) in [
            (0x41, 0),
            (0x5a, 6),
            (0x30, 29),
            (0x31, 18),
            (0xba, 41),
            (0xde, 39),
            (0xdc, 42),
            (0x08, 51),
            (0x2e, 117),
            (0x0d, 36),
            (0x09, 48),
            (0x25, 123),
            (0x26, 126),
            (0x70, 122),
            (0x7b, 111),
            (0x90, 71),
            (0x69, 92),
        ] {
            assert_eq!(keyboard_stroke(&request(key)).unwrap().code, code);
        }
        for key in b'A'..=b'Z' {
            assert!(keyboard_stroke(&request(key.into())).is_some());
        }
        for key in 0x60..=0x69 {
            assert!(keyboard_stroke(&request(key)).is_some());
        }
        for key in 0x70..=0x7b {
            assert!(keyboard_stroke(&request(key)).is_some());
        }
    }

    #[test]
    fn modifiers_map_to_command_option_and_commit_keys_drop_sticky_state() {
        let mut key = request(0x41);
        assert_eq!(keyboard_stroke(&key).unwrap().flags, 0x1e0000);
        key.include_sticky_modifiers = false;
        assert_eq!(keyboard_stroke(&key).unwrap().flags, 0);
        key.include_sticky_modifiers = true;
        key.shift = false;
        key.modifiers.ctrl = false;
        key.modifiers.alt = false;
        assert_eq!(keyboard_stroke(&key).unwrap().flags, 1 << 20);
    }

    #[test]
    fn unsupported_pc_keys_and_invalid_contract_values_are_rejected() {
        for key in [0, 0x100, 0xffff, 0x2c, 0x91, 0x13, 0x2d] {
            assert!(keyboard_stroke(&request(key)).is_none());
        }
    }

    /// Name every account entry point without calling one: the keychain belongs to the user, and what is
    /// worth checking here is the linkage. Compiled as Objective-C++ without `extern "C"`, the bridge
    /// exported C++-mangled names that resolved nothing, and every binary reaching it - the Tauri settings
    /// app among them - failed to link. A test that merely references them fails the same way.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_account_bridge_exports_c_symbols() {
        let load: fn() -> Result<Option<Vec<u8>>, &'static str> = account_load;
        let save: fn(&[u8]) -> Result<(), &'static str> = account_save;
        let clear: fn() -> Result<(), &'static str> = account_clear;
        // Linking is the assertion. black_box keeps the three references from being optimised away
        // without letting anything run.
        std::hint::black_box((load, save, clear));
    }
}
