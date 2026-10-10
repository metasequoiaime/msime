#pragma once
#include <string>
#include <string_view>
#include <vector>

namespace msime::windows {
// 候选释义每种目标语言占一行，和 macOS 一样。共享会话每个候选只存一条不含控制字符的释义，msime_client_apply_translations 遇到 "\n" 会把整批释义拒收，所以交给会话时行与行之间用 U+2028 LINE SEPARATOR（它不是控制字符），读回时按它分行。
inline constexpr std::string_view translation_line_separator = "\xE2\x80\xA8";

// 一种语言的释义：来源自带的 U+2028、U+2029 折成空格。词典、自定义接口都可能原样带着它们，不折掉的话读回时会多出一行，第二种语言的释义就被挤到第三行。
inline std::string flatten_translation_line(std::string_view line) {
  constexpr std::string_view paragraph = "\xE2\x80\xA9";
  std::string result;
  result.reserve(line.size());
  for (size_t index = 0; index < line.size();) {
    if (line.compare(index, translation_line_separator.size(),
                     translation_line_separator) == 0 ||
        line.compare(index, paragraph.size(), paragraph) == 0) {
      result.push_back(' ');
      index += 3;
    } else {
      result.push_back(line[index++]);
    }
  }
  return result;
}

// 按目标语言顺序把各行拼成交给会话的一条释义。某种语言没有释义时留一个空行，让第 N 行始终是第 N 种目标语言；末尾的空行去掉，全空时返回空。宿主 ABI 把这个字段限制在 4096 字节，放不下的行从后往前整行丢掉，绝不截断半个 UTF-8 字符。
inline std::string join_translation_lines(const std::vector<std::string> &lines) {
  constexpr size_t maximum = 4096;
  std::vector<std::string> flat;
  flat.reserve(lines.size());
  for (const auto &line : lines)
    flat.push_back(flatten_translation_line(line));
  while (!flat.empty() && flat.back().empty())
    flat.pop_back();
  std::string joined;
  for (size_t index = 0; index < flat.size(); ++index) {
    const size_t extra =
        (index ? translation_line_separator.size() : 0) + flat[index].size();
    if (joined.size() + extra > maximum)
      break;
    if (index)
      joined.append(translation_line_separator);
    joined.append(flat[index]);
  }
  while (joined.size() >= translation_line_separator.size() &&
         joined.compare(joined.size() - translation_line_separator.size(),
                        translation_line_separator.size(),
                        translation_line_separator) == 0)
    joined.resize(joined.size() - translation_line_separator.size());
  return joined;
}

// 会话里的一条释义拆回各语言的行；没有分隔符时就是一行。
inline std::vector<std::string> translation_lines(std::string_view translation) {
  std::vector<std::string> lines;
  size_t start = 0;
  for (;;) {
    const auto cut = translation.find(translation_line_separator, start);
    if (cut == std::string_view::npos) {
      lines.emplace_back(translation.substr(start));
      return lines;
    }
    lines.emplace_back(translation.substr(start, cut - start));
    start = cut + translation_line_separator.size();
  }
}

// Ctrl+Enter 上屏用的释义：第一条非空的行，也就是排在最前、有释义的那种目标语言。多行一起上屏会把分行符写进文档。
inline std::string primary_translation_line(std::string_view translation) {
  for (auto &line : translation_lines(translation))
    if (!line.empty())
      return line;
  return {};
}
} // namespace msime::windows
