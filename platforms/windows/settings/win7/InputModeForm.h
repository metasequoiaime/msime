#pragma once

#include "PreferencesStore.h"
#include "SettingsWindow.h"
#include "ControlText.h"
#include "../AppInputModeRuleList.h"
#include <algorithm>
#include <array>
#include <functional>
#include <vector>

namespace msime::settings::win7 {

class InputModeForm {
public:
  using Row = std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>;
  struct Command {
    bool handled = false;
    std::vector<Change> changes;
    std::wstring message;
  };

  int create(const Row& row) {
    const auto choice = [&](int id, const wchar_t* title, const auto& labels, int top) {
      row(L"STATIC", title, SS_LEFT, -1, top, 24);
      const HWND control = row(WC_COMBOBOXW, title, WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL,
          id, top + 34, 160);
      for (const auto* label : labels) SendMessageW(control, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label));
      controls_[static_cast<size_t>(id - default_ime_mode)] = control;
    };
    row(L"STATIC", L"中英模式", SS_LEFT, -1, 0, 28);
    row(L"STATIC", L"新焦点会话开始时使用的中文或英文状态。", SS_LEFT, -1, 40, 28);
    choice(default_ime_mode, L"默认中英文", std::array{L"中文", L"英文"}, 78);
    row(L"STATIC", L"按应用分别记忆输入状态，或让所有输入上下文保持同一状态。", SS_LEFT, -1, 160, 48);
    choice(ime_mode_scope, L"中英文状态", std::array{L"按应用记忆", L"全局统一"}, 218);
    controls_[input_mode_hud - default_ime_mode] = row(L"BUTTON", L"中英文切换提示", WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY,
        input_mode_hud, 310, 28);
    row(L"STATIC", L"切换输入模式后，在光标附近短暂显示“中”或“英”，不会抢占焦点。", SS_LEFT, -1, 346, 48);
    row(L"STATIC", L"应用例外", SS_LEFT, -1, 416, 28);
    row(L"STATIC", L"切到这些程序时从指定的中文或英文开始，优先于默认状态和记忆，「全局统一」下也生效。在程序里手动切换后，到下次切回这个程序之前不再套用。", SS_LEFT, -1, 454, 78);
    row(L"STATIC", L"程序文件名，例如 code.exe；也可粘贴完整路径。新例外从中文开始。", SS_LEFT, -1, 548, 48);
    controls_[app_rule_name - default_ime_mode] = row(L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL,
        app_rule_name, 606, 28);
    SendMessageW(control(app_rule_name), EM_LIMITTEXT, 2048, 0);
    controls_[app_rule_add - default_ime_mode] = row(L"BUTTON", L"添加应用例外", WS_TABSTOP | BS_PUSHBUTTON,
        app_rule_add, 648, 32);
    controls_[app_rule_list - default_ime_mode] = row(L"LISTBOX", L"应用例外列表", WS_TABSTOP | WS_BORDER | WS_VSCROLL |
        LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, app_rule_list, 700, 180);
    choice(app_rule_mode, L"选中应用的起始模式", std::array{L"中文", L"英文"}, 906);
    controls_[app_rule_remove - default_ime_mode] = row(L"BUTTON", L"移除选中的应用例外", WS_TABSTOP | BS_PUSHBUTTON,
        app_rule_remove, 992, 32);
    return 1042;
  }

  static bool valid(const Json& preferences) {
    for (const auto* field : {"default_ime_mode", "ime_mode_scope"})
      if (!preferences.contains(field) || !preferences.at(field).is_string()) return false;
    if (preferences.at("default_ime_mode") != "chinese" && preferences.at("default_ime_mode") != "english") return false;
    if (preferences.at("ime_mode_scope") != "app" && preferences.at("ime_mode_scope") != "global") return false;
    if (!preferences.contains("input_mode_hud") || !preferences.at("input_mode_hud").is_boolean()) return false;
    if (preferences.contains("app_input_mode_rules")) {
      const auto& rules = preferences.at("app_input_mode_rules");
      if (!rules.is_object()) return false;
      for (const auto& rule : rules.items())
        if (rule.value() != "chinese" && rule.value() != "english") return false;
    }
    return true;
  }

  void refresh(const Json& preferences) {
    SendMessageW(control(default_ime_mode), CB_SETCURSEL, preferences.at("default_ime_mode") == "english" ? 1 : 0, 0);
    SendMessageW(control(ime_mode_scope), CB_SETCURSEL, preferences.at("ime_mode_scope") == "global" ? 1 : 0, 0);
    SendMessageW(control(input_mode_hud), BM_SETCHECK, preferences.at("input_mode_hud").get<bool>() ? BST_CHECKED : BST_UNCHECKED, 0);
    const auto previous = selected();
    rules_ = preferences.value("app_input_mode_rules", Json::object());
    keys_.clear();
    SendMessageW(control(app_rule_list), LB_RESETCONTENT, 0, 0);
    int selection = -1;
    for (const auto& rule : rules_.items()) {
      if (previous && *previous == rule.key()) selection = static_cast<int>(keys_.size());
      keys_.push_back(rule.key());
      const auto label = control_text::wide(rule.key()) + (rule.value() == "english" ? L"　英文" : L"　中文");
      SendMessageW(control(app_rule_list), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
    }
    SendMessageW(control(app_rule_list), LB_SETCURSEL, selection, 0);
    update_selection();
  }

  void enable(bool enabled) {
    enabled_ = enabled;
    for (const auto item : controls_) EnableWindow(item, enabled);
    update_selection();
  }

  Command command(WPARAM event) {
    const int id = LOWORD(event);
    const int notification = HIWORD(event);
    if (id < default_ime_mode || id > app_rule_remove) return {};
    Command result{true, {}, {}};
    if (!enabled_) return result;
    if (id == app_rule_list && notification == LBN_SELCHANGE) {
      update_selection();
    } else if (id == app_rule_add && notification == BN_CLICKED) {
      const auto raw = control_text::read(control(app_rule_name));
      if (!raw) { result.message = L"程序文件名包含无法保存的文字。"; return result; }
      const auto name = normalize_app_input_mode_rule(control_text::wide(*raw));
      std::vector<std::wstring> existing;
      for (const auto& entry : rules_.items()) existing.push_back(control_text::wide(entry.key()));
      if (const auto error = validate_app_input_mode_rule(name, existing)) {
        result.message = problem(*error);
        return result;
      }
      const auto key = control_text::utf8(name);
      if (!key) { result.message = L"程序文件名包含无法保存的文字。"; return result; }
      auto next = rules_;
      next[*key] = "chinese";
      result.changes.push_back({{"app_input_mode_rules"}, std::move(next)});
    } else if (id == app_rule_remove && notification == BN_CLICKED) {
      if (const auto key = selected()) {
        auto next = rules_;
        next.erase(*key);
        result.changes.push_back({{"app_input_mode_rules"}, std::move(next)});
      }
    } else if (id == app_rule_mode && notification == CBN_SELCHANGE) {
      const auto index = SendMessageW(control(id), CB_GETCURSEL, 0, 0);
      if (const auto key = selected(); key && index >= 0 && index <= 1) {
        auto next = rules_;
        next[*key] = index == 1 ? "english" : "chinese";
        result.changes.push_back({{"app_input_mode_rules"}, std::move(next)});
      }
    } else if ((id == default_ime_mode || id == ime_mode_scope) && notification == CBN_SELCHANGE) {
      const auto index = SendMessageW(control(id), CB_GETCURSEL, 0, 0);
      if (index >= 0 && index <= 1)
        result.changes.push_back({{id == default_ime_mode ? "default_ime_mode" : "ime_mode_scope"},
            id == default_ime_mode ? (index == 1 ? "english" : "chinese") : (index == 1 ? "global" : "app")});
    } else if (id == input_mode_hud && notification == BN_CLICKED) {
      result.changes.push_back({{"input_mode_hud"}, SendMessageW(control(id), BM_GETCHECK, 0, 0) == BST_CHECKED});
    }
    return result;
  }

private:
  static const wchar_t* problem(AppInputModeRuleError error) {
    switch (error) {
    case AppInputModeRuleError::Empty: return L"请填写程序文件名，例如 code.exe。";
    case AppInputModeRuleError::TooLong: return L"程序文件名不能超过 64 个字节（一个汉字算 3 个字节）。";
    case AppInputModeRuleError::InvalidCharacter: return L"程序文件名不能包含控制字符。";
    case AppInputModeRuleError::NotExe: return L"程序文件名要以 .exe 结尾，例如 code.exe。";
    case AppInputModeRuleError::Duplicate: return L"这个程序已经有例外了，在列表里改它的模式即可。";
    case AppInputModeRuleError::TooMany: return L"最多 32 个应用例外，请先移除不再需要的。";
    }
    return L"无法添加这个应用例外。";
  }

  std::optional<std::string> selected() const {
    const auto index = SendMessageW(control(app_rule_list), LB_GETCURSEL, 0, 0);
    if (index < 0 || static_cast<size_t>(index) >= keys_.size()) return std::nullopt;
    return keys_[static_cast<size_t>(index)];
  }

  void update_selection() {
    const auto key = selected();
    SendMessageW(control(app_rule_mode), CB_SETCURSEL, key ? (rules_.at(*key) == "english" ? 1 : 0) : -1, 0);
    EnableWindow(control(app_rule_mode), enabled_ && key.has_value());
    EnableWindow(control(app_rule_remove), enabled_ && key.has_value());
  }

  HWND control(int id) const { return controls_[static_cast<size_t>(id - default_ime_mode)]; }
  std::array<HWND, app_rule_remove - default_ime_mode + 1> controls_{};
  std::vector<std::string> keys_;
  Json rules_ = Json::object();
  bool enabled_ = false;
};

} // namespace msime::settings::win7
