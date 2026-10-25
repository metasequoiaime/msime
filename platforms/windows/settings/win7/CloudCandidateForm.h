#pragma once

#include "PreferencesStore.h"
#include "SettingsWindow.h"
#include "ControlText.h"
#include <commctrl.h>
#include <array>
#include <charconv>
#include <functional>
#include <string>

namespace msime::settings::win7 {

// 表单只维护尚未提交的控件内容；接口、模板、路径和超时规则仍由 Rust 校验。
class CloudCandidateForm {
public:
  using Row = std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>;

  int create(const Row& row, int top) {
    const auto choice = [&](int id, const wchar_t* title, const auto& labels) {
      row(L"STATIC", title, SS_LEFT, -1, top, 24);
      const HWND control = row(WC_COMBOBOXW, title, WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL,
          id, top + 32, 180);
      for (const auto* label : labels) SendMessageW(control, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label));
      controls_[static_cast<size_t>(id - cloud_source)] = control;
      top += 82;
    };
    const auto field = [&](int id, const wchar_t* title, const wchar_t* description, int limit, bool multiline) {
      row(L"STATIC", title, SS_LEFT, -1, top, 24);
      row(L"STATIC", description, SS_LEFT, -1, top + 30, 48);
      const int height = multiline ? 88 : 28;
      const HWND control = row(L"EDIT", L"", WS_TABSTOP | WS_BORDER |
          (multiline ? ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL | WS_VSCROLL : ES_AUTOHSCROLL),
          id, top + 86, height);
      SendMessageW(control, EM_LIMITTEXT, limit, 0);
      controls_[static_cast<size_t>(id - cloud_source)] = control;
      top += height + 108;
    };
    choice(cloud_source, L"云候选来源", std::array{L"Google input-tools", L"自定义 HTTP 接口"});
    field(cloud_endpoint, L"接口地址", L"自定义接口使用 HTTPS 或本机 HTTP。GET 用 {query} 填入当前拼写。", 2048, false);
    choice(cloud_method, L"请求方法", std::array{L"GET", L"POST"});
    field(cloud_headers, L"请求头（JSON）", L"例如 {\"Authorization\":\"Bearer …\"}，仅保存在本机。", 4096, true);
    field(cloud_body, L"请求体模板（JSON）", L"自定义 POST 请求使用；{query} 只在 JSON 字符串值中替换并自动转义。", 4096, true);
    choice(cloud_response_format, L"响应格式", std::array{L"标准 candidates 数组", L"Google input-tools", L"JSON 路径"});
    field(cloud_response_path, L"JSON 路径", L"自定义 JSON 路径响应使用；点分对象键或数组下标，例如 choices.0.message.content。", 128, false);
    field(cloud_timeout, L"超时（毫秒）", L"100–10000 毫秒。", 5, false);
    controls_[cloud_cache - cloud_source] = row(L"BUTTON", L"缓存云候选", WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY,
        cloud_cache, top, 28);
    row(L"STATIC", L"仅在本机内存缓存，按来源隔离；无结果缓存 60 秒。", SS_LEFT, -1, top + 36, 48);
    controls_[cloud_save - cloud_source] = row(L"BUTTON", L"保存云候选服务", WS_TABSTOP | BS_PUSHBUTTON,
        cloud_save, top + 100, 32);
    return top + 154;
  }

  static bool valid(const Json& value) {
    if (!value.is_object()) return false;
    for (const auto* field : {"source", "endpoint", "method", "headers_json", "body_template", "response_format", "response_path"})
      if (!value.contains(field) || !value.at(field).is_string()) return false;
    return value.contains("timeout_ms") && value.at("timeout_ms").is_number_unsigned() &&
        value.contains("cache_enabled") && value.at("cache_enabled").is_boolean() &&
        (value.at("source") == "google" || value.at("source") == "custom") &&
        (value.at("method") == "GET" || value.at("method") == "POST") &&
        (value.at("response_format") == "candidates" || value.at("response_format") == "google" ||
            value.at("response_format") == "json_path");
  }

  void refresh(const Json& value) {
    // 无关偏好保存后不丢表单草稿；实际服务配置改变时才从新快照重新填充。
    if (base_ == value) return;
    base_ = value;
    SendMessageW(control(cloud_source), CB_SETCURSEL, value.at("source") == "custom" ? 1 : 0, 0);
    SendMessageW(control(cloud_method), CB_SETCURSEL, value.at("method") == "POST" ? 1 : 0, 0);
    SendMessageW(control(cloud_response_format), CB_SETCURSEL,
        value.at("response_format") == "json_path" ? 2 : value.at("response_format") == "google" ? 1 : 0, 0);
    for (const auto& field : text_fields)
      SetWindowTextW(control(field.first), control_text::wide(value.at(field.second).get<std::string>()).c_str());
    SetWindowTextW(control(cloud_timeout), std::to_wstring(value.at("timeout_ms").get<uint64_t>()).c_str());
    SendMessageW(control(cloud_cache), BM_SETCHECK, value.at("cache_enabled").get<bool>() ? BST_CHECKED : BST_UNCHECKED, 0);
  }

  void enable(bool enabled) {
    const bool custom = SendMessageW(control(cloud_source), CB_GETCURSEL, 0, 0) == 1;
    for (const auto item : controls_) EnableWindow(item, enabled);
    for (int id : {cloud_endpoint, cloud_method, cloud_headers, cloud_response_format})
      EnableWindow(control(id), enabled && custom);
    EnableWindow(control(cloud_body), enabled && custom && SendMessageW(control(cloud_method), CB_GETCURSEL, 0, 0) == 1);
    EnableWindow(control(cloud_response_path), enabled && custom &&
        SendMessageW(control(cloud_response_format), CB_GETCURSEL, 0, 0) == 2);
  }

  std::optional<Json> document() const {
    Json result = base_;
    const auto source = SendMessageW(control(cloud_source), CB_GETCURSEL, 0, 0);
    const auto method = SendMessageW(control(cloud_method), CB_GETCURSEL, 0, 0);
    const auto format = SendMessageW(control(cloud_response_format), CB_GETCURSEL, 0, 0);
    if (!result.is_object() || source < 0 || source > 1 || method < 0 || method > 1 || format < 0 || format > 2)
      return std::nullopt;
    result["source"] = source == 1 ? "custom" : "google";
    result["method"] = method == 1 ? "POST" : "GET";
    result["response_format"] = format == 2 ? "json_path" : format == 1 ? "google" : "candidates";
    for (const auto& field : text_fields) {
      const auto value = control_text::read(control(field.first));
      if (!value) return std::nullopt;
      result[field.second] = *value;
    }
    const auto timeout = control_text::read(control(cloud_timeout));
    if (!timeout) return std::nullopt;
    uint64_t number = 0;
    const auto parsed = std::from_chars(timeout->data(), timeout->data() + timeout->size(), number);
    if (parsed.ec != std::errc{} || parsed.ptr != timeout->data() + timeout->size()) return std::nullopt;
    result["timeout_ms"] = number;
    result["cache_enabled"] = SendMessageW(control(cloud_cache), BM_GETCHECK, 0, 0) == BST_CHECKED;
    return result;
  }

private:
  static constexpr std::array<std::pair<int, const char*>, 4> text_fields{{
      {cloud_endpoint, "endpoint"}, {cloud_headers, "headers_json"}, {cloud_body, "body_template"},
      {cloud_response_path, "response_path"},
  }};

  HWND control(int id) const { return controls_[static_cast<size_t>(id - cloud_source)]; }
  std::array<HWND, cloud_save - cloud_source + 1> controls_{};
  Json base_;
};

} // namespace msime::settings::win7
