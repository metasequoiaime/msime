#include "../src/core/SmartPunctuationChoice.h"

#include <cassert>

int main() {
  using namespace msime::linux_host;
  using Json = nlohmann::json;

  // 设置页关掉总开关时两半一起写成假；菜单再打开总开关，两半跟着打开，智能标点才真正生效。
  auto preferences = Json::parse(R"json({
    "smart_punctuation": false,
    "smart_punctuation_direct_digit": false,
    "smart_punctuation_direct_letter": false,
    "smart_punctuation_repeat": true,
    "chinese_punctuation": true
  })json");
  apply_smart_punctuation_choice(preferences, true);
  assert(preferences["smart_punctuation"] == true);
  assert(preferences["smart_punctuation_direct_digit"] == true);
  assert(preferences["smart_punctuation_direct_letter"] == true);
  // 其他标点设置不动。
  assert(preferences["smart_punctuation_repeat"] == true);
  assert(preferences["chinese_punctuation"] == true);

  // 关掉时三者一起关。
  apply_smart_punctuation_choice(preferences, false);
  assert(preferences["smart_punctuation"] == false);
  assert(preferences["smart_punctuation_direct_digit"] == false);
  assert(preferences["smart_punctuation_direct_letter"] == false);
  assert(preferences["smart_punctuation_repeat"] == true);

  // 文档里还没有这几个键时照样写全。
  auto empty = Json::object();
  apply_smart_punctuation_choice(empty, true);
  assert(empty.size() == 3 && empty["smart_punctuation_direct_letter"] == true);
  return 0;
}
