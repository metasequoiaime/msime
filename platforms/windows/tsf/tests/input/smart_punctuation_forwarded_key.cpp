// 转发给应用的按键在智能标点前文影子里记成什么：策略逐条钉住，TSF 里的接线按源码检查。
//
// 影子优先于文档回读，记错一次，下一个标点就走错半角或全角。只看虚拟键码不看修饰键时，英文模式的 Shift+5（打出 %）会被记成数字 5，Ctrl+1、Ctrl+V 会被记成数字和字母，切回中文后的第一个标点变成 ASCII（#11198 验收查出的两条回归）。
#include "SmartPunctuationForwardedKey.h"

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <string>

namespace
{
constexpr unsigned kNone = 0;
constexpr unsigned kShift = 0b001;
constexpr unsigned kCtrl = 0b010;
constexpr unsigned kAlt = 0b100;
constexpr unsigned kNumpad0 = 0x60;
constexpr unsigned kNumpad7 = 0x67;
constexpr unsigned kOemPeriod = 0xBE;

int failures = 0;

void check(bool condition, const char *what)
{
    if (!condition)
    {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}

std::string read(const std::string &path)
{
    std::ifstream file(path, std::ios::binary);
    if (!file)
    {
        std::fprintf(stderr, "FAIL: cannot open %s\n", path.c_str());
        std::exit(EXIT_FAILURE);
    }
    std::ostringstream text;
    text << file.rdbuf();
    return text.str();
}
} // namespace

int main(int argc, char **argv)
{
    using Global::ForwardedSmartPunctuationContext;

    // 1. 键盘布局给了字符就以它为准：字母数字记下，别的字符让影子失效。
    check(ForwardedSmartPunctuationContext(L'3', L'3', kNone, false) == L'3', "数字键打出 3：数字前文");
    check(ForwardedSmartPunctuationContext(L'A', L'a', kNone, false) == L'a', "字母键打出 a：字母前文");
    check(ForwardedSmartPunctuationContext(L'A', L'A', kShift, false) == L'A', "Shift+字母打出 A：字母前文");
    check(ForwardedSmartPunctuationContext(kNumpad7, L'7', kNone, false) == L'7', "小键盘打出 7：数字前文");
    check(ForwardedSmartPunctuationContext(L'3', L'中', kNone, false) == L'\0', "非 ASCII 字符：失效");
    check(ForwardedSmartPunctuationContext(kOemPeriod, L'.', kNone, false) == L'\0', "标点：失效");
    // AZERTY 这类布局的数字行不按 Shift 打出的是符号，虚拟键码仍是数字。
    check(ForwardedSmartPunctuationContext(L'1', L'&', kNone, false) == L'\0', "数字键打出 &：失效");
    // AltGr（Ctrl+Alt）打出的字母数字是用户打的字。
    check(ForwardedSmartPunctuationContext(L'E', L'e', kCtrl | kAlt, false) == L'e', "AltGr 打出字母：字母前文");

    // 2. 回归一：Shift+数字键打出的是符号。英文模式下 TIP 拿不到字符时也不能按虚拟键码记成数字。
    check(ForwardedSmartPunctuationContext(L'5', L'%', kShift, false) == L'\0', "Shift+5 打出 %：失效");
    check(ForwardedSmartPunctuationContext(L'1', L'!', kShift, false) == L'\0', "Shift+1 打出 !：失效");
    check(ForwardedSmartPunctuationContext(L'5', L'\0', kShift, false) == L'\0', "Shift+5 没有字符值：不当成数字 5");
    check(ForwardedSmartPunctuationContext(L'1', L'\0', kShift, false) == L'\0', "Shift+1 没有字符值：不当成数字 1");

    // 3. 回归二：带 Ctrl、Alt、Win 的组合键是快捷键，不产生字符，可能粘贴、移动光标或切换标签页。
    check(ForwardedSmartPunctuationContext(L'1', L'\0', kCtrl, false) == L'\0', "Ctrl+1：失效");
    check(ForwardedSmartPunctuationContext(L'2', L'\0', kCtrl, false) == L'\0', "Ctrl+2：失效");
    check(ForwardedSmartPunctuationContext(L'V', L'\0', kCtrl, false) == L'\0', "Ctrl+V 没有字符值：失效");
    check(ForwardedSmartPunctuationContext(L'V', L'\x16', kCtrl, false) == L'\0', "Ctrl+V 的控制字符：失效");
    check(ForwardedSmartPunctuationContext(L'S', L'\0', kCtrl, false) == L'\0', "Ctrl+S：失效");
    check(ForwardedSmartPunctuationContext(L'C', L'\0', kCtrl, false) == L'\0', "Ctrl+C：失效");
    check(ForwardedSmartPunctuationContext(L'Z', L'\0', kCtrl | kShift, false) == L'\0', "Ctrl+Shift+Z：失效");
    check(ForwardedSmartPunctuationContext(L'D', L'\0', kAlt, false) == L'\0', "Alt+D：失效");
    check(ForwardedSmartPunctuationContext(L'1', L'\0', kCtrl | kAlt, false) == L'\0', "Ctrl+Alt+1：失效");
    check(ForwardedSmartPunctuationContext(L'1', L'\0', kNone, true) == L'\0', "Win+1：失效");
    check(ForwardedSmartPunctuationContext(L'E', L'\0', kNone, true) == L'\0', "Win+E：失效");
    check(ForwardedSmartPunctuationContext(kNumpad0, L'\0', kCtrl, false) == L'\0', "Ctrl+小键盘 0：失效");

    // 4. 拿不到字符值、也没有修饰键时，字母键、数字键和小键盘数字仍从虚拟键码补回（#11188）。
    check(ForwardedSmartPunctuationContext(L'3', L'\0', kNone, false) == L'3', "数字键没有字符值：补回 3");
    check(ForwardedSmartPunctuationContext(kNumpad7, L'\0', kNone, false) == L'7', "小键盘没有字符值：补回 7");
    check(ForwardedSmartPunctuationContext(L'A', L'\0', kNone, false) == L'A', "字母键没有字符值：补回字母");
    check(ForwardedSmartPunctuationContext(L'A', L'\0', kShift, false) == L'A', "Shift+字母没有字符值：仍是字母");
    check(ForwardedSmartPunctuationContext(kOemPeriod, L'\0', kNone, false) == L'\0', "其他键没有字符值：失效");

    // 5. 接线：影子按当时的修饰键和布局字符归类，不能退回只看虚拟键码。
    const std::string root = argc > 1 ? argv[1] : "platforms/windows/tsf";
    const std::string composition = read(root + "/Composition/Composition.cpp");
    check(composition.find("Global::ForwardedSmartPunctuationContext(code, character, modifiers, windowsKeyDown)") !=
              std::string::npos,
          "影子归类时带上修饰键和 Win 键");
    check(composition.find("Global::ForwardedSmartPunctuationContext(code, wch)") == std::string::npos,
          "不再只凭虚拟键码和 TIP 手里的字符归类");
    check(composition.find("character = LayoutCharacterOfForwardedKey(code);") != std::string::npos,
          "TIP 没拿到字符时先问键盘布局，再考虑虚拟键码");

    if (failures != 0)
    {
        std::fprintf(stderr, "%d failure(s)\n", failures);
        return EXIT_FAILURE;
    }
    std::puts("转发按键的智能标点前文：策略与接线回归通过");
    return EXIT_SUCCESS;
}
