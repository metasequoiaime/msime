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

// 组字中（editing_text 非空、本地模式为 none）Engine 列出的符号都优先作为输入，包括网址入口和已配置的辅助码前导键。空闲时列出的 "/"、"@" 仍走标点路径。
inline bool composing_spelling(const nlohmann::json &view, char32_t character) {
  if (!view.is_object()) return false;
  const auto mode = view.find("local_mode");
  if (mode != view.end() && (!mode->is_string() || *mode != "none")) return false;
  const auto editing = view.find("editing_text");
  return editing != view.end() && editing->is_string() &&
         !editing->get_ref<const std::string &>().empty() && spelling_symbol(view, character);
}

// 不打开本地模式的方案（注音、粤拼、越南文、藏文等）自己列出的拼写符号：那份列表就是该方案的按键。
inline bool scheme_spelling(const nlohmann::json &view, char32_t character) {
  const int scheme = scheme_rules(view);
  return scheme >= 0 && !scheme::OpensTableModes(scheme) && spelling_symbol(view, character);
}

// Engine 拿来拼写、宿主要在自己的按键绑定之前交给 Engine 的字符：本地模式的全部符号（见 local_mode_spelling）、组字中列出的全部符号（见 composing_spelling，即打开网址模式的按键），以及不打开本地模式的方案列出的全部符号（见 scheme_spelling）。这样注音的数字或 "," 是注音键而不是选候选、翻页键或中文标点，粤拼的撇号用来分隔音节。全拼、双拼和五笔空闲时打开模式的按键仍走标点路径，原因见 local_mode_spelling。
inline bool engine_spelling(const nlohmann::json &view, char32_t character) {
  return local_mode_spelling(view, character) || composing_spelling(view, character) ||
         scheme_spelling(view, character);
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

// 数字行的物理键在能选词时（`picking`：数字行选词开着、不是九键、屏幕上有候选）选候选，不作为拼写交给 Engine，与 Windows 一样按物理键判断，不看当前布局在这个键上打出什么：
// - 拼写用数字的模式（Unicode、表达式）里是 Shift 加数字行，哪怕这个组合在 AZERTY 上打出的就是数字本身；Shift 打出的是模式也拼写的符号时（美式布局表达式模式的 % ^ * ( )）仍是输入。
// - 其他模式里是不按 Shift 的数字行，哪怕布局在这个键上打出的是组字中可拼的符号：AZERTY 的 1 打出 "&"，全拼、双拼组字中把它当字面符号收下（`spelling_symbols` 列着它），不这样判断它就成了组字的一部分，数字行选不了词。方案自己拼写的符号（粤拼、藏文的撇号）和本地模式拼写的符号仍是输入。
inline bool number_row_picks(const nlohmann::json &view, char32_t character, bool shift,
                             bool number_row_key, bool picking) {
  if (!picking || !number_row_key) return false;
  if (spelling_digits(view)) return shift && character >= U'0' && character <= U'9';
  return !shift && !local_mode_spelling(view, character) && !scheme_spelling(view, character);
}

// 普通拼音组字中若引擎把 AZERTY 数字行当前字面符号列为 `spelling_symbols`，裸物理数字行仍优先选候选；真正列出数字的本地模式（Unicode、算式、网址等）继续把按键交给模式输入。
inline bool number_row_picks(const nlohmann::json &view, bool shift, bool number_row_key,
                             bool picking) {
  return picking && !shift && number_row_key && !spelling_digits(view) &&
         view.value("local_mode", std::string("none")) == "none";
}

} // namespace msime::linux_host
