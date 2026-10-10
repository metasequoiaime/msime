#pragma once
#include "ToolbarLayout.h"
#include <cmath>

namespace msime::windows {
// Win32 events and window sizes are pixels; Direct2D already applies system
// DPI to its DIPs. Only the user scale belongs in the drawing coordinates.
inline double toolbar_pixel_unit(unsigned dpi, double scale) {
  return static_cast<double>(dpi ? dpi : 96) / 96.0 * scale;
}
inline std::optional<size_t> toolbar_button_at_pixel(
    double x, double y, unsigned dpi, double scale, size_t buttons,
    const ToolbarMetrics &metrics) {
  const double unit = toolbar_pixel_unit(dpi, scale);
  if (!(unit > 0.0) || !std::isfinite(unit) || !std::isfinite(x) || !std::isfinite(y))
    return std::nullopt;
  y /= unit;
  if (y < metrics.shadow.top || y >= metrics.shadow.top + metrics.height)
    return std::nullopt;
  return toolbar_button_at(x / unit, buttons, metrics);
}
inline bool toolbar_drag_at_pixel(double x, double y, unsigned dpi, double scale,
                                  const ToolbarMetrics &metrics) {
  const double unit = toolbar_pixel_unit(dpi, scale);
  return unit > 0.0 && std::isfinite(unit) && std::isfinite(x) && std::isfinite(y) &&
         y / unit >= metrics.shadow.top &&
         y / unit < metrics.shadow.top + metrics.height &&
         toolbar_is_drag_strip(x / unit, metrics);
}
// 指针是否落在工具栏的卡片上（不含四周的阴影边距）。右键菜单只在卡片上打开，阴影属于它后面的窗口。
inline bool toolbar_card_at_pixel(double x, double y, unsigned dpi, double scale,
                                  size_t buttons, const ToolbarMetrics &metrics) {
  const double unit = toolbar_pixel_unit(dpi, scale);
  if (!(unit > 0.0) || !std::isfinite(unit) || !std::isfinite(x) || !std::isfinite(y))
    return false;
  const auto card = toolbar_card(buttons, metrics);
  x /= unit;
  y /= unit;
  return x >= card.left && x < card.right && y >= card.top && y < card.bottom;
}
// 工具栏窗口不感知 DPI，它的窗口过程里量到的屏幕坐标是系统虚拟化过的逻辑坐标；弹出菜单是每显示器感知的窗口，要物理像素。同一个窗口的外框在两种坐标里只差一次线性缩放，所以按坐标在外框里的位置换算：`logical` 落在逻辑外框 [logical_start, logical_end) 里的哪里，就换到物理外框 [physical_start, physical_end) 里的同一处。外框宽度为 0（窗口还没摆好）时只平移。
inline int toolbar_physical_coordinate(int logical, int logical_start, int logical_end,
                                       int physical_start, int physical_end) {
  const int logical_extent = logical_end - logical_start;
  if (logical_extent <= 0)
    return physical_start + (logical - logical_start);
  const double ratio = static_cast<double>(physical_end - physical_start) /
                       static_cast<double>(logical_extent);
  return physical_start +
         static_cast<int>(std::lround(static_cast<double>(logical - logical_start) * ratio));
}
} // namespace msime::windows
