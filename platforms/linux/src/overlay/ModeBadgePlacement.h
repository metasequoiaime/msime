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

// 与 macOS 的 MSIMEInputModeHUDUsableCaretRect 相同：高度为 0 的矩形是输入框没报光标，不能拿来定位。
constexpr bool mode_badge_caret_usable(const ModeBadgeCaret &caret) { return caret.height > 0; }

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

// 一次切换给用户的提示。macOS 只在光标旁显示一块徽章；Linux 只有能把徽章放到光标旁（X11 客户端、X11 后端）时才这样做，其余情况由面板的文字提示跟随光标——Fcitx5 是 showCustomInputMethodInformation，IBus 是辅助文本——两处提示不同时出现。唯一的例外是用户要了标志（`show_app_logo`）而徽章只能放在屏幕角上：面板的文字提示带不了图，所以角上的徽章与它并存。
enum class ModeBadgeHint { CaretBadge, PanelText, PanelTextAndCornerBadge };

constexpr ModeBadgeHint mode_badge_hint(bool badge_available, bool badge_follows_caret, bool caret_in_root_coordinates,
                                        bool show_logo) {
  if (badge_available && badge_follows_caret && caret_in_root_coordinates) return ModeBadgeHint::CaretBadge;
  if (badge_available && show_logo) return ModeBadgeHint::PanelTextAndCornerBadge;
  return ModeBadgeHint::PanelText;
}

}  // namespace msime::linux_host
