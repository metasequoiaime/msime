#include "AppInputModeRules.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("App input mode rules test failed at line " +
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
    // 文档里 Windows 的进程基名和 macOS 的 bundle id 在同一张表里；不合法的条目跳过，不让整张表失效。
    const auto preferences = nlohmann::json::parse(R"({
      "app_input_mode_rules": {
        "code.exe": "english",
        "WeChat.exe": "chinese",
        "com.apple.Terminal": "english",
        "broken.exe": "global",
        "number.exe": 1
      }
    })");
    const auto rules = ReadAppInputModeRules(preferences);
    require(rules.size() == 3);
    // 进程名按 ASCII 不分大小写比较，和 TIP 比较进程名的方式一致。
    require(AppInputModeRuleFor(rules, "Code.EXE") == false);
    require(AppInputModeRuleFor(rules, "wechat.exe") == true);
    require(!AppInputModeRuleFor(rules, "broken.exe"));
    require(!AppInputModeRuleFor(rules, "notepad.exe"));
    // 取不到进程名时不命中任何规则。
    require(!AppInputModeRuleFor(rules, ""));
    // 非 ASCII 字符不折叠。
    const auto unicode = ReadAppInputModeRules(nlohmann::json::parse(R"({"app_input_mode_rules": {"Ä.exe": "english"}})"));
    require(AppInputModeRuleFor(unicode, "Ä.exe") == false);
    require(!AppInputModeRuleFor(unicode, "ä.exe"));
    // 没有这个键、或者它不是对象，都当作没有规则。
    require(ReadAppInputModeRules(nlohmann::json::object()).empty());
    require(ReadAppInputModeRules(nlohmann::json::parse(R"({"app_input_mode_rules": []})")).empty());
    require(ReadAppInputModeRules(nlohmann::json::array()).empty());
    std::cout << "App input mode rules: process names pick their starting mode\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  } catch (...) {
    std::cerr << "App input mode rules test failed with an unknown error\n";
    return 1;
  }
}
