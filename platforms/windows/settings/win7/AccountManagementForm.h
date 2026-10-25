#pragma once

#include "ControlText.h"
#include "SettingsWindow.h"
#include "ThemeData.h"
#include "../AccountPageModel.h"
#include <array>
#include <functional>
#include <map>
#include <optional>
#include <vector>

namespace msime::settings::win7 {

// 只适配共享账号操作与控件；会话、分区和绑定码的最终校验仍由 Rust 负责。
class AccountManagementForm {
public:
  struct Command {
    bool handled = false;
    std::optional<Json> request;
    std::wstring confirmation;
    bool export_data = false;
  };

  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row) {
    int top = 0;
    const auto add = [&](int id, const wchar_t* type, const wchar_t* text, DWORD style, int height = 28) {
      auto window = row(type, text, style, id, top, height);
      if (id >= 0) controls_[id] = window;
      top += height + 12;
      return window;
    };
    add(-1, L"STATIC", L"QQ 群助手", SS_LEFT);
    add(-1, L"STATIC", account::qq_link_note.data(), SS_LEFT, 76);
    add(account_qq_refresh, L"BUTTON", L"刷新 QQ 绑定", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_qq_list, L"LISTBOX", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 112);
    add(account_qq_code, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_NUMBER | ES_AUTOHSCROLL);
    SendMessageW(get(account_qq_code), EM_LIMITTEXT, 8, 0);
    add(account_qq_redeem, L"BUTTON", L"使用机器人给的 8 位绑定码", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_qq_remove, L"BUTTON", L"解除所选 QQ 绑定", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_qq_notice, L"STATIC", L"", SS_LEFT, 60);
    add(-1, L"STATIC", L"我的设备", SS_LEFT);
    add(account_devices_refresh, L"BUTTON", L"刷新已登录设备", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_devices_list, L"LISTBOX", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 144);
    add(account_device_detail, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY, 100);
    add(account_device_revoke, L"BUTTON", L"退出所选设备", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_devices_notice, L"STATIC", L"", SS_LEFT, 60);
    add(-1, L"STATIC", L"云端数据", SS_LEFT);
    add(-1, L"STATIC", L"查看与导出账号的云端数据，或明确选择要清空的分区。清空不会删除本机数据；账号和社区作品请使用各自的管理入口。", SS_LEFT, 76);
    add(account_data_refresh, L"BUTTON", L"刷新云端数据概览", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_data_summary, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY, 144);
    add(account_data_export, L"BUTTON", L"导出云端数据为 ZIP…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    for (size_t i = 0; i < sections.size(); ++i)
      add(account_data_section_first + static_cast<int>(i), L"BUTTON", sections[i].second, WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY);
    add(account_data_delete, L"BUTTON", L"清空所选云端数据…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_voice_retry, L"BUTTON", L"重试删除剩余语音样本", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(account_data_notice, L"STATIC", L"", SS_LEFT, 100);
    return top;
  }

  void enable(bool enabled, bool signed_in) { enabled_ = enabled; signed_in_ = signed_in; update(); }

  void clear() {
    devices_.clear(); links_.clear(); devices_ready_ = links_ready_ = false;
    SendMessageW(get(account_devices_list), LB_RESETCONTENT, 0, 0);
    SendMessageW(get(account_qq_list), LB_RESETCONTENT, 0, 0);
    for (int id : {account_device_detail, account_devices_notice, account_data_summary, account_qq_notice, account_qq_code}) SetWindowTextW(get(id), L"");
    for (size_t i = 0; i < sections.size(); ++i) SendMessageW(get(account_data_section_first + static_cast<int>(i)), BM_SETCHECK, BST_UNCHECKED, 0);
    // 语音样本属于本机匿名身份；换号或退出不能抹掉尚未清理的结果。
    if (!voice_remaining_) SetWindowTextW(get(account_data_notice), L"");
    update();
  }

  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (!enabled_) return result;
    if (id == account_qq_code && notification == EN_CHANGE) { update(); return result; }
    if ((id == account_devices_list || id == account_qq_list) && notification == LBN_SELCHANGE) {
      if (id == account_devices_list) device_detail();
      update(); return result;
    }
    if (notification != BN_CLICKED || !IsWindowEnabled(get(id))) return result;
    if (id >= account_data_section_first && id < account_data_section_first + static_cast<int>(sections.size())) { update(); return result; }
    if (id == account_devices_refresh) { devices_ready_ = false; result.request = Json{{"operation", "sessions_list"}}; }
    else if (id == account_device_revoke) {
      const auto device = selected(account_devices_list, devices_);
      if (!device) return result;
      result.request = Json{{"operation", "session_revoke"}, {"id", device->at("id")}};
      result.confirmation = flag(*device, "current") ? L"退出当前设备？这台设备需要重新登录。" : L"退出所选设备？该设备需要重新登录。";
    } else if (id == account_qq_refresh) { links_ready_ = false; result.request = Json{{"operation", "qq_links"}}; }
    else if (id == account_qq_redeem) {
      result.request = Json{{"operation", "qq_link_redeem"}, {"code", control_text::read(get(account_qq_code), 8).value_or("")}};
    } else if (id == account_qq_remove) {
      const auto link = selected(account_qq_list, links_);
      if (!link) return result;
      result.request = Json{{"operation", "qq_link_delete"}, {"id", link->at("id")}};
      result.confirmation = L"解除所选 QQ 绑定？对应群友将无法再通过机器人查看你的打字统计。";
    } else if (id == account_data_refresh) result.request = Json{{"operation", "data_summary"}};
    else if (id == account_data_export) result.export_data = true;
    else if (id == account_data_delete) {
      Json chosen = Json::array();
      result.confirmation = L"清空以下云端数据？此操作无法撤销，本机数据保持不变：";
      for (size_t i = 0; i < sections.size(); ++i) if (SendMessageW(get(account_data_section_first + static_cast<int>(i)), BM_GETCHECK, 0, 0) == BST_CHECKED) {
        chosen.push_back(sections[i].first); result.confirmation += L"\r\n• " + std::wstring(sections[i].second);
      }
      if (!chosen.empty()) result.request = Json{{"operation", "data_delete"}, {"sections", chosen}};
    } else if (id == account_voice_retry) {
      result.request = Json{{"operation", "voice_delete"}};
      result.confirmation = L"重试删除这台设备匿名账号名下剩余的语音样本？不会再次清空其他云端分区。";
    }
    update(); return result;
  }

  std::optional<Json> completed(const Json& request, const Json& response) {
    const auto operation = request.value("operation", std::string{});
    const bool devices = operation == "sessions_list" || operation == "session_revoke";
    const bool qq = operation.rfind("qq_", 0) == 0;
    const int notice = devices ? account_devices_notice : qq ? account_qq_notice : account_data_notice;
    const auto value = theme_data::value(response);
    if (!value) {
      const auto error = string(response, "error");
      SetWindowTextW(get(notice), (qq ? account::qq_link_message(control_text::wide(error)) : account::error_message(control_text::wide(error))).c_str());
      update(); return std::nullopt;
    }
    if (operation == "sessions_list" || operation == "qq_links") {
      const char* key = devices ? "sessions" : "links";
      if (!value->is_object() || !value->contains(key) || !value->at(key).is_array()) {
        SetWindowTextW(get(notice), L"返回的列表无法读取，请刷新后重试。"); update(); return std::nullopt;
      }
      auto& rows = devices ? devices_ : links_;
      const int list = devices ? account_devices_list : account_qq_list;
      const auto previous = selected(list, rows);
      const auto old_id = previous ? string(*previous, "id") : std::string{};
      rows.clear(); SendMessageW(get(list), LB_RESETCONTENT, 0, 0);
      for (const auto& item : value->at(key)) {
        if (!item.is_object() || string(item, "id").empty()) continue;
        const auto label = devices ? (flag(item, "current") ? L"[当前设备] " : L"") + control_text::wide(string(item, "name")) + L"　" + control_text::wide(string(item, "platform")) : L"绑定于 " + control_text::wide(string(item, "created_at"));
        const auto index = SendMessageW(get(list), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
        if (index == LB_ERR || index == LB_ERRSPACE) continue;
        rows.push_back(item);
        if (string(item, "id") == old_id) SendMessageW(get(list), LB_SETCURSEL, index, 0);
      }
      if (devices) { devices_ready_ = true; device_detail(); } else links_ready_ = true;
      SetWindowTextW(get(notice), rows.empty() ? L"没有记录。" : L"列表已刷新。");
    } else if (operation == "session_revoke") {
      if (!value->is_object() || !value->contains("signed_out") || !value->at("signed_out").is_boolean()) {
        SetWindowTextW(get(notice), L"无法读取退出结果，请刷新登录状态。"); return Json{{"operation", "status"}};
      }
      devices_ready_ = false;
      SetWindowTextW(get(notice), L"已退出所选设备。");
      update(); return Json{{"operation", flag(*value, "signed_out") ? "status" : "sessions_list"}};
    } else if (operation == "qq_link_redeem" || operation == "qq_link_delete") {
      if (operation == "qq_link_redeem") SetWindowTextW(get(account_qq_code), L"");
      links_ready_ = false; update(); return Json{{"operation", "qq_links"}};
    } else if (operation == "data_summary") {
      if (!value->is_object() || !value->contains("sections") || !value->at("sections").is_array()) {
        SetWindowTextW(get(notice), L"无法读取云端数据概览。"); return std::nullopt;
      }
      std::wstring text;
      for (const auto& item : value->at("sections")) {
        if (!item.is_object() || !item.contains("items") || !item.at("items").is_number_integer() || !item.contains("bytes") || !item.at("bytes").is_number_integer()) continue;
        auto label = control_text::wide(string(item, "id"));
        for (const auto& section : sections) if (string(item, "id") == section.first) label = section.second;
        text += label + L"：" + control_text::wide(item.at("items").dump()) + L" 项，约 " + control_text::wide(item.at("bytes").dump()) + L" 字节\r\n";
      }
      if (string(*value, "anonymous_voice") == "unknown") text += L"匿名语音样本的数量暂时无法读取，以上数值可能不包含它们。";
      SetWindowTextW(get(account_data_summary), text.c_str());
    } else if (operation == "data_export") {
      if (!value->is_object() || string(*value, "path") != string(request, "destination") ||
          !value->contains("bytes") || !value->at("bytes").is_number_unsigned())
        SetWindowTextW(get(notice), L"无法读取导出结果，请检查所选文件后重试。");
      else SetWindowTextW(get(notice), L"云端数据已导出到所选 ZIP 文件。");
    } else if (operation == "data_delete") {
      partial_voice(*value);
      SetWindowTextW(get(notice), voice_remaining_ ? L"所选分区已清空，语音样本仍有未删除部分。请使用单独的重试按钮。" : L"所选云端分区已清空。");
      for (size_t i = 0; i < sections.size(); ++i) SendMessageW(get(account_data_section_first + static_cast<int>(i)), BM_SETCHECK, BST_UNCHECKED, 0);
      update(); return Json{{"operation", "data_summary"}};
    } else if (operation == "voice_delete") {
      voice_remaining_ = false; SetWindowTextW(get(notice), L"剩余语音样本已删除。");
    }
    update(); return std::nullopt;
  }

  void account_deleted(const Json& value) {
    partial_voice(value);
    if (voice_remaining_) SetWindowTextW(get(account_data_notice), L"账号已注销，但语音样本仍有未删除部分，可单独重试删除。");
    update();
  }

  static bool handles(const std::string& operation) {
    return operation == "sessions_list" || operation == "session_revoke" || operation == "data_summary" || operation == "data_export" || operation == "data_delete" || operation == "voice_delete" || operation.rfind("qq_", 0) == 0;
  }

private:
  static inline constexpr std::array<std::pair<const char*, const wchar_t*>, 5> sections{{
      {"preferences", L"云端设置"}, {"dictionary", L"云词库"}, {"phrases", L"常用语"}, {"clipboard", L"云剪贴板"}, {"voice", L"语音样本"}}};
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  static std::string string(const Json& value, const char* key) { return value.is_object() && value.contains(key) && value.at(key).is_string() ? value.at(key).get<std::string>() : std::string{}; }
  static bool flag(const Json& value, const char* key) { return value.is_object() && value.contains(key) && value.at(key) == true; }
  const Json* selected(int id, const std::vector<Json>& rows) const {
    const auto index = SendMessageW(get(id), LB_GETCURSEL, 0, 0);
    return index >= 0 && static_cast<size_t>(index) < rows.size() ? &rows[static_cast<size_t>(index)] : nullptr;
  }
  void device_detail() {
    const auto item = selected(account_devices_list, devices_);
    const auto text = item ? L"版本：" + control_text::wide(string(*item, "app_version")) + L"\r\n登录时间：" + control_text::wide(string(*item, "created_at")) + L"\r\n最近活跃：" + control_text::wide(string(*item, "last_active")) : std::wstring{};
    SetWindowTextW(get(account_device_detail), text.c_str());
  }
  void partial_voice(const Json& value) {
    if (value.is_object() && value.contains("voice_samples") && value.at("voice_samples").is_object() && value.at("voice_samples").contains("ok"))
      voice_remaining_ = value.at("voice_samples").at("ok") == false;
  }
  void update() {
    for (const auto& control : controls_) EnableWindow(control.second, enabled_ && signed_in_);
    EnableWindow(get(account_device_revoke), enabled_ && signed_in_ && devices_ready_ && selected(account_devices_list, devices_));
    EnableWindow(get(account_qq_remove), enabled_ && signed_in_ && links_ready_ && selected(account_qq_list, links_));
    const auto code = control_text::read(get(account_qq_code), 8).value_or("");
    EnableWindow(get(account_qq_redeem), enabled_ && signed_in_ && links_ready_ && account::eight_digits(control_text::wide(code)));
    bool any = false;
    for (size_t i = 0; i < sections.size(); ++i) any = any || SendMessageW(get(account_data_section_first + static_cast<int>(i)), BM_GETCHECK, 0, 0) == BST_CHECKED;
    EnableWindow(get(account_data_delete), enabled_ && signed_in_ && any);
    EnableWindow(get(account_voice_retry), enabled_ && voice_remaining_);
    EnableWindow(get(account_data_notice), enabled_);
  }
  std::map<int, HWND> controls_;
  std::vector<Json> devices_, links_;
  bool enabled_ = false, signed_in_ = false, devices_ready_ = false, links_ready_ = false, voice_remaining_ = false;
};
} // namespace msime::settings::win7
