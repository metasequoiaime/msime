#pragma once

#include "ControlText.h"
#include "ThemeData.h"
#include "../VoiceSettings.h"
#include <commctrl.h>
#include <filesystem>
#include <functional>
#include <map>

namespace msime::settings::win7 {
class VoiceModelsForm {
public:
  enum Id { list = 3600, refresh, detail, install, add_file, files, clear_files, import_model,
    use, remove, cancel, notice, sources, open_source, choose_directory };
  struct Command {
    bool handled = false, pick_file = false, pick_directory = false, cancel_install = false;
    std::optional<Json> request; std::vector<Change> changes; std::wstring confirmation;
    std::optional<std::string> url;
  };
  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row, int top) {
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      const auto control = row(type, title, style, id, top, height); if (id >= 0) controls_[id] = control;
      top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
    };
    const auto button = [&](int id, const wchar_t* title) { add(id, L"BUTTON", title, WS_TABSTOP | BS_PUSHBUTTON, 32); };
    add(-1, L"STATIC", L"本地识别模型：下载到本机后离线识别。Windows 7 运行库由安装包配套提供。", SS_LEFT, 64);
    button(refresh, L"刷新本地模型目录"); add(list, L"LISTBOX", L"识别模型", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 180);
    add(detail, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY | WS_VSCROLL, 260);
    add(sources, WC_COMBOBOXW, L"离线导入文件的原始地址", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 240);
    button(open_source, L"打开所选文件的下载来源"); button(install, L"下载并安装所选模型…");
    button(add_file, L"添加离线文件…"); add(files, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY | WS_VSCROLL, 100);
    button(clear_files, L"清空所选离线文件"); button(import_model, L"校验并导入离线文件…");
    button(use, L"使用所选已安装模型"); button(remove, L"删除所选模型…"); button(choose_directory, L"选择其他已安装模型目录…");
    button(cancel, L"取消当前模型安装"); add(notice, L"STATIC", L"", SS_LEFT, 100); enable(false); return top;
  }
  void refresh_preferences(const Json& preferences) { current_path_ = preferences.at("voice_input").value("asr_model_path", std::string{}); }
  void enable(bool value) {
    enabled_ = value;
    for (const auto& item : controls_) EnableWindow(item.second, enabled_ && !installing_);
    EnableWindow(get(cancel), installing_);
    const auto* model = selected(); const bool installed = model && model->value("installed", false);
    for (const auto id : {install, add_file}) EnableWindow(get(id), enabled_ && !installing_ && model && !installed);
    EnableWindow(get(import_model), enabled_ && !installing_ && model && !installed && !import_files_.empty());
    for (const auto id : {use, remove}) EnableWindow(get(id), enabled_ && !installing_ && installed);
    EnableWindow(get(open_source), enabled_ && !installing_ && model && !source_urls_.empty());
    EnableWindow(get(clear_files), enabled_ && !installing_ && !import_files_.empty());
  }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event); if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (id == cancel && notification == BN_CLICKED && installing_) { result.cancel_install = true; SetWindowTextW(get(notice), L"已请求取消，正在等待安装收尾。"); return result; }
    if (!enabled_ || installing_ || !IsWindowEnabled(get(id))) return result;
    if (id == list && notification == LBN_SELCHANGE) { import_files_.clear(); SetWindowTextW(get(files), L""); render_detail(); return result; }
    if (notification != BN_CLICKED) return result;
    if (id == refresh) { result.request = Json{{"operation", "models"}}; return result; }
    if (id == choose_directory) { result.pick_directory = true; return result; }
    const auto* model = selected(); if (!model) return result;
    if (id == install || id == import_model) {
      result.request = Json{{"operation", "install"}, {"id", model->at("id")}};
      if (id == import_model) (*result.request)["files"] = import_files_;
      result.confirmation = L"安装「" + control_text::wide(model->at("title").get<std::string>()) + L"」？请先确认上方下载大小和模型许可。";
    }
    if (id == add_file) result.pick_file = true;
    if (id == clear_files) { import_files_.clear(); SetWindowTextW(get(files), L""); enable(enabled_); }
    if (id == use) result.changes = {{{"voice_input", "asr_provider"}, "local"}, {{"voice_input", "asr_model_path"}, model->at("path")}};
    if (id == remove) {
      result.request = Json{{"operation", "remove"}, {"id", model->at("id")}, {"removed_path", model->at("path")}};
      result.confirmation = L"删除本机的「" + control_text::wide(model->at("title").get<std::string>()) + L"」？";
      if (model->at("path") == current_path_) result.confirmation += L"当前正在使用这个模型，删除后清空所选模型目录。";
    }
    if (id == open_source) { const auto index = SendMessageW(get(sources), CB_GETCURSEL, 0, 0); if (index >= 0 && static_cast<size_t>(index) < source_urls_.size()) result.url = source_urls_[static_cast<size_t>(index)]; }
    return result;
  }
  void add_import_file(const std::filesystem::path& path) {
    if (!path.is_absolute() || import_files_.size() >= 16) return;
    const auto value = path.u8string(); if (std::find(import_files_.begin(), import_files_.end(), value) != import_files_.end()) return;
    import_files_.push_back(value); std::wstring label; for (const auto& file : import_files_) label += control_text::wide(file) + L"\r\n";
    SetWindowTextW(get(files), label.c_str()); enable(enabled_);
  }
  void started(const Json& request) {
    if (request.value("operation", std::string{}) == "install") { installing_ = true; SetWindowTextW(get(notice), L"正在准备安装，可随时取消。"); }
    enable(enabled_);
  }
  void progress(const Json& value) {
    if (!installing_ || !value.is_object()) return;
    const auto stage = value.value("stage", std::string{});
    const auto label = std::wstring(stage == "verify" ? L"校验文件" : stage == "extract" ? L"解压模型" : stage == "import" ? L"导入文件" : stage == "done" ? L"安装完成" : L"下载模型") +
      L"：" + std::to_wstring(value.value("downloaded", uint64_t{0})) + L" / " + std::to_wstring(value.value("total", uint64_t{0})) + L" 字节";
    SetWindowTextW(get(notice), label.c_str());
  }
  std::vector<Change> completed(const Json& request, const Json& response) {
    installing_ = false; std::vector<Change> changes;
    const auto operation = request.value("operation", std::string{}); const auto value = theme_data::value(response);
    const auto catalog = operation == "models" ? std::make_optional(response) : response.contains("catalog") ? std::make_optional(response.at("catalog")) : std::nullopt;
    if (catalog) load_catalog(*catalog); else load_catalog(Json{});
    if (value) {
      if (operation == "install" && current_path_.empty() && value->is_object() && value->contains("path") && value->at("path").is_string()) changes.push_back({{"voice_input", "asr_model_path"}, value->at("path")});
      if (operation == "remove" && request.value("removed_path", std::string{}) == current_path_) changes.push_back({{"voice_input", "asr_model_path"}, ""});
      if (operation == "install") { import_files_.clear(); SetWindowTextW(get(files), L""); }
      SetWindowTextW(get(notice), operation == "install" ? L"模型已安装。" : operation == "remove" ? L"模型已删除。" : L"模型目录已刷新。");
    } else {
      const auto code = voice::local_model_error_code(control_text::wide(response.value("error", std::string{})));
      const auto message = request.contains("files") ? voice::local_model_import_error_message(code) : voice::local_model_error_message(code);
      SetWindowTextW(get(notice), code == L"local_model_cancelled" ? L"安装已取消。" : message ? message->c_str() : L"本地模型操作未完成，已有文件不会自动重置。");
    }
    enable(enabled_); return changes;
  }
private:
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  const Json* selected() const { const auto index = SendMessageW(get(list), LB_GETCURSEL, 0, 0); return loaded_ && index >= 0 && static_cast<size_t>(index) < models_.size() ? &models_[static_cast<size_t>(index)] : nullptr; }
  void load_catalog(const Json& response) {
    const auto* previous = selected(); const auto id = previous ? previous->at("id").get<std::string>() : std::string{};
    loaded_ = false; models_.clear(); const auto value = theme_data::value(response); SendMessageW(get(list), LB_RESETCONTENT, 0, 0);
    if (value && value->is_object() && value->contains("models") && value->at("models").is_array()) {
      std::vector<Json> parsed; bool valid = value->at("models").size() <= 128;
      for (const auto& item : value->at("models")) {
        if (!item.is_object() || !item.contains("installed") || !item.at("installed").is_boolean()) { valid = false; break; }
        for (const auto* field : {"id", "title", "description", "path", "license_spdx", "license_source", "license_terms", "license_notice"}) if (!item.contains(field) || !item.at(field).is_string()) valid = false;
        if (!valid) break; parsed.push_back(item);
      }
      if (valid) { models_ = std::move(parsed); loaded_ = true; }
    }
    for (size_t index = 0; index < models_.size(); ++index) {
      const auto& item = models_[index]; const auto label = control_text::wide(item.at("title").get<std::string>()) + (item.at("installed").get<bool>() ? L"（已安装）" : L"（未安装）");
      SendMessageW(get(list), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str())); if (item.at("id") == id || index == 0) SendMessageW(get(list), LB_SETCURSEL, index, 0);
    }
    render_detail();
  }
  void render_detail() {
    source_urls_.clear(); SendMessageW(get(sources), CB_RESETCONTENT, 0, 0); const auto* model = selected(); std::wstring text;
    if (model) {
      text = control_text::wide(model->at("title").get<std::string>()) + L"\r\n" + control_text::wide(model->at("description").get<std::string>());
      for (const auto* field : {"archive_size", "installed_size", "memory"}) if (model->contains(field) && model->at(field).is_number_unsigned())
        text += std::wstring(L"\r\n") + (std::string_view(field) == "archive_size" ? L"下载大小：" : std::string_view(field) == "installed_size" ? L"安装大小：" : L"建议内存：") + std::to_wstring(model->at(field).get<uint64_t>()) + L" 字节";
      if (model->contains("languages") && model->at("languages").is_array()) {
        text += L"\r\n语言：";
        for (const auto& language : model->at("languages")) if (language.is_string()) text += control_text::wide(language.get<std::string>()) + L" ";
      }
      if (model->contains("streaming") && model->at("streaming").is_boolean()) text += model->at("streaming").get<bool>() ? L"\r\n流式识别：支持" : L"\r\n流式识别：不支持";
      if (model->contains("hotwords") && model->at("hotwords").is_string()) {
        const auto hotwords = model->at("hotwords").get<std::string>();
        text += std::wstring(L"\r\n热词：") + (hotwords == "native" ? L"模型原生支持" : hotwords == "pinyin" ? L"拼音近音纠正" : L"不支持");
      }
      text += L"\r\n许可：" + control_text::wide(model->at("license_spdx").get<std::string>()) + L"\r\n" + control_text::wide(model->at("license_terms").get<std::string>()) + L"\r\n" + control_text::wide(model->at("license_notice").get<std::string>());
      const auto source = model->at("license_source").get<std::string>(); append_source(L"模型许可来源", source);
      if (model->contains("import_files") && model->at("import_files").is_array()) for (const auto& file : model->at("import_files"))
        if (file.is_object() && file.contains("name") && file.at("name").is_string() && file.contains("url") && file.at("url").is_string()) append_source(control_text::wide(file.at("name").get<std::string>()), file.at("url").get<std::string>());
      if (!source_urls_.empty()) SendMessageW(get(sources), CB_SETCURSEL, 0, 0);
    }
    SetWindowTextW(get(detail), text.c_str()); enable(enabled_);
  }
  void append_source(std::wstring_view title, const std::string& url) {
    if (url.substr(0,8) != "https://" || url.find_first_of("\r\n\t") != std::string::npos) return;
    source_urls_.push_back(url); const std::wstring label(title); SendMessageW(get(sources), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
  }
  std::map<int,HWND> controls_; std::vector<Json> models_; std::vector<std::string> import_files_, source_urls_; std::string current_path_;
  bool loaded_ = false, enabled_ = false, installing_ = false;
};
} // namespace msime::settings::win7
