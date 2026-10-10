#pragma once
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <string_view>

namespace msime::windows {
// 生产 Server 把用户拖动后的工具栏位置记在状态目录的这个文件里，下次启动从这里恢复，与 macOS 的 frame autosave 一样跨重启保留。预览实例仍写自己的配置文件（floating_toolbar_x / floating_toolbar_y）；生产配置由 runtime-options.json 和共享偏好每次重新拼出来，写回那里的值会被下一次启动丢掉。位置只属于这台机器的显示器布局，所以不进同步到云端的共享偏好。
inline constexpr wchar_t floating_toolbar_position_file[] = L"floating_toolbar_position.json";

struct FloatingToolbarPosition {
  int x = 0;
  int y = 0;
};

// 坐标范围与 PreviewConfig 对 floating_toolbar_x / floating_toolbar_y 的检查一致。读不懂的文件当作没有记过位置，工具栏回到默认的右下角，不让一个坏文件影响 Server 启动。
inline std::optional<FloatingToolbarPosition>
parse_floating_toolbar_position(std::string_view text) {
  const auto document = nlohmann::json::parse(text, nullptr, false);
  if (!document.is_object())
    return std::nullopt;
  FloatingToolbarPosition position;
  for (const auto &[key, target] :
       {std::pair{"x", &position.x}, std::pair{"y", &position.y}}) {
    if (!document.contains(key) || !document.at(key).is_number_integer())
      return std::nullopt;
    // 超出 int64 的无符号数换成有符号数会绕回范围里，先单独挡掉。
    if (document.at(key).is_number_unsigned() && document.at(key).get<uint64_t>() > 32767)
      return std::nullopt;
    const auto value = document.at(key).get<int64_t>();
    if (value < -32768 || value > 32767)
      return std::nullopt;
    *target = static_cast<int>(value);
  }
  return position;
}

inline std::string serialize_floating_toolbar_position(FloatingToolbarPosition position) {
  return nlohmann::json{{"x", position.x}, {"y", position.y}}.dump();
}
} // namespace msime::windows
