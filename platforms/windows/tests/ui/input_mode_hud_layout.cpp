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
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
} // namespace
int main() {
  try {
    require(std::wstring(input_mode_hud_text(true)) == L"中");
    require(std::wstring(input_mode_hud_text(false)) == L"英");
    // 读屏播报与 macOS 逐字相同。
    require(std::wstring(input_mode_hud_announcement(true)) == L"中文输入");
    require(std::wstring(input_mode_hud_announcement(false)) == L"英文输入");

    // 默认 24 号字：和工具栏一样高，logo、一个字和两侧留白。
    const auto metrics = input_mode_hud_metrics(24.0, true);
    require(metrics.height == toolbar_metrics(24.0).height);
    require(metrics.card_width() == 12.0 * 2 + 24.0 + 6.0 + 24.0);
    // 字号跟着工具栏变，徽标整体放大。
    const auto large = input_mode_hud_metrics(28.0, true);
    require(large.height > metrics.height && large.card_width() > metrics.card_width());
    // 取不到图标时不留 logo 的位置。
    require(input_mode_hud_metrics(24.0, false).card_width() == 12.0 * 2 + 24.0);
    // 超出范围的字号退回出厂几何。
    require(input_mode_hud_metrics(200.0, true).height == 52.0);
    // 窗口比卡片多出阴影边距。
    require(metrics.window_width() == metrics.card_width() + metrics.shadow.left + metrics.shadow.right);

    InputModeHudPlacementInput input;
    input.work = {0, 0, 1920, 1040};
    input.card_width = 78;
    input.card_height = 52;
    input.shadow_left = 18;
    input.shadow_top = 16;
    // 光标下方放得下：水平居中在光标下面，离光标 10 像素。
    input.caret = HudRect{500, 300, 502, 320};
    auto placed = input_mode_hud_placement(input);
    require(placed.x == 501 - 39 - 18 && placed.y == 330 - 16);
    // 光标贴着屏幕底部：放到光标上方。
    input.caret = HudRect{500, 1000, 502, 1020};
    placed = input_mode_hud_placement(input);
    require(placed.y == 1000 - 10 - 52 - 16);
    // 光标贴着左边缘：卡片不出屏幕，留 8 像素。
    input.caret = HudRect{0, 300, 2, 320};
    placed = input_mode_hud_placement(input);
    require(placed.x == 8 - 18);
    // 光标贴着右边缘。
    input.caret = HudRect{1919, 300, 1920, 320};
    placed = input_mode_hud_placement(input);
    require(placed.x == 1920 - 8 - 78 - 18);
    // 第二块显示器上的工作区。
    input.work = {1920, 0, 3840, 1040};
    input.caret = HudRect{2500, 300, 2502, 320};
    placed = input_mode_hud_placement(input);
    require(placed.x == 2501 - 39 - 18);
    // 没有光标：水平居中、卡片底边在工作区四分之三处。
    input.caret.reset();
    placed = input_mode_hud_placement(input);
    require(placed.x == 1920 + (1920 - 78) / 2 - 18 && placed.y == 780 - 52 - 16);
    // 高度为零的光标矩形不可用，同样退回居中。
    input.caret = HudRect{2500, 300, 2502, 300};
    require(input_mode_hud_placement(input).y == placed.y);

    // 只在用户切换、开关开着、不是全屏时出现。
    require(should_show_input_mode_hud(true, true, false));
    require(!should_show_input_mode_hud(false, true, false));
    require(!should_show_input_mode_hud(true, false, false));
    require(!should_show_input_mode_hud(true, true, true));
    std::cout << "Input mode HUD: toolbar-sized badge beside the caret\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  } catch (...) {
    std::cerr << "Input mode HUD layout test failed with an unknown error\n";
    return 1;
  }
}
