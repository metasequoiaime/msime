#pragma once
#include <algorithm>
#include <cstdint>
#include <map>
#include <optional>
#include <string>
#include <vector>

namespace msime::windows {
// The TSF DLL writes Aux messages as the raw UTF-16 code units of a string with
// no length prefix, no magic and no NUL terminator: the byte count is
// length * sizeof(wchar_t) (tsf/IPC/Ipc.cpp:1449-1456). Framing therefore comes
// from the message-mode pipe, one WriteFile per message.
inline constexpr size_t max_aux_message_bytes = 256;

// The langbar right-click rectangle, in screen coordinates, as TSF hands it to
// ITfLangBarItemButton::OnClick.
struct AuxLangbarRightClick {
  int32_t left = 0;
  int32_t top = 0;
  int32_t right = 0;
  int32_t bottom = 0;
};

// Where the tray card should be anchored: the icon's horizontal centre and its
// top edge, which is what TrayMenuWindow::show expects.
struct TrayMenuAnchor {
  int center_x = 0;
  int top = 0;
};

// Decode the wire bytes. This is untrusted, session-less input, so every
// rejection is explicit and nothing is assumed about termination.
inline std::optional<std::wstring> aux_text_from_bytes(const void *bytes,
                                                       size_t size) {
  if (!bytes || size == 0 || size > max_aux_message_bytes ||
      size % sizeof(wchar_t) != 0)
    return std::nullopt;
  const auto *units = static_cast<const wchar_t *>(bytes);
  std::wstring text(units, size / sizeof(wchar_t));
  for (wchar_t unit : text)
    // A control character cannot appear in any message the DLL sends, and an
    // embedded NUL would let a prefix parse as if it were the whole message.
    if (unit < 0x20 || unit == 0x7f)
      return std::nullopt;
  return text;
}

namespace detail {
// Parse one decimal field. Rejects empty, signs other than a leading '-',
// non-digits, and anything that would overflow int32.
inline std::optional<int32_t> aux_field(std::wstring_view field) {
  if (field.empty() || field.size() > 11)
    return std::nullopt;
  bool negative = false;
  size_t index = 0;
  if (field[0] == L'-') {
    negative = true;
    index = 1;
    if (field.size() == 1)
      return std::nullopt;
  }
  int64_t value = 0;
  for (; index < field.size(); ++index) {
    const wchar_t unit = field[index];
    if (unit < L'0' || unit > L'9')
      return std::nullopt;
    value = value * 10 + (unit - L'0');
    if (value > 2147483647LL)
      return std::nullopt;
  }
  return static_cast<int32_t>(negative ? -value : value);
}
// Parse one unsigned 64-bit decimal field: 1 to 20 digits, no sign, checked
// against overflow before every step.
inline std::optional<uint64_t> aux_u64_field(std::wstring_view field) {
  if (field.empty() || field.size() > 20)
    return std::nullopt;
  uint64_t value = 0;
  for (const wchar_t unit : field) {
    if (unit < L'0' || unit > L'9')
      return std::nullopt;
    const auto digit = static_cast<uint64_t>(unit - L'0');
    if (value > (UINT64_MAX - digit) / 10)
      return std::nullopt;
    value = value * 10 + digit;
  }
  return value;
}
} // namespace detail

// Largest rectangle accepted for a language-bar button. Anything wider is not a
// button and is more likely a malformed or hostile message.
inline constexpr int32_t max_aux_extent = 4096;

inline std::optional<AuxLangbarRightClick>
parse_aux_langbar_right_click(const std::wstring &text) {
  static constexpr std::wstring_view verb = L"LangbarRightClick";
  if (text.size() <= verb.size() || text.compare(0, verb.size(), verb) != 0 ||
      text[verb.size()] != L'|')
    return std::nullopt;
  std::vector<std::wstring_view> fields;
  fields.reserve(4);
  std::wstring_view rest(text);
  rest.remove_prefix(verb.size() + 1);
  while (true) {
    const auto separator = rest.find(L'|');
    if (separator == std::wstring_view::npos) {
      fields.push_back(rest);
      break;
    }
    fields.push_back(rest.substr(0, separator));
    rest.remove_prefix(separator + 1);
    // Four coordinates is the whole message; more means it is not this verb.
    if (fields.size() > 4)
      return std::nullopt;
  }
  if (fields.size() != 4)
    return std::nullopt;
  AuxLangbarRightClick click;
  int32_t *slots[] = {&click.left, &click.top, &click.right, &click.bottom};
  for (size_t index = 0; index < fields.size(); ++index) {
    const auto value = detail::aux_field(fields[index]);
    if (!value)
      return std::nullopt;
    *slots[index] = *value;
  }
  if (click.right <= click.left || click.bottom <= click.top)
    return std::nullopt;
  if (click.right - click.left > max_aux_extent ||
      click.bottom - click.top > max_aux_extent)
    return std::nullopt;
  return click;
}

// The activation edges the TSF DLL reports.
//
// These matter because "the mode view is empty" is not the same thing as "the
// IME is off": a temporary thread-focus suspension - Win+. opening the emoji
// panel, for instance - empties the view without deactivating anything. Gating
// the floating toolbar on the view alone made it blink away on every such
// suspension. The reference warns about exactly this and tracks the edges.
enum class AuxActivation { Activated, Deactivated };
inline std::optional<AuxActivation> parse_aux_activation(const std::wstring &text) {
  if (text == L"IMEActivation")
    return AuxActivation::Activated;
  if (text == L"IMEDeactivation")
    return AuxActivation::Deactivated;
  return std::nullopt;
}

// DictionaryQuiesce / DictionaryResume
//
// Dictionary maintenance runs in the settings process and needs the exclusive
// file lock every Engine session holds a share of. It asks the Server to drop
// its sessions, does the work, and asks for them back. Both are answered with
// the same "OK" the DLL's TerminalDeactivation uses, so the caller knows the
// lock is actually free before it tries to take it.
enum class AuxDictionaryMaintenance { Quiesce, Resume };
inline std::optional<AuxDictionaryMaintenance>
parse_aux_dictionary_maintenance(const std::wstring &text) {
  if (text == L"DictionaryQuiesce")
    return AuxDictionaryMaintenance::Quiesce;
  if (text == L"DictionaryResume")
    return AuxDictionaryMaintenance::Resume;
  return std::nullopt;
}

// TerminalDeactivation|<clientId>|<focusToken>
//
// The DLL falls back to this when its Main-pipe deactivate write fails, and
// then polls the Aux pipe for a literal "OK" for up to 150 ms. Leaving it
// unanswered blocks the sending TSF thread for that whole window. The client
// id is (pid << 32) | tid and the token a request id, so both are 64-bit.
struct AuxTerminalDeactivation {
  uint64_t client_id = 0;
  uint64_t focus_token = 0;
};
inline std::optional<AuxTerminalDeactivation>
parse_aux_terminal_deactivation(const std::wstring &text) {
  static constexpr std::wstring_view verb = L"TerminalDeactivation";
  if (text.size() <= verb.size() || text.compare(0, verb.size(), verb) != 0 ||
      text[verb.size()] != L'|')
    return std::nullopt;
  std::wstring_view rest(text);
  rest.remove_prefix(verb.size() + 1);
  const auto separator = rest.find(L'|');
  if (separator == std::wstring_view::npos)
    return std::nullopt;
  // A third '|' leaves a non-digit in the token field, which rejects it.
  const auto client = detail::aux_u64_field(rest.substr(0, separator));
  const auto token = detail::aux_u64_field(rest.substr(separator + 1));
  if (!client || !token)
    return std::nullopt;
  // Zero means the DLL never had a real client, so there is nothing to
  // deactivate.
  if (*client == 0 || *token == 0)
    return std::nullopt;
  return AuxTerminalDeactivation{*client, *token};
}

// TypingStatistics|E|<characters> or TypingStatistics|C|<characters>
//
// Characters the TSF DLL let through to the application, for the shared typing statistics: E for keys typed with the keyboard closed (English), C for digits and symbols passed through in Chinese mode. The DLL sorts each batch before it leaves the process - the Server only needs the multiset to classify it, and a sorted batch cannot be read back as the words that were typed. The DLL waits for "OK" and takes silence as "statistics are off", so the reply is written only once the batch has actually been accepted.
struct AuxTypingStatistics {
  bool english = false;
  std::wstring characters;
};
inline constexpr std::wstring_view aux_typing_statistics_verb = L"TypingStatistics";

inline std::vector<std::wstring>
aux_typing_statistics_messages(bool english, std::wstring characters) {
  std::sort(characters.begin(), characters.end());
  std::wstring prefix(aux_typing_statistics_verb);
  prefix += english ? L"|E|" : L"|C|";
  const size_t room = max_aux_message_bytes / sizeof(wchar_t) - prefix.size();
  std::vector<std::wstring> messages;
  const size_t message_count =
      characters.empty() ? 0 : (characters.size() - 1) / room + 1;
  messages.reserve(message_count);
  for (size_t offset = 0; offset < characters.size(); offset += room)
    messages.push_back(prefix + characters.substr(offset, room));
  return messages;
}

inline std::optional<AuxTypingStatistics>
parse_aux_typing_statistics(const std::wstring &text) {
  const auto verb = aux_typing_statistics_verb;
  // Verb, separator, tag, separator and at least one character.
  if (text.size() < verb.size() + 4 || text.compare(0, verb.size(), verb) != 0 ||
      text[verb.size()] != L'|' || text[verb.size() + 2] != L'|')
    return std::nullopt;
  const wchar_t tag = text[verb.size() + 1];
  if (tag != L'E' && tag != L'C')
    return std::nullopt;
  std::wstring characters = text.substr(verb.size() + 3);
  // aux_text_from_bytes has already rejected control characters; a surrogate here could only be half of a pair the DLL never sends, and would not survive the UTF-8 conversion.
  for (wchar_t unit : characters)
    if (unit >= 0xD800 && unit <= 0xDFFF)
      return std::nullopt;
  return AuxTypingStatistics{tag == L'E', std::move(characters)};
}

// TypingKeys|<YYYY-MM-DD>|<keyId>=<count>,<keyId>=<count>...
//
// Per-key press counts for the key heatmap, counted by the TSF DLL because only it sees the scan code (the Main-pipe packet carries a VK, which follows the layout). The day is the local day the presses happened on, fixed when they were counted, so a batch that crossed midnight arrives as two messages and never under the flush-time day. Entries are sorted by key id and carry counts only: no order, timing or text. An empty entry list is a probe: the DLL sends it before counting anything and starts buffering only once it is answered "OK", which the Server writes only while statistics are on.
struct AuxTypingKeys {
  std::wstring day;
  std::map<std::wstring, uint64_t> counts;
};
inline constexpr std::wstring_view aux_typing_keys_verb = L"TypingKeys";
// Same bound as the shared store's key ids: the canonical ids are short ASCII names.
inline constexpr size_t max_aux_key_id_length = 32;

inline std::wstring aux_typing_keys_probe(const std::wstring &day) {
  std::wstring message(aux_typing_keys_verb);
  message += L'|';
  message += day;
  message += L'|';
  return message;
}

// Split one day's counts into messages that each fit the pipe; an entry is never split across two messages. Empty counts produce no message.
inline std::vector<std::wstring>
aux_typing_keys_messages(const std::wstring &day,
                         const std::map<std::wstring, uint64_t> &counts) {
  const std::wstring prefix = aux_typing_keys_probe(day);
  const size_t room = max_aux_message_bytes / sizeof(wchar_t);
  std::vector<std::wstring> messages;
  messages.reserve(counts.size());
  std::wstring message = prefix;
  for (const auto &[key, count] : counts) {
    if (count == 0)
      continue;
    std::wstring entry = key;
    entry += L'=';
    entry += std::to_wstring(count);
    const bool first = message.size() == prefix.size();
    if (!first && message.size() + 1 + entry.size() > room) {
      messages.push_back(std::move(message));
      message = prefix;
    }
    if (message.size() != prefix.size())
      message += L',';
    message += entry;
  }
  if (message.size() != prefix.size())
    messages.push_back(std::move(message));
  return messages;
}

inline std::optional<AuxTypingKeys>
parse_aux_typing_keys(const std::wstring &text) {
  const auto verb = aux_typing_keys_verb;
  // Verb, separator, ten-character day, separator.
  if (text.size() < verb.size() + 12 || text.compare(0, verb.size(), verb) != 0 ||
      text[verb.size()] != L'|' || text[verb.size() + 11] != L'|')
    return std::nullopt;
  std::wstring day = text.substr(verb.size() + 1, 10);
  // Only the shape is checked here; the shared store decides whether it is a real date.
  for (size_t index = 0; index < day.size(); ++index) {
    const wchar_t unit = day[index];
    const bool separator = index == 4 || index == 7;
    if (separator ? unit != L'-' : (unit < L'0' || unit > L'9'))
      return std::nullopt;
  }
  AuxTypingKeys keys{std::move(day), {}};
  std::wstring_view rest(text);
  rest.remove_prefix(verb.size() + 12);
  if (rest.empty())
    return keys;
  while (true) {
    const auto comma = rest.find(L',');
    const std::wstring_view entry = rest.substr(0, comma);
    const auto equals = entry.find(L'=');
    if (equals == std::wstring_view::npos || equals == 0 ||
        equals > max_aux_key_id_length)
      return std::nullopt;
    const std::wstring_view key = entry.substr(0, equals);
    for (const wchar_t unit : key)
      if (!((unit >= L'A' && unit <= L'Z') || (unit >= L'a' && unit <= L'z') ||
            (unit >= L'0' && unit <= L'9')))
        return std::nullopt;
    const auto count = detail::aux_u64_field(entry.substr(equals + 1));
    if (!count || *count == 0)
      return std::nullopt;
    // A repeated id is not something the DLL writes; refusing it keeps one entry per key.
    if (!keys.counts.emplace(std::wstring(key), *count).second)
      return std::nullopt;
    if (comma == std::wstring_view::npos)
      break;
    rest.remove_prefix(comma + 1);
  }
  return keys;
}

// Anchor the card on the button. The centre is computed as left + width / 2
// rather than (left + right) / 2 so a rectangle far from the origin cannot
// overflow on the way.
inline TrayMenuAnchor tray_menu_anchor(const AuxLangbarRightClick &click) {
  return {click.left + (click.right - click.left) / 2, click.top};
}
} // namespace msime::windows
