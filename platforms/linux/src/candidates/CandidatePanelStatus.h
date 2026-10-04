#pragma once

#include <sys/stat.h>
#include <unistd.h>

#include <cerrno>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <optional>
#include <string>
#include <string_view>
#include <system_error>

#include "AtomicWrite.h"
#include "../core/LinuxEdition.h"

namespace msime::linux_host {

// Neither Linux host draws its candidate list: IBus hands it to whichever panel the desktop runs and Fcitx5 to whichever user interface it loaded. Some of those ignore the candidate font, colour and skin settings, and only the running host can tell which one is drawing. It says so in a per-session file the settings page reads (HostCapabilities.candidate_panel_limit in client-core host_surface.rs, which parses the same names).
enum class CandidatePanelLimit {
  None,
  // GNOME Shell starts IBus with its panel disabled and draws the popup from the shell theme; it reads neither the panel font nor the colour attributes.
  GnomeShell,
  // The Fcitx5 classic UI shows a theme the user picked, which the host never replaces: colours and skin do not reach it, the font does.
  FcitxTheme,
  // Fcitx5 hands the list to the desktop's Kimpanel, which uses the desktop's own font and theme.
  Kimpanel,
};

inline const char *candidate_panel_limit_name(CandidatePanelLimit limit) {
  switch (limit) {
  case CandidatePanelLimit::GnomeShell:
    return "gnome_shell";
  case CandidatePanelLimit::FcitxTheme:
    return "fcitx_theme";
  case CandidatePanelLimit::Kimpanel:
    return "kimpanel";
  case CandidatePanelLimit::None:
    break;
  }
  return nullptr;
}

// {"host":"ibus"|"fcitx5","limit":<name>|null}; both values are fixed ASCII names, so no escaping is needed.
inline std::string candidate_panel_status_document(std::string_view host, CandidatePanelLimit limit) {
  const char *name = candidate_panel_limit_name(limit);
  return std::string("{\"host\":\"") + std::string(host) + "\",\"limit\":" +
         (name ? "\"" + std::string(name) + "\"" : std::string("null")) + "}\n";
}

// $XDG_RUNTIME_DIR/msime-client/candidate-panel.json, beside the panel input socket. A missing or relative runtime directory yields nothing, as it does for the socket.
inline std::optional<std::filesystem::path> candidate_panel_status_file(const char *runtime) {
  if (!runtime || runtime[0] != '/') return std::nullopt;
  return std::filesystem::path(runtime) / MSIME_EDITION_CLIENT_DIRECTORY / "candidate-panel.json";
}

// XDG_CURRENT_DESKTOP names GNOME Shell's session ("GNOME", "ubuntu:GNOME"). Desktops built on GNOME that run their own panel, and so draw IBus through ibus-ui-gtk3, list GNOME as well and are excluded by their own name.
inline bool candidate_desktop_is_gnome_shell(const char *current_desktop) {
  if (!current_desktop) return false;
  bool gnome = false;
  std::string_view rest(current_desktop);
  while (!rest.empty()) {
    const auto colon = rest.find(':');
    const auto token = rest.substr(0, colon);
    if (token == "GNOME") gnome = true;
    if (token == "GNOME-Flashback" || token == "GNOME-Classic" || token == "Budgie" || token == "Unity" ||
        token == "Pantheon")
      return false;
    if (colon == std::string_view::npos) break;
    rest.remove_prefix(colon + 1);
  }
  return gnome;
}

// Fcitx5 names its active user interface addon: the classic UI honours the MSIME theme unless the user picked another one; Kimpanel draws nothing of it.
inline CandidatePanelLimit fcitx_candidate_panel_limit(std::string_view current_ui, bool theme_replaceable) {
  if (current_ui == "kimpanel") return CandidatePanelLimit::Kimpanel;
  if (current_ui == "classicui" && !theme_replaceable) return CandidatePanelLimit::FcitxTheme;
  return CandidatePanelLimit::None;
}

// Replace the status file atomically when its content changes. The directory is shared with the session's sockets, so it has to belong to this user and admit no one else's writes, the same check the panel input socket makes.
inline bool write_candidate_panel_status(const std::filesystem::path &file, const std::string &document) {
  const auto directory = file.parent_path();
  if (!prepare_candidate_directory(directory)) return false;
  struct stat info {};
  if (::lstat(directory.c_str(), &info) != 0 || !S_ISDIR(info.st_mode) || info.st_uid != ::getuid() ||
      (info.st_mode & 022) != 0)
    return false;
  {
    std::ifstream current(file, std::ios::binary);
    if (current) {
      std::string existing(document.size() + 1, '\0');
      current.read(existing.data(), static_cast<std::streamsize>(existing.size()));
      const auto count = current.gcount();
      if (count == static_cast<std::streamsize>(document.size()) &&
          existing.compare(0, document.size(), document) == 0)
        return true;
    }
  }
  return write_candidate_file_atomically(file, document);
}

} // namespace msime::linux_host
