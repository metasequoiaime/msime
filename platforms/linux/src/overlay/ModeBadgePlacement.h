#pragma once

#include <algorithm>
#include <optional>

#include "ModeBadgeStyle.h"
#include "WaveOverlayPlacement.h"

namespace msime::linux_host {

// 输入框报给输入法的光标矩形，单位是根窗口的设备像素（X11 客户端经 XIM、GTK/Qt 输入法模块报的都是这种坐标）。Wayland 客户端报的是自己表面里的局部坐标，宿主不会把那种矩形交到这里。
struct ModeBadgeCaret {
  int x = 0;
  int y = 0;
  int width = 0;
  int height = 0;
};

// 输入框报过光标没有：只有四个分量全为 0 的矩形算没报（Fcitx5 和 IBus 给新上下文的初值就是它，GTK、Qt 在补报之前也停在这里）。macOS 的 MSIMEInputModeHUDUsableCaretRect 把高度为 0 的也当成没报，Linux 不能照搬：XIM 客户端（xterm、X 下的 Emacs、Wine 和 Java 程序）报的是一个点，Fcitx5 的 XIM 前端在换算后的位置放一个宽高都为 0 的矩形，ibus-x11 传的宽度为 0、高度通常也为 0，这是真实的光标位置，徽章照样放在它下方 gap 处。
constexpr bool mode_badge_caret_usable(const ModeBadgeCaret &caret) {
  return caret.x != 0 || caret.y != 0 || caret.width != 0 || caret.height != 0;
}

namespace mode_badge_detail {
constexpr int clamp(int value, int minimum, int maximum) {
  if (maximum < minimum) return minimum;
  return value < minimum ? minimum : (value > maximum ? maximum : value);
}
}  // namespace mode_badge_detail

// 徽章本体左上角的位置，照搬 macOS 的 MSIMEInputModeHUDFrame，只是换成 y 轴向下的根窗口坐标：水平居中在光标下方 gap 处，下面放不下就翻到光标上方，最后夹在工作区四边各 margin 以内。光标不可用时放在工作区水平居中、下沿离底边四分之一高的位置，与 macOS 的回退位置相同。
constexpr WaveOverlayPosition mode_badge_caret_position(std::optional<ModeBadgeCaret> caret, int plate_width,
                                                        int plate_height, WaveOverlayWorkArea work, int gap,
                                                        int margin) {
  using mode_badge_detail::clamp;
  const int minimum_x = work.x + margin;
  const int maximum_x = work.x + work.width - margin - plate_width;
  const int minimum_y = work.y + margin;
  const int maximum_y = work.y + work.height - margin - plate_height;
  if (!caret || !mode_badge_caret_usable(*caret)) {
    const int centered_x = work.x + (work.width - plate_width) / 2;
    const int lower_quarter_y = work.y + work.height - work.height / 4 - plate_height;
    return {clamp(centered_x, minimum_x, maximum_x), clamp(lower_quarter_y, minimum_y, maximum_y)};
  }
  const int x = clamp(caret->x + caret->width / 2 - plate_width / 2, minimum_x, maximum_x);
  const int below = caret->y + caret->height + gap;
  const int preferred = below <= maximum_y ? below : caret->y - gap - plate_height;
  return {x, clamp(preferred, minimum_y, maximum_y)};
}

// 一次切换给用户的提示。macOS 只在光标旁显示一块徽章；Linux 只有能把徽章放到光标旁（X11 客户端、X11 后端）时才这样做，其余情况由面板的文字提示跟随光标——Fcitx5 是 showCustomInputMethodInformation，IBus 是辅助文本——两处提示不同时出现。唯一的例外是用户要了标志（`show_app_logo`）、允许角落提示（`input_mode_corner_hud`）而徽章只能放在屏幕角上：面板的文字提示带不了图，所以角上的徽章与它并存。
enum class ModeBadgeHint { CaretBadge, PanelText, PanelTextAndCornerBadge };

constexpr ModeBadgeHint mode_badge_hint(bool badge_available, bool badge_follows_caret, bool caret_in_root_coordinates,
                                        bool show_logo, bool allow_corner = true) {
  if (badge_available && badge_follows_caret && caret_in_root_coordinates) return ModeBadgeHint::CaretBadge;
  if (badge_available && show_logo && allow_corner) return ModeBadgeHint::PanelTextAndCornerBadge;
  return ModeBadgeHint::PanelText;
}

// 只有可能画出东西时才建徽章表面：输入框的光标不是根窗口坐标、用户也没要标志或关闭了角落提示时，mode_badge_hint 无论有没有表面都回答面板提示，建了只是白占一条显示服务器连接。
constexpr bool mode_badge_surface_wanted(bool caret_in_root_coordinates, bool show_logo, bool allow_corner = true) {
  return caret_in_root_coordinates || (show_logo && allow_corner);
}

// 获得焦点时的提示（#2589）在输入框补报光标之后才定（宿主最多等 ModeBadgeTiming::focus_caret_wait_ms）。等完仍没有可用的光标矩形时不把徽章放到屏幕下方的回退位置，因为用户看的是刚点进去的输入框：改用面板的文字提示，它由面板跟着之后报来的光标摆放。真正的切换不走这里，那时输入框早已报过光标，与 macOS 一样用回退位置。
constexpr ModeBadgeHint mode_badge_focus_hint(ModeBadgeHint hint, const ModeBadgeCaret &caret) {
  if (hint == ModeBadgeHint::CaretBadge && !mode_badge_caret_usable(caret)) return ModeBadgeHint::PanelText;
  return hint;
}

}  // namespace msime::linux_host
