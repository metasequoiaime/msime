#pragma once

#include <atomic>
#include <chrono>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <string_view>
#include <system_error>
#include <unistd.h>

namespace msime::dictionary_lease {

// Dictionary maintenance (import, edit, clearing learned data) needs the Engine's exclusive lock, and every open input session holds it shared. Windows asks its server to drop the sessions for the duration (DictionaryQuiesce/DictionaryResume); on Linux and macOS the settings window writes this lease beside the lock instead, and the input hosts (IBus, Fcitx5 and the macOS input method) close their sessions and open no new ones while it is live. The lease carries its own expiry, in Unix milliseconds, so a settings process that dies mid-import cannot leave input off: past the expiry, or with an expiry further out than any real lease, it is ignored. The name, the expiry on the first line (readers stop at the newline; every writer puts an owner line after it) and the bound are shared with crates/client-core/src/dictionary/quiesce.rs.
inline constexpr std::string_view kDictionaryQuiesceLeaseName = ".msime-dictionary-quiesce";
inline constexpr std::int64_t kDictionaryQuiesceLeaseMaxMs = 30'000;
inline constexpr std::size_t kDictionaryQuiesceLeaseMaxBytes = 256;

inline bool dictionary_quiesce_lease_live(std::string_view contents, std::int64_t now_ms) {
  std::int64_t expiry = 0;
  bool digits = false;
  for (const char c : contents) {
    if (c == '\n') break;
    if (c < '0' || c > '9' || expiry > (INT64_MAX - 9) / 10) return false;
    expiry = expiry * 10 + (c - '0');
    digits = true;
  }
  return digits && expiry > now_ms && expiry - now_ms <= kDictionaryQuiesceLeaseMaxMs;
}

inline std::int64_t dictionary_quiesce_now_ms() {
  return std::chrono::duration_cast<std::chrono::milliseconds>(
             std::chrono::system_clock::now().time_since_epoch())
      .count();
}

inline bool dictionary_lease_path_is_safe(const std::filesystem::path &path) {
  std::error_code error;
  auto current = path;
  while (true) {
    const auto status = std::filesystem::symlink_status(current, error);
    if (!error) return !std::filesystem::is_symlink(status);
    if (error && error != std::errc::no_such_file_or_directory) return false;
    error.clear();
    const auto parent = current.parent_path();
    if (parent == current) return true;
    current = parent;
  }
}

// Called from the hosts' timers and before a session opens; a missing lease costs one failed open.
inline bool dictionary_quiesced(const std::string &user_data,
                                std::int64_t now_ms = dictionary_quiesce_now_ms()) {
  if (user_data.empty() || user_data.front() != '/') return false;
  const auto lease_path = std::filesystem::path(user_data) / std::string(kDictionaryQuiesceLeaseName);
  if (!dictionary_lease_path_is_safe(lease_path)) return false;
  std::ifstream lease(lease_path);
  if (!lease) return false;
  char buffer[32] = {};
  lease.read(buffer, sizeof buffer - 1);
  return dictionary_quiesce_lease_live(std::string_view(buffer, static_cast<std::size_t>(lease.gcount())), now_ms);
}

// Raise the lease from inside an input host, for maintenance that host runs itself (the macOS input method's own dictionary window). It is written the way the settings window writes it: an owner line `<pid> <n>` after the expiry, staged under `<lease>.<pid>-<n>` and renamed into place, so a reader never sees half of it, with an expiry at the bound. `written` receives the exact contents, for lower_dictionary_quiesce_lease. False when it could not be written.
inline bool raise_dictionary_quiesce_lease(const std::string &user_data, std::string &written,
                                           std::int64_t now_ms = dictionary_quiesce_now_ms()) {
  if (user_data.empty() || user_data.front() != '/') return false;
  static std::atomic<std::uint64_t> next{0};
  const std::string pid = std::to_string(getpid()), serial = std::to_string(next.fetch_add(1));
  const std::filesystem::path root(user_data);
  const auto lease = root / std::string(kDictionaryQuiesceLeaseName);
  if (!dictionary_lease_path_is_safe(lease)) return false;
  auto staged = lease;
  staged += "." + pid + "-" + serial;
  const std::string contents = std::to_string(now_ms + kDictionaryQuiesceLeaseMaxMs) + "\n" + pid + " " + serial + "\n";
  {
    std::ofstream file(staged, std::ios::trunc);
    if (!(file << contents)) {
      std::error_code ignored;
      std::filesystem::remove(staged, ignored);
      return false;
    }
  }
  std::error_code error;
  std::filesystem::rename(staged, lease, error);
  if (!error) {
    written = contents;
    return true;
  }
  std::filesystem::remove(staged, error);
  return false;
}

// Remove the lease only while it is still the one `written` describes, as the Rust writers do (quiesce.rs, Lease::drop): a lease another writer has put up since belongs to work still running. The read and the removal are not one step.
inline void lower_dictionary_quiesce_lease(const std::string &user_data, const std::string &written) {
  if (user_data.empty() || user_data.front() != '/') return;
  const auto lease = std::filesystem::path(user_data) / std::string(kDictionaryQuiesceLeaseName);
  if (!dictionary_lease_path_is_safe(lease)) return;
  std::ifstream file(lease, std::ios::binary);
  if (!file) return;
  std::error_code error;
  const auto size = std::filesystem::file_size(lease, error);
  if (error || size > kDictionaryQuiesceLeaseMaxBytes) return;
  std::string current(static_cast<std::size_t>(size), '\0');
  if (!current.empty() &&
      !file.read(current.data(), static_cast<std::streamsize>(current.size())))
    return;
  file.close();
  if (current != written) return;
  std::error_code ignored;
  std::filesystem::remove(lease, ignored);
}

}  // namespace msime::dictionary_lease
