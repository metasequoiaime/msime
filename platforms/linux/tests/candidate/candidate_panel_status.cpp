#include "../src/candidates/CandidatePanelStatus.h"

#include <cassert>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <sys/stat.h>

int main() {
  using namespace msime::linux_host;
  // The names must match client-core's CandidatePanelLimit serde names.
  assert(candidate_panel_status_document("ibus", CandidatePanelLimit::GnomeShell) ==
         "{\"host\":\"ibus\",\"limit\":\"gnome_shell\"}\n");
  assert(candidate_panel_status_document("fcitx5", CandidatePanelLimit::FcitxTheme) ==
         "{\"host\":\"fcitx5\",\"limit\":\"fcitx_theme\"}\n");
  assert(candidate_panel_status_document("fcitx5", CandidatePanelLimit::Kimpanel) ==
         "{\"host\":\"fcitx5\",\"limit\":\"kimpanel\"}\n");
  assert(candidate_panel_status_document("ibus", CandidatePanelLimit::IbusPanel) ==
         "{\"host\":\"ibus\",\"limit\":\"ibus_panel\"}\n");
  assert(candidate_panel_status_document("ibus", CandidatePanelLimit::None) ==
         "{\"host\":\"ibus\",\"limit\":null}\n");
  // Fcitx5 在 X11 会话里用原生画布画候选窗时加报 renderer，client-core 按这个名字读。
  assert(candidate_panel_status_document("fcitx5", CandidatePanelLimit::None, true) ==
         "{\"host\":\"fcitx5\",\"limit\":null,\"renderer\":\"native_canvas\"}\n");
  assert(candidate_session_is_x11("x11", nullptr, nullptr));
  assert(!candidate_session_is_x11("wayland", ":0", nullptr));
  assert(candidate_session_is_x11(nullptr, ":0", nullptr));
  assert(!candidate_session_is_x11(nullptr, ":0", "wayland-0"));
  assert(!candidate_session_is_x11("", nullptr, nullptr));
  // 控制台登录后 startx：pam_systemd 给的是 tty，按 DISPLAY 判断。
  assert(candidate_session_is_x11("tty", ":0", nullptr));
  assert(!candidate_session_is_x11("tty", nullptr, nullptr));
  assert(!candidate_session_is_x11("tty", ":0", "wayland-0"));

  assert(candidate_desktop_is_gnome_shell("GNOME"));
  assert(candidate_desktop_is_gnome_shell("ubuntu:GNOME"));
  // Classic 仍由 GNOME Shell 扩展绘制面板；正式会话同时给出这两个桌面标识。
  assert(candidate_desktop_is_gnome_shell("GNOME-Classic:GNOME"));
  assert(candidate_desktop_is_gnome_shell("GNOME:GNOME-Classic"));
  assert(candidate_desktop_is_gnome_shell("GNOME-Classic"));
  assert(!candidate_desktop_is_gnome_shell("GNOME-Classic:GNOME-Flashback:GNOME"));
  assert(!candidate_desktop_is_gnome_shell("Budgie:GNOME-Classic"));
  assert(!candidate_desktop_is_gnome_shell("GNOME-Flashback:GNOME"));
  assert(!candidate_desktop_is_gnome_shell("Budgie:GNOME"));
  assert(!candidate_desktop_is_gnome_shell("Unity"));
  assert(!candidate_desktop_is_gnome_shell("KDE"));
  assert(!candidate_desktop_is_gnome_shell("X-Cinnamon"));
  assert(!candidate_desktop_is_gnome_shell(""));
  assert(!candidate_desktop_is_gnome_shell(nullptr));

  assert(ibus_candidate_panel_limit(true) == CandidatePanelLimit::GnomeShell);
  assert(ibus_candidate_panel_limit(false) == CandidatePanelLimit::IbusPanel);

  assert(fcitx_candidate_panel_limit("kimpanel", true) == CandidatePanelLimit::Kimpanel);
  assert(fcitx_candidate_panel_limit("classicui", false) == CandidatePanelLimit::FcitxTheme);
  assert(fcitx_candidate_panel_limit("classicui", true) == CandidatePanelLimit::None);
  assert(fcitx_candidate_panel_limit("", false) == CandidatePanelLimit::None);

  assert(!candidate_panel_status_file(nullptr));
  assert(!candidate_panel_status_file("relative"));
  assert(*candidate_panel_status_file("/run/user/1000") ==
         std::filesystem::path("/run/user/1000/msime-client/candidate-panel.json"));

  // Synthetic runtime directory under the system temporary directory.
  auto root = std::filesystem::temp_directory_path() / ("msime-panel-status-" + std::to_string(::getpid()));
  std::filesystem::remove_all(root);
  std::filesystem::create_directories(root);
  const auto file = *candidate_panel_status_file(root.c_str());
  const auto document = candidate_panel_status_document("ibus", CandidatePanelLimit::GnomeShell);
  assert(write_candidate_panel_status(file, document));
  assert(write_candidate_panel_status(file, document));
  {
    std::ifstream in(file, std::ios::binary);
    assert(std::string(std::istreambuf_iterator<char>(in), {}) == document);
  }
  assert(!std::filesystem::exists(std::filesystem::path(file.string() + ".new")));
  std::filesystem::remove(file);
  assert(::mkfifo(file.c_str(), 0600) == 0);
  assert(write_candidate_panel_status(file, document));
  std::filesystem::remove(file);
  assert(write_candidate_panel_status(file, document));
  {
    const auto outside = root / "outside-status.json";
    std::ofstream(outside) << "keep";
    std::filesystem::create_symlink(outside, file.string() + ".new");
    assert(write_candidate_panel_status(file, candidate_panel_status_document("ibus", CandidatePanelLimit::None)));
    std::ifstream in(outside);
    assert(std::string(std::istreambuf_iterator<char>(in), {}) == "keep");
  }
  assert((std::filesystem::status(file.parent_path()).permissions() & std::filesystem::perms::group_write) ==
         std::filesystem::perms::none);
  {
    std::ofstream oversized(file, std::ios::binary | std::ios::trunc);
    oversized << std::string(8 * 1024 * 1024, 'x');
  }
  assert(write_candidate_panel_status(file, document));
  {
    std::ifstream in(file, std::ios::binary);
    assert(std::string(std::istreambuf_iterator<char>(in), {}) == document);
  }
  const auto cleared = candidate_panel_status_document("ibus", CandidatePanelLimit::None);
  assert(write_candidate_panel_status(file, cleared));
  {
    std::ifstream in(file, std::ios::binary);
    assert(std::string(std::istreambuf_iterator<char>(in), {}) == cleared);
  }

  const auto escaped_root = root / "escaped";
  const auto escaped_outside = root / "escaped-outside";
  std::filesystem::create_directories(escaped_outside / "nested");
  std::filesystem::create_directory_symlink(escaped_outside, escaped_root);
  // 以 root 身份运行时（Linux 容器里就是这样），root 自己不对外开放的目录里的链接会被当成受信任的系统链接（见 `src/core/SafePath.h`）；把目录改成其他人可写，这条链接就成了任何人都可能放进去的链接。
  std::filesystem::permissions(root, std::filesystem::perms::others_write, std::filesystem::perm_options::add);
  const auto escaped_file = *candidate_panel_status_file(
      (escaped_root / "nested").c_str());
  assert(!write_candidate_panel_status(escaped_file, document));
  assert(!std::filesystem::exists(escaped_outside / "nested/candidate-panel.json"));

  // A directory other users can write to is refused.
  std::filesystem::permissions(file.parent_path(), std::filesystem::perms::others_write, std::filesystem::perm_options::add);
  assert(!write_candidate_panel_status(file, document));
  std::filesystem::remove_all(root);
}
