#pragma once

#include "InputSchemes.h"

#include <nlohmann/json.hpp>

#include <string>

namespace msime::linux_host {

// The Engine lists in View.spelling_symbols the non-letter characters it takes as input in its current state: the operators and digits of the expression mode, the digits of the unicode mode, with nothing composed the keys that open a mode ("/" and "@"), and the keys a scheme spells with (the Zhuyin keyboard's digits and marks, Cantonese's syllable apostrophe, Vietnamese's VNI tone digits). Shared by the IBus and Fcitx5 hosts, so neither keeps its own list of which mode spells with what; a host that did would drift the first time the Engine adds a mode.
inline bool spelling_symbol(const nlohmann::json &view, char32_t character) {
  if (!view.is_object() || character < 0x21 || character > 0x7e) return false;
  const auto symbols = view.find("spelling_symbols");
  return symbols != view.end() && symbols->is_string() &&
         symbols->get_ref<const std::string &>().find(static_cast<char>(character)) !=
             std::string::npos;
}

// A character a local mode is spelling with, which the host sends to the Engine as a character before any of its own key bindings can claim it: in the expression mode "-" and "=" are not page keys, "." is not smart punctuation, "(" is not an auto-paired bracket and a digit is not a candidate shortcut. The idle mode-entry keys are left to the ordinary punctuation route, where the shared runtime decides whether a mark opens a mode (it does not after a held phrase, or when the host keeps the mark ASCII beside a digit).
inline bool local_mode_spelling(const nlohmann::json &view, char32_t character) {
  if (!view.is_object()) return false;
  const auto mode = view.find("local_mode");
  return mode != view.end() && mode->is_string() && *mode != "none" &&
         spelling_symbol(view, character);
}

// A character the Engine spells with that the host sends to the Engine before any of its own key bindings can claim it: every symbol of a local mode (see local_mode_spelling), and every symbol listed in a scheme that opens no local modes, where the list is that scheme's own keys. A Zhuyin digit or "," is then a bopomofo key rather than a candidate shortcut, a page key or Chinese punctuation, and a Cantonese apostrophe separates syllables. Quanpin's and Shuangpin's idle mode-entry keys stay on the punctuation route, as local_mode_spelling explains.
inline bool engine_spelling(const nlohmann::json &view, char32_t character) {
  if (local_mode_spelling(view, character)) return true;
  const int scheme = scheme_rules(view);
  return scheme >= 0 && !scheme::OpensLocalModes(scheme) && spelling_symbol(view, character);
}

// Space is spelling input, not a commit key: the Zhuyin keyboard's first tone while a syllable is composing. spelling_symbol leaves out the space itself, since in every other state it is the host's own key.
inline bool spelling_space(const nlohmann::json &view) {
  if (!view.is_object()) return false;
  const auto symbols = view.find("spelling_symbols");
  return symbols != view.end() && symbols->is_string() &&
         symbols->get_ref<const std::string &>().find(' ') != std::string::npos;
}

// Whether the bare number row is input rather than a candidate shortcut. The candidates are then picked with Shift and the number row, as Windows does in its Unicode mode, except where Shift and a digit key type a symbol the mode also spells with (the expression mode's % ^ * ( ) on a US layout). "1" is the digit asked about, because an open Zhuyin list still spells with "0" (a bopomofo key) while 1 to 9 pick its candidates.
inline bool spelling_digits(const nlohmann::json &view) {
  return spelling_symbol(view, U'1');
}

} // namespace msime::linux_host
