#pragma once

#include <windows.h>
#include <limits>
#include <optional>
#include <string>
#include <string_view>

namespace msime::settings::win7::control_text {

inline std::wstring wide(std::string_view value) {
  if (value.empty() || value.size() > static_cast<size_t>((std::numeric_limits<int>::max)())) return {};
  const int length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value.data(), static_cast<int>(value.size()), nullptr, 0);
  if (!length) return {};
  std::wstring result(static_cast<size_t>(length), L'\0');
  if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value.data(), static_cast<int>(value.size()), result.data(), length) != length)
    return {};
  return result;
}

inline std::optional<std::string> utf8(std::wstring_view value) {
  if (value.empty()) return std::string{};
  if (value.size() > static_cast<size_t>((std::numeric_limits<int>::max)())) return std::nullopt;
  const int length = static_cast<int>(value.size());
  const int bytes = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value.data(), length, nullptr, 0, nullptr, nullptr);
  if (!bytes) return std::nullopt;
  std::string result(static_cast<size_t>(bytes), '\0');
  if (WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value.data(), length, result.data(), bytes, nullptr, nullptr) != bytes)
    return std::nullopt;
  return result;
}

// 控件文本有明确上限，非法 UTF-16 不以替换字符悄悄写入共享配置。
inline std::optional<std::string> read(HWND control, int limit = 8192) {
  const int length = GetWindowTextLengthW(control);
  if (length < 0 || length > limit) return std::nullopt;
  std::wstring value(static_cast<size_t>(length) + 1, L'\0');
  const int copied = GetWindowTextW(control, value.data(), static_cast<int>(value.size()));
  if (copied != length) return std::nullopt;
  value.resize(static_cast<size_t>(copied));
  return utf8(value);
}

} // namespace msime::settings::win7::control_text
