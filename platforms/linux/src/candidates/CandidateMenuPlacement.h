#pragma once

#include "../overlay/WaveOverlayPlacement.h"

namespace msime::linux_host {

// 右键菜单的定位锚点。GNOME Shell 会话的菜单由扩展画，不经过这里；其余会话按显示协议分两种：
// - Pointer：纯 X11 会话，XQueryPointer 取到的根窗口指针坐标（坐标为负时由 CandidateMenuSurface::show 现取）。
// - Cursor：Wayland 会话（KDE Plasma、sway 等），用 IBus 上报的光标矩形 `engine->cursor_area`。Xwayland 的根窗口跟踪不到合成器管理的指针，XQueryPointer 只会返回陈旧的屏幕角落坐标，把菜单摆到桌面右上角，所以这里绝不读指针。
struct CandidateMenuAnchor {
  enum class Kind { Pointer, Cursor };
  Kind kind = Kind::Pointer;
  int x = 0;
  int y = 0;
  int width = 0;
  int height = 0;
};

using CandidateMenuPosition = WaveOverlayPosition;

// 按会话选锚点：有 WAYLAND_DISPLAY 的会话用光标矩形，纯 X11 用指针（坐标留给 show 现取）。
constexpr CandidateMenuAnchor candidate_menu_anchor(bool wayland_session, int cursor_x, int cursor_y,
                                                    int cursor_width, int cursor_height) {
  if (!wayland_session)
    return {CandidateMenuAnchor::Kind::Pointer, -1, -1, 0, 0};
  return {CandidateMenuAnchor::Kind::Cursor, cursor_x, cursor_y, cursor_width, cursor_height};
}

namespace candidate_menu_detail {
constexpr int clamp(int value, int minimum, int maximum) {
  if (maximum < minimum) return minimum;
  return value < minimum ? minimum : (value > maximum ? maximum : value);
}

// 光标矩形能不能当锚点：四个分量全 0 是客户端还没报光标；不完整落在工作区里多半是 Wayland 原生客户端报的表面局部坐标，不是根坐标。
constexpr bool cursor_usable(const CandidateMenuAnchor &anchor, WaveOverlayWorkArea work) {
  if (anchor.x == 0 && anchor.y == 0 && anchor.width == 0 && anchor.height == 0) return false;
  return anchor.x >= work.x && anchor.y >= work.y && anchor.x + anchor.width <= work.x + work.width &&
         anchor.y + anchor.height <= work.y + work.height;
}
}  // namespace candidate_menu_detail

// 菜单左上角位置。
// - 指针模式「下方放不下翻上方」：纯 X11 下候选框和菜单同是 X11 窗口，不存在合成器遮挡。
// - 光标模式把菜单水平居中于光标、整体放在光标上方，与光标下方的候选框错开；全程夹在工作区里。光标矩形不可用时回到工作区水平居中、下沿离底边四分之一处，绝不落到桌面角落。
constexpr CandidateMenuPosition candidate_menu_position(CandidateMenuAnchor anchor, int menu_width,
                                                        int menu_height, int margin, int gap,
                                                        WaveOverlayWorkArea work) {
  using candidate_menu_detail::clamp;
  if (anchor.kind == CandidateMenuAnchor::Kind::Pointer) {
    const int x = anchor.x - margin;
    int y = anchor.y - margin;
    if (y + menu_height - margin > work.y + work.height)
      y = anchor.y - (menu_height - margin);
    return {clamp(x, work.x - margin, work.x + work.width - menu_width + margin),
            clamp(y, work.y - margin, work.y + work.height - menu_height + margin)};
  }
  const int min_x = work.x + margin;
  const int max_x = work.x + work.width - margin - menu_width;
  const int min_y = work.y + margin;
  const int max_y = work.y + work.height - margin - menu_height;
  if (!candidate_menu_detail::cursor_usable(anchor, work)) {
    const int centered_x = work.x + (work.width - menu_width) / 2;
    const int lower_quarter_y = work.y + work.height - work.height / 4 - menu_height;
    return {clamp(centered_x, min_x, max_x), clamp(lower_quarter_y, min_y, max_y)};
  }
  const int centered_x = anchor.x + anchor.width / 2 - menu_width / 2;
  const int above_y = anchor.y - gap - menu_height;
  return {clamp(centered_x, min_x, max_x), clamp(above_y, min_y, max_y)};
}

}  // namespace msime::linux_host
