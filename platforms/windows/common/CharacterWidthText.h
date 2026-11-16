#pragma once
#include "../../../shared/input/EnglishModeOutput.h"
#include <string>
#include <string_view>

namespace msime::windows {
// 仅供宿主明确拥有的字面文字使用；网址、插件和候选正文仍按共享会话返回值输出。
inline std::string fullwidth_ascii_text(std::string_view text) {
  std::string result;
  for (const char byte : text) {
    const auto wide = msime::input::english_mode_fullwidth(byte);
    if (wide.empty()) result.push_back(byte);
    else result += wide;
  }
  return result;
}

// 提交前已核对原文；提交后的核对允许共享会话只改变 ASCII 宽度。
inline bool raw_commit_width_matches(std::string_view observed, std::string_view committed,
                                     bool fullwidth) {
  return observed == committed || (fullwidth && fullwidth_ascii_text(observed) == committed);
}
}
