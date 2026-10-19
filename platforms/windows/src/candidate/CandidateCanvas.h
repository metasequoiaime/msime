#pragma once
#include "CandidateCardSize.h"
#include "CandidateSkinLayout.h"
#include <algorithm>
#include <cmath>
#include <functional>
#include <optional>
#include <string>
#include <utility>
#include <vector>

namespace msime::windows {
// 皮肤 `base = "canvas"` 的画布：`[candidate_window.border_image]` 九宫格面板、`content_insets_dip`、`outer_insets_dip`、`line_gap_dip` 和最多 32 个 `[[candidate_window.decorations]]` 自由挂件，来自共享 catalog 的 `SkinSummary`（`canvas`、`borderImage`、`contentInsetsDip`、`outerInsetsDip`、`lineGapDip`、`decorations`）。含义照 msime-windows c0a9d371 的 candidate_skin_layers.cpp（切片、边宽、内容内边距、挂件分层）、candidate_skin_overlay.cpp（挂件的锚点、偏移和等比缩放）、candidate_skin_scene.cpp（面板替代底色、外框和阴影，外留白加在窗口四周，行距落在拼音行下方）和 msimeui 的 DrawNineSlice（CSS `border-image` 的拉伸、越界切片、边宽整体缩小、对齐设备像素）。这里只做数值换算，不碰 JSON、文件和 Direct2D，没有 Windows 的机器上也能测。

// 上右下左四条边，单位按用途是 DIP 或源图像素。
struct CandidateCanvasEdges {
  double top = 0.0, right = 0.0, bottom = 0.0, left = 0.0;
};
struct CandidateCanvasRect {
  double left = 0.0, top = 0.0, right = 0.0, bottom = 0.0;
  double width() const { return right - left; }
  double height() const { return bottom - top; }
};
// CSS 四值简写：1 个值四边相同，2 个值是「上下 左右」，4 个值是「上右下左」，其余个数四边都是 0（catalog 只放行 1、2、4 个）。
inline CandidateCanvasEdges candidate_canvas_box(const std::vector<double> &values) {
  if (values.size() == 1)
    return {values[0], values[0], values[0], values[0]};
  if (values.size() == 2)
    return {values[0], values[1], values[0], values[1]};
  if (values.size() == 4)
    return {values[0], values[1], values[2], values[3]};
  return {};
}
enum class CandidateCanvasAnchor {
  top_left, top_right, bottom_left, bottom_right, left, right, top, bottom, center
};
inline std::optional<CandidateCanvasAnchor> candidate_canvas_anchor(const std::string &name) {
  static const std::pair<const char *, CandidateCanvasAnchor> names[] = {
      {"top-left", CandidateCanvasAnchor::top_left},
      {"top-right", CandidateCanvasAnchor::top_right},
      {"bottom-left", CandidateCanvasAnchor::bottom_left},
      {"bottom-right", CandidateCanvasAnchor::bottom_right},
      {"left", CandidateCanvasAnchor::left},
      {"right", CandidateCanvasAnchor::right},
      {"top", CandidateCanvasAnchor::top},
      {"bottom", CandidateCanvasAnchor::bottom},
      {"center", CandidateCanvasAnchor::center}};
  for (const auto &[text, anchor] : names)
    if (name == text)
      return anchor;
  return std::nullopt;
}
// 九宫格面板。`source` 与 `light_source` 是资源键（候选窗用绝对路径，设置离屏预览用已安全读取的包内相对键；浅色主题有 `light_source` 时换那张图，几何不变）；`slice` 是切片线，以源图像素计，写了 `stretch_dip` 时用它，否则用 `slice_dip`；`width` 是四边在屏幕上的宽（DIP），没写 `width_dip` 时取切片的数值当 DIP（CSS `border-image-width` 缺省等于切片）；`fill` 决定画不画中心块。
struct CandidateCanvasBorder {
  std::wstring source, light_source;
  CandidateCanvasEdges slice, width;
  bool fill = true;
};
// 一个自由挂件：宽 `width`、高 `height` 的框按 `anchor` 贴着卡片，再平移 `offset_x` / `offset_y`（正数向右、向下）；图片在框里等比缩放、居中。`z_index` 低于 candidate_canvas_front_z 的画在面板之下。
struct CandidateCanvasDecoration {
  std::wstring image;
  CandidateCanvasAnchor anchor = CandidateCanvasAnchor::right;
  double offset_x = 0.0, offset_y = 0.0, width = 0.0, height = 0.0;
  int z_index = 2;
};
// 一个 canvas 包的画布。`active` 为 false（不是 canvas 包或没有皮肤）时其余字段都不看，卡片和以前逐位相同。
struct CandidateSkinCanvas {
  bool active = false;
  std::optional<CandidateCanvasBorder> border;
  std::optional<CandidateCanvasEdges> content_insets, outer_insets;
  std::optional<double> line_gap;
  // 按清单的书写顺序；candidate_canvas_layers 再按 z_index 分层。
  std::vector<CandidateCanvasDecoration> decorations;
};
// 挂件的 z_index 低于它的画在面板之下，其余画在面板与候选文字之上。面板自身相当于 z_index = 1（msime-windows 的 kDecorationFrontZIndex）。
inline constexpr int candidate_canvas_front_z = 1;
// 自动为挂件多留的窗口空白每边不超过这么多 DIP，与 `outer_insets_dip` 的上限相同；偏移写到上千 DIP 的挂件不会把窗口撑到整块屏幕那么大，超出的部分照样被窗口裁掉。
inline constexpr double candidate_canvas_max_reach = 500.0;

// 这一种明暗下要画的面板图：浅色主题有 `light_source` 时用它，否则用 `source`。
inline std::wstring candidate_canvas_panel_source(const CandidateCanvasBorder &border, bool light) {
  return light && !border.light_source.empty() ? border.light_source : border.source;
}
// 候选窗这一种明暗下真正要画的画布：面板只留这一种明暗的那张图（`source`，`light_source` 清空），解不出来时 `source` 为空、卡片画回退底色；挂件只留解得出来的那些。切片、边宽、内边距、外留白和行距照清单保留：版式不因为一张图解不出来而变。`decodes` 由宿主给出（Windows 上是 WIC 完整解码第一帧）。
inline CandidateSkinCanvas candidate_canvas_drawable(
    CandidateSkinCanvas canvas, bool light,
    const std::function<bool(const std::wstring &)> &decodes) {
  if (!canvas.active)
    return {};
  if (canvas.border) {
    auto source = candidate_canvas_panel_source(*canvas.border, light);
    canvas.border->source = !source.empty() && decodes(source) ? std::move(source) : std::wstring{};
    canvas.border->light_source.clear();
  }
  canvas.decorations.erase(
      std::remove_if(canvas.decorations.begin(), canvas.decorations.end(),
                     [&decodes](const CandidateCanvasDecoration &decoration) {
                       return decoration.image.empty() || !decodes(decoration.image);
                     }),
      canvas.decorations.end());
  return canvas;
}
// 面板图会不会画出来：只有这时候选窗才向共享层要 `canvas_panel_drawn` 配色（包为贴图挑的原配色），否则用不透明 `canvas` 外观的可读配色。
inline bool candidate_canvas_panel_drawn(const CandidateSkinCanvas &canvas) {
  return canvas.active && canvas.border && !canvas.border->source.empty();
}

// 本宿主卡片自己的内边距（卡片坐标里内容离卡片四边的距离）：行左右各缩进 pad_x / 2，首行顶在 pad_y，最后一行下面留 slack_y。
inline CandidateCanvasEdges candidate_canvas_base_padding(const CandidateCardMetrics &metrics) {
  return {metrics.pad_y, metrics.pad_x / 2.0, metrics.slack_y, metrics.pad_x / 2.0};
}
// 面板外缘到内容的距离比本宿主卡片自己的内边距多出多少（DIP，可以是负数）。msime-windows 的 ResolveContentPadding：写了 `content_insets_dip` 时就是它，但每边不小于九宫格的边宽（文字不压在边框贴图上）；没写时是九宫格边宽加上基底的内边距。本宿主的版式照旧按自己的内边距排，整块内容再挪进面板这么多，面板也就比内容的卡片大出这么多。
inline CandidateCanvasEdges candidate_canvas_content_offset(const CandidateSkinCanvas &canvas,
                                                           const CandidateCanvasEdges &base) {
  if (!canvas.active)
    return {};
  const auto border = canvas.border ? canvas.border->width : CandidateCanvasEdges{};
  if (!canvas.content_insets)
    return border;
  const auto &insets = *canvas.content_insets;
  return {(std::max)(insets.top, border.top) - base.top,
          (std::max)(insets.right, border.right) - base.right,
          (std::max)(insets.bottom, border.bottom) - base.bottom,
          (std::max)(insets.left, border.left) - base.left};
}
// `line_gap_dip` 是拼音行与候选之间的距离，只在显示拼音时生效：msime-windows 把它落在拼音行的下外边距上（再抵掉 body 的 2 DIP 行距），本宿主的首行和候选之间没有那段行距，所以拼音行的下外边距就是它，盖过版式表里的 `preedit_margin_dip.bottom`。
inline CandidateSkinMetrics candidate_canvas_skin_metrics(CandidateSkinMetrics skin,
                                                          const CandidateSkinCanvas &canvas) {
  if (canvas.active && canvas.line_gap)
    skin.preedit_margin.bottom = *canvas.line_gap;
  return skin;
}

// 挂件的框（`panel` 是面板也就是卡片的矩形，同一坐标系）。照 msime-windows CandidateSkinOverlay::Render：没写锚点或写 `right` 时贴右边、竖直居中。
inline CandidateCanvasRect candidate_canvas_decoration_box(const CandidateCanvasDecoration &decoration,
                                                           const CandidateCanvasRect &panel) {
  const double width = decoration.width, height = decoration.height;
  double x = panel.right - width;
  double y = panel.top + (panel.height() - height) * 0.5;
  const double centre_x = panel.left + (panel.width() - width) * 0.5;
  switch (decoration.anchor) {
  case CandidateCanvasAnchor::top_left:
    x = panel.left;
    y = panel.top;
    break;
  case CandidateCanvasAnchor::top_right:
    y = panel.top;
    break;
  case CandidateCanvasAnchor::bottom_left:
    x = panel.left;
    y = panel.bottom - height;
    break;
  case CandidateCanvasAnchor::bottom_right:
    y = panel.bottom - height;
    break;
  case CandidateCanvasAnchor::left:
    x = panel.left;
    break;
  case CandidateCanvasAnchor::right:
    break;
  case CandidateCanvasAnchor::top:
    x = centre_x;
    y = panel.top;
    break;
  case CandidateCanvasAnchor::bottom:
    x = centre_x;
    y = panel.bottom - height;
    break;
  case CandidateCanvasAnchor::center:
    x = centre_x;
    break;
  }
  x += decoration.offset_x;
  y += decoration.offset_y;
  return {x, y, x + width, y + height};
}
// 图片在框里等比缩放、居中（CSS `background-size: contain`），比例不同也不变形。图或框没有大小时不画。
inline std::optional<CandidateCanvasRect> candidate_canvas_contain(const CandidateCanvasRect &box,
                                                                   double natural_width,
                                                                   double natural_height) {
  if (!(box.width() > 0.0) || !(box.height() > 0.0) || !(natural_width > 0.0) ||
      !(natural_height > 0.0))
    return std::nullopt;
  const double scale = (std::min)(box.width() / natural_width, box.height() / natural_height);
  const double width = natural_width * scale, height = natural_height * scale;
  const double left = box.left + (box.width() - width) * 0.5;
  const double top = box.top + (box.height() - height) * 0.5;
  return CandidateCanvasRect{left, top, left + width, top + height};
}
// 按 z_index 升序稳定排序（同层保持书写顺序），再以 candidate_canvas_front_z 为界分成面板之下和面板之上两层，各自按顺序画，先画的在下面。
struct CandidateCanvasLayers {
  std::vector<CandidateCanvasDecoration> below, above;
};
inline CandidateCanvasLayers candidate_canvas_layers(std::vector<CandidateCanvasDecoration> decorations) {
  std::stable_sort(decorations.begin(), decorations.end(),
                   [](const CandidateCanvasDecoration &a, const CandidateCanvasDecoration &b) {
                     return a.z_index < b.z_index;
                   });
  CandidateCanvasLayers layers;
  for (auto &decoration : decorations)
    (decoration.z_index < candidate_canvas_front_z ? layers.below : layers.above)
        .push_back(std::move(decoration));
  return layers;
}

// 卡片（面板）四周的窗口空白（DIP）。不是 canvas 时就是 `base`（阴影的透明边距）。canvas 时照 msime-windows 在阴影边距之外再加 `outer_insets_dip`；挂件的框伸出面板更远时按伸出去的距离留（每边不超过 candidate_canvas_max_reach），所以没写或写小了外留白的包，挂件也不会被窗口裁掉。`available_width` / `available_height` 是工作区能给的总宽高（DIP，含面板），大于 0 时为挂件多留的那部分按比例收小到放得下，阴影边距不收：窗口放不进工作区时候选窗会整个画不出来，挂件被裁掉一截要好得多。
inline CandidateCanvasEdges candidate_canvas_margins(const CandidateSkinCanvas &canvas,
                                                    const CandidateCanvasEdges &base,
                                                    double panel_width, double panel_height,
                                                    double available_width = 0.0,
                                                    double available_height = 0.0) {
  if (!canvas.active)
    return base;
  auto reach = canvas.outer_insets.value_or(CandidateCanvasEdges{});
  const CandidateCanvasRect panel{0.0, 0.0, panel_width, panel_height};
  for (const auto &decoration : canvas.decorations) {
    const auto box = candidate_canvas_decoration_box(decoration, panel);
    reach.left = (std::max)(reach.left, -box.left);
    reach.top = (std::max)(reach.top, -box.top);
    reach.right = (std::max)(reach.right, box.right - panel_width);
    reach.bottom = (std::max)(reach.bottom, box.bottom - panel_height);
  }
  auto clamp = [](double value) {
    return std::isfinite(value) ? std::clamp(value, 0.0, candidate_canvas_max_reach) : 0.0;
  };
  reach = {clamp(reach.top), clamp(reach.right), clamp(reach.bottom), clamp(reach.left)};
  auto fit = [](double &before, double &after, double fixed, double available) {
    const double wanted = before + after;
    if (!(available > 0.0) || !(wanted > 0.0))
      return;
    const double factor = std::clamp((available - fixed) / wanted, 0.0, 1.0);
    before *= factor;
    after *= factor;
  };
  fit(reach.left, reach.right, panel_width + base.left + base.right, available_width);
  fit(reach.top, reach.bottom, panel_height + base.top + base.bottom, available_height);
  return {base.top + reach.top, base.right + reach.right, base.bottom + reach.bottom,
          base.left + reach.left};
}

// 把 DIP 坐标挪到最近的设备像素上：渲染目标的 DPI 乘上当前变换的缩放，再加上变换的平移。变换带旋转或斜切、或缩放不是正数时不挪（enabled 为 false）。
struct CandidateCanvasSnap {
  bool enabled = false;
  double scale_x = 1.0, scale_y = 1.0, offset_x = 0.0, offset_y = 0.0;
  double x(double value) const {
    return enabled ? (std::round(value * scale_x + offset_x) - offset_x) / scale_x : value;
  }
  double y(double value) const {
    return enabled ? (std::round(value * scale_y + offset_y) - offset_y) / scale_y : value;
  }
};
// 九宫格的一块：源图里的矩形（源图的 DIP，位图按 96 DPI 建，等于像素）和画到哪里。
struct CandidateCanvasTile {
  CandidateCanvasRect source, destination;
};
// CSS `border-image`（`border-image-repeat: stretch`）把一张 `pixel_width` × `pixel_height` 像素、`dip_width` × `dip_height` DIP 的图画进 `destination`：沿 `slice`（源图像素）切成九块，四角从切片缩放到 `width`（DIP），四边沿长度拉伸，中间两个方向都拉伸、只在 `fill` 时画。超出图片的切片按整张图算；对边的宽加起来放不下时四边按同一个比例缩小；每块的边落在设备像素上（`snap`），块与块之间不留缝。左右（上下）切片相接或交叉时，中间那一列（行）是空的。
inline std::vector<CandidateCanvasTile> candidate_nine_slice(
    double pixel_width, double pixel_height, double dip_width, double dip_height,
    const CandidateCanvasRect &destination, const CandidateCanvasEdges &slice,
    const CandidateCanvasEdges &width, bool fill, const CandidateCanvasSnap &snap = {}) {
  std::vector<CandidateCanvasTile> tiles;
  if (!(pixel_width > 0.0) || !(pixel_height > 0.0) || !(dip_width > 0.0) ||
      !(dip_height > 0.0) || !(destination.width() > 0.0) || !(destination.height() > 0.0))
    return tiles;
  auto non_negative = [](double value) {
    return std::isfinite(value) ? (std::max)(0.0, value) : 0.0;
  };
  const double slice_left = (std::min)(non_negative(slice.left), pixel_width);
  const double slice_top = (std::min)(non_negative(slice.top), pixel_height);
  const double slice_right = (std::min)(non_negative(slice.right), pixel_width);
  const double slice_bottom = (std::min)(non_negative(slice.bottom), pixel_height);
  double left = non_negative(width.left), top = non_negative(width.top),
         right = non_negative(width.right), bottom = non_negative(width.bottom);
  double factor = 1.0;
  if (left + right > destination.width())
    factor = (std::min)(factor, destination.width() / (left + right));
  if (top + bottom > destination.height())
    factor = (std::min)(factor, destination.height() / (top + bottom));
  left *= factor;
  top *= factor;
  right *= factor;
  bottom *= factor;
  const double x0 = snap.x(destination.left), x3 = snap.x(destination.right);
  const double x1 = (std::min)(snap.x(destination.left + left), x3);
  const double x2 = (std::max)(snap.x(destination.right - right), x1);
  const double y0 = snap.y(destination.top), y3 = snap.y(destination.bottom);
  const double y1 = (std::min)(snap.y(destination.top + top), y3);
  const double y2 = (std::max)(snap.y(destination.bottom - bottom), y1);
  const double dx[4] = {x0, x1, x2, x3}, dy[4] = {y0, y1, y2, y3};
  const double source_left[3] = {0.0, slice_left, pixel_width - slice_right};
  const double source_right[3] = {slice_left, pixel_width - slice_right, pixel_width};
  const double source_top[3] = {0.0, slice_top, pixel_height - slice_bottom};
  const double source_bottom[3] = {slice_top, pixel_height - slice_bottom, pixel_height};
  const double to_dip_x = dip_width / pixel_width, to_dip_y = dip_height / pixel_height;
  for (int row = 0; row < 3; ++row)
    for (int column = 0; column < 3; ++column) {
      if (row == 1 && column == 1 && !fill)
        continue;
      if (!(source_right[column] > source_left[column]) ||
          !(source_bottom[row] > source_top[row]) || !(dx[column + 1] > dx[column]) ||
          !(dy[row + 1] > dy[row]))
        continue;
      tiles.push_back({{source_left[column] * to_dip_x, source_top[row] * to_dip_y,
                        source_right[column] * to_dip_x, source_bottom[row] * to_dip_y},
                       {dx[column], dy[row], dx[column + 1], dy[row + 1]}});
    }
  return tiles;
}
} // namespace msime::windows
