#include "ToolbarTooltips.h"
#include <iostream>
#include <stdexcept>
#include <string>

// 工具栏按钮的悬停提示与 macOS 工具栏的 toolTip 用同一套文字。
using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Toolbar tooltip check failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
} // namespace

int main() {
  try {
    // 方案名与 macOS 工具栏的 schemeTitle 一致。
    require(toolbar_scheme_title("quanpin", "xiaohe", "wubi86") == L"全拼");
    require(toolbar_scheme_title("shuangpin", "xiaohe", "wubi86") == L"小鹤双拼");
    require(toolbar_scheme_title("shuangpin", "ziranma", "wubi86") == L"自然码双拼");
    require(toolbar_scheme_title("shuangpin", "shoudao", "wubi86") == L"首道双拼");
    require(toolbar_scheme_title("shuangpin", "microsoft", "wubi86") == L"微软双拼");
    // 用户自己定义的键位不说成小鹤双拼。
    require(toolbar_scheme_title("shuangpin", "custom", "wubi86") == L"自定义双拼");
    // 不认识的键位按 macOS 的规范化落到小鹤。
    require(toolbar_scheme_title("shuangpin", "unknown", "wubi86") == L"小鹤双拼");
    require(toolbar_scheme_title("wubi", "xiaohe", "wubi86") == L"五笔 86");
    require(toolbar_scheme_title("wubi", "xiaohe", "wubi98") == L"五笔 98");
    require(toolbar_scheme_title("japanese", "", "") == L"日语");
    require(toolbar_scheme_title("korean", "", "") == L"韩语");
    require(toolbar_scheme_title("cantonese", "", "") == L"粤拼");
    require(toolbar_scheme_title("zhuyin", "", "") == L"注音");
    require(toolbar_scheme_title("vietnamese", "", "") == L"越南语");
    require(toolbar_scheme_title("tibetan", "", "") == L"藏文");
    require(toolbar_scheme_title("stroke", "", "") == L"笔画");
    require(toolbar_scheme_title("klingon", "", "").empty());

    const ToolbarLanguageState plain;
    const std::wstring product = L"水杉输入法";
    // 语言按钮：方案名 · 下一步的动作。临时英文和 Engine 的英文模式都说切回中文。
    require(toolbar_tooltip(kToolbarLanguage, true, plain, L"小鹤双拼", true, product) ==
            L"小鹤双拼 · 切换到英文输入");
    require(toolbar_tooltip(kToolbarLanguage, false, plain, L"全拼", true, product) ==
            L"全拼 · 切换到中文输入");
    ToolbarLanguageState dedicated;
    dedicated.dedicated_english = true;
    require(toolbar_tooltip(kToolbarLanguage, true, dedicated, L"全拼", true, product) ==
            L"全拼 · 切换到中文输入");
    require(toolbar_tooltip(kToolbarLanguage, std::nullopt, dedicated, L"", true, product) ==
            L"切换到中文输入");
    require(toolbar_tooltip(kToolbarLanguage, true, plain, L"", true, product) ==
            L"切换到英文输入");
    // 状态还不知道时不猜方向。
    require(toolbar_tooltip(kToolbarLanguage, std::nullopt, plain, L"全拼", true, product) ==
            L"全拼");
    require(toolbar_tooltip(kToolbarLanguage, std::nullopt, plain, L"", true, product) ==
            L"中英文切换");

    // 两态按钮说点了会切到哪一边。
    require(toolbar_tooltip(kToolbarPunctuation, true, plain, L"", true, product) ==
            L"切换到西文标点");
    require(toolbar_tooltip(kToolbarPunctuation, false, plain, L"", true, product) ==
            L"切换到中文标点");
    require(toolbar_tooltip(kToolbarFullwidth, true, plain, L"", true, product) ==
            L"切换到半角输入");
    require(toolbar_tooltip(kToolbarFullwidth, false, plain, L"", true, product) ==
            L"切换到全角输入");
    require(toolbar_tooltip(kToolbarCharacterSet, true, plain, L"", true, product) ==
            L"切换到简体输出");
    require(toolbar_tooltip(kToolbarCharacterSet, false, plain, L"", true, product) ==
            L"切换到繁体输出");
    for (const int button : {kToolbarPunctuation, kToolbarFullwidth, kToolbarCharacterSet})
      require(!toolbar_tooltip(button, std::nullopt, plain, L"", true, product).empty());

    // 其余按钮说点了打开什么；没有共享应用时表情和屏幕键盘打开的是系统面板。
    require(toolbar_tooltip(kToolbarInputScheme, std::nullopt, plain, L"", true, product) ==
            L"切换输入方案");
    require(toolbar_tooltip(kToolbarEmoji, std::nullopt, plain, L"", true, product) ==
            L"打开水杉表情面板");
    require(toolbar_tooltip(kToolbarEmoji, std::nullopt, plain, L"", false, product) ==
            L"打开系统表情面板");
    require(toolbar_tooltip(kToolbarScreenKeyboard, std::nullopt, plain, L"", true, product) ==
            L"打开水杉屏幕键盘");
    require(toolbar_tooltip(kToolbarScreenKeyboard, std::nullopt, plain, L"", false, product) ==
            L"打开系统屏幕键盘");
    require(toolbar_tooltip(kToolbarHandwriting, std::nullopt, plain, L"", true, product) ==
            L"打开水杉手写识别板");
    require(toolbar_tooltip(kToolbarVoice, std::nullopt, plain, L"", true, product) ==
            L"开始或结束语音输入");
    require(toolbar_tooltip(kToolbarSettings, std::nullopt, plain, L"", true, product) ==
            L"打开水杉输入法设置");
    require(toolbar_tooltip(kToolbarSettings, std::nullopt, plain, L"", true, L"水杉五笔") ==
            L"打开水杉五笔设置");
    require(toolbar_tooltip(kToolbarHide, std::nullopt, plain, L"", true, product) ==
            L"隐藏悬浮状态栏");
    require(toolbar_tooltip(99, std::nullopt, plain, L"", true, product).empty());
    std::cout << "Toolbar tooltips: labels match the macOS toolbar\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  }
}
