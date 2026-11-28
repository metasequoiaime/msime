#pragma once
#include "CandidatePalette.h"
#include <algorithm>
#include <optional>

namespace msime::windows {
// 用户设的候选窗样式：`candidate_scale_percent`、`candidate_opacity_percent`、`candidate_corner_radius` 和候选行高与行距。这份结构不带 JSON，没有 JSON 库的设置窗口因此能用画卡片时的同一套规则画预览。
struct CandidateWindowStyle {
  unsigned scale_percent = 100;
  unsigned opacity_percent = 100;
  // None follows the skin package's radius, else the palette's own.
  std::optional<unsigned> corner_radius;
  // 候选文字上下各留的空白（`candidate_item_padding_vertical`，0–16 DIP）；缺省时按皮肤与窗口布局。
  std::optional<unsigned> item_padding_vertical;
  // 相邻候选行之间、横排时相邻列之间的空隙（`candidate_item_spacing`，0–16 DIP）；缺省时按皮肤与窗口布局。
  std::optional<unsigned> item_spacing;
  bool valid() const {
    return scale_percent >= 50 && scale_percent <= 200 &&
           opacity_percent >= 50 && opacity_percent <= 100 &&
           (!corner_radius || *corner_radius <= 32) &&
           (!item_padding_vertical || *item_padding_vertical <= 16) &&
           (!item_spacing || *item_spacing <= 16);
  }
  // Multiplies the DPI scale: every DIP of the card, its fonts included, grows by this factor.
  double scale() const { return static_cast<double>(scale_percent) / 100.0; }
  float opacity() const { return static_cast<float>(opacity_percent) / 100.0f; }
  bool operator==(const CandidateWindowStyle &other) const {
    return scale_percent == other.scale_percent &&
           opacity_percent == other.opacity_percent &&
           corner_radius == other.corner_radius &&
           item_padding_vertical == other.item_padding_vertical &&
           item_spacing == other.item_spacing;
  }
  bool operator!=(const CandidateWindowStyle &other) const {
    return !(*this == other);
  }
};
// The card radius: the user's value, else the skin package's, else the palette's.
inline float candidate_card_radius(const CandidateWindowStyle &style,
                                   std::optional<float> skin, float palette) {
  if (style.corner_radius)
    return static_cast<float>(*style.corner_radius);
  return skin.value_or(palette);
}
// A row's highlight never rounds more than a card radius the user chose, so a square card keeps square rows. A skin package's radius leaves the rows at the palette's own shape, as they were drawn before the setting existed.
inline float candidate_row_radius(const CandidateWindowStyle &style,
                                  float palette_row, float card) {
  if (!style.corner_radius)
    return palette_row;
  return (std::max)(0.0f, (std::min)(palette_row, card));
}
// The card's surface, border and background image fade; text, numbers and the selection keep their own alpha.
inline CandidateColor candidate_faded(CandidateColor color,
                                      const CandidateWindowStyle &style) {
  color.a *= style.opacity();
  return color;
}
} // namespace msime::windows
