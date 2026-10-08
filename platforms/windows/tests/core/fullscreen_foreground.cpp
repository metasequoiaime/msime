#include "FloatingToolbarVisibilityPolicy.h"
#include "FullscreenForeground.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
// 只有几何铺满时才信 D3D 独占的报告：QUNS 是系统全局状态，前台没铺满时可能是别的进程在独占。
static_assert(classify_foreground(false, false) ==
              ForegroundPresentation::Windowed);
static_assert(classify_foreground(false, true) ==
              ForegroundPresentation::Windowed);
static_assert(classify_foreground(true, false) ==
              ForegroundPresentation::Fullscreen);
static_assert(classify_foreground(true, true) ==
              ForegroundPresentation::ExclusiveFullscreen);
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Fullscreen foreground test failed at line " +
                           std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
} // namespace
int main() {
  try {
    // Null and the shell's own windows are never full screen: the desktop
    // covers the monitor by definition, and hiding the toolbar for it would
    // mean hiding it whenever nothing else has focus.
    require(!foreground_is_fullscreen(nullptr));
    require(!foreground_is_fullscreen(GetDesktopWindow()));
    if (HWND shell = GetShellWindow())
      require(!foreground_is_fullscreen(shell));
    // 没有前台窗口时是窗口化，也不去调 QUNS。
    std::optional<uint64_t> quns;
    require(foreground_presentation(nullptr, 0, &quns) ==
            ForegroundPresentation::Windowed);
    require(!quns);

    // A small visible window is not full screen.
    WNDCLASSEXW klass{};
    klass.cbSize = sizeof(klass);
    klass.lpfnWndProc = DefWindowProcW;
    klass.hInstance = GetModuleHandleW(nullptr);
    klass.lpszClassName = L"MSIME.Test.FullscreenProbe";
    if (!RegisterClassExW(&klass) &&
        GetLastError() != ERROR_CLASS_ALREADY_EXISTS)
      require(false);
    HWND window = CreateWindowExW(0, klass.lpszClassName, L"probe",
                                  WS_POPUP, 10, 10, 200, 120, nullptr, nullptr,
                                  klass.hInstance, nullptr);
    require(window != nullptr);
    ShowWindow(window, SW_SHOWNA);
    require(!foreground_is_fullscreen(window));
    require(foreground_presentation(window, 1000, &quns) ==
            ForegroundPresentation::Windowed);
    require(!quns);

    // Grown to cover its whole monitor, it is.
    MONITORINFO monitor{};
    monitor.cbSize = sizeof(monitor);
    require(GetMonitorInfoW(MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST),
                            &monitor) != 0);
    const RECT &screen = monitor.rcMonitor;
    require(SetWindowPos(window, nullptr, screen.left, screen.top,
                         screen.right - screen.left, screen.bottom - screen.top,
                         SWP_NOACTIVATE | SWP_NOZORDER) != 0);
    require(foreground_is_fullscreen(window));
    // 同一个前台窗口 300ms 内沿用上一次的结果；过了缓存期才重新分类，这时铺满了，才会去调 QUNS 并报出耗时。
    require(foreground_presentation(window, 1299, &quns) ==
            ForegroundPresentation::Windowed);
    require(!quns);
    require(foreground_presentation(window, 1300, &quns) !=
            ForegroundPresentation::Windowed);
    require(quns.has_value());

    // A window covering only the work area is maximised, not full screen, and
    // the toolbar must keep showing over it.
    const RECT &work = monitor.rcWork;
    if (work.bottom - work.top < screen.bottom - screen.top) {
      require(SetWindowPos(window, nullptr, work.left, work.top,
                           work.right - work.left, work.bottom - work.top,
                           SWP_NOACTIVATE | SWP_NOZORDER) != 0);
      require(!foreground_is_fullscreen(window));
    }

    // A hidden window never counts, however large.
    ShowWindow(window, SW_HIDE);
    require(SetWindowPos(window, nullptr, screen.left, screen.top,
                         screen.right - screen.left, screen.bottom - screen.top,
                         SWP_NOACTIVATE | SWP_NOZORDER) != 0);
    require(!foreground_is_fullscreen(window));
    DestroyWindow(window);

    // The policy this feeds: fullscreen wins over an enabled toolbar, and an
    // inactive IME hides it regardless.
    require(ShouldShowFloatingToolbar(true, false, true));
    require(!ShouldShowFloatingToolbar(true, true, true));
    require(!ShouldShowFloatingToolbar(false, false, true));
    require(!ShouldShowFloatingToolbar(true, false, false));

    std::cout << "Fullscreen foreground: full screen is distinguished from maximised\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  } catch (...) {
    std::cerr << "Fullscreen foreground test failed with an unknown error\n";
    return 1;
  }
}
