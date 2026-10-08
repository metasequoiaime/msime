#pragma once

#include <fcntl.h>
#include <unistd.h>

#include <cerrno>
#include <cstdio>
#include <array>
#include <filesystem>
#include <optional>
#include <string>
#include <string_view>
#include <system_error>
#include <sys/stat.h>
#include <vector>

#include "../core/SafePath.h"

namespace msime::linux_host {

inline bool clipboard_directory_is_safe(const std::filesystem::path &directory) {
  std::error_code error;
  const auto absolute = std::filesystem::absolute(directory, error).lexically_normal();
  if (error) return false;
  return storage_directory_path_is_safe(absolute);
}

inline bool prepare_clipboard_directory(const std::filesystem::path &directory) {
  std::error_code error;
  if (!clipboard_directory_is_safe(directory)) return false;
  std::filesystem::create_directories(directory, error);
  if (error) return false;
  const auto status = std::filesystem::symlink_status(directory, error);
  return !error && !std::filesystem::is_symlink(status) && std::filesystem::is_directory(status);
}

// Open the history lock without following either a replaced parent directory
// or a replaced lock-file entry. Clipboard readers and writers use the same
// helper so the standalone tool and the IBus menu cannot diverge.
inline int open_clipboard_lock(const std::filesystem::path &history) {
  const auto directory = history.has_parent_path() ? history.parent_path()
                                                   : std::filesystem::path(".");
  if (!clipboard_directory_is_safe(directory)) return -1;
  auto lock = history;
  lock += ".lock";
  return ::open(lock.c_str(), O_CREAT | O_RDWR | O_CLOEXEC | O_NOFOLLOW, 0600);
}

// Remove the history file only when its parent remains a real directory. The
// filesystem remove operation does not follow a final symlink, but it does
// traverse symlinked ancestors; clearing must use the same boundary as reads
// and writes.
inline bool remove_clipboard_file(const std::filesystem::path &file) {
  const auto directory = file.has_parent_path() ? file.parent_path()
                                                : std::filesystem::path(".");
  if (!clipboard_directory_is_safe(directory)) return false;
  std::error_code error;
  std::filesystem::remove(file, error);
  return !error;
}

inline std::optional<std::string> read_clipboard_file(const std::filesystem::path &file,
                                                      std::size_t max_bytes) {
  const auto directory = file.has_parent_path() ? file.parent_path() : std::filesystem::path(".");
  if (!clipboard_directory_is_safe(directory)) return std::nullopt;
  const int descriptor = ::open(file.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK);
  if (descriptor < 0) return std::nullopt;
  struct stat metadata {};
  if (::fstat(descriptor, &metadata) != 0 || !S_ISREG(metadata.st_mode)) {
    ::close(descriptor);
    return std::nullopt;
  }
  std::string content;
  std::array<char, 8192> buffer{};
  bool ok = true;
  while (true) {
    const ssize_t count = ::read(descriptor, buffer.data(), buffer.size());
    if (count == 0) break;
    if (count < 0) {
      if (errno == EINTR) continue;
      ok = false;
      break;
    }
    const auto bytes = static_cast<std::size_t>(count);
    if (content.size() > max_bytes || bytes > max_bytes - content.size()) {
      ok = false;
      break;
    }
    content.append(buffer.data(), bytes);
  }
  if (::close(descriptor) != 0) ok = false;
  return ok ? std::optional<std::string>(std::move(content)) : std::nullopt;
}

inline bool write_clipboard_file_atomically(const std::filesystem::path &file,
                                            std::string_view content) {
  const auto directory = file.has_parent_path() ? file.parent_path() : std::filesystem::path(".");
  if (!prepare_clipboard_directory(directory)) return false;
  std::error_code error;
  std::string pattern = (directory / ".msime-clipboard-XXXXXX").string();
  std::vector<char> name(pattern.begin(), pattern.end());
  name.push_back('\0');
  const int descriptor = ::mkstemp(name.data());
  if (descriptor < 0) return false;
  const std::filesystem::path temporary(name.data());
  bool ok = true;
  const char *bytes = content.data();
  std::size_t remaining = content.size();
  while (remaining > 0) {
    const ssize_t written = ::write(descriptor, bytes, remaining);
    if (written <= 0) {
      if (errno == EINTR) continue;
      ok = false;
      break;
    }
    bytes += written;
    remaining -= static_cast<std::size_t>(written);
  }
  if (ok && ::fsync(descriptor) != 0) ok = false;
  if (::close(descriptor) != 0) ok = false;
  if (!ok) {
    std::filesystem::remove(temporary, error);
    return false;
  }
  if (::rename(temporary.c_str(), file.c_str()) != 0) {
    std::filesystem::remove(temporary, error);
    return false;
  }
  return true;
}

} // namespace msime::linux_host
