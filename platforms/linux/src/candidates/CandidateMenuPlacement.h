#pragma once

#include "../overlay/WaveOverlayPlacement.h"

namespace msime::linux_host {

// 右键菜单的定位锚点：纯 X11 会话用指针（XQueryPointer 取到的根窗口坐标）；GNOME Shell
// 会话用候选框的真实矩形（扩展经 D-Bus 导出），菜单摆在候选框旁边、避开这个由合成器
// 绘制、压在所有 X11 客户端窗口之上的 actor。Wayland 的 Xwayland 根窗口没有真实指针
// （child 为 0、坐标陈旧在屏幕角落），所以那条路在 GNOME 会话里不能用来定位菜单。
struct CandidateMenuAnchor {
  // Pointer：纯 X11 的 XQueryPointer 根窗口坐标；CandidateRect：GNOME Shell 扩展给出的
  // 候选框矩形（x/y 为左上角，width/height 为大小）。
  enum class Kind { Pointer, CandidateRect };
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

// 候选框矩形能不能当锚点：宽高非正（扩展没数据或候选框不可见）不能用；矩形不完整落在
// 工作区里，说明扩展和我们的缩放/显示器不一致，也不能用。指针模式的坐标由 XQueryPointer
// 取，只在纯 X11 会话里传进来，这里不重复校验。
constexpr bool candidate_menu_anchor_usable(const CandidateMenuAnchor &anchor, WaveOverlayWorkArea work) {
  if (anchor.kind == CandidateMenuAnchor::Kind::Pointer) return true;
  if (anchor.width <= 0 || anchor.height <= 0) return false;
  return anchor.x >= work.x && anchor.y >= work.y &&
         anchor.x + anchor.width <= work.x + work.width &&
         anchor.y + anchor.height <= work.y + work.height;
}

// 菜单左上角位置。候选框矩形模式依次尝试候选框的右、左、下、上：取第一个能让菜单完整
// 避开候选框、又夹进工作区的位置；横向摆放时菜单与候选框垂直居中，纵向摆放时水平居中。
// 锚点不可用回到工作区水平居中、下沿离底边四分之一处，绝不落在桌面右上角。纯 X11（指针
// 模式）保持「下方放不下翻上方」，那里的候选框和菜单同是 X11 窗口，不存在合成器遮挡。
constexpr CandidateMenuPosition candidate_menu_position(CandidateMenuAnchor anchor, int menu_width,
                                                        int menu_height, int margin, int gap,
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
  const int centered_x = clamp(anchor.x + anchor.width / 2 - menu_width / 2, min_x, max_x);
  const int centered_y = clamp(anchor.y + anchor.height / 2 - menu_height / 2, min_y, max_y);
  const int right_x = anchor.x + anchor.width + gap;
  if (right_x + menu_width <= work.x + work.width - margin)
    return {clamp(right_x, min_x, max_x), centered_y};
  const int left_x = anchor.x - gap - menu_width;
  if (left_x >= work.x + margin)
    return {clamp(left_x, min_x, max_x), centered_y};
  const int below_y = anchor.y + anchor.height + gap;
  if (below_y + menu_height <= work.y + work.height - margin)
    return {centered_x, clamp(below_y, min_y, max_y)};
  const int above_y = anchor.y - gap - menu_height;
  return {centered_x, clamp(above_y, min_y, max_y)};
}

}  // namespace msime::linux_host
