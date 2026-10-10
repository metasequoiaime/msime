//! Native output only; voice policy and recognition belong to the caller.

use super::{InputTarget, MAX_TEXT_BYTES};
mod paste_policy;
use windows_sys::Win32::Foundation::GlobalFree;
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardSequenceNumber, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetForegroundWindow, GetWindowThreadProcessId, HWND_MESSAGE,
};

/// Restore an external editor, never a window owned by this panel process.
pub fn focus_external(target: InputTarget) -> bool {
    let mut process = 0;
    // SAFETY: the API validates the HWND and writes to a live local variable.
    unsafe {
        GetWindowThreadProcessId(target.0 as _, &mut process);
        if process == 0 || process == GetCurrentProcessId() {
            return false;
        }
    }
    // SAFETY: reading the foreground handle has no preconditions.
    super::focus(target) && unsafe { GetForegroundWindow() == target.0 as _ }
}

/// Copy bounded text and paste it into the remembered external editor.
/// Clipboard failure never sends Ctrl+V with unrelated existing contents.
/// History uses its shared UTF-8 byte budget, not the smaller SendInput budget.
pub fn paste_text(target: InputTarget, text: &str) -> bool {
    paste_text_with_limit(
        target,
        text,
        msime_client_core::clipboard::MAX_TEXT_BYTES,
        ClipboardPrivacy::Ordinary,
    )
}

fn paste_text_with_limit(
    target: InputTarget,
    text: &str,
    max_bytes: usize,
    privacy: ClipboardPrivacy,
) -> bool {
    if !paste_policy::valid_paste_text(text, max_bytes) {
        return false;
    }
    if !focus_external(target) {
        return false;
    }
    // A zero sequence cannot be compared afterwards, so it counts as failure here.
    let Some(sequence) = write_clipboard(text, privacy).filter(|sequence| *sequence != 0) else {
        return false;
    };
    std::thread::sleep(std::time::Duration::from_millis(30));
    // SAFETY: the sequence query has no preconditions. If another writer won,
    // keep the transcript in the UI instead of pasting its unrelated contents.
    if !focus_external(target) || unsafe { GetClipboardSequenceNumber() } != sequence {
        return false;
    }
    super::send_key(
        b'V' as u16,
        super::Modifiers {
            ctrl: true,
            ..Default::default()
        },
    )
}

/// Paste voice output through the same guarded external-editor path.
/// Keep the voice limit independent of the larger clipboard-history budget.
/// 识别文本不能留在任何剪贴板历史里（包括水杉自己的），所以和 Server 原生 ctrl_v 一样带上隐私标记；标记放不上就不粘贴，文字留在面板里。
pub fn paste_voice_text(target: InputTarget, text: &str) -> bool {
    paste_text_with_limit(target, text, MAX_TEXT_BYTES, ClipboardPrivacy::Voice)
}

/// 写剪贴板时要不要带上不进历史、不上云的标记。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClipboardPrivacy {
    /// 用户自己要复制或粘贴的内容（剪贴板历史、复制按钮），照常进系统历史。
    Ordinary,
    /// 语音识别文本：写入 `VOICE_CLIPBOARD_MARKERS`，每个值都是 DWORD 0。
    Voice,
}

/// Replace the clipboard with `text`, returning the clipboard sequence number
/// (which Windows may report as 0) once the data has been handed over.
/// The caller validates `text`; this only owns the Win32 transfer.
pub(crate) fn write_unicode_clipboard(text: &str) -> Option<u32> {
    write_clipboard(text, ClipboardPrivacy::Ordinary)
}

/// 在已打开、已清空的剪贴板上放一个值为 0 的 DWORD 注册格式。失败时在这里释放分配的内存，返回 false。
///
/// # Safety
///
/// 调用线程必须正持有剪贴板（OpenClipboard 成功、EmptyClipboard 之后）。
unsafe fn set_zero_dword_format(name: &str) -> bool {
    let wide = super::wide(name);
    // SAFETY: 调用方持有剪贴板；GMEM_MOVEABLE 内存只有 SetClipboardData 成功后才交给系统，失败时在这里释放。
    unsafe {
        let format = RegisterClipboardFormatW(wide.as_ptr());
        if format == 0 {
            return false;
        }
        let memory = GlobalAlloc(GMEM_MOVEABLE, size_of::<u32>());
        if memory.is_null() {
            return false;
        }
        let value = GlobalLock(memory).cast::<u32>();
        if value.is_null() {
            GlobalFree(memory);
            return false;
        }
        value.write_unaligned(0);
        GlobalUnlock(memory);
        if SetClipboardData(format, memory).is_null() {
            GlobalFree(memory);
            return false;
        }
        true
    }
}

fn write_clipboard(text: &str, privacy: ClipboardPrivacy) -> Option<u32> {
    let units: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: STATIC is a system-provided window class. This non-visible owner
    // is created and destroyed on the current thread. A NULL clipboard owner
    // is unsuitable for EmptyClipboard followed by SetClipboardData.
    unsafe {
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let owner = CreateWindowExW(
            0,
            class.as_ptr(),
            std::ptr::null(),
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
        if owner.is_null() {
            return None;
        }
        let memory = GlobalAlloc(GMEM_MOVEABLE, units.len() * size_of::<u16>());
        if memory.is_null() {
            DestroyWindow(owner);
            return None;
        }
        let destination = GlobalLock(memory).cast::<u16>();
        if destination.is_null() {
            GlobalFree(memory);
            DestroyWindow(owner);
            return None;
        }
        std::ptr::copy_nonoverlapping(units.as_ptr(), destination, units.len());
        GlobalUnlock(memory);
        let mut opened = false;
        for _ in 0..5 {
            if OpenClipboard(owner) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // 与 Server 原生 ctrl_v 同一顺序：清空后先放标记再放正文；任何一个标记放不上就清空剪贴板、不写正文，调用方因此不会发出 Ctrl+V。
        let emptied = opened && EmptyClipboard() != 0;
        let marked = emptied
            && (privacy == ClipboardPrivacy::Ordinary
                || crate::clipboard_privacy::VOICE_CLIPBOARD_MARKERS
                    .iter()
                    .all(|name| set_zero_dword_format(name)));
        // CF_UNICODETEXT is 13; ownership of GMEM_MOVEABLE memory transfers to
        // Windows only on successful SetClipboardData. Never free it afterward.
        let transferred = marked && !SetClipboardData(13, memory).is_null();
        if emptied && !transferred {
            // 不留下只有标记、没有正文的半份写入。
            EmptyClipboard();
        }
        let sequence = GetClipboardSequenceNumber();
        if opened {
            CloseClipboard();
        }
        if !transferred {
            GlobalFree(memory);
        }
        DestroyWindow(owner);
        transferred.then_some(sequence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_transcripts_do_not_touch_the_clipboard() {
        let invalid_target = InputTarget(0);
        assert!(!paste_voice_text(invalid_target, ""));
        assert!(!paste_voice_text(invalid_target, "x\0y"));
        assert!(!paste_voice_text(
            invalid_target,
            &"x".repeat(MAX_TEXT_BYTES + 1)
        ));
        assert!(!focus_external(invalid_target));
    }
}
