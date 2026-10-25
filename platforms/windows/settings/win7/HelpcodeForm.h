#pragma once

#include "ControlText.h"
#include "PreferencesStore.h"
#include "SettingsWindow.h"
#include "../../../../shared/contracts/msime_edition.h"
#include <commctrl.h>
#include <algorithm>
#include <array>
#include <functional>

namespace msime::settings::win7 {

struct HelpcodeOption { std::string value; std::wstring label; };
struct HelpcodeSources {
  std::vector<HelpcodeOption> custom;
  std::vector<HelpcodeOption> packs;
};

class HelpcodeForm {
public:
  static bool offered() {
    for (std::string_view scheme : {MSIME_EDITION_INPUT_SCHEMES})
      if (scheme == "quanpin" || scheme == "shuangpin") return true;
    return false;
  }

  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row, int top) {
    if (!offered()) return top;
    row(L"STATIC", L"辅助码", SS_LEFT, -1, top, 28);
    top += 42;
    for (size_t index = 0; index < families_.size(); ++index) {
      auto& family = families_[index];
      const int base = helpcode_first + static_cast<int>(index) * 3;
      family.enabled = row(L"BUTTON", index == 0 ? L"全拼辅助码" : L"双拼辅助码",
          WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY, base, top, 28);
      family.schema = row(WC_COMBOBOXW, index == 0 ? L"全拼辅助码方案" : L"双拼辅助码方案",
          WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, base + 1, top + 40, 240);
      family.show = row(L"BUTTON", index == 0 ? L"在候选窗口中显示全拼辅助码" : L"在候选窗口中显示双拼辅助码",
          WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY, base + 2, top + 84, 28);
      top += 138;
    }
    return top;
  }

  static bool valid(const Json& preferences) {
    if (!offered()) return true;
    for (const auto* name : {"quanpin_helpcode", "shuangpin_helpcode"}) {
      if (!preferences.contains(name) || !preferences.at(name).is_object()) return false;
      const auto& group = preferences.at(name);
      for (const auto* key : {"enabled", "show_in_candidate_window"})
        if (!group.contains(key) || !group.at(key).is_boolean()) return false;
      if (!group.contains("schema") || !group.at("schema").is_string()) return false;
    }
    if (preferences.contains("plugins")) {
      const auto& plugins = preferences.at("plugins");
      if (!plugins.is_object()) return false;
      for (const auto* key : {"helpcode_pack_quanpin", "helpcode_pack_shuangpin"})
        if (plugins.contains(key) && !plugins.at(key).is_string()) return false;
    }
    return true;
  }

  void refresh(const Json& preferences, const HelpcodeSources& sources) {
    if (!offered()) return;
    plugins_ = preferences.value("plugins", Json::object());
    for (size_t index = 0; index < families_.size(); ++index) {
      auto& family = families_[index];
      const auto& group = preferences.at(group_key(index));
      family.on = group.at("enabled").get<bool>();
      SendMessageW(family.enabled, BM_SETCHECK, family.on ? BST_CHECKED : BST_UNCHECKED, 0);
      SendMessageW(family.show, BM_SETCHECK, group.at("show_in_candidate_window").get<bool>() ? BST_CHECKED : BST_UNCHECKED, 0);
      family.options = {{"lantian", L"蓝天小雨点"}, {"ziranma", L"自然码"},
          {"shouyou2_0", L"首右2.0"}, {"shouyouplus", L"首右plus"},
          {"xiaohe", L"小鹤"}, {"jiajia", L"加加"}, {"wubi86", L"五笔 86"}};
      family.options.insert(family.options.end(), sources.custom.begin(), sources.custom.end());
      const auto schema = group.at("schema").get<std::string>();
      const auto pack = plugins_.value(pack_key(index), std::string{});
      const auto contains = [&](const std::string& value) {
        return std::any_of(family.options.begin(), family.options.end(), [&](const HelpcodeOption& option) { return option.value == value; });
      };
      // 资源暂时缺失时仍展示当前选项，进入页面不能替用户换方案。
      if (!contains(schema)) family.options.push_back({schema, control_text::wide(schema)});
      for (const auto& option : sources.packs)
        family.options.push_back({"pack:" + option.value, option.label + L"（插件）"});
      if (!pack.empty() && !contains("pack:" + pack))
        family.options.push_back({"pack:" + pack, control_text::wide(pack) + L"（插件，未找到）"});
      const auto selected = pack.empty() ? schema : "pack:" + pack;
      SendMessageW(family.schema, CB_RESETCONTENT, 0, 0);
      for (size_t option = 0; option < family.options.size(); ++option) {
        SendMessageW(family.schema, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(family.options[option].label.c_str()));
        if (family.options[option].value == selected) SendMessageW(family.schema, CB_SETCURSEL, option, 0);
      }
    }
  }

  void enable(bool enabled) {
    enabled_ = enabled;
    for (const auto& family : families_) {
      if (!family.enabled) continue;
      EnableWindow(family.enabled, enabled);
      EnableWindow(family.schema, enabled && family.on);
      EnableWindow(family.show, enabled);
    }
  }

  std::vector<Change> command(WPARAM event) const {
    if (!offered() || !enabled_) return {};
    const int offset = LOWORD(event) - helpcode_first;
    if (offset < 0 || offset >= 6) return {};
    const size_t index = static_cast<size_t>(offset / 3);
    const auto& family = families_[index];
    if (offset % 3 != 1 && HIWORD(event) == BN_CLICKED)
      return {{{group_key(index), offset % 3 == 0 ? "enabled" : "show_in_candidate_window"},
          SendMessageW(offset % 3 == 0 ? family.enabled : family.show, BM_GETCHECK, 0, 0) == BST_CHECKED}};
    if (offset % 3 != 1 || HIWORD(event) != CBN_SELCHANGE || !IsWindowEnabled(family.schema)) return {};
    const auto selected = SendMessageW(family.schema, CB_GETCURSEL, 0, 0);
    if (selected < 0 || static_cast<size_t>(selected) >= family.options.size()) return {};
    const auto& value = family.options[static_cast<size_t>(selected)].value;
    auto plugins = plugins_;
    if (value.compare(0, 5, "pack:") == 0) {
      plugins[pack_key(index)] = value.substr(5);
      return {{{"plugins"}, std::move(plugins)}};
    }
    std::vector<Change> changes{{{group_key(index), "schema"}, value}};
    if (!plugins.value(pack_key(index), std::string{}).empty()) {
      plugins[pack_key(index)] = "";
      changes.push_back({{"plugins"}, std::move(plugins)});
    }
    return changes;
  }

private:
  static const char* group_key(size_t index) { return index == 0 ? "quanpin_helpcode" : "shuangpin_helpcode"; }
  static const char* pack_key(size_t index) { return index == 0 ? "helpcode_pack_quanpin" : "helpcode_pack_shuangpin"; }
  struct Family {
    HWND enabled = nullptr;
    HWND schema = nullptr;
    HWND show = nullptr;
    bool on = false;
    std::vector<HelpcodeOption> options;
  };
  std::array<Family, 2> families_{};
  Json plugins_ = Json::object();
  bool enabled_ = false;
};

} // namespace msime::settings::win7
