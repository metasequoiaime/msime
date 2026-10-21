#pragma once
#include <cairo/cairo.h>
#include <pango/pangocairo.h>

#include <algorithm>
#include <cstdint>
#include <string>

#include "ModeBadgeStyle.h"

namespace msime::linux_host {

// Drawing the badge: a rounded plate, the product logo and one "中" or "英". It lives here so the Wayland and X11 backends share one drawing; they differ only in where the pixels go, and two drawings would drift apart between session types.

// 徽章字的字体：常规字重，与 macOS 提示的 NSFontWeightRegular 相同。首选 Noto Sans CJK SC，没装时依次退到思源黑体和 fontconfig 的默认无衬线字体。
inline PangoLayout *mode_badge_text_layout(cairo_t *cairo, const std::string &text, double glyph_size) {
  auto *layout = pango_cairo_create_layout(cairo);
  auto *font = pango_font_description_new();
  pango_font_description_set_family(font, "Noto Sans CJK SC,Source Han Sans SC,sans-serif");
  pango_font_description_set_weight(font, PANGO_WEIGHT_NORMAL);
  pango_font_description_set_absolute_size(font, glyph_size * PANGO_SCALE);
  pango_layout_set_font_description(layout, font);
  pango_font_description_free(font);
  pango_layout_set_text(layout, text.c_str(), -1);
  return layout;
}

// The logo when it loads, otherwise null; the caller destroys it.
inline cairo_surface_t *mode_badge_logo(const std::string &icon_path) {
  if (icon_path.empty()) return nullptr;
  auto *logo = cairo_image_surface_create_from_png(icon_path.c_str());
  if (cairo_surface_status(logo) == CAIRO_STATUS_SUCCESS && cairo_image_surface_get_width(logo) > 0 &&
      cairo_image_surface_get_height(logo) > 0)
    return logo;
  cairo_surface_destroy(logo);
  return nullptr;
}

// 按给定缩放排好的徽章，用徽章实际画字的字体测量：取两个字里较宽的那个，切换模式时宽度不变；标志只在读得到时才算进去。
inline ModeBadgeLayout measure_mode_badge(const ModeBadgeMetrics &metrics, const std::string &icon_path) {
  auto *surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, 1, 1);
  auto *cairo = cairo_create(surface);
  const auto glyph_size = mode_badge_glyph_size(metrics);
  double glyph_width = 0;
  for (const char *text : {"中", "英"}) {
    auto *layout = mode_badge_text_layout(cairo, text, glyph_size);
    PangoRectangle logical{};
    pango_layout_get_extents(layout, nullptr, &logical);
    glyph_width = std::max(glyph_width, static_cast<double>(logical.width) / PANGO_SCALE);
    g_object_unref(layout);
  }
  cairo_destroy(cairo);
  cairo_surface_destroy(surface);
  auto *logo = mode_badge_logo(icon_path);
  const bool has_logo = logo != nullptr;
  if (logo) cairo_surface_destroy(logo);
  return mode_badge_layout(metrics, glyph_width, has_logo);
}

inline void mode_badge_set_rgb(cairo_t *cairo, std::uint32_t rgb) {
  cairo_set_source_rgb(cairo, ((rgb >> 16) & 0xff) / 255.0, ((rgb >> 8) & 0xff) / 255.0, (rgb & 0xff) / 255.0);
}

// 一个圆角矩形路径，left/top 为左上角。
inline void mode_badge_rounded_rect(cairo_t *cairo, double left, double top, double w, double h, double radius) {
  radius = std::max(0.0, std::min(radius, std::min(w, h) / 2.0));
  cairo_new_sub_path(cairo);
  cairo_arc(cairo, left + w - radius, top + radius, radius, -1.5708, 0);
  cairo_arc(cairo, left + w - radius, top + h - radius, radius, 0, 1.5708);
  cairo_arc(cairo, left + radius, top + h - radius, radius, 1.5708, 3.1416);
  cairo_arc(cairo, left + radius, top + radius, radius, 3.1416, 4.7124);
  cairo_close_path(cairo);
}

// 在 layout 的逻辑像素里画徽章（调用方把上下文缩放到设备像素）。本体是主题的不透明底色，主题有描边时加 1 px 描边，字用主题的文字色，取法与悬浮工具栏从候选调色板推导的一致。layout 留了阴影边时，先在本体下方画一圈向下偏 2 px、逐层变淡的阴影，近似 macOS 提示窗的系统阴影。alpha 小于 1 时整块（阴影、本体、标志和字）一起变淡，用于淡出。
inline void paint_mode_badge(cairo_t *cairo, const ModeBadgeLayout &layout, const std::string &text,
                             const std::string &icon_path, const FloatingSurfaceColors &colors, double alpha = 1.0) {
  cairo_set_operator(cairo, CAIRO_OPERATOR_SOURCE);
  cairo_set_source_rgba(cairo, 0, 0, 0, 0);
  cairo_paint(cairo);
  cairo_set_operator(cairo, CAIRO_OPERATOR_OVER);
  alpha = std::clamp(alpha, 0.0, 1.0);
  if (alpha <= 0.0) return;
  if (alpha < 1.0) cairo_push_group(cairo);

  const double origin = layout.margin;
  const double w = layout.plate_width, h = layout.plate_height;
  if (layout.margin > 0) {
    const int layers = static_cast<int>(layout.margin);
    for (int spread = layers; spread >= 1; --spread) {
      mode_badge_rounded_rect(cairo, origin - spread, origin - spread + 2.0, w + 2.0 * spread, h + 2.0 * spread,
                              layout.radius + spread);
      cairo_set_source_rgba(cairo, 0, 0, 0, 0.022);
      cairo_fill(cairo);
    }
  }

  // 本体，描边时向内缩半条线宽，线条留在本体以内。
  const double inset = colors.border ? ModeBadgeGeometry::border_width / 2.0 : 0.0;
  mode_badge_rounded_rect(cairo, origin + inset, origin + inset, w - 2.0 * inset, h - 2.0 * inset,
                          layout.radius - inset);
  mode_badge_set_rgb(cairo, colors.surface);
  if (colors.border) {
    cairo_fill_preserve(cairo);
    mode_badge_set_rgb(cairo, *colors.border);
    cairo_set_line_width(cairo, ModeBadgeGeometry::border_width);
    cairo_stroke(cairo);
  } else {
    cairo_fill(cairo);
  }

  // 标志按比例放进它的方格并垂直居中。读不到时只画字：提示少一半也好过没有。
  double text_left = origin + layout.inset;
  if (layout.logo) {
    if (auto *logo = mode_badge_logo(icon_path)) {
      const double source_w = cairo_image_surface_get_width(logo);
      const double source_h = cairo_image_surface_get_height(logo);
      const double scale = layout.logo_side / std::max(source_w, source_h);
      cairo_save(cairo);
      cairo_translate(cairo, text_left + (layout.logo_side - source_w * scale) / 2.0,
                      origin + (h - source_h * scale) / 2.0);
      cairo_scale(cairo, scale, scale);
      cairo_set_source_surface(cairo, logo, 0, 0);
      cairo_pattern_set_filter(cairo_get_source(cairo), CAIRO_FILTER_BEST);
      cairo_paint(cairo);
      cairo_restore(cairo);
      cairo_surface_destroy(logo);
    }
    text_left += layout.logo_side + layout.spacing;
  }

  // 字居中在按较宽那个字留出的位置里。
  auto *text_layout = mode_badge_text_layout(cairo, text, layout.glyph_size);
  PangoRectangle logical{};
  pango_layout_get_extents(text_layout, nullptr, &logical);
  const double text_w = static_cast<double>(logical.width) / PANGO_SCALE;
  const double text_h = static_cast<double>(logical.height) / PANGO_SCALE;
  cairo_move_to(cairo, text_left + (layout.glyph_width - text_w) / 2.0, origin + (h - text_h) / 2.0);
  mode_badge_set_rgb(cairo, colors.text);
  pango_cairo_show_layout(cairo, text_layout);
  g_object_unref(text_layout);

  if (alpha < 1.0) {
    cairo_pop_group_to_source(cairo);
    cairo_paint_with_alpha(cairo, alpha);
  }
}

}  // namespace msime::linux_host
