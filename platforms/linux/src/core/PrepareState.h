#pragma once

#include <filesystem>

#include "SafePath.h"

namespace msime_linux {

// 逐组件检查状态路径，避免 mkdir 沿着中间符号链接在外部创建目录。
inline bool state_directory_path_is_safe(const std::filesystem::path &path) {
  return path.is_absolute() && msime::linux_host::storage_directory_path_is_safe(path);
}

} // namespace msime_linux
