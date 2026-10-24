#include "../src/overlay/ModeBadgePlacement.h"
#include "../src/overlay/ModeBadgeStyle.h"

#include <cassert>
#include <cmath>

int main() {
  namespace host = msime::linux_host;

  // 徽章有自己的固定尺寸，与 macOS 紧凑切换提示相同：宿主只传默认的 1 倍缩放，从不读 Linux 不提供的 floating_toolbar.font_size 或 scale_percent。阴影由后端按合成器决定，默认不留边。
  const host::ModeBadgeMetrics defaults{};
  assert(defaults.scale == 1.0 && !defaults.shadow);

  // 带标志：8 + 16 + 4 + 字宽 + 8 宽，32 高，18 px 的字，8 px 圆角。
  const auto badge = host::mode_badge_layout(defaults, 18.0, true);
  assert(badge.plate_width == 54 && badge.plate_height == 32);
  assert(badge.width == 54 && badge.height == 32 && badge.margin == 0.0);
  assert(std::abs(badge.glyph_size - 18.0) < 1e-9);
  assert(badge.logo_side == 16.0 && badge.spacing == 4.0 && badge.inset == 8.0 && badge.radius == 8.0);
  // 字宽有小数时本体向外取整，字永远不会被裁掉。
  assert(host::mode_badge_layout(defaults, 18.4, true).width == 55);

  // 默认不带标志（show_app_logo 默认关）：字单独夹在两侧内边距之间。
  const auto bare = host::mode_badge_layout(defaults, 18.0, false);
  assert(bare.width == 34 && bare.height == 32 && !bare.logo && bare.logo_side == 0.0 && bare.spacing == 0.0);

  // 画阴影时表面四周各多 8 px 透明边，本体尺寸不变。
  const auto shadowed = host::mode_badge_layout(host::ModeBadgeMetrics{1.0, true}, 18.0, false);
  assert(shadowed.plate_width == 34 && shadowed.plate_height == 32 && shadowed.margin == 8.0);
  assert(shadowed.width == 50 && shadowed.height == 48);

  // 布局自己也能整体缩放：1.5 倍时高 48，标志、间距、内边距和圆角一起变大。
  const auto larger = host::mode_badge_layout(host::ModeBadgeMetrics{1.5, false}, 27.0, true);
  assert(larger.plate_height == 48 && larger.plate_width == 81);
  assert(larger.logo_side == 24.0 && larger.spacing == 6.0 && larger.inset == 12.0 && larger.radius == 12.0);
  assert(std::abs(larger.glyph_size - 27.0) < 1e-9);

  // 0.6 秒后分四帧淡出，与 macOS 的 0.6 + 0.18 秒相同；最后一帧不透明度为 0，宿主随即隐藏。
  assert(host::ModeBadgeTiming::visible_ms == 600 && host::ModeBadgeTiming::fade_ms == 180);
  assert(host::mode_badge_fade_alpha(0) == 1.0);
  assert(host::mode_badge_fade_alpha(1) == 0.75 && host::mode_badge_fade_alpha(2) == 0.5);
  assert(host::mode_badge_fade_alpha(3) == 0.25 && host::mode_badge_fade_alpha(4) == 0.0);
  assert(host::mode_badge_fade_alpha(9) == 0.0);

  // 光标旁的位置，照搬 macOS 的 MSIMEInputModeHUDFrame：1920 x 1080 的显示器，底部 48 px 面板，徽章 34 x 32，光标下方 10 px，离工作区边缘至少 8 px。
  {
    const host::WaveOverlayWorkArea work{0, 0, 1920, 1032};
    const auto at = [&](std::optional<host::ModeBadgeCaret> caret) {
      return host::mode_badge_caret_position(caret, 34, 32, work, 10, 8);
    };
    // 水平居中在光标下方。
    auto position = at(host::ModeBadgeCaret{500, 300, 2, 20});
    assert(position.x == 500 + 1 - 17 && position.y == 330);
    // 下方放不下就翻到光标上方。
    position = at(host::ModeBadgeCaret{500, 1010, 2, 20});
    assert(position.y == 1010 - 10 - 32);
    // 贴着左右边缘时夹回工作区以内。
    assert(at(host::ModeBadgeCaret{0, 300, 2, 20}).x == 8);
    assert(at(host::ModeBadgeCaret{1919, 300, 2, 20}).x == 1920 - 8 - 34);
    // 输入框没报光标（全 0 的矩形）时，放在水平居中、下沿离底边四分之一高的位置。
    position = at(std::nullopt);
    assert(position.x == (1920 - 34) / 2 && position.y == 1032 - 1032 / 4 - 32);
    const auto unusable = at(host::ModeBadgeCaret{0, 0, 0, 0});
    assert(unusable.x == position.x && unusable.y == position.y);
    // XIM 客户端报的是一个点（宽高为 0）：那是真实的光标位置，徽章放在它下方 gap 处，不走回退位置。
    assert(host::mode_badge_caret_usable(host::ModeBadgeCaret{500, 300, 0, 0}));
    assert(!host::mode_badge_caret_usable(host::ModeBadgeCaret{}));
    position = at(host::ModeBadgeCaret{500, 300, 0, 0});
    assert(position.x == 500 - 17 && position.y == 310);
    // 光标在根窗口负坐标上的显示器里时照样放在它下面。
    const host::WaveOverlayWorkArea left{-2560, 0, 2496, 1440};
    position = host::mode_badge_caret_position(host::ModeBadgeCaret{-1000, 200, 1, 18}, 34, 32, left, 10, 8);
    assert(position.x == -1000 - 17 && position.y == 228);
  }

  // 一次切换只出一处提示：能放到光标旁就只有徽章；否则面板的文字提示跟随光标，用户要了标志时再在角上显示带标志的徽章。
  {
    using host::ModeBadgeHint;
    assert(host::mode_badge_hint(true, true, true, false) == ModeBadgeHint::CaretBadge);
    assert(host::mode_badge_hint(true, true, true, true) == ModeBadgeHint::CaretBadge);
    assert(host::mode_badge_hint(true, false, true, false) == ModeBadgeHint::PanelText);
    assert(host::mode_badge_hint(true, true, false, false) == ModeBadgeHint::PanelText);
    assert(host::mode_badge_hint(true, false, true, true) == ModeBadgeHint::PanelTextAndCornerBadge);
    assert(host::mode_badge_hint(true, true, false, true) == ModeBadgeHint::PanelTextAndCornerBadge);
    assert(host::mode_badge_hint(false, false, false, true) == ModeBadgeHint::PanelText);
    // 关闭角落提示后，带标志的 Wayland 提示退回面板；X11 光标徽章及其标志仍可用。
    assert(host::mode_badge_hint(true, false, false, true, false) == ModeBadgeHint::PanelText);
    assert(host::mode_badge_hint(true, true, false, true, false) == ModeBadgeHint::PanelText);
    assert(host::mode_badge_hint(true, false, true, true, false) == ModeBadgeHint::PanelText);
    assert(host::mode_badge_hint(true, true, true, true, false) == ModeBadgeHint::CaretBadge);
    assert(!host::mode_badge_surface_wanted(false, true, false));
    assert(host::mode_badge_surface_wanted(true, true, false));
    // Wayland 客户端且没要标志时结果一定是面板提示，不必建表面。
    assert(!host::mode_badge_surface_wanted(false, false));
    assert(host::mode_badge_surface_wanted(true, false) && host::mode_badge_surface_wanted(false, true));
  }

  // 获得焦点时等过光标仍拿不到矩形，就改用面板提示，不把徽章放到屏幕下方；有矩形或本来就不是光标徽章时照旧。
  {
    using host::ModeBadgeHint;
    assert(host::ModeBadgeTiming::focus_caret_wait_ms > 0 &&
           host::ModeBadgeTiming::focus_caret_wait_ms < host::ModeBadgeTiming::visible_ms);
    const host::ModeBadgeCaret none{};
    const host::ModeBadgeCaret caret{400, 300, 1, 18};
    const host::ModeBadgeCaret point{400, 300, 0, 0};
    assert(host::mode_badge_focus_hint(ModeBadgeHint::CaretBadge, none) == ModeBadgeHint::PanelText);
    assert(host::mode_badge_focus_hint(ModeBadgeHint::CaretBadge, caret) == ModeBadgeHint::CaretBadge);
    assert(host::mode_badge_focus_hint(ModeBadgeHint::CaretBadge, point) == ModeBadgeHint::CaretBadge);
    assert(host::mode_badge_focus_hint(ModeBadgeHint::PanelTextAndCornerBadge, none) ==
           ModeBadgeHint::PanelTextAndCornerBadge);
    assert(host::mode_badge_focus_hint(ModeBadgeHint::PanelText, caret) == ModeBadgeHint::PanelText);
  }
}
