#pragma once
#include "CandidateSkinAssets.h"
#include <map>
#include <optional>
#include <string>
#include <vector>

namespace msime::windows {
// 皮肤根目录下若干个包（自定义主题两个明暗槽位指名的那些）的文件指纹，用来发现包被编辑。所有包的文件按完整路径放进同一份指纹，任一个包的文件增删改都算变化。
class SkinResourceRevision {
public:
  bool changed(const std::filesystem::path &root,
               const std::vector<std::string> &ids) {
    using Stamp = std::pair<std::filesystem::file_time_type, uintmax_t>;
    std::map<std::filesystem::path, Stamp> next;
    bool complete = true;
    for (const auto &id : ids) {
      if (root.empty() || !valid_candidate_skin_id(id))
        continue;
      std::error_code error;
      const auto directory = root / std::filesystem::u8path(id);
      const auto status = std::filesystem::symlink_status(directory, error);
      if (!error && std::filesystem::is_directory(status)) {
        std::filesystem::recursive_directory_iterator entry(directory, error),
            end;
        size_t visited = 0;
        for (; !error && entry != end; entry.increment(error)) {
          // Bound UI-thread work. Oversized/unreadable trees conservatively
          // request a refresh instead of caching an incomplete fingerprint.
          if (++visited > 1024) {
            complete = false;
            break;
          }
          const auto type = entry->symlink_status(error);
          if (error)
            break;
          if (std::filesystem::is_symlink(type))
            continue; // never follow links
          if (!std::filesystem::is_regular_file(type))
            continue;
          const auto time = entry->last_write_time(error);
          if (error)
            break;
          const auto size = entry->file_size(error);
          if (error)
            break;
          next.emplace(entry->path(), Stamp{time, size});
        }
        complete = complete && !error;
      } else if (error && error != std::errc::no_such_file_or_directory) {
        complete = false;
      }
    }
    const bool different = !complete || !previous_ || *previous_ != next;
    previous_ = std::move(next);
    return different;
  }

private:
  std::optional<std::map<std::filesystem::path,
                         std::pair<std::filesystem::file_time_type, uintmax_t>>>
      previous_;
};
} // namespace msime::windows
