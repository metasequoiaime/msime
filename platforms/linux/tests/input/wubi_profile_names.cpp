#include "../src/candidates/WubiProfileNames.h"

#include <cassert>
#include <string_view>

int main() {
  constexpr auto &profiles = msime::linux_host::kWubiProfileNames;
  static_assert(profiles.size() == 3);
  assert(std::string_view(profiles[0].value) == "wubi86");
  assert(std::string_view(profiles[0].label) == "五笔 86");
  assert(std::string_view(profiles[1].value) == "wubi98");
  assert(std::string_view(profiles[1].label) == "五笔 98");
  // 新世纪追加在末尾，下标 2 就是 Engine 的编码 2。
  assert(std::string_view(profiles[2].value) == "wubixsj");
  assert(std::string_view(profiles[2].label) == "五笔 新世纪");
  // 菜单名称跟随版本，缺省或不认识的版本退回方案名。
  assert(std::string_view(msime::linux_host::wubi_scheme_label("wubi86")) == "五笔 86");
  assert(std::string_view(msime::linux_host::wubi_scheme_label("wubi98")) == "五笔 98");
  assert(std::string_view(msime::linux_host::wubi_scheme_label("wubixsj")) == "五笔 新世纪");
  assert(std::string_view(msime::linux_host::wubi_scheme_label("unknown")) == "五笔");
  assert(std::string_view(msime::linux_host::wubi_scheme_label("")) == "五笔");
  // 界面名称不出现商标。
  for (const auto &name : profiles) {
    assert(std::string_view(name.label).find("王码") == std::string_view::npos);
    assert(std::string_view(name.label).find("五笔字型") == std::string_view::npos);
  }
  return 0;
}
