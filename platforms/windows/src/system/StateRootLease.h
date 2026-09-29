#pragma once
#include <filesystem>
#include <stdexcept>
#include <windows.h>

namespace msime::windows {
inline void reject_reparse_ancestors(const std::filesystem::path &root) {
  if (!root.is_absolute())
    throw std::invalid_argument("Relative state root");
  std::filesystem::path current;
  for (const auto &component : root) {
    current /= component;
    if (component == root.root_name() || component == root.root_directory())
      continue;
    const auto attributes = GetFileAttributesW(current.c_str());
    if (attributes == INVALID_FILE_ATTRIBUTES) {
      const auto error = GetLastError();
      if (error == ERROR_FILE_NOT_FOUND || error == ERROR_PATH_NOT_FOUND)
        continue;
      throw std::runtime_error("State root unavailable");
    }
    if (attributes & FILE_ATTRIBUTE_REPARSE_POINT)
      throw std::runtime_error("State root contains a reparse point");
  }
}

// Hold through resource preparation, all sessions and ordered Server shutdown.
// The stable file is never deleted: ownership is the OS handle, not existence.
class StateRootLease final {
public:
  explicit StateRootLease(const std::filesystem::path &root) {
    reject_reparse_ancestors(root);
    std::filesystem::create_directories(root);
    handle_ = CreateFileW((root / L".msime-client-server.lock").c_str(),
                          GENERIC_READ | GENERIC_WRITE, 0, nullptr, OPEN_ALWAYS,
                          FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                          nullptr);
    if (handle_ == INVALID_HANDLE_VALUE)
      throw std::runtime_error("State root unavailable");
    BY_HANDLE_FILE_INFORMATION info{};
    if (!GetFileInformationByHandle(handle_, &info) ||
        (info.dwFileAttributes &
         (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY))) {
      CloseHandle(handle_);
      handle_ = INVALID_HANDLE_VALUE;
      throw std::runtime_error("Invalid state lock file");
    }
  }
  ~StateRootLease() { CloseHandle(handle_); }
  StateRootLease(const StateRootLease &) = delete;
  StateRootLease &operator=(const StateRootLease &) = delete;

private:
  HANDLE handle_ = INVALID_HANDLE_VALUE;
};
} // namespace msime::windows
