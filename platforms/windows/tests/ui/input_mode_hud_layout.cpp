#include "candidate/InputModeHudLayout.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Input mode HUD layout test failed at line " +
                           std::to_string(line));
}
void require_at(bool value, int line) {
  if (!value)
    require_failed(line);
}
#define require(value) require_at((value), __LINE__)
} // namespace
int main() {
  try {
    require(std::wstring(input_mode_hud_text(true)) == L"中");
    require(std::wstring(input_mode_hud_text(false)) == L"英");
    // 读屏播报与 macOS 逐字相同。
    require(std::wstring(input_mode_hud_announcement(true)) == L"中文输入");
    require(std::wstring(input_mode_hud_announcement(false)) == L"英文输入");

    // macOS 的紧凑尺寸：18 号字、高 32、logo 16、两侧各 8、logo 和字之间 4、圆角 8、描边 1。
    const auto metrics = input_mode_hud_metrics(true);
    require(metrics.font == 18.0 && metrics.height == 32.0 && metrics.logo == 16.0);
    require(metrics.inset == 8.0 && metrics.spacing == 4.0 && metrics.glyph == 18.0);
    require(metrics.radius == 8.0 && metrics.border == 1.0);
    require(metrics.card_width() == 8.0 * 2 + 16.0 + 4.0 + 18.0);
    // 和悬浮工具栏的尺寸无关：工具栏 24 号字时也不跟着它的高度。
    require(metrics.height != toolbar_metrics(24.0).height);
    // `show_app_logo` 关着（或取不到图标）时不留 logo 的位置。
    require(input_mode_hud_metrics(false).card_width() == 8.0 * 2 + 18.0);
    require(input_mode_hud_metrics(false).height == 32.0);
    // 窗口比卡片多出阴影边距。
    require(metrics.shadow.scale > 0.0);
    require(metrics.window_width() == metrics.card_width() + metrics.shadow.left + metrics.shadow.right);
    require(metrics.window_height() == 32.0 + metrics.shadow.top + metrics.shadow.bottom);

    // 完全显示 0.6 秒，再在 0.18 秒里线性淡出，与 macOS 相同。
    static_assert(input_mode_hud_visible_ms == 600 && input_mode_hud_fade_ms == 180);
    require(input_mode_hud_fade_opacity(0) == 1.0f);
    require(input_mode_hud_fade_opacity(90) == 0.5f);
    require(input_mode_hud_fade_opacity(179) > 0.0f);
    require(input_mode_hud_fade_opacity(180) == 0.0f && input_mode_hud_fade_opacity(5000) == 0.0f);
    for (unsigned long long elapsed = 1; elapsed <= 180; ++elapsed)
      require(input_mode_hud_fade_opacity(elapsed) < input_mode_hud_fade_opacity(elapsed - 1));

    InputModeHudPlacementInput input;
    input.work = {0, 0, 1920, 1040};
    input.card_width = 54;
    input.card_height = 32;
    input.shadow_left = 18;
    input.shadow_top = 16;
    // 光标下方放得下：水平居中在光标下面，离光标 10 像素。
    input.caret = HudRect{500, 300, 502, 320};
    auto placed = input_mode_hud_placement(input);
    require(placed.x == 501 - 27 - 18 && placed.y == 330 - 16);
    // 光标贴着屏幕底部：放到光标上方。
    input.caret = HudRect{500, 1000, 502, 1020};
    placed = input_mode_hud_placement(input);
    require(placed.y == 1000 - 10 - 32 - 16);
    // 光标贴着左边缘：卡片不出屏幕，留 8 像素。
    input.caret = HudRect{0, 300, 2, 320};
    placed = input_mode_hud_placement(input);
    require(placed.x == 8 - 18);
    // 光标贴着右边缘。
    input.caret = HudRect{1919, 300, 1920, 320};
    placed = input_mode_hud_placement(input);
    require(placed.x == 1920 - 8 - 54 - 18);
    // 第二块显示器上的工作区。
    input.work = {1920, 0, 3840, 1040};
    input.caret = HudRect{2500, 300, 2502, 320};
    placed = input_mode_hud_placement(input);
    require(placed.x == 2501 - 27 - 18);
    // 没有光标：水平居中、卡片底边在工作区四分之三处。
    input.caret.reset();
    placed = input_mode_hud_placement(input);
    require(placed.x == 1920 + (1920 - 54) / 2 - 18 && placed.y == 780 - 32 - 16);
    // 高度为零的光标矩形不可用，同样退回居中。
    input.caret = HudRect{2500, 300, 2502, 300};
    require(input_mode_hud_placement(input).y == placed.y);

    // 只在用户切换、开关开着、不是全屏时出现。
    require(should_show_input_mode_hud(true, true, false));
    require(!should_show_input_mode_hud(false, true, false));
    require(!should_show_input_mode_hud(true, false, false));
    require(!should_show_input_mode_hud(true, true, true));
    std::cout << "Input mode HUD: compact badge beside the caret\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  } catch (...) {
    std::cerr << "Input mode HUD layout test failed with an unknown error\n";
    return 1;
  }
}
