#pragma once

#include "PluginPreferencesModel.h"
#include "ThemeData.h"
#include <commctrl.h>
#include <functional>
#include <map>

namespace msime::settings::win7 {
class PluginMentionsForm {
public:
  enum Id { entries = 2600, text, key, phone, new_entry, apply_entry, remove_entry, save, reload, notice };
  struct Command { bool handled = false; std::optional<Json> request; std::wstring confirmation; };
  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row, int top) {
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      controls_[id] = row(type, title, style, id, top, height); top += height + 12;
    };
    row(L"STATIC", L"@ 名单（名字、地点和联系人，仅保存在本机）", SS_LEFT, -1, top, 36); top += 48;
    add(entries, L"LISTBOX", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 180);
    for (const auto& field : std::vector<std::pair<int, const wchar_t*>>{{text, L"名字或地点"}, {key, L"拼音（可留空，例如 zhang'san）"}, {phone, L"联系人手机号（可留空）"}}) {
      row(L"STATIC", field.second, SS_LEFT, -1, top, 24); top += 36;
      add(field.first, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
      SendMessageW(get(field.first), EM_SETLIMITTEXT, field.first == text ? plugin_preferences::model::max_mention_text_utf16 : 64, 0);
    }
    add(new_entry, L"BUTTON", L"新建条目", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(apply_entry, L"BUTTON", L"加入 / 更新草稿", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(remove_entry, L"BUTTON", L"从草稿移除所选条目", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(save, L"BUTTON", L"保存 @ 名单", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(reload, L"BUTTON", L"重新读取 @ 名单…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(notice, L"STATIC", L"", SS_LEFT, 90);
    return top;
  }
  static Json load_request() { return {{"operation", "load_mentions"}}; }
  void enable(bool enabled) { enabled_ = enabled; update(); }
  void completed(const Json& response, std::string_view operation) {
    const auto value = theme_data::value(response);
    if (operation == "save_mentions") {
      if (!value) { SetWindowTextW(get(notice), L"名单未保存，草稿仍保留。请检查条目或目录权限后重试。"); return; }
      dirty_ = false; SetWindowTextW(get(notice), L"名单已保存。"); update(); return;
    }
    if (!value || !value->is_array()) { ready_ = false; SetWindowTextW(get(notice), L"名单无法读取，原文件保持不变；请修复后重新读取。"); update(); return; }
    std::vector<plugin_preferences::model::Mention> parsed;
    std::vector<std::string> phones;
    for (const auto& item : *value) {
      if (!item.is_object() || !item.contains("text") || !item.at("text").is_string() ||
          (item.contains("key") && !item.at("key").is_string()) || (item.contains("phone") && !item.at("phone").is_string())) {
        ready_ = false; SetWindowTextW(get(notice), L"名单格式无效，原文件保持不变。"); update(); return;
      }
      parsed.push_back({control_text::wide(item.at("text").get<std::string>()), control_text::wide(item.value("key", std::string{}))});
      phones.push_back(item.value("phone", std::string{}));
    }
    mentions_ = std::move(parsed); phones_ = std::move(phones); dirty_ = false; ready_ = true;
    rebuild(); clear(); SetWindowTextW(get(notice), L""); update();
  }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (!enabled_ || !IsWindowEnabled(get(id))) return result;
    if (id == entries && notification == LBN_SELCHANGE) { show(); update(); return result; }
    if (notification != BN_CLICKED) return result;
    if (id == reload) { result.request = load_request(); if (dirty_) result.confirmation = L"放弃 @ 名单的未保存草稿并重新读取？"; return result; }
    if (!ready_) return result;
    if (id == new_entry) { clear(); update(); return result; }
    const auto index = selected();
    if (id == apply_entry) {
      const auto name = control_text::read(get(text), 199), pinyin = control_text::read(get(key), 64), number = control_text::read(get(phone), 64);
      if (!name || !pinyin || !number) { SetWindowTextW(get(notice), L"条目无法读取，请检查长度。"); return result; }
      auto next = mentions_; auto phones = phones_;
      const plugin_preferences::model::Mention mention{control_text::wide(*name), control_text::wide(*pinyin)};
      if (index) { next[*index] = mention; phones[*index] = *number; }
      else { next.push_back(mention); phones.push_back(*number); }
      if (const auto issue = plugin_preferences::model::mention_list_issue(next)) { SetWindowTextW(get(notice), issue->c_str()); return result; }
      mentions_ = plugin_preferences::model::trimmed_mentions(next); phones_ = std::move(phones); dirty_ = true;
      rebuild(); clear(); SetWindowTextW(get(notice), L"草稿已更新，点击保存后生效。"); update();
    } else if (id == remove_entry && index) {
      mentions_.erase(mentions_.begin() + static_cast<std::ptrdiff_t>(*index)); phones_.erase(phones_.begin() + static_cast<std::ptrdiff_t>(*index));
      dirty_ = true; rebuild(); clear(); update();
    } else if (id == save && dirty_) {
      if (const auto issue = plugin_preferences::model::mention_list_issue(mentions_)) { SetWindowTextW(get(notice), issue->c_str()); return result; }
      Json rows = Json::array();
      for (size_t i = 0; i < mentions_.size(); ++i) rows.push_back({{"text", control_text::utf8(mentions_[i].text).value()}, {"key", control_text::utf8(mentions_[i].key).value()}, {"phone", phones_[i]}});
      result.request = Json{{"operation", "save_mentions"}, {"entries", std::move(rows)}};
    }
    return result;
  }
private:
  HWND get(int id) const { const auto item = controls_.find(id); return item == controls_.end() ? nullptr : item->second; }
  std::optional<size_t> selected() const { const auto index = SendMessageW(get(entries), LB_GETCURSEL, 0, 0); return index >= 0 && static_cast<size_t>(index) < mentions_.size() ? std::make_optional(static_cast<size_t>(index)) : std::nullopt; }
  void clear() { SendMessageW(get(entries), LB_SETCURSEL, static_cast<WPARAM>(-1), 0); for (const int id : {text, key, phone}) SetWindowTextW(get(id), L""); }
  void show() { if (const auto index = selected()) { SetWindowTextW(get(text), mentions_[*index].text.c_str()); SetWindowTextW(get(key), mentions_[*index].key.c_str()); SetWindowTextW(get(phone), control_text::wide(phones_[*index]).c_str()); } }
  void rebuild() { SendMessageW(get(entries), LB_RESETCONTENT, 0, 0); for (const auto& item : mentions_) { const auto label = item.text + L" · " + item.key; SendMessageW(get(entries), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str())); } }
  void update() { for (const auto& item : controls_) EnableWindow(item.second, enabled_ && ready_); EnableWindow(get(reload), enabled_); EnableWindow(get(save), enabled_ && ready_ && dirty_); EnableWindow(get(remove_entry), enabled_ && ready_ && selected().has_value()); }
  std::map<int, HWND> controls_;
  std::vector<plugin_preferences::model::Mention> mentions_;
  std::vector<std::string> phones_;
  bool enabled_ = false, ready_ = false, dirty_ = false;
};
} // namespace msime::settings::win7
