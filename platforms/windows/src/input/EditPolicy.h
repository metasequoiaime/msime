#pragma once
#include "KeyEvent.h"
#include <string_view>
#include <nlohmann/json.hpp>

namespace msime::windows {
enum class TsfPreeditStyle { Local, Pinyin, Empty };
inline TsfPreeditStyle preference_tsf_preedit_style(const nlohmann::json &p) {
  const auto value = p.value("tsf_preedit_style", "raw");
  if (value == "raw") return TsfPreeditStyle::Local;
  if (value == "pinyin") return TsfPreeditStyle::Pinyin;
  if (value == "empty") return TsfPreeditStyle::Empty;
  throw std::invalid_argument("Invalid TSF preedit style preference");
}
enum class EditKind { None, Character, Erase, Caret };
// Whether the Engine takes this key's text as input in its current state: View.spelling_symbols, which lists V's digits and operators, U's digits, and on an empty composition in a scheme that opens the table modes (the pinyin schemes and Wubi, `opens_table_modes`) the "/" and "@" that open their modes.
inline bool spelled_by_engine(std::string_view spelling_symbols, uint32_t text) {
  return text >= 0x21 && text <= 0x7E &&
         spelling_symbols.find(static_cast<char>(text)) != std::string_view::npos;
}
// Whether digit key 1-9 picks the candidate in its slot, from the View's mode and spelling symbols rather than the text on screen. The TIP classifies the same key by the same rule, so the two never disagree about whether it was a selection. U keeps its key-based rule, Shift+digit selects and a bare digit is hex whatever the layout prints. Elsewhere a key whose text the Engine spells is input; in a mode that spells digits (V) the digit keys that print something else, Shift+1's "!" on a US layout, select with or without Shift; everywhere else a bare digit selects.
inline bool digit_selects_candidate(std::string_view mode,
                                    std::string_view spelling_symbols,
                                    uint32_t text, uint32_t modifiers) {
  if (mode == "unknown")
    return false;
  if (mode == "unicode")
    return modifiers == 1;
  if (spelled_by_engine(spelling_symbols, text))
    return false;
  if (spelling_symbols.find_first_of("0123456789") != std::string_view::npos)
    return modifiers <= 1;
  return modifiers == 0;
}
// Only composition editing. Native priority paths (shortcuts,
// word-to-character, punctuation and navigation) remain separate; never infer
// Engine mode from text.
inline EditKind edit_kind(const FanyImeNamedpipeData &packet,
                          std::string_view mode, bool composing,
                          bool microsoft_shuangpin = false,
                          std::string_view editing = {}, size_t caret = 0,
                          bool japanese_scheme = false,
                          std::string_view spelling_symbols = {},
                          bool apostrophe_is_punctuation = false) {
  if (packet.event_type != FanyImePipeEventType::KeyEvent || mode == "unknown")
    return EditKind::None;
  const auto modifiers = PipeMetadata::key_modifiers(packet.modifiers_down);
  if (modifiers & ~1u)
    return EditKind::None;
  const auto key = normalize_digit_key(packet.keycode);
  const auto text = static_cast<uint32_t>(packet.wch);
  // Japanese reserves the OEM minus key for the long-vowel mark. Route it through Engine instead of treating it as navigation or punctuation. Like the reference, it also starts a composition from an empty buffer: the TSF sends it as input there, and the Engine offers ー and the literal hyphen.
  if (modifiers == 0 &&
      should_send_composition_reply(false, false, false, false, false,
                                    japanese_scheme && key == 0xBD &&
                                        text == '-'))
    return EditKind::Character;
  if (composing && modifiers == 0) {
    if (key == 0x08 || key == 0x2E)
      return EditKind::Erase;
    if (key == 0x25 || key == 0x27)
      return EditKind::Caret;
  }
  // A scheme that spells with Space lists it among its spelling symbols (Zhuyin's first tone). Its Space command then types that key rather than picking a row (the runtime's SelectHighlighted), so it is composition input like any spelled symbol.
  if (key == 0x20 && modifiers == 0 && spelling_symbols.find(' ') != std::string_view::npos)
    return EditKind::Character;
  if (translate_key(packet).kind != KeyKind::Character)
    return EditKind::None;
  if (microsoft_shuangpin && mode == "none" && key == 0xBA && text == ';') {
    if (caret > editing.size())
      caret = editing.size();
    const auto separator =
        caret == 0 ? std::string_view::npos : editing.rfind('\'', caret - 1);
    const auto start = separator == std::string_view::npos ? 0 : separator + 1;
    if ((caret - start) % 2 == 1)
      return EditKind::Character;
  }
  // The pinyin syllable separator. Under Stroke (scheme::ApostropheIsPunctuationWhileComposing) the Engine refuses it, so it goes on to the punctuation route like a comma.
  if (composing && modifiers == 0 && text == '\'' && mode == "none" && !apostrophe_is_punctuation)
    return EditKind::Character;
  if (key >= 'A' && key <= 'Z' &&
      ((text >= 'a' && text <= 'z') || (text >= 'A' && text <= 'Z')))
    return EditKind::Character;
  if (mode == "unicode") {
    if (modifiers == 1 && key >= '1' && key <= '9')
      return EditKind::None;
    if (modifiers == 0 && key >= '0' && key <= '9')
      return EditKind::Character;
    if (text == '+')
      return EditKind::Character;
    return EditKind::None;
  }
  // V's digits and operators, and on an empty composition in a scheme that opens the table modes (the pinyin schemes and Wubi, `opens_table_modes`) the "/" or "@" that opens its mode.
  if (spelled_by_engine(spelling_symbols, text))
    return EditKind::Character;
  return EditKind::None;
}
} // namespace msime::windows
