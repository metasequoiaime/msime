#pragma once

#include "ControlText.h"
#include "PreferencesStore.h"
#include "ThemeData.h"
#include <charconv>
#include <commctrl.h>
#include <functional>
#include <map>

namespace msime::settings::win7 {
class VocabularyForm {
public:
  enum Id {
    books = 2300, refresh_button, summary, daily, session_limit, save_settings,
    import_book, remove_book, reset_progress, start_review, card, reveal, known, unknown, notice,
  };
  struct Command {
    bool handled = false, file = false;
    std::optional<Json> request;
    std::wstring confirmation;
  };
  int create(const std::function<HWND(const wchar_t*, const wchar_t*, DWORD, int, int, int)>& row) {
    int top = 0;
    const auto add = [&](int id, const wchar_t* type, const wchar_t* title, DWORD style, int height = 28) {
      auto control = row(type, title, style, id, top, height);
      if (id >= 0) controls_[id] = control;
      top += (std::wstring_view(type) == WC_COMBOBOXW ? 28 : height) + 12;
    };
    add(-1, L"STATIC", L"背单词：导入自己的词表或使用已安装的单词本插件，复习进度保存在本机。", SS_LEFT, 56);
    add(books, WC_COMBOBOXW, L"词书", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST, 300);
    add(refresh_button, L"BUTTON", L"刷新词书与进度", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(summary, L"STATIC", L"", SS_LEFT, 64);
    add(-1, L"STATIC", L"每日新词数量", SS_LEFT);
    add(daily, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL | ES_NUMBER);
    add(-1, L"STATIC", L"每次复习最多出题数量", SS_LEFT);
    add(session_limit, L"EDIT", L"", WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL | ES_NUMBER);
    SendMessageW(get(daily), EM_SETLIMITTEXT, 10, 0); SendMessageW(get(session_limit), EM_SETLIMITTEXT, 10, 0);
    add(save_settings, L"BUTTON", L"保存复习设置", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(import_book, L"BUTTON", L"导入词表（CSV / TXT）…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(-1, L"STATIC", L"每行填写单词和释义，可带音标；文本支持 UTF-8、UTF-16、GB18030，文件最多 1 MiB。插件词书在插件页卸载。", SS_LEFT, 60);
    add(remove_book, L"BUTTON", L"删除所选词书…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(reset_progress, L"BUTTON", L"清空全部复习进度…", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(start_review, L"BUTTON", L"开始复习", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(card, L"EDIT", L"", WS_BORDER | ES_READONLY | ES_MULTILINE | WS_VSCROLL, 180);
    add(reveal, L"BUTTON", L"显示释义", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(known, L"BUTTON", L"认识", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(unknown, L"BUTTON", L"不认识", WS_TABSTOP | BS_PUSHBUTTON, 32);
    add(notice, L"STATIC", L"", SS_LEFT, 96);
    return top;
  }
  static Json load_request() { return Json{{"operation", "load"}}; }
  void enable(bool enabled) { enabled_ = enabled; update(); }
  Command command(WPARAM event) {
    const int id = LOWORD(event), notification = HIWORD(event);
    if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (!enabled_ || !IsWindowEnabled(get(id))) return result;
    if (id == books && notification == CBN_SELCHANGE) { reviewing_ = false; show_card(); update(); return result; }
    if (notification != BN_CLICKED) return result;
    if (id == refresh_button) result.request = load_request();
    else if (id == import_book) { result.request = Json{{"operation", "import_file"}}; result.file = true; }
    else if (id == save_settings) {
      const auto selected = selected_book();
      if (!selected) return result;
      uint32_t number = 0, limit = 0;
      if (!read_number(daily, number) || !read_number(session_limit, limit)) {
        SetWindowTextW(get(notice), L"新词和每次出题数量必须是非负整数，每次出题数量至少为 1。"); return result;
      }
      result.request = Json{{"operation", "set_settings"}, {"wordbook", selected->at("id")}, {"new_per_day", number}, {"session_limit", limit}};
    } else if (id == remove_book) {
      const auto selected = selected_book();
      if (!selected) return result;
      result.request = Json{{"operation", "remove"}, {"wordbook", selected->at("id")}};
      result.confirmation = L"删除所选词书及其复习进度？此操作无法撤销。";
    } else if (id == reset_progress) {
      result.request = Json{{"operation", "reset"}};
      result.confirmation = L"清空全部词书的复习进度？已经学过的词会从头开始，此操作无法撤销。";
    } else if (id == start_review) {
      const auto selected = selected_book();
      if (!selected) return result;
      if (selected->at("id") != status_->at("settings").at("wordbook")) {
        SetWindowTextW(get(notice), L"请先保存所选词书，再开始复习。"); return result;
      }
      reviewing_ = true; revealed_ = false; show_card(); update();
    } else if (id == reveal) { revealed_ = true; show_card(); update(); }
    else if ((id == known || id == unknown) && current_card())
      result.request = Json{{"operation", "answer"}, {"word", current_card()->at("word")}, {"known", id == known}};
    return result;
  }
  void completed(const Json& response, std::string_view operation = "load") {
    const auto value = theme_data::value(response);
    if (!value || !valid(*value)) {
      ready_ = false; reviewing_ = false; SetWindowTextW(get(card), L"");
      const auto error = response.is_object() && response.contains("error") && response.at("error").is_string() ? response.at("error").get<std::string>() : "";
      SetWindowTextW(get(notice), error == "vocabulary_import_file" ? L"无法读取词表，或文件超过 1 MiB，请检查文件或拆分后导入。" :
          error == "vocabulary_encoding" ? L"词表编码无法读取，请使用 UTF-8、UTF-16 或 GB18030。" : L"读取或保存背单词进度失败，请刷新后重试。已有进度不会自动清空。"); update(); return;
    }
    status_ = *value; ready_ = true; revealed_ = false;
    if (operation != "answer" && operation != "load") reviewing_ = false;
    SendMessageW(get(books), CB_RESETCONTENT, 0, 0);
    const auto& settings = status_->at("settings");
    int index = 0;
    for (const auto& book : status_->at("wordbooks")) {
      const auto label = control_text::wide(book.at("name").get<std::string>()) + L"（" + control_text::wide(book.at("total").dump()) + L"）";
      SendMessageW(get(books), CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
      if (book.at("id") == settings.at("wordbook")) SendMessageW(get(books), CB_SETCURSEL, index, 0);
      ++index;
    }
    const auto metrics = L"今日待复习：" + control_text::wide(status_->at("due").dump()) + L"　已完成：" +
        control_text::wide(status_->at("answeredToday").dump()) + L"　未开始：" + control_text::wide(status_->at("remaining").dump());
    SetWindowTextW(get(summary), metrics.c_str());
    SetWindowTextW(get(daily), control_text::wide(settings.at("newPerDay").dump()).c_str());
    SetWindowTextW(get(session_limit), control_text::wide(settings.at("sessionLimit").dump()).c_str());
    SetWindowTextW(get(notice), L""); show_card(); update();
  }
private:
  static bool count(const Json& value, const char* key) {
    return value.contains(key) && (value.at(key).is_number_unsigned() || (value.at(key).is_number_integer() && value.at(key).get<int64_t>() >= 0));
  }
  static bool valid(const Json& value) {
    if (!value.is_object() || !value.contains("wordbooks") || !value.at("wordbooks").is_array() || !value.contains("queue") || !value.at("queue").is_array()) return false;
    for (const auto* key : {"due", "answeredToday", "remaining"}) if (!count(value, key)) return false;
    if (!value.contains("settings") || !value.at("settings").is_object()) return false;
    const auto& settings = value.at("settings");
    if (!settings.contains("wordbook") || !settings.at("wordbook").is_string() || !count(settings, "newPerDay") || !count(settings, "sessionLimit")) return false;
    for (const auto& book : value.at("wordbooks")) {
      if (!book.is_object() || !book.contains("id") || !book.at("id").is_string() || !book.contains("name") || !book.at("name").is_string() ||
          !count(book, "total") || !book.contains("builtin") || !book.at("builtin").is_boolean() || (book.contains("pack") && !book.at("pack").is_boolean())) return false;
    }
    for (const auto& entry : value.at("queue")) {
      if (!entry.is_object()) return false;
      for (const auto* key : {"word", "meaning", "phonetic"}) if (!entry.contains(key) || !entry.at(key).is_string()) return false;
    }
    return true;
  }
  bool read_number(int id, uint32_t& number) const {
    const auto text = control_text::read(get(id), 10);
    if (!text || text->empty()) return false;
    const auto result = std::from_chars(text->data(), text->data() + text->size(), number);
    return result.ec == std::errc{} && result.ptr == text->data() + text->size();
  }
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  const Json* selected_book() const {
    const auto index = SendMessageW(get(books), CB_GETCURSEL, 0, 0);
    if (!ready_ || !status_ || index < 0 || static_cast<size_t>(index) >= status_->at("wordbooks").size()) return nullptr;
    return &status_->at("wordbooks").at(static_cast<size_t>(index));
  }
  const Json* current_card() const {
    return ready_ && reviewing_ && status_ && !status_->at("queue").empty() ? &status_->at("queue").front() : nullptr;
  }
  void show_card() {
    const auto current = current_card();
    std::wstring text;
    if (current) {
      text = control_text::wide(current->at("word").get<std::string>());
      text += L"\r\n" + control_text::wide(current->at("phonetic").get<std::string>());
      if (revealed_) text += L"\r\n\r\n" + control_text::wide(current->at("meaning").get<std::string>());
    } else if (reviewing_) text = L"本次复习已完成。";
    SetWindowTextW(get(card), text.c_str());
  }
  void update() {
    for (const auto& control : controls_) EnableWindow(control.second, enabled_);
    const auto selected = selected_book();
    for (const int id : {books, daily, session_limit, save_settings, reset_progress}) EnableWindow(get(id), enabled_ && ready_);
    EnableWindow(get(remove_book), enabled_ && selected && selected->at("builtin") == false && !selected->value("pack", false));
    EnableWindow(get(save_settings), enabled_ && selected);
    EnableWindow(get(start_review), enabled_ && selected);
    EnableWindow(get(reveal), enabled_ && current_card() && !revealed_);
    for (const int id : {known, unknown}) EnableWindow(get(id), enabled_ && current_card() && revealed_);
  }
  std::map<int, HWND> controls_;
  std::optional<Json> status_;
  bool enabled_ = false, ready_ = false, reviewing_ = false, revealed_ = false;
};
} // namespace msime::settings::win7
