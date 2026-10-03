#pragma once

#include <filesystem>
#include <iterator>
#include <system_error>
#include <vector>

namespace msime_linux {

// 逐组件检查状态路径，避免 mkdir 沿着中间符号链接在外部创建目录。
inline bool state_directory_path_is_safe(const std::filesystem::path &path) {
  if (!path.is_absolute()) return false;
  const auto components = [&] {
    std::vector<std::filesystem::path> result;
    result.reserve(static_cast<std::size_t>(std::distance(path.begin(), path.end())));
    for (const auto &component : path) result.push_back(component);
    return result;
  }();
  std::filesystem::path current = path.root_path();
  bool saw_prefix_alias = false;
  bool saw_real_component = false;
  std::error_code error;
  for (std::size_t index = 0; index < components.size(); ++index) {
    const auto &component = components[index];
    if (component == path.root_name() || component == path.root_directory()) continue;
    current /= component;
    const auto status = std::filesystem::symlink_status(current, error);
    if (!error) {
      if (std::filesystem::is_symlink(status)) {
        const bool system_alias = !saw_real_component && !saw_prefix_alias &&
                                  (component == "tmp" || component == "var");
        if (!system_alias) return false;
        saw_prefix_alias = true;
        continue;
      }
      if (!std::filesystem::is_directory(status) && index + 1 < components.size()) return false;
      if (index + 1 == components.size() && !std::filesystem::is_directory(status)) return false;
      saw_real_component = true;
      continue;
    }
    if (error != std::errc::no_such_file_or_directory) return false;
    error.clear();
  }
  return true;
}

} // namespace msime_linux
