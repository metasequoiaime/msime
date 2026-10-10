#pragma once

#include <filesystem>
#include <string>
#include <string_view>
#include <system_error>

#include "../core/AtomicFileWrite.h"
#include "../core/SafePath.h"

namespace msime::linux_host {

// Create a private same-directory temporary file and publish it with rename.
// mkstemp uses O_EXCL, so a pre-existing symlink cannot redirect the write.
inline bool candidate_directory_path_is_safe(const std::filesystem::path &directory) {
  return storage_directory_path_is_safe(directory);
}

inline bool prepare_candidate_directory(const std::filesystem::path &directory) {
  if (!candidate_directory_path_is_safe(directory)) return false;
  std::error_code error;
  std::filesystem::create_directories(directory, error);
  if (error) return false;
  return candidate_directory_path_is_safe(directory);
}

inline bool write_candidate_file_atomically(const std::filesystem::path &file,
                                            std::string_view content) {
  const auto parent = file.has_parent_path() ? file.parent_path() : std::filesystem::path(".");
  if (!prepare_candidate_directory(parent)) return false;
  return publish_file_atomically(file, content, file.string() + ".tmp-XXXXXX");
}

} // namespace msime::linux_host
