#pragma once

#include <windows.h>

#include <algorithm>
#include <string>

namespace msime::windows {

// 写入需要真实的窗口所有者；空句柄配合 EmptyClipboard 会使 SetClipboardData 失败。
inline bool write_cloud_clipboard_text(const std::wstring &text, DWORD expected_sequence) {
  if (text.empty() || text.find(L'\0') != std::wstring::npos) return false;
  const auto window = CreateWindowExW(0, L"STATIC", L"", 0, 0, 0, 0, 0, HWND_MESSAGE,
                                       nullptr, GetModuleHandleW(nullptr), nullptr);
  if (!window) return false;
  struct DestroyWindowScope {
    HWND window;
    ~DestroyWindowScope() { DestroyWindow(window); }
  } owner{window};
  if (!OpenClipboard(window)) return false;
  struct CloseClipboardScope { ~CloseClipboardScope() { CloseClipboard(); } } close;
  // 持有系统剪贴板期间比较代次，避免读取与清空之间覆盖用户新复制。
  if (GetClipboardSequenceNumber() != expected_sequence) return false;
  const auto memory = GlobalAlloc(GMEM_MOVEABLE, (text.size() + 1) * sizeof(wchar_t));
  if (!memory) return false;
  auto *target = static_cast<wchar_t *>(GlobalLock(memory));
  if (!target) { GlobalFree(memory); return false; }
  std::copy(text.begin(), text.end(), target);
  target[text.size()] = L'\0';
  GlobalUnlock(memory);
  if (!EmptyClipboard() || !SetClipboardData(CF_UNICODETEXT, memory)) {
    GlobalFree(memory);
    return false;
  }
  return true;
}

} // namespace msime::windows
