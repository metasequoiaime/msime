#include "../../src/core/VimMode.h"

#include <cassert>
#include <string>
#include <vector>

using msime::mac::VimModeEscapeSwitchesToEnglish;
using msime::mac::VimModeListContains;

int main() {
    // 名单匹配：ASCII 不分大小写，bundle id 用户怎么写都算同一个应用。
    const std::vector<std::string> apps{"com.apple.Terminal", "com.neovide.neovide"};
    assert(VimModeListContains(apps, "com.apple.Terminal"));
    assert(VimModeListContains(apps, "com.apple.terminal"));
    assert(VimModeListContains(apps, "COM.APPLE.TERMINAL"));
    assert(VimModeListContains(apps, "com.neovide.neovide"));
    assert(!VimModeListContains(apps, "com.apple.Safari"));
    // 前缀或尾部多一截都不算同一条。
    assert(!VimModeListContains(apps, "com.apple.Terminal2"));
    assert(!VimModeListContains(apps, "com.apple.Termina"));
    // 空标识与空名单一律不命中。
    assert(!VimModeListContains(apps, ""));
    assert(!VimModeListContains({}, "com.apple.Terminal"));

    // 纯 Esc 策略：名单命中、无组字、无修饰、非长按、当前中文时切英文。
    assert(VimModeEscapeSwitchesToEnglish(53, false, false, false, true, false));
    // 长按 Esc 的重复事件不切换。
    assert(!VimModeEscapeSwitchesToEnglish(53, true, false, false, true, false));
    // 带任一修饰键的 Esc 保持原行为。
    assert(!VimModeEscapeSwitchesToEnglish(53, false, true, false, true, false));
    // 组字开着时这次 Esc 归引擎取消组字，不能既取消又切。
    assert(!VimModeEscapeSwitchesToEnglish(53, false, false, true, true, false));
    // 应用不在名单里不切换。
    assert(!VimModeEscapeSwitchesToEnglish(53, false, false, false, false, false));
    // 已经是英文时没有可切的。
    assert(!VimModeEscapeSwitchesToEnglish(53, false, false, false, true, true));
    // 不是 Esc 的键一律不切换。
    assert(!VimModeEscapeSwitchesToEnglish(51, false, false, false, true, false));
    assert(!VimModeEscapeSwitchesToEnglish(36, false, false, false, true, false));
    return 0;
}
