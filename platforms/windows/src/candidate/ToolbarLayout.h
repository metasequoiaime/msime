#pragma once
#include <algorithm>
#include <cstddef>
#include <optional>

namespace msime::windows {
// The margin the drop shadow needs around the bar. A shadow drawn inside the
// window is clipped by it, so the window is grown by these and the bar is
// inset, exactly as upstream does with its frame padding.
struct ToolbarShadow {
  double left = 0.0;
  double top = 0.0;
  double right = 0.0;
  double bottom = 0.0;
  // Blur radius and offset multiplier; 0 draws nothing.
  double scale = 0.0;
};

inline ToolbarShadow toolbar_shadow(bool enabled) {
  if (!enabled)
    return {};
  // Upstream's frame padding and card shadow scale.
  return {18.0, 16.0, 18.0, 20.0, 0.45};
}

// Floating toolbar geometry, in device independent pixels.
//
// The cell pitch and bar height used to be literals repeated at five call
// sites - the window sizing, the drawing loop, the hover hit and the click
// hit. Two consequences. The icon size setting only changed the glyph
// while the cell it sat in stayed the same size, so a larger icon crowded its
// cell instead of enlarging the bar. And four copies of the same arithmetic is
// four chances for the highlight and the click to disagree about which button
// the pointer is over.
//
// The shadow margin goes through the same helper for the same reason: it
// offsets every drawn coordinate, so a hit test that forgot it would select
// the button to the left of the one lit up.
struct ToolbarMetrics {
  // The glyph size everything else is derived from. Kept here rather than read from the setting at each call site: an out-of-range setting falls back to the shipped geometry, and text drawn from the raw setting would then be sized for a bar that was never built.
  double icon = 24.0;
  // A button's square: the glyph plus 4 DIP a side, as macOS sizes its buttons (font + kToolbarButtonPadding, the same height). The hover and pressed fill covers all of it.
  double button = 32.0;
  // From one button to the next: the button plus the 2 DIP macOS leaves between buttons (kToolbarButtonSpacing).
  double cell = 34.0;
  // The bar itself, excluding the shadow margin: font + 20, macOS's kToolbarHeight at the shipped size.
  double height = 44.0;
  // The product mark at the far left. It is decoration, not a button: it says which IME the bar belongs to, and it is the obvious place to grab the bar. 34 DIP whatever the icon size, macOS's kToolbarLogoWidth; like the rest of the chrome it only follows the user's scale.
  double logo = 34.0;
  // The leading run after the logo slot, macOS's values: 4 DIP that still belong to the logo (kToolbarLogoGap), 3 more (kToolbarLogoDividerGap), the 1.2 DIP divider (kToolbarDividerWidth), and 4 before the first button (kToolbarDividerButtonGap). There is no accent drag bar: the logo, or the grip in its place, is the only drag cue, as on macOS.
  double logo_gap = 4.0;
  double divider_gap = 3.0;
  double divider = 1.2;
  double button_gap = 4.0;
  // Past the last button (kToolbarTrailingChrome).
  double trailing = 6.0;
  // The band the buttons occupy, centred in the bar.
  double icon_top = 6.0;
  double icon_bottom = 38.0;
  ToolbarShadow shadow;
};

// Derived from the configured icon size, with the macOS toolbar's geometry (FloatingToolbarPanel.mm applySizingPreferences:).
//
// A button is the glyph plus 8 DIP, square, and the next one starts 2 DIP after it, so the pitch is the glyph plus 10. The padding does not grow with the icon, as on macOS: larger glyphs are what make a larger bar, not wider gaps. The bar is the glyph plus 20, which leaves 6 DIP above and below the buttons. The chrome around the buttons - the logo slot (or the grip in its place), the gaps either side of the divider, the divider itself and the trailing inset - is fixed and only follows the user's scale, which the caller applies.
inline ToolbarMetrics toolbar_metrics(double font_size, bool shadow = true) {
  ToolbarMetrics metrics;
  metrics.shadow = toolbar_shadow(shadow);
  if (!(font_size >= 8.0) || !(font_size <= 64.0))
    return metrics; // Out of range: keep the shipped geometry.
  metrics.icon = font_size;
  metrics.button = font_size + 8.0;
  metrics.cell = metrics.button + 2.0;
  metrics.height = font_size + 20.0;
  metrics.icon_top = (metrics.height - metrics.button) / 2.0;
  metrics.icon_bottom = metrics.icon_top + metrics.button;
  return metrics;
}

// 偏好 `show_app_logo` 关掉时 logo 槽收窄成一条握把：画两列三行小圆点，仍是拖动区的一部分。宽度是 macOS 的 kToolbarGripWidth，固定 10，与 logo 槽一样不随图标大小变。
inline constexpr double toolbar_grip_width = 10.0;
inline ToolbarMetrics toolbar_metrics(double font_size, bool shadow, bool logo) {
  auto metrics = toolbar_metrics(font_size, shadow);
  if (!logo)
    metrics.logo = toolbar_grip_width;
  return metrics;
}

// Where the first button starts, in bar coordinates: the logo slot and the leading run after it.
inline double toolbar_leading_width(const ToolbarMetrics &metrics) {
  return metrics.logo + metrics.logo_gap + metrics.divider_gap +
         metrics.divider + metrics.button_gap;
}

// The bar, without the shadow margin around it. The last button has no gap after it, only the trailing inset, as macOS's stack leaves none past its last view.
inline double toolbar_content_width(size_t buttons,
                                    const ToolbarMetrics &metrics) {
  const double row = buttons == 0
                         ? 0.0
                         : metrics.cell * static_cast<double>(buttons) -
                               (metrics.cell - metrics.button);
  return toolbar_leading_width(metrics) + row + metrics.trailing;
}

// The window, which is the bar plus room for the shadow to fall outside it.
inline double toolbar_window_width(size_t buttons,
                                   const ToolbarMetrics &metrics) {
  return metrics.shadow.left + toolbar_content_width(buttons, metrics) +
         metrics.shadow.right;
}
inline double toolbar_window_height(const ToolbarMetrics &metrics) {
  return metrics.shadow.top + metrics.height + metrics.shadow.bottom;
}

// The bar's rectangle in window coordinates: what gets filled, and what the shadow is cast from.
struct ToolbarRect {
  double left, top, right, bottom;
};
inline ToolbarRect toolbar_card(size_t buttons, const ToolbarMetrics &metrics) {
  return {metrics.shadow.left, metrics.shadow.top,
          metrics.shadow.left + toolbar_content_width(buttons, metrics),
          metrics.shadow.top + metrics.height};
}

// Where the product mark is drawn: a 22 DIP square (macOS's kToolbarLogoMarkSide), centred in the logo slot and the gap that belongs to it, as macOS centres the mark in its logo view. Like macOS it leaves at least 2 DIP either side of the mark inside that view and the bar, so it shrinks only when the slot or the bar is smaller than the mark. Square rather than the slot itself because the mark is an app icon, and stretching one to fill a wider box is the one thing that makes it look wrong at a glance. The grip's dots are centred on the same point.
inline constexpr double toolbar_logo_mark = 22.0;
inline ToolbarRect toolbar_logo(const ToolbarMetrics &metrics) {
  const double side =
      (std::max)(0.0, (std::min)((std::min)(metrics.logo + metrics.logo_gap,
                                            metrics.height) -
                                     4.0,
                                 toolbar_logo_mark));
  const double middle_x =
      metrics.shadow.left + (metrics.logo + metrics.logo_gap) / 2.0;
  const double middle_y = metrics.shadow.top + metrics.height / 2.0;
  return {middle_x - side / 2.0, middle_y - side / 2.0, middle_x + side / 2.0,
          middle_y + side / 2.0};
}

// The hairline between the logo and the buttons: 1.2 DIP wide and as tall as a button, centred in the bar.
inline ToolbarRect toolbar_divider(const ToolbarMetrics &metrics) {
  const double left = metrics.shadow.left + metrics.logo + metrics.logo_gap +
                      metrics.divider_gap;
  return {left, metrics.shadow.top + metrics.icon_top, left + metrics.divider,
          metrics.shadow.top + metrics.icon_bottom};
}

// The hover and pressed fill's corner radius: a quarter of the button, never under 2 DIP, as macOS's MetasequoiaFloatingToolbarButton draws it (8 at the shipped size).
inline double toolbar_button_radius(const ToolbarMetrics &metrics) {
  return (std::max)(2.0, metrics.button * 0.25);
}

// Which button sits under `x` in window coordinates, or nothing for the shadow margin, the logo, the leading run, the 2 DIP between two buttons and the margin past the last button. One implementation so drawing, hover and click cannot disagree.
inline std::optional<size_t> toolbar_button_at(double x, size_t buttons,
                                               const ToolbarMetrics &metrics) {
  const double first = metrics.shadow.left + toolbar_leading_width(metrics);
  if (buttons == 0 || metrics.cell <= 0.0 || x < first)
    return std::nullopt;
  const auto index = static_cast<size_t>((x - first) / metrics.cell);
  if (index >= buttons)
    return std::nullopt;
  if (x - first - metrics.cell * static_cast<double>(index) >= metrics.button)
    return std::nullopt;
  return index;
}

// The square a button occupies, in window coordinates, as drawing uses it.
inline ToolbarRect toolbar_cell(size_t index, const ToolbarMetrics &metrics) {
  const double left = metrics.shadow.left + toolbar_leading_width(metrics) +
                      metrics.cell * static_cast<double>(index);
  return {left, metrics.shadow.top + metrics.icon_top, left + metrics.button,
          metrics.shadow.top + metrics.icon_bottom};
}

// The logo slot and the gap that belongs to it: the part of the bar that shows the move cursor, as macOS shows the open hand over its logo view. Dragging is not limited to it - every part of the bar that is not a button drags (toolbar_drag_at_pixel) - but this is where the bar says so. The shadow margin outside the bar belongs to whatever is behind it.
inline bool toolbar_is_logo_slot(double x, const ToolbarMetrics &metrics) {
  return x >= metrics.shadow.left &&
         x < metrics.shadow.left + metrics.logo + metrics.logo_gap;
}
} // namespace msime::windows
