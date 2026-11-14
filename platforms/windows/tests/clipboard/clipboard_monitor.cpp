#include "ClipboardHistory.h"

#ifdef _WIN32
#include "clipboard/ClipboardPrivacyPolicy.h"
#include "ClipboardTestDesktop.h"
#include <windows.h>
#include <cwchar>
#include <vector>
#endif

#include <filesystem>
#include <iostream>

#ifdef _WIN32
namespace {
// 只在专用测试桌面写合成内容，不读取或输出原剪贴板正文。
bool copy_text(HWND owner, const wchar_t *text, bool excluded = false) {
  if (!OpenClipboard(owner)) return false;
  struct Close { ~Close() { CloseClipboard(); } } close;
  if (!EmptyClipboard()) return false;
  const auto length = std::wcslen(text) + 1;
  const auto data = GlobalAlloc(GMEM_MOVEABLE, length * sizeof(wchar_t));
  if (!data) return false;
  auto *bytes = static_cast<wchar_t *>(GlobalLock(data));
  if (!bytes) { GlobalFree(data); return false; }
  std::wmemcpy(bytes, text, length);
  GlobalUnlock(data);
  if (!SetClipboardData(CF_UNICODETEXT, data)) { GlobalFree(data); return false; }
  if (excluded) {
    const auto marker = RegisterClipboardFormatW(msime::windows::clipboard_exclude_monitor_format);
    const auto flag = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, sizeof(DWORD));
    if (!flag) return false;
    if (!marker || !SetClipboardData(marker, flag)) { GlobalFree(flag); return false; }
  }
  return true;
}

// 每次复制都派发实际的系统通知，再写下一条；不等待旧的 80 毫秒防抖窗口。
bool dispatch_change() {
  const auto deadline = GetTickCount64() + 1000;
  do {
    bool changed = false;
    MSG message{};
    while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
      changed = changed || message.message == WM_CLIPBOARDUPDATE;
      TranslateMessage(&message);
      DispatchMessageW(&message);
    }
    if (changed) return true;
    MsgWaitForMultipleObjects(0, nullptr, FALSE, 10, QS_ALLINPUT);
  } while (GetTickCount64() < deadline);
  return false;
}

void drain_timers() {
  const auto deadline = GetTickCount64() + 200;
  do {
    MSG message{};
    while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) DispatchMessageW(&message);
    MsgWaitForMultipleObjects(0, nullptr, FALSE, 10, QS_ALLINPUT);
  } while (GetTickCount64() < deadline);
}
}
#endif

int main() {
#ifndef _WIN32
  std::cout << "Clipboard monitor requires Windows\n";
  return 0;
#else
  ClipboardTestDesktop isolated;
  if (!isolated.ready()) {
    std::cerr << "Isolated clipboard desktop setup failed\n";
    return 1;
  }
  using msime::windows::ClipboardHistory;
  using msime::windows::ClipboardMonitor;
  ClipboardHistory history(std::filesystem::temp_directory_path() /
                           "msime-clipboard-monitor-native-test.json");
  const auto owner = CreateWindowExW(0, L"STATIC", L"", 0, 0, 0, 0, 0,
                                     HWND_MESSAGE, nullptr, GetModuleHandleW(nullptr), nullptr);
  if (!owner) return 1;
  struct Destroy { HWND window; ~Destroy() { DestroyWindow(window); } } destroy{owner};
  std::vector<std::string> captured;
  bool released_before_callback = true;
  ClipboardMonitor monitor(history, [&](std::string text) {
    released_before_callback = released_before_callback && GetOpenClipboardWindow() == nullptr;
    const bool opened = OpenClipboard(owner) != FALSE;
    released_before_callback = released_before_callback && opened;
    if (opened) CloseClipboard();
    captured.push_back(std::move(text));
  });
  if (!monitor.start() || !monitor.start()) {
    std::cerr << "Clipboard listener registration failed\n";
    return 1;
  }
  drain_timers();
  if (!captured.empty()) {
    std::cerr << "Registration imported pre-existing clipboard text\n";
    return 1;
  }
  history.set_enabled(true);
  for (const auto *text : {L"Synthetic clipboard alpha", L"Synthetic clipboard beta",
                            L"Synthetic clipboard gamma"}) {
    if (!copy_text(owner, text) || !dispatch_change()) {
      std::cerr << "Synthetic clipboard update was not delivered\n";
      return 1;
    }
  }
  drain_timers();
  if (captured != std::vector<std::string>{"Synthetic clipboard alpha", "Synthetic clipboard beta",
                                          "Synthetic clipboard gamma"} || !released_before_callback) {
    std::cerr << "Continuous copies were lost or persistence held the system clipboard\n";
    return 1;
  }
  history.set_enabled(false);
  if (!copy_text(owner, L"Synthetic disabled") || !dispatch_change()) return 1;
  drain_timers();
  history.set_enabled(true);
  if (!copy_text(owner, L"Synthetic excluded", true) || !dispatch_change()) return 1;
  drain_timers();
  if (captured.size() != 3) {
    std::cerr << "Disabled or excluded clipboard content was captured\n";
    return 1;
  }
  monitor.stop();
  monitor.stop();
  std::cout << "Clipboard registration, continuous copies, privacy and shutdown passed\n";
  return 0;
#endif
}
