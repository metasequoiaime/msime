#pragma once
#include "ToolbarLayout.h"
#include <algorithm>
#include <cmath>
#include <optional>

namespace msime::windows {
// 中英文切换提示（输入模式 HUD）的几何与摆放，和 macOS 的 MSIMEInputModeHUDPanel 同一套规则：徽标的高度、logo 和字号都取悬浮工具栏的（floating_toolbar.font_size 与 scale_percent），显示 0.6 秒；有光标位置时水平居中在光标下方，下面放不下就放到上方，没有光标时放在屏幕水平居中、下方三分之一处。纯计算，不依赖 Windows 头文件，主机上的单测直接包含。

// 「英」是英文模式，「中」是中文模式，与 macOS 的 MSIMEInputModeHUDText 相同。
inline const wchar_t *input_mode_hud_text(bool chinese) { return chinese ? L"中" : L"英"; }

// 徽标出现时给读屏软件的播报，与 macOS 的 NSAccessibilityAnnouncementRequestedNotification 念的相同。
inline const wchar_t *input_mode_hud_announcement(bool chinese) { return chinese ? L"中文输入" : L"英文输入"; }

// 完全显示的时长，与 macOS 的 kVisibleDuration 相同。
inline constexpr unsigned input_mode_hud_visible_ms = 600;

// 徽标在设备无关像素里的尺寸。卡片和工具栏一样高，左边是工具栏的 logo，右边一个字，两侧留半个字宽：24 号字时宽 78、高 52，和 macOS 用工具栏 24pt 算出的比例一致。
struct InputModeHudMetrics {
  double font = 24.0;
  double height = 52.0;
  double inset = 12.0;
  double logo = 24.0;
  double spacing = 6.0;
  double glyph = 24.0;
  ToolbarShadow shadow;
  double card_width() const {
    return inset * 2.0 + (logo > 0.0 ? logo + spacing : 0.0) + glyph;
  }
  double window_width() const { return shadow.left + card_width() + shadow.right; }
  double window_height() const { return shadow.top + height + shadow.bottom; }
};

// `show_logo` 为假时不给 logo 留位置：和 macOS 一样，只有可执行文件里取不到图标时才不画（单测链接的程序就没有图标资源）。超出工具栏范围的字号退回工具栏的出厂几何。
inline InputModeHudMetrics input_mode_hud_metrics(double font_size, bool show_logo) {
  const auto toolbar = toolbar_metrics(font_size);
  InputModeHudMetrics metrics;
  metrics.font = toolbar.icon;
  metrics.height = toolbar.height;
  metrics.inset = toolbar.icon / 2.0;
  metrics.logo = show_logo ? std::min(toolbar.icon, toolbar.icon_bottom - toolbar.icon_top) : 0.0;
  metrics.spacing = toolbar.icon / 4.0;
  metrics.glyph = toolbar.icon;
  metrics.shadow = toolbar.shadow;
  return metrics;
}

// 屏幕上的矩形，物理像素。
struct HudRect {
  long left = 0, top = 0, right = 0, bottom = 0;
};

struct InputModeHudPlacementInput {
  // 光标所在行的矩形；没有可用的光标时为空。
  std::optional<HudRect> caret;
  // 光标所在显示器的工作区。
  HudRect work;
  // 卡片本身和它四周阴影的像素尺寸。
  long card_width = 0, card_height = 0;
  long shadow_left = 0, shadow_top = 0;
  // 卡片离屏幕边缘和离光标的距离。
  long margin = 8, gap = 10;
};

// 窗口左上角的位置：卡片按规则摆好，再减去阴影边距。
struct HudPoint {
  long x = 0, y = 0;
};

inline long input_mode_hud_clamp(long value, long minimum, long maximum) {
  if (maximum < minimum)
    return minimum;
  return std::clamp(value, minimum, maximum);
}

inline bool input_mode_hud_usable_caret(const HudRect &caret) {
  return caret.bottom > caret.top && caret.right >= caret.left;
}

inline HudPoint input_mode_hud_placement(const InputModeHudPlacementInput &input) {
  const long min_x = input.work.left + input.margin;
  const long max_x = input.work.right - input.margin - input.card_width;
  const long min_y = input.work.top + input.margin;
  const long max_y = input.work.bottom - input.margin - input.card_height;
  long x = 0, y = 0;
  if (!input.caret || !input_mode_hud_usable_caret(*input.caret)) {
    // 没有光标：水平居中，卡片底边落在工作区自上而下四分之三处。
    x = input.work.left + (input.work.right - input.work.left - input.card_width) / 2;
    y = input.work.top + (input.work.bottom - input.work.top) * 3 / 4 - input.card_height;
  } else {
    const auto &caret = *input.caret;
    x = (caret.left + caret.right) / 2 - input.card_width / 2;
    const long below = caret.bottom + input.gap;
    y = below + input.card_height <= input.work.bottom - input.margin
            ? below
            : caret.top - input.gap - input.card_height;
  }
  return {input_mode_hud_clamp(x, min_x, max_x) - input.shadow_left,
          input_mode_hud_clamp(y, min_y, max_y) - input.shadow_top};
}

// 只有用户在同一会话里切换中英文时才出现（ModeAuthorityDecision::user_changed）：开关关着、全屏呈现（游戏、视频）时不出现，和悬浮工具栏在全屏时隐藏同一个理由——游戏里 Shift 常被当成奔跑键，每按一次闪一下会挡画面。
inline bool should_show_input_mode_hud(bool enabled, bool user_changed, bool fullscreen) {
  return enabled && user_changed && !fullscreen;
}
} // namespace msime::windows
