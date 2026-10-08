#pragma once
#include <cstdint>
#include <initializer_list>
#include <optional>
#include <windows.h>
// WIN32_LEAN_AND_MEAN 会把 shellapi.h 挡在 windows.h 外面，SHQueryUserNotificationState 在这里声明。
#include <shellapi.h>

namespace msime::windows {
// Is the foreground window a full-screen application?
//
// The toolbar sits topmost, so without this it floats over full-screen video
// and presentations. Mirrors the reference CheckFullscreen: compare the
// window's rectangle against its monitor and exclude the shell, whose desktop
// and tray windows legitimately cover the screen.
inline bool foreground_is_fullscreen(HWND foreground) {
  if (!foreground || !IsWindowVisible(foreground))
    return false;
  // The desktop and the shell tray are always "full screen" and must not count.
  if (foreground == GetShellWindow() || foreground == GetDesktopWindow())
    return false;
  wchar_t klass[64]{};
  if (GetClassNameW(foreground, klass, 64)) {
    for (const wchar_t *shell : {L"Progman", L"WorkerW", L"Shell_TrayWnd"})
      if (!lstrcmpiW(klass, shell))
        return false;
  }
  RECT window{};
  if (!GetWindowRect(foreground, &window))
    return false;
  MONITORINFO monitor{};
  monitor.cbSize = sizeof(monitor);
  if (!GetMonitorInfoW(MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST),
                       &monitor))
    return false;
  // Compare against the full monitor rather than the work area: a maximised
  // window covers the work area and must not be mistaken for full screen.
  const RECT &screen = monitor.rcMonitor;
  return window.left <= screen.left && window.top <= screen.top &&
         window.right >= screen.right && window.bottom >= screen.bottom;
}
// 前台的呈现方式。Fullscreen 是几何上铺满显示器（无边框全屏，或开了全屏优化的全屏），外部置顶窗口仍能叠在上面；ExclusiveFullscreen 是系统报告的 D3D 独占全屏，外部窗口显示不出来，弹出来还可能把游戏挤出全屏。
enum class ForegroundPresentation { Windowed, Fullscreen, ExclusiveFullscreen };
// 只有几何铺满时才信 D3D 独占的报告：SHQueryUserNotificationState 是系统全局状态，不针对某个窗口。
constexpr ForegroundPresentation classify_foreground(bool covers_monitor,
                                                     bool d3d_exclusive) {
  if (!covers_monitor)
    return ForegroundPresentation::Windowed;
  return d3d_exclusive ? ForegroundPresentation::ExclusiveFullscreen
                       : ForegroundPresentation::Fullscreen;
}
// 前台窗口的呈现方式，只在主循环线程上调用。只有几何铺满时才调 SHQueryUserNotificationState，并且只认 QUNS_RUNNING_D3D_FULL_SCREEN（QUNS_BUSY 有误报，不作判据）。前台 HWND 不变时结果缓存 300ms。真正调用了 QUNS 时，quns_microseconds 收到这一次的耗时，主循环据此判断要不要把分类挪出主循环。
inline ForegroundPresentation
foreground_presentation(HWND foreground, uint64_t now_ms,
                        std::optional<uint64_t> *quns_microseconds = nullptr) {
  static HWND cached_window = nullptr;
  static uint64_t cached_at = 0;
  static bool cached = false;
  static ForegroundPresentation cached_value = ForegroundPresentation::Windowed;
  if (cached && foreground == cached_window && now_ms - cached_at < 300)
    return cached_value;
  const bool covers = foreground_is_fullscreen(foreground);
  bool exclusive = false;
  if (covers) {
    LARGE_INTEGER frequency{}, start{}, end{};
    QueryPerformanceFrequency(&frequency);
    QueryPerformanceCounter(&start);
    QUERY_USER_NOTIFICATION_STATE state{};
    exclusive = SUCCEEDED(SHQueryUserNotificationState(&state)) &&
                state == QUNS_RUNNING_D3D_FULL_SCREEN;
    QueryPerformanceCounter(&end);
    if (quns_microseconds && frequency.QuadPart > 0)
      *quns_microseconds = static_cast<uint64_t>(
          (end.QuadPart - start.QuadPart) * 1000000 / frequency.QuadPart);
  }
  cached_window = foreground;
  cached_at = now_ms;
  cached = true;
  cached_value = classify_foreground(covers, exclusive);
  return cached_value;
}
} // namespace msime::windows
