#include "WaveOverlayUtils.h"

#include "WaveOverlayScale.h"

#include <atomic>

namespace msime::windows {
namespace {
using GetDpiForMonitorFn = HRESULT(WINAPI *)(HMONITOR, int /*MONITOR_DPI_TYPE*/,
                                             UINT *, UINT *);

// Resolved lazily rather than linked: shcore.dll is the only home of a
// per-HMONITOR DPI query, and binding to it at load time would add an import
// the rest of the server does not need. A failed lookup is not cached; it is
// cheap enough to retry on the next recording.
std::atomic<GetDpiForMonitorFn> get_dpi_for_monitor{nullptr};

GetDpiForMonitorFn resolve_get_dpi_for_monitor() {
  GetDpiForMonitorFn resolved =
      get_dpi_for_monitor.load(std::memory_order_acquire);
  if (resolved)
    return resolved;
  HMODULE shcore = GetModuleHandleW(L"shcore.dll");
  if (!shcore)
    shcore = LoadLibraryW(L"shcore.dll"); // system DLL, kept until process exit
  if (!shcore)
    return nullptr;
  // Through void*: GetProcAddress returns a generic FARPROC, and casting that
  // straight to the real signature is what -Wcast-function-type rejects.
  resolved = reinterpret_cast<GetDpiForMonitorFn>(
      reinterpret_cast<void *>(GetProcAddress(shcore, "GetDpiForMonitor")));
  if (resolved)
    get_dpi_for_monitor.store(resolved, std::memory_order_release);
  return resolved;
}
} // namespace

UINT monitor_effective_dpi(HMONITOR monitor) {
  UINT dpi_x = 0;
  UINT dpi_y = 0;
  const GetDpiForMonitorFn query = resolve_get_dpi_for_monitor();
  if (query && SUCCEEDED(query(monitor, 0 /* MDT_EFFECTIVE_DPI */, &dpi_x,
                               &dpi_y)))
    return wave_overlay_dpi(dpi_x, GetDpiForSystem());
  return wave_overlay_dpi(0, GetDpiForSystem());
}

bool wave_overlay_monitor_metrics(WaveOverlayMonitorMetrics *metrics) {
  if (!metrics)
    return false;
  const WaveOverlayDpiScope dpi_scope;
  const HWND foreground = GetForegroundWindow();
  const HMONITOR monitor = MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST);
  MONITORINFO info{};
  info.cbSize = sizeof(info);
  if (!monitor || !GetMonitorInfoW(monitor, &info))
    return false;
  metrics->monitor = info.rcMonitor;
  metrics->work = info.rcWork;
  metrics->dpi = monitor_effective_dpi(monitor);
  return true;
}
} // namespace msime::windows
