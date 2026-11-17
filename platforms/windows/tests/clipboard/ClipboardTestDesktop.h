#pragma once
#include <windows.h>
#include <string>

// 每个测试进程使用独立窗口站的剪贴板，不读取或改写登录用户的内容。
class ClipboardTestDesktop final {
public:
  ClipboardTestDesktop() {
    previous_station_ = GetProcessWindowStation();
    previous_desktop_ = GetThreadDesktop(GetCurrentThreadId());
    if (!previous_station_ || !previous_desktop_) return;
    const auto name = L"MSIMEClipboardMonitorTest-" + std::to_wstring(GetCurrentProcessId());
    station_ = CreateWindowStationW(name.c_str(), CWF_CREATE_ONLY, WINSTA_ALL_ACCESS, nullptr);
    if (!station_ || !SetProcessWindowStation(station_)) return;
    desktop_ = CreateDesktopW(L"ClipboardTest", nullptr, nullptr, 0, GENERIC_ALL, nullptr);
    ready_ = desktop_ && SetThreadDesktop(desktop_);
  }
  ~ClipboardTestDesktop() {
    if (desktop_) {
      SetThreadDesktop(previous_desktop_);
      CloseDesktop(desktop_);
    }
    if (station_) {
      SetProcessWindowStation(previous_station_);
      CloseWindowStation(station_);
    }
  }
  bool ready() const { return ready_; }
  ClipboardTestDesktop(const ClipboardTestDesktop &) = delete;
  ClipboardTestDesktop &operator=(const ClipboardTestDesktop &) = delete;
private:
  HWINSTA previous_station_ = nullptr, station_ = nullptr;
  HDESK previous_desktop_ = nullptr, desktop_ = nullptr;
  bool ready_ = false;
};
