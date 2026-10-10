#include "DiagnosticLog.h"
#include "SystemPathAlias.h"

#include <algorithm>
#include <atomic>
#include <cerrno>
#include <cstdarg>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <ctime>
#include <fcntl.h>
#include <filesystem>
#include <mutex>
#include <pthread.h>
#include <string>
#include <sys/stat.h>
#include <system_error>
#include <unistd.h>

namespace {
constexpr std::uintmax_t kMaxLogBytes = 1024 * 1024;
constexpr std::size_t kMaxEventBytes = 192;

bool directoryIsSafe(const std::filesystem::path &directory) noexcept {
  try {
    return msime::mac::StoragePathIsSafe(directory, true);
  } catch (...) {
    return false;
  }
}

// Mirrors Log::enabled_ for lock-free checks; configure() is the only writer.
std::atomic_bool gEnabled{false};

class Log {
public:
  void configure(const std::string &directory, bool enabled) noexcept {
    std::lock_guard lock(mutex_);
    const auto root = std::filesystem::path(directory);
    enabled_ = enabled && !directory.empty() && directory.size() <= 4096 &&
               directoryIsSafe(root);
    path_.clear();
    gEnabled.store(enabled_, std::memory_order_relaxed);
    if (enabled_)
      path_ = (root / "diagnostic.log").string();
  }

  void write(std::string_view event) noexcept {
    std::lock_guard lock(mutex_);
    if (!enabled_ || path_.empty() ||
        !directoryIsSafe(std::filesystem::path(path_).parent_path()))
      return;
    try {
      rotateIfNeeded();
      struct stat existing = {};
      if (lstat(path_.c_str(), &existing) == 0 &&
          (!S_ISREG(existing.st_mode) || existing.st_nlink != 1))
        return;
      // The thread id lets main-thread redraws and key handling be told apart from work finishing on other threads, as the Windows log does.
      std::uint64_t thread = 0;
      (void)pthread_threadid_np(nullptr, &thread);
      const std::string record = timestamp() + " [p" +
                                 std::to_string(getpid()) + ":t" +
                                 std::to_string(thread) + "] " +
                                 sanitize(event) + "\n";
      const int fd = open(
          path_.c_str(), O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC | O_NOFOLLOW,
          S_IRUSR | S_IWUSR);
      if (fd < 0)
        return;
      (void)fchmod(fd, S_IRUSR | S_IWUSR);
      std::size_t offset = 0;
      while (offset < record.size()) {
        const ssize_t result =
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
    // Milliseconds order records written within the same second, such as a key waiting behind a redraw.
    const auto milliseconds =
        std::chrono::duration_cast<std::chrono::milliseconds>(
            now.time_since_epoch())
            .count() %
        1000;
    std::tm local{};
    localtime_r(&seconds, &local);
    char buffer[32]{};
    if (std::strftime(buffer, sizeof(buffer), "%Y-%m-%d %H:%M:%S", &local) == 0)
      return "0000-00-00 00:00:00.000";
    char fraction[8]{};
    std::snprintf(fraction, sizeof(fraction), ".%03lld",
                  static_cast<long long>(milliseconds));
    return std::string(buffer) + fraction;
  }

  static std::string sanitize(std::string_view event) {
    const std::size_t length = std::min(event.size(), kMaxEventBytes);
    std::string result;
    result.reserve(length);
    for (std::size_t i = 0; i < length; ++i) {
      const auto byte = static_cast<unsigned char>(event[i]);
      result.push_back(byte >= 0x20 && byte <= 0x7e ? static_cast<char>(byte)
                                                    : '?');
    }
    return result;
  }

  void rotateIfNeeded() noexcept {
    std::error_code error;
    const auto size = std::filesystem::file_size(path_, error);
    if (error || size <= kMaxLogBytes)
      return;
    const std::string rotated = path_ + ".1";
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

void msime_macos_diagnostic_configure(const std::string &directory,
                                      bool enabled) noexcept {
  log().configure(directory, enabled);
}

void msime_macos_diagnostic_write(std::string_view event) noexcept {
  log().write(event);
}

bool msime_macos_diagnostic_enabled() noexcept {
  return gEnabled.load(std::memory_order_relaxed);
}

void msime_macos_diagnostic_writef(const char *format, ...) noexcept {
  if (!format || !msime_macos_diagnostic_enabled())
    return;
  char buffer[kMaxEventBytes + 1]{};
  va_list arguments;
  va_start(arguments, format);
  const int length = std::vsnprintf(buffer, sizeof(buffer), format, arguments);
  va_end(arguments);
  if (length < 0)
    return;
  log().write(std::string_view(
      buffer, std::min(static_cast<std::size_t>(length), kMaxEventBytes)));
}

void msime_macos_diagnostic_host_line(const char *line) noexcept {
  if (!line || !msime_macos_diagnostic_enabled())
    return;
  // Only the category. msime_client.h words every line "<category>: <detail>" with a fixed category before the first colon, and the detail names the pack, the file and the error; this log is read back through the MCP server by an AI assistant and promises event names and error categories, never paths or error text. A line without a colon breaks that wording, so none of it is trusted and it is written as "host_api: uncategorized".
  constexpr char kPrefix[] = "host_api: ";
  constexpr char kUncategorized[] = "uncategorized";
  char buffer[kMaxEventBytes]{};
  std::size_t length = sizeof(kPrefix) - 1;
  std::memcpy(buffer, kPrefix, length);
  const char *colon = std::strchr(line, ':');
  const char *category = colon ? line : kUncategorized;
  const std::size_t categoryLength =
      colon ? static_cast<std::size_t>(colon - line) : sizeof(kUncategorized) - 1;
  const std::size_t copied = std::min(categoryLength, sizeof(buffer) - length);
  std::memcpy(buffer + length, category, copied);
  length += copied;
  log().write(std::string_view(buffer, length));
}
