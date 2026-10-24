#pragma once

#include "ControlText.h"
#include "SettingsWindow.h"
#include "CommunitySkinData.h"
#include "../CommunityPageModel.h"
#include <commctrl.h>
#include <objbase.h>
#include <algorithm>
#include <functional>
#include <map>

namespace msime::settings::win7 {

// 只持有页面状态和明确操作；认证、下载、安装与同步交给共享服务。
class CommunitySkinForm {
public:
  using Row = std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>;
  struct Command {
    bool handled = false;
    std::optional<Json> action;
    std::wstring confirmation;
    std::wstring message;
    bool local = false;
    bool login = false;
    bool pick_preview = false;
  };
  int create(const Row& row) {
    int top = 0;
    const auto control = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      auto window = row(type, title, style, id, top, height);
      if (id >= 0) controls_[id] = window;
      top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
      return window;
    };
    const auto note = [&](const wchar_t* text, int height = 28) { control(-1, L"STATIC", text, SS_LEFT, height); };
    const auto button = [&](int id, const wchar_t* text) { control(id, L"BUTTON", text, WS_TABSTOP | BS_PUSHBUTTON, 32); };
    const auto combo = [&](int id, const wchar_t* title) {
      note(title);
      return control(id, WC_COMBOBOXW, title, WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 220);
    };
    note(L"社区候选窗口皮肤，安装后在「主题与皮肤」中选用。", 48);
    note(L"搜索皮肤");
    control(community_search, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    SendMessageW(get(community_search), EM_LIMITTEXT, 512, 0);
    auto category = combo(community_filter, L"分类");
    SendMessageW(category, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(L"全部分类"));
    for (const auto& item : community::skin_categories)
      SendMessageW(category, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(item.label.data()));
    SendMessageW(category, CB_SETCURSEL, 0, 0);
    control(community_mine, L"BUTTON", L"只看我的皮肤库", WS_TABSTOP | BS_AUTOCHECKBOX);
    button(community_reload, L"搜索 / 刷新");
    control(community_list, L"LISTBOX", L"社区皮肤", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 180);
    button(community_more, L"加载更多");
    control(community_notice, L"STATIC", L"", SS_LEFT, 96);
    control(community_detail_text, L"EDIT", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | ES_MULTILINE | ES_READONLY, 200);
    button(community_detail, L"读取所选皮肤详情");
    control(community_preview_image, L"STATIC", L"社区皮肤预览", SS_OWNERDRAW | WS_BORDER, 180);
    button(community_preview_load, L"读取所选皮肤预览图");
    button(community_install, L"安装所选皮肤");
    button(community_local, L"打开本地主题与皮肤");
    button(community_login, L"打开账号与同步");
    note(L"为已下载的作品评分（不能给自己的作品评分）", 48);
    auto stars = combo(community_stars, L"评分");
    for (const auto* text : {L"1 星", L"2 星", L"3 星", L"4 星", L"5 星"})
      SendMessageW(stars, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(text));
    SendMessageW(stars, CB_SETCURSEL, 4, 0);
    button(community_rate, L"提交评分");
    note(L"我的作品管理");
    auto visibility = combo(community_visibility, L"可见性");
    for (const auto* text : {L"公开", L"私有"}) SendMessageW(visibility, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(text));
    button(community_visibility_save, L"修改可见性");
    auto edit_category = combo(community_category, L"作品分类");
    for (const auto& item : community::skin_categories)
      SendMessageW(edit_category, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(item.label.data()));
    button(community_category_save, L"修改作品分类");
    button(community_unpublish, L"下架所选作品…");
    button(community_sync, L"同步我的皮肤库…");
    note(L"举报所选作品");
    auto reason = combo(community_report_reason, L"举报原因");
    for (const auto& item : community::report_reasons)
      SendMessageW(reason, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(item.data()));
    SendMessageW(reason, CB_SETCURSEL, 0, 0);
    control(community_report_detail, L"EDIT", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | ES_MULTILINE, 80);
    SendMessageW(get(community_report_detail), EM_LIMITTEXT, community::report_detail_limit, 0);
    button(community_report, L"提交举报…");
    note(L"发布本地皮肤");
    combo(community_publish_skin, L"本地皮肤");
    note(community::skin_publish_warning.data(), 112);
    note(L"作品名称");
    control(community_publish_name, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    SendMessageW(get(community_publish_name), EM_LIMITTEXT, 128, 0);
    note(L"作品说明");
    control(community_publish_description, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | WS_VSCROLL, 80);
    SendMessageW(get(community_publish_description), EM_LIMITTEXT, 560, 0);
    auto publish_visibility = combo(community_publish_visibility, L"发布可见性");
    for (const auto* text : {L"公开", L"私有"}) SendMessageW(publish_visibility, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(text));
    SendMessageW(publish_visibility, CB_SETCURSEL, 0, 0);
    auto publish_category = combo(community_publish_category, L"发布分类");
    for (const auto& item : community::skin_categories)
      SendMessageW(publish_category, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(item.label.data()));
    SendMessageW(publish_category, CB_SETCURSEL, community::skin_categories.size() - 1, 0);
    note(L"素材授权（仅填写你有权授予的许可，如 CC-BY-4.0）", 48);
    control(community_publish_license, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    SendMessageW(get(community_publish_license), EM_LIMITTEXT, community::asset_license_limit, 0);
    button(community_license_save, L"将素材授权写入所选本地皮肤…");
    button(community_preview_add, L"给本地皮肤添加 PNG / JPEG 预览图…");
    button(community_pack_check, L"检查待发布皮肤包（不上传）");
    control(community_publish_agree, L"BUTTON", L"我确认拥有所需权利，了解公开作品将供他人下载", WS_TABSTOP | BS_AUTOCHECKBOX, 48);
    button(community_publish, L"发布所选本地皮肤…");
    return top;
  }
  void catalog(const Json& response) {
    const auto value = theme_data::value(response);
    if (!value || !value->is_object() || !value->contains("packages") || !value->at("packages").is_array()) return;
    const auto previous = local_skin();
    local_ = value->at("packages");
    SendMessageW(get(community_publish_skin), CB_RESETCONTENT, 0, 0);
    int selected = local_.empty() ? -1 : 0;
    for (size_t index = 0; index < local_.size(); ++index) {
      const auto label = community::local_option_label(control_text::wide(string(local_[index], "name")), control_text::wide(string(local_[index], "id")));
      SendMessageW(get(community_publish_skin), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
      if (string(local_[index], "id") == previous) selected = static_cast<int>(index);
    }
    SendMessageW(get(community_publish_skin), CB_SETCURSEL, selected, 0);
    if (previous != local_skin()) reset_publication();
  }
  void preview(theme_data::Image image) { image_ = std::move(image); InvalidateRect(get(community_preview_image), nullptr, TRUE); }
  bool draw(const DRAWITEMSTRUCT& item) const { return item.CtlID == community_preview_image && community_data::draw(item, image_); }
  void enable(bool enabled) {
    enabled_ = enabled;
    for (const auto& item : controls_) EnableWindow(item.second, enabled);
    const bool selected = !selected_.is_null();
    const bool owned = selected && boolean(selected_, "owned");
    EnableWindow(get(community_more), enabled && has_more_);
    for (int id : {community_detail, community_install, community_report, community_preview_load}) EnableWindow(get(id), enabled && selected);
    EnableWindow(get(community_rate), enabled && selected && !owned);
    for (int id : {community_visibility, community_visibility_save, community_category, community_category_save, community_unpublish})
      EnableWindow(get(id), enabled && owned);
    EnableWindow(get(community_login), enabled && sign_in_);
    for (int id : {community_pack_check, community_publish, community_license_save, community_preview_add})
      EnableWindow(get(id), enabled && !local_skin().empty());
  }
  Json list_request(bool more = false) const {
    Json action{{"operation", "candidate_skin_list"}, {"offset", more ? offset_ : 0},
        {"search", search_}, {"mine", mine_}};
    if (!category_.empty()) action["category"] = category_;
    return action;
  }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notice = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (!enabled_) return result;
    if (id == community_publish_skin && notice == CBN_SELCHANGE) { reset_publication(); return result; }
    if (id == community_list && notice == LBN_SELCHANGE) {
      const auto index = SendMessageW(get(id), LB_GETCURSEL, 0, 0);
      if (index >= 0 && static_cast<size_t>(index) < items_.size()) select(items_[static_cast<size_t>(index)]);
      return result;
    }
    if (notice != BN_CLICKED) return result;
    if (id == community_pack_check || id == community_publish || id == community_license_save || id == community_preview_add) {
      const auto skin = local_skin();
      if (skin.empty()) { result.message = L"请先选择本地皮肤。"; return result; }
      if (id == community_license_save) {
        const auto license = control_text::read(get(community_publish_license));
        if (!license || !community::asset_license_valid(control_text::wide(*license))) { result.message = L"请填写有效的素材许可，UTF-8 长度不超过 120 字节。"; return result; }
        result.action = Json{{"operation", "candidate_skin_add_license"}, {"skin_id", skin}, {"assets", control_text::utf8(community::trim(control_text::wide(*license))).value()}};
        result.confirmation = L"将填写的素材授权写入本地皮肤「" + control_text::wide(skin) + L"」？";
        return result;
      }
      if (id == community_preview_add) {
        result.action = Json{{"operation", "candidate_skin_add_preview"}, {"skin_id", skin}};
        result.pick_preview = true; return result;
      }
      const auto visibility = SendMessageW(get(community_publish_visibility), CB_GETCURSEL, 0, 0);
      if (visibility != 0 && visibility != 1) return result;
      if (id == community_pack_check) {
        result.action = Json{{"operation", "candidate_skin_pack_preview"}, {"skin_id", skin}, {"visibility", visibility ? "private" : "public"}};
        return result;
      }
      const auto name = control_text::read(get(community_publish_name)), description = control_text::read(get(community_publish_description));
      const auto fields = community::publish_fields(name ? control_text::wide(*name) : L"", description ? control_text::wide(*description) : L"");
      if (!name || !description || !fields.name_valid || !fields.description_valid || SendMessageW(get(community_publish_agree), BM_GETCHECK, 0, 0) != BST_CHECKED) {
        result.message = L"名称为 1–32 个字，说明最多 280 个字，并需确认拥有所需权利。"; return result;
      }
      const auto category = SendMessageW(get(community_publish_category), CB_GETCURSEL, 0, 0);
      if (category < 0 || static_cast<size_t>(category) >= community::skin_categories.size()) return result;
      if (publication_id_.empty()) { result.message = L"无法创建发布标识，请重新选择本地皮肤。"; return result; }
      result.action = Json{{"operation", "candidate_skin_publish"}, {"skin_id", skin}, {"id", publication_id_},
          {"name", control_text::utf8(fields.name).value()}, {"description", control_text::utf8(fields.description).value()},
          {"visibility", visibility ? "private" : "public"}, {"category", control_text::utf8(community::skin_categories[static_cast<size_t>(category)].value).value()}};
      result.confirmation = L"将本地皮肤「" + control_text::wide(skin) + L"」上传到账号皮肤库，作品名为「" + fields.name +
          (visibility ? L"」，仅自己可见。是否继续？" : L"」，公开后其他用户可以下载。是否继续？");
      return result;
    }
    if (id == community_local) { result.local = true; return result; }
    if (id == community_login) { result.login = true; return result; }
    if (id == community_reload) {
      const auto search = control_text::read(get(community_search));
      if (!search) { result.message = L"无法读取搜索文字。"; return result; }
      search_ = control_text::utf8(community::bounded_graphemes(control_text::wide(*search), community::search_limit)).value_or("");
      mine_ = SendMessageW(get(community_mine), BM_GETCHECK, 0, 0) == BST_CHECKED;
      const auto index = SendMessageW(get(community_filter), CB_GETCURSEL, 0, 0);
      category_ = index > 0 && static_cast<size_t>(index) <= community::skin_categories.size() ?
          control_text::utf8(community::skin_categories[static_cast<size_t>(index - 1)].value).value_or("") : "";
      result.action = list_request(); return result;
    }
    if (id == community_more) { if (has_more_) result.action = list_request(true); return result; }
    if (id == community_sync) {
      result.action = Json{{"operation", "candidate_skin_sync"}};
      result.confirmation = L"将本地皮肤与账号皮肤库同步，可能上传本地作品、下载云端作品或移除已删除的本地副本。是否继续？";
      return result;
    }
    if (selected_.is_null()) return result;
    const auto selected_id = string(selected_, "id");
    if (id == community_detail) result.action = Json{{"operation", "candidate_skin_detail"}, {"id", selected_id}};
    if (id == community_preview_load) result.action = Json{{"operation", "candidate_skin_preview"}, {"id", selected_id}};
    if (id == community_install) {
      result.action = Json{{"operation", "candidate_skin_install"}, {"id", selected_id}, {"replace", replace_}};
      if (replace_) result.confirmation = L"已有同名皮肤「" + control_text::wide(string(selected_, "package_id")) + L"」，将完整替换该文件夹。是否继续？";
    }
    if (id == community_rate && !boolean(selected_, "owned")) {
      const auto stars = SendMessageW(get(community_stars), CB_GETCURSEL, 0, 0);
      if (stars >= 0 && stars < 5) result.action = Json{{"operation", "candidate_skin_rate"}, {"id", selected_id}, {"stars", stars + 1}};
    }
    if (id == community_report) {
      const auto index = SendMessageW(get(community_report_reason), CB_GETCURSEL, 0, 0);
      const auto detail = control_text::read(get(community_report_detail));
      if (index < 0 || static_cast<size_t>(index) >= community::report_reasons.size() || !detail) return result;
      result.action = Json{{"operation", "report"}, {"kind", "candidate-skins"}, {"id", selected_id},
          {"reason", control_text::utf8(community::report_reasons[static_cast<size_t>(index)]).value()}, {"detail", *detail}};
      result.confirmation = L"提交对「" + control_text::wide(string(selected_, "name")) + L"」的举报？";
    }
    if (boolean(selected_, "owned")) {
      if (id == community_unpublish) {
        result.action = Json{{"operation", "candidate_skin_unpublish"}, {"id", selected_id}};
        result.confirmation = L"下架「" + control_text::wide(string(selected_, "name")) + L"」？本地皮肤文件夹会保留。";
      }
      if (id == community_visibility_save) {
        const auto index = SendMessageW(get(community_visibility), CB_GETCURSEL, 0, 0);
        if (index == 0 || index == 1) {
          result.action = Json{{"operation", "candidate_skin_set_visibility"}, {"id", selected_id}, {"visibility", index ? "private" : "public"}};
          result.confirmation = index ? L"将所选作品设为私有？" : L"将所选作品公开展示给社区用户？";
        }
      }
      if (id == community_category_save) {
        const auto index = SendMessageW(get(community_category), CB_GETCURSEL, 0, 0);
        if (index >= 0 && static_cast<size_t>(index) < community::skin_categories.size())
          result.action = Json{{"operation", "candidate_skin_set_category"}, {"id", selected_id},
              {"category", control_text::utf8(community::skin_categories[static_cast<size_t>(index)].value).value()}};
      }
    }
    return result;
  }
  void completed(const Json& action, const Json& response) {
    const auto value = theme_data::value(response);
    const auto operation = string(action, "operation");
    sign_in_ = false;
    if (!value) {
      const auto error = string(response, "error");
      sign_in_ = community::needs_sign_in(control_text::wide(error));
      if (operation == "candidate_skin_install" && error == "candidate_skin_exists") replace_ = true;
      if (operation == "candidate_skin_list" && action.value("offset", size_t{0}) == 0) clear_list();
      SetWindowTextW(get(community_notice), community::candidate_skin_message(control_text::wide(error)).c_str());
      enable(enabled_); return;
    }
    std::wstring notice;
    if (operation == "candidate_skin_list") {
      if (!value->is_object() || !value->contains("skins") || !value->at("skins").is_array() ||
          !value->contains("has_more") || !value->at("has_more").is_boolean()) {
        if (action.value("offset", size_t{0}) == 0) clear_list();
        SetWindowTextW(get(community_notice), L"皮肤列表返回内容无法读取，请刷新重试。"); return;
      }
      if (action.value("offset", size_t{0}) == 0) clear_list();
      offset_ = action.value("offset", size_t{0}) + value->at("skins").size();
      has_more_ = value->at("has_more").get<bool>() && !value->at("skins").empty();
      for (const auto& item : value->at("skins")) {
        if (!valid_item(item)) continue;
        const auto duplicate = std::find_if(items_.begin(), items_.end(), [&](const Json& prior) { return prior.at("id") == item.at("id"); });
        if (duplicate != items_.end()) continue;
        items_.push_back(item);
        const auto label = control_text::wide(string(item, "name")) + L" · " + control_text::wide(string(item, "author"));
        SendMessageW(get(community_list), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
      }
      if (selected_.is_null() && !items_.empty()) { SendMessageW(get(community_list), LB_SETCURSEL, 0, 0); select(items_.front()); }
      notice = items_.empty() ? L"没有匹配的候选窗口皮肤。" : L"已读取 " + std::to_wstring(items_.size()) + L" 款皮肤。";
    } else if (operation == "candidate_skin_detail" || operation == "candidate_skin_set_visibility" || operation == "candidate_skin_set_category") {
      if (!valid_item(*value) || string(*value, "id") != string(action, "id")) notice = L"皮肤详情返回内容无法读取，请刷新重试。";
      else {
        for (auto& item : items_) if (item.at("id") == value->at("id")) item = *value;
        select(*value); notice = L"皮肤详情已更新。";
      }
    } else if (operation == "candidate_skin_install") { replace_ = false; notice = L"皮肤已安装，请到「主题与皮肤」选用。"; }
    else if (operation == "candidate_skin_preview") notice = image_.bgra.empty() ? L"预览图无法读取。" : L"预览图已读取。";
    else if (operation == "candidate_skin_add_preview" || operation == "candidate_skin_add_license") notice = L"本地皮肤包已更新，可重新检查后发布。";
    else if (operation == "candidate_skin_pack_preview") {
      notice = L"皮肤包检查通过，尚未上传。";
      if (value->is_object()) {
        const auto found = value->find("size");
        if (found != value->end() && found->is_number()) notice += L"\r\n大小：" + community::megabytes(found->get<double>());
        const auto name = control_text::read(get(community_publish_name));
        if (name && name->empty()) SetWindowTextW(get(community_publish_name), control_text::wide(string(*value, "suggestedName")).c_str());
      }
    }
    else if (operation == "candidate_skin_publish") notice = L"皮肤已发布，可刷新「我的皮肤库」查看。";
    else if (operation == "candidate_skin_rate") notice = L"评分已提交。";
    else if (operation == "report") { SetWindowTextW(get(community_report_detail), L""); notice = community::reported_notice; }
    else if (operation == "candidate_skin_unpublish") { clear_list(); notice = L"作品已下架，本地皮肤文件保留。请刷新列表。"; }
    else if (operation == "candidate_skin_sync") {
      notice = L"同步已完成。";
      if (value->is_object() && boolean(*value, "stopped")) notice = L"同步已停止，请检查账号状态。";
      if (value->is_object() && value->contains("skipped") && value->at("skipped").is_array() && !value->at("skipped").empty())
        notice += L"有 " + std::to_wstring(value->at("skipped").size()) + L" 款皮肤未同步，请检查素材授权和包内容。";
    }
    SetWindowTextW(get(community_notice), notice.c_str());
    enable(enabled_);
  }
private:
  static std::string string(const Json& object, const char* key) {
    const auto found = object.find(key); return found != object.end() && found->is_string() ? found->get<std::string>() : std::string{};
  }
  static bool boolean(const Json& object, const char* key) {
    const auto found = object.find(key); return found != object.end() && found->is_boolean() && found->get<bool>();
  }
  static bool valid_item(const Json& item) {
    return item.is_object() && !string(item, "id").empty() && !string(item, "package_id").empty() && !string(item, "name").empty();
  }
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  void clear_list() {
    items_.clear(); selected_ = Json(); offset_ = 0; has_more_ = false; replace_ = false;
    SendMessageW(get(community_list), LB_RESETCONTENT, 0, 0);
    SetWindowTextW(get(community_detail_text), L"");
    preview({});
  }
  void select(const Json& item) {
    const bool changed = selected_.is_null() || string(selected_, "id") != string(item, "id");
    selected_ = item;
    if (changed) { replace_ = false; SetWindowTextW(get(community_report_detail), L""); preview({}); }
    std::wstring detail = control_text::wide(string(item, "name")) + L"\r\n" + control_text::wide(string(item, "description")) + L"\r\n";
    detail += community::card_author(community::category_label(control_text::wide(string(item, "category"))), control_text::wide(string(item, "author")),
        boolean(item, "owned"), string(item, "visibility") == "private", string(item, "moderation") == "removed");
    const auto number = [&](const char* key) { const auto found = item.find(key); return found != item.end() && found->is_number() ? found->get<double>() : 0.0; };
    detail += L"\r\n" + community::rating_metrics(number("downloads"), L"下载", number("rating_count"), number("rating_average"));
    if (item.contains("license") && item.at("license").is_object()) {
      const auto& license = item.at("license");
      detail += L"\r\n" + community::license_line(control_text::wide(string(license, "assets")), control_text::wide(string(license, "code")), control_text::wide(string(license, "source")));
    }
    detail += L"\r\n包 ID：" + control_text::wide(string(item, "package_id")) + L"\r\n版本：" + control_text::wide(string(item, "version"));
    if (number("my_rating") > 0) detail += L"\r\n我的评分：" + community::zh_number(number("my_rating"));
    SetWindowTextW(get(community_detail_text), detail.c_str());
    SendMessageW(get(community_visibility), CB_SETCURSEL, string(item, "visibility") == "private" ? 1 : 0, 0);
    SendMessageW(get(community_category), CB_SETCURSEL, static_cast<WPARAM>(-1), 0);
    for (size_t index = 0; index < community::skin_categories.size(); ++index)
      if (control_text::utf8(community::skin_categories[index].value).value() == string(item, "category")) SendMessageW(get(community_category), CB_SETCURSEL, index, 0);
    enable(enabled_);
  }
  std::map<int, HWND> controls_;
  std::string local_skin() const {
    const auto index = SendMessageW(get(community_publish_skin), CB_GETCURSEL, 0, 0);
    return index >= 0 && static_cast<size_t>(index) < local_.size() ? string(local_[static_cast<size_t>(index)], "id") : std::string{};
  }
  void reset_publication() {
    publication_id_.clear();
    GUID guid{}; wchar_t uuid[40]{};
    if (SUCCEEDED(CoCreateGuid(&guid)) && StringFromGUID2(guid, uuid, 40) == 39) {
      publication_id_ = control_text::utf8(std::wstring_view(uuid + 1, 36)).value_or("");
      std::transform(publication_id_.begin(), publication_id_.end(), publication_id_.begin(), [](char c) { return c >= 'A' && c <= 'F' ? static_cast<char>(c - 'A' + 'a') : c; });
    }
    SetWindowTextW(get(community_publish_name), L"");
    SetWindowTextW(get(community_publish_description), L"");
    SendMessageW(get(community_publish_agree), BM_SETCHECK, BST_UNCHECKED, 0);
  }
  std::vector<Json> items_;
  Json local_ = Json::array();
  theme_data::Image image_;
  std::string publication_id_;
  Json selected_;
  size_t offset_ = 0;
  bool has_more_ = false, mine_ = false, replace_ = false, enabled_ = false, sign_in_ = false;
  std::string search_, category_;
};
} // namespace msime::settings::win7
