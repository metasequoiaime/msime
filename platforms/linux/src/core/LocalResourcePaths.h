#pragma once

#include <cstdlib>
#include <filesystem>
#include <string>
#include <vector>

#ifndef MSIME_RELATIVE_DATA_DIR
#define MSIME_RELATIVE_DATA_DIR "../share"
#endif

namespace msime_linux {
// Both tools consume files, even when the Host API takes a containing
// directory (the Emoji catalog is always resources/msime-others.db).
inline bool resource_file(const std::filesystem::path &path) {
  std::error_code error;
  return path.is_absolute() && std::filesystem::is_regular_file(path, error);
}

inline std::string local_resource(const char *environment,
                                  const std::filesystem::path &relative,
                                  bool return_directory = false) {
  const auto resolve = [return_directory](const std::filesystem::path &file) {
    return resource_file(file)
        ? (return_directory ? file.parent_path() : file).string()
        : std::string{};
  };
  if (const char *value = std::getenv(environment); value && *value) {
    // An explicit override must not silently select another catalog/model.
    auto file = std::filesystem::path(value);
    if (return_directory)
      file /= relative.filename();
    return resolve(file);
  }

  std::vector<std::filesystem::path> roots;
  const auto add = [&roots](const char *value) {
    if (value && *value && std::filesystem::path(value).is_absolute())
      roots.emplace_back(value);
  };
  const char *data_home = std::getenv("XDG_DATA_HOME");
  if (data_home && *data_home && std::filesystem::path(data_home).is_absolute()) {
    add(data_home);
  } else if (const char *home = std::getenv("HOME"); home && *home &&
             std::filesystem::path(home).is_absolute()) {
    roots.emplace_back(std::filesystem::path(home) / ".local/share");
  }

  // argv[0] may contain only a command name resolved through PATH. Linux's
  // executable link identifies the actual binary, also for symlink launchers.
  std::error_code error;
  const auto executable = std::filesystem::read_symlink("/proc/self/exe", error);
  if (!error && executable.is_absolute())
    roots.emplace_back(executable.parent_path() / MSIME_RELATIVE_DATA_DIR);

  const char *data_dirs = std::getenv("XDG_DATA_DIRS");
  const std::string directories = data_dirs && *data_dirs
      ? data_dirs : "/usr/local/share:/usr/share";
  std::size_t start = 0;
  for (;;) {
    const auto end = directories.find(':', start);
    const auto directory = directories.substr(
        start, end == std::string::npos ? end : end - start);
    add(directory.c_str());
    if (end == std::string::npos)
      break;
    start = end + 1;
  }
  for (const auto &root : roots) {
    const auto found = resolve((root / relative).lexically_normal());
    if (!found.empty())
      return found;
  }
  return {};
}
} // namespace msime_linux
