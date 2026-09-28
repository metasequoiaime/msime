#include "../src/candidates/PanelRestoreRecord.h"

#include <cassert>
#include <cstdlib>
#include <fstream>

namespace {
nlohmann::json read(const std::filesystem::path &file) {
  std::ifstream in(file);
  return nlohmann::json::parse(in);
}
}  // namespace

int main() {
  using msime::linux_host::panel_restore_file;
  using msime::linux_host::panel_takeover_entry;
  using msime::linux_host::record_panel_takeover;
  using msime::linux_host::read_panel_restore;
  using Json = nlohmann::json;

  // $XDG_STATE_HOME when absolute, otherwise ~/.local/state; nothing without an absolute base.
  assert(panel_restore_file("/state", "/home/user") == std::filesystem::path("/state/msime-client/panel-restore.json"));
  assert(panel_restore_file("relative", "/home/user") ==
         std::filesystem::path("/home/user/.local/state/msime-client/panel-restore.json"));
  assert(panel_restore_file(nullptr, "/home/user") ==
         std::filesystem::path("/home/user/.local/state/msime-client/panel-restore.json"));
  assert(!panel_restore_file(nullptr, nullptr));
  assert(!panel_restore_file("relative", "also-relative"));

  // The first change keeps the replaced value; a later one keeps it while the setting still holds MSIME's write.
  const auto first = panel_takeover_entry(Json(nullptr), "Sans 10", "Noto Sans SC 18px");
  assert(first == Json({{"prior", "Sans 10"}, {"written", "Noto Sans SC 18px"}}));
  assert(panel_takeover_entry(first, "Noto Sans SC 18px", "Serif 20px") ==
         Json({{"prior", "Sans 10"}, {"written", "Serif 20px"}}));
  // The user changed it in between: their value is the one to restore.
  assert(panel_takeover_entry(first, "Monospace 12", "Serif 20px") ==
         Json({{"prior", "Monospace 12"}, {"written", "Serif 20px"}}));
  // A setting the user never set is recorded as null, and stays null while MSIME's write holds.
  const auto unset = panel_takeover_entry(Json(nullptr), Json(nullptr), true);
  assert(unset == Json({{"prior", nullptr}, {"written", true}}));
  assert(panel_takeover_entry(unset, true, true) == unset);
  // A setting already holding MSIME's own value starts the entry with the value the host says it stands in for, and a later write keeps the recorded one.
  const auto stand_in = panel_takeover_entry(Json(nullptr), "msime", "msime", "default");
  assert(stand_in == Json({{"prior", "default"}, {"written", "msime"}}));
  assert(panel_takeover_entry(Json({{"prior", "default-dark"}, {"written", "msime"}}), "msime", "msime", "default") ==
         Json({{"prior", "default-dark"}, {"written", "msime"}}));

  char temporary[] = "/tmp/msime-panel-restore-XXXXXX";
  const auto *directory = mkdtemp(temporary);
  assert(directory != nullptr);
  const std::filesystem::path root(directory);
  const auto file = *panel_restore_file((root / "state").c_str(), nullptr);

  // Each host keeps its own section; the directory is created on the first write.
  assert(record_panel_takeover(file, "fcitx5", "Theme", "default", "msime"));
  assert(record_panel_takeover(file, "ibus", "use-custom-font", Json(nullptr), true));
  assert(record_panel_takeover(file, "fcitx5", "Theme", "msime", "msime"));
  assert(read(file) == Json({{"fcitx5", {{"Theme", {{"prior", "default"}, {"written", "msime"}}}}},
                             {"ibus", {{"use-custom-font", {{"prior", nullptr}, {"written", true}}}}}}));
  assert(!std::filesystem::exists(file.string() + ".new"));
  assert(record_panel_takeover(file, "fcitx5", "DarkTheme", "msime", "msime", "default-dark"));
  assert(read(file)["fcitx5"]["DarkTheme"] == Json({{"prior", "default-dark"}, {"written", "msime"}}));

  // An unreadable record is replaced rather than blocking the write.
  std::ofstream(file) << "{not json";
  assert(record_panel_takeover(file, "fcitx5", "Font", "Sans 10", "Noto Sans SC 18px"));

  std::ofstream oversized(file, std::ios::binary | std::ios::trunc);
  oversized << std::string(msime::linux_host::kPanelRestoreMaxBytes + 1, 'x');
  oversized.close();
  assert(!read_panel_restore(file));
  assert(record_panel_takeover(file, "fcitx5", "Font", "Sans 10", "Noto Sans SC 18px"));
  assert(read(file) == Json({{"fcitx5", {{"Font", {{"prior", "Sans 10"}, {"written", "Noto Sans SC 18px"}}}}}}));

  std::filesystem::remove_all(root);
  return 0;
}
