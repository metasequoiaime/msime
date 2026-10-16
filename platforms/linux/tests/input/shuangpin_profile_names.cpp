#include "../src/candidates/ShuangpinProfileNames.h"

#include <cassert>
#include <string_view>

int main() {
  constexpr auto &profiles = msime::linux_host::kShuangpinProfileNames;
  static_assert(profiles.size() == 5);
  assert(std::string_view(profiles[0].value) == "xiaohe");
  assert(std::string_view(profiles[0].label) == "小鹤");
  assert(std::string_view(profiles[1].value) == "ziranma");
  assert(std::string_view(profiles[1].label) == "自然码");
  assert(std::string_view(profiles[2].value) == "shoudao");
  assert(std::string_view(profiles[2].label) == "首道");
  assert(std::string_view(profiles[3].value) == "microsoft");
  assert(std::string_view(profiles[3].label) == "微软");
  assert(std::string_view(profiles[4].value) == "smart_abc");
  assert(std::string_view(profiles[4].label) == "智能 ABC");
  return 0;
}
