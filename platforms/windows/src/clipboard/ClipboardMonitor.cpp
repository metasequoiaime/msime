#include "ClipboardHistory.h"
#ifdef _WIN32
#include <windows.h>
#include <string>

namespace msime::windows {
namespace {
constexpr UINT_PTR capture_timer_id = 1;
constexpr UINT capture_debounce_ms = 80;

std::string wide_to_utf8(const std::wstring &text) {
  if (text.empty()) return {};
  const int size = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), nullptr, 0, nullptr, nullptr);
  if (size <= 0) return {};
  std::string result(size, '\0');
  WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), result.data(), size, nullptr, nullptr);
  return result;
}
}
ClipboardMonitor::ClipboardMonitor(ClipboardHistory &history, Callback callback) : history_(history), callback_(std::move(callback)) {}
ClipboardMonitor::~ClipboardMonitor() { stop(); }
LRESULT CALLBACK ClipboardMonitor::window_proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
  auto *monitor = reinterpret_cast<ClipboardMonitor *>(GetWindowLongPtrW(static_cast<HWND>(window), GWLP_USERDATA));
  if (message == WM_NCCREATE) {
    auto *create = reinterpret_cast<CREATESTRUCTW *>(lparam);
    monitor = static_cast<ClipboardMonitor *>(create->lpCreateParams);
    SetWindowLongPtrW(static_cast<HWND>(window), GWLP_USERDATA, reinterpret_cast<LONG_PTR>(monitor));
  }
  if (monitor && message == WM_CLIPBOARDUPDATE) {
    SetTimer(static_cast<HWND>(window), capture_timer_id, capture_debounce_ms,
             nullptr);
    return 0;
  }
  if (monitor && message == WM_TIMER && wparam == capture_timer_id) {
    KillTimer(static_cast<HWND>(window), capture_timer_id);
    if (!monitor->history_.enabled()) return 0;
    const auto sequence = GetClipboardSequenceNumber();
    if (sequence == monitor->sequence_ ||
        !IsClipboardFormatAvailable(CF_UNICODETEXT) ||
        !OpenClipboard(static_cast<HWND>(window)))
      return 0;
    // Windows allows one clipboard owner at a time, so every path from here
    // must close it. The second availability check below can fail when another
    // process replaces the contents between the two checks; leaving the
    // clipboard open there broke Ctrl+C and Ctrl+V in every application on the
    // desktop until this process exited.
    std::wstring value;
    {
      struct ClipboardScope {
        ~ClipboardScope() { CloseClipboard(); }
      } scope;
      if (IsClipboardFormatAvailable(CF_UNICODETEXT)) {
        auto data = GetClipboardData(CF_UNICODETEXT);
        const auto *text = data ? static_cast<const wchar_t *>(GlobalLock(data)) : nullptr;
        if (text) {
          value = bounded_clipboard_text(text, GlobalSize(data));
          GlobalUnlock(data);
        }
      }
    }
    // Do not hold the system clipboard while conversion or the shared writer
    // waits for disk/preferences locks. Other applications must remain able
    // to copy and paste during persistence.
    monitor->sequence_ = sequence;
    // The callback owns persistence through client-core. Writing the legacy
    // string-array archive here would discard Tauri's structured entries.
    if (auto utf8 = wide_to_utf8(value); !utf8.empty() && monitor->callback_) {
      monitor->callback_(std::move(utf8));
      // The Tauri shell owns the shared clipboard panel. Signal only that the
      // bounded store changed, and only once the callback has written it, so
      // the shell never re-reads ahead of the write. The shell re-reads the
      // file under its own lock, so clipboard text never crosses this process
      // boundary in an event.
      if (monitor->change_event_)
        SetEvent(monitor->change_event_);
    }
    return 0;
  }
  return DefWindowProcW(window, message, wparam, lparam);
}
bool ClipboardMonitor::start() {
  if (window_) return true;
  const auto instance = GetModuleHandleW(nullptr);
  const wchar_t name[] = L"MSIMEClientClipboardMonitor" MSIME_EDITION_NAME_SUFFIX;
  WNDCLASSW klass{}; klass.hInstance = instance; klass.lpfnWndProc = window_proc; klass.lpszClassName = name;
  // A name collision is the one failure worth distinguishing: everything else
  // surfaces later as a CreateWindowExW failure with no hint of the cause.
  if (!RegisterClassW(&klass) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS)
    return false;
  window_ = CreateWindowExW(0, name, L"", 0, 0, 0, 0, 0, HWND_MESSAGE, nullptr, instance, this);
  if (!window_) return false;
  change_event_ = CreateEventW(nullptr, FALSE, FALSE,
                               clipboard_history_change_event_name);
  sequence_ = GetClipboardSequenceNumber();
  if (AddClipboardFormatListener(static_cast<HWND>(window_)) != FALSE)
    return true;
  DestroyWindow(static_cast<HWND>(window_));
  window_ = nullptr;
  if (change_event_) {
    CloseHandle(change_event_);
    change_event_ = nullptr;
  }
  return false;
}
void ClipboardMonitor::stop() {
  if (window_) {
    KillTimer(static_cast<HWND>(window_), capture_timer_id);
    RemoveClipboardFormatListener(static_cast<HWND>(window_));
    DestroyWindow(static_cast<HWND>(window_)); window_ = nullptr;
  }
  if (change_event_) {
    CloseHandle(change_event_);
    change_event_ = nullptr;
  }
}
} // namespace msime::windows
#endif
