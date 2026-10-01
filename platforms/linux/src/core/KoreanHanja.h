#pragma once

#include <nlohmann/json.hpp>

#include <cstdint>
#include <string>

namespace msime::linux_host {

// X keysyms, which IBus key values and Fcitx5 symbols both are. Hangul_Hanja is the Hanja key of a Korean keyboard; F9 is the second key ibus-hangul and fcitx5-hangul convert with by default, for keyboards without one. Ctrl+F9 stays the voice toggle: only the bare key converts.
inline constexpr uint32_t kKeysymHangulHanja = 0xff34;
inline constexpr uint32_t kKeysymF9 = 0xffc6;

inline bool korean_hanja_key(uint32_t keysym) {
  return keysym == kKeysymHangulHanja || keysym == kKeysymF9;
}

// The Korean rules hold for the view: scheme 4 outside the dedicated English mode and outside every local mode, which keep their own rules in that scheme (msime_client.h). Shared by the IBus and Fcitx5 hosts so both read "Korean" the same way.
inline bool korean_rules(const nlohmann::json &view) {
  if (!view.is_object()) return false;
  const auto scheme = view.find("scheme");
  if (scheme == view.end() || !scheme->is_number_integer() || scheme->get<int64_t>() != 4) return false;
  const auto english = view.find("dedicated_english");
  if (english != view.end() && english->is_boolean() && english->get<bool>()) return false;
  const auto mode = view.find("local_mode");
  return mode == view.end() || !mode->is_string() || mode->get_ref<const std::string &>() == "none";
}

// A Korean syllable is composing: editing_text holds its key letters and is non-empty exactly while it composes.
inline bool korean_composition(const nlohmann::json &view) {
  if (!korean_rules(view)) return false;
  const auto editing = view.find("editing_text");
  return editing != view.end() && editing->is_string() && !editing->get_ref<const std::string &>().empty();
}

// The Hanja list of the composing syllable is open. Under the Korean rules the Engine offers candidates only after MSIME_CONVERT_HANJA, so a non-empty list is that list (msime_client.h).
inline bool korean_hanja_list_open(const nlohmann::json &view) {
  if (!korean_composition(view)) return false;
  const auto candidates = view.find("candidates");
  return candidates != view.end() && candidates->is_array() && !candidates->empty();
}

// The marks among the candidate paging and word-to-character keys (- = [ ] , .). While a Hanja list is open they stay punctuation, as they are with no list: the Engine closes the list and writes the Hangul with the mark, so a mark typed after a syllable never turns a page or picks an edge character instead.
inline bool korean_hanja_punctuation_key(uint32_t keysym) {
  switch (keysym) {
  case '-': case '=': case '[': case ']': case ',': case '.': return true;
  default: return false;
  }
}

} // namespace msime::linux_host
