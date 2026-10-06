#include "../src/candidates/WubiProfileNames.h"

#include <cassert>
#include <string_view>

int main() {
  constexpr auto &profiles = msime::linux_host::kWubiProfileNames;
  static_assert(profiles.size() == 2);
  assert(std::string_view(profiles[0].value) == "wubi86");
  assert(std::string_view(profiles[0].label) == "86 五笔");
  assert(std::string_view(profiles[1].value) == "wubi98");
  assert(std::string_view(profiles[1].label) == "98 五笔");
  // 菜单名称跟随版本，缺省或不认识的版本退回方案名。
  assert(std::string_view(msime::linux_host::wubi_scheme_label("wubi86")) == "86 五笔");
  assert(std::string_view(msime::linux_host::wubi_scheme_label("wubi98")) == "98 五笔");
  assert(std::string_view(msime::linux_host::wubi_scheme_label("unknown")) == "五笔");
  return 0;
}
