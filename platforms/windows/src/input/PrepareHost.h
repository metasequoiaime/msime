#pragma once
#include <algorithm>
#include <cerrno>
#include <filesystem>
#include <fstream>
#include <functional>
#ifdef _WIN32
#include <windows.h>
#else
#include <fcntl.h>
#include <unistd.h>
#endif
#include <nlohmann/json.hpp>
#include <limits>
#include <stdexcept>
#include <string>
#include <string_view>

namespace msime::windows {
inline bool write_new_file(const std::filesystem::path &path,
                           std::string_view contents) {
#ifdef _WIN32
  HANDLE handle = CreateFileW(path.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_NEW,
                              FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                              nullptr);
  if (handle == INVALID_HANDLE_VALUE) return false;
  BY_HANDLE_FILE_INFORMATION info{};
  bool ok = GetFileInformationByHandle(handle, &info) &&
            !(info.dwFileAttributes &
              (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY));
  std::size_t offset = 0;
  while (ok && offset < contents.size()) {
    // Parenthesized so windows.h's min/max macros cannot expand them; this header includes windows.h itself and some including targets do not define NOMINMAX.
    const DWORD chunk = static_cast<DWORD>((std::min<std::size_t>)(
        contents.size() - offset,
        static_cast<std::size_t>((std::numeric_limits<DWORD>::max)())));
    DWORD written = 0;
    ok = WriteFile(handle, contents.data() + offset, chunk, &written, nullptr) &&
         written == chunk;
    offset += written;
  }
  if (!CloseHandle(handle)) ok = false;
  if (!ok) DeleteFileW(path.c_str());
  return ok;
#else
  const int descriptor = ::open(path.c_str(), O_WRONLY | O_CREAT | O_EXCL |
                                                O_CLOEXEC | O_NOFOLLOW,
                                0600);
  if (descriptor < 0) return false;
  bool ok = true;
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
      ok = false;
      break;
    }
  }
  if (::close(descriptor) != 0) ok = false;
  if (!ok) {
    std::error_code ignored;
    std::filesystem::remove(path, ignored);
  }
  return ok;
#endif
}

// Only orchestration belongs here. Resource verification and Engine dictionary
// preparation stay in the shared Host API, supplied by the native executable.
inline std::filesystem::path prepare_host_state_in_directory(
    const std::filesystem::path &resources,
    const std::filesystem::path &requested_state,
    const std::function<std::string(const std::string &)> &prepare) {
  if (!resources.is_absolute() || !requested_state.is_absolute() ||
      !std::filesystem::is_directory(resources))
    throw std::runtime_error("Absolute resource and new state paths required");
  const auto state = requested_state.lexically_normal();
  const auto request = nlohmann::json{
      {"resources", std::filesystem::canonical(resources).u8string()},
      {"state_root", state.u8string()}}.dump();
  if (request.size() > 16384)
    throw std::runtime_error("Preparation request oversized");
  const auto response = nlohmann::json::parse(prepare(request));
  if (!response.value("ok", false) || !response.at("value").is_object())
    throw std::runtime_error("Shared host preparation failed");
  const auto document = response.at("value").dump(2) + "\n";
  if (document.size() > 16384)
    throw std::runtime_error("Prepared configuration oversized");
  const auto temporary = state / ".runtime-options-prepared";
  const auto destination = state / "runtime-options.json";
  if (!write_new_file(temporary, document))
    throw std::runtime_error("Cannot write prepared configuration");
  // A same-directory hard link publishes complete contents without replacing
  // any destination created concurrently. Unsupported filesystems fail closed.
  // Do not remove prepared data on failure: the user may need it to diagnose.
  std::filesystem::create_hard_link(temporary, destination);
  std::filesystem::remove(temporary);
  return destination;
}

inline std::filesystem::path prepare_host_state(
    const std::filesystem::path &resources,
    const std::filesystem::path &requested_state,
    const std::function<std::string(const std::string &)> &prepare) {
  // create_directory is exclusive: never prepare against live or existing state.
  // Its parent must already exist. On Windows the new directory inherits the
  // user's LocalAppData ACL; this tool must run as that user, not the installer.
  const auto state = requested_state.lexically_normal();
  if (!resources.is_absolute() || !requested_state.is_absolute() ||
      !std::filesystem::is_directory(resources))
    throw std::runtime_error("Absolute resource and new state paths required");
  if (!std::filesystem::create_directory(state))
    throw std::runtime_error("A fresh state directory is required");
  return prepare_host_state_in_directory(resources, state, prepare);
}
} // namespace msime::windows
