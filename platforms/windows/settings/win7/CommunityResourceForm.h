#pragma once

#include "ControlText.h"
#include "ThemeData.h"
#include "CommunityResourceData.h"
#include "../CommunityPageModel.h"
#include <commctrl.h>
#include <objbase.h>
#include <functional>
#include <map>
#include <stdexcept>
#include <vector>
#include <algorithm>

namespace msime::settings::win7 {
class CommunityResourceForm {
public:
  enum Id { open = 3100, kind, scope, search, reload, list, more, detail_text, detail, save_copy, stars,
    rate, report_reason, report_detail, report, apply_local, apply_cloud, unpublish, editor_new,
    editor_existing, editor_title, name, description, prompt, entry_kind, entry_code, entry_word,
    entry_weight, entry_add, draft_entries, entry_remove, agree, publish, notice };
  struct Command { bool handled = false; std::optional<Json> request; std::wstring confirmation, message; };
  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row) {
    int top = 0;
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      const auto control = row(type, title, style, id, top, height); if (id >= 0) controls_[id] = control;
      top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
    };
    const auto note = [&](const wchar_t* text, int height = 36) { add(-1, L"STATIC", text, SS_LEFT, height); };
    const auto button = [&](int id, const wchar_t* title) { add(id, L"BUTTON", title, WS_TABSTOP | BS_PUSHBUTTON, 32); };
    const auto combo = [&](int id, const auto& options) {
      add(id, WC_COMBOBOXW, L"", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 240);
      for (const auto& option : options) SendMessageW(get(id), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(option.label.data()));
      SendMessageW(get(id), CB_SETCURSEL, 0, 0);
    };
    combo(kind, community::resource_kinds); combo(scope, community::resource_scopes); note(L"搜索作品");
    add(search, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL); button(reload, L"搜索 / 刷新");
    add(list, L"LISTBOX", L"社区词库与回复模板", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 180);
    button(more, L"加载更多"); add(notice, L"STATIC", L"", SS_LEFT, 100);
    add(detail_text, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY | WS_VSCROLL, 220);
    button(detail, L"读取所选作品详情"); button(save_copy, L"收藏 / 取消收藏");
    note(L"回复模板可收藏与管理；Windows 没有使用回复模板的触屏键盘。", 64);
    button(apply_local, L"导入所选词库到本机…"); button(apply_cloud, L"导入所选词库到账号云词库…");
    add(stars, WC_COMBOBOXW, L"评分", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 220);
    for (const auto title : {L"1 星", L"2 星", L"3 星", L"4 星", L"5 星"}) SendMessageW(get(stars), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(title));
    SendMessageW(get(stars), CB_SETCURSEL, 4, 0); button(rate, L"提交评分"); button(unpublish, L"下架我的作品…");
    note(L"举报所选作品"); add(report_reason, WC_COMBOBOXW, L"举报原因", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 220);
    for (const auto title : community::report_reasons) SendMessageW(get(report_reason), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(title.data()));
    SendMessageW(get(report_reason), CB_SETCURSEL, 0, 0); add(report_detail, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | WS_VSCROLL, 90); button(report, L"提交举报…");
    button(editor_new, L"按当前分类新建作品并清空发布草稿"); button(editor_existing, L"编辑我的所选作品并发布新版本");
    add(editor_title, L"STATIC", L"", SS_LEFT, 44); note(community::resource_publish_warning.data(), 96);
    note(L"名称"); add(name, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    note(L"说明"); add(description, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | WS_VSCROLL, 90);
    note(L"回复模板提示词（仅回复模板使用）"); add(prompt, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | WS_VSCROLL, 130);
    note(L"词库词条（最多 128 条）"); combo(entry_kind, community::shared_word_kinds);
    note(L"编码、词条、权重"); add(entry_code, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    add(entry_word, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL); add(entry_weight, L"EDIT", community::default_shared_word_weight.data(), WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    button(entry_add, L"加入草稿词条"); add(draft_entries, L"LISTBOX", L"发布草稿词条", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 160); button(entry_remove, L"删除所选草稿词条");
    add(agree, L"BUTTON", L"我拥有公开发布所需的权利", WS_TABSTOP | BS_AUTOCHECKBOX); button(publish, L"确认公开发布…");
    SendMessageW(get(search), EM_SETLIMITTEXT, 512, 0); SendMessageW(get(name), EM_SETLIMITTEXT, 128, 0);
    SendMessageW(get(description), EM_SETLIMITTEXT, 560, 0); SendMessageW(get(prompt), EM_SETLIMITTEXT, 4000, 0); SendMessageW(get(report_detail), EM_SETLIMITTEXT, community::report_detail_limit, 0);
    reset_editor(false); enable(false); return top;
  }
  void enable(bool enabled) {
    enabled_ = enabled; for (const auto& item : controls_) EnableWindow(item.second, enabled);
    const bool selected = !selected_.is_null(), owned = selected && selected_.value("owned", false), dictionary = selected && selected_.value("kind", std::string{}) == "dictionary";
    for (const int id : {detail, save_copy, report}) EnableWindow(get(id), enabled && selected);
    for (const int id : {editor_existing, unpublish}) EnableWindow(get(id), enabled && owned);
    for (const int id : {apply_local, apply_cloud}) EnableWindow(get(id), enabled && dictionary);
    EnableWindow(get(rate), enabled && selected && !owned); EnableWindow(get(more), enabled && has_more_);
    for (const int id : {entry_kind, entry_code, entry_word, entry_weight, entry_add, draft_entries, entry_remove}) EnableWindow(get(id), enabled && editor_kind_ == "dictionary");
    EnableWindow(get(prompt), enabled && editor_kind_ == "reply"); EnableWindow(get(publish), enabled && !editor_id_.empty());
  }
  Json list_request(bool more = false) const { return {{"operation", "resource_list"}, {"kind", kind_}, {"scope", scope_}, {"search", search_}, {"offset", more ? offset_ : 0}}; }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event); if (!controls_.count(id)) return {};
    Command result; result.handled = true; if (!enabled_ || !IsWindowEnabled(get(id))) return result;
    if (id == list && notification == LBN_SELCHANGE) { const auto index = SendMessageW(get(list), LB_GETCURSEL, 0, 0); if (index >= 0 && static_cast<size_t>(index) < items_.size()) select(items_[static_cast<size_t>(index)]); return result; }
    if (notification != BN_CLICKED) return result;
    if (id == reload) {
      const auto query = control_text::read(get(search)); if (!query) { result.message = L"无法读取搜索内容。"; return result; }
      kind_ = SendMessageW(get(kind), CB_GETCURSEL, 0, 0) == 1 ? "reply" : "dictionary";
      const auto index = SendMessageW(get(scope), CB_GETCURSEL, 0, 0); scope_ = index >= 0 && static_cast<size_t>(index) < community::resource_scopes.size() ? control_text::utf8(community::resource_scopes[static_cast<size_t>(index)].value).value_or("") : "";
      search_ = control_text::utf8(community::bounded_graphemes(control_text::wide(*query), community::search_limit)).value_or(""); result.request = list_request();
    } else if (id == more && has_more_) result.request = list_request(true);
    else if (id == editor_new || id == editor_existing) reset_editor(id == editor_existing);
    else if (id == entry_add) {
      const auto index = SendMessageW(get(entry_kind), CB_GETCURSEL, 0, 0); if (index < 0 || static_cast<size_t>(index) >= community::shared_word_kinds.size()) return result;
      const auto value = community::shared_word(entries_, community::shared_word_kinds[static_cast<size_t>(index)].value, wide(entry_code), wide(entry_word), wide(entry_weight));
      if (!value) result.message = community::shared_word_invalid; else { entries_.push_back(*value); show_entries(); SetWindowTextW(get(entry_code), L""); SetWindowTextW(get(entry_word), L""); }
    } else if (id == entry_remove) { const auto index = SendMessageW(get(draft_entries), LB_GETCURSEL, 0, 0); if (index >= 0 && static_cast<size_t>(index) < entries_.size()) { entries_.erase(entries_.begin() + index); show_entries(); } }
    else if (id == publish) {
      const auto warning = community::resource_publish_error(control_text::wide(editor_kind_), wide(name), wide(description), wide(prompt), entries_.size(), editor_revision_ > 0, SendMessageW(get(agree), BM_GETCHECK, 0, 0) == BST_CHECKED);
      if (!warning.empty()) { result.message = warning; return result; }
      const auto fields = community::publish_fields(wide(name), wide(description)); Json content = Json::object();
      if (editor_kind_ == "reply") content["prompt"] = control_text::utf8(community::trim(wide(prompt))).value();
      else { content["entries"] = Json::array(); for (const auto& entry : entries_) content["entries"].push_back(word_json(entry)); }
      result.request = Json{{"operation", "resource_publish"}, {"id", editor_id_}, {"kind", editor_kind_}, {"name", control_text::utf8(fields.name).value()}, {"description", control_text::utf8(fields.description).value()}, {"content", std::move(content)}, {"revision", editor_revision_}};
      result.confirmation = L"公开发布「" + fields.name + L"」？发布内容将展示给其他用户。";
    } else if (!selected_.is_null()) {
      const auto publication = selected_.at("id");
      if (id == detail) result.request = Json{{"operation", "resource_detail"}, {"id", publication}};
      if (id == save_copy) result.request = Json{{"operation", "resource_save"}, {"id", publication}, {"saved", !selected_.value("saved", false)}};
      if (id == apply_cloud) { result.request = Json{{"operation", "resource_apply"}, {"id", publication}, {"resource_revision", selected_.at("revision")}}; result.confirmation = L"将所选词库合并到账号云词库？本机输入不会立即改变，云端作品版本变化时会拒绝。"; }
      if (id == apply_local) { result.request = Json{{"operation", "resource_import_local"}, {"entries", selected_.at("content").at("entries")}, {"request_id", local_id_}}; result.confirmation = L"将当前显示的作品词条导入这台设备的词库？账号云词库不改变。"; }
      if (id == unpublish) { result.request = Json{{"operation", "resource_unpublish"}, {"id", publication}}; result.confirmation = L"下架我的作品？已导入本机或云词库的副本保留。"; }
      if (id == rate) { const auto index = SendMessageW(get(stars), CB_GETCURSEL, 0, 0); if (index >= 0 && index < 5) result.request = Json{{"operation", "resource_rate"}, {"id", publication}, {"stars", index + 1}}; }
      if (id == report) {
        const auto index = SendMessageW(get(report_reason), CB_GETCURSEL, 0, 0); const auto text = control_text::read(get(report_detail));
        if (index >= 0 && static_cast<size_t>(index) < community::report_reasons.size() && text) { result.request = Json{{"operation", "report"}, {"kind", selected_.at("kind") == "dictionary" ? "dictionaries" : "replies"}, {"id", publication}, {"reason", control_text::utf8(community::report_reasons[static_cast<size_t>(index)]).value()}, {"detail", *text}}; result.confirmation = L"提交对所选作品的举报？"; }
      }
    }
    return result;
  }
  void completed(const Json& request, const Json& response) {
    try { complete(request, response); }
    catch (const std::exception&) { clear_list(); SetWindowTextW(get(notice), L"社区返回内容无法读取，请重新查询。"); enable(enabled_); }
  }
private:
  void complete(const Json& request, const Json& response) {
    const auto operation = request.at("operation").get<std::string>(); const auto value = theme_data::value(response);
    if (!value && operation == "resource_import_local") {
      const auto partial = response.value("partial", Json::object());
      const auto message = L"本机已应用 " + std::to_wstring(partial.value("applied", uint64_t{0})) + L" 条，部分操作未完成。" +
          community::resource_message(control_text::wide(response.value("error", std::string{}))) + L" 可手动重试，已应用词条沿用原回执。";
      SetWindowTextW(get(notice), message.c_str()); enable(enabled_); return;
    }
    if (!value) { clear_list(); SetWindowTextW(get(notice), community::resource_message(control_text::wide(response.value("error", std::string{}))).c_str()); enable(enabled_); return; }
    std::wstring message = L"操作已完成。";
    if (operation == "resource_list") {
      if (!value->is_object() || !value->contains("items") || !value->at("items").is_array() || !value->contains("has_more") || !value->at("has_more").is_boolean()) throw std::runtime_error("invalid_list");
      if (request.value("offset", size_t{0}) == 0) clear_list(); offset_ = request.value("offset", size_t{0}) + value->at("items").size(); has_more_ = value->at("has_more").get<bool>() && !value->at("items").empty();
      for (const auto& item : value->at("items")) { if (!valid_item(item)) continue; if (std::any_of(items_.begin(), items_.end(), [&](const Json& prior) { return prior.at("id") == item.at("id"); })) continue;
        items_.push_back(item); const auto title = control_text::wide(item.at("name").get<std::string>()) + L" · " + control_text::wide(item.value("author", std::string{})); SendMessageW(get(list), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(title.c_str())); }
      if (selected_.is_null() && !items_.empty()) { SendMessageW(get(list), LB_SETCURSEL, 0, 0); select(items_.front()); } message = items_.empty() ? L"没有匹配的作品。" : L"作品列表已刷新。";
    } else if (operation == "resource_detail") { if (!valid_item(*value) || value->at("id") != request.at("id")) throw std::runtime_error("invalid_detail"); select(*value); message = L"作品详情已更新。"; }
    else if (operation == "resource_save") { if (!selected_.is_null() && selected_.at("id") == request.at("id")) { selected_["saved"] = request.at("saved"); select(selected_); } message = request.at("saved").get<bool>() ? L"作品已收藏。" : L"已取消收藏。"; }
    else if (operation == "resource_publish") { if (value->is_object() && value->value("id", std::string{}) == editor_id_ && value->contains("revision") && value->at("revision").is_number_unsigned()) editor_revision_ = value->at("revision").get<uint32_t>(); message = L"作品已发布，请刷新我的作品。"; }
    else if (operation == "resource_apply") message = L"词库已导入账号云词库。";
    else if (operation == "resource_import_local") message = L"本机词库已导入 " + std::to_wstring(value->value("applied", int64_t{0})) + L" 个词条。";
    else if (operation == "resource_unpublish") { clear_list(); message = L"作品已下架，已导入的副本保留。"; }
    else if (operation == "report") { SetWindowTextW(get(report_detail), L""); message = community::reported_notice; }
    SetWindowTextW(get(notice), message.c_str()); enable(enabled_);
  }
  static bool valid_item(const Json& item) {
    if (!item.is_object()) return false;
    for (const auto* key : {"id", "kind", "name"}) if (!item.contains(key) || !item.at(key).is_string() || item.at(key).get_ref<const std::string&>().empty()) return false;
    for (const auto* key : {"owned", "saved"}) if (!item.contains(key) || !item.at(key).is_boolean()) return false;
    if (!item.contains("revision") || !item.at("revision").is_number_unsigned() || !item.contains("content") || !item.at("content").is_object()) return false;
    const auto& content = item.at("content");
    if (item.at("kind") == "reply") return content.contains("prompt") && content.at("prompt").is_string();
    if (item.at("kind") != "dictionary" || !content.contains("entries") || !content.at("entries").is_array() || content.at("entries").size() > community::shared_word_limit) return false;
    return community_resource_data::entries(content.at("entries")).has_value();
  }
  static Json word_json(const community::SharedWord& word) { return {{"kind", control_text::utf8(word.kind).value()}, {"code", control_text::utf8(word.code).value()}, {"word", control_text::utf8(word.word).value()}, {"weight", word.weight}}; }
  static std::string uuid() { GUID guid{}; wchar_t value[40]{}; if (FAILED(CoCreateGuid(&guid)) || StringFromGUID2(guid, value, 40) != 39) return {}; return control_text::utf8(std::wstring_view(value + 1, 36)).value_or(""); }
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  std::wstring wide(int id) const { return control_text::wide(control_text::read(get(id)).value_or("")); }
  void show_entries() { SendMessageW(get(draft_entries), LB_RESETCONTENT, 0, 0); for (const auto& entry : entries_) { const auto text = entry.kind + L" · " + entry.code + L" · " + entry.word + L" · " + std::to_wstring(entry.weight); SendMessageW(get(draft_entries), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(text.c_str())); } }
  void reset_editor(bool existing) {
    entries_.clear(); editor_id_ = existing ? selected_.at("id").get<std::string>() : uuid(); editor_revision_ = existing ? selected_.at("revision").get<uint32_t>() : 0;
    editor_kind_ = existing ? selected_.at("kind").get<std::string>() : SendMessageW(get(kind), CB_GETCURSEL, 0, 0) == 1 ? "reply" : "dictionary";
    SetWindowTextW(get(editor_title), (std::wstring(existing ? L"发布新版本：" : L"新作品：") + community::resource_kind_title(control_text::wide(editor_kind_))).c_str());
    SetWindowTextW(get(name), existing ? control_text::wide(selected_.at("name").get<std::string>()).c_str() : L""); SetWindowTextW(get(description), existing ? control_text::wide(selected_.value("description", std::string{})).c_str() : L"");
    SetWindowTextW(get(prompt), existing ? control_text::wide(selected_.at("content").value("prompt", std::string{})).c_str() : L"");
    if (existing && editor_kind_ == "dictionary") for (const auto& entry : selected_.at("content").at("entries")) entries_.push_back({control_text::wide(entry.at("kind").get<std::string>()), control_text::wide(entry.at("code").get<std::string>()), control_text::wide(entry.at("word").get<std::string>()), entry.at("weight").get<int64_t>()});
    SendMessageW(get(agree), BM_SETCHECK, BST_UNCHECKED, 0); show_entries(); enable(enabled_);
  }
  void clear_list() { items_.clear(); selected_ = Json(); offset_ = 0; has_more_ = false; SendMessageW(get(list), LB_RESETCONTENT, 0, 0); SetWindowTextW(get(detail_text), L""); }
  void select(const Json& item) {
    if (selected_.is_null() || selected_.at("id") != item.at("id") || selected_.at("revision") != item.at("revision")) local_id_ = uuid(); selected_ = item;
    std::wstring text = control_text::wide(item.at("name").get<std::string>()) + L"\r\n" + control_text::wide(item.value("description", std::string{}));
    text += L"\r\n" + community::card_author(L"", control_text::wide(item.value("author", std::string{})), item.value("owned", false), false, item.value("moderation", std::string{}) == "removed");
    text += L"\r\n" + community::resource_card_metrics(item.value("saves", 0.0), item.value("rating_count", 0.0), item.value("rating_average", 0.0));
    if (item.at("kind") == "reply") text += L"\r\n\r\n" + control_text::wide(item.at("content").at("prompt").get<std::string>());
    else for (const auto& entry : item.at("content").at("entries")) text += L"\r\n" + control_text::wide(entry.at("code").get<std::string>()) + L" · " + control_text::wide(entry.at("word").get<std::string>());
    SetWindowTextW(get(detail_text), text.c_str()); enable(enabled_);
  }
  std::map<int, HWND> controls_; std::vector<Json> items_; Json selected_; std::vector<community::SharedWord> entries_;
  std::string kind_ = "dictionary", scope_, search_, editor_id_, editor_kind_ = "dictionary", local_id_; uint32_t editor_revision_ = 0;
  size_t offset_ = 0; bool enabled_ = false, has_more_ = false;
};
} // namespace msime::settings::win7
