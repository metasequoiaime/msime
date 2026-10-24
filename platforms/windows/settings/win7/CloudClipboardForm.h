#pragma once

#include "ControlText.h"
#include "PreferencesStore.h"
#include "SettingsWindow.h"
#include "ThemeData.h"
#include <array>
#include <algorithm>
#include <functional>
#include <map>
#include <optional>
#include <vector>

namespace msime::settings::win7 {

// 只持有窗口展示与动作；来源校验、网络、加密和密钥保存仍由 worker 与共享服务处理。
class CloudClipboardForm {
public:
  struct Command {
    bool handled = false;
    std::vector<Change> changes;
    std::optional<Json> request;
    std::optional<std::wstring> copy;
    std::wstring confirmation;
  };
  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row) {
    int top = 0;
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      const auto control = row(type, title, style, id, top, height);
      if (id >= 0) controls_[id] = control;
      top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
      return control;
    };
    add(-1, L"STATIC", L"云剪贴板在设备上加密。服务器可见条目数量、时间、置顶状态和设备名称；新增设备需要核对并批准。", SS_LEFT, 62);
    add(cloud_clipboard_source, L"STATIC", L"", SS_LEFT, 44);
    add(-1, L"STATIC", L"自建中继地址（留空并清除令牌后使用水杉账号）", SS_LEFT, 44);
    add(cloud_clipboard_relay_url, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    add(-1, L"STATIC", L"中继令牌（留空沿用已保存令牌）", SS_LEFT, 42);
    add(cloud_clipboard_relay_token, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL | ES_PASSWORD);
    SendMessageW(get(cloud_clipboard_relay_url), EM_SETLIMITTEXT, 4096, 0);
    SendMessageW(get(cloud_clipboard_relay_token), EM_SETLIMITTEXT, 4096, 0);
    add(cloud_clipboard_relay_save, L"BUTTON", L"保存中继配置", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(cloud_clipboard_relay_clear, L"BUTTON", L"清除中继配置…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(cloud_clipboard_refresh, L"BUTTON", L"刷新云剪贴板与设备", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(cloud_clipboard_status, L"STATIC", L"点击刷新读取云端状态。", SS_LEFT, 106);
    add(cloud_clipboard_enabled, L"BUTTON", L"开启云剪贴板", WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY);
    add(-1, L"STATIC", L"未置顶条目的保留期", SS_LEFT);
    add(cloud_clipboard_retention, WC_COMBOBOXW, L"保留期", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST, 180);
    for (const auto* label : {L"不自动过期", L"1 天", L"7 天", L"30 天"})
      SendMessageW(get(cloud_clipboard_retention), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label));
    add(cloud_clipboard_search, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL);
    SendMessageW(get(cloud_clipboard_search), EM_SETLIMITTEXT, 1024, 0);
    add(cloud_clipboard_find, L"BUTTON", L"搜索已解密内容", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(cloud_clipboard_items, L"LISTBOX", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 200);
    add(cloud_clipboard_metadata, L"STATIC", L"", SS_LEFT, 52);
    add(cloud_clipboard_detail, L"EDIT", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | ES_MULTILINE | ES_READONLY, 150);
    add(cloud_clipboard_copy, L"BUTTON", L"复制所选内容", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(cloud_clipboard_pin, L"BUTTON", L"置顶 / 取消置顶", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(cloud_clipboard_delete, L"BUTTON", L"删除所选内容…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(cloud_clipboard_clear, L"BUTTON", L"清空云端内容…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(-1, L"STATIC", L"添加文字（最多 4000 个 UTF-16 单元）", SS_LEFT, 42);
    add(cloud_clipboard_text, L"EDIT", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | ES_MULTILINE | ES_WANTRETURN, 130);
    SendMessageW(get(cloud_clipboard_text), EM_SETLIMITTEXT, 4000, 0);
    add(cloud_clipboard_add, L"BUTTON", L"加密并上传文字", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(-1, L"STATIC", L"设备批准：请与另一台设备显示的 16 位核对码逐组比对，再选择允许。", SS_LEFT, 52);
    add(cloud_clipboard_devices, L"LISTBOX", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 160);
    add(cloud_clipboard_device_detail, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY, 132);
    add(cloud_clipboard_approve, L"BUTTON", L"核对一致，允许此设备…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(cloud_clipboard_reject, L"BUTTON", L"拒绝此设备…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(cloud_clipboard_remove, L"BUTTON", L"移除此设备并更换密钥…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(cloud_clipboard_reset, L"BUTTON", L"重置云剪贴板…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(cloud_clipboard_notice, L"STATIC", L"", SS_LEFT, 98);
    return top;
  }

  void refresh(const Snapshot& snapshot) {
    const auto& preferences = snapshot.preferences();
    const auto config = preferences.value("cloud_clipboard_relay", Json::object());
    if (!config_loaded_ || config != config_) {
      config_ = config; config_loaded_ = true;
      SetWindowTextW(get(cloud_clipboard_relay_url), control_text::wide(config.value("url", std::string{})).c_str());
      SetWindowTextW(get(cloud_clipboard_relay_token), L"");
    }
    if (binding_ && binding_->at("revision") != snapshot.revision()) invalidate();
    SetWindowTextW(get(cloud_clipboard_source), config.empty() ? L"当前来源：水杉账号" : L"当前来源：自建中继（请求失败不会改走水杉账号）");
  }
  void enable(bool enabled) { enabled_ = enabled; update(); }
  void copy_result(bool success) {
    SetWindowTextW(get(cloud_clipboard_notice), success ? L"已复制。" : L"无法复制到系统剪贴板，请重试。");
  }
  void cancelled() {
    SendMessageW(get(cloud_clipboard_enabled), BM_SETCHECK, remote_enabled_ ? BST_CHECKED : BST_UNCHECKED, 0);
  }
  Json refresh_request() const {
    return Json{{"action", {{"operation", "list"}, {"search", control_text::read(get(cloud_clipboard_search), 1024).value_or("")}}}};
  }
  // 独立面板在传输前重读同一来源，并核对同一条目的标识与正文。
  std::optional<Json> transfer_request() const {
    const auto* item = selected(cloud_clipboard_items, items_);
    if (!ready_ || !item || !binding_) return std::nullopt;
    auto request = bound({{"operation", "list"}, {"search", control_text::read(get(cloud_clipboard_search), 1024).value_or("")}});
    request["selected"] = {{"id", item->at("id")}, {"text", item->at("text")}};
    return request;
  }
  bool contains_transfer(const Json& selected) const {
    if (!ready_ || !selected.is_object() || !selected.contains("id") || !selected.contains("text")) return false;
    return std::any_of(items_.begin(), items_.end(), [&](const Json& item) { return item.at("id") == selected.at("id") && item.at("text") == selected.at("text"); });
  }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command command; command.handled = true;
    if (!enabled_ || !IsWindowEnabled(get(id))) return command;
    if (id == cloud_clipboard_items && notification == LBN_SELCHANGE) { item_detail(); update(); return command; }
    if (id == cloud_clipboard_devices && notification == LBN_SELCHANGE) { device_detail(); update(); return command; }
    if (id == cloud_clipboard_retention && notification == CBN_SELCHANGE) {
      const auto index = SendMessageW(get(id), CB_GETCURSEL, 0, 0);
      if (index >= 0 && index < static_cast<LRESULT>(retentions.size()) && retentions[static_cast<size_t>(index)] != retention_)
        command.request = bound({{"operation", "set_retention"}, {"days", retentions[static_cast<size_t>(index)]}});
      return command;
    }
    if (notification != BN_CLICKED) return command;
    if (id == cloud_clipboard_relay_save || id == cloud_clipboard_relay_clear) {
      const auto url = control_text::read(get(cloud_clipboard_relay_url), 4096), token = control_text::read(get(cloud_clipboard_relay_token), 4096);
      if (!url || !token) { SetWindowTextW(get(cloud_clipboard_notice), L"地址或令牌无法读取。"); return command; }
      const Json config{{"url", id == cloud_clipboard_relay_clear ? "" : *url}, {"token", id == cloud_clipboard_relay_clear ? "" : token->empty() ? config_.value("token", std::string{}) : *token}};
      if (config != config_ && !(config_.empty() && config.at("url") == "" && config.at("token") == "")) {
        command.changes.push_back({{"cloud_clipboard_relay"}, config});
        if (id == cloud_clipboard_relay_clear) command.confirmation = L"清除中继配置？之后手动刷新将使用水杉账号。中继里的内容与设备密钥不会被删除。";
      }
    } else if (id == cloud_clipboard_refresh || id == cloud_clipboard_find) command.request = refresh_request();
    else if (id == cloud_clipboard_enabled) {
      const bool enabled = SendMessageW(get(id), BM_GETCHECK, 0, 0) == BST_CHECKED;
      if (enabled != remote_enabled_) {
        command.request = bound({{"operation", "set_enabled"}, {"enabled", enabled}});
        if (!enabled) command.confirmation = L"关闭云剪贴板？云端条目、密钥和全部设备登记将被删除，之后需要重新批准设备。";
      }
    } else if (id == cloud_clipboard_add) {
      const auto text = control_text::read(get(cloud_clipboard_text), 4000);
      if (text) command.request = bound({{"operation", "add"}, {"text", *text}});
    } else if (id == cloud_clipboard_clear || id == cloud_clipboard_reset) {
      command.request = bound({{"operation", id == cloud_clipboard_clear ? "clear" : "reset"}});
      command.confirmation = id == cloud_clipboard_clear ? L"删除全部云端内容（包括置顶条目）？此操作无法撤销。" :
          L"重置云剪贴板？将更换密钥并删除全部云端内容，其他设备需要重新批准。此操作无法撤销。";
    } else if (const auto item = selected(cloud_clipboard_items, items_); item &&
        (id == cloud_clipboard_copy || id == cloud_clipboard_pin || id == cloud_clipboard_delete)) {
      if (id == cloud_clipboard_copy) command.copy = control_text::wide(item->at("text").get<std::string>());
      else if (id == cloud_clipboard_pin) command.request = bound({{"operation", "pin"}, {"id", item->at("id")}, {"pinned", !item->at("pinned").get<bool>()}});
      else { command.request = bound({{"operation", "delete"}, {"id", item->at("id")}}); command.confirmation = L"删除所选云端内容？此操作无法撤销。"; }
    } else if (const auto device = selected(cloud_clipboard_devices, devices_)) {
      const auto name = control_text::wide(device->at("name").get<std::string>());
      if (id == cloud_clipboard_approve) {
        command.request = bound({{"operation", "approve"}, {"device_id", device->at("device_id")}});
        command.confirmation = L"允许设备“" + name + L"”？\r\n核对码：" + control_text::wide(device->at("code").get<std::string>()) + L"\r\n请确认与该设备显示的 16 位核对码一致。";
      } else if (id == cloud_clipboard_reject || id == cloud_clipboard_remove) {
        command.request = bound({{"operation", id == cloud_clipboard_remove ? "remove" : "reject"}, {"device_id", device->at("device_id")}});
        command.confirmation = id == cloud_clipboard_remove ? L"移除设备“" + name + L"”？将更换密钥并删除全部云端内容，其他设备需要重新批准。" : L"拒绝设备“" + name + L"”访问云剪贴板？";
      }
    }
    return command;
  }

  void completed(const Json& request, const Json& completion) {
    invalidate();
    const auto response = completion.find("response");
    if (!completion.is_object() || response == completion.end()) { failed("invalid_cloud_clipboard"); return; }
    const auto value = theme_data::value(*response);
    if (!value) { failed(response->is_object() && response->contains("error") && response->at("error").is_string() ? response->at("error").get<std::string>() : "invalid_cloud_clipboard"); return; }
    const auto operation = request.at("action").value("operation", std::string{});
    if (operation == "add") SetWindowTextW(get(cloud_clipboard_text), L"");
    const auto list = completion.find("list"), devices = completion.find("devices");
    const auto page = list == completion.end() ? std::nullopt : theme_data::value(*list);
    const auto status = devices == completion.end() ? std::nullopt : theme_data::value(*devices);
    if (!page || !status || !valid_page(*page) || !valid_devices(*status) ||
        page->at("enabled") != status->at("enabled") || page->at("e2e").at("code") != status->at("code") ||
        !completion.contains("source") || !completion.at("source").is_string() ||
        !completion.contains("revision") || !completion.at("revision").is_number_unsigned()) {
      failed("cloud_clipboard_refresh_failed"); return;
    }
    binding_ = Json{{"source", completion.at("source")}, {"revision", completion.at("revision")}};
    remote_enabled_ = page->at("enabled").get<bool>();
    ready_ = page->at("e2e").at("status") == "ready" && status->at("status") == "ready" && remote_enabled_;
    status_ready_ = true; retention_ = page->at("retention_days").get<int>();
    if (ready_) items_ = page->at("items").get<std::vector<Json>>();
    devices_ = status->at("devices").get<std::vector<Json>>();
    const auto state = page->at("e2e").at("status").get<std::string>();
    std::wstring label = state == "ready" ? L"端到端加密已就绪。" : state == "pending" ? L"本机等待批准，请在已有设备上核对以下 16 位核对码并允许。\r\n" : L"云剪贴板已关闭。";
    label += L"\r\n本机核对码：" + control_text::wide(status->at("code").get<std::string>());
    label += L"\r\n待批准设备：" + std::to_wstring(page->at("e2e").value("pending_devices", 0u)) +
        L"；无法解密的条目：" + std::to_wstring(page->at("e2e").value("undecryptable", 0u));
    SetWindowTextW(get(cloud_clipboard_status), label.c_str());
    SendMessageW(get(cloud_clipboard_enabled), BM_SETCHECK, remote_enabled_ ? BST_CHECKED : BST_UNCHECKED, 0);
    int index = -1;
    for (size_t i = 0; i < retentions.size(); ++i) if (retentions[i] == retention_) index = static_cast<int>(i);
    SendMessageW(get(cloud_clipboard_retention), CB_SETCURSEL, index, 0);
    for (const auto& item : items_) {
      auto text = control_text::wide(item.at("text").get<std::string>());
      for (auto& ch : text) if (ch == L'\r' || ch == L'\n' || ch == L'\t') ch = L' ';
      if (text.size() > 96) { text.resize(96); if (text.back() >= 0xD800 && text.back() <= 0xDBFF) text.pop_back(); text += L"…"; }
      if (item.at("pinned") == true) text = L"[置顶] " + text;
      SendMessageW(get(cloud_clipboard_items), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(text.c_str()));
    }
    for (const auto& device : devices_) {
      const auto name = control_text::wide(device.at("name").get<std::string>()) +
          (device.at("current") == true ? L" [本机]" : L"") + (device.at("approved") == true ? L" [已批准]" : L" [等待批准]");
      SendMessageW(get(cloud_clipboard_devices), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(name.c_str()));
    }
    SetWindowTextW(get(cloud_clipboard_notice), operation == "list" ? L"" : L"操作已完成。" );
    update();
  }

private:
  static bool string(const Json& value, const char* key) { return value.contains(key) && value.at(key).is_string(); }
  static bool flag(const Json& value, const char* key) { return value.contains(key) && value.at(key).is_boolean(); }
  static bool hex(const Json& value, const char* key, size_t length) {
    if (!string(value, key)) return false;
    const auto& text = value.at(key).get_ref<const std::string&>();
    return text.size() == length && std::all_of(text.begin(), text.end(), [](char ch) { return (ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'f'); });
  }
  static bool verification(const Json& value, const char* key) {
    if (!string(value, key)) return false;
    const auto& code = value.at(key).get_ref<const std::string&>();
    if (code.size() != 19) return false;
    for (size_t i = 0; i < code.size(); ++i)
      if (i == 4 || i == 9 || i == 14) { if (code[i] != ' ') return false; }
      else if (code[i] < '0' || code[i] > '9') return false;
    return true;
  }
  static bool valid_page(const Json& page) {
    if (!page.is_object() || !flag(page, "enabled") || !page.contains("retention_days") || !page.at("retention_days").is_number_integer() ||
        !page.contains("items") || !page.at("items").is_array() || page.at("items").size() > 50 || !page.contains("e2e") || !page.at("e2e").is_object()) return false;
    const auto& e2e = page.at("e2e");
    if (!verification(e2e, "code") || !string(e2e, "status") || (e2e.at("status") != "ready" && e2e.at("status") != "pending" && e2e.at("status") != "disabled")) return false;
    for (const auto* key : {"pending_devices", "undecryptable"}) if (e2e.contains(key) && (!e2e.at(key).is_number_unsigned() || e2e.at(key).get<uint64_t>() > 1000)) return false;
    const auto retention = page.at("retention_days").get<int64_t>();
    if (std::find(retentions.begin(), retentions.end(), retention) == retentions.end()) return false;
    for (const auto& item : page.at("items")) if (!item.is_object() || !hex(item, "id", 64) || !string(item, "text") || !flag(item, "pinned") || !string(item, "updated_at") || !string(item, "device")) return false;
    return true;
  }
  static bool valid_devices(const Json& status) {
    if (!status.is_object() || !flag(status, "enabled") || !string(status, "status") ||
        (status.at("status") != "ready" && status.at("status") != "pending" && status.at("status") != "disabled") || !verification(status, "code") || !hex(status, "device_id", 32) || !status.contains("devices") || !status.at("devices").is_array() || status.at("devices").size() > 20) return false;
    for (const auto& device : status.at("devices")) if (!device.is_object() || !hex(device, "device_id", 32) || !string(device, "name") || !string(device, "created_at") || !verification(device, "code") || !flag(device, "approved") || !flag(device, "current") ||
        device.at("current").get<bool>() != (device.at("device_id") == status.at("device_id"))) return false;
    return true;
  }
  void failed(const std::string& code) {
    const wchar_t* message = L"读取或操作失败，请刷新后重试。";
    if (code == "clipboard_source_changed") message = L"配置已更改，当前结果已清除。请刷新确认操作状态。";
    else if (code == "account_unauthorized" || code == "unauthorized") message = L"请先在账号页登录，再刷新云剪贴板。";
    else if (code == "relay_unauthorized") message = L"中继拒绝了令牌，请检查中继配置；未改走水杉账号。";
    else if (code == "clipboard_device_pending") message = L"本机等待已有设备批准，请刷新查看核对码。";
    else if (code == "clipboard_sync_disabled") message = L"云剪贴板已关闭，请刷新后开启。";
    else if (code == "invalid_cloud_clipboard") message = L"内容或响应不合规。文字不能为空，最多 4000 个 UTF-16 单元。";
    else if (code == "cloud_clipboard_refresh_failed") message = L"操作已返回成功，但最新状态无法读取。请刷新确认，避免重复操作。";
    SetWindowTextW(get(cloud_clipboard_status), L"当前云端状态不可用。");
    SetWindowTextW(get(cloud_clipboard_notice), message); update();
  }
  void invalidate() {
    binding_.reset(); ready_ = false; status_ready_ = false; items_.clear(); devices_.clear();
    for (int id : {cloud_clipboard_items, cloud_clipboard_devices}) SendMessageW(get(id), LB_RESETCONTENT, 0, 0);
    for (int id : {cloud_clipboard_detail, cloud_clipboard_device_detail, cloud_clipboard_metadata}) SetWindowTextW(get(id), L"");
    SetWindowTextW(get(cloud_clipboard_status), L"请刷新读取当前来源的云端状态。"); update();
  }
  Json bound(const Json& action) const {
    Json request{{"action", action}};
    if (binding_) request["binding"] = *binding_;
    request["search"] = control_text::read(get(cloud_clipboard_search), 1024).value_or("");
    return request;
  }
  const Json* selected(int id, const std::vector<Json>& values) const {
    const auto index = SendMessageW(get(id), LB_GETCURSEL, 0, 0);
    return index >= 0 && static_cast<size_t>(index) < values.size() ? &values[static_cast<size_t>(index)] : nullptr;
  }
  void item_detail() {
    const auto item = selected(cloud_clipboard_items, items_);
    const auto metadata = item ? L"来源设备：" + control_text::wide(item->at("device").get<std::string>()) + L"\r\n更新时间：" + control_text::wide(item->at("updated_at").get<std::string>()) : L"";
    SetWindowTextW(get(cloud_clipboard_metadata), metadata.c_str());
    SetWindowTextW(get(cloud_clipboard_detail), item ? control_text::wide(item->at("text").get<std::string>()).c_str() : L"");
  }
  void device_detail() {
    const auto device = selected(cloud_clipboard_devices, devices_);
    const auto label = device ? control_text::wide(device->at("name").get<std::string>()) + L"\r\n核对码：" +
        control_text::wide(device->at("code").get<std::string>()) + L"\r\n登记时间：" + control_text::wide(device->at("created_at").get<std::string>()) : L"";
    SetWindowTextW(get(cloud_clipboard_device_detail), label.c_str());
  }
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  void update() {
    for (const auto& control : controls_) EnableWindow(control.second, enabled_);
    for (int id : {cloud_clipboard_enabled}) EnableWindow(get(id), enabled_ && status_ready_);
    for (int id : {cloud_clipboard_retention, cloud_clipboard_text, cloud_clipboard_add, cloud_clipboard_clear}) EnableWindow(get(id), enabled_ && ready_);
    EnableWindow(get(cloud_clipboard_reset), enabled_ && status_ready_ && remote_enabled_);
    const auto item = selected(cloud_clipboard_items, items_);
    for (int id : {cloud_clipboard_copy, cloud_clipboard_pin, cloud_clipboard_delete}) EnableWindow(get(id), enabled_ && ready_ && item);
    const auto device = selected(cloud_clipboard_devices, devices_);
    const bool other = device && device->at("current") == false;
    const bool approved = device && device->at("approved") == true;
    EnableWindow(get(cloud_clipboard_approve), enabled_ && ready_ && other && !approved);
    EnableWindow(get(cloud_clipboard_reject), enabled_ && ready_ && other && !approved);
    EnableWindow(get(cloud_clipboard_remove), enabled_ && ready_ && other && approved);
  }
  static inline constexpr std::array<int, 4> retentions{0,1,7,30};
  std::map<int, HWND> controls_;
  std::vector<Json> items_, devices_;
  Json config_ = Json::object();
  std::optional<Json> binding_;
  bool enabled_ = false, ready_ = false, status_ready_ = false, remote_enabled_ = false, config_loaded_ = false;
  int retention_ = 0;
};
} // namespace msime::settings::win7
