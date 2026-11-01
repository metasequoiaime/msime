#pragma once

#include "ControlText.h"
#include "PreferencesStore.h"
#include "SettingsWindow.h"
#include "ThemeData.h"
#include "DictionaryData.h"
#include "../PluginEditorModel.h"
#include <commctrl.h>
#include <charconv>
#include <functional>
#include <map>
#include <optional>
#include <vector>

namespace msime::settings::win7 {
class DictionaryForm {
public:
  struct Command {
    bool handled = false;
    std::optional<Json> request;
    std::wstring confirmation;
    bool file = false;
    bool save = false;
    // 打开另一个窗口的路由：「词库同步…」打开 Win7 的词库同步面板（panels/win7/DictionarySyncWindow.cpp）。
    std::optional<std::string> route;
  };
  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row) {
    int top = 0;
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      auto control = row(type, title, style, id, top, height);
      if (id >= 0) controls_[id] = control;
      top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
      return control;
    };
    add(-1, L"STATIC", L"本机词库：新增、编辑、导入和清除学习数据通过输入法维护协议执行，不会强行取消正在进行的输入。", SS_LEFT, 62);
    add(dictionary_kind, WC_COMBOBOXW, L"词库类型", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST, 320);
    for (const auto& kind : dictionary_data::model::local_kinds) {
      kinds_.push_back(std::wstring(kind.id));
      SendMessageW(get(dictionary_kind), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(kind.title.data()));
    }
    for (const auto& kind : dictionary_data::model::resource_kinds) {
      kinds_.push_back(std::wstring(kind.id));
      SendMessageW(get(dictionary_kind), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(kind.title.data()));
    }
    SendMessageW(get(dictionary_kind), CB_SETCURSEL, 0, 0);
    add(dictionary_user_only, L"BUTTON", L"只查看个人词条", WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY);
    SendMessageW(get(dictionary_user_only), BM_SETCHECK, BST_CHECKED, 0);
    add(-1, L"STATIC", L"查询词条或编码（留空列出所选词库）", SS_LEFT, 42);
    add(dictionary_query, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    SendMessageW(get(dictionary_query), EM_SETLIMITTEXT, 1024, 0);
    add(dictionary_search, L"BUTTON", L"查找 / 刷新", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(dictionary_page, L"STATIC", L"", SS_LEFT, 42);
    add(dictionary_previous, L"BUTTON", L"上一页", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(dictionary_next, L"BUTTON", L"下一页", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(dictionary_entries, L"LISTBOX", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 220);
    add(dictionary_entry_source, L"STATIC", L"", SS_LEFT, 52);
    add(-1, L"STATIC", L"编码", SS_LEFT);
    add(dictionary_key, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    add(-1, L"STATIC", L"词条 / 快捷短语", SS_LEFT);
    add(dictionary_value, L"EDIT", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | ES_MULTILINE, 84);
    // 与共享页 dictionary-entries.tsx 的快捷短语说明同一句；Win7 的编辑框各类型共用，所以写明只对快捷短语生效。
    add(-1, L"STATIC", (L"快捷短语：" + std::wstring(msime::settings::plugins::quick_phrase_cursor_note)).c_str(), SS_LEFT, 84);
    add(-1, L"STATIC", L"权重（1–100000000）", SS_LEFT);
    add(dictionary_weight, L"EDIT", L"10000", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL | ES_NUMBER);
    add(dictionary_new, L"BUTTON", L"填写新词条", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(dictionary_save, L"BUTTON", L"保存词条", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(dictionary_delete, L"BUTTON", L"删除所选词条…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(-1, L"STATIC", L"导入格式（导出仅支持前两种 TSV 格式）", SS_LEFT, 42);
    add(dictionary_format, WC_COMBOBOXW, L"文件格式", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST, 240);
    formats();
    add(dictionary_import, L"BUTTON", L"导入词库文件…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(dictionary_export, L"BUTTON", L"导出当前类型…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(dictionary_export_all, L"BUTTON", L"导出全部个人词库…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(dictionary_reset, L"BUTTON", L"清除全部学习数据…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    // 「我的词条」经 WebDAV 网盘同步：Win7 导航不收云端同步的页面，与云剪贴板一样在独立面板里设置。
    add(dictionary_sync_open, L"BUTTON", L"词库同步（WebDAV 网盘）…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(dictionary_notice, L"STATIC", L"", SS_LEFT, 140);
    return top;
  }
  void enable(bool enabled) { enabled_ = enabled; update(); }
  Json list_request() const {
    // 个人粤语词库没有内置词条，`browse` 的 `cantonese` 又是资源词库，所以它总是发 `list`。
    Json request{{"operation", resource() || (!user_only() && kind() != "cantonese") ? "browse" : "list"}, {"kind", kind()},
        {"query", control_text::read(get(dictionary_query), 1024).value_or("")}, {"offset", offset_}, {"limit", page_size}};
    if (user_only()) request["user_only"] = true;
    return request;
  }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (!enabled_ || !IsWindowEnabled(get(id))) return result;
    if (id == dictionary_kind && notification == CBN_SELCHANGE) {
      offset_ = 0; ready_ = false; entries_.clear(); clear_editor(); formats(); update(); result.request = list_request(); return result;
    }
    if (id == dictionary_format && notification == CBN_SELCHANGE) { update(); return result; }
    if (id == dictionary_entries && notification == LBN_SELCHANGE) { selected_entry(); update(); return result; }
    if (notification != BN_CLICKED) return result;
    if (id == dictionary_sync_open) { result.route = "dictionary-sync"; return result; }
    if (id == dictionary_search || id == dictionary_user_only) { offset_ = 0; result.request = list_request(); }
    else if (id == dictionary_previous || id == dictionary_next) {
      offset_ = id == dictionary_previous ? (offset_ >= page_size ? offset_ - page_size : 0) : offset_ + page_size;
      result.request = list_request();
    } else if (id == dictionary_new) { clear_editor(); update(); }
    else if (id == dictionary_delete && previous_) {
      result.request = Json{{"operation", "edit"}, {"previous", *previous_}, {"replacement", nullptr}};
      result.confirmation = L"删除所选词条？内置词条会记录为已删除，清除学习数据后恢复。";
    } else if (id == dictionary_save) {
      const auto key = control_text::read(get(dictionary_key), 1024), value = control_text::read(get(dictionary_value), 4096), weight = control_text::read(get(dictionary_weight), 20);
      int64_t number = 0;
      if (!key || !value || !weight) { notice(L"无法读取词条，请检查内容长度。"); return result; }
      const auto parsed = std::from_chars(weight->data(), weight->data() + weight->size(), number);
      if (parsed.ec != std::errc{} || parsed.ptr != weight->data() + weight->size()) { notice(L"权重必须是整数。"); return result; }
      Json next{{"kind", kind()}, {"key", *key}, {"value", *value}, {"weight", number}};
      if (previous_ && previous_->at("key") == *key && previous_->at("value") == *value && previous_->at("weight") == number) return result;
      result.request = Json{{"operation", "edit"}, {"previous", previous_.value_or(Json())}, {"replacement", next}};
    } else if (id == dictionary_import || id == dictionary_export) {
      result.file = true; result.save = id == dictionary_export;
      result.request = Json{{"operation", result.save ? "export_file" : "import_file"}, {"kind", kind()}, {"format", format()}};
    } else if (id == dictionary_export_all) {
      result.file = true; result.save = true; result.request = Json{{"operation", "export_all_file"}};
    } else if (id == dictionary_reset) {
      result.request = Json{{"operation", "reset"}};
      result.confirmation = L"清除全部学习数据并恢复内置词库？个人词条、调权、删除记录和固定位置等数据将被清除。此操作无法撤销，建议先导出备份。";
    }
    return result;
  }
  void completed(const Json& request, const Json& completion, bool preserve_feedback = false) {
    const auto response = theme_data::value(completion);
    if (!response) { ready_ = false; failure(completion); if (preserve_feedback) notice(L"操作已完成，但列表刷新失败，请手动刷新确认。"); update(); return; }
    const auto operation = request.value("operation", std::string{});
    if (operation != "list" && operation != "browse") {
      const auto invalid = [&] { ready_ = false; notice(L"词库返回内容无法确认，请刷新核对操作状态。"); update(); };
      if (!response->is_object()) { invalid(); return; }
      if (operation == "import_file") {
        if (!count(*response, "applied") || !count(*response, "failed") ||
            !boolean(*response, "truncated") || !boolean(*response, "swapped")) { invalid(); return; }
        dictionary_data::model::ImportReport report;
        report.applied = response->value("applied", int64_t{0}); report.failed = response->value("failed", int64_t{0});
        report.truncated = response->value("truncated", false); report.swapped = response->value("swapped", false);
        if (response->contains("first_failures") && response->at("first_failures").is_array()) {
          for (const auto& failure : response->at("first_failures"))
            if (failure.is_object() && count(failure, "line") && failure.contains("reason") && failure.at("reason").is_string() && report.first_failures.size() < 8)
              report.first_failures.emplace_back(failure.at("line").get<int64_t>(), control_text::wide(failure.at("reason").get<std::string>()));
        }
        notice(dictionary_data::model::describe_import(dictionary_data::model::kind_label(control_text::wide(kind())), report));
      } else {
        if ((operation == "edit" && (!boolean(*response, "applied") || response->at("applied") != true)) ||
            (operation == "reset" && (!boolean(*response, "reset") || response->at("reset") != true)) ||
            ((operation == "export_file" || operation == "export_all_file") && (!count(*response, "rows") || !response->contains("path") || !response->at("path").is_string()))) { invalid(); return; }
        notice(operation == "export_file" || operation == "export_all_file" ? L"词库已导出。" : L"操作已完成。请刷新查看当前词条。");
      }
      if (operation == "edit" || operation == "reset") clear_editor();
      update(); return;
    }
    ready_ = false; entries_.clear(); clear_editor();
    SendMessageW(get(dictionary_entries), LB_RESETCONTENT, 0, 0);
    if (!response->is_object() || !response->contains("entries") || !response->at("entries").is_array() || response->at("entries").size() > page_size ||
        !response->contains("has_more") || !response->at("has_more").is_boolean() ||
        (response->contains("available") && !response->at("available").is_boolean())) { notice(L"词库返回内容无法读取，请刷新后重试。"); update(); return; }
    for (const auto& entry : response->at("entries")) {
      if (!entry.is_object() || !entry.contains("kind") || entry.at("kind") != kind() || !entry.contains("key") || !entry.at("key").is_string() ||
          !entry.contains("value") || !entry.at("value").is_string() || !count(entry, "weight") ||
          !entry.contains("source") || (entry.at("source") != "user" && entry.at("source") != "bundled")) {
        entries_.clear(); notice(L"词条返回内容无法读取，请刷新后重试。"); update(); return;
      }
      entries_.push_back(entry);
    }
    ready_ = true; has_more_ = response->at("has_more").get<bool>() && !entries_.empty();
    for (const auto& entry : entries_) {
      const auto text = control_text::wide(entry.at("key").get<std::string>()) + L" · " + control_text::wide(entry.at("value").get<std::string>()) + L" · " + std::to_wstring(entry.at("weight").get<int64_t>());
      SendMessageW(get(dictionary_entries), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(text.c_str()));
    }
    const auto status = response->value("available", true) ? dictionary_data::model::page_status(offset_, entries_.size(), has_more_) : dictionary_data::model::resource_unavailable(control_text::wide(kind()));
    SetWindowTextW(get(dictionary_page), status.c_str());
    if (!preserve_feedback) notice(L"");
    update();
  }
private:
  static bool boolean(const Json& value, const char* key) { return value.contains(key) && value.at(key).is_boolean(); }
  static bool count(const Json& value, const char* key) {
    if (!value.contains(key)) return false;
    const auto& number = value.at(key);
    return number.is_number_unsigned() ? number.get<uint64_t>() <= static_cast<uint64_t>((std::numeric_limits<int64_t>::max)()) :
        number.is_number_integer() && number.get<int64_t>() >= 0;
  }
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  std::string kind() const {
    const auto index = SendMessageW(get(dictionary_kind), CB_GETCURSEL, 0, 0);
    return index >= 0 && static_cast<size_t>(index) < kinds_.size() ? control_text::utf8(kinds_[static_cast<size_t>(index)]).value_or("") : "pinyin";
  }
  // 按所选的那一项判断：本机词库排在前面，粤语两边都有，不能只看 id。
  bool resource() const {
    const auto index = SendMessageW(get(dictionary_kind), CB_GETCURSEL, 0, 0);
    return index >= static_cast<LRESULT>(dictionary_data::model::local_kinds.size()) && static_cast<size_t>(index) < kinds_.size();
  }
  bool user_only() const { return !resource() && SendMessageW(get(dictionary_user_only), BM_GETCHECK, 0, 0) == BST_CHECKED; }
  std::string format() const {
    const auto index = SendMessageW(get(dictionary_format), CB_GETCURSEL, 0, 0);
    return index >= 0 && static_cast<size_t>(index) < formats_.size() ? control_text::utf8(formats_[static_cast<size_t>(index)].id).value_or("standard") : "standard";
  }
  void formats() {
    formats_ = dictionary_data::model::formats_for(control_text::wide(kind()));
    SendMessageW(get(dictionary_format), CB_RESETCONTENT, 0, 0);
    for (const auto& item : formats_) SendMessageW(get(dictionary_format), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(item.label.data()));
    SendMessageW(get(dictionary_format), CB_SETCURSEL, 0, 0);
  }
  void clear_editor() {
    previous_.reset();
    SendMessageW(get(dictionary_entries), LB_SETCURSEL, static_cast<WPARAM>(-1), 0);
    SetWindowTextW(get(dictionary_key), L""); SetWindowTextW(get(dictionary_value), L""); SetWindowTextW(get(dictionary_weight), L"10000");
    SetWindowTextW(get(dictionary_entry_source), L"填写新词条，保存时由共享引擎校验编码。");
  }
  void selected_entry() {
    const auto index = SendMessageW(get(dictionary_entries), LB_GETCURSEL, 0, 0);
    if (index < 0 || static_cast<size_t>(index) >= entries_.size()) return;
    previous_ = entries_[static_cast<size_t>(index)];
    SetWindowTextW(get(dictionary_key), control_text::wide(previous_->at("key").get<std::string>()).c_str());
    SetWindowTextW(get(dictionary_value), control_text::wide(previous_->at("value").get<std::string>()).c_str());
    SetWindowTextW(get(dictionary_weight), std::to_wstring(previous_->at("weight").get<int64_t>()).c_str());
    SetWindowTextW(get(dictionary_entry_source), resource() ? L"资源词库只读。" : previous_->at("source") == "bundled" ? L"内置词条：可以调整权重或删除，编码与文字只读。" : L"个人词条：可以编辑编码、文字与权重。");
  }
  void notice(const std::wstring& message) { SetWindowTextW(get(dictionary_notice), message.c_str()); }
  void failure(const Json& response) {
    const auto code = response.is_object() && response.contains("error") && response.at("error").is_string() ? response.at("error").get<std::string>() : "";
    const wchar_t* message = L"词库操作失败，请刷新后重试。";
    if (code == "dictionary_busy") message = L"词库正被输入法占用，请结束当前输入后重试。";
    else if (code == "dictionary_invalid_entry") message = L"编码不合规或与词条不匹配，请检查完整拼音、五笔或短语编码。";
    else if (code == "dictionary_invalid_word") message = L"词条为空、过长、含控制字符，或权重不在 1–100000000 之间。";
    else if (code == "dictionary_bundled_readonly") message = L"内置词条只能调整权重或删除。";
    else if (code == "dictionary_too_large") message = L"文件读取失败或超过上限：文本最多 32 MiB，细胞词库最多 8 MiB。";
    else if (code == "dictionary_encoding") message = L"无法解码文件，请使用 UTF-8、UTF-16 或 GB18030 文本。";
    else if (code == "dictionary_export_empty") message = L"当前类型没有可导出的词条。";
    else if (code == "dictionary_export_limit") message = L"导出内容超过 32 MiB，请分批导出。";
    else if (code == "dictionary_not_prepared") message = L"输入法尚未完成初始化，请先完成安装准备。";
    notice(message);
  }
  void update() {
    for (const auto& control : controls_) EnableWindow(control.second, enabled_);
    EnableWindow(get(dictionary_user_only), enabled_ && !resource());
    const bool editable = enabled_ && !resource();
    const bool bundled = previous_ && previous_->at("source") == "bundled";
    for (int id : {dictionary_key, dictionary_value}) { EnableWindow(get(id), editable); SendMessageW(get(id), EM_SETREADONLY, bundled, 0); }
    for (int id : {dictionary_weight, dictionary_new, dictionary_save, dictionary_format, dictionary_import}) EnableWindow(get(id), editable);
    EnableWindow(get(dictionary_entries), enabled_ && ready_);
    EnableWindow(get(dictionary_save), editable && (ready_ || !previous_));
    EnableWindow(get(dictionary_delete), editable && ready_ && previous_.has_value());
    EnableWindow(get(dictionary_export), editable && (format() == "standard" || format() == "windows"));
    EnableWindow(get(dictionary_previous), enabled_ && ready_ && offset_ > 0);
    EnableWindow(get(dictionary_next), enabled_ && ready_ && has_more_ && offset_ <= 1000000 - page_size);
  }
  static inline constexpr uint32_t page_size = 50;
  std::map<int, HWND> controls_;
  std::vector<std::wstring> kinds_;
  std::vector<dictionary_data::model::Format> formats_;
  std::vector<Json> entries_;
  std::optional<Json> previous_;
  uint32_t offset_ = 0;
  bool enabled_ = false, ready_ = false, has_more_ = false;
};
} // namespace msime::settings::win7
