#include "../../common/AuxTypingKeySources.h"
#include <cassert>
#include <iostream>

using namespace msime::windows;

int main() {
  const std::wstring day = L"2026-11-01";
  const auto probe =
      parse_any_aux_typing_keys(aux_typing_key_sources_probe(day));
  assert(probe && probe->source == L"unknown" && probe->counts.empty());
  for (int raw = 0; raw <= static_cast<int>(TypingSource::SmartAbc); ++raw) {
    const auto source = static_cast<TypingSource>(raw);
    const auto accepted = parse_aux_typing_key_sources(
        aux_typing_key_sources_probe(day, typing_source_id(source)));
    assert(accepted && !accepted->source.empty());
  }
  const auto legacy = parse_any_aux_typing_keys(aux_typing_keys_probe(day));
  assert(legacy && legacy->source.empty());
  assert(!parse_aux_typing_keys(aux_typing_key_sources_probe(day)));
  std::map<std::wstring, uint64_t> counts;
  for (int at = 0; at < 60; ++at)
    counts[L"Key" + std::to_wstring(at)] = static_cast<uint64_t>(at + 1);
  const auto messages =
      aux_typing_key_sources_messages(day, TypingSource::Japanese, counts);
  assert(messages.size() > 1);
  std::map<std::wstring, uint64_t> restored;
  for (const auto &message : messages) {
    assert(message.size() * sizeof(wchar_t) <= max_aux_message_bytes);
    const auto piece = parse_any_aux_typing_keys(message);
    assert(piece && piece->day == day && piece->source == L"japanese");
    for (const auto &[key, count] : piece->counts)
      restored[key] += count;
  }
  assert(restored == counts);
  assert(
      aux_typing_key_sources_messages(day, TypingSource::English, {}).empty());
  for (const auto &bad : {L"TypingKeySources|2026-11-01||KeyA=1",
                          L"TypingKeySources|2026-11-01|fake|KeyA=1",
                          L"TypingKeySources|2026-11-01|english|KeyA=0",
                          L"TypingKeySources|2026-11-01|english|KeyA=1,KeyA=2",
                          L"TypingKeySources|2026-1-01|english|KeyA=1"})
    assert(!parse_any_aux_typing_keys(bad));
  unsigned calls = 0;
  const auto modern =
      probe_typing_key_sources(day, [&](const std::wstring &message) {
        ++calls;
        return parse_aux_typing_key_sources(message).has_value();
      });
  assert(modern && *modern && calls == 1);
  calls = 0;
  const auto fallback =
      probe_typing_key_sources(day, [&](const std::wstring &message) {
        ++calls;
        return parse_aux_typing_keys(message).has_value();
      });
  assert(fallback && !*fallback && calls == 2);
  assert(!probe_typing_key_sources(day,
                                   [](const std::wstring &) { return false; }));
  std::cout << "Aux typing key source protocol passed\n";
}
