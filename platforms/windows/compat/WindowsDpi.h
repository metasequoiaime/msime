#pragma once

#include <windows.h>
#include <stdexcept>

namespace msime::windows::compat {

// 按导出能力选择 DPI 接口。新版系统的调用与结果原样透传；旧系统只在进程初始化时退回系统 DPI 感知，线程作用域不修改进程状态。
struct DpiApi {
  using WindowDpi = UINT(WINAPI *)(HWND);
  using SystemDpi = UINT(WINAPI *)();
  using ThreadContext = DPI_AWARENESS_CONTEXT(WINAPI *)(DPI_AWARENESS_CONTEXT);
  using ProcessContext = BOOL(WINAPI *)(DPI_AWARENESS_CONTEXT);
  using WindowContext = DPI_AWARENESS_CONTEXT(WINAPI *)(HWND);
  using Awareness = DPI_AWARENESS(WINAPI *)(DPI_AWARENESS_CONTEXT);
  using SystemAwareProcess = BOOL(WINAPI *)();
  using PointTransform = BOOL(WINAPI *)(HWND, LPPOINT);

  WindowDpi query_window = nullptr;
  SystemDpi query_system = nullptr;
  ThreadContext set_thread = nullptr;
  ProcessContext set_process = nullptr;
  WindowContext query_context = nullptr;
  Awareness query_awareness = nullptr;
  SystemAwareProcess set_system_aware_process = ::SetProcessDPIAware;
  PointTransform logical_to_physical = nullptr;
  PointTransform old_logical_to_physical = ::LogicalToPhysicalPoint;

  BOOL physical_point(HWND window, LPPOINT point) const {
    // Win7 没有每屏 DPI 入口，使用其真实坐标转换；新接口失败时不再套一次旧转换。
    return logical_to_physical ? logical_to_physical(window,point) : old_logical_to_physical(window,point);
  }

  static UINT gdi_dpi(HWND window) {
    const HDC dc = GetDC(window);
    if (!dc)
      return USER_DEFAULT_SCREEN_DPI;
    const int value = GetDeviceCaps(dc, LOGPIXELSX);
    ReleaseDC(window, dc);
    return value > 0 ? static_cast<UINT>(value) : USER_DEFAULT_SCREEN_DPI;
  }

  UINT window_dpi(HWND window) const {
    if (query_window)
      return query_window(window);
    // 无效 HWND 在新 API 下返回 0；不能用系统 DPI 把无效窗口伪装成有效窗口。
    return IsWindow(window) ? gdi_dpi(window) : 0;
  }

  UINT system_dpi() const {
    return query_system ? query_system() : gdi_dpi(nullptr);
  }

  DPI_AWARENESS_CONTEXT thread_context(DPI_AWARENESS_CONTEXT context) const {
    if (set_thread)
      return set_thread(context);
    SetLastError(ERROR_CALL_NOT_IMPLEMENTED);
    return nullptr;
  }

  BOOL process_context(DPI_AWARENESS_CONTEXT context) const {
    if (set_process)
      return set_process(context);
    if (context == DPI_AWARENESS_CONTEXT_SYSTEM_AWARE ||
        context == DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE ||
        context == DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
      return set_system_aware_process();
    SetLastError(ERROR_NOT_SUPPORTED);
    return FALSE;
  }

  DPI_AWARENESS_CONTEXT window_context(HWND window) const {
    if (query_context)
      return query_context(window);
    if (!IsWindow(window))
      return nullptr;
    return IsProcessDPIAware() ? DPI_AWARENESS_CONTEXT_SYSTEM_AWARE
                              : DPI_AWARENESS_CONTEXT_UNAWARE;
  }

  DPI_AWARENESS awareness(DPI_AWARENESS_CONTEXT context) const {
    if (query_awareness)
      return query_awareness(context);
    if (context == DPI_AWARENESS_CONTEXT_UNAWARE ||
        context == DPI_AWARENESS_CONTEXT_UNAWARE_GDISCALED)
      return DPI_AWARENESS_UNAWARE;
    if (context == DPI_AWARENESS_CONTEXT_SYSTEM_AWARE)
      return DPI_AWARENESS_SYSTEM_AWARE;
    if (context == DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE ||
        context == DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
      return DPI_AWARENESS_PER_MONITOR_AWARE;
    return DPI_AWARENESS_INVALID;
  }
};

template <class Function>
inline Function user32_function(const char *name) {
  const HMODULE module = GetModuleHandleW(L"user32.dll");
  return module ? reinterpret_cast<Function>(
                      reinterpret_cast<void *>(GetProcAddress(module, name)))
                : nullptr;
}

inline const DpiApi &dpi_api() {
  static const DpiApi api = [] {
    DpiApi loaded;
    loaded.query_window = user32_function<DpiApi::WindowDpi>("GetDpiForWindow");
    loaded.query_system = user32_function<DpiApi::SystemDpi>("GetDpiForSystem");
    loaded.set_thread = user32_function<DpiApi::ThreadContext>("SetThreadDpiAwarenessContext");
    loaded.set_process = user32_function<DpiApi::ProcessContext>("SetProcessDpiAwarenessContext");
    loaded.query_context = user32_function<DpiApi::WindowContext>("GetWindowDpiAwarenessContext");
    loaded.query_awareness = user32_function<DpiApi::Awareness>("GetAwarenessFromDpiAwarenessContext");
    loaded.logical_to_physical = user32_function<DpiApi::PointTransform>("LogicalToPhysicalPointForPerMonitorDPI");
    return loaded;
  }();
  return api;
}

class ThreadDpiScope {
public:
  explicit ThreadDpiScope(const DpiApi &api = dpi_api())
      : api_(api), previous_(api.set_thread ? api.thread_context(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
                                          : nullptr) {
    // 新系统有 API 却切换失败时保留原来的失败处理；旧系统没有线程 DPI，不把系统能力缺失当成窗口故障。
    if (api_.set_thread && !previous_)
      throw std::runtime_error("Per-monitor DPI unavailable");
  }
  ~ThreadDpiScope() {
    if (previous_)
      api_.thread_context(previous_);
  }
  ThreadDpiScope(const ThreadDpiScope &) = delete;
  ThreadDpiScope &operator=(const ThreadDpiScope &) = delete;
  ThreadDpiScope(DpiApi &&) = delete;

private:
  const DpiApi &api_;
  DPI_AWARENESS_CONTEXT previous_;
};

// 只供独立宿主在创建窗口前调用。现代 Windows 继续依赖窗口自己的线程上下文；Win7 进程一次性启用系统 DPI，不能在临时线程作用域中切换它。
inline void initialize_windows7_process_dpi() {
  const auto &api = dpi_api();
  if (!api.set_thread && !IsProcessDPIAware())
    api.process_context(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
}

inline UINT window_dpi(HWND window) { return dpi_api().window_dpi(window); }
inline BOOL physical_point(HWND window, LPPOINT point) { return dpi_api().physical_point(window,point); }
inline UINT system_dpi() { return dpi_api().system_dpi(); }
inline DPI_AWARENESS_CONTEXT thread_context(DPI_AWARENESS_CONTEXT context) {
  return dpi_api().thread_context(context);
}
inline BOOL process_context(DPI_AWARENESS_CONTEXT context) {
  return dpi_api().process_context(context);
}
inline DPI_AWARENESS_CONTEXT window_context(HWND window) {
  return dpi_api().window_context(window);
}
inline DPI_AWARENESS awareness(DPI_AWARENESS_CONTEXT context) {
  return dpi_api().awareness(context);
}

} // namespace msime::windows::compat
