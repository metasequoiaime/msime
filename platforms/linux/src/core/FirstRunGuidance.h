#pragma once

#include <filesystem>
#include <string_view>

#include "LinuxEdition.h"

namespace msime::linux_host {

// On Windows the installer prepares the data directory, so the input method is ready the moment it can be selected. A Linux package installs no user state; until msime-linux-setup (or the settings window's first-run page) publishes runtime-options.json, selecting MSIME cannot open a session. The frontends tell that state apart from a broken configuration and point the user at setup instead of a generic error.
// 产品名和首次配置命令按版本取（LinuxEdition.h）：五笔版要叫用户运行 msime-linux-wubi-setup，而不是 full 的那个。
inline constexpr std::string_view kFirstRunHint =
    MSIME_EDITION_DISPLAY_NAME "尚未完成首次配置：请打开「" MSIME_EDITION_DISPLAY_NAME "」设置，或在终端运行 " MSIME_EDITION_SETUP_PROGRAM;

// Installed beside the IBus launcher; it opens the settings window and posts a notification at most once per login session. Both frontends call the same script so that limit is shared between them.
inline constexpr std::string_view kFirstRunGuideProgram = "msime-linux-first-run-guide";

enum class RuntimeOptionsState { Found, NotConfigured, Invalid };

struct RuntimeOptionsLocation {
  RuntimeOptionsState state;
  std::filesystem::path path;
};

// Where the Fcitx5 addon reads its runtime options, matching the IBus launcher: an explicit override is taken as given (and must be absolute), otherwise the user locator under XDG_CONFIG_HOME (or HOME/.config), and only an absent user file falls back to the system file. Neither file existing is the one state reported as NotConfigured; an existing but unreadable file or a dangling symlink is Found and fails later as a broken configuration, as before. Filesystem errors propagate.
inline RuntimeOptionsLocation locate_runtime_options(const char *override_path,
                                                     const char *xdg_config_home,
                                                     const char *home,
                                                     const std::filesystem::path &system_path) {
  if (override_path) {
    std::filesystem::path path(override_path);
    if (!path.is_absolute()) return {RuntimeOptionsState::Invalid, {}};
    return {RuntimeOptionsState::Found, path};
  }
  std::filesystem::path path = xdg_config_home && *xdg_config_home ? std::filesystem::path(xdg_config_home)
                               : home && *home ? std::filesystem::path(home) / ".config"
                                               : std::filesystem::path();
  if (!path.is_absolute()) return {RuntimeOptionsState::Invalid, {}};
  path /= MSIME_EDITION_CLIENT_DIRECTORY "/runtime-options.json";
  if (std::filesystem::exists(path) || std::filesystem::is_symlink(path))
    return {RuntimeOptionsState::Found, path};
  if (!system_path.is_absolute()) return {RuntimeOptionsState::Invalid, {}};
  if (std::filesystem::exists(system_path) || std::filesystem::is_symlink(system_path))
    return {RuntimeOptionsState::Found, system_path};
  return {RuntimeOptionsState::NotConfigured, system_path};
}

}  // namespace msime::linux_host
