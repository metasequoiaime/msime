#pragma once

#include <string>
#include <string_view>

namespace msime::windows {

// Convert simplified Chinese output at the Windows host boundary. The Engine
// keeps its canonical text unchanged; malformed or unsupported text is kept.
std::string simplified_to_traditional(std::string_view text,
                                      bool traditional_output);

// 来源 18 已由共享运行时转换中文片段，不能再次转换其中的日文汉字。
inline std::string candidate_output(std::string_view text, bool traditional_output,
                                    unsigned source) {
  return simplified_to_traditional(text, traditional_output && source != 18);
}

} // namespace msime::windows
