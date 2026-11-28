#pragma once
#include <cstddef>
#include <cstdint>
#include <string>
#include <string_view>

#include "../../../shared/contracts/punctuation/policy.h"

// 按键保留英文（标点契约第 7 版 `preserveEnglish` 段，共享偏好 `preserve_english_punctuation`，#11890）。Server 从偏好算出集合，经 Worker 帧 PreserveEnglishPunctuationChanged 推给 TIP；TIP 的标点表对集合里的键出这个键本身，不推进引号交替和书名号嵌套，其余键照旧。位 i 对应契约 `preserve_english[i]`。
namespace msime::windows {

inline int preserved_english_index(wchar_t key) {
  const auto &keys = metasequoia::punctuation_contract::preserve_english;
  for (std::size_t i = 0; i < keys.size(); ++i)
    if (static_cast<wchar_t>(static_cast<unsigned char>(keys[i])) == key) return static_cast<int>(i);
  return -1;
}

// 偏好里的键（每项一个 ASCII 字符）算成位集，不认识的键忽略；偏好已由 client-core 校验过，这里只防坏文档。
inline std::uint32_t preserved_english_mask(std::string_view keys) {
  std::uint32_t mask = 0;
  for (const char key : keys) {
    const int index = preserved_english_index(static_cast<wchar_t>(static_cast<unsigned char>(key)));
    if (index >= 0) mask |= 1u << index;
  }
  return mask;
}

// 帧载荷 "p" 后跟集合里的键，按契约顺序；空集合就是 "p"。
inline std::wstring preserved_english_payload(std::uint32_t mask) {
  std::wstring payload(1, L'p');
  const auto &keys = metasequoia::punctuation_contract::preserve_english;
  for (std::size_t i = 0; i < keys.size(); ++i)
    if (mask & (1u << i)) payload.push_back(static_cast<wchar_t>(static_cast<unsigned char>(keys[i])));
  return payload;
}

// 解析帧载荷；格式不对（不以 "p" 开头、有契约外的键、重复、没有结束符）时返回 false，TIP 丢掉这一帧。
inline bool parse_preserved_english(const wchar_t *data, std::size_t capacity, std::uint32_t &mask) {
  if (!data || capacity == 0 || data[0] != L'p') return false;
  std::uint32_t parsed = 0;
  for (std::size_t i = 1; i < capacity; ++i) {
    if (data[i] == L'\0') {
      mask = parsed;
      return true;
    }
    const int index = preserved_english_index(data[i]);
    if (index < 0 || (parsed & (1u << index))) return false;
    parsed |= 1u << index;
  }
  return false;
}

// 保留英文的键在中文标点下出的字：键本身（静态字符串）；不在集合里时为 nullptr，照常查标点表。
inline const wchar_t *preserved_english_text(wchar_t key, std::uint32_t mask) {
  static constexpr wchar_t texts[][2] = {
      {L',', 0}, {L'.', 0}, {L'?', 0}, {L'!', 0}, {L';', 0}, {L':', 0}, {L'(', 0},
      {L')', 0}, {L'[', 0}, {L']', 0}, {L'\\', 0}, {L'`', 0}, {L'$', 0}, {L'^', 0},
      {L'_', 0}, {L'{', 0}, {L'}', 0}, {L'"', 0}, {L'\'', 0}, {L'<', 0}, {L'>', 0}};
  static_assert(sizeof(texts) / sizeof(texts[0]) == metasequoia::punctuation_contract::preserve_english.size());
  const int index = preserved_english_index(key);
  if (index < 0 || !(mask & (1u << index))) return nullptr;
  return texts[index];
}

} // namespace msime::windows
