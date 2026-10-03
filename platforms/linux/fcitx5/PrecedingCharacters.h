#pragma once

#include <fcitx-utils/utf8.h>
#include <algorithm>
#include <cstddef>
#include <optional>
#include <string>
#include <vector>

namespace msime::fcitx_host {

template <typename StringVector>
// `length` must be the result of lengthValidated(text); callers use this form
// when that scan has already been paid for by another surrounding-text check.
inline std::optional<StringVector> preceding_characters_with_validated_length(
    const std::string &text, std::size_t cursor, std::size_t count,
    std::size_t length) {
  if (cursor > length)
    return std::nullopt;
  const auto available = std::min<std::size_t>(count, cursor);
  StringVector characters;
  characters.reserve(available);
  auto start = fcitx::utf8::nextNChar(text.begin(), cursor - available);
  for (std::size_t index = 0; index < available; ++index) {
    const auto next = fcitx::utf8::nextChar(start);
    characters.emplace_back(start, next);
    start = next;
  }
  return characters;
}

template <typename StringVector>
inline std::optional<StringVector> preceding_characters_with_storage(
    const std::string &text, std::size_t cursor, std::size_t count) {
  const auto length = fcitx::utf8::lengthValidated(text);
  if (length == fcitx::utf8::INVALID_LENGTH)
    return std::nullopt;
  return preceding_characters_with_validated_length<StringVector>(
      text, cursor, count, length);
}

inline std::optional<std::vector<std::string>> preceding_characters(
    const std::string &text, std::size_t cursor, std::size_t count) {
  return preceding_characters_with_storage<std::vector<std::string>>(
      text, cursor, count);
}

}  // namespace msime::fcitx_host
