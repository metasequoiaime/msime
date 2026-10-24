#pragma once

#include "ControlText.h"
#include "PreferencesStore.h"
#include "SettingsWindow.h"
#include "ThemeData.h"
#include "../HistoryPagination.h"
#include <algorithm>
#include <array>
#include <limits>
#include <functional>
#include <map>
#include <optional>
#include <vector>

namespace msime::settings::win7 {
namespace clipboard_paging = msime::settings;

class ClipboardForm {
public:
  struct Command {
    bool handled = false;
    std::vector<Change> changes;
    std::optional<Json> request;
    std::wstring confirmation;
    std::optional<std::wstring> copy;
  };

  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row) {
    int top = 0;
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      const auto control = row(type, title, style, id, top, height);
      if (id >= 0) controls_[id] = control;
      top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
      return control;
    };
    add(clipboard_enabled, L"BUTTON", L"剪贴板历史", WS_TABSTOP | BS_AUTOCHECKBOX | BS_NOTIFY);
    add(-1, L"STATIC", L"开启后记录复制的文本。关闭并保存后清空已保存的记录；密码管理器标记为私密的内容不采集。", SS_LEFT, 62);
    add(-1, L"STATIC", L"历史记录上限：达到上限后替换最旧的未固定记录。", SS_LEFT, 42);
    add(clipboard_limit, WC_COMBOBOXW, L"历史记录上限", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST, 180);
    for (int limit : limits) {
      const auto label = std::to_wstring(limit) + L" 条";
      SendMessageW(get(clipboard_limit), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
    }
    add(clipboard_refresh, L"BUTTON", L"刷新历史记录", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(clipboard_page, L"STATIC", L"", SS_LEFT);
    add(clipboard_previous, L"BUTTON", L"上一页", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(clipboard_next, L"BUTTON", L"下一页", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(clipboard_list, L"LISTBOX", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT, 200);
    add(clipboard_timestamp, L"STATIC", L"", SS_LEFT);
    add(clipboard_detail, L"EDIT", L"", WS_TABSTOP | WS_BORDER | WS_VSCROLL | ES_MULTILINE | ES_READONLY, 144);
    add(clipboard_pin, L"BUTTON", L"固定 / 取消固定", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(clipboard_copy, L"BUTTON", L"重新复制", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(clipboard_remove, L"BUTTON", L"删除所选记录", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(clipboard_clear, L"BUTTON", L"清空全部历史记录…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(clipboard_notice, L"STATIC", L"", SS_LEFT, 84);
    return top;
  }

  void refresh(const Json& preferences) {
    history_enabled_ = preferences.contains("clipboard_history") && preferences.at("clipboard_history") == true;
    SendMessageW(get(clipboard_enabled), BM_SETCHECK, history_enabled_ ? BST_CHECKED : BST_UNCHECKED, 0);
    const auto count = preferences.value("clipboard_history_limit", 50);
    current_limit_ = count;
    int selected = -1;
    for (size_t i = 0; i < limits.size(); ++i) if (limits[i] == count) selected = static_cast<int>(i);
    SendMessageW(get(clipboard_limit), CB_SETCURSEL, selected, 0);
    if (!history_enabled_) { entries_.clear(); offset_ = 0; ready_ = false; render(); }
    update();
  }
  void enable(bool enabled) { enabled_ = enabled; update(); }
  void cancelled(const Json& preferences) { refresh(preferences); }
  void copy_failed() { SetWindowTextW(get(clipboard_notice), L"无法复制到系统剪贴板，请稍后重试。"); }

  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (!enabled_ || !IsWindowEnabled(get(id))) return result;
    if (id == clipboard_list && notification == LBN_SELCHANGE) { detail(); update(); return result; }
    if (id == clipboard_limit && notification == CBN_SELCHANGE) {
      const auto index = SendMessageW(get(id), CB_GETCURSEL, 0, 0);
      if (index >= 0 && static_cast<size_t>(index) < limits.size() && limits[static_cast<size_t>(index)] != current_limit_) {
        result.changes.push_back({{"clipboard_history_limit"}, limits[static_cast<size_t>(index)]});
        result.request = Json{{"operation", "list"}};
      }
      return result;
    }
    if (notification != BN_CLICKED) return result;
    if (id == clipboard_enabled) {
      const bool next = SendMessageW(get(id), BM_GETCHECK, 0, 0) == BST_CHECKED;
      if (next != history_enabled_) {
        result.changes.push_back({{"clipboard_history"}, next});
        result.request = Json{{"operation", next ? "sync" : "clear"}};
        if (!next) result.confirmation = L"关闭剪贴板历史并清空全部已保存记录？此操作无法撤销。";
      }
    } else if (id == clipboard_refresh) result.request = Json{{"operation", "list"}};
    else if (id == clipboard_previous || id == clipboard_next) {
      const auto paging = clipboard_paging::history_page(entries_.size(), offset_);
      offset_ = id == clipboard_previous ? (paging.offset >= paging.page_size ? paging.offset - paging.page_size : 0) : std::min(paging.offset + paging.page_size, paging.last_offset);
      render();
    } else if (id == clipboard_clear) {
      result.request = Json{{"operation", "clear"}};
      result.confirmation = L"清空全部剪贴板历史，包括固定的记录？此操作无法撤销。";
    } else if (const auto item = selected()) {
      if (id == clipboard_pin) result.request = Json{{"operation", "set_pinned"}, {"text", item->text}, {"pinned", !item->pinned}};
      else if (id == clipboard_remove) result.request = Json{{"operation", "remove"}, {"text", item->text}};
      else if (id == clipboard_copy) { result.copy = control_text::wide(item->text); result.request = Json{{"operation", "capture"}, {"text", item->text}}; }
    }
    return result;
  }

  void completed(const Json& response) {
    const auto value = theme_data::value(response);
    if (!value || !value->is_object() || !value->contains("enabled") || !value->at("enabled").is_boolean() ||
        !value->contains("entries") || !value->at("entries").is_array()) {
      ready_ = false;
      SetWindowTextW(get(clipboard_notice), L"读取或修改历史记录失败，原文件不会自动重置。请刷新后重试。");
      update(); return;
    }
    const auto previous = selected();
    const auto previous_text = previous ? previous->text : std::string{};
    std::vector<Entry> entries;
    for (const auto& item : value->at("entries")) {
      if (!item.is_object() || !item.contains("text") || !item.at("text").is_string() || !item.contains("pinned") || !item.at("pinned").is_boolean()) {
        ready_ = false; SetWindowTextW(get(clipboard_notice), L"历史记录返回内容无法读取，请刷新后重试。"); update(); return;
      }
      const auto timestamp = item.contains("timestampMs") && item.at("timestampMs").is_number_unsigned() ? item.at("timestampMs").get<uint64_t>() : 0;
      entries.push_back({item.at("text").get<std::string>(), item.at("pinned").get<bool>(), timestamp});
    }
    entries_ = std::move(entries); ready_ = true;
    if (!history_enabled_ || value->at("enabled") == false) entries_.clear();
    auto found = std::find_if(entries_.begin(), entries_.end(), [&](const Entry& item) { return item.text == previous_text; });
    if (found != entries_.end()) offset_ = static_cast<size_t>(std::distance(entries_.begin(), found)) / clipboard_paging::clipboard_history_page_size * clipboard_paging::clipboard_history_page_size;
    SetWindowTextW(get(clipboard_notice), history_enabled_ ? (entries_.empty() ? L"暂无历史记录。" : L"") : L"请先开启剪贴板历史。");
    render(previous_text);
  }

private:
  struct Entry { std::string text; bool pinned; uint64_t timestamp; };
  static inline constexpr std::array<int, 5> limits{50,100,200,500,1000};
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  const Entry* selected() const {
    const auto index = SendMessageW(get(clipboard_list), LB_GETCURSEL, 0, 0);
    const auto paging = clipboard_paging::history_page(entries_.size(), offset_);
    return index >= 0 && static_cast<size_t>(index) < paging.count ? &entries_[paging.offset + static_cast<size_t>(index)] : nullptr;
  }
  void detail() {
    const auto item = selected();
    const auto text = item ? control_text::wide(item->text) : std::wstring{};
    SetWindowTextW(get(clipboard_detail), text.c_str());
    SetWindowTextW(get(clipboard_timestamp), item ? time_label(item->timestamp).c_str() : L"");
    SetWindowTextW(get(clipboard_pin), item && item->pinned ? L"取消固定" : L"固定记录");
  }
  static std::wstring time_label(uint64_t milliseconds) {
    constexpr uint64_t epoch = 116444736000000000ull;
    if (!milliseconds || milliseconds > (std::numeric_limits<uint64_t>::max() - epoch) / 10000) return {};
    ULARGE_INTEGER ticks{}; ticks.QuadPart = epoch + milliseconds * 10000;
    const FILETIME utc{ticks.LowPart, ticks.HighPart};
    FILETIME local{}; SYSTEMTIME time{};
    wchar_t date[96]{}, clock[96]{};
    if (!FileTimeToLocalFileTime(&utc, &local) || !FileTimeToSystemTime(&local, &time) ||
        !GetDateFormatW(LOCALE_USER_DEFAULT, DATE_SHORTDATE, &time, nullptr, date, 96) ||
        !GetTimeFormatW(LOCALE_USER_DEFAULT, 0, &time, nullptr, clock, 96)) return {};
    return L"复制时间：" + std::wstring(date) + L" " + clock;
  }
  void render(const std::string& selected_text = {}) {
    const auto paging = clipboard_paging::history_page(entries_.size(), offset_); offset_ = paging.offset;
    SetWindowTextW(get(clipboard_page), clipboard_paging::history_page_status(paging).c_str());
    SendMessageW(get(clipboard_list), LB_RESETCONTENT, 0, 0);
    for (size_t i = 0; i < paging.count; ++i) {
      const auto& item = entries_[offset_ + i];
      auto label = control_text::wide(item.text);
      for (auto& ch : label) if (ch == L'\r' || ch == L'\n' || ch == L'\t') ch = L' ';
      if (label.size() > 96) {
        label.resize(96);
        if (label.back() >= 0xD800 && label.back() <= 0xDBFF) label.pop_back();
        label += L"…";
      }
      if (item.pinned) label = L"[固定] " + label;
      const auto index = SendMessageW(get(clipboard_list), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
      if (item.text == selected_text) SendMessageW(get(clipboard_list), LB_SETCURSEL, index, 0);
    }
    detail(); update();
  }
  void update() {
    for (const auto& item : controls_) EnableWindow(item.second, enabled_);
    const auto paging = clipboard_paging::history_page(entries_.size(), offset_);
    const bool managed = enabled_ && history_enabled_ && ready_;
    for (int id : {clipboard_list, clipboard_detail}) EnableWindow(get(id), managed);
    for (int id : {clipboard_pin, clipboard_copy, clipboard_remove}) EnableWindow(get(id), managed && selected());
    EnableWindow(get(clipboard_previous), managed && paging.offset > 0);
    EnableWindow(get(clipboard_next), managed && paging.offset < paging.last_offset);
    EnableWindow(get(clipboard_clear), managed && !entries_.empty());
  }
  std::map<int, HWND> controls_;
  std::vector<Entry> entries_;
  size_t offset_ = 0;
  int current_limit_ = 50;
  bool enabled_ = false, history_enabled_ = false, ready_ = false;
};
} // namespace msime::settings::win7
