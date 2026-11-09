#include "VimMode.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Vim mode test failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)

// 名单命中、中文模式、无组字无修饰键的一次普通 Esc：唯一该切英文的形状，其余用例各改一项。
VimModeEscape listed_escape() {
  VimModeEscape key;
  key.virtualKey = 0x1B;
  key.listed = true;
  key.chineseMode = true;
  return key;
}
} // namespace
int main() {
  try {
    // 文档里 Windows 的进程基名和 macOS 的 bundle id 在同一张名单里；不是字符串或为空的项跳过，不让整张名单失效。
    const auto preferences = nlohmann::json::parse(R"({
      "vim_mode_apps": ["Code.exe", "com.apple.Terminal", "", 1, "gvim.exe"]
    })");
    const auto apps = ReadVimModeApps(preferences);
    require(apps.size() == 3);
    require(VimModeListContains(apps, "code.exe"));
    require(VimModeListContains(apps, "CODE.EXE"));
    require(VimModeListContains(apps, "gvim.exe"));
    require(!VimModeListContains(apps, "notepad.exe"));
    require(!VimModeListContains(apps, "code"));
    require(!VimModeListContains(apps, ""));

    // 只折叠 ASCII：非 ASCII 的程序名按原样比较。
    const auto unicode = ReadVimModeApps(nlohmann::json::parse(R"({"vim_mode_apps": ["Ä.exe"]})"));
    require(VimModeListContains(unicode, "Ä.exe"));
    require(!VimModeListContains(unicode, "ä.exe"));

    // 键缺失、类型不对、整份不是对象：都当作没有名单。
    require(ReadVimModeApps(nlohmann::json::object()).empty());
    require(ReadVimModeApps(nlohmann::json::parse(R"({"vim_mode_apps": {}})")).empty());
    require(ReadVimModeApps(nlohmann::json::parse(R"({"vim_mode_apps": "code.exe"})")).empty());
    require(ReadVimModeApps(nlohmann::json::array()).empty());

    require(VimModeEscapeSwitchesToEnglish(listed_escape()));

    auto other_key = listed_escape();
    other_key.virtualKey = 'H';
    require(!VimModeEscapeSwitchesToEnglish(other_key));

    // 组字开着时 Esc 被 TIP 吞下去取消组字，这一次不切。
    auto eaten = listed_escape();
    eaten.eaten = true;
    require(!VimModeEscapeSwitchesToEnglish(eaten));
    auto composing = listed_escape();
    composing.compositionActive = true;
    require(!VimModeEscapeSwitchesToEnglish(composing));

    auto repeat = listed_escape();
    repeat.autoRepeat = true;
    require(!VimModeEscapeSwitchesToEnglish(repeat));

    auto modified = listed_escape();
    modified.modifierHeld = true;
    require(!VimModeEscapeSwitchesToEnglish(modified));

    auto deferred = listed_escape();
    deferred.deferredKeysPending = true;
    require(!VimModeEscapeSwitchesToEnglish(deferred));

    auto unlisted = listed_escape();
    unlisted.listed = false;
    require(!VimModeEscapeSwitchesToEnglish(unlisted));

    // 已经是英文：OnTestKeyDown 和 OnKeyDown 都会判定同一次 Esc，第二次必须无事可做。
    auto english = listed_escape();
    english.chineseMode = false;
    require(!VimModeEscapeSwitchesToEnglish(english));
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}
