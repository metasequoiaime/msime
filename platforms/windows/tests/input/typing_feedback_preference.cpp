#include "TypingFeedbackPreference.h"

#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
void require(bool value, int line) {
  if (!value)
    throw std::runtime_error("typing feedback preference failed at line " + std::to_string(line));
}
#define REQUIRE(value) require((value), __LINE__)

nlohmann::json plugins(nlohmann::json value) { return nlohmann::json{{"plugins", std::move(value)}}; }
} // namespace

int main() {
  try {
    // 出厂设置什么都不开：交给应用的键不必报给 Server。
    REQUIRE(!typing_feedback_wanted(nlohmann::json::object()));
    REQUIRE(!typing_feedback_wanted(plugins({{"key_sound", {{"enabled", false}}}, {"effect_style", "off"}, {"combo_counter", false}})));
    // 按键音、任一绘制样式、特效包或连击计数，开一个就要。
    REQUIRE(typing_feedback_wanted(plugins({{"key_sound", {{"enabled", true}}}})));
    for (const char *style : {"flash", "sparks", "power_mode"})
      REQUIRE(typing_feedback_wanted(plugins({{"effect_style", style}})));
    REQUIRE(typing_feedback_wanted(plugins({{"effect_pack", "neon"}})));
    REQUIRE(typing_feedback_wanted(plugins({{"combo_counter", true}})));
    // 读不懂的值当作关着。
    REQUIRE(!typing_feedback_wanted(plugins({{"key_sound", true}})));
    REQUIRE(!typing_feedback_wanted(plugins({{"key_sound", {{"enabled", "yes"}}}})));
    REQUIRE(!typing_feedback_wanted(plugins({{"effect_style", "glitter"}})));
    REQUIRE(!typing_feedback_wanted(plugins({{"effect_pack", ""}})));
    REQUIRE(!typing_feedback_wanted(plugins({{"effect_pack", 3}})));
    REQUIRE(!typing_feedback_wanted(plugins(nlohmann::json::array())));
    REQUIRE(!typing_feedback_wanted(nlohmann::json::array()));

    std::cout << "Windows typing feedback preference checks passed\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
