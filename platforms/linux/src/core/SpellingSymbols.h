#pragma once

#include "InputSchemes.h"

#include <nlohmann/json.hpp>

#include <string>

namespace msime::linux_host {

// Engine 在 View.spelling_symbols 里列出当前状态下它当作输入的非字母字符：表达式模式的运算符和数字、Unicode 模式的数字、网址模式的数字和网址符号、没有组字时打开模式的按键（"/" 和 "@"）、组字原文是网址触发词时打开网址模式的按键（"." 或 ":"），以及方案拼写用的按键（注音键盘的数字和符号、粤拼的音节分隔撇号、越南文 VNI 的声调数字、藏文威利转写的撇号、叠写加号、消歧句点、连字符和上屏垂符的斜杠）。IBus 和 Fcitx5 宿主共用它，因此两边都不必各自维护哪个模式拼写什么的列表；自己维护的列表会在 Engine 第一次新增模式时就跑偏。
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

// 组字中（editing_text 非空、不在本地模式）Engine 列出的符号是它要收的输入：全拼、双拼和五笔的组字原文是网址触发词（www、http、https、ftp）时，Engine 只列出打开网址模式的那个键（"." 或 ":"），宿主必须把它当字符交给 Engine，而不是翻页键或中文标点。其他组字状态下这些方案不列任何符号，所以这条不会抢走普通的标点和翻页键。空闲时列出的 "/"、"@" 不在此列，editing_text 为空。
inline bool composing_spelling(const nlohmann::json &view, char32_t character) {
  if (!view.is_object()) return false;
  const auto mode = view.find("local_mode");
  if (mode != view.end() && (!mode->is_string() || *mode != "none")) return false;
  const auto editing = view.find("editing_text");
  return editing != view.end() && editing->is_string() &&
         !editing->get_ref<const std::string &>().empty() && spelling_symbol(view, character);
}

// A character the Engine spells with that the host sends to the Engine before any of its own key bindings can claim it: every symbol of a local mode (see local_mode_spelling), every symbol listed while a composition is open (see composing_spelling, the URL-mode entry keys), and every symbol listed in a scheme that opens no local modes, where the list is that scheme's own keys. A Zhuyin digit or "," is then a bopomofo key rather than a candidate shortcut, a page key or Chinese punctuation, and a Cantonese apostrophe separates syllables. Quanpin's and Shuangpin's idle mode-entry keys stay on the punctuation route, as local_mode_spelling explains.
inline bool engine_spelling(const nlohmann::json &view, char32_t character) {
  if (local_mode_spelling(view, character) || composing_spelling(view, character)) return true;
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
