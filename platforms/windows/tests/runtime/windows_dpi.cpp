#include "../../compat/WindowsDpi.h"
#include "../../compat/WindowsVersion.h"

#include <cassert>
#include <cstdio>

namespace {
int system_aware_calls = 0;
DPI_AWARENESS_CONTEXT received_context = nullptr;
HWND received_window = nullptr;
LPPOINT received_point = nullptr;
int old_point_calls = 0;
BOOL WINAPI native_point(HWND window, LPPOINT point) {
  received_window = window; received_point = point; SetLastError(ERROR_ACCESS_DENIED); return FALSE;
}
BOOL WINAPI old_point(HWND window, LPPOINT point) {
  ++old_point_calls; received_window = window; received_point = point; point->x += 17; return TRUE;
}

UINT WINAPI native_window_dpi(HWND window) {
  received_window = window;
  SetLastError(ERROR_ACCESS_DENIED);
  return 0;
}
UINT WINAPI native_system_dpi() { return 168; }
DPI_AWARENESS_CONTEXT WINAPI native_thread_context(DPI_AWARENESS_CONTEXT context) {
  received_context = context;
  return DPI_AWARENESS_CONTEXT_SYSTEM_AWARE;
}
DPI_AWARENESS_CONTEXT WINAPI failed_thread_context(DPI_AWARENESS_CONTEXT) {
  SetLastError(ERROR_ACCESS_DENIED);
  return nullptr;
}
BOOL WINAPI native_process_context(DPI_AWARENESS_CONTEXT context) {
  received_context = context;
  SetLastError(ERROR_ACCESS_DENIED);
  return FALSE;
}
BOOL WINAPI system_aware_process() {
  ++system_aware_calls;
  return TRUE;
}
DPI_AWARENESS_CONTEXT WINAPI native_window_context(HWND window) {
  received_window = window;
  return DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2;
}
DPI_AWARENESS WINAPI native_awareness(DPI_AWARENESS_CONTEXT context) {
  received_context = context;
  return DPI_AWARENESS_PER_MONITOR_AWARE;
}
} // namespace

int main() {
  using msime::windows::compat::DpiApi;

  // 真实 HWND 验证无新 API 的路径，包括无效窗口和 GDI 查询；查询结果不能把旧系统固定为 96 DPI。
  // 真实创建产品使用的窗口样式，防止 Win7 在 DPI 查询前就被新样式拒绝。
  const HWND window = CreateWindowExW(msime::windows::compat::no_redirection_bitmap_style(), L"STATIC", L"dpi-fixture", WS_POPUP,
                                      0, 0, 64, 64, nullptr, nullptr,
                                      GetModuleHandleW(nullptr), nullptr);
  assert(window);
  DpiApi windows7;
  POINT anchor{3,5};
  windows7.old_logical_to_physical = old_point;
  assert(windows7.physical_point(window,&anchor));
  assert(old_point_calls == 1 && anchor.x == 20 && received_window == window && received_point == &anchor);
  DpiApi actual_old;
  POINT native_anchor{10,10}, compat_anchor = native_anchor;
  const BOOL native_result = LogicalToPhysicalPoint(window,&native_anchor);
  assert(actual_old.physical_point(window,&compat_anchor) == native_result);
  assert(native_anchor.x == compat_anchor.x && native_anchor.y == compat_anchor.y);
  windows7.set_system_aware_process = system_aware_process;
  assert(windows7.window_dpi(nullptr) == 0);
  assert(windows7.window_context(nullptr) == nullptr);
  const HDC dc = GetDC(window);
  assert(dc);
  const UINT expected = static_cast<UINT>(GetDeviceCaps(dc, LOGPIXELSX));
  ReleaseDC(window, dc);
  assert(expected > 0);
  assert(windows7.window_dpi(window) == expected);
  assert(windows7.system_dpi() > 0);
  assert(windows7.window_context(window) ==
         (IsProcessDPIAware() ? DPI_AWARENESS_CONTEXT_SYSTEM_AWARE
                             : DPI_AWARENESS_CONTEXT_UNAWARE));
  const BOOL original_awareness = IsProcessDPIAware();
  assert(windows7.thread_context(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) == nullptr);
  assert(GetLastError() == ERROR_CALL_NOT_IMPLEMENTED);
  assert(IsProcessDPIAware() == original_awareness);
  assert(system_aware_calls == 0);
  {
    msime::windows::compat::ThreadDpiScope scope(windows7);
    assert(IsProcessDPIAware() == original_awareness);
    assert(system_aware_calls == 0);
  }
  assert(windows7.process_context(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2));
  assert(system_aware_calls == 1);
  assert(!windows7.process_context(nullptr));
  assert(system_aware_calls == 1);
  assert(windows7.awareness(nullptr) == DPI_AWARENESS_INVALID);
  assert(windows7.awareness(DPI_AWARENESS_CONTEXT_UNAWARE) == DPI_AWARENESS_UNAWARE);
  assert(windows7.awareness(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE) == DPI_AWARENESS_SYSTEM_AWARE);
  assert(windows7.awareness(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) == DPI_AWARENESS_PER_MONITOR_AWARE);

  // 新系统分支必须保留错误、零结果及上下文令牌；新 API 调用失败时不能再改一次进程状态。
  DpiApi modern;
  modern.logical_to_physical = native_point; modern.old_logical_to_physical = old_point;
  assert(!modern.physical_point(window,&anchor) && GetLastError() == ERROR_ACCESS_DENIED);
  assert(received_window == window && received_point == &anchor && old_point_calls == 1);
  modern.query_window = native_window_dpi;
  modern.query_system = native_system_dpi;
  modern.set_thread = native_thread_context;
  modern.set_process = native_process_context;
  modern.query_context = native_window_context;
  modern.query_awareness = native_awareness;
  modern.set_system_aware_process = system_aware_process;
  assert(modern.window_dpi(window) == 0);
  assert(received_window == window);
  assert(GetLastError() == ERROR_ACCESS_DENIED);
  assert(modern.system_dpi() == 168);
  assert(modern.thread_context(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) ==
         DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
  assert(received_context == DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
  assert(!modern.process_context(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2));
  assert(GetLastError() == ERROR_ACCESS_DENIED);
  assert(system_aware_calls == 1);
  assert(modern.window_context(window) == DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
  assert(received_window == window);
  assert(modern.awareness(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE) == DPI_AWARENESS_PER_MONITOR_AWARE);
  assert(received_context == DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
  {
    msime::windows::compat::ThreadDpiScope scope(modern);
    assert(received_context == DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
  }
  assert(received_context == DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
  modern.set_thread = failed_thread_context;
  bool failed = false;
  try {
    msime::windows::compat::ThreadDpiScope scope(modern);
  } catch (const std::runtime_error &) {
    failed = true;
  }
  assert(failed);

  // 系统实际导出也经同一入口调用，并核对线程作用域恢复；Win7 缺失入口时仍验证回退。
  const auto &actual = msime::windows::compat::dpi_api();
  assert(actual.window_dpi(window) > 0);
  assert(actual.system_dpi() > 0);
  const auto previous = actual.thread_context(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
  if (previous)
    assert(actual.thread_context(previous) != nullptr);
  else
    assert(GetLastError() == ERROR_CALL_NOT_IMPLEMENTED ||
           GetLastError() == ERROR_INVALID_PARAMETER);

  assert(DestroyWindow(window));
  assert(windows7.window_dpi(window) == 0);
  std::puts("Windows DPI native forwarding and Windows 7 fallback passed");
}
