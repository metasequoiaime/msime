#pragma once

#include "PluginData.h"
#include <commctrl.h>
#include <functional>
#include <map>

namespace msime::settings::win7 {
class PluginScriptsForm {
public:
  enum Id { scripts = 2500, detail, capabilities, capability_detail, allow_capability, deny_capability, authorize, revoke, earlier, later, refresh, notice };
  struct Command { bool handled = false; std::optional<Json> request; std::wstring confirmation; };
  using Model = plugin_preferences::model::Package;
  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row, int top) {
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      controls_[id] = row(type, title, style, id, top, height); top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
    };
    row(L"STATIC", L"脚本插件（实验）", SS_LEFT, -1, top, 28); top += 40;
    row(L"STATIC", plugin_preferences::model::script_group_note().c_str(), SS_LEFT, -1, top, 150); top += 162;
    add(scripts, WC_COMBOBOXW, L"选择脚本插件", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST, 300);
    add(detail, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_READONLY | ES_MULTILINE | WS_VSCROLL, 220);
    row(L"STATIC", plugin_preferences::model::grant_dialog_intro.data(), SS_LEFT, -1, top, 90); top += 102;
    add(capabilities, L"LISTBOX", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 180);
    add(capability_detail, L"STATIC", L"", SS_LEFT, 100);
    add(allow_capability, L"BUTTON", L"允许所选能力", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(deny_capability, L"BUTTON", L"取消所选能力", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(authorize, L"BUTTON", L"确认授权并启用…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(revoke, L"BUTTON", L"停用并撤销全部授权…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(earlier, L"BUTTON", L"提前执行所选脚本", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(later, L"BUTTON", L"推后执行所选脚本", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(refresh, L"BUTTON", L"重新读取插件目录与授权", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(notice, L"STATIC", L"", SS_LEFT, 100);
    return top;
  }
  void enable(bool enabled) { enabled_ = enabled; update(); }
  void completed(const Json& catalog, const Json& grants, const Json& response) {
    std::wstring previous; if (const auto* pack = selected()) previous = pack->id;
    const auto packages = plugin_data::packages(catalog); grants_ = plugin_data::grants(grants);
    packages_ = packages.value_or(std::vector<Model>{});
    indices_.clear(); SendMessageW(get(scripts), CB_RESETCONTENT, 0, 0);
    for (size_t index = 0; index < packages_.size(); ++index) if (packages_[index].kind == L"script") {
      const auto& pack = packages_[index];
      const auto label = plugin_preferences::model::pack_label(pack) + L" · " + plugin_preferences::model::script_marker(pack, grants_ ? &*grants_ : nullptr, plugin_preferences::model::running_script_ids(grants_ ? &*grants_ : nullptr, packages_));
      SendMessageW(get(scripts), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
      if (pack.id == previous) SendMessageW(get(scripts), CB_SETCURSEL, indices_.size(), 0);
      indices_.push_back(index);
    }
    if (SendMessageW(get(scripts), CB_GETCURSEL, 0, 0) < 0 && !indices_.empty()) SendMessageW(get(scripts), CB_SETCURSEL, 0, 0);
    reset();
    if (!packages || !grants_) SetWindowTextW(get(notice), L"目录或授权无法读取，授权操作已禁用；请修复后重新读取。");
    else if (!theme_data::value(response)) {
      const bool changed = response.is_object() && response.value("error", std::string{}) == "plugin_script_changed";
      SetWindowTextW(get(notice), changed ? L"插件内容已经改变，未保存授权；已读取最新内容，请重新选择允许的能力。" : L"插件操作失败，未授予新的权限；请检查后重试。");
    } else SetWindowTextW(get(notice), L"");
    update();
  }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (!enabled_ || !IsWindowEnabled(get(id))) return result;
    if (id == scripts && notification == CBN_SELCHANGE) { reset(); update(); return result; }
    if (id == capabilities && notification == LBN_SELCHANGE) { describe(); update(); return result; }
    if (notification != BN_CLICKED) return result;
    if (id == refresh) { result.request = Json{{"operation", "catalog"}}; return result; }
    const auto* pack = selected(); if (!pack || !grants_) return result;
    if (id == allow_capability || id == deny_capability) {
      const auto index = SendMessageW(get(capabilities), LB_GETCURSEL, 0, 0);
      const auto items = plugin_preferences::model::grant_items(pack->permissions, granted_, false);
      if (index >= 0 && static_cast<size_t>(index) < items.size()) {
        const auto& item = items[static_cast<size_t>(index)];
        if (id == allow_capability && item.disabled) return result;
        granted_ = plugin_preferences::model::toggle_capability(granted_, item.capability, id == allow_capability, pack->permissions);
        rebuild_capabilities(); SendMessageW(get(capabilities), LB_SETCURSEL, index, 0); describe(); update();
      }
    } else if (id == authorize || id == revoke) {
      Json granted = Json::array(); if (id == authorize) for (const auto& capability : granted_) granted.push_back(control_text::utf8(capability).value());
      result.request = Json{{"operation", "set_script_grant"}, {"id", control_text::utf8(pack->id).value()}, {"hash", control_text::utf8(pack->content_hash).value()}, {"enabled", id == authorize}, {"granted", std::move(granted)}};
      result.confirmation = id == revoke ? L"停用「" + pack->name + L"」并撤销它的全部授权？" : L"授权并启用「" + pack->name + L"」？\r\n\r\n允许的能力：";
      if (id == authorize) {
        if (granted_.empty()) result.confirmation += L"无";
        for (const auto& capability : granted_) result.confirmation += L"\r\n" + plugin_preferences::model::capability_label(capability).title;
        result.confirmation += L"\r\n\r\n" + std::wstring(plugin_preferences::model::script_residual_risk);
      }
    } else if (id == earlier || id == later) {
      const auto order = plugin_preferences::model::moved_order(plugin_preferences::model::running_script_ids(&*grants_, packages_), pack->id, id == earlier ? -1 : 1);
      if (order) { Json ids = Json::array(); for (const auto& entry : *order) ids.push_back(control_text::utf8(entry).value()); result.request = Json{{"operation", "reorder_scripts"}, {"ids", std::move(ids)}}; }
    }
    return result;
  }
private:
  HWND get(int id) const { const auto item = controls_.find(id); return item == controls_.end() ? nullptr : item->second; }
  const Model* selected() const { const auto index = SendMessageW(get(scripts), CB_GETCURSEL, 0, 0); return index >= 0 && static_cast<size_t>(index) < indices_.size() && indices_[static_cast<size_t>(index)] < packages_.size() ? &packages_[indices_[static_cast<size_t>(index)]] : nullptr; }
  void reset() {
    granted_.clear(); std::wstring text;
    if (const auto* pack = selected()) {
      const auto* grant = plugin_preferences::model::script_grant(grants_ ? &*grants_ : nullptr, pack->id);
      granted_ = plugin_preferences::model::grant_dialog_initial(*pack, grant);
      text = pack->name + L"\r\n" + plugin_preferences::model::script_status(*pack, plugin_preferences::model::script_grant_state(*pack, grant), plugin_preferences::model::script_health(grants_ ? &*grants_ : nullptr, *pack));
      text += L"\r\n内容哈希：" + pack->content_hash;
      for (const auto& command : pack->commands) text += L"\r\n" + plugin_preferences::model::script_command_line(command, plugin_preferences::model::script_command_status(*pack, command.trigger, grants_ ? &*grants_ : nullptr, packages_));
      const auto running = plugin_preferences::model::running_script_ids(grants_ ? &*grants_ : nullptr, packages_);
      const auto position = plugin_preferences::model::running_position(running, pack->id);
      if (grant && grant->enabled) text += L"\r\n" + plugin_preferences::model::script_order_note(position, running.size());
    }
    SetWindowTextW(get(detail), text.c_str()); rebuild_capabilities(); describe();
  }
  void rebuild_capabilities() {
    SendMessageW(get(capabilities), LB_RESETCONTENT, 0, 0);
    if (const auto* pack = selected()) for (const auto& item : plugin_preferences::model::grant_items(pack->permissions, granted_, false)) {
      const auto label = std::wstring(item.checked ? L"[允许] " : L"[未允许] ") + item.title;
      SendMessageW(get(capabilities), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
    }
  }
  void describe() {
    std::wstring text;
    if (const auto* pack = selected()) {
      if (pack->permissions.empty()) text = plugin_preferences::model::grant_dialog_no_permissions;
      const auto items = plugin_preferences::model::grant_items(pack->permissions, granted_, false);
      const auto index = SendMessageW(get(capabilities), LB_GETCURSEL, 0, 0);
      if (index >= 0 && static_cast<size_t>(index) < items.size()) text = items[static_cast<size_t>(index)].description;
    }
    SetWindowTextW(get(capability_detail), text.c_str());
  }
  void update() {
    const auto* pack = selected(); const bool enabled = enabled_ && pack && grants_;
    for (const auto& item : controls_) EnableWindow(item.second, enabled);
    EnableWindow(get(refresh), enabled_); EnableWindow(get(scripts), enabled_ && !indices_.empty());
    const auto items = pack ? plugin_preferences::model::grant_items(pack->permissions, granted_, false) : std::vector<plugin_preferences::model::GrantItem>{};
    const auto index = SendMessageW(get(capabilities), LB_GETCURSEL, 0, 0);
    const auto* item = index >= 0 && static_cast<size_t>(index) < items.size() ? &items[static_cast<size_t>(index)] : nullptr;
    EnableWindow(get(allow_capability), enabled && item && !item->disabled && !item->checked);
    EnableWindow(get(deny_capability), enabled && item && item->checked);
    const auto running = plugin_preferences::model::running_script_ids(grants_ ? &*grants_ : nullptr, packages_);
    EnableWindow(get(earlier), enabled && plugin_preferences::model::moved_order(running, pack->id, -1).has_value());
    EnableWindow(get(later), enabled && plugin_preferences::model::moved_order(running, pack->id, 1).has_value());
  }
  std::map<int, HWND> controls_;
  std::vector<Model> packages_;
  std::vector<size_t> indices_;
  std::optional<plugin_preferences::model::ScriptGrants> grants_;
  std::vector<std::wstring> granted_;
  bool enabled_ = false;
};
} // namespace msime::settings::win7
