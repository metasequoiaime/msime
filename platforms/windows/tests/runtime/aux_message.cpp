#include "AuxMessage.h"
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>
#include <cstring>
#include <map>
#include <optional>

using namespace msime::windows;
namespace {
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("Aux message test failed at line " +
                             std::to_string(line));
}
// Reproduce exactly what the TSF DLL writes: the raw code units, no NUL.
std::vector<char> wire(const std::wstring &text) {
  std::vector<char> bytes(text.size() * sizeof(wchar_t));
  if (!text.empty())
    std::memcpy(bytes.data(), text.data(), bytes.size());
  return bytes;
}
std::optional<AuxLangbarRightClick> decode(const std::wstring &text) {
  const auto bytes = wire(text);
  const auto decoded = aux_text_from_bytes(bytes.data(), bytes.size());
  if (!decoded)
    return std::nullopt;
  return parse_aux_langbar_right_click(*decoded);
}
} // namespace
#define require(...) require_at((__VA_ARGS__), __LINE__)

int main() {
  try {
    // The message the language bar actually sends.
    const auto click = decode(L"LangbarRightClick|100|200|140|240");
    require(click.has_value());
    require(click->left == 100 && click->top == 200);
    require(click->right == 140 && click->bottom == 240);
    const auto anchor = tray_menu_anchor(*click);
    require(anchor.center_x == 120 && anchor.top == 200);

    // A secondary monitor left of or above the primary gives negative screen
    // coordinates, which are legitimate.
    const auto negative = decode(L"LangbarRightClick|-1920|-100|-1880|-60");
    require(negative.has_value());
    require(tray_menu_anchor(*negative).center_x == -1900);
    require(tray_menu_anchor(*negative).top == -100);

    // The centre must not be computed as (left + right) / 2.
    const auto far_right = decode(L"LangbarRightClick|2000000000|10|2000000040|50");
    require(far_right.has_value());
    require(tray_menu_anchor(*far_right).center_x == 2000000020);

    // The other three Aux verbs are not this message and must not parse.
    require(!decode(L"IMEActivation"));
    require(!decode(L"IMEDeactivation"));
    require(!decode(L"TerminalDeactivation|123|456"));

    // Malformed rectangles.
    require(!decode(L"LangbarRightClick|100|200|100|240"));
    require(!decode(L"LangbarRightClick|100|200|140|200"));
    require(!decode(L"LangbarRightClick|0|0|5000|40"));
    require(!decode(L"LangbarRightClick|0|0|40|5000"));

    // Malformed fields.
    require(!decode(L"LangbarRightClick"));
    require(!decode(L"LangbarRightClick|1|2|3"));
    require(!decode(L"LangbarRightClick|1|2|3|4|5"));
    require(!decode(L"LangbarRightClick|1|2|3|x"));
    require(!decode(L"LangbarRightClick|1|2|3|"));
    require(!decode(L"LangbarRightClick|1|2|3|+4"));
    require(!decode(L"LangbarRightClick|1|2|3|99999999999"));
    require(!decode(L"langbarrightclick|100|200|140|240"));
    require(!decode(L"LangbarRightClickX|100|200|140|240"));

    // Envelope rejections, before any parsing happens.
    require(!aux_text_from_bytes(nullptr, 8));
    const auto empty = wire(L"");
    require(!aux_text_from_bytes(empty.data(), 0));
    const auto valid = wire(L"LangbarRightClick|100|200|140|240");
    // An odd byte count is a truncated code unit.
    require(!aux_text_from_bytes(valid.data(), valid.size() - 1));
    // Longer than any real message.
    std::vector<char> oversized(max_aux_message_bytes + 2, 'a');
    require(!aux_text_from_bytes(oversized.data(), oversized.size()));
    // An embedded NUL would let a prefix parse as the whole message.
    auto embedded = wire(L"LangbarRightClick|1|2|3|4");
    embedded[2] = 0;
    require(!aux_text_from_bytes(embedded.data(), embedded.size()));
    const auto newline = wire(L"LangbarRightClick|1|2|3|4\n");
    require(!aux_text_from_bytes(newline.data(), newline.size()));

    // The activation edges. "Mode view is empty" is not "the IME is off": a
  // temporary focus suspension empties the view without deactivating anything,
  // and gating the toolbar on the view alone made it blink away every time.
  require(parse_aux_activation(L"IMEActivation") == AuxActivation::Activated);
  require(parse_aux_activation(L"IMEDeactivation") == AuxActivation::Deactivated);
  require(!parse_aux_activation(L"IMEActivation|1"));
  require(!parse_aux_activation(L"imeactivation"));
  require(!parse_aux_activation(L""));
  require(!parse_aux_activation(L"LangbarRightClick|1|2|3|4"));

  // The two maintenance verbs are exact words, not prefixes: a longer verb
  // that merely starts with one must not be mistaken for it, because the
  // answer grants exclusive access to the dictionaries.
  require(parse_aux_dictionary_maintenance(L"DictionaryQuiesce") ==
          AuxDictionaryMaintenance::Quiesce);
  require(parse_aux_dictionary_maintenance(L"DictionaryResume") ==
          AuxDictionaryMaintenance::Resume);
  require(!parse_aux_dictionary_maintenance(L"DictionaryQuiesceNow"));
  require(!parse_aux_dictionary_maintenance(L"DictionaryQuiesce|1"));
  require(!parse_aux_dictionary_maintenance(L"dictionaryquiesce"));
  require(!parse_aux_dictionary_maintenance(L"Dictionary"));
  require(!parse_aux_dictionary_maintenance(L""));
  // And they are not confused with the other verbs on the same pipe.
  require(!parse_aux_dictionary_maintenance(L"IMEActivation"));
  require(!parse_aux_dictionary_maintenance(L"RestartServer"));

  // TerminalDeactivation carries a client id and a focus token, both positive.
  const auto terminal = parse_aux_terminal_deactivation(L"TerminalDeactivation|7|42");
  require(terminal && terminal->client_id == 7 && terminal->focus_token == 42);
  // The DLL's real client id is (pid << 32) | tid and the token a 64-bit
  // request id, so both routinely exceed int32.
  const auto real = parse_aux_terminal_deactivation(
      L"TerminalDeactivation|5299989648942|18446744073709551615");
  require(real && real->client_id == ((1234ull << 32) | 5678) &&
          real->focus_token == 18446744073709551615ull);
  require(!parse_aux_terminal_deactivation(
      L"TerminalDeactivation|5299989648942|18446744073709551616"));
  require(!parse_aux_terminal_deactivation(
      L"TerminalDeactivation|99999999999999999999|42"));
  require(!parse_aux_terminal_deactivation(
      L"TerminalDeactivation|100000000000000000000|42"));
  require(!parse_aux_terminal_deactivation(L"TerminalDeactivation|+1|42"));
  require(!parse_aux_terminal_deactivation(L"TerminalDeactivation||42"));
  require(!parse_aux_terminal_deactivation(L"TerminalDeactivation|7|"));
  require(!parse_aux_terminal_deactivation(L"TerminalDeactivation|7"));
  require(!parse_aux_terminal_deactivation(L"TerminalDeactivation|7|42|9"));
  require(!parse_aux_terminal_deactivation(L"TerminalDeactivation|0|42"));
  require(!parse_aux_terminal_deactivation(L"TerminalDeactivation|7|0"));
  require(!parse_aux_terminal_deactivation(L"TerminalDeactivation|-1|42"));
  require(!parse_aux_terminal_deactivation(L"TerminalDeactivation|a|42"));
  require(!parse_aux_terminal_deactivation(L"TerminalDeactivation"));
  require(!parse_aux_terminal_deactivation(L"IMEActivation"));
  // The verbs never claim each other's messages.
  require(!parse_aux_langbar_right_click(L"TerminalDeactivation|7|42"));
  require(!parse_aux_terminal_deactivation(L"LangbarRightClick|1|2|3|4"));

  // Passthrough typing statistics: what the DLL builds is what the Server parses, sorted, and split to fit the pipe.
  {
    const auto messages = aux_typing_statistics_messages(true, L"hello");
    require(messages.size() == 1 && messages[0] == L"TypingStatistics|E|ehllo");
    const auto bytes = wire(messages[0]);
    const auto text = aux_text_from_bytes(bytes.data(), bytes.size());
    require(text.has_value());
    const auto parsed = parse_aux_typing_statistics(*text);
    require(parsed && parsed->english && parsed->characters == L"ehllo");
    const auto chinese = parse_aux_typing_statistics(
        aux_typing_statistics_messages(false, L"3,1")[0]);
    require(chinese && !chinese->english && chinese->characters == L",13");
    require(aux_typing_statistics_messages(true, L"").empty());
    const std::wstring many(300, L'x');
    const auto room = max_aux_message_bytes / sizeof(wchar_t) -
                      std::wstring_view(L"TypingStatistics|E|").size();
    size_t carried = 0;
    const auto many_messages = aux_typing_statistics_messages(true, many);
    require(many_messages.capacity() == (many.size() + room - 1) / room);
    for (const auto &message : many_messages) {
      require(message.size() * sizeof(wchar_t) <= max_aux_message_bytes);
      const auto piece = parse_aux_typing_statistics(message);
      require(piece.has_value());
      carried += piece->characters.size();
    }
    require(carried == many.size());
    require(!parse_aux_typing_statistics(L"TypingStatistics|E|"));
    require(!parse_aux_typing_statistics(L"TypingStatistics|X|a"));
    require(!parse_aux_typing_statistics(L"TypingStatistics|Ea"));
    require(!parse_aux_typing_statistics(L"TypingStatisticsX|E|a"));
    require(!parse_aux_typing_statistics(std::wstring(L"TypingStatistics|E|") + wchar_t(0xD83D)));
    require(!parse_aux_langbar_right_click(L"TypingStatistics|E|a"));
  }

  // Key heatmap counts: per-day, sorted by id, split on entry boundaries, and an empty list is the probe.
  {
    const std::map<std::wstring, uint64_t> counts{{L"Space", 2}, {L"KeyA", 3}};
    const auto messages = aux_typing_keys_messages(L"2026-10-01", counts);
    require(messages.size() == 1 &&
            messages[0] == L"TypingKeys|2026-10-01|KeyA=3,Space=2");
    const auto bytes = wire(messages[0]);
    const auto text = aux_text_from_bytes(bytes.data(), bytes.size());
    require(text.has_value());
    const auto parsed = parse_aux_typing_keys(*text);
    require(parsed && parsed->day == L"2026-10-01" && parsed->counts == counts);
    require(aux_typing_keys_messages(L"2026-10-01", {}).empty());

    const auto probe = parse_aux_typing_keys(aux_typing_keys_probe(L"2026-10-01"));
    require(probe && probe->day == L"2026-10-01" && probe->counts.empty());
    require(aux_typing_keys_probe(L"2026-10-01") == L"TypingKeys|2026-10-01|");

    // A batch wider than one message is carried whole, one entry never straddling two messages.
    std::map<std::wstring, uint64_t> wide;
    for (const auto *id : {L"KeyA", L"KeyB", L"KeyC", L"KeyD", L"KeyE", L"KeyF",
                           L"KeyG", L"KeyH", L"Digit1", L"Digit2", L"ArrowRight",
                           L"ArrowLeft", L"Backspace", L"NumpadSubtract",
                           L"BracketRight", L"ControlRight", L"IntlBackslash"})
      wide.emplace(id, 18446744073709551615ULL);
    const auto split = aux_typing_keys_messages(L"2026-10-01", wide);
    require(split.size() > 1);
    std::map<std::wstring, uint64_t> carried;
    for (const auto &message : split) {
      require(message.size() * sizeof(wchar_t) <= max_aux_message_bytes);
      const auto piece = parse_aux_typing_keys(message);
      require(piece && piece->day == L"2026-10-01" && !piece->counts.empty());
      carried.insert(piece->counts.begin(), piece->counts.end());
    }
    require(carried == wide);

    require(!parse_aux_typing_keys(L"TypingKeys|2026-10-01"));
    require(!parse_aux_typing_keys(L"TypingKeys|2026-1-01|KeyA=1"));
    require(!parse_aux_typing_keys(L"TypingKeys|2026/10/01|KeyA=1"));
    require(!parse_aux_typing_keys(L"TypingKeys|2026-10-01|KeyA"));
    require(!parse_aux_typing_keys(L"TypingKeys|2026-10-01|KeyA="));
    require(!parse_aux_typing_keys(L"TypingKeys|2026-10-01|=1"));
    require(!parse_aux_typing_keys(L"TypingKeys|2026-10-01|KeyA=0"));
    require(!parse_aux_typing_keys(L"TypingKeys|2026-10-01|KeyA=-1"));
    require(!parse_aux_typing_keys(L"TypingKeys|2026-10-01|KeyA=1,"));
    require(!parse_aux_typing_keys(L"TypingKeys|2026-10-01|KeyA=1,KeyA=2"));
    require(!parse_aux_typing_keys(L"TypingKeys|2026-10-01|Key A=1"));
    require(!parse_aux_typing_keys(L"TypingKeys|2026-10-01|KeyA=1|KeyB=1"));
    require(!parse_aux_typing_keys(L"TypingKeys|2026-10-01|KeyA=18446744073709551616"));
    require(!parse_aux_typing_keys(L"TypingKeys|2026-10-01|" + std::wstring(33, L'K') + L"=1"));
    require(parse_aux_typing_keys(L"TypingKeys|2026-10-01|" + std::wstring(32, L'K') + L"=1").has_value());
    require(!parse_aux_typing_keys(L"TypingKeysX|2026-10-01|KeyA=1"));
    require(!parse_aux_typing_statistics(L"TypingKeys|2026-10-01|KeyA=1"));
    require(!parse_aux_typing_keys(L"TypingStatistics|E|a"));
  }

  std::cout << "Aux message: langbar rectangle parsed, malformed rejected\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << "\n";
    return 1;
  }
}
