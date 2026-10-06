#pragma once

#include <array>
#include <string_view>

namespace msime::linux_host {
struct WubiProfileName {
  const char *value;
  const char *label;
};

// 共享偏好 `wubi_profile` 的取值与界面名称，顺序同 Engine 的编码（0 为 86，1 为 98）。
inline constexpr std::array<WubiProfileName, 2> kWubiProfileNames{{
    {"wubi86", "86 五笔"},
    {"wubi98", "98 五笔"},
}};

// 输入方案菜单和状态区里五笔一项的名称，跟随存储的码表版本；不认识的版本只写方案名，不猜一个错的。返回的是字面量，可直接交给 `ibus_text_new_from_static_string`。
inline const char *wubi_scheme_label(std::string_view profile) {
  for (const auto &name : kWubiProfileNames)
    if (profile == name.value) return name.label;
  return "五笔";
}
} // namespace msime::linux_host
