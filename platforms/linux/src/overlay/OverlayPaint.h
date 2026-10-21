#pragma once

#include <cairo/cairo.h>
#include <pango/pangocairo.h>

#include <algorithm>
#include <cstdint>
#include <optional>
#include <string>

#include "ModeBadgePainter.h"

namespace msime::linux_host {

// 键位图、悬浮工具栏和它的菜单、提示共用的几样画法。圆角矩形和取色沿用切换徽章的（ModeBadgePainter.h），这样几块自绘浮窗的圆角和阴影是同一种画法。

// 界面文字的字体：首选 Noto Sans CJK SC，没装时依次退到思源黑体和 fontconfig 的默认无衬线字体，与切换徽章相同。
inline constexpr const char *kOverlayFontFamily = "Noto Sans CJK SC,Source Han Sans SC,sans-serif";
// 键帽上的键名用等宽字，对应 macOS 的 monospacedSystemFont。
inline constexpr const char *kOverlayMonospaceFamily = "DejaVu Sans Mono,Noto Sans Mono,monospace";

// size 是逻辑像素的绝对字号。调用方 g_object_unref 返回的布局。
inline PangoLayout *overlay_text_layout(cairo_t *cairo, const std::string &text, double size,
                                        PangoWeight weight = PANGO_WEIGHT_NORMAL,
                                        const char *family = kOverlayFontFamily) {
  auto *layout = pango_cairo_create_layout(cairo);
  auto *font = pango_font_description_new();
  pango_font_description_set_family(font, family);
  pango_font_description_set_weight(font, weight);
  pango_font_description_set_absolute_size(font, size * PANGO_SCALE);
  pango_layout_set_font_description(layout, font);
  pango_font_description_free(font);
  pango_layout_set_text(layout, text.c_str(), -1);
  return layout;
}

struct OverlayTextSize {
  double width = 0;
  double height = 0;
};

inline OverlayTextSize overlay_text_size(PangoLayout *layout) {
  PangoRectangle logical{};
  pango_layout_get_extents(layout, nullptr, &logical);
  return {static_cast<double>(logical.width) / PANGO_SCALE, static_cast<double>(logical.height) / PANGO_SCALE};
}

// 不画任何东西，只量一段字：测量用一块 1×1 的图像表面，字体选择与真正画时相同。
inline OverlayTextSize overlay_measure_text(const std::string &text, double size, PangoWeight weight = PANGO_WEIGHT_NORMAL,
                                            const char *family = kOverlayFontFamily) {
  auto *surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, 1, 1);
  auto *cairo = cairo_create(surface);
  auto *layout = overlay_text_layout(cairo, text, size, weight, family);
  const auto measured = overlay_text_size(layout);
  g_object_unref(layout);
  cairo_destroy(cairo);
  cairo_surface_destroy(surface);
  return measured;
}

inline void overlay_set_rgba(cairo_t *cairo, std::uint32_t rgb, double alpha) {
  cairo_set_source_rgba(cairo, ((rgb >> 16) & 0xff) / 255.0, ((rgb >> 8) & 0xff) / 255.0, (rgb & 0xff) / 255.0, alpha);
}

// 本体下方一圈向下偏 2、逐层变淡的阴影，近似 macOS 浮窗的系统阴影；layers 是留出的透明边宽度（逻辑像素）。与切换徽章的阴影同一种画法。
inline void overlay_paint_shadow(cairo_t *cairo, double left, double top, double width, double height, double radius,
                                 int layers) {
  for (int spread = layers; spread >= 1; --spread) {
    mode_badge_rounded_rect(cairo, left - spread, top - spread + 2.0, width + 2.0 * spread, height + 2.0 * spread,
                            radius + spread);
    cairo_set_source_rgba(cairo, 0, 0, 0, 0.018);
    cairo_fill(cairo);
  }
}

// 卡片：圆角矩形填底色，主题有描边时向内缩半条线宽描 1 像素的边，线条留在卡片以内。
inline void overlay_paint_card(cairo_t *cairo, double left, double top, double width, double height, double radius,
                               std::uint32_t surface, const std::optional<std::uint32_t> &border) {
  const double inset = border ? 0.5 : 0.0;
  mode_badge_rounded_rect(cairo, left + inset, top + inset, width - 2.0 * inset, height - 2.0 * inset, radius - inset);
  mode_badge_set_rgb(cairo, surface);
  if (border) {
    cairo_fill_preserve(cairo);
    mode_badge_set_rgb(cairo, *border);
    cairo_set_line_width(cairo, 1.0);
    cairo_stroke(cairo);
  } else {
    cairo_fill(cairo);
  }
}

}  // namespace msime::linux_host
