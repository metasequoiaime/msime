#include "../src/candidates/CandidatePanelStatus.h"

#include <cassert>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>

int main() {
  using namespace msime::linux_host;
  // The names must match client-core's CandidatePanelLimit serde names.
  assert(candidate_panel_status_document("ibus", CandidatePanelLimit::GnomeShell) ==
         "{\"host\":\"ibus\",\"limit\":\"gnome_shell\"}\n");
  assert(candidate_panel_status_document("fcitx5", CandidatePanelLimit::FcitxTheme) ==
         "{\"host\":\"fcitx5\",\"limit\":\"fcitx_theme\"}\n");
  assert(candidate_panel_status_document("fcitx5", CandidatePanelLimit::Kimpanel) ==
         "{\"host\":\"fcitx5\",\"limit\":\"kimpanel\"}\n");
  assert(candidate_panel_status_document("ibus", CandidatePanelLimit::None) ==
         "{\"host\":\"ibus\",\"limit\":null}\n");

  assert(candidate_desktop_is_gnome_shell("GNOME"));
  assert(candidate_desktop_is_gnome_shell("ubuntu:GNOME"));
  assert(!candidate_desktop_is_gnome_shell("GNOME-Flashback:GNOME"));
  assert(!candidate_desktop_is_gnome_shell("Budgie:GNOME"));
  assert(!candidate_desktop_is_gnome_shell("Unity"));
  assert(!candidate_desktop_is_gnome_shell("KDE"));
  assert(!candidate_desktop_is_gnome_shell("X-Cinnamon"));
  assert(!candidate_desktop_is_gnome_shell(""));
  assert(!candidate_desktop_is_gnome_shell(nullptr));

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
  const auto escaped_file = *candidate_panel_status_file(
      (escaped_root / "nested").c_str());
  assert(!write_candidate_panel_status(escaped_file, document));
  assert(!std::filesystem::exists(escaped_outside / "nested/candidate-panel.json"));

  // A directory other users can write to is refused.
  std::filesystem::permissions(file.parent_path(), std::filesystem::perms::others_write, std::filesystem::perm_options::add);
  assert(!write_candidate_panel_status(file, document));
  std::filesystem::remove_all(root);
}
