#include "../../settings/win7/SettingsWindow.h"
#include "../../settings/win7/PreferencesStore.h"
#include "../../settings/win7/ThemeData.h"
#include "../../settings/win7/ControlText.h"
#include <cassert>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <functional>
#include <vector>

namespace {
using namespace msime::settings::win7;
struct TemporaryState {
  std::filesystem::path path;
  std::wstring previous;
  bool present = false;
  TemporaryState() {
    wchar_t root[MAX_PATH]{}, unique[MAX_PATH]{};
    assert(GetTempPathW(MAX_PATH, root));
    assert(GetTempFileNameW(root, L"m7c", 0, unique));
    assert(DeleteFileW(unique) && CreateDirectoryW(unique, nullptr)); path = unique;
    const auto length = GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", nullptr, 0);
    present = length != 0;
    if (present) { previous.resize(length); previous.resize(GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", previous.data(), length)); }
    assert(SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", path.c_str()));
  }
  ~TemporaryState() {
    SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", present ? previous.c_str() : nullptr);
    std::error_code error; std::filesystem::remove_all(path, error);
  }
};
void pump(const std::function<bool()>& condition) {
  const auto deadline = GetTickCount64() + 10000;
  for (;;) {
    MSG message{};
    while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
      assert(message.message != WM_QUIT); TranslateMessage(&message); DispatchMessageW(&message);
    }
    if (condition()) return;
    assert(GetTickCount64() < deadline);
    MsgWaitForMultipleObjects(0, nullptr, FALSE, 20, QS_ALLINPUT);
  }
}
}

int main() {
  TemporaryState temporary;
  PreferencesStore store(temporary.path.u8string());
  bool allow_clear = false, copy_ok = false, turn_off_during_read = false;
  unsigned copy_attempts = 0;
  std::wstring copied;
  std::vector<Json> calls;
  const DWORD ui_thread = GetCurrentThreadId();
  const auto capture = [&](const std::string& text) {
    return theme_data::call(msime_client_capture_clipboard_history, {{"directory", temporary.path.u8string()}, {"text", text}});
  };
  const auto listed = [&] {
    return theme_data::value(theme_data::call(msime_client_desktop_clipboard_history,
        {{"directory", temporary.path.u8string()}, {"action", {{"operation", "list"}}}})).value();
  };
  SettingsWindow::ClipboardServices clipboard;
  clipboard.transport = [&](const std::string& body) {
    assert(GetCurrentThreadId() != ui_thread);
    auto request = Json::parse(body); assert(request.at("directory") == temporary.path.u8string());
    auto action = request.at("action"); calls.push_back(action);
    const auto operation = action.at("operation").get<std::string>();
    if (operation == "sync" || operation == "capture") {
      // 只捕获合成文本；不读取或改写测试机的系统剪贴板。
      assert(store.load().snapshot->preferences().at("clipboard_history") == true);
      const auto captured = capture(operation == "sync" ? "合成初始剪贴板" : action.at("text").get<std::string>());
      assert(captured.at("ok") == true);
      request["action"] = Json{{"operation", "list"}};
    }
    const auto response = theme_data::call(msime_client_desktop_clipboard_history, request);
    if (turn_off_during_read) {
      turn_off_during_read = false;
      const auto current = store.load();
      assert(store.save(*current.snapshot, {{{"clipboard_history"}, false}}).outcome == Outcome::saved);
    }
    return response.dump();
  };
  clipboard.copy = [&](HWND owner, std::wstring_view text) {
    assert(GetCurrentThreadId() == ui_thread && IsWindow(owner));
    ++copy_attempts;
    if (copy_ok) copied = text;
    return copy_ok;
  };
  SettingsWindow window({}, [&](HWND, const wchar_t* title, const wchar_t* text) {
    assert(std::wstring_view(title) == L"剪贴板" && text && *text); return allow_clear;
  }, {}, {}, {}, {}, clipboard);
  assert(window.create(GetModuleHandleW(nullptr)));
  const auto host = window.handle(), body = GetDlgItem(host, page_body), idle = GetDlgItem(body, language_switch);
  const auto wait = [&] { pump([&] { return IsWindowEnabled(idle) != FALSE; }); };
  wait();
  const auto selector = GetDlgItem(host, page_selector);
  SendMessageW(selector, CB_SETCURSEL, 11, 0);
  SendMessageW(host, WM_COMMAND, MAKEWPARAM(page_selector, CBN_SELCHANGE), reinterpret_cast<LPARAM>(selector)); wait();
  assert(calls.size() == 1 && calls.back().at("operation") == "list");
  assert(store.load().snapshot->preferences().at("clipboard_history") == false);
  const auto click = [&](int id) { SendMessageW(GetDlgItem(body, id), BM_CLICK, 0, 0); wait(); };
  click(clipboard_enabled);
  assert(calls.back().at("operation") == "sync" && listed().at("entries").size() == 1);
  const auto limit = GetDlgItem(body, clipboard_limit);
  SendMessageW(limit, CB_SETCURSEL, 2, 0);
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(clipboard_limit, CBN_SELCHANGE), reinterpret_cast<LPARAM>(limit)); wait();
  assert(store.load().snapshot->preferences().at("clipboard_history_limit") == 200);
  const auto revision = store.load().snapshot->revision();
  auto count = calls.size();
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(clipboard_limit, CBN_SELCHANGE), reinterpret_cast<LPARAM>(limit));
  assert(calls.size() == count && store.load().snapshot->revision() == revision);
  const std::string unicode_record = "合成🙂\r\nline \t";
  for (int index = 0; index < 25; ++index) assert(capture(index == 4 ? unicode_record : "合成记录 " + std::to_string(index)).at("ok") == true);
  click(clipboard_refresh);
  const auto list = GetDlgItem(body, clipboard_list);
  assert(SendMessageW(list, LB_GETCOUNT, 0, 0) == 20 && IsWindowEnabled(GetDlgItem(body, clipboard_next)));
  count = calls.size(); click(clipboard_next);
  assert(calls.size() == count && SendMessageW(list, LB_GETCOUNT, 0, 0) == 6);
  const auto select = [&] {
    SendMessageW(list, LB_SETCURSEL, 0, 0);
    SendMessageW(body, WM_COMMAND, MAKEWPARAM(clipboard_list, LBN_SELCHANGE), reinterpret_cast<LPARAM>(list));
  };
  select();
  const auto selected = control_text::read(GetDlgItem(body, clipboard_detail)).value();
  assert(selected == unicode_record);
  assert(control_text::read(GetDlgItem(body, clipboard_timestamp))->find("复制时间") != std::string::npos);
  click(clipboard_pin);
  assert(listed().at("entries").at(0).at("text") == selected && listed().at("entries").at(0).at("pinned") == true);
  assert(control_text::read(GetDlgItem(body, clipboard_detail)).value() == selected);
  count = calls.size(); click(clipboard_copy);
  assert(copy_attempts == 1 && calls.size() == count);
  copy_ok = true; click(clipboard_copy);
  assert(copy_attempts == 2 && copied == control_text::wide(selected) && calls.back().at("operation") == "capture");
  click(clipboard_remove);
  assert(listed().at("entries").size() == 25);
  count = calls.size(); click(clipboard_clear);
  assert(calls.size() == count && listed().at("entries").size() == 25);
  click(clipboard_enabled);
  assert(calls.size() == count && SendMessageW(GetDlgItem(body, clipboard_enabled), BM_GETCHECK, 0, 0) == BST_CHECKED);
  // 外部保存造成 CAS 冲突时，不能清空仍启用的历史。
  const auto current = store.load();
  assert(store.save(*current.snapshot, {{{"keybindings", "toggle_character_set_ctrl_shift_f"},
      !current.snapshot->preferences().at("keybindings").at("toggle_character_set_ctrl_shift_f").get<bool>()}}).outcome == Outcome::saved);
  allow_clear = true; click(clipboard_enabled);
  assert(calls.size() == count && store.load().snapshot->preferences().at("clipboard_history") == true && listed().at("entries").size() == 25);
  click(clipboard_enabled);
  assert(store.load().snapshot->preferences().at("clipboard_history") == false && !std::filesystem::exists(temporary.path / L"clipboard_history.json"));
  click(clipboard_enabled);
  const auto history_file = temporary.path / L"clipboard_history.json";
  std::string document;
  { std::ifstream input(history_file, std::ios::binary); document.assign(std::istreambuf_iterator<char>(input), {}); }
  { std::ofstream output(history_file, std::ios::binary | std::ios::trunc); output << "{synthetic-broken"; assert(output.good()); }
  click(clipboard_refresh);
  assert(!IsWindowEnabled(GetDlgItem(body, clipboard_clear)));
  assert(control_text::read(GetDlgItem(body, clipboard_notice))->find("不会自动重置") != std::string::npos);
  { std::ifstream input(history_file, std::ios::binary); assert(std::string(std::istreambuf_iterator<char>(input), {}) == "{synthetic-broken"); }
  { std::ofstream output(history_file, std::ios::binary | std::ios::trunc); output << document; assert(output.good()); }
  turn_off_during_read = true; click(clipboard_refresh);
  assert(store.load().snapshot->preferences().at("clipboard_history") == false);
  assert(SendMessageW(list, LB_GETCOUNT, 0, 0) == 0 && !IsWindowEnabled(GetDlgItem(body, clipboard_copy)));
  DestroyWindow(host);
  std::puts("Windows 7 clipboard: real shared store, paging, pinning, capture boundary, conflict and corruption passed");
}
