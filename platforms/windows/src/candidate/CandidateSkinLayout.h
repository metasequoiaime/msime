#pragma once
#include <algorithm>
#include <cmath>
#include <optional>

namespace msime::windows {
// 皮肤包 `[candidate_window]` 里的版式、外框、阴影、材质、翻页箭头、收紧宽度和卷轴展开这几组键，来自共享 catalog 的 `SkinSummary`（`geometry` / `horizontal` / `vertical`、`borderWidthDip`、`itemCornerRadiusDip`、`shadow`、`backdrop`、`pageArrows`、`fitContent`、`expandPanel`、`expandRows`），含义照 msime-windows c0a9d371 的 candidate_skin_tokens.cpp 与 candidate_skin_scene.cpp。没写的键为空，卡片保留宿主自己的值：不写这些键的皮肤和没有皮肤时，画出来与以前一样。这里不碰 JSON 和 Direct2D，换算规则在没有 Windows 的机器上也能测。
//
// msime-windows 里没写的键沿用 `base` 外观自己的出厂清单（例如 `wechat` 的 `padding = 2`）；有 Windows 外观基底或 canvas 基底的包，共享 catalog 给的 `resolvedLayout` 就是这份出厂版式叠上包自己写的键（见 `shared`）。只有不带 `resolvedLayout` 的包，没写的键才回到本宿主的默认版式。

// 版式长度：固定 DIP，或按候选字号缩放的 em。em 换算后取整，同 msime-windows 的 CandidateSkinCatalog::ResolveLength。
struct CandidateSkinLength {
  double value = 0.0;
  bool em = false;
};
inline double candidate_skin_length(const CandidateSkinLength &length,
                                    double font_size) {
  return length.em ? std::round(font_size * length.value) : length.value;
}
struct CandidateSkinInsets {
  double top = 0.0, right = 0.0, bottom = 0.0, left = 0.0;
};
enum class CandidatePageArrowStyle { chevron, triangle };
// 一张版式表：`[candidate_window]` 本身，或只对一种排列生效的 `[candidate_window.horizontal]` / `[candidate_window.vertical]`。`trim_hidden_preedit_gap` 不在这里：msime-windows 用它抵掉预编辑行隐藏后 body 仍留着的 2 DIP 行距，本宿主的首行和候选之间本来就没有这段行距，这个键在这里什么也不用做。
struct CandidateSkinGeometry {
  std::optional<CandidateSkinLength> padding, item_gap, item_extra_height,
      number_inset, item_padding_right, item_min_width, number_gap,
      selected_bar_width;
  std::optional<double> number_scale, selected_bar_height_em;
  std::optional<bool> selected_bar_inside, item_corners_follow_card, justify_rows;
  std::optional<CandidatePageArrowStyle> page_arrow_style;
  std::optional<CandidateSkinInsets> preedit_margin;
};
enum class CandidateSkinShadow { none, soft, strong };
enum class CandidateSkinBackdrop { none, acrylic, mica };
struct CandidateSkinLayout {
  CandidateSkinGeometry geometry, horizontal, vertical;
  // `border_width_dip`（0-4）与 `item_corner_radius_dip`（0-16），没写时用调色板自己的线宽和高亮圆角。
  std::optional<float> border_width, item_corner_radius;
  std::optional<CandidateSkinShadow> shadow;
  // msime-windows 里没写等于 `none`。
  CandidateSkinBackdrop backdrop = CandidateSkinBackdrop::none;
  // 包有九宫格面板（`border_image`，只在 `base = "canvas"` 时有）。面板盖住整张卡片，msime-windows 这时不垫系统材质（CandidateBackdrop.h）。
  bool panel_image = false;
  std::optional<bool> page_arrows, fit_content;
  // 包带着共享层合并好的版式（catalog 的 `resolvedLayout`：Windows 外观基底和 canvas 的出厂清单叠上包自己的声明）。和 macOS 一样，带着它的卡片用 msime-windows 固定的 160 DIP 最小宽度，没有它的原生主题用 7em。
  bool shared = false;
  // `expand_panel`（横排的卷轴展开）与 `expand_rows`（展开后一屏的网格行数，2-8），见 CandidateExpandPanel.h。
  std::optional<bool> expand_panel;
  std::optional<int> expand_rows;
};
// 当前排列下生效的版式表：横排或竖排表里写了的键盖过共享表，同 msime-windows 的 ApplyLayer 先套 geometry 再套排列表。
inline CandidateSkinGeometry candidate_skin_geometry(const CandidateSkinLayout &layout,
                                                     bool horizontal) {
  auto result = layout.geometry;
  const auto &over = horizontal ? layout.horizontal : layout.vertical;
  auto take = [](auto &target, const auto &value) {
    if (value)
      target = value;
  };
  take(result.padding, over.padding);
  take(result.item_gap, over.item_gap);
  take(result.item_extra_height, over.item_extra_height);
  take(result.number_inset, over.number_inset);
  take(result.item_padding_right, over.item_padding_right);
  take(result.item_min_width, over.item_min_width);
  take(result.number_gap, over.number_gap);
  take(result.selected_bar_width, over.selected_bar_width);
  take(result.number_scale, over.number_scale);
  take(result.selected_bar_height_em, over.selected_bar_height_em);
  take(result.selected_bar_inside, over.selected_bar_inside);
  take(result.item_corners_follow_card, over.item_corners_follow_card);
  take(result.justify_rows, over.justify_rows);
  take(result.page_arrow_style, over.page_arrow_style);
  take(result.preedit_margin, over.preedit_margin);
  return result;
}
// 按当前排列和候选字号解析好的版式，长度都是 DIP。空的键交给 candidate_card_metrics 用宿主自己的值；`horizontal` 记下解析时的排列，只对横排生效的键（`item_min_width`、`justify_rows`）和翻页箭头的摆法据此决定。
struct CandidateSkinMetrics {
  bool horizontal = false;
  std::optional<double> padding, item_gap, item_extra_height, number_inset,
      item_padding_right, item_min_width, number_gap, number_scale,
      selected_bar_width, selected_bar_height;
  bool selected_bar_inside = false, corners_follow_card = false,
       justify_rows = false, page_arrows = false, fit_content = false;
  // CandidateSkinLayout::shared：版式来自共享层合并好的出厂清单，最小宽度用 160 DIP。
  bool shared_layout = false;
  // 卷轴展开只在横排生效（msime-windows 的 ResolveExpandPanel 只在横排时问）；打开时 page_arrows 被忽略。expand_rows 没写时是 4。
  bool expand_panel = false;
  int expand_rows = 4;
  CandidatePageArrowStyle page_arrow_style = CandidatePageArrowStyle::chevron;
  CandidateSkinInsets preedit_margin{};
  // 调色板的 `show_selected_bar`（主题或皮肤包打开了选中竖条）。不是版式键，由窗口按当前调色板填上：缺省版式和 macOS 的 MSIMECandidateTextLeft(showSelectedBar) 一样，有竖条时序号多让出 6 DIP。
  bool show_selected_bar = false;
};
inline CandidateSkinMetrics candidate_skin_metrics(const CandidateSkinLayout &layout,
                                                   bool horizontal,
                                                   double font_size) {
  const auto geometry = candidate_skin_geometry(layout, horizontal);
  CandidateSkinMetrics metrics;
  metrics.horizontal = horizontal;
  auto length = [font_size](const std::optional<CandidateSkinLength> &value) {
    return value ? std::optional<double>{candidate_skin_length(*value, font_size)}
                 : std::nullopt;
  };
  metrics.padding = length(geometry.padding);
  metrics.item_gap = length(geometry.item_gap);
  metrics.item_extra_height = length(geometry.item_extra_height);
  metrics.number_inset = length(geometry.number_inset);
  metrics.item_padding_right = length(geometry.item_padding_right);
  metrics.item_min_width = length(geometry.item_min_width);
  metrics.number_gap = length(geometry.number_gap);
  metrics.selected_bar_width = length(geometry.selected_bar_width);
  metrics.number_scale = geometry.number_scale;
  // 选中竖条的高是候选字号的倍数，不是 em 长度，不取整（msime-windows 的 selectedBarHeight = fontSize * selectedBarHeightEm）。
  if (geometry.selected_bar_height_em)
    metrics.selected_bar_height = font_size * *geometry.selected_bar_height_em;
  metrics.selected_bar_inside = geometry.selected_bar_inside.value_or(false);
  metrics.corners_follow_card = geometry.item_corners_follow_card.value_or(false);
  metrics.justify_rows = geometry.justify_rows.value_or(false);
  metrics.page_arrow_style =
      geometry.page_arrow_style.value_or(CandidatePageArrowStyle::chevron);
  metrics.preedit_margin = geometry.preedit_margin.value_or(CandidateSkinInsets{});
  metrics.page_arrows = layout.page_arrows.value_or(false);
  metrics.fit_content = layout.fit_content.value_or(false);
  metrics.shared_layout = layout.shared;
  metrics.expand_panel = horizontal && layout.expand_panel.value_or(false);
  metrics.expand_rows = std::clamp(layout.expand_rows.value_or(4), 2, 8);
  if (metrics.expand_panel)
    metrics.page_arrows = false;
  return metrics;
}
// 卡片阴影的不透明度。皮肤的档位只缩放阴影的不透明度、不改几何（msime-windows 的 ShadowAlphaScale：soft 乘 0.5，strong 乘 1.6），`none` 不画；没写时是宿主自己的 `alpha`。Windows 上没有阴影的用户偏好，皮肤直接决定。
inline float candidate_skin_shadow_alpha(std::optional<CandidateSkinShadow> shadow,
                                         float alpha) {
  if (!shadow)
    return alpha;
  switch (*shadow) {
  case CandidateSkinShadow::none:
    return 0.0f;
  case CandidateSkinShadow::soft:
    return alpha * 0.5f;
  case CandidateSkinShadow::strong:
    return (std::min)(alpha * 1.6f, 1.0f);
  }
  return alpha;
}
// 要材质的皮肤在材质画不出来时（系统关闭透明效果、高对比度、建不起合成）卡片底色的不透明度：按 msime-windows 的回退把 `surface` 补成不透明，免得为材质准备的半透明底色直接透出后面的窗口。垫上材质时卡片不画底色，由 CandidateBackdrop.h 的 candidate_backdrop_surface_alpha 决定。用户的候选窗不透明度之后照常作用在它上面。
inline float candidate_skin_surface_alpha(CandidateSkinBackdrop backdrop,
                                          float alpha) {
  return backdrop == CandidateSkinBackdrop::none ? alpha : 1.0f;
}
} // namespace msime::windows
