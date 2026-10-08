#pragma once
#include <algorithm>
#include <cstddef>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

// 「候选窗口」页游戏组两张程序列表（game_compatibility.overlay_processes、excluded_processes）的规范化与校验，规则与 crates/client-core/src/preferences.rs 的 GameCompatibilityPreferences::validate 一致。写入前在这里先查一遍，是为了就地说出具体原因：SetStrings 只做区分大小写的去重，Save 被拒时只能显示一句笼统的提示。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings {

// 两张表合计的条数上限，和每个程序名的字符数上限（按 Unicode 字符计，与 Rust 的 chars().count() 相同）。
inline constexpr std::size_t game_process_limit = 32;
inline constexpr std::size_t game_process_name_chars = 64;

enum class GameProcessError { Empty, TooLong, InvalidCharacter, NotExe, Duplicate, TooMany };

// 只把 ASCII 字母转成小写，非 ASCII 字符原样保留。
inline std::wstring ascii_lowercase(std::wstring_view value) {
  std::wstring result(value);
  for (auto &c : result)
    if (c >= L'A' && c <= L'Z')
      c = static_cast<wchar_t>(c - L'A' + L'a');
  return result;
}

// 用户在输入框里填的程序名：去掉首尾空白（含不换行空格和全角空格），再转 ASCII 小写。
inline std::wstring normalize_game_process(std::wstring_view raw) {
  constexpr std::wstring_view blanks = L" \t\r\n\v\f\u00A0\u3000";
  const auto first = raw.find_first_not_of(blanks);
  if (first == std::wstring_view::npos)
    return {};
  const auto last = raw.find_last_not_of(blanks);
  return ascii_lowercase(raw.substr(first, last - first + 1));
}

// 规范化后的程序名能否加进任一张表；overlay 和 excluded 是两张表现有的条目。重复按 ASCII 不分大小写比较，两张表之间也算重复。
inline std::optional<GameProcessError>
validate_game_process(std::wstring const &name,
                      std::vector<std::wstring> const &overlay,
                      std::vector<std::wstring> const &excluded) {
  if (name.empty())
    return GameProcessError::Empty;
  // UTF-16 的代理对只算一个字符，所以不数低位代理。
  const auto characters = static_cast<std::size_t>(
      std::count_if(name.begin(), name.end(),
                    [](wchar_t c) { return c < 0xDC00 || c > 0xDFFF; }));
  if (characters > game_process_name_chars)
    return GameProcessError::TooLong;
  // 与 Rust 的 char::is_control 相同（Cc 类：U+0000–U+001F、U+007F–U+009F），再加上 Windows 文件名不允许的字符。
  constexpr std::wstring_view reserved = L"\\/:*?\"<>|";
  for (const wchar_t c : name)
    if (c < 0x20 || (c >= 0x7F && c <= 0x9F) ||
        reserved.find(c) != std::wstring_view::npos)
      return GameProcessError::InvalidCharacter;
  if (name.size() < 4 ||
      ascii_lowercase(std::wstring_view(name).substr(name.size() - 4)) != L".exe")
    return GameProcessError::NotExe;
  const auto folded = ascii_lowercase(name);
  for (auto const *list : {&overlay, &excluded})
    for (auto const &existing : *list)
      if (ascii_lowercase(existing) == folded)
        return GameProcessError::Duplicate;
  if (overlay.size() + excluded.size() >= game_process_limit)
    return GameProcessError::TooMany;
  return std::nullopt;
}

} // namespace msime::settings
