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
  // The glyph size everything else is derived from. Kept here rather than read
  // from the setting at each call site: an out-of-range setting falls back to
  // the shipped geometry, and text drawn from the raw setting would then be
  // sized for a bar that was never built.
  double icon = 24.0;
  double cell = 48.0;
  // The bar itself, excluding the shadow margin.
  double height = 52.0;
  // The product mark at the far left, before the drag strip. It is decoration,
  // not a button: it says which IME the bar belongs to, and it is the widest
  // part of the drag target.
  double logo = 36.0;
  // The drag strip on the left; the only part of the window that drags.
  double handle = 8.0;
  double icon_top = 8.0;
  double icon_bottom = 44.0;
  ToolbarShadow shadow;
};

// Derived from the configured icon size.
//
// The pitch was three times the icon: a 24 DIP glyph centred in a 72 DIP cell,
// which is 24 DIP of empty bar between one icon and the next. At that spacing
// the buttons read as scattered marks rather than a row of controls. Two icon
// widths puts 12 DIP either side of the glyph - still a comfortable click
// target, and the hover pill it draws is very close to square.
//
// The height is deliberately left alone. It is what the icon sits in
// vertically and what the bar's rounded corners are cut from; shrinking it
// with the pitch would have made the bar thinner as well as shorter, which is
// not what was too loose.
inline ToolbarMetrics toolbar_metrics(double font_size, bool shadow = true) {
  ToolbarMetrics metrics;
  metrics.shadow = toolbar_shadow(shadow);
  if (!(font_size >= 8.0) || !(font_size <= 64.0))
    return metrics; // Out of range: keep the shipped geometry.
  metrics.icon = font_size;
  metrics.cell = font_size * 2.0;
  metrics.height = font_size * 2.0 + 4.0;
  // The mark is drawn at the icon size, in a slot half again as wide, so it
  // has a gutter either side and is not jammed into the bar's rounded corner.
  metrics.logo = font_size * 1.5;
  metrics.icon_top = metrics.height / 6.5;
  metrics.icon_bottom = metrics.height - metrics.icon_top;
  return metrics;
}

// 偏好 `show_app_logo` 关掉时 logo 槽收窄成一条握把：画两列三行小圆点，仍是拖动区的一部分。宽度与 macOS 的 kToolbarGripWidth 同比例（图标 24 时 10），再小也留 8，免得握把细到按不准。
inline ToolbarMetrics toolbar_metrics(double font_size, bool shadow, bool logo) {
  auto metrics = toolbar_metrics(font_size, shadow);
  if (!logo)
    metrics.logo = (std::max)(8.0, metrics.icon * 10.0 / 24.0);
  return metrics;
}

// The bar, without the shadow margin around it.
inline double toolbar_content_width(size_t buttons,
                                    const ToolbarMetrics &metrics) {
  return metrics.logo + metrics.handle * 2.0 +
         metrics.cell * static_cast<double>(buttons);
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

// The bar's rectangle in window coordinates: what gets filled, and what the
// shadow is cast from.
struct ToolbarRect {
  double left, top, right, bottom;
};
inline ToolbarRect toolbar_card(size_t buttons, const ToolbarMetrics &metrics) {
  return {metrics.shadow.left, metrics.shadow.top,
          metrics.shadow.left + toolbar_content_width(buttons, metrics),
          metrics.shadow.top + metrics.height};
}

// Where the product mark is drawn: a square the size of an icon, centred in
// the logo slot at the far left of the bar. Square rather than the slot itself
// because the mark is an app icon, and stretching one to fill a wider box is
// the one thing that makes it look wrong at a glance.
inline ToolbarRect toolbar_logo(const ToolbarMetrics &metrics) {
  const double side =
      std::min({metrics.icon, metrics.logo, metrics.icon_bottom - metrics.icon_top});
  const double middle_x = metrics.shadow.left + metrics.logo / 2.0;
  const double middle_y = metrics.shadow.top + metrics.height / 2.0;
  return {middle_x - side / 2.0, middle_y - side / 2.0, middle_x + side / 2.0,
          middle_y + side / 2.0};
}

// Which button sits under `x` in window coordinates, or nothing for the shadow
// margin, the logo, the drag strip, and the margin past the last button. One
// implementation so drawing, hover and click cannot disagree.
inline std::optional<size_t> toolbar_button_at(double x, size_t buttons,
                                               const ToolbarMetrics &metrics) {
  const double first = metrics.shadow.left + metrics.logo + metrics.handle;
  if (buttons == 0 || metrics.cell <= 0.0 || x < first)
    return std::nullopt;
  const auto index = static_cast<size_t>((x - first) / metrics.cell);
  if (index >= buttons)
    return std::nullopt;
  return index;
}

// The cell a button occupies, in window coordinates, as drawing uses it.
inline ToolbarRect toolbar_cell(size_t index, const ToolbarMetrics &metrics) {
  const double left = metrics.shadow.left + metrics.logo + metrics.handle +
                      metrics.cell * static_cast<double>(index);
  return {left, metrics.shadow.top + metrics.icon_top, left + metrics.cell,
          metrics.shadow.top + metrics.icon_bottom};
}

// The drag strip, which is everything left of the first button but inside the
// bar - not the shadow margin, which belongs to whatever is behind it. The
// logo is part of it: it is decoration with nothing to click, so making it
// drag is free, and it is the obvious thing to grab the bar by.
inline bool toolbar_is_drag_strip(double x, const ToolbarMetrics &metrics) {
  return x >= metrics.shadow.left &&
         x < metrics.shadow.left + metrics.logo + metrics.handle;
}
} // namespace msime::windows
