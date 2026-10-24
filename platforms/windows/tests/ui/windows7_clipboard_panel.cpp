#include "../../panels/win7/ClipboardWindow.h"
#include "../../settings/win7/PreferencesStore.h"
#include "../../settings/win7/ControlText.h"
#include "../../settings/win7/ThemeData.h"
#include <cassert>
#include <cstdio>
#include <fstream>
#include <thread>

namespace {
using namespace msime::settings::win7;
using namespace msime::windows::win7;
struct TemporaryState {
  std::filesystem::path path;
  TemporaryState() { wchar_t root[MAX_PATH]{}, file[MAX_PATH]{}; assert(GetTempPathW(MAX_PATH, root) && GetTempFileNameW(root, L"m7c", 0, file)); assert(DeleteFileW(file) && CreateDirectoryW(file, nullptr)); path = file; }
  ~TemporaryState() { std::error_code error; std::filesystem::remove_all(path, error); }
};
struct Environment {
  const wchar_t* name;
  std::wstring previous;
  bool present = false;
  Environment(const wchar_t* key, const std::filesystem::path& value) : name(key) {
    const auto size = GetEnvironmentVariableW(name, nullptr, 0); present = size != 0;
    if (present) { previous.resize(size); previous.resize(GetEnvironmentVariableW(name, previous.data(), size)); }
    assert(SetEnvironmentVariableW(name, value.c_str()));
  }
  ~Environment() { SetEnvironmentVariableW(name, present ? previous.c_str() : nullptr); }
};
void pump(const std::function<bool()>& condition) {
  const auto deadline = GetTickCount64() + 10000;
  for (;;) {
    MSG message{}; while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) { assert(message.message != WM_QUIT); TranslateMessage(&message); DispatchMessageW(&message); }
    if (condition()) return; assert(GetTickCount64() < deadline); MsgWaitForMultipleObjects(0, nullptr, FALSE, 20, QS_ALLINPUT);
  }
}
}
int main() {
  TemporaryState temporary;
  Environment root(L"MSIME_CLIENT_STATE_DIR", temporary.path);
  Environment options(L"MSIME_CLIENT_HOST_OPTIONS", temporary.path / L"missing-options.json");
  PreferencesStore store(temporary.path.u8string()); auto initial = store.load(); assert(initial.snapshot);
  assert(store.save(*initial.snapshot, {{{"clipboard_history"}, true}}).outcome == Outcome::saved);
  const auto call = [&](auto api, Json request) { return theme_data::call(api, request); };
  const auto capture = [&](const std::string& text) { assert(theme_data::value(call(msime_client_capture_clipboard_history, {{"directory", temporary.path.u8string()}, {"text", text}}))); };
  for (int index = 0; index < 21; ++index) capture("synthetic-" + std::to_string(index));
  const std::wstring chosen = L"  合成🙂\r\nsecond  "; capture(control_text::utf8(chosen).value());
  assert(theme_data::value(call(msime_client_typing_statistics, {{"directory", temporary.path.u8string()}, {"action", {{"operation", "set_enabled"}, {"enabled", true}}}})));
  unsigned inserts = 0, copies = 0;
  bool insert_ok = false, copy_ok = false;
  std::wstring received;
  const auto ui_thread = std::this_thread::get_id();
  ClipboardWindow window([&](std::wstring_view text) { ++inserts; received = text; return insert_ok; }, [&](HWND, std::wstring_view text) { ++copies; received = text; return copy_ok; }, [&](const std::string& bytes) {
    assert(std::this_thread::get_id() != ui_thread);
    auto request = Json::parse(bytes);
    // 系统采集只在该边界替换；不读写真实用户剪贴板。
    if (request.at("action").at("operation") == "sync") request["action"] = {{"operation", "list"}};
    return theme_data::call(msime_client_desktop_clipboard_history, request).dump();
  });
  assert(window.create(GetModuleHandleW(nullptr))); const auto host = window.handle();
  const auto control = [&](int id) { const auto child = GetDlgItem(host, id); assert(child); return child; };
  const auto wait = [&] { pump([&] { return IsWindowEnabled(control(clipboard_panel_refresh)) != FALSE; }); }; wait();
  const auto click = [&](int id) { SendMessageW(control(id), BM_CLICK, 0, 0); wait(); };
  const auto select = [&](int index) { SendMessageW(control(clipboard_panel_entries), LB_SETCURSEL, index, 0); SendMessageW(host, WM_COMMAND, MAKEWPARAM(clipboard_panel_entries, LBN_SELCHANGE), reinterpret_cast<LPARAM>(control(clipboard_panel_entries))); };
  assert(GetWindowLongPtrW(host, GWL_EXSTYLE) & WS_EX_NOACTIVATE);
  assert(SendMessageW(host, WM_MOUSEACTIVATE, 0, 0) == MA_NOACTIVATE);
  assert(SendMessageW(control(clipboard_panel_entries), LB_GETCOUNT, 0, 0) == 20);
  select(0); click(clipboard_panel_insert); assert(inserts == 1 && received == chosen);
  assert(!control_text::read(control(clipboard_panel_status)).value().empty());
  const auto total = [&] { const auto response = theme_data::value(call(msime_client_typing_statistics, {{"directory", temporary.path.u8string()}, {"action", {{"operation", "load"}}}})); assert(response); return response->value("total", 0); };
  assert(total() == 0);
  insert_ok = true; click(clipboard_panel_insert); assert(inserts == 2 && total() > 0);
  const auto counted = total(); click(clipboard_panel_copy); assert(copies == 1 && total() == counted);
  copy_ok = true; click(clipboard_panel_copy); assert(copies == 2 && received == chosen && total() == counted);
  click(clipboard_panel_pin);
  auto listed = theme_data::value(call(msime_client_desktop_clipboard_history, {{"directory", temporary.path.u8string()}, {"action", {{"operation", "list"}}}}));
  assert(listed && listed->at("entries").at(0).at("pinned") == true);
  click(clipboard_panel_next); assert(SendMessageW(control(clipboard_panel_entries), LB_GETCOUNT, 0, 0) == 2);
  select(1); click(clipboard_panel_remove); assert(SendMessageW(control(clipboard_panel_entries), LB_GETCOUNT, 0, 0) == 1);
  click(clipboard_panel_previous); select(0);
  // 窗口里的旧条目不能绕过外部关闭历史后的实时核对。
  const auto before_close = store.load(); assert(store.save(*before_close.snapshot, {{{"clipboard_history"}, false}}).outcome == Outcome::saved);
  click(clipboard_panel_insert); assert(inserts == 2 && !IsWindowEnabled(control(clipboard_panel_insert)));
  assert(SendMessageW(control(clipboard_panel_entries), LB_GETCOUNT, 0, 0) == 0);
  const auto closed = store.load(); assert(store.save(*closed.snapshot, {{{"clipboard_history"}, true}}).outcome == Outcome::saved);
  const auto file = temporary.path / L"clipboard_history.json";
  { std::ofstream output(file, std::ios::binary); output << "{invalid-synthetic"; assert(output.good()); }
  click(clipboard_panel_refresh); assert(!IsWindowEnabled(control(clipboard_panel_insert)));
  { std::ifstream input(file); std::string bytes(std::istreambuf_iterator<char>(input), {}); assert(bytes == "{invalid-synthetic"); }
  DestroyWindow(host);
  std::puts("Windows 7 clipboard panel: real history, HWND paging/pin/remove, literal transfers, statistics, live enable check and damaged-file preservation passed");
}
