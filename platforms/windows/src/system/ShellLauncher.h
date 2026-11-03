#pragma once
#include "ShellSurfaces.h"

namespace msime::windows {
// Start the shared desktop shell on the requested surface. The tray menu is the
// only caller: a click either reaches the shell or the row stays disabled, so
// this never reports success it did not observe. Returns false when the shell
// could not be started; it does not wait for the surface to appear.
bool launch_shell_surface(const std::filesystem::path &executable,
                          const ShellSurfaceRequest &request);
bool launch_shell_surface(const std::filesystem::path &executable,
                          const ShellSurfaceRequest &request,
                          const ShellLaunchContext &context);
// 本进程环境里 MSIME_WINDOWS_PANEL_HOST 选定的面板宿主，按 panel_host_from_setting 解析；读不到或值超长时按 Shared。
PanelHost configured_panel_host();
} // namespace msime::windows
