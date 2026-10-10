#include "../../LanguageBar/ModeIconPolicy.h"
#include <cstdio>
#include <utility>

using msime::tsf::mode_icon;
using msime::tsf::ModeIcon;
using msime::windows::scheme::InputMode;

namespace {
int failures = 0;

void check(bool condition, const char *what) {
    if (!condition) {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}
} // namespace

int main() {
    // 每个方案一个字，与 macOS 输入菜单的模式图标对应；全拼是 中。
    const std::pair<InputMode, ModeIcon> modes[] = {
        {InputMode::Chinese, ModeIcon::Chinese},       {InputMode::Shuangpin, ModeIcon::Shuangpin},
        {InputMode::Wubi, ModeIcon::Wubi},             {InputMode::Japanese, ModeIcon::Japanese},
        {InputMode::Korean, ModeIcon::Korean},         {InputMode::Cantonese, ModeIcon::Cantonese},
        {InputMode::Zhuyin, ModeIcon::Zhuyin},         {InputMode::Vietnamese, ModeIcon::Vietnamese},
        {InputMode::Tibetan, ModeIcon::Tibetan},       {InputMode::Stroke, ModeIcon::Stroke},
    };
    for (const auto &[mode, icon] : modes) {
        check(mode_icon(true, false, false, mode) == icon, "an open button shows the mode's character");
        // 关着（英文）和禁用时都是 英，哪个方案都一样。
        check(mode_icon(false, false, false, mode) == ModeIcon::English, "a closed button shows English");
        check(mode_icon(true, true, false, mode) == ModeIcon::English, "a disabled button shows English");
        // Caps Lock 在按钮可用时排在最前，开关都一样；禁用时不显示。
        check(mode_icon(true, false, true, mode) == ModeIcon::CapsLock, "Caps Lock beats the mode");
        check(mode_icon(false, false, true, mode) == ModeIcon::CapsLock, "Caps Lock beats English");
        check(mode_icon(true, true, true, mode) == ModeIcon::English, "a disabled button ignores Caps Lock");
    }
    // 旧 Server 只发 '0'，双拼和五笔仍显示 中；不认识的码读成中文。
    check(mode_icon(true, false, false, msime::windows::scheme::input_mode_from_code(L'0')) == ModeIcon::Chinese,
          "an old Server's Chinese code shows Chinese");
    check(mode_icon(true, false, false, msime::windows::scheme::input_mode_from_code(L':')) == ModeIcon::Chinese,
          "an unknown code shows Chinese");
    if (failures)
        return 1;
    std::puts("Mode icon policy: one character per scheme, Caps Lock and English first");
    return 0;
}
