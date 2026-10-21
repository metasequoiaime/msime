#pragma once

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <vector>

namespace msime::linux_host {

struct WaveOverlayWorkArea {
  int x = 0;
  int y = 0;
  int width = 0;
  int height = 0;
};

struct WaveOverlayPosition {
  int x = 0;
  int y = 0;
};

// One output as the X server reports it: `full` is the monitor rectangle, `work` the part of it not covered by panels.
struct WaveOverlayMonitor {
  WaveOverlayWorkArea full;
  WaveOverlayWorkArea work;
};

// 离工作区底边的间距，与 macOS 和 Windows 语音条一样是 10 px（`WaveOverlayLayout.h` 里的 `kWaveOverlayWorkAreaGap`）。
constexpr int kWaveOverlayBottomMargin = 10;

constexpr WaveOverlayPosition
wave_overlay_bottom_center(WaveOverlayWorkArea work_area, int overlay_width,
                           int overlay_height,
                           int bottom_margin = kWaveOverlayBottomMargin) {
  const auto width = std::max(0, overlay_width);
  const auto height = std::max(0, overlay_height);
  const auto margin = std::max(0, bottom_margin);
  const auto x = work_area.width > width
                     ? work_area.x + (work_area.width - width) / 2
                     : work_area.x;
  const auto y = work_area.height >= height + margin
                     ? work_area.y + work_area.height - height - margin
                     : work_area.y;
  return {x, y};
}

// Same rule as the Windows voice bar: centred horizontally on the whole monitor, so a side panel does not push it off centre, and `bottom_margin` above the bottom of the work area, so it clears a bottom panel.
constexpr WaveOverlayPosition
wave_overlay_monitor_bottom_center(const WaveOverlayMonitor &monitor,
                                   int overlay_width, int overlay_height,
                                   int bottom_margin) {
  const auto horizontal = wave_overlay_bottom_center(
      monitor.full, overlay_width, overlay_height, bottom_margin);
  const auto vertical = wave_overlay_bottom_center(
      monitor.work, overlay_width, overlay_height, bottom_margin);
  return {horizontal.x, vertical.y};
}

// The mode badge's corner: `margin` in from the right and bottom edges of the work area, the same spot the Wayland badge's layer-shell anchor and margins give it. An area too small for the overlay plus its margin pins that axis to the area's top-left edge instead of pushing the overlay off the monitor.
constexpr WaveOverlayPosition
wave_overlay_bottom_right(WaveOverlayWorkArea work_area, int overlay_width,
                          int overlay_height, int margin) {
  const auto width = std::max(0, overlay_width);
  const auto height = std::max(0, overlay_height);
  const auto inset = std::max(0, margin);
  const auto x = work_area.width >= width + inset
                     ? work_area.x + work_area.width - width - inset
                     : work_area.x;
  const auto y = work_area.height >= height + inset
                     ? work_area.y + work_area.height - height - inset
                     : work_area.y;
  return {x, y};
}

constexpr bool wave_overlay_contains(WaveOverlayWorkArea area,
                                     WaveOverlayPosition point) {
  return area.width > 0 && area.height > 0 && point.x >= area.x &&
         point.y >= area.y &&
         static_cast<std::int64_t>(point.x) <
             static_cast<std::int64_t>(area.x) + area.width &&
         static_cast<std::int64_t>(point.y) <
             static_cast<std::int64_t>(area.y) + area.height;
}

// The monitor holding the focused window's centre, else the one under the pointer, else the primary, else the first. Empty only when there are no monitors.
inline std::optional<std::size_t>
wave_overlay_pick_monitor(const std::vector<WaveOverlayMonitor> &monitors,
                          std::optional<WaveOverlayPosition> focus_center,
                          std::optional<WaveOverlayPosition> pointer,
                          std::optional<std::size_t> primary) {
  if (monitors.empty())
    return std::nullopt;
  for (const auto &point : {focus_center, pointer}) {
    if (!point)
      continue;
    for (std::size_t index = 0; index < monitors.size(); ++index)
      if (wave_overlay_contains(monitors[index].full, *point))
        return index;
  }
  if (primary && *primary < monitors.size())
    return primary;
  return std::size_t{0};
}

constexpr std::optional<WaveOverlayWorkArea>
wave_overlay_intersection(WaveOverlayWorkArea first,
                          WaveOverlayWorkArea second) {
  const auto left = std::max<std::int64_t>(first.x, second.x);
  const auto top = std::max<std::int64_t>(first.y, second.y);
  const auto right =
      std::min<std::int64_t>(static_cast<std::int64_t>(first.x) + first.width,
                             static_cast<std::int64_t>(second.x) + second.width);
  const auto bottom = std::min<std::int64_t>(
      static_cast<std::int64_t>(first.y) + first.height,
      static_cast<std::int64_t>(second.y) + second.height);
  if (first.width <= 0 || first.height <= 0 || second.width <= 0 ||
      second.height <= 0 || right <= left || bottom <= top)
    return std::nullopt;
  return WaveOverlayWorkArea{static_cast<int>(left), static_cast<int>(top),
                             static_cast<int>(right - left),
                             static_cast<int>(bottom - top)};
}

// The work area of one monitor. `work_areas` is either the single `_NET_WORKAREA` rectangle of the current desktop, which spans every monitor, or Mutter's per-monitor `_GTK_WORKAREAS_D<n>` list; the largest overlap with the monitor wins. Without any overlap the whole monitor is used.
inline WaveOverlayWorkArea
wave_overlay_monitor_work(WaveOverlayWorkArea full,
                          const std::vector<WaveOverlayWorkArea> &work_areas) {
  std::optional<WaveOverlayWorkArea> best;
  for (const auto &area : work_areas) {
    const auto overlap = wave_overlay_intersection(full, area);
    if (overlap && (!best || static_cast<std::int64_t>(overlap->width) *
                                     overlap->height >
                                 static_cast<std::int64_t>(best->width) *
                                     best->height))
      best = overlap;
  }
  return best.value_or(full);
}

// GDK_SCALE wins when set, as it does for GTK itself; otherwise Xft.dpi relative to 96. Clamped to [1, 4] so a bogus resource cannot shrink the bar or make it cover the screen.
inline double wave_overlay_scale(std::optional<double> xft_dpi,
                                 std::optional<double> gdk_scale) {
  double scale = 1.0;
  if (gdk_scale && *gdk_scale > 0.0)
    scale = *gdk_scale;
  else if (xft_dpi && *xft_dpi > 0.0)
    scale = *xft_dpi / 96.0;
  if (!std::isfinite(scale))
    return 1.0;
  return std::clamp(scale, 1.0, 4.0);
}

// A logical length in device pixels, rounded so fractional scales such as 1.5 stay on the pixel grid.
inline int wave_overlay_scaled(int logical, double scale) {
  return static_cast<int>(std::lround(logical * scale));
}

} // namespace msime::linux_host
