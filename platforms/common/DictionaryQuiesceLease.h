#pragma once

#include <atomic>
#include <chrono>
#include <cstdint>
#include <cerrno>
#include <filesystem>
#include <fcntl.h>
#include <iterator>
#include <string>
#include <string_view>
#include <sys/stat.h>
#include <system_error>
#include <sys/file.h>
#include <unistd.h>

namespace msime::dictionary_lease {

// Dictionary maintenance (import, edit, clearing learned data) needs the Engine's exclusive lock, and every open input session holds it shared. Windows asks its server to drop the sessions for the duration (DictionaryQuiesce/DictionaryResume); on Linux and macOS the settings window writes this lease beside the lock instead, and the input hosts (IBus, Fcitx5 and the macOS input method) close their sessions and open no new ones while it is live. The lease carries its own expiry, in Unix milliseconds, so a settings process that dies mid-import cannot leave input off: past the expiry, or with an expiry further out than any real lease, it is ignored. The name, the expiry on the first line (readers stop at the newline; every writer puts an owner line after it) and the bound are shared with crates/client-core/src/dictionary/quiesce.rs.
inline constexpr std::string_view kDictionaryQuiesceLeaseName = ".msime-dictionary-quiesce";
// Rust writers use the same persistent advisory lock. It serializes the
// read-and-remove sequence with every publisher, so a writer cannot remove a
// newer writer's lease between checking its contents and unlinking it.
inline constexpr std::string_view kDictionaryQuiesceLeaseLockName = ".msime-dictionary-quiesce.lock";
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
  auto current = path.root_path();
  bool saw_prefix_alias = false;
  bool saw_real_component = false;
  const auto relative = path.relative_path();
  const auto component_count = static_cast<std::size_t>(std::distance(relative.begin(), relative.end()));
  std::size_t index = 0;
  for (const auto &component : relative) {
    current /= component;
    const auto status = std::filesystem::symlink_status(current, error);
    if (!error) {
      if (std::filesystem::is_symlink(status)) {
        const bool target = index + 1 == component_count;
        if (target || saw_real_component || saw_prefix_alias) return false;
        saw_prefix_alias = true;
      } else {
        saw_real_component = true;
      }
    }
    if (error && error != std::errc::no_such_file_or_directory) return false;
    error.clear();
    ++index;
  }
  return true;
}

// Called from the hosts' timers and before a session opens; a missing lease costs one failed open.
inline bool dictionary_quiesced(const std::string &user_data,
                                std::int64_t now_ms = dictionary_quiesce_now_ms()) {
  if (user_data.empty() || user_data.front() != '/') return false;
  const auto lease_path = std::filesystem::path(user_data) / std::string(kDictionaryQuiesceLeaseName);
  if (!dictionary_lease_path_is_safe(lease_path)) return false;
  const int descriptor = ::open(lease_path.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK);
  if (descriptor < 0) return false;
  struct CloseOnExit {
    int descriptor;
    ~CloseOnExit() { ::close(descriptor); }
  } close_on_exit{descriptor};
  struct stat metadata {};
  if (::fstat(descriptor, &metadata) != 0 || !S_ISREG(metadata.st_mode)) return false;
  char buffer[32] = {};
  for (;;) {
    const ssize_t count = ::read(descriptor, buffer, sizeof buffer - 1);
    if (count > 0)
      return dictionary_quiesce_lease_live(std::string_view(buffer, static_cast<std::size_t>(count)), now_ms);
    if (count == 0) return false;
    if (errno == EINTR) continue;
    return false;
  }
}

inline bool write_staged_dictionary_lease(const std::filesystem::path &staged,
                                          std::string_view contents) {
  const int descriptor = ::open(staged.c_str(), O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW, 0600);
  if (descriptor < 0) return false;
  bool complete = true;
  const char *data = contents.data();
  std::size_t remaining = contents.size();
  while (remaining != 0) {
    const auto written = ::write(descriptor, data, remaining);
    if (written > 0) {
      data += written;
      remaining -= static_cast<std::size_t>(written);
    } else if (written < 0 && errno == EINTR) {
      continue;
    } else {
      complete = false;
      break;
    }
  }
  if (::close(descriptor) != 0) complete = false;
  if (!complete) {
    std::error_code ignored;
    std::filesystem::remove(staged, ignored);
  }
  return complete;
}

inline int lock_dictionary_quiesce_lease(const std::filesystem::path &root) {
  const auto lock = root / std::string(kDictionaryQuiesceLeaseLockName);
  const int descriptor = ::open(lock.c_str(), O_RDWR | O_CREAT | O_CLOEXEC | O_NOFOLLOW, 0600);
  if (descriptor < 0 || ::flock(descriptor, LOCK_EX) != 0) {
    if (descriptor >= 0) ::close(descriptor);
    return -1;
  }
  return descriptor;
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
  const int lock = lock_dictionary_quiesce_lease(root);
  if (lock < 0) return false;
  auto staged = lease;
  staged += "." + pid + "-" + serial;
  const std::string contents = std::to_string(now_ms + kDictionaryQuiesceLeaseMaxMs) + "\n" + pid + " " + serial + "\n";
  if (!write_staged_dictionary_lease(staged, contents)) {
    ::close(lock);
    return false;
  }
  std::error_code error;
  std::filesystem::rename(staged, lease, error);
  if (!error) {
    written = contents;
    ::close(lock);
    return true;
  }
  std::filesystem::remove(staged, error);
  ::close(lock);
  return false;
}

// Remove the lease only while it is still the one `written` describes, as the Rust writers do (quiesce.rs, Lease::drop): a lease another writer has put up since belongs to work still running. The read and the removal are not one step.
inline void lower_dictionary_quiesce_lease(const std::string &user_data, const std::string &written) {
  if (user_data.empty() || user_data.front() != '/') return;
  const auto lease = std::filesystem::path(user_data) / std::string(kDictionaryQuiesceLeaseName);
  if (!dictionary_lease_path_is_safe(lease)) return;
  const int lock = lock_dictionary_quiesce_lease(std::filesystem::path(user_data));
  if (lock < 0) return;
  const int descriptor = ::open(lease.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK);
  if (descriptor < 0) {
    ::close(lock);
    return;
  }
  struct CloseOnExit {
    int descriptor;
    ~CloseOnExit() { ::close(descriptor); }
  } close_on_exit{descriptor};
  struct stat metadata {};
  if (::fstat(descriptor, &metadata) != 0 || !S_ISREG(metadata.st_mode) || metadata.st_size < 0 ||
      static_cast<std::uintmax_t>(metadata.st_size) > kDictionaryQuiesceLeaseMaxBytes) {
    ::close(lock);
    return;
  }
  std::string current(static_cast<std::size_t>(metadata.st_size), '\0');
  std::size_t offset = 0;
  while (offset < current.size()) {
    const ssize_t count = ::read(descriptor, current.data() + offset, current.size() - offset);
    if (count > 0) {
      offset += static_cast<std::size_t>(count);
      continue;
    }
    if (count < 0 && errno == EINTR) continue;
    ::close(lock);
    return;
  }
  if (current != written) {
    ::close(lock);
    return;
  }
  std::error_code ignored;
  std::filesystem::remove(lease, ignored);
  ::close(lock);
}

}  // namespace msime::dictionary_lease
