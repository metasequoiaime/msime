#include "../../common/CharacterSetOutputPolicy.h"
#include <cassert>

using msime::windows::character_set::is_traditional;
using msime::windows::character_set::preference_key;
using msime::windows::character_set::stored_value;

int main() {
  assert(preference_key("quanpin") == std::string_view("traditional_chinese_output"));
  assert(preference_key("cantonese") == std::string_view("cantonese_simplified_output"));
  assert(is_traditional("quanpin", true, false));
  assert(!is_traditional("quanpin", false, true));
  assert(is_traditional("cantonese", false, false));
  assert(!is_traditional("cantonese", false, true));
  assert(stored_value("quanpin", true));
  assert(!stored_value("quanpin", false));
  assert(!stored_value("cantonese", true));
  assert(stored_value("cantonese", false));
}
