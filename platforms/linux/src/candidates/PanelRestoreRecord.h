#pragma once

#include <fcntl.h>
#include <sys/file.h>
#include <unistd.h>

#include <filesystem>
#include <fstream>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <string_view>
#include <system_error>

namespace msime::linux_host {

// Windows draws its candidate window itself, so nothing MSIME styles there belongs to anyone else, and its uninstaller removes every trace of MSIME. Neither Linux host draws the list: the panel font, theme and wheel paging they write (Fcitx5's classicui options Theme, DarkTheme, Font and WheelForPaging, the IBus panel's custom-font and use-custom-font) are desktop settings every input method shares. So before a host first changes one, it records the value it replaces and the value it wrote, and msime-linux-setup --unregister puts the replaced value back while the setting still holds MSIME's. The record is $XDG_STATE_HOME/msime-client/panel-restore.json, shaped {"<host>": {"<key>": {"prior": <value>, "written": <value>}}}; a null prior is a setting the user never set, which uninstall resets rather than sets.
inline constexpr std::string_view kPanelRestoreFile = "msime-client/panel-restore.json";
inline constexpr std::size_t kPanelRestoreMaxBytes = 64 * 1024;

inline std::optional<nlohmann::json> read_panel_restore(const std::filesystem::path &file) {
  std::error_code error;
  const auto size = std::filesystem::file_size(file, error);
  if (error || size > kPanelRestoreMaxBytes) return std::nullopt;
  std::ifstream in(file, std::ios::binary);
  if (!in) return std::nullopt;
  std::string bytes(static_cast<std::size_t>(size), '\0');
  if (!bytes.empty() &&
      !in.read(bytes.data(), static_cast<std::streamsize>(bytes.size())))
    return std::nullopt;
  auto parsed = nlohmann::json::parse(bytes, nullptr, false);
  return parsed.is_object() ? std::optional<nlohmann::json>(std::move(parsed)) : std::nullopt;
}

// A relative XDG value is ignored, as the specification requires; msime-linux-setup resolves the same path.
inline std::optional<std::filesystem::path> panel_restore_file(const char *xdg_state_home, const char *home) {
  if (xdg_state_home && std::filesystem::path(xdg_state_home).is_absolute())
    return std::filesystem::path(xdg_state_home) / kPanelRestoreFile;
  if (home && std::filesystem::path(home).is_absolute())
    return std::filesystem::path(home) / ".local/state" / kPanelRestoreFile;
  return std::nullopt;
}

// The entry after `key` changes from `current` to `written`. The first change keeps `restore` as the value to put back: `current` itself, unless the setting already holds MSIME's own value (written by a build that kept no record, or after the record was lost), in which case the host passes the value MSIME's stands in for, since uninstall removes what MSIME's names. A later change keeps the recorded one while the setting still holds what MSIME last wrote; when it holds something else the user changed it in between, and their value is the one to go back to.
inline nlohmann::json panel_takeover_entry(const nlohmann::json &recorded, const nlohmann::json &current,
                                           const nlohmann::json &written, const nlohmann::json &restore) {
  const bool kept = recorded.is_object() && recorded.contains("prior") && recorded.contains("written") &&
                    recorded.at("written") == current;
  return {{"prior", kept ? recorded.at("prior") : restore}, {"written", written}};
}
inline nlohmann::json panel_takeover_entry(const nlohmann::json &recorded, const nlohmann::json &current,
                                           const nlohmann::json &written) {
  return panel_takeover_entry(recorded, current, written, current);
}

// Records the change of `key` under `host` before the host makes it. The IBus and Fcitx5 hosts may both run, so the read-modify-write holds a lock beside the record and replaces the file atomically. msime-linux-setup --unregister takes the same lock and never removes its file, so every writer locks one inode. Returns whether the record now describes the change.
inline bool record_panel_takeover(const std::filesystem::path &file, std::string_view host, std::string_view key,
                                  const nlohmann::json &current, const nlohmann::json &written,
                                  const nlohmann::json &restore) {
  std::error_code error;
  std::filesystem::create_directories(file.parent_path(), error);
  if (error) return false;
  auto lock_path = file;
  lock_path += ".lock";
  const int lock = open(lock_path.c_str(), O_RDWR | O_CREAT | O_CLOEXEC, 0600);
  if (lock < 0) return false;
  flock(lock, LOCK_EX);
  nlohmann::json record = nlohmann::json::object();
  {
    // An unreadable or oversized record is replaced; the values it held cannot be restored either way.
    if (auto parsed = read_panel_restore(file)) record = std::move(*parsed);
  }
  auto &section = record[std::string(host)];
  if (!section.is_object()) section = nlohmann::json::object();
  const auto recorded = section.value(std::string(key), nlohmann::json(nullptr));
  auto entry = panel_takeover_entry(recorded, current, written, restore);
  bool saved = entry == recorded;
  if (!saved) {
    section[std::string(key)] = std::move(entry);
    auto staged = file;
    staged += ".new";
    {
      std::ofstream out(staged, std::ios::binary | std::ios::trunc);
      saved = static_cast<bool>(out << record.dump(2) << '\n') && static_cast<bool>(out.flush());
    }
    if (saved) std::filesystem::rename(staged, file, error);
    if (!saved || error) {
      std::filesystem::remove(staged, error);
      saved = false;
    }
  }
  flock(lock, LOCK_UN);
  close(lock);
  return saved;
}
inline bool record_panel_takeover(const std::filesystem::path &file, std::string_view host, std::string_view key,
                                  const nlohmann::json &current, const nlohmann::json &written) {
  return record_panel_takeover(file, host, key, current, written, current);
}

}  // namespace msime::linux_host
