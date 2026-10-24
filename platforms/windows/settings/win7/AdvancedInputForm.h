#pragma once

#include "PreferencesStore.h"
#include "SettingsWindow.h"
#include "ControlText.h"
#include "HelpcodeForm.h"
#include "../InputSettingsModel.h"
#include "../TranslationServiceSettings.h"
#include "../../../../shared/contracts/msime_edition.h"
#include <commctrl.h>
#include <algorithm>
#include <array>
#include <functional>

namespace msime::settings::win7 {

class AdvancedInputForm {
public:
  using Row = std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>;
  struct Command {
    bool handled = false;
    std::vector<Change> changes;
    bool reset_confirmation = false;
  };

  int create(const Row& row) {
    int top = 0;
    row(L"STATIC", L"快捷模式", SS_LEFT, -1, top, 28);
    shift_ = row(L"BUTTON", L"Shift+字母进入快捷模式", WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY,
        shift_letter_modes, 42, 28);
    row(L"STATIC", L"没有输入时按 Shift 加各模式的字母进入该模式。关掉后 Shift+字母照常打出大写字母，各模式开关保留；/ 和 @ 不受影响。", SS_LEFT, -1, 78, 72);
    top = 166;
    for (size_t index = 0; index < input::local_modes.size(); ++index) {
      const auto& mode = input::local_modes[index];
      if (mode.id == L"temporary_japanese" && !MSIME_EDITION_TEMPORARY_JAPANESE) continue;
      modes_[index] = row(L"BUTTON", mode.title.data(), WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY,
          local_mode_first + static_cast<int>(index), top, 28);
      descriptions_[index] = row(L"STATIC", L"", SS_LEFT, -1, top + 36, 120);
      top += 176;
    }
    row(L"STATIC", L"模糊音", SS_LEFT, -1, top, 28);
    fuzzy_ = row(L"BUTTON", L"启用模糊音", WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY, fuzzy_enabled, top + 42, 28);
    row(L"STATIC", L"全拼、九键与双拼均支持；更改在当前输入结束后生效。第一次启用时勾选全部规则；关闭总开关保留已选规则。", SS_LEFT, -1, top + 78, 72);
    top += 170;
    std::wstring_view previous_group;
    for (size_t index = 0; index < input::fuzzy_rules.size(); ++index) {
      const auto& rule = input::fuzzy_rules[index];
      if (rule.group != previous_group) {
        row(L"STATIC", rule.group.data(), SS_LEFT, -1, top, 24);
        top += 34;
        previous_group = rule.group;
      }
      rules_controls_[index] = row(L"BUTTON", rule.label.data(), WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY,
          fuzzy_rule_first + static_cast<int>(index), top, 28);
      top += 38;
    }
    reset_ = row(L"BUTTON", L"重置模糊音配置…", WS_TABSTOP | BS_PUSHBUTTON, fuzzy_reset, top + 12, 32);
    top += 76;
    top = helpcode_.create(row, top);
    row(L"STATIC", L"拼音方案调频", SS_LEFT, -1, top, 28);
    row(L"STATIC", L"学习选词习惯开启时生效；关闭学习会保留调频配置。", SS_LEFT, -1, top + 36, 48);
    top += 100;
    const auto choice = [&](int id, const wchar_t* label) {
      row(L"STATIC", label, SS_LEFT, -1, top, 24);
      const auto control = row(WC_COMBOBOXW, label, WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL,
          id, top + 34, 200);
      top += 90;
      return control;
    };
    frequency_ = choice(frequency_mode, L"调频方式");
    for (const auto* label : {L"关闭", L"一次置顶", L"折半调频", L"线性调频", L"一次置前"})
      SendMessageW(frequency_, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label));
    trigger_ = choice(frequency_trigger, L"触发频次（第几次上屏触发）");
    step_ = choice(frequency_step, L"线性调频步长");
    return top + 16;
  }

  static bool valid(const Json& preferences) {
    for (const auto* group : {"local_modes", "fuzzy_pinyin", "frequency"})
      if (!preferences.contains(group) || !preferences.at(group).is_object()) return false;
    const auto& modes = preferences.at("local_modes");
    if (!modes.contains("shift_letter_entry") || !modes.at("shift_letter_entry").is_boolean()) return false;
    for (const auto& mode : input::local_modes) {
      const auto key = id(mode.id);
      if (!modes.contains(key) && (key == "chaizi" || key == "symbol")) continue;
      if (!modes.contains(key) || !modes.at(key).is_boolean()) return false;
    }
    const auto& fuzzy = preferences.at("fuzzy_pinyin");
    if (!fuzzy.contains("enabled") || !fuzzy.at("enabled").is_boolean() ||
        !fuzzy.contains("rules") || !fuzzy.at("rules").is_array()) return false;
    for (const auto& rule : fuzzy.at("rules")) if (!rule.is_string()) return false;
    if (fuzzy.contains("seeded") && !fuzzy.at("seeded").is_boolean()) return false;
    const auto& frequency = preferences.at("frequency");
    if (!frequency.contains("mode") || !frequency.at("mode").is_string()) return false;
    const auto mode = frequency.at("mode").get<std::string>();
    if (std::find(frequency_values.begin(), frequency_values.end(), mode) == frequency_values.end()) return false;
    for (const auto* field : {"trigger_count", "linear_step"})
      if (!frequency.contains(field) || !frequency.at(field).is_number_integer() ||
          frequency.at(field).get<int>() < 1 || frequency.at(field).get<int>() > 10) return false;
    return HelpcodeForm::valid(preferences);
  }

  void refresh(const Json& preferences, const HelpcodeSources& sources) {
    modes_value_ = preferences.at("local_modes");
    fuzzy_value_ = preferences.at("fuzzy_pinyin");
    frequency_value_ = preferences.at("frequency");
    learning_ = preferences.at("learning").get<bool>();
    helpcode_.refresh(preferences, sources);
    SendMessageW(shift_, BM_SETCHECK, modes_value_.at("shift_letter_entry").get<bool>() ? BST_CHECKED : BST_UNCHECKED, 0);
    const bool translation = translation_available(preferences);
    for (size_t index = 0; index < input::local_modes.size(); ++index) {
      if (!modes_[index]) continue;
      const auto& mode = input::local_modes[index];
      SendMessageW(modes_[index], BM_SETCHECK, modes_value_.value(id(mode.id), mode.default_on) ? BST_CHECKED : BST_UNCHECKED, 0);
      SetWindowTextW(descriptions_[index], input::local_mode_description(mode, translation).c_str());
    }
    SendMessageW(fuzzy_, BM_SETCHECK, fuzzy_value_.at("enabled").get<bool>() ? BST_CHECKED : BST_UNCHECKED, 0);
    for (size_t index = 0; index < input::fuzzy_rules.size(); ++index) {
      const auto& rules = fuzzy_value_.at("rules");
      const bool checked = std::find(rules.begin(), rules.end(), id(input::fuzzy_rules[index].id)) != rules.end();
      SendMessageW(rules_controls_[index], BM_SETCHECK, checked ? BST_CHECKED : BST_UNCHECKED, 0);
    }
    for (size_t index = 0; index < frequency_values.size(); ++index)
      if (frequency_value_.at("mode") == frequency_values[index]) SendMessageW(frequency_, CB_SETCURSEL, index, 0);
    for (const auto& field : std::array<std::pair<HWND, const char*>, 2>{{{trigger_, "trigger_count"}, {step_, "linear_step"}}}) {
      SendMessageW(field.first, CB_RESETCONTENT, 0, 0);
      for (int value = 1; value <= 6; ++value)
        SendMessageW(field.first, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(std::to_wstring(value).c_str()));
      const int value = frequency_value_.at(field.second).get<int>();
      if (value > 6) {
        const auto label = std::to_wstring(value) + L"（当前值）";
        SendMessageW(field.first, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
      }
      SendMessageW(field.first, CB_SETCURSEL, value > 6 ? 6 : value - 1, 0);
    }
  }

  void enable(bool enabled) {
    enabled_ = enabled;
    helpcode_.enable(enabled);
    EnableWindow(shift_, enabled);
    const bool shift = modes_value_.value("shift_letter_entry", true);
    const bool mention = modes_value_.value("mention", false);
    for (size_t index = 0; index < modes_.size(); ++index)
      if (modes_[index]) EnableWindow(modes_[index], enabled && !input::local_mode_disabled(input::local_modes[index], shift, mention));
    EnableWindow(fuzzy_, enabled);
    const bool fuzzy = fuzzy_value_.value("enabled", false);
    for (const auto control : rules_controls_) EnableWindow(control, enabled && fuzzy);
    EnableWindow(reset_, enabled && fuzzy);
    EnableWindow(frequency_, enabled && learning_);
    const auto mode = frequency_value_.value("mode", std::string("disabled"));
    EnableWindow(trigger_, enabled && learning_ && mode != "disabled");
    EnableWindow(step_, enabled && learning_ && mode == "linear");
  }

  Command command(WPARAM event) const {
    const int control = LOWORD(event);
    if (control >= helpcode_first && control < helpcode_first + 6)
      return {true, helpcode_.command(event), false};
    const int notification = HIWORD(event);
    const bool mode = control >= local_mode_first && control < local_mode_first + static_cast<int>(modes_.size());
    const bool rule = control >= fuzzy_rule_first && control < fuzzy_rule_first + static_cast<int>(rules_controls_.size());
    if (control != shift_letter_modes && control != fuzzy_enabled && control != fuzzy_reset &&
        control != frequency_mode && control != frequency_trigger && control != frequency_step && !mode && !rule) return {};
    Command result{true, {}, false};
    if (!enabled_) return result;
    if (control == fuzzy_reset && notification == BN_CLICKED) {
      result.reset_confirmation = true;
    } else if (control == fuzzy_enabled && notification == BN_CLICKED) {
      const bool enabled = SendMessageW(fuzzy_, BM_GETCHECK, 0, 0) == BST_CHECKED;
      result.changes.push_back({{"fuzzy_pinyin", "enabled"}, enabled});
      if (enabled && !fuzzy_value_.value("seeded", false)) {
        Json rules = Json::array();
        for (const auto& field : input::fuzzy_rules) rules.push_back(id(field.id));
        result.changes.push_back({{"fuzzy_pinyin", "rules"}, std::move(rules)});
        result.changes.push_back({{"fuzzy_pinyin", "seeded"}, true});
      }
    } else if (control == shift_letter_modes && notification == BN_CLICKED) {
      result.changes.push_back({{"local_modes", "shift_letter_entry"}, SendMessageW(shift_, BM_GETCHECK, 0, 0) == BST_CHECKED});
    } else if (mode && notification == BN_CLICKED) {
      const size_t index = static_cast<size_t>(control - local_mode_first);
      if (modes_[index] && IsWindowEnabled(modes_[index]))
        result.changes.push_back({{"local_modes", id(input::local_modes[index].id)}, SendMessageW(modes_[index], BM_GETCHECK, 0, 0) == BST_CHECKED});
    } else if (rule && notification == BN_CLICKED) {
      const size_t index = static_cast<size_t>(control - fuzzy_rule_first);
      if (!IsWindowEnabled(rules_controls_[index])) return result;
      const auto value = id(input::fuzzy_rules[index].id);
      auto rules = fuzzy_value_.at("rules");
      rules.erase(std::remove(rules.begin(), rules.end(), value), rules.end());
      if (SendMessageW(rules_controls_[index], BM_GETCHECK, 0, 0) == BST_CHECKED) rules.push_back(value);
      result.changes.push_back({{"fuzzy_pinyin", "rules"}, std::move(rules)});
    } else if (control == frequency_mode && notification == CBN_SELCHANGE) {
      const auto index = SendMessageW(frequency_, CB_GETCURSEL, 0, 0);
      if (index >= 0 && static_cast<size_t>(index) < frequency_values.size())
        result.changes.push_back({{"frequency", "mode"}, frequency_values[static_cast<size_t>(index)]});
    } else if ((control == frequency_trigger || control == frequency_step) && notification == CBN_SELCHANGE) {
      const auto index = SendMessageW(control == frequency_trigger ? trigger_ : step_, CB_GETCURSEL, 0, 0);
      if (index >= 0 && index < 6)
        result.changes.push_back({{"frequency", control == frequency_trigger ? "trigger_count" : "linear_step"}, index + 1});
    }
    return result;
  }

  static std::vector<Change> reset() {
    // 清空时保留 seeded，再打开不重新勾回全部规则。
    return {{{"fuzzy_pinyin", "enabled"}, false}, {{"fuzzy_pinyin", "rules"}, Json::array()}};
  }

private:
  static std::string id(std::wstring_view value) {
    std::string result;
    for (const auto letter : value) result.push_back(static_cast<char>(letter));
    return result;
  }

  static bool translation_available(const Json& preferences) {
    TranslationServiceChoice choice;
    const auto group = [&preferences](const char* key) {
      return preferences.contains(key) && preferences.at(key).is_object() ? preferences.at(key) : Json::object();
    };
    choice.account = preferences.value("translation_account", false);
    choice.niutrans = group("niutrans").value("enabled", false);
    choice.custom = group("custom_translation").value("enabled", false);
    const auto tencent = group("tencent_tmt");
    choice.tencent = tencent.value("enabled", false);
    choice.tencent_secret_id = control_text::wide(tencent.value("secret_id", std::string{}));
    choice.tencent_secret_key = control_text::wide(tencent.value("secret_key", std::string{}));
    return translation_provider(choice) != L"none";
  }

  static constexpr std::array<const char*, 5> frequency_values{{"disabled", "pin", "halve", "linear", "promote"}};
  HelpcodeForm helpcode_;
  HWND shift_ = nullptr;
  HWND fuzzy_ = nullptr;
  HWND reset_ = nullptr;
  HWND frequency_ = nullptr;
  HWND trigger_ = nullptr;
  HWND step_ = nullptr;
  std::array<HWND, input::local_modes.size()> modes_{};
  std::array<HWND, input::local_modes.size()> descriptions_{};
  std::array<HWND, input::fuzzy_rules.size()> rules_controls_{};
  Json modes_value_ = Json::object();
  Json fuzzy_value_ = Json::object();
  Json frequency_value_ = Json::object();
  bool learning_ = false;
  bool enabled_ = false;
};

} // namespace msime::settings::win7
