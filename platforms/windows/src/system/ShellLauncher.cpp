#include "ShellLauncher.h"
#include <memory>
#include <windows.h>

namespace msime::windows {
namespace {
struct ForegroundWindow { DWORD pid; HWND window = nullptr; };
BOOL CALLBACK find_process_window(HWND window, LPARAM data) {
  auto *result = reinterpret_cast<ForegroundWindow *>(data);
  DWORD pid = 0;
  GetWindowThreadProcessId(window, &pid);
  if (pid == result->pid && IsWindowVisible(window) && GetWindow(window, GW_OWNER) == nullptr) {
    result->window = window;
    return FALSE;
  }
  return TRUE;
}

bool launch_shell_surface_impl(const std::filesystem::path &executable,
                               const ShellSurfaceRequest &request,
                               const ShellLaunchContext *context) {
  if (executable.empty() || !executable.is_absolute())
    return false;
  std::unique_ptr<wchar_t, decltype(&FreeEnvironmentStringsW)> inherited(
      GetEnvironmentStringsW(), FreeEnvironmentStringsW);
  if (!inherited)
    return false;
  std::wstring environment;
  try {
    environment = shell_environment_block(inherited.get(), request, context);
  } catch (const std::invalid_argument &) {
    return false;
  }
  // CreateProcessW may write to the command line buffer, so it owns a copy.
  std::wstring command_line = L"\"" + executable.wstring() + L"\" --route=";
  try {
    command_line += shell_route_argument(request);
  } catch (const std::invalid_argument &) {
    return false;
  }
  STARTUPINFOW startup{};
  startup.cb = sizeof(startup);
  startup.dwFlags = STARTF_USESHOWWINDOW;
  startup.wShowWindow = SW_SHOWNORMAL;
  PROCESS_INFORMATION process{};
  const auto directory = executable.parent_path().wstring();
  if (!CreateProcessW(executable.c_str(), command_line.data(), nullptr, nullptr,
                      FALSE, CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW,
                      environment.data(), directory.c_str(), &startup,
                      &process))
    return false;
  // Give the newly requested surface a chance to appear even when another
  // shell instance handles the route through the single-instance plugin.
  WaitForInputIdle(process.hProcess, 1500);
  ForegroundWindow foreground{process.dwProcessId};
  EnumWindows(find_process_window, reinterpret_cast<LPARAM>(&foreground));
  if (foreground.window) {
    ShowWindow(foreground.window, SW_RESTORE);
    SetForegroundWindow(foreground.window);
  }
  // The shell owns its own lifetime; this process only started it.
  CloseHandle(process.hThread);
  CloseHandle(process.hProcess);
  return true;
}
} // namespace

bool launch_shell_surface(const std::filesystem::path &executable,
                          const ShellSurfaceRequest &request) {
  return launch_shell_surface_impl(executable, request, nullptr);
}

bool launch_shell_surface(const std::filesystem::path &executable,
                          const ShellSurfaceRequest &request,
                          const ShellLaunchContext &context) {
  return launch_shell_surface_impl(executable, request, &context);
}

PanelHost configured_panel_host() {
  wchar_t value[16]{};
  const DWORD length = GetEnvironmentVariableW(panel_host_variable, value, 16);
  if (!length || length >= 16)
    return PanelHost::Shared;
  return panel_host_from_setting(std::wstring_view(value, length));
}
} // namespace msime::windows
