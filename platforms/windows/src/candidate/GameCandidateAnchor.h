#pragma once
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <optional>
#include <windows.h>

namespace msime::windows {
// 游戏会话兜底定位的输入。anchor 为空表示宿主报的是 INVALID_Y（还没有可用的文本 extent）；client 是前台窗口客户区的屏幕坐标，scale 是它的 DPI 缩放。
struct GameAnchorInput {
  std::optional<POINT> anchor;
  bool game_host = false;
  // 前台窗口存在、属于这个客户端的进程，并且客户区不为空。用户 Alt-Tab 切走或前台是启动器时为 false，候选窗不会锚到别的应用上。
  bool foreground_owned = false;
  RECT client{};
  double scale = 1.0;
};
// 游戏给不出可信光标时，把候选窗放在游戏客户区左下部，那里通常是聊天输入框的位置。只对游戏会话生效：锚点是 INVALID_Y，或是垃圾值（落在客户区外，或贴着客户区顶边；锚点取的是文本 extent 的 {left, bottom}，真实的文本行不会贴着顶边）。锚点可信时返回空，调用方照用宿主给的锚点。
inline std::optional<POINT> game_candidate_anchor(const GameAnchorInput &in) {
  if (!in.game_host || !in.foreground_owned)
    return std::nullopt;
  const int64_t width = int64_t(in.client.right) - in.client.left;
  const int64_t height = int64_t(in.client.bottom) - in.client.top;
  if (width <= 0 || height <= 0)
    return std::nullopt;
  if (in.anchor) {
    const auto &point = *in.anchor;
    const bool outside = point.x < in.client.left || point.x > in.client.right ||
                         point.y < in.client.top || point.y > in.client.bottom;
    if (!outside && int64_t(point.y) - in.client.top >= 2)
      return std::nullopt;
  }
  const double scale = std::isfinite(in.scale) && in.scale > 0.0 ? in.scale : 1.0;
  const int64_t inset = (std::max)(static_cast<int64_t>(std::lround(24.0 * scale)),
                                   width / 20);
  return POINT{static_cast<LONG>(in.client.left + inset),
               static_cast<LONG>(in.client.bottom - height / 5)};
}
} // namespace msime::windows
