#pragma once

#include <array>

namespace msime::linux_host {
struct ShuangpinProfileName {
  const char *value;
  const char *label;
};

inline constexpr std::array<ShuangpinProfileName, 5> kShuangpinProfileNames{{
    {"xiaohe", "小鹤"},
    {"ziranma", "自然码"},
    {"shoudao", "首道"},
    {"microsoft", "微软"},
    {"smart_abc", "智能 ABC"},
}};
} // namespace msime::linux_host
