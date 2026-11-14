#include "../../src/clipboard/CloudClipboardWriter.h"

#include <cassert>
#include <string>

namespace {
// 单独的窗口站拥有自己的系统剪贴板，测试不读写登录用户的剪贴板。
class ClipboardTestDesktop {
public:
  ClipboardTestDesktop() {
    previous_station_ = GetProcessWindowStation();
    previous_desktop_ = GetThreadDesktop(GetCurrentThreadId());
    const auto name = L"MSIMEClipboardTest-" + std::to_wstring(GetCurrentProcessId());
    station_ = CreateWindowStationW(name.c_str(), CWF_CREATE_ONLY, WINSTA_ALL_ACCESS, nullptr);
    assert(station_);
    assert(SetProcessWindowStation(station_));
    desktop_ = CreateDesktopW(L"ClipboardTest", nullptr, nullptr, 0, GENERIC_ALL, nullptr);
    assert(desktop_);
    assert(SetThreadDesktop(desktop_));
  }
  ~ClipboardTestDesktop() {
    assert(SetThreadDesktop(previous_desktop_));
    assert(SetProcessWindowStation(previous_station_));
    assert(CloseDesktop(desktop_));
    assert(CloseWindowStation(station_));
  }
private:
  HWINSTA previous_station_ = nullptr, station_ = nullptr;
  HDESK previous_desktop_ = nullptr, desktop_ = nullptr;
};

std::wstring read_text() {
  assert(OpenClipboard(nullptr));
  const auto memory = GetClipboardData(CF_UNICODETEXT);
  assert(memory);
  const auto *text = static_cast<const wchar_t *>(GlobalLock(memory));
  assert(text);
  const std::wstring result(text);
  GlobalUnlock(memory);
  assert(CloseClipboard());
  return result;
}
}

int main() {
  ClipboardTestDesktop isolated;
  using msime::windows::write_cloud_clipboard_text;
  const auto foreground = GetForegroundWindow();
  const auto initial_sequence = GetClipboardSequenceNumber();
  assert(write_cloud_clipboard_text(L"合成云端复制", initial_sequence));
  // 所有者窗口销毁后，已提交的系统数据仍可读取，而且不改变前台焦点。
  assert(read_text() == L"合成云端复制");
  assert(GetForegroundWindow() == foreground);
  const auto received_sequence = GetClipboardSequenceNumber();
  assert(received_sequence != initial_sequence);
  assert(write_cloud_clipboard_text(L"合成本机新复制", received_sequence));
  const auto local_sequence = GetClipboardSequenceNumber();
  assert(!write_cloud_clipboard_text(L"过期云端复制", received_sequence));
  assert(read_text() == L"合成本机新复制");
  assert(GetClipboardSequenceNumber() == local_sequence);
  assert(!write_cloud_clipboard_text(L"", local_sequence));
  assert(!write_cloud_clipboard_text(std::wstring(L"合成\0无效", 5), local_sequence));
  assert(read_text() == L"合成本机新复制");
}
