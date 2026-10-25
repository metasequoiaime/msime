use crate::{edition_named, foreground_is_external};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Security::Cryptography::{
    BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
};
use windows_sys::Win32::System::DataExchange::COPYDATASTRUCT;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId,
    SendMessageTimeoutW, GUITHREADINFO, HWND_MESSAGE, SMTO_ABORTIFHUNG, SMTO_BLOCK,
    SMTO_ERRORONEXIT, WM_COPYDATA,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupedKeyError {
    Unavailable,
    Unconfirmed,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct Target {
    foreground: HWND,
    focus: HWND,
    thread: u32,
    process: u32,
}

fn target() -> Option<Target> {
    if !foreground_is_external() {
        return None;
    }
    // 安全：只读取当前前台和其输入队列中的焦点 HWND；不激活或移动窗口。
    unsafe {
        let foreground = GetForegroundWindow();
        let foreground_thread = GetWindowThreadProcessId(foreground, core::ptr::null_mut());
        let mut info: GUITHREADINFO = core::mem::zeroed();
        info.cbSize = core::mem::size_of::<GUITHREADINFO>() as u32;
        if foreground_thread == 0 || GetGUIThreadInfo(foreground_thread, &mut info) == 0 {
            return None;
        }
        let mut process = 0;
        let thread = GetWindowThreadProcessId(info.hwndFocus, &mut process);
        (thread != 0 && process != 0 && process != std::process::id()).then_some(Target {
            foreground,
            focus: info.hwndFocus,
            thread,
            process,
        })
    }
}

fn send(window: HWND, bytes: &[u8; 24]) -> Option<bool> {
    let data = COPYDATASTRUCT {
        dwData: 0x4d53_474b,
        cbData: bytes.len() as u32,
        lpData: bytes.as_ptr().cast::<core::ffi::c_void>() as *mut _,
    };
    let mut accepted = 0;
    // 安全：系统封送固定字节载荷；此栈帧在同步调用结束前保持有效，不传私有指针。
    let sent = unsafe {
        SendMessageTimeoutW(
            window,
            WM_COPYDATA,
            0,
            (&data as *const COPYDATASTRUCT) as isize,
            SMTO_ABORTIFHUNG | SMTO_BLOCK | SMTO_ERRORONEXIT,
            250,
            &mut accepted,
        )
    };
    (sent != 0).then_some(accepted != 0)
}

/// 将多义全拼按键交给当前 TIP。失败或超时均不重发，也不回退普通键盘注入。
pub fn send_grouped_key(letter: char) -> Result<(), GroupedKeyError> {
    if !letter.is_ascii_lowercase() {
        return Err(GroupedKeyError::Unavailable);
    }
    let expected = target().ok_or(GroupedKeyError::Unavailable)?;
    let class = edition_named("MetasequoiaIMEWorkerWnd").ok_or(GroupedKeyError::Unavailable)?;
    let mut previous = core::ptr::null_mut();
    let mut receiver = core::ptr::null_mut();
    // 安全：只枚举本版本消息窗口，按真实焦点线程及进程匹配；不向其他窗口发送消息。
    unsafe {
        for _ in 0..256 {
            let window = FindWindowExW(HWND_MESSAGE, previous, class.as_ptr(), core::ptr::null());
            if window.is_null() {
                break;
            }
            let mut process = 0;
            if GetWindowThreadProcessId(window, &mut process) == expected.thread
                && process == expected.process
            {
                receiver = window;
                break;
            }
            previous = window;
        }
    }
    if receiver.is_null() || target() != Some(expected) {
        return Err(GroupedKeyError::Unavailable);
    }
    let mut nonce = [0u8; 8];
    // 安全：系统随机源只写入这八个可写字节，不依赖外部密钥句柄。
    let random = unsafe {
        BCryptGenRandom(
            core::ptr::null_mut(),
            nonce.as_mut_ptr(),
            nonce.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if random < 0 || nonce == [0; 8] {
        return Err(GroupedKeyError::Unavailable);
    }
    // 与 shared/contracts/grouped_panel_key.h 一致，避免跨位宽结构体布局差异。
    let mut bytes = [0u8; 24];
    bytes[..4].copy_from_slice(&1u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
    bytes[8..16].copy_from_slice(&nonce);
    if send(receiver, &bytes) != Some(true) || target() != Some(expected) {
        return Err(GroupedKeyError::Unavailable);
    }
    bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&(letter as u32).to_le_bytes());
    match send(receiver, &bytes) {
        Some(true) => Ok(()),
        Some(false) => Err(GroupedKeyError::Unavailable),
        None => Err(GroupedKeyError::Unconfirmed),
    }
}
