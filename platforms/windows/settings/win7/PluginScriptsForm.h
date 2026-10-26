#pragma once

#include "PluginData.h"
#include <commctrl.h>
#include <functional>
#include <map>

namespace msime::settings::win7 {
class PluginScriptsForm {
public:
  enum Id { scripts = 2500, detail, capabilities, capability_detail, allow_capability, deny_capability, authorize, revoke, earlier, later, refresh, notice,
    settings, setting_choice, setting_text, save_settings, clear_data };
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
    // 清单声明的设置项（共享页的 `ScriptSettingsForm`）：列表里每项一行「标题：值」，选中一项后在下拉框（开关和选项）或文本框（整数和文本）里改，按「保存设置」整份写入。
    row(L"STATIC", plugin_preferences::model::script_settings_note.data(), SS_LEFT, -1, top, 60); top += 72;
    add(settings, L"LISTBOX", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 120);
    add(setting_choice, WC_COMBOBOXW, L"所选设置项的值", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST, 200);
    add(setting_text, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL, 28);
    add(save_settings, L"BUTTON", L"保存设置", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(clear_data, L"BUTTON", L"清除数据…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(refresh, L"BUTTON", L"重新读取插件目录与授权", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(notice, L"STATIC", L"", SS_LEFT, 100);
    return top;
  }
  void enable(bool enabled) { enabled_ = enabled; update(); }
  // 下一次读到目录时选中这个脚本插件（刚从社区装好的那个），读到后 revealed 交出下拉框一次，设置窗口把它滚进视野并给它焦点。
  void select(std::wstring id) { wanted_ = std::move(id); }
  HWND revealed() { return std::exchange(revealed_, false) ? get(scripts) : nullptr; }
  void completed(const Json& catalog, const Json& grants, const Json& response, const Json& states = Json::object(), const std::string& operation = {}) {
    std::wstring previous; if (const auto* pack = selected()) previous = pack->id;
    auto previous_draft = std::move(draft_);
    states_.clear();
    if (states.is_object()) for (const auto& [id, state] : states.items()) if (auto parsed = plugin_data::script_state(state)) states_[control_text::wide(id)] = std::move(*parsed);
    failure_key_.clear();
    if (!wanted_.empty()) { previous = std::exchange(wanted_, {}); revealed_ = true; }
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
      const auto code = response.is_object() ? response.value("error", std::string{}) : std::string{};
      const bool changed = code == "plugin_script_changed";
      std::wstring text = changed ? L"插件内容已经改变，未保存授权；已读取最新内容，请重新选择允许的能力。" : L"插件操作失败，未授予新的权限；请检查后重试。";
      // 设置和清除数据失败时按失败码说明；宿主说出是哪一项设置时，把那一项的标题写在前面。
      if (operation == "set_script_settings" || operation == "clear_script_data") {
        const auto detail = response.is_object() ? control_text::wide(response.value("detail", std::string{})) : std::wstring{};
        text = plugin_preferences::model::error_message(control_text::wide(code), detail, operation == "clear_script_data" ? L"数据没有清除，请重试。" : L"设置没有保存，请重试。");
        if (code == "plugin_script_setting_invalid") failure_key_ = plugin_preferences::model::setting_error_key(detail);
      }
      SetWindowTextW(get(notice), text.c_str());
    } else SetWindowTextW(get(notice), operation == "set_script_settings" ? L"已保存。" : L"");
    // 保存被拒时留着草稿，让用户改了再试。
    if (operation == "set_script_settings" && !theme_data::value(response) && selected() && selected()->id == previous) draft_ = std::move(previous_draft);
    rebuild_settings(); show_setting();
    update();
  }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (!enabled_ || !IsWindowEnabled(get(id))) return result;
    if (id == scripts && notification == CBN_SELCHANGE) { reset(); update(); return result; }
    if (id == capabilities && notification == LBN_SELCHANGE) { describe(); update(); return result; }
    if (id == settings && notification == LBN_SELCHANGE) { show_setting(); update(); return result; }
    if (id == setting_choice && notification == CBN_SELCHANGE) { pick_setting(); return result; }
    if (id == setting_text && notification == EN_CHANGE) { if (!filling_) type_setting(); return result; }
    if (notification != BN_CLICKED) return result;
    if (id == refresh) { result.request = Json{{"operation", "catalog"}}; return result; }
    const auto* pack = selected(); if (!pack || !grants_) return result;
    if (id == save_settings) {
      if (!settings_savable(*pack)) return result;
      Json values = Json::object();
      for (const auto& setting : pack->settings) {
        const auto value = plugin_preferences::model::parsed_setting_value(setting, draft(setting));
        const auto key = control_text::utf8(setting.key).value();
        if (const auto* flag = std::get_if<bool>(&value)) values[key] = *flag;
        else if (const auto* number = std::get_if<std::int64_t>(&value)) values[key] = *number;
        else values[key] = control_text::utf8(std::get<std::wstring>(value)).value();
      }
      result.request = Json{{"operation", "set_script_settings"}, {"id", control_text::utf8(pack->id).value()}, {"values", std::move(values)}};
      return result;
    }
    if (id == clear_data) {
      result.request = Json{{"operation", "clear_script_data"}, {"id", control_text::utf8(pack->id).value()}};
      result.confirmation = plugin_preferences::model::script_clear_confirmation(*pack, storage(*pack));
      return result;
    }
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
      const auto* saved = plugin_preferences::model::script_grant(&*grants_, pack->id);
      result.confirmation = id == revoke ? L"停用「" + pack->name + L"」并撤销它的全部授权？" : L"授权并启用「" + pack->name + L"」？\r\n\r\n允许的能力：";
      // 插件存着数据时，停用会把数据一起删掉（共享页详情的 `revoke`）。
      if (id == revoke && plugin_preferences::model::script_persisting(saved, storage(*pack)))
        result.confirmation = plugin_preferences::model::script_revoke_confirmation(*pack, storage(*pack));
      if (id == authorize) {
        if (granted_.empty()) result.confirmation += L"无";
        for (const auto& capability : granted_) result.confirmation += L"\r\n" + plugin_preferences::model::capability_label(capability).title;
        // 「长期保存数据」的红字说明在这里照抄一遍；这次授权会删掉存下的数据时一并说明，这个确认就是共享页的第二次确认。
        if (plugin_preferences::model::contains(granted_, L"persist_data")) result.confirmation += L"\r\n\r\n注意：" + std::wstring(plugin_preferences::model::persist_data_warning);
        if (plugin_preferences::model::grant_deletes_stored_data(saved, granted_, storage(*pack))) result.confirmation += L"\r\n\r\n" + plugin_preferences::model::grant_dialog_deletion(storage(*pack));
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
      // 数据：随包的只读数据，以及授予过 `persist_data`（或存储里有数据）时已存的大小。
      if (pack->data_bytes > 0) text += L"\r\n自带数据：" + plugin_preferences::model::format_kilobytes(pack->data_bytes) + L"（" + std::wstring(plugin_preferences::model::script_bundled_data_note) + L"）";
      if (plugin_preferences::model::script_persisting(grant, storage(*pack)))
        text += L"\r\n已存数据：" + plugin_preferences::model::format_kilobytes(storage(*pack)) + L"（" + std::wstring(plugin_preferences::model::script_stored_data_note) + L"）";
    }
    SetWindowTextW(get(detail), text.c_str()); rebuild_capabilities(); describe();
    draft_.clear(); rebuild_settings(); show_setting();
  }
  double storage(const Model& pack) const { const auto found = states_.find(pack.id); return found == states_.end() ? 0 : found->second.storage_bytes; }
  const plugin_preferences::model::ScriptPluginState* state(const Model& pack) const { const auto found = states_.find(pack.id); return found == states_.end() ? nullptr : &found->second; }
  // 草稿里一项的值：没改过时是读到的值（数字项是原文）。
  plugin_preferences::model::ScriptSettingValue draft(const plugin_preferences::model::ScriptSetting& setting) const {
    if (const auto found = draft_.find(setting.key); found != draft_.end()) return found->second;
    const auto* current = selected() ? state(*selected()) : nullptr;
    return plugin_preferences::model::draft_setting_value(setting, current ? current->values : std::map<std::wstring, plugin_preferences::model::ScriptSettingValue>{});
  }
  const plugin_preferences::model::ScriptSetting* selected_setting() const {
    const auto* pack = selected(); const auto index = SendMessageW(get(settings), LB_GETCURSEL, 0, 0);
    return pack && index >= 0 && static_cast<size_t>(index) < pack->settings.size() ? &pack->settings[static_cast<size_t>(index)] : nullptr;
  }
  // 草稿和读到的值不同、页面又查不出问题时才能保存。
  bool settings_savable(const Model& pack) const {
    const auto* current = state(pack); if (!current || pack.settings.empty()) return false;
    bool dirty = false;
    for (const auto& setting : pack.settings) {
      const auto value = plugin_preferences::model::parsed_setting_value(setting, draft(setting));
      if (!plugin_preferences::model::setting_issue(setting, value).empty()) return false;
      dirty = dirty || value != plugin_preferences::model::setting_value(setting, current->values);
    }
    return dirty;
  }
  void rebuild_settings() {
    const auto index = SendMessageW(get(settings), LB_GETCURSEL, 0, 0);
    SendMessageW(get(settings), LB_RESETCONTENT, 0, 0);
    if (const auto* pack = selected(); pack && state(*pack)) for (const auto& setting : pack->settings) {
      const auto value = draft(setting);
      const auto shown = setting.type == L"number" ? std::get<std::wstring>(value) : plugin_preferences::model::setting_value_text(setting, value);
      auto label = setting.title + L"：" + shown;
      const auto issue = plugin_preferences::model::setting_issue(setting, plugin_preferences::model::parsed_setting_value(setting, value));
      if (!issue.empty()) label += L"（" + issue + L"）";
      else if (setting.key == failure_key_) label += L"（值不符合插件的要求）";
      SendMessageW(get(settings), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
    }
    const auto count = SendMessageW(get(settings), LB_GETCOUNT, 0, 0);
    if (count > 0) SendMessageW(get(settings), LB_SETCURSEL, index >= 0 && index < count ? index : 0, 0);
  }
  // 选中一项设置之后，把它的值放进下拉框（开关和选项）或文本框（整数和文本）。
  void show_setting() {
    filling_ = true;
    SendMessageW(get(setting_choice), CB_RESETCONTENT, 0, 0); SetWindowTextW(get(setting_text), L"");
    if (const auto* setting = selected_setting()) {
      const auto value = draft(*setting);
      if (setting->type == L"bool") {
        for (const auto* label : {L"开", L"关"}) SendMessageW(get(setting_choice), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label));
        const auto* flag = std::get_if<bool>(&value); SendMessageW(get(setting_choice), CB_SETCURSEL, flag && *flag ? 0 : 1, 0);
      } else if (setting->type == L"choice") {
        const auto* current = std::get_if<std::wstring>(&value);
        for (size_t index = 0; index < setting->options.size(); ++index) {
          SendMessageW(get(setting_choice), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(setting->options[index].title.c_str()));
          if (current && setting->options[index].value == *current) SendMessageW(get(setting_choice), CB_SETCURSEL, index, 0);
        }
      } else {
        SendMessageW(get(setting_text), EM_LIMITTEXT, setting->type == L"text" ? setting->max_length : 32, 0);
        const auto* current = std::get_if<std::wstring>(&value); SetWindowTextW(get(setting_text), current ? current->c_str() : L"");
      }
    }
    filling_ = false;
  }
  void pick_setting() {
    const auto* setting = selected_setting(); if (!setting) return;
    const auto index = SendMessageW(get(setting_choice), CB_GETCURSEL, 0, 0); if (index < 0) return;
    if (setting->type == L"bool") draft_[setting->key] = index == 0;
    else if (setting->type == L"choice" && static_cast<size_t>(index) < setting->options.size()) draft_[setting->key] = setting->options[static_cast<size_t>(index)].value;
    else return;
    edited(*setting);
  }
  void type_setting() {
    const auto* setting = selected_setting(); if (!setting || (setting->type != L"number" && setting->type != L"text")) return;
    draft_[setting->key] = control_text::wide(control_text::read(get(setting_text)).value_or(std::string{}));
    edited(*setting);
  }
  void edited(const plugin_preferences::model::ScriptSetting& setting) {
    if (setting.key == failure_key_) failure_key_.clear();
    rebuild_settings(); update();
  }
  void rebuild_capabilities() {
    SendMessageW(get(capabilities), LB_RESETCONTENT, 0, 0);
    // 每行前面写它所在的分组（读文字、改文字、记住数据），同共享页授权对话框的分组。
    if (const auto* pack = selected()) for (const auto& item : plugin_preferences::model::grant_items(pack->permissions, granted_, false, plugin_preferences::model::has_extended_command(*pack))) {
      const auto label = std::wstring(item.checked ? L"[允许] " : L"[未允许] ") + item.group + L"：" + item.title;
      SendMessageW(get(capabilities), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
    }
  }
  void describe() {
    std::wstring text;
    if (const auto* pack = selected()) {
      if (pack->permissions.empty()) text = plugin_preferences::model::grant_dialog_no_permissions(*pack);
      else if (!pack->settings.empty()) text = plugin_preferences::model::grant_dialog_settings_note;
      const auto items = plugin_preferences::model::grant_items(pack->permissions, granted_, false, plugin_preferences::model::has_extended_command(*pack));
      const auto index = SendMessageW(get(capabilities), LB_GETCURSEL, 0, 0);
      // 「长期保存数据」的说明是警告：这里画不出红字，前面写「注意：」。
      if (index >= 0 && static_cast<size_t>(index) < items.size()) text = (items[static_cast<size_t>(index)].warning ? L"注意：" : L"") + items[static_cast<size_t>(index)].description;
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
    const auto* setting = enabled ? selected_setting() : nullptr;
    EnableWindow(get(settings), enabled && SendMessageW(get(settings), LB_GETCOUNT, 0, 0) > 0);
    EnableWindow(get(setting_choice), setting && (setting->type == L"bool" || setting->type == L"choice"));
    EnableWindow(get(setting_text), setting && (setting->type == L"number" || setting->type == L"text"));
    EnableWindow(get(save_settings), enabled && settings_savable(*pack));
    EnableWindow(get(clear_data), enabled && state(*pack) && storage(*pack) > 0);
  }
  std::map<int, HWND> controls_;
  std::vector<Model> packages_;
  std::vector<size_t> indices_;
  std::optional<plugin_preferences::model::ScriptGrants> grants_;
  std::vector<std::wstring> granted_;
  std::wstring wanted_;
  bool enabled_ = false, revealed_ = false;
  // 每个脚本插件的设置值和存储大小（按 id），选中插件的设置草稿，宿主拒绝的那一项设置；filling_ 为真时是页面自己在填文本框，不算编辑。
  std::map<std::wstring, plugin_preferences::model::ScriptPluginState> states_;
  std::map<std::wstring, plugin_preferences::model::ScriptSettingValue> draft_;
  std::wstring failure_key_;
  bool filling_ = false;
};
} // namespace msime::settings::win7
