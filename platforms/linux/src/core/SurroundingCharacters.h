#pragma once

#include <glib.h>
#include <cstddef>
#include <optional>
#include <string>
#include <vector>

namespace msime::linux_host {

template <typename StringVector>
inline std::optional<StringVector> preceding_characters_from_byte_offset_with_storage(
    const std::string &text, std::size_t offset, std::size_t count) {
  if (offset > text.size()) return std::nullopt;
  const auto *begin = text.c_str();
  StringVector characters;
  characters.reserve(count);
  while (characters.size() < count && offset > 0) {
    const auto *end = begin + offset;
    const auto *start = g_utf8_find_prev_char(begin, end);
    if (!start) return std::nullopt;
    characters.emplace(characters.begin(), start,
                       static_cast<std::size_t>(end - start));
    offset = static_cast<std::size_t>(start - begin);
  }
  return characters;
}

inline std::optional<std::vector<std::string>> preceding_characters_from_byte_offset(
    const std::string &text, std::size_t offset, std::size_t count) {
  return preceding_characters_from_byte_offset_with_storage<std::vector<std::string>>(
      text, offset, count);
}

}  // namespace msime::linux_host
