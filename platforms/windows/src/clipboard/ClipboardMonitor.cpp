#include "ClipboardHistory.h"
#ifdef _WIN32
#include "ClipboardPrivacyPolicy.h"
#include <windows.h>
#include <string>

namespace msime::windows {
namespace {
constexpr UINT_PTR capture_timer_id = 1;
constexpr UINT capture_retry_ms = 20;
constexpr unsigned int capture_retry_limit = 5;

std::string wide_to_utf8(const std::wstring &text) {
  if (text.empty()) return {};
  const int size = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), nullptr, 0, nullptr, nullptr);
  if (size <= 0) return {};
  std::string result(size, '\0');
  WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text.data(), static_cast<int>(text.size()), result.data(), size, nullptr, nullptr);
  return result;
}

bool clipboard_format_present(UINT format) {
  return format != 0 && IsClipboardFormatAvailable(format) != FALSE;
}

// 只在剪贴板已打开时调用：读 DWORD 许可格式要取数据。
ClipboardPermission clipboard_permission(UINT format) {
  if (!clipboard_format_present(format))
    return ClipboardPermission::absent;
  const auto data = GetClipboardData(format);
  const void *bytes = data ? GlobalLock(data) : nullptr;
  const auto permission = clipboard_permission_from_data(bytes, bytes ? GlobalSize(data) : 0);
  if (bytes)
    GlobalUnlock(data);
  return permission;
}

// 格式 id 是会话内全局的，注册一次即可；注册失败得到 0，等同于剪贴板上没有这个格式。
ClipboardPrivacyMarkers clipboard_privacy_markers() {
  static const UINT exclude_monitor = RegisterClipboardFormatW(clipboard_exclude_monitor_format);
  static const UINT viewer_ignore = RegisterClipboardFormatW(clipboard_viewer_ignore_format);
  static const UINT history = RegisterClipboardFormatW(clipboard_history_permission_format);
  static const UINT cloud = RegisterClipboardFormatW(clipboard_cloud_permission_format);
  ClipboardPrivacyMarkers markers;
  markers.exclude_from_monitor = clipboard_format_present(exclude_monitor);
  markers.viewer_ignore = clipboard_format_present(viewer_ignore);
  markers.history = clipboard_permission(history);
  markers.cloud = clipboard_permission(cloud);
  return markers;
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
  if (monitor && (message == WM_CLIPBOARDUPDATE ||
                  (message == WM_TIMER && wparam == capture_timer_id))) {
    KillTimer(static_cast<HWND>(window), capture_timer_id);
    if (message == WM_CLIPBOARDUPDATE) monitor->capture_attempts_ = 0;
    // 通知到达就保存这一条；只有剪贴板暂时被其他进程锁住时重试，不能把连续复制防抖成最后一条。
    if (!monitor->capture(window) && monitor->capture_attempts_++ < capture_retry_limit)
      SetTimer(window, capture_timer_id, capture_retry_ms, nullptr);
    return 0;
  }
  return DefWindowProcW(window, message, wparam, lparam);
}
bool ClipboardMonitor::capture(HWND window) {
  if (!history_.enabled()) return true;
  if (GetClipboardSequenceNumber() == sequence_) return true;
  if (!OpenClipboard(window)) return false;
  std::wstring value;
  unsigned long sequence = 0;
  {
    // 所有退出路径都释放系统锁；锁内读取代次，避免打开之前的变化让序号和正文错配。
    struct ClipboardScope {
      ~ClipboardScope() { CloseClipboard(); }
    } scope;
    sequence = GetClipboardSequenceNumber();
    if (sequence == sequence_) return true;
    // 密码管理器等标记为不可记录的内容整条丢弃，仍将这次变化记为已处理。
    if (!clipboard_sample_excluded(clipboard_privacy_markers()) &&
        IsClipboardFormatAvailable(CF_UNICODETEXT)) {
      auto data = GetClipboardData(CF_UNICODETEXT);
      const auto *text = data ? static_cast<const wchar_t *>(GlobalLock(data)) : nullptr;
      if (text) {
        value = bounded_clipboard_text(text, GlobalSize(data));
        GlobalUnlock(data);
      }
    }
  }
  // 转码及共享存储回调可能等磁盘锁，不能在此期间占住系统剪贴板。
  sequence_ = sequence;
  if (auto utf8 = wide_to_utf8(value); !utf8.empty() && callback_) {
    // 共享层持有结构化历史，只在回调写完后通知面板重新读取；事件中不带正文。
    callback_(std::move(utf8));
    if (change_event_) SetEvent(change_event_);
  }
  return true;
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
