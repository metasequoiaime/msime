#pragma once

#include <array>
#include <string_view>

namespace msime::windows::win7 {
// 原生面板宿主认的 `--route=` 面板路由。Win7 包的 msime-client-panels-win7.exe 和现代包的 msime-client-panels.exe 是同一份 main.cpp，认同一组路由；托盘、悬浮工具栏和 WinUI 设置在 MSIME_WINDOWS_PANEL_HOST=native 时把面板请求交给它，所以它们发出的每个面板路由都必须在这里（tests/ui/shell_surfaces.cpp 和 settings_navigation.cpp 核对）。没有显式路由时打开手写。
inline constexpr std::string_view default_panel_route = "handwriting";
inline constexpr std::array<std::string_view, 9> panel_routes{{
    "handwriting", "clipboard", "cloud-clipboard", "vocabulary", "cloud-dictionary",
    "dictionary-sync", "emoji", "keyboard", "voice"}};
constexpr bool panel_route(std::string_view route) {
  for (const auto candidate : panel_routes)
    if (candidate == route)
      return true;
  return false;
}
} // namespace msime::windows::win7
