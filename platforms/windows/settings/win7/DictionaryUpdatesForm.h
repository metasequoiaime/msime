#pragma once

#include "ControlText.h"
#include "SettingsWindow.h"
#include "ThemeData.h"
#include <functional>
#include <map>

namespace msime::settings::win7 {
class DictionaryUpdatesForm {
public:
  struct Command {
    bool handled = false;
    std::optional<Json> request;
    std::wstring confirmation;
  };
  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row, int top) {
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      controls_[id] = row(type, title, style, id, top, height); top += height + 12;
    };
    add(dictionary_updates_enabled, L"BUTTON", L"自动更新官方词库", WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY);
    add(dictionary_updates_version, L"STATIC", L"词库版本：读取中…", SS_LEFT, 42);
    add(dictionary_updates_refresh, L"BUTTON", L"刷新更新状态", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(dictionary_updates_check, L"BUTTON", L"检查词库更新", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(dictionary_updates_rollback, L"BUTTON", L"恢复上一版词库…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(dictionary_updates_notice, L"STATIC", L"", SS_LEFT, 100);
    return top;
  }
  static Json status_request() { return Json{{"operation", "updates"}, {"command", "status"}}; }
  void enable(bool enabled) { enabled_ = enabled; update(); }
  bool running() const { return status_ && (status_->at("phase") == "checking" || status_->at("phase") == "downloading"); }
  Command command(WPARAM event) {
    const int id = LOWORD(event);
    if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (HIWORD(event) != BN_CLICKED || !enabled_ || !IsWindowEnabled(get(id))) return result;
    result.request = status_request();
    if (id == dictionary_updates_check) (*result.request)["command"] = "check";
    else if (id == dictionary_updates_enabled) {
      (*result.request)["command"] = "configure";
      (*result.request)["enabled"] = SendMessageW(get(id), BM_GETCHECK, 0, 0) == BST_CHECKED;
    } else if (id == dictionary_updates_rollback) {
      (*result.request)["command"] = "rollback";
      result.confirmation = L"恢复上一版官方词库并关闭自动更新？下次启动输入法时生效，个人词条和学习记录保留。";
    } else if (id != dictionary_updates_refresh) result.request.reset();
    return result;
  }
  void completed(const Json& response) {
    const auto value = theme_data::value(response);
    if (!value || !valid(*value)) {
      status_.reset();
      SetWindowTextW(get(dictionary_updates_notice), L"无法确认词库更新状态，请刷新后重试；继续使用原词库。");
      SetWindowTextW(get(dictionary_updates_version), L"词库版本：未知");
      update(); return;
    }
    status_ = *value;
    const auto version = L"词库版本：" + control_text::wide(status_->at("version").get<std::string>());
    SetWindowTextW(get(dictionary_updates_version), version.c_str());
    SendMessageW(get(dictionary_updates_enabled), BM_SETCHECK, status_->at("enabled") == true ? BST_CHECKED : BST_UNCHECKED, 0);
    std::wstring message = L"每天自动检查兼容的词库版本，个人词条和学习记录保留。";
    const auto phase = status_->at("phase").get<std::string>();
    if (phase == "failed" || (!status_->at("error").is_null() && !status_->at("error").get<std::string>().empty())) message = L"更新失败，继续使用原词库。";
    else if (phase == "checking") message = L"正在检查词库更新…";
    else if (phase == "downloading") {
      const auto downloaded = status_->at("downloaded_bytes").get<uint64_t>(), total = status_->at("total_bytes").get<uint64_t>();
      message = L"正在下载 " + std::to_wstring(downloaded / 1048576) + L" / " + std::to_wstring(total / 1048576 + (total % 1048576 != 0)) + L" MiB";
    } else if (status_->at("pending_restart") == true) message = L"词库已准备好，下次启动输入法时生效。";
    else if (phase == "current") message = L"词库已是最新版本。";
    SetWindowTextW(get(dictionary_updates_notice), message.c_str());
    update();
  }
private:
  static bool valid(const Json& value) {
    if (!value.is_object()) return false;
    for (const auto* key : {"enabled", "can_rollback", "pending_restart"})
      if (!value.contains(key) || !value.at(key).is_boolean()) return false;
    for (const auto* key : {"phase", "version"}) if (!value.contains(key) || !value.at(key).is_string()) return false;
    for (const auto* key : {"checked_at", "downloaded_bytes", "total_bytes"}) {
      if (!value.contains(key)) return false;
      const auto& number = value.at(key);
      if (!number.is_number_unsigned() && !(number.is_number_integer() && number.get<int64_t>() >= 0)) return false;
    }
    return value.contains("error") && (value.at("error").is_null() || value.at("error").is_string());
  }
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  void update() {
    for (const auto& control : controls_) EnableWindow(control.second, enabled_);
    EnableWindow(get(dictionary_updates_enabled), enabled_ && status_.has_value());
    EnableWindow(get(dictionary_updates_check), enabled_ && status_ && !running());
    EnableWindow(get(dictionary_updates_rollback), enabled_ && status_ && !running() && status_->at("can_rollback") == true);
    SetWindowTextW(get(dictionary_updates_check), running() ? L"更新中…" : L"检查词库更新");
  }
  std::map<int, HWND> controls_;
  std::optional<Json> status_;
  bool enabled_ = false;
};
} // namespace msime::settings::win7
