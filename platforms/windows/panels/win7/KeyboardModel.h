#pragma once

#include "../../settings/win7/ControlText.h"
#include "../../settings/win7/ThemeData.h"
#include <vector>
#include <optional>
#include <map>

namespace msime::windows::win7::keyboard {
struct Key {
  std::wstring label, shifted, hint;
  WORD code = 0;
  double weight = 1;
  bool modifier = false;
};
using Rows = std::vector<std::vector<Key>>;
struct Modifiers { bool shift = false, caps = false, ctrl = false, alt = false, win = false; };
inline bool letter(const Key& key) { return key.code >= 'A' && key.code <= 'Z'; }
inline bool commit_key(WORD code) {
  return code == VK_SPACE || code == VK_RETURN || code == VK_TAB || code == VK_BACK || code == VK_DELETE ||
    code == VK_MULTIPLY || code == VK_ADD || code == VK_SUBTRACT || code == VK_DECIMAL || code == VK_DIVIDE ||
    (code >= '0' && code <= '9') || (code >= VK_NUMPAD0 && code <= VK_NUMPAD9);
}
inline std::wstring face(const Key& key, const Modifiers& modifiers) {
  const bool shifted = letter(key) ? modifiers.caps != modifiers.shift : modifiers.shift;
  if (!shifted) return key.label;
  if (!key.shifted.empty()) return key.shifted;
  return letter(key) ? std::wstring(1,static_cast<wchar_t>(key.code)) : key.label;
}
inline bool pressed(const Key& key, const Modifiers& modifiers) {
  switch (key.code) {
  case VK_SHIFT: return modifiers.shift;
  case VK_CAPITAL: return modifiers.caps;
  case VK_CONTROL: return modifiers.ctrl;
  case VK_MENU: return modifiers.alt;
  case VK_LWIN: return modifiers.win;
  default: return false;
  }
}
inline void toggle(WORD code, Modifiers& modifiers) {
  switch (code) {
  case VK_SHIFT: modifiers.shift = !modifiers.shift; break;
  case VK_CAPITAL: modifiers.caps = !modifiers.caps; break;
  case VK_CONTROL: modifiers.ctrl = !modifiers.ctrl; break;
  case VK_MENU: modifiers.alt = !modifiers.alt; break;
  case VK_LWIN: modifiers.win = !modifiers.win; break;
  }
}
inline std::vector<INPUT> sequence(const Key& key, const Modifiers& modifiers) {
  std::vector<WORD> held;
  const bool sticky = !commit_key(key.code);
  if (sticky && modifiers.ctrl) held.push_back(VK_CONTROL);
  if (sticky && modifiers.alt) held.push_back(VK_MENU);
  if (sticky && modifiers.win) held.push_back(VK_LWIN);
  if (sticky && (letter(key) ? modifiers.caps != modifiers.shift : modifiers.shift)) held.push_back(VK_SHIFT);
  std::vector<INPUT> inputs;
  const auto append = [&](WORD code, bool up) {
    INPUT input{}; input.type = INPUT_KEYBOARD; input.ki.wVk = code;
    // 导航与数字小键盘的除号使用扩展键标记；普通数字小键盘数字保持各自键码。
    const bool extended = code == VK_INSERT || code == VK_DELETE || code == VK_HOME || code == VK_END || code == VK_PRIOR || code == VK_NEXT ||
      code == VK_LEFT || code == VK_RIGHT || code == VK_UP || code == VK_DOWN || code == VK_DIVIDE || code == VK_LWIN || code == VK_APPS || code == VK_SNAPSHOT || code == VK_NUMLOCK;
    input.ki.dwFlags = (up ? KEYEVENTF_KEYUP : 0) | (extended ? KEYEVENTF_EXTENDEDKEY : 0); inputs.push_back(input);
  };
  for (const auto code : held) append(code,false);
  append(key.code,false); append(key.code,true);
  for (auto item = held.rbegin(); item != held.rend(); ++item) append(*item,true);
  return inputs;
}
inline Key key(const wchar_t* label, WORD code, const wchar_t* shifted = L"", double weight = 1, bool modifier = false) {
  return {label,shifted,L"",code,weight,modifier};
}
inline Rows full_rows() {
  Rows rows(7);
  auto& number = rows[0]; number.push_back(key(L"Num Lock",VK_NUMLOCK));
  for (WORD index = 0; index < 10; ++index) number.push_back({L"Num " + std::to_wstring(index),L"",L"",static_cast<WORD>(VK_NUMPAD0 + index),1,false});
  for (const auto& item : std::vector<Key>{key(L"Num *",VK_MULTIPLY),key(L"Num +",VK_ADD),key(L"Num -",VK_SUBTRACT),key(L"Num /",VK_DIVIDE),key(L"Num .",VK_DECIMAL)}) number.push_back(item);
  auto& functions = rows[1]; functions.push_back(key(L"Esc",VK_ESCAPE));
  for (WORD index = 0; index < 12; ++index) functions.push_back({L"F" + std::to_wstring(index + 1),L"",L"",static_cast<WORD>(VK_F1 + index),1,false});
  for (const auto& item : std::vector<Key>{key(L"PrtSc",VK_SNAPSHOT),key(L"Scroll",VK_SCROLL),key(L"Pause",VK_PAUSE),key(L"Ins",VK_INSERT),key(L"Home",VK_HOME),key(L"End",VK_END),key(L"PgUp",VK_PRIOR),key(L"PgDn",VK_NEXT)}) functions.push_back(item);
  rows[2].push_back(key(L"`",VK_OEM_3,L"~"));
  const std::wstring numbers = L"1234567890", shifted = L"!@#$%^&*()";
  for (size_t index = 0; index < numbers.size(); ++index) rows[2].push_back({numbers.substr(index,1),shifted.substr(index,1),L"",static_cast<WORD>(numbers[index]),1,false});
  rows[2].push_back(key(L"-",VK_OEM_MINUS,L"_")); rows[2].push_back(key(L"=",VK_OEM_PLUS,L"+")); rows[2].push_back(key(L"Backspace",VK_BACK,L"",1.9));
  const auto letters = [](std::vector<Key>& row, std::wstring_view values) {
    for (const auto code : values) row.push_back({std::wstring(1,static_cast<wchar_t>(code + (L'a' - L'A'))),L"",L"",static_cast<WORD>(code),1,false});
  };
  rows[3].push_back(key(L"Tab",VK_TAB,L"",1.5)); letters(rows[3],L"QWERTYUIOP");
  rows[3].push_back(key(L"[",VK_OEM_4,L"{")); rows[3].push_back(key(L"]",VK_OEM_6,L"}")); rows[3].push_back(key(L"\\",VK_OEM_5,L"|",1.4));
  rows[4].push_back(key(L"Caps Lock",VK_CAPITAL,L"",1.85,true)); letters(rows[4],L"ASDFGHJKL");
  rows[4].push_back(key(L";",VK_OEM_1,L":")); rows[4].push_back(key(L"'",VK_OEM_7,L"\"")); rows[4].push_back(key(L"Enter",VK_RETURN,L"",2));
  rows[5].push_back(key(L"Shift",VK_SHIFT,L"",2.35,true)); letters(rows[5],L"ZXCVBNM");
  rows[5].push_back(key(L",",VK_OEM_COMMA,L"<")); rows[5].push_back(key(L".",VK_OEM_PERIOD,L">")); rows[5].push_back(key(L"/",VK_OEM_2,L"?")); rows[5].push_back(key(L"Shift",VK_SHIFT,L"",2.15,true));
  rows[6] = {key(L"Ctrl",VK_CONTROL,L"",1.25,true),key(L"Win",VK_LWIN,L"",1.25,true),key(L"Alt",VK_MENU,L"",1.25,true),key(L"Space",VK_SPACE,L" ",6.7),
    key(L"Alt",VK_MENU,L"",1.25,true),key(L"Win",VK_LWIN,L"",1.25,true),key(L"Menu",VK_APPS,L"",1.25),key(L"Del",VK_DELETE,L"",1.25),
    key(L"←",VK_LEFT,L"",1.25),key(L"↑",VK_UP,L"",1.25),key(L"↓",VK_DOWN,L"",1.25),key(L"→",VK_RIGHT,L"",1.25),key(L"Ctrl",VK_CONTROL,L"",1.25,true)};
  return rows;
}
inline Rows nine_rows(const std::map<WORD,std::wstring>& hints = {}) {
  Rows rows(5);
  for (WORD index = 1; index <= 9; ++index) {
    auto item = key(std::to_wstring(index).c_str(),static_cast<WORD>('0' + index)); const auto found = hints.find(item.code); if (found != hints.end()) item.hint = found->second;
    rows[(index - 1) / 3].push_back(std::move(item));
  }
  auto zero = key(L"0",'0'); const auto found = hints.find('0'); if (found != hints.end()) zero.hint = found->second;
  rows[3] = {key(L"Backspace",VK_BACK),std::move(zero),key(L"Enter",VK_RETURN)}; rows[4] = {key(L"Space",VK_SPACE,L" ")}; return rows;
}
inline std::optional<std::map<WORD,std::wstring>> shuangpin_hints(const msime::settings::win7::Json& preferences) {
  namespace shared = msime::settings::win7;
  shared::Json query{{"profile",preferences.value("nine_key_shuangpin_profile",std::string("migala3"))}};
  if (query.at("profile") == "custom") query["table"] = preferences.value("nine_key_shuangpin_custom_profile",shared::Json::object());
  const auto value = shared::theme_data::value(shared::theme_data::call(msime_client_nine_key_shuangpin_layout,query));
  if (!value || !value->is_object() || !value->contains("keys") || !value->at("keys").is_array() || value->at("keys").size() > 10) return std::nullopt;
  std::map<WORD,std::wstring> result;
  for (const auto& item : value->at("keys")) {
    if (!item.is_object() || !item.contains("key") || !item.at("key").is_string() || !item.contains("zero_initial") || !item.at("zero_initial").is_boolean()) return std::nullopt;
    const auto digit = item.at("key").get<std::string>(); if (digit.size() != 1 || digit.front() < '0' || digit.front() > '9') return std::nullopt;
    std::wstring label;
    for (const auto* field : {"initials","finals"}) {
      if (!item.contains(field) || !item.at(field).is_array()) return std::nullopt;
      for (const auto& unit : item.at(field)) { if (!unit.is_string()) return std::nullopt; if (!label.empty()) label += L" "; label += shared::control_text::wide(unit.get<std::string>()); }
    }
    if (item.at("zero_initial").get<bool>()) label += L" 零声母";
    if (!result.emplace(static_cast<WORD>(digit.front()),std::move(label)).second) return std::nullopt;
  }
  return result;
}
}
