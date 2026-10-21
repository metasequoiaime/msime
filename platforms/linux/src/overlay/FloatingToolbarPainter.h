#pragma once

#include <cairo/cairo.h>
#include <pango/pangocairo.h>

#include <algorithm>
#include <cmath>
#include <optional>
#include <string>
#include <vector>

#include "FloatingPanelColors.h"
#include "FloatingToolbarModel.h"
#include "OverlayPaint.h"

namespace msime::linux_host {

// 悬浮工具栏、它弹出的菜单和按钮提示的画法。X11 后端把像素放进 override-redirect 窗口，单测把它们画进图像表面；两边用同一份画法，不会各画各的。

// 一个图标画在边长 side、左上角 (x, y) 的方格里，线宽随方格缩放。对应 macOS 工具栏用的 SF Symbols（常规字重）。
inline void floating_toolbar_paint_icon(cairo_t *cairo, FloatingToolbarIcon icon, double x, double y, double side,
                                        std::uint32_t color) {
  if (icon == FloatingToolbarIcon::Glyph || side <= 0) return;
  cairo_save(cairo);
  cairo_new_path(cairo);
  cairo_translate(cairo, x, y);
  cairo_scale(cairo, side, side);
  mode_badge_set_rgb(cairo, color);
  cairo_set_line_cap(cairo, CAIRO_LINE_CAP_ROUND);
  cairo_set_line_join(cairo, CAIRO_LINE_JOIN_ROUND);
  cairo_set_line_width(cairo, 0.075);
  constexpr double pi = 3.14159265358979323846;
  switch (icon) {
  case FloatingToolbarIcon::List:
    // list.bullet：三个圆点各带一条横线。
    for (const double row : {0.27, 0.5, 0.73}) {
      cairo_arc(cairo, 0.2, row, 0.06, 0, 2 * pi);
      cairo_fill(cairo);
      cairo_move_to(cairo, 0.37, row);
      cairo_line_to(cairo, 0.86, row);
      cairo_stroke(cairo);
    }
    break;
  case FloatingToolbarIcon::Smile:
    // face.smiling：一个圆圈、两只眼睛和一道笑弧。
    cairo_arc(cairo, 0.5, 0.5, 0.39, 0, 2 * pi);
    cairo_stroke(cairo);
    cairo_arc(cairo, 0.36, 0.41, 0.045, 0, 2 * pi);
    cairo_fill(cairo);
    cairo_arc(cairo, 0.64, 0.41, 0.045, 0, 2 * pi);
    cairo_fill(cairo);
    cairo_arc(cairo, 0.5, 0.5, 0.21, 0.18 * pi, 0.82 * pi);
    cairo_stroke(cairo);
    break;
  case FloatingToolbarIcon::Handwriting:
    // hand.draw：一支斜放的笔和笔尖下一道波浪线。
    cairo_move_to(cairo, 0.44, 0.58);
    cairo_line_to(cairo, 0.8, 0.22);
    cairo_line_to(cairo, 0.88, 0.3);
    cairo_line_to(cairo, 0.52, 0.66);
    cairo_line_to(cairo, 0.4, 0.7);
    cairo_close_path(cairo);
    cairo_stroke(cairo);
    cairo_move_to(cairo, 0.1, 0.82);
    cairo_curve_to(cairo, 0.2, 0.68, 0.28, 0.68, 0.32, 0.8);
    cairo_curve_to(cairo, 0.36, 0.92, 0.46, 0.9, 0.56, 0.8);
    cairo_stroke(cairo);
    break;
  case FloatingToolbarIcon::Keyboard:
    // keyboard：圆角外框，两排键和一条空格键。
    mode_badge_rounded_rect(cairo, 0.06, 0.22, 0.88, 0.56, 0.1);
    cairo_stroke(cairo);
    for (const double row : {0.37, 0.5})
      for (int column = 0; column < 5; ++column) {
        cairo_rectangle(cairo, 0.17 + column * 0.145 - 0.035, row - 0.035, 0.07, 0.07);
        cairo_fill(cairo);
      }
    cairo_move_to(cairo, 0.3, 0.65);
    cairo_line_to(cairo, 0.7, 0.65);
    cairo_stroke(cairo);
    break;
  case FloatingToolbarIcon::Microphone:
    // mic.fill：实心的话筒头，下面一道托架和支杆。
    mode_badge_rounded_rect(cairo, 0.36, 0.08, 0.28, 0.5, 0.14);
    cairo_fill(cairo);
    cairo_arc(cairo, 0.5, 0.43, 0.25, 0, pi);
    cairo_stroke(cairo);
    cairo_move_to(cairo, 0.5, 0.68);
    cairo_line_to(cairo, 0.5, 0.86);
    cairo_move_to(cairo, 0.36, 0.88);
    cairo_line_to(cairo, 0.64, 0.88);
    cairo_stroke(cairo);
    break;
  case FloatingToolbarIcon::Gear: {
    // gearshape：八个齿的齿轮轮廓和中间的圆孔。
    constexpr int teeth = 8;
    const double outer = 0.45, inner = 0.35;
    for (int step = 0; step < teeth * 4; ++step) {
      const double angle = (static_cast<double>(step) / (teeth * 4)) * 2 * pi - pi / 2;
      const double radius = (step % 4 == 1 || step % 4 == 2) ? outer : inner;
      const double px = 0.5 + radius * std::cos(angle), py = 0.5 + radius * std::sin(angle);
      if (step == 0) cairo_move_to(cairo, px, py);
      else cairo_line_to(cairo, px, py);
    }
    cairo_close_path(cairo);
    cairo_stroke(cairo);
    cairo_arc(cairo, 0.5, 0.5, 0.14, 0, 2 * pi);
    cairo_stroke(cairo);
    break;
  }
  case FloatingToolbarIcon::Glyph:
    break;
  }
  cairo_restore(cairo);
}

// 没有产品图标可读时画的简化标志，照 macOS 的后备画法：深色圆角方块、浅绿描边和一道白色的「水杉」笔画。
inline void floating_toolbar_paint_fallback_mark(cairo_t *cairo, double x, double y, double side, double scale) {
  mode_badge_rounded_rect(cairo, x, y, side, side, 4.0 * scale);
  mode_badge_set_rgb(cairo, 0x252525);
  cairo_fill_preserve(cairo);
  mode_badge_set_rgb(cairo, 0xA8DF8E);
  cairo_set_line_width(cairo, 2.0 * scale);
  cairo_stroke(cairo);
  // macOS 的路径在 y 向上的坐标里，这里翻成 y 向下。
  const auto point = [&](double fx, double fy) { return std::pair{x + side * fx, y + side * (1.0 - fy)}; };
  auto [x0, y0] = point(0.68, 0.88);
  cairo_move_to(cairo, x0, y0);
  for (const auto &[fx, fy] : {std::pair{0.28, 0.73}, std::pair{0.68, 0.62}, std::pair{0.28, 0.44}}) {
    auto [px, py] = point(fx, fy);
    cairo_line_to(cairo, px, py);
  }
  auto [c1x, c1y] = point(0.68, 0.38);
  auto [c2x, c2y] = point(0.42, 0.25);
  auto [ex, ey] = point(0.25, 0.16);
  cairo_curve_to(cairo, c1x, c1y, c2x, c2y, ex, ey);
  cairo_set_line_width(cairo, 2.5 * scale);
  cairo_set_line_cap(cairo, CAIRO_LINE_CAP_ROUND);
  cairo_set_line_join(cairo, CAIRO_LINE_JOIN_ROUND);
  mode_badge_set_rgb(cairo, 0xFFFFFF);
  cairo_stroke(cairo);
}

// 不能用的按钮和菜单项的字色：文字色在底色上约 35% 的一层，与 macOS 停用控件的淡色相近。
inline std::uint32_t floating_toolbar_disabled_ink(const FloatingToolbarColors &colors) {
  return composite_color(colors.text, 0x59, colors.surface);
}

// 一帧工具栏要画的东西。hovered / pressed 是按钮下标。
struct FloatingToolbarFrame {
  std::vector<FloatingToolbarFace> faces;
  std::optional<std::size_t> hovered;
  std::optional<std::size_t> pressed;
  // 产品图标（PNG）的路径；空或读不到时画简化标志。只在 show_logo 时用。
  std::string icon_path;
  bool show_logo = false;
};

// 在 layout 的逻辑像素里画整条工具栏（调用方把上下文缩放到设备像素并清成透明），照 macOS 的排法：阴影、圆角本体和描边，左端标志或握把，分隔线，再是一排按钮；指针停着或按下的按钮先垫一块圆角悬停底，半径取按钮高的四分之一（至少 2），与 macOS 的 MetasequoiaFloatingToolbarButton 相同。
inline void paint_floating_toolbar(cairo_t *cairo, const FloatingToolbarLayout &layout, const FloatingToolbarFrame &frame,
                                   const FloatingToolbarColors &colors) {
  const auto &plate = layout.plate;
  if (layout.margin > 0)
    overlay_paint_shadow(cairo, plate.x, plate.y, plate.width, plate.height, layout.radius,
                         static_cast<int>(layout.margin));
  overlay_paint_card(cairo, plate.x, plate.y, plate.width, plate.height, layout.radius, colors.surface, colors.border);

  const auto &handle = layout.handle;
  if (frame.show_logo) {
    const double side = std::min(std::min(handle.width, handle.height) - 4.0 * layout.scale,
                                 FloatingToolbarGeometry::logo_mark_side * layout.scale);
    const double mark_x = handle.x + (handle.width - side) / 2.0, mark_y = handle.y + (handle.height - side) / 2.0;
    if (auto *logo = mode_badge_logo(frame.icon_path)) {
      const double source_w = cairo_image_surface_get_width(logo), source_h = cairo_image_surface_get_height(logo);
      const double fit = side / std::max(source_w, source_h);
      cairo_save(cairo);
      cairo_translate(cairo, mark_x + (side - source_w * fit) / 2.0, mark_y + (side - source_h * fit) / 2.0);
      cairo_scale(cairo, fit, fit);
      cairo_set_source_surface(cairo, logo, 0, 0);
      cairo_pattern_set_filter(cairo_get_source(cairo), CAIRO_FILTER_BEST);
      cairo_paint(cairo);
      cairo_restore(cairo);
      cairo_surface_destroy(logo);
    } else {
      floating_toolbar_paint_fallback_mark(cairo, mark_x, mark_y, side, layout.scale);
    }
  } else {
    // 握把：两列三行的小圆点，颜色与分隔线相同。
    const double dot = 2.4 * layout.scale, pitch = 4.4 * layout.scale;
    mode_badge_set_rgb(cairo, colors.divider);
    const double center_x = handle.x + handle.width / 2.0, center_y = handle.y + handle.height / 2.0;
    for (int column = 0; column < 2; ++column)
      for (int row = 0; row < 3; ++row) {
        cairo_arc(cairo, center_x + (column - 0.5) * pitch, center_y + (row - 1) * pitch, dot / 2.0, 0, 6.2831853);
        cairo_fill(cairo);
      }
  }

  if (layout.divider_visible) {
    mode_badge_set_rgb(cairo, colors.divider);
    cairo_rectangle(cairo, layout.divider.x, layout.divider.y, layout.divider.width, layout.divider.height);
    cairo_fill(cairo);
  }

  for (std::size_t index = 0; index < layout.buttons.size() && index < frame.faces.size(); ++index) {
    const auto &cell = layout.buttons[index];
    const auto &face = frame.faces[index];
    // 不能用的按钮不垫悬停底，字和图标是文字色在底色上的一层淡色，与 macOS 停用的 NSButton 一样淡下去。
    if (face.enabled && (frame.hovered == index || frame.pressed == index)) {
      mode_badge_rounded_rect(cairo, cell.x, cell.y, cell.width, cell.height, std::max(2.0, cell.height * 0.25));
      mode_badge_set_rgb(cairo, colors.hover);
      cairo_fill(cairo);
    }
    const auto ink = face.enabled ? colors.text : floating_toolbar_disabled_ink(colors);
    if (face.icon != FloatingToolbarIcon::Glyph) {
      const double side = std::min(layout.icon_size, cell.height);
      floating_toolbar_paint_icon(cairo, face.icon, cell.x + (cell.width - side) / 2.0, cell.y + (cell.height - side) / 2.0,
                                  side, ink);
      continue;
    }
    auto *text = overlay_text_layout(cairo, face.glyph, layout.glyph_size);
    const auto size = overlay_text_size(text);
    mode_badge_set_rgb(cairo, ink);
    cairo_move_to(cairo, cell.x + (cell.width - size.width) / 2.0, cell.y + (cell.height - size.height) / 2.0);
    pango_cairo_show_layout(cairo, text);
    // show_layout 不清掉 move_to 留下的当前点，接着画的路径会从这里拉出一条线。
    cairo_new_path(cairo);
    g_object_unref(text);
  }
}

// 弹出菜单的几何，逻辑像素：14 的字，一行高 28，分隔线一行 9，左边留 28 给勾，右边留 14，上下各 6 的内边距，圆角 8。乘工具栏的 scale_percent，与工具栏一起放大。
struct FloatingToolbarMenuLayout {
  double width = 0;
  double height = 0;
  double margin = 0;
  FloatingToolbarRect plate;
  std::vector<FloatingToolbarRect> rows;
  double font_size = 0;
  double check_column = 0;
  double radius = 0;
  double scale = 1;
};

inline FloatingToolbarMenuLayout floating_toolbar_menu_layout(const std::vector<FloatingToolbarMenuItem> &items,
                                                              double scale, bool shadow) {
  FloatingToolbarMenuLayout layout;
  layout.scale = scale;
  layout.margin = shadow ? 8.0 : 0.0;
  layout.font_size = 14.0 * scale;
  layout.check_column = 28.0 * scale;
  layout.radius = 8.0 * scale;
  const double padding = 6.0 * scale, row = 28.0 * scale, separator = 9.0 * scale, trailing = 14.0 * scale;
  double text_width = 0;
  for (const auto &item : items)
    if (!item.separator) text_width = std::max(text_width, overlay_measure_text(item.label, layout.font_size).width);
  const double plate_width = std::ceil(layout.check_column + text_width + trailing);
  double y = layout.margin + padding;
  for (const auto &item : items) {
    const double height = item.separator ? separator : row;
    layout.rows.push_back({layout.margin + padding / 2.0, y, plate_width - padding, height});
    y += height;
  }
  const double plate_height = std::ceil(y - layout.margin + padding);
  layout.plate = {layout.margin, layout.margin, plate_width, plate_height};
  layout.width = plate_width + 2.0 * layout.margin;
  layout.height = plate_height + 2.0 * layout.margin;
  return layout;
}

// 指针下的那一项，分隔线和不能用的项不算。
inline std::optional<std::size_t> floating_toolbar_menu_hit(const FloatingToolbarMenuLayout &layout,
                                                            const std::vector<FloatingToolbarMenuItem> &items, double x,
                                                            double y) {
  for (std::size_t index = 0; index < layout.rows.size() && index < items.size(); ++index)
    if (!items[index].separator && !items[index].disabled && layout.rows[index].contains(x, y)) return index;
  return std::nullopt;
}

inline void paint_floating_toolbar_menu(cairo_t *cairo, const FloatingToolbarMenuLayout &layout,
                                        const std::vector<FloatingToolbarMenuItem> &items,
                                        std::optional<std::size_t> hovered, const FloatingToolbarColors &colors) {
  const auto &plate = layout.plate;
  if (layout.margin > 0)
    overlay_paint_shadow(cairo, plate.x, plate.y, plate.width, plate.height, layout.radius,
                         static_cast<int>(layout.margin));
  overlay_paint_card(cairo, plate.x, plate.y, plate.width, plate.height, layout.radius, colors.surface, colors.border);
  for (std::size_t index = 0; index < layout.rows.size() && index < items.size(); ++index) {
    const auto &row = layout.rows[index];
    const auto &item = items[index];
    if (item.separator) {
      mode_badge_set_rgb(cairo, colors.divider);
      cairo_rectangle(cairo, row.x + 4.0 * layout.scale, row.y + row.height / 2.0 - 0.5, row.width - 8.0 * layout.scale,
                      1.0);
      cairo_fill(cairo);
      continue;
    }
    if (hovered == index) {
      mode_badge_rounded_rect(cairo, row.x, row.y, row.width, row.height, 5.0 * layout.scale);
      mode_badge_set_rgb(cairo, colors.hover);
      cairo_fill(cairo);
    }
    auto *text = overlay_text_layout(cairo, item.label, layout.font_size);
    const auto size = overlay_text_size(text);
    mode_badge_set_rgb(cairo, item.disabled ? floating_toolbar_disabled_ink(colors) : colors.text);
    cairo_move_to(cairo, plate.x + layout.check_column, row.y + (row.height - size.height) / 2.0);
    pango_cairo_show_layout(cairo, text);
    cairo_new_path(cairo);
    g_object_unref(text);
    if (item.checked) {
      // 勾：画在左边那一栏的中间，颜色是选中项的强调色，与候选窗的选中项相同。
      const double side = 10.0 * layout.scale;
      const double cx = plate.x + layout.check_column / 2.0, cy = row.y + row.height / 2.0;
      cairo_move_to(cairo, cx - side / 2.0, cy);
      cairo_line_to(cairo, cx - side / 8.0, cy + side * 0.38);
      cairo_line_to(cairo, cx + side / 2.0, cy - side * 0.42);
      cairo_set_line_width(cairo, 1.8 * layout.scale);
      cairo_set_line_cap(cairo, CAIRO_LINE_CAP_ROUND);
      cairo_set_line_join(cairo, CAIRO_LINE_JOIN_ROUND);
      mode_badge_set_rgb(cairo, colors.accent);
      cairo_stroke(cairo);
    }
  }
}

// 按钮提示：一行 12 的字，左右 8、上下 4 的内边距，圆角 6，配色与工具栏相同。
struct FloatingToolbarTooltipLayout {
  double width = 0;
  double height = 0;
  double margin = 0;
  FloatingToolbarRect plate;
  double font_size = 0;
};

inline FloatingToolbarTooltipLayout floating_toolbar_tooltip_layout(const std::string &text, double scale, bool shadow) {
  FloatingToolbarTooltipLayout layout;
  layout.margin = shadow ? 6.0 : 0.0;
  layout.font_size = 12.0 * scale;
  const auto size = overlay_measure_text(text, layout.font_size);
  const double plate_width = std::ceil(size.width + 16.0 * scale), plate_height = std::ceil(size.height + 8.0 * scale);
  layout.plate = {layout.margin, layout.margin, plate_width, plate_height};
  layout.width = plate_width + 2.0 * layout.margin;
  layout.height = plate_height + 2.0 * layout.margin;
  return layout;
}

inline void paint_floating_toolbar_tooltip(cairo_t *cairo, const FloatingToolbarTooltipLayout &layout,
                                           const std::string &text, const FloatingToolbarColors &colors) {
  const auto &plate = layout.plate;
  if (layout.margin > 0)
    overlay_paint_shadow(cairo, plate.x, plate.y, plate.width, plate.height, 6.0, static_cast<int>(layout.margin));
  overlay_paint_card(cairo, plate.x, plate.y, plate.width, plate.height, 6.0, colors.surface,
                     colors.border.value_or(colors.divider));
  auto *pango = overlay_text_layout(cairo, text, layout.font_size);
  const auto size = overlay_text_size(pango);
  mode_badge_set_rgb(cairo, colors.text);
  cairo_move_to(cairo, plate.x + (plate.width - size.width) / 2.0, plate.y + (plate.height - size.height) / 2.0);
  pango_cairo_show_layout(cairo, pango);
  g_object_unref(pango);
}

}  // namespace msime::linux_host
