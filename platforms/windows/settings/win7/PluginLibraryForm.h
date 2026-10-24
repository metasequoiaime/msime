#pragma once

#include "PluginData.h"
#include "HelpcodeForm.h"
#include <commctrl.h>
#include <functional>
#include <map>

namespace msime::settings::win7 {
class PluginLibraryForm {
public:
  enum Id {
    search = 2450, filter, find_button, refresh_button, packages_list, detail, import_file,
    import_folder, use_pack, stop_pack, earlier, later, remove_pack, notice, missing, clear_missing,
  };
  struct Command {
    bool handled = false, file = false, folder = false;
    std::optional<Json> request;
    std::vector<Change> changes;
    std::wstring confirmation;
  };
  using Model = plugin_preferences::model::Preferences;
  using Package = plugin_preferences::model::Package;
  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row, int top) {
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      controls_[id] = row(type, title, style, id, top, height);
      top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
    };
    add(search, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    SendMessageW(get(search), EM_SETCUEBANNER, TRUE, reinterpret_cast<LPARAM>(L"搜索插件名称、描述或标识"));
    SendMessageW(get(search), EM_SETLIMITTEXT, 1024, 0);
    add(filter, WC_COMBOBOXW, L"插件类型", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST, 320);
    SendMessageW(get(filter), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(L"全部类型"));
    for (const auto kind : plugin_preferences::model::kind_order) {
      const auto label = plugin_preferences::model::kind_label(kind);
      SendMessageW(get(filter), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
    }
    SendMessageW(get(filter), CB_SETCURSEL, 0, 0);
    add(find_button, L"BUTTON", L"筛选已安装插件", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(refresh_button, L"BUTTON", L"刷新插件库", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(packages_list, L"LISTBOX", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 220);
    add(detail, L"EDIT", L"", WS_BORDER | ES_READONLY | ES_MULTILINE | WS_VSCROLL, 220);
    add(import_file, L"BUTTON", L"导入插件文件…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(import_folder, L"BUTTON", L"导入插件文件夹…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(use_pack, L"BUTTON", L"选用 / 启用所选插件", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(stop_pack, L"BUTTON", L"停止使用所选插件", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(earlier, L"BUTTON", L"提前所选指令表 / 短语表", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(later, L"BUTTON", L"推后所选指令表 / 短语表", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(remove_pack, L"BUTTON", L"删除所选插件…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(missing, WC_COMBOBOXW, L"缺失的插件选择", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST, 240);
    add(clear_missing, L"BUTTON", L"清除所选缺失插件的引用", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(notice, L"STATIC", L"", SS_LEFT, 180);
    return top;
  }
  static Json catalog_request() { return Json{{"operation", "catalog"}}; }
  void enable(bool enabled) { enabled_ = enabled; update(); }
  void preferences(std::optional<Model> preferences) { preferences_ = std::move(preferences); rebuild_missing(); update(); }
  const std::vector<Package>& packages() const { return packages_; }
  bool loaded() const { return ready_; }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (!enabled_ || !IsWindowEnabled(get(id))) return result;
    if (id == filter && notification == CBN_SELCHANGE) { rebuild(); return result; }
    if (id == packages_list && notification == LBN_SELCHANGE) { show_detail(); update(); return result; }
    if (notification != BN_CLICKED) return result;
    if (id == find_button) { rebuild(); return result; }
    if (id == refresh_button) { result.request = catalog_request(); return result; }
    if (id == import_file || id == import_folder) {
      result.file = id == import_file; result.folder = id == import_folder;
      result.request = Json{{"operation", "import"}}; return result;
    }
    if (!preferences_) return result;
    if (id == clear_missing) {
      const auto index = SendMessageW(get(missing), CB_GETCURSEL, 0, 0);
      if (index >= 0 && static_cast<size_t>(index) < missing_.size()) {
        const auto& entry = missing_[static_cast<size_t>(index)];
        result.changes = plugin_preferences::changes(*preferences_, plugin_preferences::model::missing_dropped(*preferences_, entry));
      }
      return result;
    }
    const auto* pack = selected(); if (!pack) return result;
    if (id == use_pack) result.changes = plugin_preferences::changes(*preferences_, plugin_preferences::model::with_pack_selected(*preferences_, *pack));
    else if (id == stop_pack) {
      auto next = *preferences_;
      if (pack->kind == L"music" && next.music_pack == pack->id) next.music_pack.clear();
      else if (pack->kind == L"effect" && next.effect_pack == pack->id) next.effect_pack.clear();
      else if (pack->kind == L"command_table") next.command_tables = plugin_preferences::model::with_table(next.command_tables, pack->id, false);
      else if (pack->kind == L"phrase_table") next.phrase_tables = plugin_preferences::model::with_table(next.phrase_tables, pack->id, false);
      else if (pack->kind == L"helpcode") {
        if (next.helpcode_pack_quanpin == pack->id) next.helpcode_pack_quanpin.clear();
        if (next.helpcode_pack_shuangpin == pack->id) next.helpcode_pack_shuangpin.clear();
      }
      result.changes = plugin_preferences::changes(*preferences_, next);
    }
    else if (id == remove_pack) {
      result.request = Json{{"operation", "remove"}, {"kind", control_text::utf8(pack->kind).value()}, {"id", control_text::utf8(pack->id).value()}};
      result.changes = plugin_preferences::changes(*preferences_, plugin_preferences::model::without_removed_pack(*preferences_, pack->kind, pack->id));
      result.confirmation = L"删除所选插件，并清除仍指向它的设置？此操作无法撤销。";
    } else if (id == earlier || id == later) {
      auto next = *preferences_;
      auto& list = pack->kind == L"command_table" ? next.command_tables : next.phrase_tables;
      const auto order = plugin_preferences::model::moved_order(list, pack->id, id == earlier ? -1 : 1);
      if (order) { list = *order; result.changes = plugin_preferences::changes(*preferences_, next); }
    }
    return result;
  }
  void completed(const Json& response) {
    std::optional<std::pair<std::wstring, std::wstring>> previous;
    if (const auto* pack = selected()) previous = std::make_pair(pack->kind, pack->id);
    const auto parsed = plugin_data::packages(response);
    packages_ = parsed.value_or(std::vector<Package>{}); ready_ = parsed.has_value();
    if (!ready_) SetWindowTextW(get(notice), L"插件目录无法读取，请刷新后重试。现有选择保持不变。");
    else {
      std::wstring issues;
      const auto value = theme_data::value(response);
      for (const auto& issue : value->at("issues")) {
        if (!issue.is_object() || !issue.contains("folder") || !issue.at("folder").is_string() || !issue.contains("reason") || !issue.at("reason").is_string()) continue;
        issues += control_text::wide(issue.at("folder").get<std::string>()) + L"：" + control_text::wide(issue.at("reason").get<std::string>()) + L"\r\n";
      }
      SetWindowTextW(get(notice), issues.empty() ? L"" : (L"无法载入的插件：\r\n" + issues).c_str());
    }
    rebuild(previous); rebuild_missing(); update();
  }
  void failure() { SetWindowTextW(get(notice), L"插件操作失败，请检查文件、包类型或当前授权后刷新。原有设置不会自动重置。"); }
private:
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  const Package* selected() const {
    const auto index = SendMessageW(get(packages_list), LB_GETCURSEL, 0, 0);
    return ready_ && index >= 0 && static_cast<size_t>(index) < shown_.size() && shown_[static_cast<size_t>(index)] < packages_.size() ? &packages_[shown_[static_cast<size_t>(index)]] : nullptr;
  }
  void rebuild(std::optional<std::pair<std::wstring, std::wstring>> previous = {}) {
    if (!previous) if (const auto* pack = selected()) previous = std::make_pair(pack->kind, pack->id);
    shown_.clear(); SendMessageW(get(packages_list), LB_RESETCONTENT, 0, 0);
    const auto query = control_text::wide(control_text::read(get(search), 1024).value_or(""));
    const auto category = SendMessageW(get(filter), CB_GETCURSEL, 0, 0);
    for (size_t index = 0; index < packages_.size(); ++index) {
      const auto& pack = packages_[index];
      if (category > 0 && static_cast<size_t>(category) <= plugin_preferences::model::kind_order.size() && pack.kind != plugin_preferences::model::kind_order[static_cast<size_t>(category) - 1]) continue;
      if (!query.empty() && (pack.name + L" " + pack.description + L" " + pack.id).find(query) == std::wstring::npos) continue;
      const auto label = plugin_preferences::model::pack_kind_label(pack) + L" · " + plugin_preferences::model::pack_label(pack);
      SendMessageW(get(packages_list), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
      if (previous && pack.kind == previous->first && pack.id == previous->second) SendMessageW(get(packages_list), LB_SETCURSEL, shown_.size(), 0);
      shown_.push_back(index);
    }
    show_detail(); update();
  }
  void show_detail() {
    const auto* pack = selected();
    auto label = pack ? pack->name + L"\r\n" + plugin_preferences::model::pack_row_description(*pack) + L"\r\n许可：" + pack->license +
        L"\r\n" + pack->description + L"\r\n标识：" + pack->id : std::wstring{};
    if (pack) {
      for (const auto& track : pack->tracks) label += L"\r\n曲目：" + track;
      for (const auto& command : pack->commands) label += L"\r\n/" + command.trigger + L" " + command.title + L"：" + command.template_text;
      for (const auto& phrase : pack->phrases) label += L"\r\n" + phrase.key + L"：" + phrase.text;
      for (const auto& entry : pack->preview) label += L"\r\n" + entry.character + L"：" + entry.code;
      for (const auto& word : pack->first_words) label += L"\r\n单词：" + word;
      for (const auto& group : pack->groups) label += L"\r\n" + group.title + L"：" + plugin_preferences::model::joined(group.items, L" ");
      if (!pack->style.empty()) label += L"\r\n效果样式：" + plugin_preferences::model::effect_style_label(pack->style);
      if (pack->intensity) label += L"\r\n效果强度：" + plugin_preferences::model::js_number(*pack->intensity) + L"%";
      if (!pack->colors.empty()) label += L"\r\n颜色：" + plugin_preferences::model::joined(pack->colors, L"、");
      if (pack->kind == L"script") label += L"\r\n在下方「脚本插件（实验）」中查看授权和执行顺序。";
    }
    SetWindowTextW(get(detail), label.c_str());
  }
  void rebuild_missing() {
    const std::set<std::wstring> kinds(plugin_preferences::model::kind_order.begin(), plugin_preferences::model::kind_order.end());
    missing_ = ready_ && preferences_ ? plugin_preferences::model::missing_selections(*preferences_, packages_, kinds) : std::vector<plugin_preferences::model::MissingSelection>{};
    SendMessageW(get(missing), CB_RESETCONTENT, 0, 0);
    for (const auto& entry : missing_) {
      const auto label = plugin_preferences::model::kind_label(entry.kind) + L" · " + entry.id;
      SendMessageW(get(missing), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
    }
    if (!missing_.empty()) SendMessageW(get(missing), CB_SETCURSEL, 0, 0);
  }
  void update() {
    for (const auto& control : controls_) EnableWindow(control.second, enabled_);
    const auto* pack = selected();
    const bool usable = enabled_ && preferences_ && pack;
    const bool activatable = usable && pack->kind != L"wordbook" && pack->kind != L"symbol_set" && pack->kind != L"script" && (pack->kind != L"helpcode" || HelpcodeForm::offered());
    const bool full = usable && ((pack->kind == L"command_table" && !plugin_preferences::model::contains(preferences_->command_tables, pack->id) && preferences_->command_tables.size() >= plugin_preferences::model::max_command_tables) ||
        (pack->kind == L"phrase_table" && !plugin_preferences::model::contains(preferences_->phrase_tables, pack->id) && preferences_->phrase_tables.size() >= plugin_preferences::model::max_phrase_tables));
    EnableWindow(get(use_pack), activatable && !full); EnableWindow(get(stop_pack), activatable && pack->kind != L"sound");
    EnableWindow(get(remove_pack), usable && !pack->builtin);
    const bool table = usable && (pack->kind == L"command_table" || pack->kind == L"phrase_table");
    EnableWindow(get(earlier), table); EnableWindow(get(later), table);
    EnableWindow(get(packages_list), enabled_ && ready_);
    EnableWindow(get(clear_missing), enabled_ && ready_ && preferences_ && !missing_.empty());
  }
  std::map<int, HWND> controls_;
  std::vector<Package> packages_;
  std::vector<size_t> shown_;
  std::vector<plugin_preferences::model::MissingSelection> missing_;
  std::optional<Model> preferences_;
  bool enabled_ = false, ready_ = false;
};
} // namespace msime::settings::win7
