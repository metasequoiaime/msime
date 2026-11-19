#pragma once

#include "../overlay/WaveOverlayPlacement.h"

namespace msime::linux_host {

// 右键菜单的定位锚点：指针模式给纯 X11 会话（坐标是根窗口坐标，由 XQueryPointer 取），
// 光标模式给 Wayland 会话（坐标来自 IBus 上报的光标矩形）。Wayland 的 Xwayland 根窗口
// 没有真实指针（child 为 0、坐标陈旧在屏幕角落），因此那条路不能用来定位菜单。
struct CandidateMenuAnchor {
  enum class Kind { Pointer, Caret };
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

// 光标矩形能不能当锚点：四个分量全 0 是客户端还没报过光标；矩形不完整落在工作区里，
// 说明坐标不是根窗口坐标（Wayland 原生客户端上报的是自己表面里的局部坐标）。这两种
// 都不能用，否则菜单会被摆到屏幕角落或错误位置。指针模式的坐标由 XQueryPointer 取，
// 只在纯 X11 会话里传进来，这里不重复校验。
constexpr bool candidate_menu_anchor_usable(const CandidateMenuAnchor &anchor, WaveOverlayWorkArea work) {
  if (anchor.kind != CandidateMenuAnchor::Kind::Caret) return true;
  if (anchor.x == 0 && anchor.y == 0 && anchor.width == 0 && anchor.height == 0) return false;
  return anchor.x >= work.x && anchor.y >= work.y &&
         anchor.x + anchor.width <= work.x + work.width &&
         anchor.y + anchor.height <= work.y + work.height;
}

// 菜单左上角位置。光标模式优先放在光标右方（候选窗通常在光标上/下方，侧方不挡候选），
// 右方放不下翻左方，再放不下放到下方（留出候选窗的估计高度），最后才翻上方；全程夹在
// 工作区内。锚点不可用时回到工作区水平居中、下沿离底边四分之一处，绝不落在桌面右上角。
// 指针模式保持原来的「下方放不下翻上方」。
constexpr CandidateMenuPosition candidate_menu_position(CandidateMenuAnchor anchor, int menu_width,
                                                        int menu_height, int margin, int gap,
                                                        int candidate_clearance,
                                                        WaveOverlayWorkArea work) {
  using candidate_menu_detail::clamp;
  const int min_x = work.x + margin;
  const int max_x = work.x + work.width - margin - menu_width;
  const int min_y = work.y + margin;
  const int max_y = work.y + work.height - margin - menu_height;
  if (!candidate_menu_anchor_usable(anchor, work)) {
    const int centered_x = work.x + (work.width - menu_width) / 2;
    const int lower_quarter_y = work.y + work.height - work.height / 4 - menu_height;
    return {clamp(centered_x, min_x, max_x), clamp(lower_quarter_y, min_y, max_y)};
  }
  if (anchor.kind == CandidateMenuAnchor::Kind::Pointer) {
    const int x = anchor.x - margin;
    int y = anchor.y - margin;
    if (y + menu_height - margin > work.y + work.height)
      y = anchor.y - (menu_height - margin);
    return {clamp(x, work.x - margin, work.x + work.width - menu_width + margin),
            clamp(y, work.y - margin, work.y + work.height - menu_height + margin)};
  }
  const int caret_right = anchor.x + anchor.width;
  const int caret_bottom = anchor.y + anchor.height;
  const int near_y = clamp(anchor.y - margin, min_y, max_y);
  const int right_x = caret_right + gap;
  if (right_x + menu_width <= max_x)
    return {clamp(right_x, min_x, max_x), near_y};
  const int left_x = anchor.x - gap - menu_width;
  if (left_x >= min_x)
    return {clamp(left_x, min_x, max_x), near_y};
  const int below_y = caret_bottom + candidate_clearance + gap;
  if (below_y + menu_height <= max_y)
    return {clamp(anchor.x - margin, min_x, max_x), below_y};
  const int above_y = anchor.y - gap - candidate_clearance - menu_height;
  return {clamp(anchor.x - margin, min_x, max_x), clamp(above_y, min_y, max_y)};
}

}  // namespace msime::linux_host
