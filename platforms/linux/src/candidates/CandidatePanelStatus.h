#pragma once

#include <fcntl.h>
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
  // IBus 交给桌面面板（ibus-ui-gtk3 一类）的只有候选文字和 RGB 前景、背景属性，面板字体由宿主写进它的设置：字体和颜色生效，候选窗的圆角、边框和外部皮肤的装饰图都无从表达。只有 Fcitx5 的 classicui 主题画这些，所以 Linux 的能力位照样声明它们，由 IBus 宿主在这里说明自己的面板画不了。
  IbusPanel,
};

inline const char *candidate_panel_limit_name(CandidatePanelLimit limit) {
  switch (limit) {
  case CandidatePanelLimit::GnomeShell:
    return "gnome_shell";
  case CandidatePanelLimit::FcitxTheme:
    return "fcitx_theme";
  case CandidatePanelLimit::Kimpanel:
    return "kimpanel";
  case CandidatePanelLimit::IbusPanel:
    return "ibus_panel";
  case CandidatePanelLimit::None:
    break;
  }
  return nullptr;
}

// {"host":"ibus"|"fcitx5","limit":<name>|null}; both values are fixed ASCII names, so no escaping is needed. `native_canvas` 为真时再加 `"renderer":"native_canvas"`：Fcitx5 在 X11 会话里用自己的原生画布按 macOS 的结构画候选窗，设置页据此在 Linux 上也提供展开候选、拼音字号和选中外观（client-core host_surface.rs 的 `linux_native_canvas_renderer`）。
inline std::string candidate_panel_status_document(std::string_view host, CandidatePanelLimit limit,
                                                   bool native_canvas = false) {
  const char *name = candidate_panel_limit_name(limit);
  return std::string("{\"host\":\"") + std::string(host) + "\",\"limit\":" +
         (name ? "\"" + std::string(name) + "\"" : std::string("null")) +
         (native_canvas ? ",\"renderer\":\"native_canvas\"" : "") + "}\n";
}

// Fcitx5 的候选窗这次会话里是不是原生画布画的：会话是 X11。XDG_SESSION_TYPE 是 x11 或 wayland 时照它；是别的值（控制台登录后 startx 时为 tty）或没设时看有没有 DISPLAY 而没有 WAYLAND_DISPLAY。Wayland 会话里走 XWayland 的程序也不用原生画布画水杉主题（FcitxEngine 用同一个判断），所以不算。
inline bool candidate_session_is_x11(const char *session_type, const char *display, const char *wayland_display) {
  const std::string_view type = session_type ? session_type : "";
  if (type == "x11") return true;
  if (type == "wayland") return false;
  return display && display[0] && !(wayland_display && wayland_display[0]);
}

// $XDG_RUNTIME_DIR/msime-client/candidate-panel.json, beside the panel input socket. A missing or relative runtime directory yields nothing, as it does for the socket.
inline std::optional<std::filesystem::path> candidate_panel_status_file(const char *runtime) {
  if (!runtime || runtime[0] != '/') return std::nullopt;
  return std::filesystem::path(runtime) / MSIME_EDITION_CLIENT_DIRECTORY / "candidate-panel.json";
}

// XDG_CURRENT_DESKTOP 标识 Shell 会话（GNOME、ubuntu:GNOME、GNOME-Classic:GNOME）。Classic 是 Shell 扩展组合；Flashback、Budgie 等自带面板的桌面也可能带 GNOME，按自身名称优先排除。
inline bool candidate_desktop_is_gnome_shell(const char *current_desktop) {
  if (!current_desktop) return false;
  bool gnome = false;
  std::string_view rest(current_desktop);
  while (!rest.empty()) {
    const auto colon = rest.find(':');
    const auto token = rest.substr(0, colon);
    if (token == "GNOME" || token == "GNOME-Classic") gnome = true;
    if (token == "GNOME-Flashback" || token == "Budgie" || token == "Unity" ||
        token == "Pantheon")
      return false;
    if (colon == std::string_view::npos) break;
    rest.remove_prefix(colon + 1);
  }
  return gnome;
}

// IBus 宿主的结论：GNOME Shell 自己画候选弹窗，字体、颜色和皮肤都不读；别的桌面上 IBus 面板读字体和颜色，画不了圆角、边框和装饰图。IBus 没有全部生效的情形，所以从不报告 None。
inline CandidatePanelLimit ibus_candidate_panel_limit(bool gnome_shell) {
  return gnome_shell ? CandidatePanelLimit::GnomeShell : CandidatePanelLimit::IbusPanel;
}

// Fcitx5 names its active user interface addon: the classic UI honours the MSIME theme unless the user picked another one; Kimpanel draws nothing of it.
inline CandidatePanelLimit fcitx_candidate_panel_limit(std::string_view current_ui, bool theme_replaceable) {
  if (current_ui == "kimpanel") return CandidatePanelLimit::Kimpanel;
  if (current_ui == "classicui" && !theme_replaceable) return CandidatePanelLimit::FcitxTheme;
  return CandidatePanelLimit::None;
}

// Replace the status file atomically when its content changes. The directory is shared with the session's sockets, so it has to belong to this user and admit no one else's writes, the same check the panel input socket makes. Whoever comes first creates it, so it is created 0700 here as the socket does: `create_directories` would follow the umask, and under the common 002 the directory comes out group-writable and every later check refuses it.
inline bool write_candidate_panel_status(const std::filesystem::path &file, const std::string &document) {
  const auto directory = file.parent_path();
  if (!candidate_directory_path_is_safe(directory)) return false;
  if (::mkdir(directory.c_str(), 0700) != 0 && errno != EEXIST) return false;
  struct stat info {};
  if (::lstat(directory.c_str(), &info) != 0 || !S_ISDIR(info.st_mode) || info.st_uid != ::getuid() ||
      (info.st_mode & 022) != 0)
    return false;
  {
    const int descriptor = ::open(file.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK);
    if (descriptor >= 0) {
      struct CloseOnExit {
        int descriptor;
        ~CloseOnExit() { ::close(descriptor); }
      } close_on_exit{descriptor};
      struct stat metadata {};
      if (::fstat(descriptor, &metadata) == 0 && S_ISREG(metadata.st_mode)) {
        std::string existing(document.size() + 1, '\0');
        ssize_t count = 0;
        for (;;) {
          count = ::read(descriptor, existing.data(), existing.size());
          if (count < 0 && errno == EINTR) continue;
          break;
        }
        if (count >= 0 && count == static_cast<ssize_t>(document.size()) &&
            existing.compare(0, document.size(), document) == 0)
          return true;
      }
    }
  }
  return write_candidate_file_atomically(file, document);
}

} // namespace msime::linux_host
