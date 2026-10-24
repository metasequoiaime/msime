#pragma once

#include "PluginData.h"
#include "../CommunityPageModel.h"
#include <commctrl.h>
#include <objbase.h>
#include <functional>
#include <map>

namespace msime::settings::win7 {
class CommunityPluginForm {
public:
  enum Id { open = 3000, search, filter, mine, reload, list, more, detail_text, detail, install, local,
    login, stars, rate, unpublish, report_reason, report_detail, report, installed, pack_check,
    publish_name, publish_description, publish_agree, publish, notice };
  struct Command { bool handled = false, local = false, login = false; std::optional<Json> request; std::wstring confirmation, message; };
  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row) {
    int top = 0;
    const auto add = [&](int id, const wchar_t* type, const wchar_t* text, DWORD style, int height = 28) {
      const auto control = row(type, text, style, id, top, height); if (id >= 0) controls_[id] = control;
      top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
    };
    const auto note = [&](const wchar_t* text, int height = 36) { add(-1, L"STATIC", text, SS_LEFT, height); };
    const auto button = [&](int id, const wchar_t* text) { add(id, L"BUTTON", text, WS_TABSTOP | BS_PUSHBUTTON, 32); };
    const auto combo = [&](int id) { add(id, WC_COMBOBOXW, L"", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 240); };
    note(L"社区插件：安装后由你在本地插件页选用，不自动改变输入设置。", 56);
    note(L"搜索"); add(search, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    combo(filter); SendMessageW(get(filter), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(L"全部类型"));
    for (const auto kind : community::plugin_kinds) { const auto title = plugin_preferences::model::kind_label(kind); SendMessageW(get(filter), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(title.c_str())); }
    SendMessageW(get(filter), CB_SETCURSEL, 0, 0);
    add(mine, L"BUTTON", L"只看我的作品", WS_TABSTOP | BS_AUTOCHECKBOX); button(reload, L"搜索 / 刷新");
    add(list, L"LISTBOX", L"社区插件", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 180);
    button(more, L"加载更多"); add(notice, L"STATIC", L"", SS_LEFT, 100);
    add(detail_text, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY | WS_VSCROLL, 200);
    button(detail, L"读取所选插件详情"); button(install, L"安装所选插件…"); button(local, L"打开本地插件"); button(login, L"打开账号与同步");
    note(L"下载后可为他人作品评分"); combo(stars); for (const auto title : {L"1 星", L"2 星", L"3 星", L"4 星", L"5 星"}) SendMessageW(get(stars), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(title));
    SendMessageW(get(stars), CB_SETCURSEL, 4, 0); button(rate, L"提交评分"); button(unpublish, L"下架我的作品…");
    note(L"举报所选作品"); combo(report_reason); for (const auto reason : community::report_reasons) SendMessageW(get(report_reason), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(reason.data()));
    SendMessageW(get(report_reason), CB_SETCURSEL, 0, 0);
    add(report_detail, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | WS_VSCROLL, 90); button(report, L"提交举报…");
    note(L"发布我的本地插件"); note(community::plugin_publish_warning.data(), 112); combo(installed);
    button(pack_check, L"检查本地包（不上传）"); note(L"作品名称"); add(publish_name, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    note(L"作品说明"); add(publish_description, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | WS_VSCROLL, 90);
    add(publish_agree, L"BUTTON", L"我拥有发布所需的权利，并同意将所选插件公开分享", WS_TABSTOP | BS_AUTOCHECKBOX, 52);
    button(publish, L"确认发布插件…");
    SendMessageW(get(search), EM_SETLIMITTEXT, 512, 0); SendMessageW(get(report_detail), EM_SETLIMITTEXT, community::report_detail_limit, 0);
    SendMessageW(get(publish_name), EM_SETLIMITTEXT, 128, 0); SendMessageW(get(publish_description), EM_SETLIMITTEXT, 560, 0);
    enable(false); return top;
  }
  void enable(bool enabled) {
    enabled_ = enabled; for (const auto& item : controls_) EnableWindow(item.second, enabled);
    const bool selected = !selected_.is_null(), owned = selected && selected_.value("owned", false);
    for (const int id : {detail, install, report}) EnableWindow(get(id), enabled && selected);
    EnableWindow(get(rate), enabled && selected && !owned); EnableWindow(get(unpublish), enabled && owned);
    EnableWindow(get(more), enabled && has_more_); EnableWindow(get(login), enabled && sign_in_);
    EnableWindow(get(pack_check), enabled && local_package().has_value()); EnableWindow(get(publish), enabled && local_package().has_value());
  }
  Json list_request(bool more = false) const {
    Json action{{"operation", "plugin_list"}, {"search", search_}, {"offset", more ? offset_ : 0}, {"mine", mine_}};
    if (!kind_.empty()) action["kind"] = kind_; return action;
  }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event); if (!controls_.count(id)) return {};
    Command result; result.handled = true; if (!enabled_ || !IsWindowEnabled(get(id))) return result;
    if (id == list && notification == LBN_SELCHANGE) { const auto index = SendMessageW(get(list), LB_GETCURSEL, 0, 0); if (index >= 0 && static_cast<size_t>(index) < items_.size()) select(items_[static_cast<size_t>(index)]); return result; }
    if (id == installed && notification == CBN_SELCHANGE) { reset_publication(); return result; }
    if (notification != BN_CLICKED) return result;
    if (id == local) result.local = true;
    else if (id == login) result.login = true;
    else if (id == reload) {
      const auto text = control_text::read(get(search)); if (!text) { result.message = L"无法读取搜索文字。"; return result; }
      search_ = control_text::utf8(community::bounded_graphemes(control_text::wide(*text), community::search_limit)).value_or("");
      mine_ = SendMessageW(get(mine), BM_GETCHECK, 0, 0) == BST_CHECKED;
      const auto index = SendMessageW(get(filter), CB_GETCURSEL, 0, 0); kind_ = index > 0 && static_cast<size_t>(index) <= community::plugin_kinds.size() ? control_text::utf8(community::plugin_kinds[static_cast<size_t>(index - 1)]).value_or("") : "";
      result.request = list_request();
    } else if (id == more && has_more_) result.request = list_request(true);
    else if (id == pack_check || id == publish) {
      const auto package = local_package(); if (!package) return result;
      Json action{{"operation", id == pack_check ? "plugin_pack_preview" : "plugin_publish"}, {"kind", package->at("kind")}, {"plugin_id", package->at("id")}};
      if (id == publish) {
        const auto name = control_text::read(get(publish_name)), description = control_text::read(get(publish_description));
        const auto fields = community::publish_fields(name ? control_text::wide(*name) : L"", description ? control_text::wide(*description) : L"");
        if (!name || !description || !fields.name_valid || !fields.description_valid || publication_id_.empty() || SendMessageW(get(publish_agree), BM_GETCHECK, 0, 0) != BST_CHECKED) {
          result.message = L"名称为 1–32 个字，说明最多 280 个字，并需确认拥有发布权利。"; return result;
        }
        action["id"] = publication_id_; action["name"] = control_text::utf8(fields.name).value(); action["description"] = control_text::utf8(fields.description).value();
        result.confirmation = L"公开发布本地插件「" + control_text::wide(package->at("id").get<std::string>()) + L"」，作品名「" + fields.name + L"」？其他用户可以下载包内全部文件。";
      }
      result.request = std::move(action);
    } else if (!selected_.is_null()) {
      const auto id_value = selected_.at("id");
      if (id == detail) result.request = Json{{"operation", "plugin_detail"}, {"id", id_value}};
      if (id == install) { result.request = Json{{"operation", "plugin_install"}, {"id", id_value}, {"kind", selected_.at("kind")}, {"plugin_id", selected_.at("plugin_id")}}; result.confirmation = L"安装所选插件？同名本地包可能被替换，现有偏好不自动改变。"; }
      if (id == rate && !selected_.value("owned", false)) { const auto score = SendMessageW(get(stars), CB_GETCURSEL, 0, 0); if (score >= 0 && score < 5) result.request = Json{{"operation", "plugin_rate"}, {"id", id_value}, {"stars", score + 1}}; }
      if (id == unpublish && selected_.value("owned", false)) { result.request = Json{{"operation", "plugin_delete"}, {"id", id_value}}; result.confirmation = L"下架我的插件作品？本机已安装的包保留。"; }
      if (id == report) {
        const auto index = SendMessageW(get(report_reason), CB_GETCURSEL, 0, 0); const auto text = control_text::read(get(report_detail));
        if (index >= 0 && static_cast<size_t>(index) < community::report_reasons.size() && text) { result.request = Json{{"operation", "report"}, {"kind", "plugins"}, {"id", id_value}, {"reason", control_text::utf8(community::report_reasons[static_cast<size_t>(index)]).value()}, {"detail", *text}}; result.confirmation = L"提交对所选插件的举报？"; }
      }
    }
    return result;
  }
  void completed(const Json& request, const Json& response) {
    try { complete(request, response); }
    catch (const std::exception&) { clear_list(); SetWindowTextW(get(notice), L"社区返回内容无法读取，请刷新重试。"); enable(enabled_); }
  }
private:
  void complete(const Json& request, const Json& response) {
    if (response.contains("local_catalog")) catalog(response.at("local_catalog"));
    const auto operation = request.at("operation").get<std::string>(); const auto value = theme_data::value(response); sign_in_ = false;
    if (!value) {
      clear_list(); const auto error = response.value("error", std::string{}); sign_in_ = community::needs_sign_in(control_text::wide(error));
      SetWindowTextW(get(notice), community::plugin_message(control_text::wide(error), L"", operation == "plugin_publish").c_str()); enable(enabled_); return;
    }
    std::wstring notice_text = L"操作已完成。";
    if (operation == "plugin_list") {
      if (!value->is_object() || !value->contains("plugins") || !value->at("plugins").is_array() || !value->contains("has_more") || !value->at("has_more").is_boolean()) throw std::runtime_error("invalid_list");
      if (request.value("offset", size_t{0}) == 0) clear_list();
      offset_ = request.value("offset", size_t{0}) + value->at("plugins").size(); has_more_ = value->at("has_more").get<bool>() && !value->at("plugins").empty();
      for (const auto& item : value->at("plugins")) {
        if (!valid_item(item)) continue;
        if (std::any_of(items_.begin(), items_.end(), [&](const Json& previous) { return previous.at("id") == item.at("id"); })) continue;
        items_.push_back(item); const auto title = control_text::wide(item.at("name").get<std::string>()) + L" · " + plugin_preferences::model::kind_label(control_text::wide(item.at("kind").get<std::string>()));
        SendMessageW(get(list), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(title.c_str()));
      }
      if (selected_.is_null() && !items_.empty()) { SendMessageW(get(list), LB_SETCURSEL, 0, 0); select(items_.front()); }
      notice_text = items_.empty() ? community::plugin_empty_notice(has_more_, !search_.empty() || !kind_.empty(), mine_, true) : L"社区插件已刷新。";
    } else if (operation == "plugin_detail") { if (!valid_item(*value) || value->at("id") != request.at("id")) throw std::runtime_error("invalid_detail"); select(*value); notice_text = L"插件详情已更新。"; }
    else if (operation == "plugin_pack_preview") {
      notice_text = L"本地包检查通过，尚未上传。";
      if (value->is_object()) {
        if (value->contains("size") && value->at("size").is_number()) notice_text += L"\r\n大小：" + community::megabytes(value->at("size").get<double>());
        if (control_text::read(get(publish_name)).value_or("").empty()) SetWindowTextW(get(publish_name), control_text::wide(value->value("suggestedName", std::string{})).c_str());
        if (control_text::read(get(publish_description)).value_or("").empty()) SetWindowTextW(get(publish_description), control_text::wide(value->value("suggestedDescription", std::string{})).c_str());
      }
    } else if (operation == "plugin_install") notice_text = L"插件已安装，请到本地插件页选用。";
    else if (operation == "plugin_publish") notice_text = L"插件已发布，可刷新「我的作品」查看。";
    else if (operation == "plugin_delete") { clear_list(); notice_text = L"作品已下架，本地插件包保留。"; }
    else if (operation == "report") { SetWindowTextW(get(report_detail), L""); notice_text = community::reported_notice; }
    SetWindowTextW(get(notice), notice_text.c_str()); enable(enabled_);
  }
  void catalog(const Json& response) {
    const auto packages = plugin_data::packages(response); const auto old = local_package(); local_.clear(); SendMessageW(get(installed), CB_RESETCONTENT, 0, 0);
    int chosen = -1;
    if (packages) for (const auto& package : *packages) {
      if (package.builtin || std::find(community::plugin_kinds.begin(), community::plugin_kinds.end(), package.kind) == community::plugin_kinds.end()) continue;
      local_.push_back({{"id", control_text::utf8(package.id).value()}, {"kind", control_text::utf8(package.kind).value()}});
      const auto title = plugin_preferences::model::kind_label(package.kind) + L" · " + community::local_option_label(package.name, package.id);
      SendMessageW(get(installed), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(title.c_str()));
      if (chosen < 0 || (old && *old == local_.back())) chosen = static_cast<int>(local_.size() - 1);
    }
    SendMessageW(get(installed), CB_SETCURSEL, chosen, 0); if (old != local_package()) reset_publication();
  }
  std::optional<Json> local_package() const { const auto index = SendMessageW(get(installed), CB_GETCURSEL, 0, 0); return index >= 0 && static_cast<size_t>(index) < local_.size() ? std::make_optional(local_[static_cast<size_t>(index)]) : std::nullopt; }
  void reset_publication() { publication_id_ = new_publication_id(); SetWindowTextW(get(publish_name), L""); SetWindowTextW(get(publish_description), L""); SendMessageW(get(publish_agree), BM_SETCHECK, BST_UNCHECKED, 0); }
  static std::string new_publication_id() { GUID guid{}; wchar_t value[40]{}; if (FAILED(CoCreateGuid(&guid)) || StringFromGUID2(guid, value, 40) != 39) return {}; return control_text::utf8(std::wstring_view(value + 1, 36)).value_or(""); }
  static bool valid_item(const Json& item) {
    if (!item.is_object()) return false;
    for (const auto* key : {"id", "name", "kind", "plugin_id"}) if (!item.contains(key) || !item.at(key).is_string() || item.at(key).get_ref<const std::string&>().empty()) return false;
    if (item.contains("owned") && !item.at("owned").is_boolean()) return false;
    const auto kind = control_text::wide(item.at("kind").get<std::string>()); return std::find(community::plugin_kinds.begin(), community::plugin_kinds.end(), kind) != community::plugin_kinds.end();
  }
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  void clear_list() { items_.clear(); selected_ = Json(); offset_ = 0; has_more_ = false; SendMessageW(get(list), LB_RESETCONTENT, 0, 0); SetWindowTextW(get(detail_text), L""); }
  void select(const Json& item) {
    selected_ = item;
    std::wstring description = control_text::wide(item.at("name").get<std::string>()) + L"\r\n" + control_text::wide(item.value("description", std::string{}));
    description += L"\r\n" + community::card_author(plugin_preferences::model::kind_label(control_text::wide(item.at("kind").get<std::string>())), control_text::wide(item.value("author", std::string{})), item.value("owned", false), false, item.value("moderation", std::string{}) == "removed");
    description += L"\r\n版本：" + control_text::wide(item.value("version", std::string{})) + L"\r\n许可：" + control_text::wide(item.value("license", std::string{}));
    description += L"\r\n" + community::rating_metrics(item.value("downloads", 0.0), L"下载", item.value("rating_count", 0.0), item.value("rating_average", 0.0));
    SetWindowTextW(get(detail_text), description.c_str()); enable(enabled_);
  }
  std::map<int, HWND> controls_; std::vector<Json> items_, local_; Json selected_;
  std::string search_, kind_, publication_id_; size_t offset_ = 0;
  bool enabled_ = false, mine_ = false, has_more_ = false, sign_in_ = false;
};
} // namespace msime::settings::win7
