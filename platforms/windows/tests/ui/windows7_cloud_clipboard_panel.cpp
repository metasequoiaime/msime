#include "../../panels/win7/CloudClipboardWindow.h"
#include "../../settings/win7/PreferencesStore.h"
#include "../../settings/win7/ControlText.h"
#include "../../settings/win7/ThemeData.h"
#include <cassert>
#include <cstdio>

namespace {
using namespace msime::settings::win7;
using namespace msime::windows::win7;
struct TemporaryState {
  std::filesystem::path path;
  std::wstring previous;
  bool present = false;
  TemporaryState() {
    wchar_t root[MAX_PATH]{}, file[MAX_PATH]{}; assert(GetTempPathW(MAX_PATH, root) && GetTempFileNameW(root, L"m7e", 0, file)); assert(DeleteFileW(file) && CreateDirectoryW(file, nullptr)); path = file;
    const auto size = GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", nullptr, 0); present = size != 0;
    if (present) { previous.resize(size); previous.resize(GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", previous.data(), size)); }
    assert(SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", path.c_str()));
  }
  ~TemporaryState() { SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", present ? previous.c_str() : nullptr); std::error_code error; std::filesystem::remove_all(path, error); }
};
void pump(const std::function<bool()>& condition) {
  const auto deadline = GetTickCount64() + 10000;
  for (;;) { MSG message{}; while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) { assert(message.message != WM_QUIT); TranslateMessage(&message); DispatchMessageW(&message); }
    if (condition()) return; assert(GetTickCount64() < deadline); MsgWaitForMultipleObjects(0, nullptr, FALSE, 20, QS_ALLINPUT); }
}
}
int main() {
  TemporaryState temporary; PreferencesStore store(temporary.path.u8string()); assert(store.load().snapshot);
  const auto ui_thread = GetCurrentThreadId();
  const auto success = [](Json value) { return Json{{"ok", true}, {"value", std::move(value)}}.dump(); };
  unsigned accounts = 0, relays = 0, migrations = 0, inserts = 0, copies = 0, confirms = 0;
  bool failed = false, allowed = false, exists = true, pinned = false, during = false, malformed = false, insert_ok = false;
  std::string state = "ready", search;
  const std::string record = "  合成🙂\r\nsecond  ", id(64, 'a');
  const auto save_relay = [&](const char* token) { const auto snapshot = store.load(); assert(store.save(*snapshot.snapshot, {{{"cloud_clipboard_relay"}, {{"url", "https://clipboard.example.invalid"}, {"token", token}}}}).outcome == Outcome::saved); };
  const auto backend = [&](const std::string& bytes, bool relay) {
    assert(GetCurrentThreadId() != ui_thread); const auto request = Json::parse(bytes);
    assert(request.at("preferences_directory") == temporary.path.u8string());
    if (relay) ++relays; else ++accounts;
    const auto& action = request.at("action"); const auto operation = action.at("operation").get<std::string>();
    if (during) { during = false; save_relay("synthetic-token-change"); }
    if (failed && relay) return Json{{"ok", false}, {"error", "relay_unauthorized"}}.dump();
    if (operation == "list") {
      search = action.value("search", std::string{}); Json rows = Json::array();
      if (exists && state == "ready") rows.push_back({{"id", id}, {"text", record}, {"pinned", pinned}, {"device", "合成设备"}, {"updated_at", "2026-10-24T00:00:00Z"}});
      return success({{"enabled", true}, {"retention_days", 0}, {"items", rows}, {"e2e", {{"status", state}, {"code", "0123 4567 8901 2345"}, {"pending_devices", 0u}, {"undecryptable", 0u}}}});
    }
    if (operation == "devices") return success({{"enabled", true}, {"status", state}, {"code", malformed ? "invalid" : "0123 4567 8901 2345"}, {"device_id", std::string(32, 'b')}, {"devices", Json::array()}});
    if (operation == "pin") pinned = action.at("pinned").get<bool>();
    if (operation == "delete") exists = false;
    if (operation == "add") { assert(action.at("text") == "合成上传"); exists = true; }
    return success(nullptr);
  };
  CloudClipboardWindow::Services services;
  services.account = [&](const std::string& bytes) { return backend(bytes, false); };
  services.relay = [&](const std::string& bytes) { return backend(bytes, true); };
  std::wstring received;
  CloudClipboardWindow window([&](std::wstring_view text) { ++inserts; received = text; return insert_ok; }, [&](HWND, std::wstring_view text) { ++copies; received = text; return true; }, services,
      [&](const std::string& bytes) { assert(GetCurrentThreadId() != ui_thread); assert(Json::parse(bytes).at("action").at("operation") == "status"); ++migrations; return success(Json::object()); },
      [&](HWND, const wchar_t* title, const wchar_t* text) { assert(std::wstring_view(title) == L"云剪贴板" && text && *text); ++confirms; return allowed; });
  assert(window.create(GetModuleHandleW(nullptr))); const auto host = window.handle(), body = GetDlgItem(host, cloud_clipboard_panel_body);
  const auto control = [&](int id) { const auto value = GetDlgItem(id == cloud_clipboard_panel_insert || id == cloud_clipboard_panel_notice ? host : body, id); assert(value); return value; };
  const auto wait = [&] { pump([&] { return IsWindowEnabled(control(cloud_clipboard_refresh)) != FALSE; }); }; wait();
  const auto click = [&](int id) { SendMessageW(control(id), BM_CLICK, 0, 0); wait(); };
  const auto select = [&] { SendMessageW(control(cloud_clipboard_items), LB_SETCURSEL, 0, 0); SendMessageW(body, WM_COMMAND, MAKEWPARAM(cloud_clipboard_items, LBN_SELCHANGE), reinterpret_cast<LPARAM>(control(cloud_clipboard_items))); };
  assert(migrations == 1 && accounts == 2 && relays == 0);
  SendMessageW(body, WM_VSCROLL, SB_BOTTOM, 0);
  const auto bottom = GetScrollPos(body, SB_VERT); assert(bottom > 0);
  SendMessageW(body, WM_VSCROLL, SB_ENDSCROLL, 0); assert(GetScrollPos(body, SB_VERT) == bottom);
  SendMessageW(body, WM_VSCROLL, SB_TOP, 0); assert(GetScrollPos(body, SB_VERT) == 0);
  SendMessageW(body, WM_VSCROLL, SB_PAGEDOWN, 0); assert(GetScrollPos(body, SB_VERT) > 0);
  SendMessageW(body, WM_VSCROLL, SB_PAGEUP, 0); assert(GetScrollPos(body, SB_VERT) == 0);
  select(); click(cloud_clipboard_copy); assert(copies == 1 && received == control_text::wide(record));
  select(); click(cloud_clipboard_panel_insert); assert(inserts == 1 && received == control_text::wide(record));
  insert_ok = true; select(); click(cloud_clipboard_panel_insert); assert(inserts == 2);
  select(); click(cloud_clipboard_pin); assert(pinned);
  assert(SetWindowTextW(control(cloud_clipboard_search), L"合成")); click(cloud_clipboard_find); assert(search == "合成");
  select(); const auto calls = accounts; click(cloud_clipboard_delete); assert(confirms == 1 && accounts == calls && exists);
  allowed = true; click(cloud_clipboard_delete); assert(!exists);
  assert(SetWindowTextW(control(cloud_clipboard_text), L"合成上传")); click(cloud_clipboard_add); assert(exists);
  select(); save_relay("synthetic-token-a"); const auto previous_calls = accounts; click(cloud_clipboard_panel_insert);
  assert(inserts == 2 && accounts == previous_calls && relays == 0 && !IsWindowEnabled(control(cloud_clipboard_panel_insert)));
  click(cloud_clipboard_refresh); const auto migrated = migrations; assert(relays == 2);
  select(); failed = true; click(cloud_clipboard_copy); assert(copies == 1 && accounts == previous_calls && migrations == migrated);
  failed = false; click(cloud_clipboard_refresh); select(); during = true; click(cloud_clipboard_panel_insert); assert(inserts == 2 && migrations == migrated);
  click(cloud_clipboard_refresh); malformed = true; click(cloud_clipboard_refresh); select(); assert(!IsWindowEnabled(control(cloud_clipboard_panel_insert)));
  malformed = false; state = "pending"; click(cloud_clipboard_refresh); assert(!IsWindowEnabled(control(cloud_clipboard_add)) && !IsWindowEnabled(control(cloud_clipboard_panel_insert)));
  DestroyWindow(host);
  std::puts("Windows 7 cloud clipboard panel: shared source checks, literal copy/insert, confirmations, relay failure isolation, changed-source refusal and invalid-state protection passed");
}
