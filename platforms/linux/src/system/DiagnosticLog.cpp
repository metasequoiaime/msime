#include "DiagnosticLog.h"

#include "../core/SafePath.h"

#include <algorithm>
#include <cerrno>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <ctime>
#include <fcntl.h>
#include <filesystem>
#include <mutex>
#include <string>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

namespace {
constexpr std::uintmax_t kMaxLogBytes = 1024 * 1024;
constexpr std::size_t kMaxEventBytes = 192;

bool directory_is_safe(const std::filesystem::path &directory) noexcept {
  if (!directory.is_absolute())
    return false;
  try {
    return msime::linux_host::storage_directory_path_is_safe(directory);
  } catch (...) {
    return false;
  }
}

class Log {
public:
  void configure(const std::string &directory, bool enabled) noexcept {
    std::lock_guard lock(mutex_);
    const auto root = std::filesystem::path(directory);
    enabled_ = enabled && !directory.empty() && directory.size() <= 4096 &&
               directory_is_safe(root);
    path_.clear();
    if (enabled_)
      path_ = (root / "diagnostic.log").string();
  }

  void write(std::string_view event) noexcept {
    std::lock_guard lock(mutex_);
    if (!enabled_ || path_.empty() ||
        !directory_is_safe(std::filesystem::path(path_).parent_path()))
      return;
    try {
      rotate_if_needed();
      const auto record = timestamp() + " [p" + std::to_string(getpid()) +
                          "] " + sanitize(event) + "\n";
      const int fd = open(
          path_.c_str(), O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC | O_NOFOLLOW,
          S_IRUSR | S_IWUSR);
      if (fd < 0)
        return;
      (void)fchmod(fd, S_IRUSR | S_IWUSR);
      std::size_t offset = 0;
      while (offset < record.size()) {
        const auto result =
            ::write(fd, record.data() + offset, record.size() - offset);
        if (result < 0 && errno == EINTR)
          continue;
        if (result <= 0)
          break;
        offset += static_cast<std::size_t>(result);
      }
      close(fd);
    } catch (...) {
      // Diagnostics must never affect the input path.
    }
  }

private:
  static std::string timestamp() {
    const auto now = std::chrono::system_clock::now();
    const auto seconds = std::chrono::system_clock::to_time_t(now);
    std::tm local{};
    localtime_r(&seconds, &local);
    char buffer[32]{};
    if (std::strftime(buffer, sizeof(buffer), "%Y-%m-%d %H:%M:%S", &local) == 0)
      return "0000-00-00 00:00:00";
    return buffer;
  }

  static std::string sanitize(std::string_view event) {
    const auto length = std::min(event.size(), kMaxEventBytes);
    std::string result;
    result.reserve(length);
    for (std::size_t i = 0; i < length; ++i) {
      const auto byte = static_cast<unsigned char>(event[i]);
      result.push_back(byte >= 0x20 && byte <= 0x7e ? static_cast<char>(byte)
                                                    : '?');
    }
    return result;
  }

  void rotate_if_needed() noexcept {
    std::error_code error;
    const auto size = std::filesystem::file_size(path_, error);
    if (error || size <= kMaxLogBytes)
      return;
    const auto rotated = path_ + ".1";
    std::filesystem::remove(rotated, error);
    error.clear();
    std::filesystem::rename(path_, rotated, error);
  }

  std::mutex mutex_;
  bool enabled_ = false;
  std::string path_;
};

Log &log() {
  static Log value;
  return value;
}
} // namespace

void msime_linux_diagnostic_configure(const std::string &directory,
                                      bool enabled) {
  log().configure(directory, enabled);
}

void msime_linux_diagnostic_write(std::string_view event) {
  log().write(event);
}
