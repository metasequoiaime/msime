#pragma once
#include "TypingSource.h"
#include <cstdint>
#include <map>
#include <string>

namespace msime::windows {
// 同一日内聚合来源，不保存按键或语言切换顺序。
struct TypingKeyBatch {
  std::wstring day;
  std::map<std::wstring, uint64_t> counts;
  std::map<TypingSource, std::map<std::wstring, uint64_t>> sources;
  void add(const wchar_t *key, TypingSource source) {
    ++counts[key];
    ++sources[source][key];
  }
};
} // namespace msime::windows
