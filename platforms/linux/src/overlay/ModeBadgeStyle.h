#pragma once
#include <algorithm>
#include <cmath>

#include "../candidates/CandidatePalette.h"

namespace msime::linux_host {

// The badge has a fixed size of its own and does not read the shared floating_toolbar.font_size or scale_percent: Linux draws no floating toolbar and its settings page offers no toolbar size, so values imported from another platform must not resize the badge, and macOS stopped sizing its input-mode HUD from the toolbar too (2026-10-14-macos-compact-input-mode-hud.md). The geometry is the one the badge always had at the toolbar's default 24 px and 100 %: a (font + 20) x scale height, a 0.95 x font glyph, the 22 px full-colour brand mark leading it with 6 px between them, a 12 px inset on both sides and a 10 px corner radius, all multiplied by the scale; the width is fitted to that content. All lengths are logical pixels; the X11 backend multiplies them by the desktop scale.
struct ModeBadgeGeometry {
  static constexpr double default_font_size = 24.0;
  static constexpr double glyph_scale = 0.95;
  static constexpr double height_padding = 20.0;
  static constexpr double logo_side = 22.0;
  static constexpr double corner_radius = 10.0;
  static constexpr double horizontal_inset = 12.0;
  static constexpr double content_spacing = 6.0;
};

struct ModeBadgeMetrics {
  double font_size = ModeBadgeGeometry::default_font_size;
  double scale = 1.0;
};

// The badge laid out for one glyph width: the plate's whole-pixel size (rounded outward so nothing is clipped, as macOS rounds its window) and where the logo and the glyph go inside it.
struct ModeBadgeLayout {
  int width = 0;
  int height = 0;
  double glyph_size = 0;
  double glyph_width = 0;
  double logo_side = 0;
  double spacing = 0;
  double inset = 0;
  double radius = 0;
  bool logo = false;
};

// glyph_width is the wider of the two characters (中 and 英) at glyph_size, so switching modes never changes the badge's width. Without a logo the glyph stands alone between the insets, as the macOS badge drops the mark it cannot load.
inline double mode_badge_glyph_size(const ModeBadgeMetrics &metrics) {
  return metrics.font_size * metrics.scale * ModeBadgeGeometry::glyph_scale;
}

inline ModeBadgeLayout mode_badge_layout(const ModeBadgeMetrics &metrics, double glyph_width, bool logo) {
  using G = ModeBadgeGeometry;
  ModeBadgeLayout layout;
  layout.glyph_size = mode_badge_glyph_size(metrics);
  layout.glyph_width = std::max(0.0, glyph_width);
  layout.logo = logo;
  layout.logo_side = logo ? G::logo_side * metrics.scale : 0.0;
  layout.spacing = logo ? G::content_spacing * metrics.scale : 0.0;
  layout.inset = G::horizontal_inset * metrics.scale;
  layout.radius = G::corner_radius * metrics.scale;
  layout.width = static_cast<int>(std::ceil(2.0 * layout.inset + layout.logo_side + layout.spacing + layout.glyph_width));
  layout.height = static_cast<int>(std::ceil((metrics.font_size + G::height_padding) * metrics.scale));
  return layout;
}

// What one show() draws: the badge's fixed size and the colours from the resolved theme.
struct ModeBadgeStyle {
  ModeBadgeMetrics metrics;
  FloatingSurfaceColors colors;
};

}  // namespace msime::linux_host
