#pragma once

#include "PreferencesStore.h"
#include "SettingsWindow.h"
#include <commctrl.h>
#include <algorithm>
#include <array>
#include <functional>
#include <tuple>

namespace msime::settings::win7 {

class GlossForm {
public:
  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row) {
    row(L"STATIC", L"多语言与释义", SS_LEFT, -1, 0, 28);
    english_ = row(L"BUTTON", L"显示英文释义", WS_TABSTOP | BS_AUTOCHECKBOX, gloss_english, 42, 28);
    row(L"STATIC", L"在候选词后面标出它的英文意思，中文候选给英文、英文候选给中文。释义来自随输入法打包的离线词库，不联网。", SS_LEFT, -1, 78, 72);
    translations_ = row(L"BUTTON", L"候选词翻译", WS_TABSTOP | BS_AUTOCHECKBOX, gloss_translations, 170, 28);
    row(L"STATIC", L"为当前候选请求翻译结果并显示在候选行。目标语言与第二种语言也用于选择离线释义。", SS_LEFT, -1, 206, 60);
    row(L"STATIC", L"目标语言", SS_LEFT, -1, 282, 24);
    target_ = row(WC_COMBOBOXW, L"目标语言", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, gloss_target, 316, 240);
    pronunciation_ = row(L"BUTTON", L"显示读音", WS_TABSTOP | BS_AUTOCHECKBOX, gloss_pronunciation, 370, 28);
    row(L"STATIC", L"在释义后标出英文音标、日文罗马音。音标来自离线词表，罗马音由系统生成，都不联网。", SS_LEFT, -1, 406, 60);
    row(L"STATIC", L"第二种语言", SS_LEFT, -1, 482, 24);
    secondary_ = row(WC_COMBOBOXW, L"第二种语言", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, gloss_secondary, 516, 240);
    SendMessageW(secondary_, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(L"不显示第二种语言"));
    for (const auto& language : languages) {
      SendMessageW(target_, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(language.second));
      SendMessageW(secondary_, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(language.second));
    }
    return 572;
  }

  static bool valid(const Json& preferences) {
    for (const auto* key : {"candidate_english_gloss", "candidate_translations", "candidate_pronunciation"})
      if (!preferences.contains(key) || !preferences.at(key).is_boolean()) return false;
    const auto language = [](const Json& value) {
      return value.is_string() && std::any_of(languages.begin(), languages.end(), [&](const auto& option) { return value == option.first; });
    };
    if (!preferences.contains("translation_target_language") || !language(preferences.at("translation_target_language"))) return false;
    return !preferences.contains("translation_secondary_language") || language(preferences.at("translation_secondary_language"));
  }

  void refresh(const Json& preferences) {
    glosses_ = preferences.at("candidate_english_gloss").get<bool>() || preferences.at("candidate_translations").get<bool>();
    SendMessageW(english_, BM_SETCHECK, preferences.at("candidate_english_gloss").get<bool>() ? BST_CHECKED : BST_UNCHECKED, 0);
    SendMessageW(translations_, BM_SETCHECK, preferences.at("candidate_translations").get<bool>() ? BST_CHECKED : BST_UNCHECKED, 0);
    SendMessageW(pronunciation_, BM_SETCHECK, preferences.at("candidate_pronunciation").get<bool>() ? BST_CHECKED : BST_UNCHECKED, 0);
    SendMessageW(secondary_, CB_SETCURSEL, 0, 0);
    for (size_t index = 0; index < languages.size(); ++index) {
      if (preferences.at("translation_target_language") == languages[index].first) SendMessageW(target_, CB_SETCURSEL, index, 0);
      if (preferences.value("translation_secondary_language", std::string{}) == languages[index].first)
        SendMessageW(secondary_, CB_SETCURSEL, index + 1, 0);
    }
  }

  void enable(bool enabled) {
    enabled_ = enabled;
    for (const auto control : {english_, translations_}) EnableWindow(control, enabled);
    for (const auto control : {target_, secondary_, pronunciation_}) EnableWindow(control, enabled && glosses_);
  }

  bool handles(int id) const { return id >= gloss_english && id <= gloss_secondary; }

  std::vector<Change> command(WPARAM event) const {
    if (!enabled_) return {};
    const int id = LOWORD(event), notification = HIWORD(event);
    for (const auto& field : std::array<std::tuple<int, HWND, const char*>, 3>{{
        {gloss_english, english_, "candidate_english_gloss"}, {gloss_translations, translations_, "candidate_translations"},
        {gloss_pronunciation, pronunciation_, "candidate_pronunciation"}}})
      if (id == std::get<0>(field) && notification == BN_CLICKED && IsWindowEnabled(std::get<1>(field)))
        return {{{std::get<2>(field)}, SendMessageW(std::get<1>(field), BM_GETCHECK, 0, 0) == BST_CHECKED}};
    if (notification != CBN_SELCHANGE || (id != gloss_target && id != gloss_secondary)) return {};
    const auto control = id == gloss_target ? target_ : secondary_;
    if (!IsWindowEnabled(control)) return {};
    const auto selected = SendMessageW(control, CB_GETCURSEL, 0, 0);
    const int offset = id == gloss_secondary ? 1 : 0;
    if (offset == 1 && selected == 0) return {{{"translation_secondary_language"}, nullptr}};
    if (selected < offset || static_cast<size_t>(selected - offset) >= languages.size()) return {};
    return {{{id == gloss_target ? "translation_target_language" : "translation_secondary_language"}, languages[static_cast<size_t>(selected - offset)].first}};
  }

private:
  static constexpr std::array<std::pair<const char*, const wchar_t*>, 7> languages{{
      {"en", L"英语"}, {"fr", L"法语"}, {"ja", L"日语"}, {"es", L"西班牙语"},
      {"ru", L"俄语"}, {"de", L"德语"}, {"ko", L"韩语"}}};
  HWND english_ = nullptr, translations_ = nullptr, pronunciation_ = nullptr;
  HWND target_ = nullptr, secondary_ = nullptr;
  bool enabled_ = false, glosses_ = false;
};

} // namespace msime::settings::win7
