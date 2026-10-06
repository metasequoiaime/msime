#pragma once

#include <sys/stat.h>

#include <cerrno>
#include <cstddef>
#include <filesystem>
#include <vector>

namespace msime::linux_host {

// `crates/path-trust/src/lib.rs` 中的 Linux 规则，所有逐层检查存储路径的 Rust crate 都向它查询；这个头文件是给 IBus 和 Fcitx5 宿主用的 C++ 副本，必须与它保持一致。

// 判断 `path` 是否是只有 root 才能创建或修改的符号链接：链接属于 root，所在目录也属于 root，且组和其他用户都不可写。Fedora Silverblue 的 `/home -> var/home` 和链接到另一块磁盘的 `/home` 都能通过；用户目录里的链接，或 `/tmp` 这类设置了 sticky 位、所有人可写的目录里的链接则不行。对应 `crates/path-trust/src/lib.rs` 中的 `is_root_only_link`。
inline bool is_root_only_link(const std::filesystem::path &path) {
  struct stat link {};
  if (::lstat(path.c_str(), &link) != 0 || !S_ISLNK(link.st_mode) || link.st_uid != 0) return false;
  struct stat parent {};
  const auto parent_path = path.parent_path();
  return ::stat(parent_path.c_str(), &parent) == 0 && S_ISDIR(parent.st_mode) && parent.st_uid == 0 &&
         (parent.st_mode & 022) == 0;
}

// 判断 `directory` 已存在的每一级是否都是目录且都不是符号链接，唯一的例外是绝对路径最后一级之上至多一个仅 root 可改的链接（见 `is_root_only_link`）。不存在的层级可以接受，调用方正要创建它们；其它错误一律拒绝该路径。对应 `crates/path-trust/src/lib.rs` 中的 `reject_symlinked_components`；要求已存在的层级必须是目录，是这个宿主自己加的约束。
inline bool storage_directory_path_is_safe(const std::filesystem::path &directory) {
  // 与 Rust 的 `Path::components` 一样，末尾的分隔符和 `.` 层级不代表任何一级。
  std::vector<std::filesystem::path> components;
  for (const auto &component : directory) {
    if (component.empty() || component == ".") continue;
    components.push_back(component);
  }
  std::filesystem::path current;
  bool saw_system_link = false;
  for (std::size_t index = 0; index < components.size(); ++index) {
    const auto &component = components[index];
    current /= component;
    if (component == directory.root_name() || component == directory.root_directory() || component == "..")
      continue;
    struct stat info {};
    if (::lstat(current.c_str(), &info) != 0) {
      if (errno == ENOENT) continue;
      return false;
    }
    if (S_ISLNK(info.st_mode)) {
      const bool last = index + 1 == components.size();
      if (last || saw_system_link || !directory.is_absolute() || !is_root_only_link(current)) return false;
      saw_system_link = true;
      continue;
    }
    if (!S_ISDIR(info.st_mode)) return false;
  }
  return true;
}

} // namespace msime::linux_host
