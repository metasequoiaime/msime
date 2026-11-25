#pragma once

#include "../overlay/WaveOverlayPlacement.h"

namespace msime::linux_host {

// 右键菜单的定位锚点：纯 X11 会话用指针（XQueryPointer 取到的根窗口坐标）。GNOME Shell 会话的菜单由扩展画，不经过这里。
struct CandidateMenuAnchor {
  // Pointer：纯 X11 的 XQueryPointer 根窗口坐标。
  enum class Kind { Pointer };
  Kind kind = Kind::Pointer;
  int x = 0;
  int y = 0;
  int width = 0;
  int height = 0;
};

using CandidateMenuPosition = WaveOverlayPosition;

namespace candidate_menu_detail {
constexpr int clamp(int value, int minimum, int maximum) {
  if (maximum < minimum) return minimum;
  return value < minimum ? minimum : (value > maximum ? maximum : value);
}
}  // namespace candidate_menu_detail

// 菜单左上角位置。指针模式「下方放不下翻上方」：纯 X11 下候选框和菜单同是 X11 窗口，不存在合成器遮挡。
constexpr CandidateMenuPosition candidate_menu_position(CandidateMenuAnchor anchor, int menu_width,
                                                        int menu_height, int margin, int /*gap*/,
                                                        WaveOverlayWorkArea work) {
  using candidate_menu_detail::clamp;
  const int x = anchor.x - margin;
  int y = anchor.y - margin;
  if (y + menu_height - margin > work.y + work.height)
    y = anchor.y - (menu_height - margin);
  return {clamp(x, work.x - margin, work.x + work.width - menu_width + margin),
          clamp(y, work.y - margin, work.y + work.height - menu_height + margin)};
}

}  // namespace msime::linux_host
