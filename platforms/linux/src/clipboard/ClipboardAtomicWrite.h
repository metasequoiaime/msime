#pragma once

#include <fcntl.h>
#include <unistd.h>

#include <cerrno>
#include <cstdio>
#include <filesystem>
#include <string>
#include <string_view>
#include <system_error>
#include <vector>

namespace msime::linux_host {

inline bool prepare_clipboard_directory(const std::filesystem::path &directory) {
  std::error_code error;
  auto current = directory;
  while (true) {
    const auto status = std::filesystem::symlink_status(current, error);
    if (!error) {
      if (std::filesystem::is_symlink(status) || !std::filesystem::is_directory(status)) return false;
      break;
    }
    if (error != std::errc::no_such_file_or_directory) return false;
    error.clear();
    const auto parent = current.parent_path();
    if (parent == current) break;
    current = parent;
  }
  std::filesystem::create_directories(directory, error);
  if (error) return false;
  const auto status = std::filesystem::symlink_status(directory, error);
  return !error && !std::filesystem::is_symlink(status) && std::filesystem::is_directory(status);
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
