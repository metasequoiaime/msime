#pragma once
#include <array>
#include <filesystem>
#include <optional>
#include <string>
#include <stdexcept>
#include <windows.h>

namespace msime::windows {
// 只有名称代理（name surrogate）类的重解析点——符号链接、目录联接、挂载点——会把路径重定向到别处，与 Rust 在 Windows 上 `symlink_metadata().file_type().is_symlink()` 的判定一致。OneDrive 云文件等其它重解析点不改变路径指向，Rust 宿主照常接受，这里也必须接受，否则两个宿主对同一个状态根给出不同结论。规则以 `crates/path-trust/src/lib.rs` 为准，Windows 没有受信任的系统链接。
inline bool is_name_surrogate_reparse_point(DWORD attributes, DWORD tag) {
  return (attributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0 &&
         IsReparseTagNameSurrogate(tag) != 0;
}

// 读取句柄的属性与重解析标签；句柄须用 `FILE_FLAG_OPEN_REPARSE_POINT` 打开，读到的才是重解析点本身而不是它指向的目标。
inline bool read_attribute_tag(HANDLE handle, FILE_ATTRIBUTE_TAG_INFO &info) {
  return GetFileInformationByHandleEx(handle, FileAttributeTagInfo, &info,
                                      sizeof(info)) != 0;
}

// 最后一级文件的检查，同样与 Rust 的 path-trust 一致：只拒绝目录和名称代理重解析点（符号链接、目录联接），OneDrive 云文件这类重解析点放行。句柄须用 `FILE_FLAG_OPEN_REPARSE_POINT` 打开；读不到标签时按不可信处理。
inline bool handle_is_trusted_file(HANDLE handle) {
  FILE_ATTRIBUTE_TAG_INFO info{};
  return read_attribute_tag(handle, info) &&
         (info.FileAttributes & FILE_ATTRIBUTE_DIRECTORY) == 0 &&
         !is_name_surrogate_reparse_point(info.FileAttributes, info.ReparseTag);
}

// Read a handle that has already been opened with OPEN_REPARSE_POINT. Keeping
// the read on that handle lets callers tie the bytes to the file entry they
// checked, instead of reopening a path after the check.
inline std::optional<std::string> read_trusted_handle(HANDLE handle,
                                                       std::size_t max_bytes) {
  if (handle == INVALID_HANDLE_VALUE || !handle_is_trusted_file(handle))
    return std::nullopt;
  std::string document;
  document.reserve(max_bytes + 1);
  std::array<char, 4096> buffer{};
  for (;;) {
    DWORD count = 0;
    if (!ReadFile(handle, buffer.data(), static_cast<DWORD>(buffer.size()), &count,
                  nullptr))
      return std::nullopt;
    if (count == 0)
      return document;
    if (static_cast<std::size_t>(count) > max_bytes ||
        document.size() > max_bytes - static_cast<std::size_t>(count))
      return std::nullopt;
    document.append(buffer.data(), count);
  }
}

// 对应 `path_trust::reject_symlinked_components`：任何一级（包括最后一级）是名称代理重解析点就拒绝，不存在的层级放行，由调用方随后创建。
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
    if ((attributes & FILE_ATTRIBUTE_REPARSE_POINT) == 0)
      continue;
    // 打开重解析点本身读取它的标签，不跟随也不触发云文件回调；读不到标签时按不可信处理。
    const HANDLE handle = CreateFileW(
        current.c_str(), FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
        OPEN_EXISTING, FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
        nullptr);
    if (handle == INVALID_HANDLE_VALUE)
      throw std::runtime_error("State root unavailable");
    FILE_ATTRIBUTE_TAG_INFO info{};
    const bool read = read_attribute_tag(handle, info);
    CloseHandle(handle);
    if (!read)
      throw std::runtime_error("State root unavailable");
    if (is_name_surrogate_reparse_point(info.FileAttributes, info.ReparseTag))
      throw std::runtime_error("State root contains a reparse point");
  }
}

// Read a regular file through a handle that cannot follow a name-surrogate
// reparse point. The caller supplies the byte limit for the document contract.
inline std::optional<std::string> read_private_file(const std::filesystem::path &path,
                                                     std::size_t max_bytes) {
  try {
    reject_reparse_ancestors(path.parent_path());
  } catch (...) {
    return std::nullopt;
  }
  HANDLE handle = CreateFileW(
      path.c_str(), GENERIC_READ,
      FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
      OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT, nullptr);
  if (handle == INVALID_HANDLE_VALUE) {
    if (handle != INVALID_HANDLE_VALUE)
      CloseHandle(handle);
    return std::nullopt;
  }
  const auto document = read_trusted_handle(handle, max_bytes);
  CloseHandle(handle);
  return document;
}

// Read and remove one private file through the same trusted handle. This is
// used for one-shot installer hand-offs: closing the reader and then removing
// by path would let a concurrent writer replace the entry in between.
inline std::optional<std::string> take_private_file(
    const std::filesystem::path &path, std::size_t max_bytes) {
  try {
    reject_reparse_ancestors(path.parent_path());
  } catch (...) {
    return std::nullopt;
  }
  HANDLE handle = CreateFileW(
      path.c_str(), GENERIC_READ | DELETE,
      FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
      OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT, nullptr);
  if (handle == INVALID_HANDLE_VALUE)
    return std::nullopt;
  const bool trusted = handle_is_trusted_file(handle);
  const auto document = trusted ? read_trusted_handle(handle, max_bytes)
                                : std::nullopt;
  FILE_DISPOSITION_INFO disposition{TRUE};
  const bool removed = trusted &&
      SetFileInformationByHandle(handle, FileDispositionInfo, &disposition,
                                 sizeof(disposition)) != FALSE;
  CloseHandle(handle);
  if (!removed)
    return std::nullopt;
  return document;
}

// Delete a private regular file through the handle that was checked. A path
// based DeleteFileW after validating the parent would re-resolve that parent
// if a concurrent writer replaced it with a junction.
inline bool remove_private_file(const std::filesystem::path &path) {
  try {
    reject_reparse_ancestors(path.parent_path());
  } catch (...) {
    return false;
  }
  HANDLE handle = CreateFileW(
      path.c_str(), DELETE,
      FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
      OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
      nullptr);
  if (handle == INVALID_HANDLE_VALUE)
    return false;
  const bool trusted = handle_is_trusted_file(handle);
  FILE_DISPOSITION_INFO disposition{TRUE};
  const bool removed = trusted &&
      SetFileInformationByHandle(handle, FileDispositionInfo, &disposition,
                                 sizeof(disposition)) != FALSE;
  CloseHandle(handle);
  return removed;
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
    if (!handle_is_trusted_file(handle_)) {
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
