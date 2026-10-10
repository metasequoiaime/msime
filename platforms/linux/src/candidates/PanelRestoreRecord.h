#pragma once

#include <fcntl.h>
#include <sys/file.h>
#include <unistd.h>

#include <filesystem>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <string_view>
#include <system_error>

#include "AtomicWrite.h"
#include "../core/LinuxEdition.h"

namespace msime::linux_host {

// Windows draws its candidate window itself, so nothing MSIME styles there belongs to anyone else, and its uninstaller removes every trace of MSIME. Neither Linux host draws the list: the panel font, theme and wheel paging they write (Fcitx5's classicui options Theme, DarkTheme, Font and WheelForPaging, the IBus panel's custom-font and use-custom-font) are desktop settings every input method shares. So before a host first changes one, it records the value it replaces and the value it wrote, and msime-linux-setup --unregister puts the replaced value back while the setting still holds MSIME's. The record is $XDG_STATE_HOME/msime-client/panel-restore.json, shaped {"<host>": {"<key>": {"prior": <value>, "written": <value>}}}; a null prior is a setting the user never set, which uninstall resets rather than sets.
// 记录按版本分开（$XDG_STATE_HOME/<client_directory>）：每个版本的卸载只放回它自己接管之前的值。
inline constexpr std::string_view kPanelRestoreFile = MSIME_EDITION_CLIENT_DIRECTORY "/panel-restore.json";
inline constexpr std::size_t kPanelRestoreMaxBytes = 64 * 1024;

inline std::optional<nlohmann::json> read_panel_restore(const std::filesystem::path &file) {
  std::error_code error;
  const auto status = std::filesystem::symlink_status(file, error);
  if (error || !std::filesystem::is_regular_file(status)) return std::nullopt;
  const int descriptor = ::open(file.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK);
  if (descriptor < 0) return std::nullopt;
  struct CloseOnExit {
    int descriptor;
    ~CloseOnExit() { ::close(descriptor); }
  } close_on_exit{descriptor};
  struct stat metadata {};
  if (::fstat(descriptor, &metadata) != 0 || !S_ISREG(metadata.st_mode) || metadata.st_nlink != 1 ||
      metadata.st_size < 0 ||
      static_cast<std::uintmax_t>(metadata.st_size) > kPanelRestoreMaxBytes)
    return std::nullopt;
  std::string bytes(static_cast<std::size_t>(metadata.st_size), '\0');
  std::size_t offset = 0;
  while (offset < bytes.size()) {
    const ssize_t count = ::read(descriptor, bytes.data() + offset, bytes.size() - offset);
    if (count > 0) {
      offset += static_cast<std::size_t>(count);
      continue;
    }
    if (count < 0 && errno == EINTR) continue;
    return std::nullopt;
  }
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
  if (!prepare_candidate_directory(file.parent_path())) return false;
  auto lock_path = file;
  lock_path += ".lock";
  const int lock = open(lock_path.c_str(), O_RDWR | O_CREAT | O_CLOEXEC | O_NOFOLLOW, 0600);
  if (lock < 0) return false;
  struct stat lock_metadata {};
  if (fstat(lock, &lock_metadata) != 0 || !S_ISREG(lock_metadata.st_mode) || lock_metadata.st_nlink != 1 ||
      flock(lock, LOCK_EX) != 0) {
    close(lock);
    return false;
  }
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
    const auto serialized = record.dump(2) + '\n';
    saved = write_candidate_file_atomically(file, serialized);
  }
  flock(lock, LOCK_UN);
  close(lock);
  return saved;
}
inline bool record_panel_takeover(const std::filesystem::path &file, std::string_view host, std::string_view key,
                                  const nlohmann::json &current, const nlohmann::json &written) {
  return record_panel_takeover(file, host, key, current, written, current);
}

// `held` 只含当前值仍由水杉持有的面板项，恢复记录的 `prior`，没有原值则用 `stock`；`written` 不匹配时忽略旧原值，不覆盖用户后改的项。与卸载脚本 `restorable` 共用语义，在宿主内立即恢复。
// 恢复不修改记录：这不是一次新的接管，把恢复写进去只会把用户自己的值换成被恢复的值。
inline nlohmann::json panel_restore_values(const nlohmann::json &record, std::string_view host,
                                          const nlohmann::json &held, const nlohmann::json &stock) {
  nlohmann::json restore = nlohmann::json::object();
  if (!held.is_object()) return restore;
  const auto section = record.is_object() ? record.find(std::string(host)) : record.cend();
  const bool have_section = record.is_object() && section != record.cend() && section->is_object();
  for (const auto &item : held.items()) {
    const auto &key = item.key();
    nlohmann::json prior = nullptr;
    if (have_section) {
      const auto recorded = section->find(key);
      if (recorded != section->cend() && recorded->is_object() && recorded->contains("prior") &&
          recorded->contains("written") && recorded->at("written") == item.value())
        prior = recorded->at("prior");
    }
    if (prior.is_string()) restore[key] = prior;
    else if (stock.is_object() && stock.contains(key)) restore[key] = stock.at(key);
  }
  return restore;
}

}  // namespace msime::linux_host
