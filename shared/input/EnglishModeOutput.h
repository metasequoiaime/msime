#pragma once

#include <cstdint>
#include <string>
#include <string_view>

namespace msime::input {

// English mode never reaches the engine's punctuation policy (crates/engine/src/punctuation.rs), because the session is closed or ignored while the IME is off. The "always Chinese punctuation" lock, and a Ctrl+. or toolbar toggle made in English mode, still have to convert marks there, as they do on Windows with the IME closed (KeyEventSink.cpp's FUNCTION_PUNCTUATION and FUNCTION_DOUBLE_SINGLE_BYTE branches), so the hosts carry a forward copy of its contract (shared/contracts/punctuation/policy.h). This is deliberately not the Linux host's chinese_punctuation_mark: that table is the reverse Space rewrite, maps '/' to 、 and has no alternating quotes.
struct EnglishPunctuationState {
  bool double_quote_open = false;
  bool single_quote_open = false;
  bool corner_single_open = false;
  bool corner_double_open = false;
  int book_title_nesting = 0;
};

// 引号、方头括号样式与顿号键：宿主从当前 View 的 punctuation_marks 取值，样式与开关已经由 host-api 解析，这里不读偏好。
struct PunctuationMarks {
  bool corner_quotes = false;
  bool hollow_brackets = false;
  bool enumeration_comma_backslash = true;
  bool enumeration_comma_slash = false;
};

// View.punctuation_marks 转成本机结构；旧 View 缺少顿号字段时沿用「反斜杠开、斜杠关」。
inline PunctuationMarks punctuation_marks_from_view(std::string_view quotes,
                                                     std::string_view brackets,
                                                     bool backslash = true, bool slash = false) {
  return {quotes == "corner", brackets == "hollow", backslash, slash};
}

inline std::string english_mode_chinese_punctuation(
    char value, EnglishPunctuationState &state, const PunctuationMarks &marks) {
  switch (value) {
  case ',': return "，";
  case '.': return "。";
  case '?': return "？";
  case '!': return "！";
  case ';': return "；";
  case ':': return "：";
  case '(': return "（";
  case ')': return "）";
  case '[': return marks.hollow_brackets ? "〖" : "【";
  case ']': return marks.hollow_brackets ? "〗" : "】";
  case '\\': return marks.enumeration_comma_backslash ? "、" : "";
  case '/': return marks.enumeration_comma_slash ? "、" : "";
  case '`': return "·";
  case '$': return "￥";
  case '^': return "……";
  case '_': return "——";
  case '"':
    state.double_quote_open = !state.double_quote_open;
    if (marks.corner_quotes)
      return state.double_quote_open ? "『" : "』";
    return state.double_quote_open ? "“" : "”";
  case '\'':
    state.single_quote_open = !state.single_quote_open;
    if (marks.corner_quotes)
      return state.single_quote_open ? "「" : "」";
    return state.single_quote_open ? "‘" : "’";
  // 契约第 3 版：`{` `}` 的直角引号不吃样式，交替状态与 `'` `"` 分开保存。
  case '{':
    state.corner_single_open = !state.corner_single_open;
    return state.corner_single_open ? "「" : "」";
  case '}':
    state.corner_double_open = !state.corner_double_open;
    return state.corner_double_open ? "『" : "』";
  case '<':
    return state.book_title_nesting++ == 0 ? "《" : "〈";
  case '>':
    // An unmatched '>' leaves the depth at zero, as Engine's policy does.
    if (state.book_title_nesting > 0 && --state.book_title_nesting > 0)
      return "〉";
    return "》";
  default:
    return {};
  }
}

// The fullwidth form of a printable ASCII character as UTF-8: Space becomes U+3000 and 0x21-0x7e sit one block away at U+FF01-U+FF5E. Empty for anything outside printable ASCII.
inline std::string english_mode_fullwidth(char value) {
  const auto byte = static_cast<unsigned char>(value);
  if (byte == 0x20)
    return "\u3000";
  if (byte < 0x21 || byte > 0x7e)
    return {};
  const std::uint32_t code = byte + 0xfee0;
  std::string wide;
  wide.push_back(static_cast<char>(0xe0 | (code >> 12)));
  wide.push_back(static_cast<char>(0x80 | ((code >> 6) & 0x3f)));
  wide.push_back(static_cast<char>(0x80 | (code & 0x3f)));
  return wide;
}

// What an English-mode key press commits instead of passing through, empty when the key should go to the application unchanged. Windows order: Chinese punctuation first (only while the lock pins it on or a Ctrl+. in English mode turned it on), then fullwidth, which covers printable ASCII including Space (U+3000). Keypad keys never become Chinese punctuation, matching Chinese mode.
inline std::string english_mode_output(char32_t character, bool keypad,
                                       bool chinese_punctuation, bool fullwidth,
                                       EnglishPunctuationState &state,
                                       const PunctuationMarks &marks) {
  if (character < 0x20 || character > 0x7e)
    return {};
  const auto value = static_cast<char>(character);
  if (chinese_punctuation && !keypad) {
    // 固定表保留全角人民币；实际按键输出按当前宽度选择，不改其他中文标点。
    if (value == '$' && !fullwidth)
      return "¥";
    auto mark = english_mode_chinese_punctuation(value, state, marks);
    if (!mark.empty())
      return mark;
  }
  if (!fullwidth)
    return {};
  return english_mode_fullwidth(value);
}

} // namespace msime::input
