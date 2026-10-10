#pragma once

#include <unistd.h>

#include <cerrno>
#include <cstddef>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <string_view>
#include <system_error>
#include <vector>

namespace msime::linux_host {

// The caller checks its directory policy and supplies a template in the target directory.
// mkstemp creates the temporary with O_EXCL; rename publishes only a fully synced file.
inline bool publish_file_atomically(const std::filesystem::path &file, std::string_view content,
                                    std::string_view temporary_pattern) {
  std::vector<char> name(temporary_pattern.begin(), temporary_pattern.end());
  name.push_back('\0');
  const int descriptor = ::mkstemp(name.data());
  if (descriptor < 0) return false;
  const std::filesystem::path temporary(name.data());

  bool ok = true;
  const char *bytes = content.data();
  std::size_t remaining = content.size();
  while (remaining > 0) {
    const ssize_t written = ::write(descriptor, bytes, remaining);
    if (written < 0 && errno == EINTR) continue;
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
    std::error_code error;
    std::filesystem::remove(temporary, error);
    return false;
  }
  if (::rename(temporary.c_str(), file.c_str()) != 0) {
    std::error_code error;
    std::filesystem::remove(temporary, error);
    return false;
  }
  return true;
}

} // namespace msime::linux_host
