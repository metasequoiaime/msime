#pragma once

#include <fcntl.h>
#include <unistd.h>

#include <cstdio>
#include <filesystem>
#include <string>
#include <string_view>
#include <system_error>
#include <vector>

#include "../core/SafePath.h"

namespace msime::linux_host {

// Create a private same-directory temporary file and publish it with rename.
// mkstemp uses O_EXCL, so a pre-existing symlink cannot redirect the write.
inline bool candidate_directory_path_is_safe(const std::filesystem::path &directory) {
  return storage_directory_path_is_safe(directory);
}

inline bool prepare_candidate_directory(const std::filesystem::path &directory) {
  if (!candidate_directory_path_is_safe(directory)) return false;
  std::error_code error;
  std::filesystem::create_directories(directory, error);
  if (error) return false;
  return candidate_directory_path_is_safe(directory);
}

inline bool write_candidate_file_atomically(const std::filesystem::path &file,
                                            std::string_view content) {
  const auto parent = file.has_parent_path() ? file.parent_path() : std::filesystem::path(".");
  if (!prepare_candidate_directory(parent)) return false;
  std::error_code error;

  std::string pattern = file.string() + ".tmp-XXXXXX";
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
