#pragma once
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <string_view>

namespace msime::windows {
// 只观察 Server 已套用的皮肤，不观察设置预览或另一个明暗槽位。
// 字体、布局和资源重绘不重复提交；跨次使用的去重仍归共享统计。
class AppliedSkinStatistics {
public:
  std::optional<std::string> observe(std::string_view global_theme,
                                     std::string_view resolved_package) {
    std::string id(resolved_package.empty() ? global_theme : resolved_package);
    if (id.empty() || id == last_)
      return std::nullopt;
    last_ = id;
    return id;
  }

private:
  std::string last_;
};

inline std::string typing_statistics_record_skin_request(
    std::string_view directory, std::string_view id) {
  if (directory.empty() || id.empty())
    return {};
  // ID 校验、统计开关、去重和上限都由共享 record_skin 处理。
  return nlohmann::json{{"directory", std::string(directory)},
                        {"action", {{"operation", "record_skin"},
                                    {"id", std::string(id)}}}}
      .dump();
}
} // namespace msime::windows
