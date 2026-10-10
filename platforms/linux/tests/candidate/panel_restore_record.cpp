#include "../src/candidates/PanelRestoreRecord.h"

#include <cassert>
#include <cstdlib>
#include <fstream>
#include <sys/stat.h>

namespace {
nlohmann::json read(const std::filesystem::path &file) {
  std::ifstream in(file);
  return nlohmann::json::parse(in);
}
}  // namespace

int main() {
  using msime::linux_host::panel_restore_file;
  using msime::linux_host::panel_restore_values;
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

  // 退出接管时放回的值：记录里的 prior；记录没留下可用的 prior（早先版本没记，或记录丢了）时按该选项替代的自带主题翠底，而不是把水杉自己的主题留在那里。
  // held 只放当前仍等于水杉写入值的选项，所以用户在 fcitx5-configtool 选的主题从不进这里，也从不被写回。
  const Json stock = {{"Theme", "default"}, {"DarkTheme", "default-dark"}};
  const Json owned = {{"fcitx5",
                      {{"Theme", {{"prior", "Nord-Dark"}, {"written", "msime"}}},
                       {"DarkTheme", {{"prior", nullptr}, {"written", "msime"}}},
                       {"Font", {{"prior", "Sans 10"}, {"written", "Noto Sans SC 18px"}}}}},
                     {"ibus", {{"use-custom-font", {{"prior", nullptr}, {"written", true}}}}}};
  assert(panel_restore_values(owned, "fcitx5", Json{{"Theme", "msime"}, {"DarkTheme", "msime"}}, stock) ==
         Json({{"Theme", "Nord-Dark"}, {"DarkTheme", "default-dark"}}));
  // 只有一项仍是水杉的时候只回一项，另一项（用户改过的）根本不进 held。
  assert(panel_restore_values(owned, "fcitx5", Json{{"DarkTheme", "msime"}}, stock) ==
         Json({{"DarkTheme", "default-dark"}}));
  // 没有记录（旧版本的写入、记录被删）时按自带主题恢复；另一个宿主的记录不参与。
  assert(panel_restore_values(Json::object(), "fcitx5", Json{{"Theme", "msime"}}, stock) ==
         Json({{"Theme", "default"}}));
  assert(panel_restore_values(owned, "other-host", Json{{"Theme", "msime"}}, stock) ==
         Json({{"Theme", "default"}}));
  // 记录的 written 与当前值不一致（记录已过期）时不拿它记的 prior 去覆盖，改回自带主题。
  const Json stale = {{"fcitx5", {{"Theme", {{"prior", "Nord-Dark"}, {"written", "other"}}}}}};
  assert(panel_restore_values(stale, "fcitx5", Json{{"Theme", "msime"}}, stock) == Json({{"Theme", "default"}}));
  // 没有仍由水杉持有的项，或 held 不是对象时不返回任何值；Font 这类没有自带值的选项不翠底。
  assert(panel_restore_values(owned, "fcitx5", Json::object(), stock) == Json::object());
  assert(panel_restore_values(Json(nullptr), "fcitx5", Json::array(), stock) == Json::object());
  assert(panel_restore_values(owned, "fcitx5", Json{{"Font", "msime"}}, stock) == Json::object());

  // Each host keeps its own section; the directory is created on the first write.
  assert(record_panel_takeover(file, "fcitx5", "Theme", "default", "msime"));
  assert(record_panel_takeover(file, "ibus", "use-custom-font", Json(nullptr), true));
  assert(record_panel_takeover(file, "fcitx5", "Theme", "msime", "msime"));
  assert(read(file) == Json({{"fcitx5", {{"Theme", {{"prior", "default"}, {"written", "msime"}}}}},
                             {"ibus", {{"use-custom-font", {{"prior", nullptr}, {"written", true}}}}}}));
  assert(!std::filesystem::exists(file.string() + ".new"));
  {
    const auto outside_lock = root / "outside-record.lock";
    std::ofstream(outside_lock) << "keep";
    std::filesystem::remove(file.string() + ".lock");
    std::filesystem::create_symlink(outside_lock, file.string() + ".lock");
    assert(!record_panel_takeover(file, "fcitx5", "PointerSize", "default", "msime", "default"));
    std::ifstream in(outside_lock);
    assert(std::string(std::istreambuf_iterator<char>(in), {}) == "keep");
    std::filesystem::remove(file.string() + ".lock");
  }
  {
    const auto outside = root / "outside-record.json";
    std::ofstream(outside) << "keep";
    std::filesystem::create_symlink(outside, file.string() + ".new");
    assert(record_panel_takeover(file, "fcitx5", "PointerSize", "default", "msime", "default"));
    std::ifstream in(outside);
    assert(std::string(std::istreambuf_iterator<char>(in), {}) == "keep");
  }
  assert(record_panel_takeover(file, "fcitx5", "DarkTheme", "msime", "msime", "default-dark"));
  assert(read(file)["fcitx5"]["DarkTheme"] == Json({{"prior", "default-dark"}, {"written", "msime"}}));

  // A planted record symlink must not be read as the user's restore state.
  const auto outside = root / "outside-record.json";
  std::ofstream(outside) << R"({"fcitx5":{"Font":{"prior":"attacker","written":"msime"}}})";
  std::filesystem::remove(file);
  std::filesystem::create_symlink(outside, file);
  assert(!read_panel_restore(file));
  std::filesystem::remove(file);

  // A planted record hard link must not be read as private restore state.
  {
    const auto hardlink_target = root / "outside-record-hardlink.json";
    std::ofstream(hardlink_target) << R"({"fcitx5":{"Font":{"prior":"attacker","written":"msime"}}})";
    std::filesystem::create_hard_link(hardlink_target, file);
    assert(!read_panel_restore(file));
    assert(std::filesystem::exists(hardlink_target));
    std::filesystem::remove(file);
  }

  // A hard-linked lock must not become the panel takeover lock.
  {
    const auto hardlink_lock_target = root / "outside-record-lock-hardlink";
    const auto hardlink_lock = std::filesystem::path(file.string() + ".lock");
    std::ofstream(hardlink_lock_target) << "keep";
    std::filesystem::remove(hardlink_lock);
    std::filesystem::create_hard_link(hardlink_lock_target, hardlink_lock);
    assert(!record_panel_takeover(file, "fcitx5", "Font", "Sans 10", "Noto Sans SC 18px"));
    assert(std::filesystem::exists(hardlink_lock_target));
    std::filesystem::remove(hardlink_lock);
    std::filesystem::remove(hardlink_lock_target);
  }

  // A planted FIFO must be rejected without blocking the settings writer.
  assert(::mkfifo(file.c_str(), 0600) == 0);
  assert(!read_panel_restore(file));
  std::filesystem::remove(file);

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
