#pragma once
#include "AuxMessage.h"
#include "TypingSource.h"

namespace msime::windows {
inline constexpr std::wstring_view aux_typing_key_sources_verb =
    L"TypingKeySources";

inline std::wstring
aux_typing_key_sources_probe(const std::wstring &day,
                             std::string_view source = "unknown") {
  std::wstring message(aux_typing_key_sources_verb);
  message += L'|';
  message += day;
  message += L'|';
  for (const char unit : source)
    message += static_cast<wchar_t>(unit);
  message += L'|';
  return message;
}

// 来源随每片发送；协议探测只有空计数，不会重复记入总计。
inline std::vector<std::wstring> aux_typing_key_sources_messages(
    const std::wstring &day, TypingSource source,
    const std::map<std::wstring, uint64_t> &counts) {
  const auto prefix =
      aux_typing_key_sources_probe(day, typing_source_id(source));
  const size_t room = max_aux_message_bytes / sizeof(wchar_t);
  std::vector<std::wstring> result;
  auto message = prefix;
  for (const auto &[key, count] : counts) {
    if (!count)
      continue;
    const auto entry = key + L"=" + std::to_wstring(count);
    if (message.size() != prefix.size() &&
        message.size() + 1 + entry.size() > room) {
      result.push_back(std::move(message));
      message = prefix;
    }
    if (message.size() != prefix.size())
      message += L',';
    message += entry;
  }
  if (message.size() != prefix.size())
    result.push_back(std::move(message));
  return result;
}

inline std::optional<AuxTypingKeys>
parse_aux_typing_key_sources(const std::wstring &text) {
  const auto verb = aux_typing_key_sources_verb;
  if (text.size() < verb.size() + 14 ||
      text.compare(0, verb.size(), verb) != 0 || text[verb.size()] != L'|' ||
      text[verb.size() + 11] != L'|')
    return std::nullopt;
  const auto source_start = verb.size() + 12;
  const auto end = text.find(L'|', source_start);
  if (end == std::wstring::npos || end == source_start ||
      end - source_start > 32)
    return std::nullopt;
  std::string source;
  for (size_t at = source_start; at < end; ++at) {
    const auto unit = text[at];
    if (!((unit >= L'a' && unit <= L'z') || (unit >= L'A' && unit <= L'Z') ||
          unit == L'_'))
      return std::nullopt;
    source += static_cast<char>(unit);
  }
  bool known = false;
  for (int raw = 0; raw <= static_cast<int>(TypingSource::SmartAbc); ++raw)
    known = known || typing_source_id(static_cast<TypingSource>(raw)) == source;
  if (!known)
    return std::nullopt;
  auto keys = parse_aux_typing_keys(
      aux_typing_keys_probe(text.substr(verb.size() + 1, 10)) +
      text.substr(end + 1));
  if (keys)
    keys->source = text.substr(source_start, end - source_start);
  return keys;
}

inline std::optional<AuxTypingKeys>
parse_any_aux_typing_keys(const std::wstring &text) {
  if (auto keys = parse_aux_typing_key_sources(text))
    return keys;
  return parse_aux_typing_keys(text);
}

// 新 TIP 先只探测新格式；旧 Server
// 不认识时，仅探测旧格式。数据发送失败不能改格式重发。
template <class Send>
std::optional<bool> probe_typing_key_sources(const std::wstring &day,
                                             Send send) {
  if (send(aux_typing_key_sources_probe(day)))
    return true;
  if (send(aux_typing_keys_probe(day)))
    return false;
  return std::nullopt;
}
} // namespace msime::windows
