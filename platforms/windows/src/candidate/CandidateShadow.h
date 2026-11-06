#pragma once

#include "CandidateCardSize.h"
#include <cmath>
#include <cstdint>
#include <stdexcept>

namespace msime::windows {
// Transparent space around the composed candidate card. The values cover the
// two CSS-equivalent shadow passes through the same alpha threshold used by
// the Windows baseline, so neither the blur nor its right/down offset is cut
// by the popup HWND.
struct CandidateShadowInsets {
  double left = 32.0;
  double top = 20.0;
  double right = 32.0;
  double bottom = 40.0;
};

struct CandidateShadowFrame {
  double width = 0.0;
  double height = 0.0;
  double card_left = 0.0;
  double card_top = 0.0;
  double card_width = 0.0;
  double card_height = 0.0;
};

inline CandidateShadowFrame
candidate_shadow_frame(double card_width, double card_height,
                       double decoration_height,
                       CandidateShadowInsets insets = {}) {
  const auto valid = [](double value) {
    return std::isfinite(value) && value >= 0.0;
  };
  if (!valid(card_width) || !valid(card_height) || !valid(decoration_height) ||
      !valid(insets.left) || !valid(insets.top) || !valid(insets.right) ||
      !valid(insets.bottom) || card_width <= 0.0 || card_height <= 0.0)
    throw std::invalid_argument("Invalid candidate shadow frame");
  return {insets.left + card_width + insets.right,
          insets.top + decoration_height + card_height + insets.bottom,
          insets.left,
          insets.top + decoration_height,
          card_width,
          card_height};
}

struct CandidateShadowPixels {
  int left = 0;
  int top = 0;
  int right = 0;
  int bottom = 0;
};

// 窗口四周透明边距的设备像素：card_bounds 按它们给窗口留边，绘制时也必须按同样的整像素扣回去（candidate_card_extent）。候选缩放不是 100% 时 32 DIP 不是整像素，例如 110% 是 35.2，取 35。
inline CandidateShadowPixels candidate_shadow_pixels(const CandidateShadowInsets &insets, double scale) {
  return {static_cast<int>(std::lround(insets.left * scale)), static_cast<int>(std::lround(insets.top * scale)),
          static_cast<int>(std::lround(insets.right * scale)), static_cast<int>(std::lround(insets.bottom * scale))};
}
// 卡片（canvas 时是整块面板）的像素尺寸，向上取整：绘制时换回的 DIP 只会比量出来的宽，不会窄。
inline int64_t candidate_card_pixels(double dip, double scale) {
  return static_cast<int64_t>(std::ceil(dip * scale));
}
// 绘制时卡片拿到的 DIP 长度：客户区 `client` 像素扣掉两侧阴影的整像素（candidate_shadow_pixels），再按和 card_bounds 同一个 scale 换回 DIP，全程用 double。#10856：原来是拿 float 的客户区 DIP 减去没取整的 DIP 边距，边距被四舍五入掉的那零点几像素算到了卡片头上，绘制时的卡片比量尺寸时窄，横排「一行放不放得下」翻面，最后一个候选折到第二行、画到卡片外面。
inline double candidate_card_extent(int64_t client, int before, int after, double scale) {
  return static_cast<double>(client - before - after) / scale;
}

inline CandidateBounds candidate_shadow_bounds(CandidatePlacementInput content,
                                               CandidateShadowPixels shadow) {
  if (shadow.left < 0 || shadow.top < 0 || shadow.right < 0 ||
      shadow.bottom < 0)
    throw std::invalid_argument("Invalid candidate shadow margins");
  content.work_left += shadow.left;
  content.work_top += shadow.top;
  content.work_right -= shadow.right;
  content.work_bottom -= shadow.bottom;
  if (content.work_right <= content.work_left ||
      content.work_bottom <= content.work_top)
    throw std::invalid_argument("Candidate shadow exceeds work area");
  const auto placed = candidate_card_placement(content);
  return {placed.x - shadow.left, placed.y - shadow.top,
          content.width + shadow.left + shadow.right,
          content.height + shadow.top + shadow.bottom};
}
} // namespace msime::windows
