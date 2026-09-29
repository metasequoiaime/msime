//! Windows host capabilities for the shared panels.
//!
//! The desktop shell hosts the shared React panels and forbids unsafe code, so
//! the Win32 calls those panels need are wrapped here: remembering the window
//! that owned the caret, injecting synthetic input into it, placing a panel on
//! the work area and opening a directory in the shell.
//!
//! Input wrappers receive panel-owned text. The voice controller reads bounded
//! recognition results from the authenticated Server; it never logs them.

// The workspace denies unsafe code; the Win32 clipboard, input and window calls behind these wrappers
// are C functions.
// The exemption is stated here rather than left implicit by opting out of
// the workspace lint table, which would also silently drop every other lint
// the workspace adds later.
#![allow(unsafe_code)]
#![cfg(windows)]

use msime_client_core::{is_bounded_text, is_bounded_utf16};

pub mod ink;
pub mod voice_controller;

use std::path::Path;
use std::time::Duration;

const CLIPBOARD_HISTORY_CHANGE_EVENT: &[u16] = &[
    'L' as u16,
    'o' as u16,
    'c' as u16,
    'a' as u16,
    'l' as u16,
    '\\' as u16,
    'M' as u16,
    'S' as u16,
    'I' as u16,
    'M' as u16,
    'E' as u16,
    '.' as u16,
    'C' as u16,
    'l' as u16,
    'i' as u16,
    'e' as u16,
    'n' as u16,
    't' as u16,
    '.' as u16,
    'C' as u16,
    'l' as u16,
    'i' as u16,
    'p' as u16,
    'b' as u16,
    'o' as u16,
    'a' as u16,
    'r' as u16,
    'd' as u16,
    'H' as u16,
    'i' as u16,
    's' as u16,
    't' as u16,
    'o' as u16,
    'r' as u16,
    'y' as u16,
    'C' as u16,
    'h' as u16,
    'a' as u16,
    'n' as u16,
    'g' as u16,
    'e' as u16,
    'd' as u16,
    0,
];

mod voice_output;
pub use voice_output::{focus_external, paste_text, paste_voice_text};

/// The window that owned the caret before a panel appeared. Panels never take
/// focus, so a click still has to reach this window rather than the panel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputTarget(isize);

/// Longest text a panel may inject in one call, matching the shared contract.
pub const MAX_TEXT_BYTES: usize = 4096;

const ACCOUNT_SESSION_TARGET: &[u16] = &[
    'M' as u16, 'S' as u16, 'I' as u16, 'M' as u16, 'E' as u16, '-' as u16, 'C' as u16, 'l' as u16,
    'i' as u16, 'e' as u16, 'n' as u16, 't' as u16, '.' as u16, 'A' as u16, 'c' as u16, 'c' as u16,
    'o' as u16, 'u' as u16, 'n' as u16, 't' as u16, 'S' as u16, 'e' as u16, 's' as u16, 's' as u16,
    'i' as u16, 'o' as u16, 'n' as u16, 0,
];

const MAX_ACCOUNT_SESSION_BYTES: usize = 16 * 1024;

/// Load the desktop account session from the per-user Windows Credential
/// Manager. The shell never writes session JSON to a normal preferences file.
pub fn load_account_session() -> Result<Option<String>, ()> {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND};
    use windows_sys::Win32::Security::Credentials::{
        CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC,
    };
    let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
    let ok = unsafe {
        CredReadW(
            ACCOUNT_SESSION_TARGET.as_ptr(),
            CRED_TYPE_GENERIC,
            0,
            &mut credential,
        )
    };
    if ok == 0 {
        return if unsafe { GetLastError() } == ERROR_NOT_FOUND {
            Ok(None)
        } else {
            Err(())
        };
    }
    if credential.is_null() {
        return Err(());
    }
    let result = unsafe {
        let value = &*credential;
        if value.CredentialBlob.is_null()
            || value.CredentialBlobSize as usize > MAX_ACCOUNT_SESSION_BYTES
        {
            Err(())
        } else {
            let bytes =
                std::slice::from_raw_parts(value.CredentialBlob, value.CredentialBlobSize as usize);
            String::from_utf8(bytes.to_vec()).map_err(|_| ())
        }
    };
    unsafe { CredFree(credential.cast()) };
    result.map(Some)
}

/// Save or clear the desktop account session using Windows Credential Manager.
pub fn save_account_session(value: Option<&str>) -> Result<(), ()> {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND};
    use windows_sys::Win32::Security::Credentials::{
        CredDeleteW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
    };
    if let Some(value) = value {
        if value.is_empty() || value.len() > MAX_ACCOUNT_SESSION_BYTES {
            return Err(());
        }
        let mut bytes = value.as_bytes().to_vec();
        let mut credential = CREDENTIALW {
            Flags: 0,
            Type: CRED_TYPE_GENERIC,
            TargetName: ACCOUNT_SESSION_TARGET.as_ptr() as *mut u16,
            Comment: std::ptr::null_mut(),
            LastWritten: windows_sys::Win32::Foundation::FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            },
            CredentialBlobSize: bytes.len() as u32,
            CredentialBlob: bytes.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: std::ptr::null_mut(),
            UserName: std::ptr::null_mut(),
        };
        let ok = unsafe { CredWriteW(&mut credential, 0) };
        if ok == 0 {
            Err(())
        } else {
            Ok(())
        }
    } else {
        let ok = unsafe { CredDeleteW(ACCOUNT_SESSION_TARGET.as_ptr(), CRED_TYPE_GENERIC, 0) };
        if ok != 0 || unsafe { GetLastError() } == ERROR_NOT_FOUND {
            Ok(())
        } else {
            Err(())
        }
    }
}

const CLIPBOARD_CAPTURE_TIMER_ID: usize = 1;
const CLIPBOARD_CAPTURE_MESSAGE: u32 = windows_sys::Win32::UI::WindowsAndMessaging::WM_APP + 1;
const CLIPBOARD_CAPTURE_DEBOUNCE_MS: u32 = 80;

/// Modifier keys a panel asks to be held while its key is pressed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub win: bool,
}

/// Work area in physical pixels, as `(left, top, right, bottom)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkArea {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl WorkArea {
    /// Bottom-centered placement for a panel of this size, where the native
    /// panels sat. Oversized panels stay pinned to the work area origin.
    pub fn bottom_center(&self, width: f64, height: f64) -> (f64, f64) {
        let available = self.right - self.left;
        let x = self.left + ((available - width) / 2.0).max(0.0);
        let y = (self.bottom - height - 12.0).max(self.top);
        (x, y)
    }

    /// Placement centred on the work area, where the shipped emoji and handwriting panels opened. Oversized panels stay pinned to the work area origin.
    pub fn center(&self, width: f64, height: f64) -> (f64, f64) {
        let x = self.left + ((self.right - self.left - width) / 2.0).max(0.0);
        let y = self.top + ((self.bottom - self.top - height) / 2.0).max(0.0);
        (x, y)
    }
}

/// The foreground window, or `None` when the desktop has no active window.
pub fn foreground_window() -> Option<InputTarget> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    // SAFETY: querying the foreground window has no preconditions.
    let window = unsafe { GetForegroundWindow() };
    (!window.is_null()).then_some(InputTarget(window as isize))
}

/// Bring the remembered window forward so synthetic input reaches it. Fails
/// when the window closed while the panel was open.
pub fn focus(target: InputTarget) -> bool {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, IsWindow, SetForegroundWindow,
    };
    let window = target.0 as HWND;
    // SAFETY: all calls validate the handle themselves. SendInput targets the
    // foreground globally, so confirm the actual foreground immediately after
    // requesting the transition instead of relying on the request alone.
    unsafe {
        IsWindow(window) != 0 && SetForegroundWindow(window) != 0 && GetForegroundWindow() == window
    }
}

/// Is the foreground window a usable destination for synthetic input?
///
/// The panels are `WS_EX_NOACTIVATE`, so the foreground really is whatever the
/// user last clicked, and the reference deliberately never calls
/// SetForegroundWindow: it just sends to whatever is in front. The only case to
/// refuse is the foreground belonging to this process, which would make the
/// panel type into itself.
pub fn foreground_is_external() -> bool {
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };
    // SAFETY: all three calls validate their arguments themselves.
    unsafe {
        let window = GetForegroundWindow();
        if window.is_null() {
            return false;
        }
        let mut process = 0u32;
        GetWindowThreadProcessId(window, &mut process);
        process != 0 && process != GetCurrentProcessId()
    }
}

/// Wait for the native Windows Server to publish a new clipboard-history row.
///
/// The event is session-local and carries no clipboard contents. A missing
/// Server or a denied event handle is treated like a timeout; the desktop
/// monitor continues its bounded file-poll fallback in either case.
pub fn wait_for_clipboard_history_change(timeout: Duration) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{OpenEventW, WaitForSingleObject};
    const SYNCHRONIZE: u32 = 0x0010_0000;
    let millis = timeout.as_millis().min(u128::from(u32::MAX)) as u32;
    // SAFETY: the name is a static, nul-terminated UTF-16 string and the
    // returned handle is closed on every path.
    let event = unsafe { OpenEventW(SYNCHRONIZE, 0, CLIPBOARD_HISTORY_CHANGE_EVENT.as_ptr()) };
    if event.is_null() {
        return false;
    }
    let result = unsafe { WaitForSingleObject(event, millis) };
    unsafe { CloseHandle(event) };
    result == WAIT_OBJECT_0
}

const CF_UNICODETEXT: u32 = 13;
const MAX_CLIPBOARD_UNITS: usize = 1_000_000;

struct ClipboardGuard;

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::System::DataExchange::CloseClipboard;
        // SAFETY: constructed only after OpenClipboard succeeds on this thread.
        unsafe {
            CloseClipboard();
        }
    }
}

struct GlobalLockGuard(*mut core::ffi::c_void);

impl Drop for GlobalLockGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::System::Memory::GlobalUnlock;
        // SAFETY: the handle was successfully locked and remains valid for the
        // lifetime of this guard because the clipboard is still open.
        unsafe {
            GlobalUnlock(self.0);
        }
    }
}

/// Read the current Windows Unicode clipboard without spawning a shell.
///
/// The caller owns normalization and persistence. This wrapper only accepts a
/// bounded, NUL-terminated UTF-16 payload and never returns clipboard data in
/// logs or diagnostics.
pub fn read_clipboard_text() -> Result<String, ()> {
    use windows_sys::Win32::System::DataExchange::{
        GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize};
    // SAFETY: a null owner is documented for callers that do not own a window;
    // all returned clipboard handles are checked before access.
    let opened = unsafe { OpenClipboard(std::ptr::null_mut()) } != 0;
    if !opened {
        return Err(());
    }
    let _clipboard = ClipboardGuard;
    // SAFETY: these calls operate on the clipboard opened above.
    if unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT) } == 0 {
        return Err(());
    }
    let handle = unsafe { GetClipboardData(CF_UNICODETEXT) };
    if handle.is_null() {
        return Err(());
    }
    let bytes = unsafe { GlobalSize(handle) };
    if bytes < 2 {
        return Err(());
    }
    let units = (bytes / std::mem::size_of::<u16>()).min(MAX_CLIPBOARD_UNITS);
    let pointer = unsafe { GlobalLock(handle) } as *const u16;
    if pointer.is_null() {
        return Err(());
    }
    let _lock = GlobalLockGuard(handle);
    // SAFETY: GlobalSize bounds this slice and the lock guard keeps the memory
    // pinned until after String::from_utf16 has copied it.
    let value = unsafe { std::slice::from_raw_parts(pointer, units) };
    let end = value.iter().position(|unit| *unit == 0).ok_or(())?;
    String::from_utf16(&value[..end]).map_err(|_| ())
}

/// Replace the Windows Unicode clipboard without relying on PowerShell.
///
/// Interior NULs are rejected instead of being silently truncated by the
/// Win32 string format. The transfer itself goes through the voice output
/// path, which owns the clipboard with a message-only window (a NULL owner
/// makes SetClipboardData fail after EmptyClipboard) and retries a clipboard
/// another process is briefly holding.
pub fn write_clipboard_text(text: &str) -> bool {
    if text.contains('\0') || !is_bounded_utf16(text, MAX_CLIPBOARD_UNITS - 1) {
        return false;
    }
    voice_output::write_unicode_clipboard(text).is_some()
}

/// Keys that must carry `KEYEVENTF_EXTENDEDKEY`.
///
/// Without the flag these arrive as their numeric-keypad twins: the arrow
/// cluster becomes 2/4/6/8, Home/End/PgUp/PgDn/Ins/Del become 7/1/9/3/0/., and
/// applications that read scan codes rather than virtual keys see the wrong
/// key entirely. The ported React layout exposes all of them, so this matters
/// more here than it did upstream.
fn extended_key(virtual_key: u16) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        VK_APPS, VK_DELETE, VK_DIVIDE, VK_DOWN, VK_END, VK_HOME, VK_INSERT, VK_LEFT, VK_LWIN,
        VK_NEXT, VK_NUMLOCK, VK_PRIOR, VK_RCONTROL, VK_RIGHT, VK_RMENU, VK_RWIN, VK_UP,
    };
    matches!(
        virtual_key,
        VK_DELETE
            | VK_LWIN
            | VK_RWIN
            | VK_RMENU
            | VK_RCONTROL
            | VK_INSERT
            | VK_HOME
            | VK_END
            | VK_PRIOR
            | VK_NEXT
            | VK_LEFT
            | VK_RIGHT
            | VK_UP
            | VK_DOWN
            | VK_NUMLOCK
            | VK_DIVIDE
            | VK_APPS
    )
}

fn key_input(
    virtual_key: u16,
    release: bool,
) -> windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        MapVirtualKeyW, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
        KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC,
    };
    let mut flags = if release { KEYEVENTF_KEYUP } else { 0 };
    if extended_key(virtual_key) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: virtual_key,
                // Applications that read the scan code instead of the virtual
                // key saw 0 for every synthetic stroke.
                wScan: unsafe { MapVirtualKeyW(virtual_key as u32, MAPVK_VK_TO_VSC) } as u16,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: unsafe {
                    windows_sys::Win32::UI::WindowsAndMessaging::GetMessageExtraInfo() as usize
                },
            },
        },
    }
}

fn unicode_input(
    unit: u16,
    release: bool,
) -> windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: 0,
                wScan: unit,
                dwFlags: KEYEVENTF_UNICODE | if release { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send(inputs: &[windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT]) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT};
    if inputs.is_empty() {
        return false;
    }
    // SAFETY: the pointer and length describe the slice above, and the size
    // argument is the structure the API expects.
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    sent as usize == inputs.len()
}

/// Press and release one key with the requested modifiers held around it.
/// The whole sequence is submitted at once so nothing else interleaves.
pub fn send_key(virtual_key: u16, modifiers: Modifiers) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT};
    if virtual_key == 0 {
        return false;
    }
    let mut held: Vec<u16> = Vec::new();
    if modifiers.ctrl {
        held.push(VK_CONTROL);
    }
    if modifiers.alt {
        held.push(VK_MENU);
    }
    if modifiers.win {
        held.push(VK_LWIN);
    }
    if modifiers.shift {
        held.push(VK_SHIFT);
    }
    let mut inputs = Vec::with_capacity(held.len() * 2 + 2);
    for modifier in &held {
        inputs.push(key_input(*modifier, false));
    }
    inputs.push(key_input(virtual_key, false));
    inputs.push(key_input(virtual_key, true));
    for modifier in held.iter().rev() {
        inputs.push(key_input(*modifier, true));
    }
    send(&inputs)
}

/// Type text the panel already holds. Surrogate pairs are delivered as the two
/// code units the receiving control expects.
pub fn valid_text(text: &str) -> bool {
    !text.is_empty() && is_bounded_text(text, MAX_TEXT_BYTES)
}

pub fn send_text(text: &str) -> bool {
    if !valid_text(text) {
        return false;
    }
    let mut inputs = Vec::new();
    for unit in text.encode_utf16() {
        inputs.push(unicode_input(unit, false));
        inputs.push(unicode_input(unit, true));
    }
    send(&inputs)
}

unsafe extern "system" fn clipboard_monitor_window_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    message: u32,
    wparam: usize,
    _lparam: isize,
) -> isize {
    use windows_sys::Win32::System::DataExchange::RemoveClipboardFormatListener;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, DestroyWindow, KillTimer, PostQuitMessage, PostThreadMessageW, SetTimer,
        WM_CLIPBOARDUPDATE, WM_CLOSE, WM_DESTROY, WM_TIMER,
    };

    match message {
        WM_CLIPBOARDUPDATE => {
            // Clipboard writers often publish several formats in sequence;
            // defer the read until the writer has finished populating them.
            SetTimer(
                hwnd,
                CLIPBOARD_CAPTURE_TIMER_ID,
                CLIPBOARD_CAPTURE_DEBOUNCE_MS,
                None,
            );
            0
        }
        WM_TIMER if wparam == CLIPBOARD_CAPTURE_TIMER_ID => {
            KillTimer(hwnd, CLIPBOARD_CAPTURE_TIMER_ID);
            PostThreadMessageW(GetCurrentThreadId(), CLIPBOARD_CAPTURE_MESSAGE, 0, 0);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            RemoveClipboardFormatListener(hwnd);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, _lparam),
    }
}

/// Watch native clipboard updates until the process exits and invoke `on_text`
/// for each debounced Unicode value. The callback owns the persistence policy;
/// this crate only performs Win32 observation and decoding.
pub fn start_clipboard_monitor<F>(mut on_text: F)
where
    F: FnMut(String) + Send + 'static,
{
    std::thread::Builder::new()
        .name("msime-windows-clipboard-monitor".to_owned())
        .spawn(move || {
            use windows_sys::Win32::System::DataExchange::AddClipboardFormatListener;
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                CreateWindowExW, DispatchMessageW, GetMessageW, RegisterClassExW, TranslateMessage,
                HWND_MESSAGE, WNDCLASSEXW,
            };

            const CLASS_NAME: &[u16] = &[
                'M' as u16, 'S' as u16, 'I' as u16, 'M' as u16, 'E' as u16, 'C' as u16, 'l' as u16,
                'i' as u16, 'p' as u16, 'b' as u16, 'o' as u16, 'a' as u16, 'r' as u16, 'd' as u16,
                'L' as u16, 'i' as u16, 's' as u16, 't' as u16, 'e' as u16, 'n' as u16, 'e' as u16,
                'r' as u16, 0,
            ];
            // SAFETY: all pointers refer to static data or this thread's
            // message loop, and the callback is confined to this thread.
            unsafe {
                let class = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    lpfnWndProc: Some(clipboard_monitor_window_proc),
                    lpszClassName: CLASS_NAME.as_ptr(),
                    ..std::mem::zeroed()
                };
                if RegisterClassExW(&class) == 0 {
                    return;
                }
                let hwnd = CreateWindowExW(
                    0,
                    CLASS_NAME.as_ptr(),
                    CLASS_NAME.as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                );
                if hwnd.is_null() || AddClipboardFormatListener(hwnd) == 0 {
                    return;
                }
                let mut message = std::mem::zeroed();
                while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                    if message.message == CLIPBOARD_CAPTURE_MESSAGE {
                        if let Ok(text) = read_clipboard_text() {
                            on_text(text);
                        }
                    } else {
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
            }
        })
        .ok();
}

/// The foreground monitor's work area, excluding the taskbar.
///
/// Panels are non-activating, so the foreground window remains the editor the
/// user is working in. Using that window's monitor keeps a panel on the same
/// display in multi-monitor setups; the system work area remains a safe
/// fallback when Windows reports no foreground window or monitor information.
pub fn work_area() -> Option<WorkArea> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, SystemParametersInfoW, SPI_GETWORKAREA,
    };

    let to_work_area = |rect: RECT| {
        (rect.right > rect.left && rect.bottom > rect.top).then_some(WorkArea {
            left: f64::from(rect.left),
            top: f64::from(rect.top),
            right: f64::from(rect.right),
            bottom: f64::from(rect.bottom),
        })
    };

    // SAFETY: these calls only query process-independent window/monitor state;
    // `GetMonitorInfoW` receives a caller-owned, correctly sized structure.
    let foreground = unsafe { GetForegroundWindow() };
    if !foreground.is_null() {
        let monitor = unsafe { MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST) };
        if !monitor.is_null() {
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                rcMonitor: RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                rcWork: RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                dwFlags: 0,
            };
            if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
                if let Some(area) = to_work_area(info.rcWork) {
                    return Some(area);
                }
            }
        }
    }

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let read =
        unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut rect as *mut RECT).cast(), 0) };
    (read != 0).then(|| to_work_area(rect)).flatten()
}

/// Reveal an existing directory in the shell. The caller owns the path; a missing or relative path is refused rather than handed to the shell.
pub fn open_directory(path: &Path) -> bool {
    if !path.is_absolute() || !path.is_dir() {
        return false;
    }
    let mut target: Vec<u16> = path.as_os_str().encode_wide().collect();
    target.push(0);
    shell_open(&target)
}

/// Open an https URL in the default browser. ShellExecuteW hands the URL to its registered handler directly, so unlike `cmd /C start` no console window flashes up from the GUI process and no shell parses the text. Anything other than an https URL is refused.
pub fn open_url(url: &str) -> bool {
    if !url.starts_with("https://") || url.contains('\0') {
        return false;
    }
    let mut target: Vec<u16> = url.encode_utf16().collect();
    target.push(0);
    shell_open(&target)
}

fn shell_open(target: &[u16]) -> bool {
    use windows_sys::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
    };
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let mut operation: Vec<u16> = "open".encode_utf16().collect();
    operation.push(0);
    // SAFETY: the apartment is released below, including on the failure path.
    let entered = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    // SAFETY: both strings are NUL terminated and outlive the call.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if entered >= 0 {
        // SAFETY: balances the successful initialization above.
        unsafe { CoUninitialize() };
    }
    // ShellExecuteW reports success with a value above the legacy error range.
    result as isize > 32
}

use std::os::windows::ffi::OsStrExt;

/// The state directory the managed Server uses, resolved the same way `production_state_directory` in `server_main.cpp` does: an absolute `METASEQUOIA_IME_DATA_DIR`, then the `DataDir` the installer records in the 64-bit machine view, then `%LOCALAPPDATA%\MSIME-Client`. The Server hands this directory to the shell it launches; a shell started any other way, such as from the Start Menu, needs it to find the same runtime options and preferences.
pub fn server_state_directory() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    if let Some(value) = std::env::var_os("METASEQUOIA_IME_DATA_DIR") {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            return Some(path);
        }
    }
    if let Some(path) = installed_data_directory().filter(|path| path.is_absolute()) {
        return Some(path);
    }
    std::env::var_os("LOCALAPPDATA")
        .map(|local| PathBuf::from(local).join("MSIME-Client"))
        .filter(|path| path.is_absolute())
}

fn installed_data_directory() -> Option<std::path::PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY,
    };
    let key: Vec<u16> = "Software\\Metasequoia\\MetasequoiaIME\0"
        .encode_utf16()
        .collect();
    let name: Vec<u16> = "DataDir\0".encode_utf16().collect();
    let flags = RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY;
    let mut bytes = 0u32;
    // SAFETY: both names are NUL terminated; a null buffer asks only for the size.
    let sized = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            name.as_ptr(),
            flags,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut bytes,
        )
    };
    if sized != ERROR_SUCCESS || (bytes as usize) < std::mem::size_of::<u16>() {
        return None;
    }
    let mut value = vec![0u16; (bytes as usize).div_ceil(std::mem::size_of::<u16>())];
    // SAFETY: the buffer holds `bytes` bytes, the size the first call reported.
    let read = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            name.as_ptr(),
            flags,
            std::ptr::null_mut(),
            value.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if read != ERROR_SUCCESS {
        return None;
    }
    // RRF_RT_REG_SZ guarantees a terminator; keep only what precedes it.
    let length = value
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(value.len());
    (length > 0).then(|| std::ffi::OsString::from_wide(&value[..length]).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_centers_on_the_work_area_and_clamps_oversized_panels() {
        let area = WorkArea {
            left: 0.0,
            top: 0.0,
            right: 1920.0,
            bottom: 1040.0,
        };
        assert_eq!(area.bottom_center(1100.0, 400.0), (410.0, 628.0));
        // A panel taller or wider than the work area stays at its origin.
        assert_eq!(area.bottom_center(3000.0, 2000.0), (0.0, 0.0));
        let offset = WorkArea {
            left: -1920.0,
            top: -100.0,
            right: 0.0,
            bottom: 980.0,
        };
        assert_eq!(offset.bottom_center(1100.0, 400.0), (-1510.0, 568.0));
        assert_eq!(area.center(550.0, 610.0), (685.0, 215.0));
        assert_eq!(area.center(3000.0, 2000.0), (0.0, 0.0));
        assert_eq!(offset.center(980.0, 650.0), (-1450.0, 115.0));
    }

    #[test]
    fn text_injection_refuses_empty_and_oversized_input() {
        assert!(!send_text(""));
        assert!(!send_text(&"a".repeat(MAX_TEXT_BYTES + 1)));
        assert!(!valid_text("line\nfeed"));
        assert!(!valid_text("tab\tvalue"));
        assert!(valid_text("合成候选"));
    }

    #[test]
    fn key_injection_refuses_an_unset_virtual_key() {
        assert!(!send_key(0, Modifiers::default()));
    }

    #[test]
    fn directories_must_be_absolute_and_exist() {
        assert!(!open_directory(Path::new("relative")));
        assert!(!open_directory(Path::new(
            "C:\\definitely-missing-msime-path"
        )));
    }

    #[test]
    fn urls_must_be_https() {
        assert!(!open_url("http://example.com"));
        assert!(!open_url("file:///C:/Windows/System32/calc.exe"));
        assert!(!open_url("calc.exe"));
        assert!(!open_url("https://example.com/\0calc"));
    }

    #[test]
    fn extended_keys_cover_the_cluster_the_panel_exposes() {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
        // Without KEYEVENTF_EXTENDEDKEY these arrive as their numeric-keypad
        // twins: the arrows become 2/4/6/8 and Home/End/PgUp/PgDn/Ins/Del
        // become 7/1/9/3/0/. - so the panel would type digits.
        for key in [
            VK_LEFT,
            VK_RIGHT,
            VK_UP,
            VK_DOWN,
            VK_HOME,
            VK_END,
            VK_PRIOR,
            VK_NEXT,
            VK_INSERT,
            VK_DELETE,
            VK_APPS,
            VK_NUMLOCK,
            VK_DIVIDE,
            VK_LWIN,
            VK_RWIN,
            VK_RMENU,
            VK_RCONTROL,
        ] {
            assert!(extended_key(key), "{key} should be extended");
        }
        // Ordinary keys must not carry the flag, or they would be misread the
        // other way round.
        for key in [
            VK_SPACE,
            VK_RETURN,
            VK_BACK,
            VK_TAB,
            VK_SHIFT,
            VK_LCONTROL,
            VK_LMENU,
            VK_NUMPAD0,
            VK_NUMPAD9,
            VK_MULTIPLY,
            VK_ADD,
            VK_SUBTRACT,
            VK_DECIMAL,
            VK_CAPITAL,
        ] {
            assert!(!extended_key(key), "{key} should not be extended");
        }
    }

    #[test]
    fn a_synthetic_stroke_carries_a_scan_code() {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
        // Applications that read the scan code rather than the virtual key saw
        // zero for every synthetic stroke.
        let down = key_input(VK_SPACE, false);
        let ki = unsafe { down.Anonymous.ki };
        assert_ne!(ki.wScan, 0);
        assert_eq!(ki.wVk, VK_SPACE);
        assert_eq!(ki.dwFlags & KEYEVENTF_KEYUP, 0);
        assert_eq!(ki.dwFlags & KEYEVENTF_EXTENDEDKEY, 0);

        let up = key_input(VK_SPACE, true);
        assert_ne!(unsafe { up.Anonymous.ki }.dwFlags & KEYEVENTF_KEYUP, 0);

        // An extended key carries both the flag and a scan code, and keeps the
        // flag on release - a Win key released without it stays stuck down.
        let left = key_input(VK_LEFT, false);
        let left_ki = unsafe { left.Anonymous.ki };
        assert_ne!(left_ki.dwFlags & KEYEVENTF_EXTENDEDKEY, 0);
        assert_ne!(left_ki.wScan, 0);
        let left_up = unsafe { key_input(VK_LEFT, true).Anonymous.ki };
        assert_ne!(left_up.dwFlags & KEYEVENTF_EXTENDEDKEY, 0);
        assert_ne!(left_up.dwFlags & KEYEVENTF_KEYUP, 0);
    }
}
