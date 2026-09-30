#pragma once
#include <cairo/cairo.h>
#include <pango/pangocairo.h>

#include <algorithm>
#include <cstdint>
#include <string>

#include "ModeBadgeStyle.h"

namespace msime::linux_host {

// Drawing the badge: a rounded plate, the product logo and one "中" or "英". It lives here so the Wayland and X11 backends share one drawing; they differ only in where the pixels go, and two drawings would drift apart between session types.

// The badge glyph's font at the given pixel size, semibold as the macOS badge draws it.
inline PangoLayout *mode_badge_text_layout(cairo_t *cairo, const std::string &text, double glyph_size) {
  auto *layout = pango_cairo_create_layout(cairo);
  auto *font = pango_font_description_from_string("Noto Sans CJK SC");
  pango_font_description_set_weight(font, PANGO_WEIGHT_SEMIBOLD);
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

// The layout for the given metrics, measured with the font the badge draws in: the wider of the two glyphs, so the badge keeps one width across modes, and the logo only when it loads.
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

// Paint the badge in layout's logical pixels (the caller scales the context to the device). The plate is the theme's opaque surface with its outline when the theme draws one, the glyph is its text colour, both as the floating toolbar derives them from the candidate palette.
inline void paint_mode_badge(cairo_t *cairo, const ModeBadgeLayout &layout, const std::string &text,
                             const std::string &icon_path, const FloatingSurfaceColors &colors) {
  cairo_set_operator(cairo, CAIRO_OPERATOR_SOURCE);
  cairo_set_source_rgba(cairo, 0, 0, 0, 0);
  cairo_paint(cairo);
  cairo_set_operator(cairo, CAIRO_OPERATOR_OVER);

  // The plate, inset by half the hairline so the outline stays inside the surface.
  const double w = layout.width, h = layout.height;
  const double inset = colors.border ? 0.5 : 0.0;
  const double radius = std::min(layout.radius, std::min(w, h) / 2.0 - inset);
  cairo_new_sub_path(cairo);
  cairo_arc(cairo, w - inset - radius, inset + radius, radius, -1.5708, 0);
  cairo_arc(cairo, w - inset - radius, h - inset - radius, radius, 0, 1.5708);
  cairo_arc(cairo, inset + radius, h - inset - radius, radius, 1.5708, 3.1416);
  cairo_arc(cairo, inset + radius, inset + radius, radius, 3.1416, 4.7124);
  cairo_close_path(cairo);
  mode_badge_set_rgb(cairo, colors.surface);
  if (colors.border) {
    cairo_fill_preserve(cairo);
    mode_badge_set_rgb(cairo, *colors.border);
    cairo_set_line_width(cairo, 1.0);
    cairo_stroke(cairo);
  } else {
    cairo_fill(cairo);
  }

  // The logo, fitted into its square and centred vertically. When it cannot be read only the glyph is drawn: half the hint beats none.
  double text_left = layout.inset;
  if (layout.logo) {
    if (auto *logo = mode_badge_logo(icon_path)) {
      const double source_w = cairo_image_surface_get_width(logo);
      const double source_h = cairo_image_surface_get_height(logo);
      const double scale = layout.logo_side / std::max(source_w, source_h);
      cairo_save(cairo);
      cairo_translate(cairo, layout.inset + (layout.logo_side - source_w * scale) / 2.0,
                      (h - source_h * scale) / 2.0);
      cairo_scale(cairo, scale, scale);
      cairo_set_source_surface(cairo, logo, 0, 0);
      cairo_pattern_set_filter(cairo_get_source(cairo), CAIRO_FILTER_BEST);
      cairo_paint(cairo);
      cairo_restore(cairo);
      cairo_surface_destroy(logo);
    }
    text_left += layout.logo_side + layout.spacing;
  }

  // The glyph, centred in the slot sized for the wider of the two.
  auto *text_layout = mode_badge_text_layout(cairo, text, layout.glyph_size);
  PangoRectangle logical{};
  pango_layout_get_extents(text_layout, nullptr, &logical);
  const double text_w = static_cast<double>(logical.width) / PANGO_SCALE;
  const double text_h = static_cast<double>(logical.height) / PANGO_SCALE;
  cairo_move_to(cairo, text_left + (layout.glyph_width - text_w) / 2.0, (h - text_h) / 2.0);
  mode_badge_set_rgb(cairo, colors.text);
  pango_cairo_show_layout(cairo, text_layout);
  g_object_unref(text_layout);
}

}  // namespace msime::linux_host
