#pragma once

#include "ControlText.h"
#include "PreferencesStore.h"
#include "SettingsWindow.h"
#include "../HelpcodeModesModel.h"
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

// 控件 ID 从 helpcode_first 起：每族三个（开关、方案、显示，`族 * 3 + 0..2`），接着两族的「一码严格筛选」，再接双拼的十一种触发方式，一共 19 个，下一段从 helpcode_first + 20 开始。
class HelpcodeForm {
public:
  static constexpr int strict_first = 6;
  static constexpr int modes_first = strict_first + 2;
  static constexpr int control_count = modes_first + static_cast<int>(helpcode::shuangpin_modes.size());
  static_assert(helpcode_first + control_count <= scheme_language, "helpcode controls overlap the next range");

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
      family.strict = row(L"BUTTON", index == 0 ? L"全拼一码严格筛选" : L"双拼一码严格筛选",
          WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY, helpcode_first + strict_first + static_cast<int>(index), top + 128, 28);
      top += 182;
    }
    // 双拼辅助码的触发方式接在双拼一族后面，顺序和文案见 HelpcodeModesModel.h。
    for (size_t index = 0; index < modes_.size(); ++index) {
      const auto title = L"双拼" + std::wstring(helpcode::shuangpin_modes[index].title);
      modes_[index] = row(L"BUTTON", title.c_str(), WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY,
          helpcode_first + modes_first + static_cast<int>(index), top, 28);
      top += 44;
    }
    return top + 10;
  }

  static bool valid(const Json& preferences) {
    if (!offered()) return true;
    for (const auto* name : {"quanpin_helpcode", "shuangpin_helpcode"}) {
      if (!preferences.contains(name) || !preferences.at(name).is_object()) return false;
      const auto& group = preferences.at(name);
      for (const auto* key : {"enabled", "show_in_candidate_window"})
        if (!group.contains(key) || !group.at(key).is_boolean()) return false;
      if (!group.contains("schema") || !group.at("schema").is_string()) return false;
      // 这两项等于出厂值时 client-core 不写进文档，缺了按出厂值读；在的话类型要对。
      if (group.contains("strict_filter") && !group.at("strict_filter").is_boolean()) return false;
      if (group.contains("modes")) {
        const auto& modes = group.at("modes");
        if (!modes.is_object()) return false;
        for (const auto& mode : helpcode::shuangpin_modes) {
          const auto field = narrow(mode.field);
          if (modes.contains(field) && !modes.at(field).is_boolean()) return false;
        }
      }
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
      SendMessageW(family.strict, BM_SETCHECK, group.value("strict_filter", false) ? BST_CHECKED : BST_UNCHECKED, 0);
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
    // 存下整段 `modes`（可能带着 macOS 同步来的 manual_tab），改一项时在它上面改，别的键原样留着。
    stored_modes_ = preferences.at(group_key(1)).value("modes", Json::object());
    for (size_t index = 0; index < modes_.size(); ++index) {
      mode_values_[index] = mode_value(index);
      SendMessageW(modes_[index], BM_SETCHECK, mode_values_[index] ? BST_CHECKED : BST_UNCHECKED, 0);
    }
  }

  void enable(bool enabled) {
    enabled_ = enabled;
    for (const auto& family : families_) {
      if (!family.enabled) continue;
      EnableWindow(family.enabled, enabled);
      EnableWindow(family.schema, enabled && family.on);
      EnableWindow(family.show, enabled);
      EnableWindow(family.strict, enabled && family.on);
    }
    const auto on = [this](std::wstring_view field) {
      for (size_t index = 0; index < modes_.size(); ++index)
        if (helpcode::shuangpin_modes[index].field == field) return mode_values_[index];
      return false;
    };
    for (size_t index = 0; index < modes_.size(); ++index)
      if (modes_[index])
        EnableWindow(modes_[index], enabled && helpcode::mode_enabled(helpcode::shuangpin_modes[index],
            families_[1].on, on(L"direct"), on(L"mid_sentence"), on(L"direct_helpcode")));
  }

  std::vector<Change> command(WPARAM event) const {
    if (!offered() || !enabled_) return {};
    const int offset = LOWORD(event) - helpcode_first;
    if (offset < 0 || offset >= control_count) return {};
    if (offset >= strict_first) {
      if (HIWORD(event) != BN_CLICKED) return {};
      if (offset < modes_first) {
        const auto index = static_cast<size_t>(offset - strict_first);
        const HWND strict = families_[index].strict;
        if (!IsWindowEnabled(strict)) return {};
        return {{{group_key(index), "strict_filter"}, SendMessageW(strict, BM_GETCHECK, 0, 0) == BST_CHECKED}};
      }
      // 整段 `modes` 写回去：缺这一段时文档里没有它，只能整段补上；互斥由 client-core 在写入端归一，保存后整页按归一后的值刷新。
      const auto index = static_cast<size_t>(offset - modes_first);
      if (!IsWindowEnabled(modes_[index])) return {};
      auto modes = stored_modes_;
      modes[narrow(helpcode::shuangpin_modes[index].field)] = SendMessageW(modes_[index], BM_GETCHECK, 0, 0) == BST_CHECKED;
      return {{{group_key(1), "modes"}, std::move(modes)}};
    }
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
  static std::string narrow(std::wstring_view value) {
    std::string result;
    for (const auto letter : value) result.push_back(static_cast<char>(letter));
    return result;
  }

  // 文档里的值，缺这一项时按 client-core 的出厂值。
  bool mode_value(size_t index) const {
    const auto& mode = helpcode::shuangpin_modes[index];
    return stored_modes_.value(narrow(mode.field), mode.fallback);
  }

  static const char* group_key(size_t index) { return index == 0 ? "quanpin_helpcode" : "shuangpin_helpcode"; }
  static const char* pack_key(size_t index) { return index == 0 ? "helpcode_pack_quanpin" : "helpcode_pack_shuangpin"; }
  struct Family {
    HWND enabled = nullptr;
    HWND schema = nullptr;
    HWND show = nullptr;
    HWND strict = nullptr;
    bool on = false;
    std::vector<HelpcodeOption> options;
  };
  std::array<Family, 2> families_{};
  std::array<HWND, helpcode::shuangpin_modes.size()> modes_{};
  std::array<bool, helpcode::shuangpin_modes.size()> mode_values_{};
  Json stored_modes_ = Json::object();
  Json plugins_ = Json::object();
  bool enabled_ = false;
};

} // namespace msime::settings::win7
