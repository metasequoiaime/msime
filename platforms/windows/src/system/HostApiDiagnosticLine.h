#pragma once
#include <cstddef>
#include <string>
#include <string_view>

namespace msime::windows {
// host-api 经 msime_client_set_diagnostic_sink 送来的一行是 "<category>: <detail>"（msime_client.h）：冒号前是固定的英文类别，冒号后才有包名、文件和错误原文。server.log 会经 msime-mcp 交给 AI 助手读，只承诺事件名和错误类别，所以这里只留类别，写成 "host_api: <category>"；没有冒号的行不符合约定，一个字都不信，写成 "host_api: uncategorized"。与 macOS 的 msime_macos_diagnostic_host_line 同一规则，类别同样截到整条不超过 192 字节。
inline constexpr std::size_t host_api_diagnostic_line_limit = 192;

inline std::string host_api_diagnostic_line(const char *line) {
  constexpr std::string_view prefix = "host_api: ";
  constexpr std::string_view uncategorized = "uncategorized";
  const std::string_view text = line ? std::string_view(line) : std::string_view();
  const auto colon = text.find(':');
  const auto category = colon == std::string_view::npos ? uncategorized : text.substr(0, colon);
  std::string result(prefix);
  result.append(category.substr(0, host_api_diagnostic_line_limit - prefix.size()));
  return result;
}
} // namespace msime::windows
