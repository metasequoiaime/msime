#pragma once
#include <algorithm>
#include <cmath>

#include "../candidates/CandidatePalette.h"

namespace msime::linux_host {

// 徽章用 macOS 紧凑切换提示（`InputModeHUDPanel.mm`，见 2026-10-14-macos-compact-input-mode-hud.md）的同一套尺寸：18 px 常规字重的「中」或「英」，32 px 高，左右各 8 px 内边距，8 px 圆角，1 px 描边；用户打开「显示水杉图标」（`show_app_logo`）时字前加一个 16 px 的全彩标志，两者相隔 4 px。徽章不读共享的 floating_toolbar.font_size 和 scale_percent：工具栏放大是为了好点，这个点击穿透、只露 0.6 秒的提示用不着跟着放大，macOS 也已不再跟随工具栏。宽度按内容算。所有长度都是逻辑像素，X11 后端再乘桌面缩放。
//
// macOS 的提示窗有系统阴影；这里在 ARGB 缓冲里自己画一圈柔和阴影，为此表面四周各留 shadow_margin 的透明边，徽章本体（plate）画在里面。只有合成器在场时才画：没有合成器的 X11 上透明像素会变成黑边。
struct ModeBadgeGeometry {
  static constexpr double glyph_size = 18.0;
  static constexpr double height = 32.0;
  static constexpr double logo_side = 16.0;
  static constexpr double corner_radius = 8.0;
  static constexpr double horizontal_inset = 8.0;
  static constexpr double content_spacing = 4.0;
  static constexpr double border_width = 1.0;
  static constexpr double shadow_margin = 8.0;
  // 与 macOS 相同：徽章放在光标下方 10 px，离屏幕（工作区）边缘至少 8 px。
  static constexpr double caret_gap = 10.0;
  static constexpr double screen_margin = 8.0;
};

// 徽章显示 0.6 秒，再用 0.18 秒淡出，与 macOS 的 kVisibleDuration、kFadeDuration 相同。淡出分几帧重画降低透明度，帧数够看出渐隐即可，不必逐帧动画。
struct ModeBadgeTiming {
  static constexpr int visible_ms = 600;
  static constexpr int fade_ms = 180;
  static constexpr int fade_steps = 4;
  // 获得焦点时最多等这么久让输入框报光标矩形（见 ModeBadgePlacement.h 的 mode_badge_focus_hint）。GTK 和 Qt 的输入法模块在获得焦点之后才在空闲回调里补报光标，通常几毫秒就到；矩形与上次相同时 Fcitx5 不发变化事件，这时等满这段时间再用记下的矩形。
  static constexpr int focus_caret_wait_ms = 150;
};

// 淡出第 step 帧（从 1 起）的不透明度；step 到 fade_steps 时为 0，宿主随即隐藏。
constexpr double mode_badge_fade_alpha(int step) {
  const int steps = ModeBadgeTiming::fade_steps;
  if (step <= 0) return 1.0;
  if (step >= steps) return 0.0;
  return static_cast<double>(steps - step) / steps;
}

struct ModeBadgeMetrics {
  double scale = 1.0;
  // 是否画阴影（要留出透明边）。由后端按合成器是否在场决定，宿主不填。
  bool shadow = false;
};

// 按一个字宽排好的徽章：表面的整像素尺寸（向外取整，什么都不裁掉，与 macOS 取整窗口一致）、徽章本体在表面里的位置和大小，以及标志和字的位置。
struct ModeBadgeLayout {
  int width = 0;
  int height = 0;
  int plate_width = 0;
  int plate_height = 0;
  double margin = 0;
  double glyph_size = 0;
  double glyph_width = 0;
  double logo_side = 0;
  double spacing = 0;
  double inset = 0;
  double radius = 0;
  bool logo = false;
};

// glyph_width 取「中」「英」两字在 glyph_size 下较宽的一个，所以切换模式时徽章宽度不变。没有标志时字单独居中在两侧内边距之间。
inline double mode_badge_glyph_size(const ModeBadgeMetrics &metrics) {
  return ModeBadgeGeometry::glyph_size * metrics.scale;
}

inline ModeBadgeLayout mode_badge_layout(const ModeBadgeMetrics &metrics, double glyph_width, bool logo) {
  using G = ModeBadgeGeometry;
  ModeBadgeLayout layout;
  layout.glyph_size = mode_badge_glyph_size(metrics);
  layout.glyph_width = std::max(0.0, glyph_width);
  layout.logo = logo;
  layout.logo_side = logo ? G::logo_side * metrics.scale : 0.0;
  layout.spacing = logo ? G::content_spacing * metrics.scale : 0.0;
  layout.inset = G::horizontal_inset * metrics.scale;
  layout.radius = G::corner_radius * metrics.scale;
  layout.plate_width = static_cast<int>(std::ceil(2.0 * layout.inset + layout.logo_side + layout.spacing + layout.glyph_width));
  layout.plate_height = static_cast<int>(std::ceil(G::height * metrics.scale));
  layout.margin = metrics.shadow ? std::ceil(G::shadow_margin * metrics.scale) : 0.0;
  const int margin = static_cast<int>(layout.margin);
  layout.width = layout.plate_width + 2 * margin;
  layout.height = layout.plate_height + 2 * margin;
  return layout;
}

// 一次 show() 要画的东西：缩放和解析出的主题颜色。
struct ModeBadgeStyle {
  ModeBadgeMetrics metrics;
  FloatingSurfaceColors colors;
};

}  // namespace msime::linux_host
