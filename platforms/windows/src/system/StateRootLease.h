#pragma once
#include <filesystem>
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
