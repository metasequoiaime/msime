#pragma once

#include "SafePath.h"

#include <cerrno>
#include <cstddef>
#include <fcntl.h>
#include <filesystem>
#include <stdexcept>
#include <string>
#include <unistd.h>

namespace msime::linux_host {

inline constexpr std::size_t kRuntimeOptionsMaxBytes = 16 * 1024;

// The IBus and Fcitx5 hosts reload this document while they are running. Open
// the leaf with O_NOFOLLOW and read one byte beyond the contract so a file that
// grows after the first read cannot turn a bounded parser into an unbounded one.
inline std::string read_runtime_options(const std::filesystem::path &path) {
  if (!storage_directory_path_is_safe(path.parent_path()))
    throw std::runtime_error("runtime options path is unsafe");
  const int descriptor = ::open(path.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK);
  if (descriptor < 0)
    throw std::runtime_error("runtime options unavailable");
  struct CloseOnExit {
    int descriptor;
    ~CloseOnExit() { ::close(descriptor); }
  } close_on_exit{descriptor};
  struct stat metadata {};
  if (::fstat(descriptor, &metadata) != 0 || !S_ISREG(metadata.st_mode)) {
    throw std::runtime_error("runtime options unavailable");
  }
  std::string document;
  document.reserve(kRuntimeOptionsMaxBytes);
  char buffer[4096];
  for (;;) {
    const std::size_t remaining = kRuntimeOptionsMaxBytes + 1 - document.size();
    const ssize_t count = ::read(descriptor, buffer, (remaining < sizeof buffer) ? remaining : sizeof buffer);
    if (count > 0) {
      document.append(buffer, static_cast<std::size_t>(count));
      if (document.size() > kRuntimeOptionsMaxBytes) {
        throw std::runtime_error("runtime options too large");
      }
      continue;
    }
    if (count == 0) break;
    if (errno == EINTR) continue;
    throw std::runtime_error("runtime options unavailable");
  }
  if (document.empty()) throw std::runtime_error("runtime options unavailable");
  return document;
}

} // namespace msime::linux_host
