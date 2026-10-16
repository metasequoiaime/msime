#pragma once
#include <cstddef>
#include <cstdint>

namespace msime::windows {
// 载荷是开关和两个单字节标点；零表示关闭或无效，整份配置一次原子发布。
constexpr bool wubi_candidate_punctuation(wchar_t value) {
  return (value >= L'!' && value <= L'/') || (value >= L':' && value <= L'@') ||
         (value >= L'[' && value <= L'`') || (value >= L'{' && value <= L'~');
}
inline std::uint32_t parse_wubi_candidate_keys(const wchar_t *data, std::size_t capacity) {
  if (!data || capacity < 4 || data[3] != L'\0' ||
      (data[0] != L'0' && data[0] != L'1') || !wubi_candidate_punctuation(data[1]) ||
      !wubi_candidate_punctuation(data[2]) || data[1] == data[2]) return 0;
  return data[0] == L'1' ? 0x10000u | (std::uint32_t(data[1]) << 8) | std::uint32_t(data[2]) : 0;
}
// 交给共享会话按字符处理；它持有当前页、候选重排和缺候选时的标点回退。
constexpr bool wubi_candidate_character(std::uint32_t keys, wchar_t mode, bool english, wchar_t value) {
  return mode == L'9' && !english && keys != 0 &&
         (value == wchar_t((keys >> 8) & 0xff) || value == wchar_t(keys & 0xff));
}
} // namespace msime::windows
