#pragma once

#include <cerrno>
#include <cstdint>
#include <filesystem>
#include <string>
#include <string_view>
#include <system_error>
#include <vector>

#ifdef _WIN32
#include "StateRootLease.h"
#include <windows.h>
#else
#include <fcntl.h>
#include <unistd.h>
#endif

namespace msime::windows {

inline constexpr std::uint64_t kAudioMuteStateMaxBytes = 1024 * 1024;

inline bool read_audio_mute_state(const std::filesystem::path &path,
                                  std::string &contents) {
#ifdef _WIN32
  HANDLE handle = CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr,
                              OPEN_EXISTING,
                              FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                              nullptr);
  if (handle == INVALID_HANDLE_VALUE) return false;
  LARGE_INTEGER size{};
  bool ok = handle_is_trusted_file(handle) &&
            GetFileSizeEx(handle, &size) && size.QuadPart >= 0 &&
            static_cast<std::uint64_t>(size.QuadPart) <= kAudioMuteStateMaxBytes;
  if (ok) contents.resize(static_cast<std::size_t>(size.QuadPart));
  std::size_t offset = 0;
  while (ok && offset < contents.size()) {
    const DWORD chunk = static_cast<DWORD>(
        (contents.size() - offset) > static_cast<std::size_t>(MAXDWORD)
            ? MAXDWORD
            : contents.size() - offset);
    DWORD read = 0;
    ok = ReadFile(handle, contents.data() + offset, chunk, &read, nullptr) &&
         read == chunk;
    offset += read;
  }
  if (!CloseHandle(handle)) ok = false;
  if (!ok) contents.clear();
  return ok;
#else
  const int descriptor = ::open(path.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
  if (descriptor < 0) return false;
  contents.clear();
  char buffer[8192];
  bool ok = true;
  while (true) {
    const ssize_t read = ::read(descriptor, buffer, sizeof(buffer));
    if (read == 0) break;
    if (read < 0 && errno == EINTR) continue;
    if (read < 0 || contents.size() > kAudioMuteStateMaxBytes -
                         static_cast<std::size_t>(read)) {
      ok = false;
      break;
    }
    contents.append(buffer, static_cast<std::size_t>(read));
  }
  if (::close(descriptor) != 0) ok = false;
  if (!ok) contents.clear();
  return ok;
#endif
}

inline bool write_audio_mute_state(const std::filesystem::path &path,
                                   std::string_view contents) {
  if (contents.size() > kAudioMuteStateMaxBytes || path.parent_path().empty())
    return false;
#ifdef _WIN32
  wchar_t temporary_name[MAX_PATH]{};
  if (!GetTempFileNameW(path.parent_path().c_str(), L"msi", 0, temporary_name))
    return false;
  const auto remove_temporary = [&] { DeleteFileW(temporary_name); };
  HANDLE handle = CreateFileW(temporary_name, GENERIC_WRITE, 0, nullptr,
                              OPEN_EXISTING,
                              FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                              nullptr);
  if (handle == INVALID_HANDLE_VALUE) {
    remove_temporary();
    return false;
  }
  bool ok = handle_is_trusted_file(handle);
  std::size_t offset = 0;
  while (ok && offset < contents.size()) {
    const DWORD chunk = static_cast<DWORD>(
        (contents.size() - offset) > static_cast<std::size_t>(MAXDWORD)
            ? MAXDWORD
            : contents.size() - offset);
    DWORD written = 0;
    ok = WriteFile(handle, contents.data() + offset, chunk, &written, nullptr) &&
         written == chunk;
    offset += written;
  }
  if (ok) ok = FlushFileBuffers(handle) != FALSE;
  if (!CloseHandle(handle)) ok = false;
  if (!ok) {
    remove_temporary();
    return false;
  }
  if (!MoveFileExW(temporary_name, path.c_str(),
                   MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)) {
    remove_temporary();
    return false;
  }
  return true;
#else
  auto temporary_path = path;
  temporary_path += ".tmp-XXXXXX";
  const auto native = temporary_path.native();
  std::vector<char> temporary_name(native.begin(), native.end());
  temporary_name.push_back('\0');
  const int descriptor = ::mkstemp(temporary_name.data());
  if (descriptor < 0) return false;
  const std::filesystem::path temporary(temporary_name.data());
  bool ok = true;
  const char *data = contents.data();
  std::size_t remaining = contents.size();
  while (remaining != 0) {
    const ssize_t written = ::write(descriptor, data, remaining);
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
  if (ok) ok = ::fsync(descriptor) == 0;
  if (::close(descriptor) != 0) ok = false;
  if (ok) {
    std::error_code error;
    std::filesystem::rename(temporary, path, error);
    ok = !error;
  }
  if (!ok) {
    std::error_code ignored;
    std::filesystem::remove(temporary, ignored);
  }
  return ok;
#endif
}

} // namespace msime::windows
