#pragma once
#include "ToolbarLayout.h"
#include <algorithm>
#include <cmath>
#include <optional>
#include <string>

namespace msime::windows {
struct InputStateHudText {
  std::wstring text;
  std::wstring announcement;
  // 回应用户刚点的菜单项（例如「加入生词本」）的提示：不受「切换提示」开关约束，那个开关只管输入状态切换。
  bool answers_action = false;
};
inline InputStateHudText character_set_hud(bool traditional) {
  return {traditional ? L"繁" : L"简", traditional ? L"繁体输出" : L"简体输出"};
}
inline InputStateHudText fullwidth_hud(bool fullwidth) {
  return {fullwidth ? L"全角" : L"半角", fullwidth ? L"全角输入" : L"半角输入"};
}
inline InputStateHudText punctuation_hud(bool chinese) {
  return {chinese ? L"中文标点" : L"英文标点", chinese ? L"中文标点" : L"英文标点"};
}
inline InputStateHudText dedicated_english_hud(bool enabled) {
  return {enabled ? L"英文候选" : L"英文候选关",
          enabled ? L"英文候选模式已开启" : L"英文候选模式已关闭"};
}
inline InputStateHudText translations_hud(bool enabled) {
  return {enabled ? L"译文开" : L"译文关", enabled ? L"候选译文已开启" : L"候选译文已关闭"};
}
// 字形沿用工具栏的语言标记，避免日语等方案切回输入态时误报「中」。
inline InputStateHudText input_language_hud(bool chinese, const std::string &scheme) {
  if (!chinese) return {L"英", L"英文输入"};
  if (scheme == "japanese") return {L"日", L"日语输入"};
  if (scheme == "korean") return {L"한", L"韩语输入"};
  if (scheme == "vietnamese") return {L"越", L"越南语输入"};
  if (scheme == "tibetan") return {L"ཀ", L"藏文输入"};
  return {L"中", L"中文输入"};
}
// 输入状态 HUD 的几何与摆放，和 macOS 的 MSIMEInputModeHUDPanel 同一套规则：固定字高、按文字宽度展开，不随悬浮工具栏的字号和缩放变化，只随显示器 DPI 缩放；logo 只在 `show_app_logo` 开着时画；完全显示 0.6 秒，再用 0.18 秒淡出；有光标位置时水平居中在光标下方，下面放不下就放到上方，没有光标时放在屏幕水平居中、下方三分之一处。纯计算，不依赖 Windows 头文件，主机上的单测直接包含。

// 「英」是英文模式，「中」是中文模式，与 macOS 的 MSIMEInputModeHUDText 相同。
inline const wchar_t *input_mode_hud_text(bool chinese) { return chinese ? L"中" : L"英"; }

// 徽标出现时给读屏软件的播报，与 macOS 的 NSAccessibilityAnnouncementRequestedNotification 念的相同。
inline const wchar_t *input_mode_hud_announcement(bool chinese) { return chinese ? L"中文输入" : L"英文输入"; }

// 完全显示的时长，与 macOS 的 kVisibleDuration 相同。
inline constexpr unsigned input_mode_hud_visible_ms = 600;
// 之后淡出的时长，与 macOS 的 kFadeDuration 相同；淡出按大约每 16 毫秒一帧重画。
inline constexpr unsigned input_mode_hud_fade_ms = 180;
inline constexpr unsigned input_mode_hud_fade_frame_ms = 16;
// 淡出开始 `elapsed` 毫秒后的不透明度：从 1 线性降到 0，到 0 时收起窗口。
inline float input_mode_hud_fade_opacity(unsigned long long elapsed) {
  if (elapsed >= input_mode_hud_fade_ms)
    return 0.0f;
  return 1.0f - static_cast<float>(elapsed) / static_cast<float>(input_mode_hud_fade_ms);
}

// 徽标在设备无关像素里的尺寸，取 macOS 的固定紧凑尺寸（InputModeHUDPanel.mm 的 kFontSize 等常量）：18 号常规字、高 32、logo 16、左右各留 8、logo 和字之间 4、圆角 8、描边 1。它和悬浮工具栏的字号、缩放无关：放大工具栏是为了好点，鼠标穿透、只显示 0.6 秒的提示不需要跟着放大。
struct InputModeHudMetrics {
  double font = 18.0;
  double height = 32.0;
  double inset = 8.0;
  double logo = 16.0;
  double spacing = 4.0;
  // 实测文字宽度，单字最少留出一个全角字的宽度。
  double glyph = 18.0;
  double radius = 8.0;
  double border = 1.0;
  ToolbarShadow shadow;
  double card_width() const {
    return inset * 2.0 + (logo > 0.0 ? logo + spacing : 0.0) + glyph;
  }
  double window_width() const { return shadow.left + card_width() + shadow.right; }
  double window_height() const { return shadow.top + height + shadow.bottom; }
};

// `show_logo` 是偏好 `show_app_logo` 开着、而且可执行文件里取得到图标（单测链接的程序没有图标资源）：为假时不给 logo 留位置，和 macOS 一样。
inline InputModeHudMetrics input_mode_hud_metrics(bool show_logo, double text_width = 18.0) {
  InputModeHudMetrics metrics;
  metrics.glyph = std::max(18.0, std::ceil(text_width));
  if (!show_logo)
    metrics.logo = 0.0;
  metrics.shadow = toolbar_shadow(true);
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
