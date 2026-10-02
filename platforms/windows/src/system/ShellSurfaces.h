#pragma once
#include "TrayMenuLayout.h"
#include <cwctype>
#include <filesystem>
#include <optional>
#include <stdexcept>
#include <string>
#include <vector>

namespace msime::windows {
// What a tray entry asks a desktop surface to open. Settings uses the native WinUI 3 process; panels use the shared Tauri process. Both hosts read the same `--route=` argument, so routing remains one small protocol.
struct ShellSurfaceRequest {
  // Panel route head. Empty opens the settings window itself.
  std::string panel;
  // Settings category. Empty keeps the settings default section.
  std::string page;
};
// Paths are supplied by the Server that owns the prepared host document. The
// settings shell must use the same store; its default Tauri app-data directory
// is unrelated to the input method state directory.
struct ShellLaunchContext {
  std::filesystem::path state_root;
  std::filesystem::path host_options;
};
// The floating toolbar, the input modes and the stored switches belong to this process, so they are the rows the shell never hears about. 主题 and 词库… open their pages of the settings app by the ids it already routes.
inline std::optional<ShellSurfaceRequest>
shell_surface_request(TrayMenuCommand command) {
  switch (command) {
  case TrayMenuCommand::OpenEmojiPanel:
    return ShellSurfaceRequest{"emoji", {}};
  case TrayMenuCommand::OpenHandwritingPanel:
    return ShellSurfaceRequest{"handwriting", {}};
  case TrayMenuCommand::OpenKeyboardPanel:
    return ShellSurfaceRequest{"keyboard", {}};
  case TrayMenuCommand::ToggleVoiceInput:
    return ShellSurfaceRequest{"voice", {}};
  case TrayMenuCommand::OpenSettings:
    return ShellSurfaceRequest{{}, {}};
  case TrayMenuCommand::OpenAbout:
    return ShellSurfaceRequest{{}, "about"};
  case TrayMenuCommand::OpenTheme:
    return ShellSurfaceRequest{{}, "skin"};
  case TrayMenuCommand::OpenDictionary:
    return ShellSurfaceRequest{{}, "dictionary"};
  case TrayMenuCommand::ToggleFloatingToolbar:
  case TrayMenuCommand::SelectChinese:
  case TrayMenuCommand::SelectEnglish:
  case TrayMenuCommand::ToggleFullwidth:
  case TrayMenuCommand::ToggleChinesePunctuation:
  case TrayMenuCommand::ToggleTranslations:
  case TrayMenuCommand::SelectQuanpin:
  case TrayMenuCommand::SelectShuangpin:
  case TrayMenuCommand::SelectWubi:
  case TrayMenuCommand::SelectJapanese:
  case TrayMenuCommand::SelectKorean:
  case TrayMenuCommand::SelectCantonese:
  case TrayMenuCommand::SelectZhuyin:
  case TrayMenuCommand::SelectVietnamese:
    break;
  }
  return std::nullopt;
}
// The cross-platform surface route the shell parses (client-core host_surface::SurfaceRoute). A settings section travels as "settings:<category>"; the bare section name is not a route head and would be rejected.
inline std::string shell_surface_route(const ShellSurfaceRequest &request) {
  if (!request.page.empty())
    return "settings:" + request.page;
  if (request.panel.empty())
    return "settings";
  return request.panel;
}

// File names the package stages beside the Server. Settings is a native WinUI
// 3 process; the shared Tauri process remains the owner of the other panels.
inline std::vector<std::wstring>
shell_executable_names(const ShellSurfaceRequest &request) {
  if (request.panel.empty())
    return {L"msime-client-settings.exe"};
  return {L"MSIME.exe"};
}
inline std::optional<std::filesystem::path>
shell_executable(const std::filesystem::path &directory,
                 const std::wstring &configured,
                 const ShellSurfaceRequest &request) {
  std::error_code error;
  if (request.panel.empty() && !configured.empty()) {
    const std::filesystem::path path(configured);
    if (!path.is_absolute() || !std::filesystem::is_regular_file(path, error))
      return std::nullopt;
    return path;
  }
  if (directory.empty() || !directory.is_absolute())
    return std::nullopt;
  for (const auto &name : shell_executable_names(request)) {
    const auto path = directory / name;
    if (std::filesystem::is_regular_file(path, error))
      return path;
  }
  return std::nullopt;
}
// Compose the child environment from this process's block plus the request. Existing MSIME_CLIENT_ROUTE and the two context entries are dropped, so a value this process was started with cannot outvote the clicked row.
// The result is the double-NUL terminated block CreateProcessW expects.
inline std::wstring shell_environment_block(const wchar_t *existing,
                                            const ShellSurfaceRequest &request,
                                            const ShellLaunchContext *context) {
  static constexpr std::wstring_view state_name = L"MSIME_CLIENT_STATE_DIR=";
  static constexpr std::wstring_view options_name = L"MSIME_CLIENT_HOST_OPTIONS=";
  static constexpr std::wstring_view route_name = L"MSIME_CLIENT_ROUTE=";
  auto owned = [](std::wstring_view entry) {
    auto starts_with = [entry](std::wstring_view name) {
      if (entry.size() < name.size())
        return false;
      for (size_t i = 0; i < name.size(); ++i)
        if (towupper(entry[i]) != towupper(name[i]))
          return false;
      return true;
    };
    return starts_with(state_name) || starts_with(options_name) ||
           starts_with(route_name);
  };
  std::wstring block;
  for (const wchar_t *entry = existing; entry && *entry;) {
    const std::wstring_view value(entry);
    // A leading '=' names a drive's current directory, which the child needs.
    if (!owned(value)) {
      block.append(value);
      block.push_back(L'\0');
    }
    entry += value.size() + 1;
  }
  // The contract only carries short lowercase ASCII identifiers; nothing a caller could turn into another variable or a command line. A route may additionally carry one ':' separating the surface from its section.
  auto validate = [](const std::string &value, bool allow_separator) {
    if (value.size() > 64)
      throw std::invalid_argument("Invalid shell surface request");
    size_t separators = 0;
    for (unsigned char c : value) {
      if (c == ':' && allow_separator) {
        if (++separators > 1)
          throw std::invalid_argument("Invalid shell surface request");
      } else if (!((c >= 'a' && c <= 'z') || c == '-')) {
        throw std::invalid_argument("Invalid shell surface request");
      }
    }
  };
  validate(request.panel, false);
  validate(request.page, false);
  const auto route = shell_surface_route(request);
  validate(route, true);
  block.append(route_name);
  block.append(route.begin(), route.end());
  block.push_back(L'\0');
  if (context) {
    if (!context->state_root.is_absolute() || !context->host_options.is_absolute())
      throw std::invalid_argument("Invalid shell launch context");
    block.append(state_name);
    block.append(context->state_root.wstring());
    block.push_back(L'\0');
    block.append(options_name);
    block.append(context->host_options.wstring());
    block.push_back(L'\0');
  }
  block.push_back(L'\0');
  return block;
}
inline std::wstring shell_environment_block(const wchar_t *existing,
                                            const ShellSurfaceRequest &request) {
  return shell_environment_block(existing, request, nullptr);
}

// The Tauri shell uses the command-line route when a second launch is handed to the already-running instance. Keep the route in the same short ASCII vocabulary as the environment contract.
inline std::wstring shell_route_argument(const ShellSurfaceRequest &request) {
  std::string route;
  if (!request.panel.empty())
    route = request.panel;
  else if (!request.page.empty())
    route = "settings:" + request.page;
  else
    route = "settings";
  if (route.size() > 64 || route.find_first_not_of(
                              "abcdefghijklmnopqrstuvwxyz0123456789-:") !=
          std::string::npos)
    throw std::invalid_argument("Invalid shell surface route");
  return std::wstring(route.begin(), route.end());
}
} // namespace msime::windows
