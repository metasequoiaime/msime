#include "../../panels/win7/CloudClipboardWindow.h"
#include "../../settings/CloudClipboardRoute.h"
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
  // WebDAV「测试连接」：real_probe 时转给真实 C ABI，否则答 probe_answer；记下每次收到的草稿。
  unsigned probes = 0; bool real_probe = false; Json probe_answer, probe_request;
  services.webdav_probe = [&](const std::string& bytes) {
    assert(GetCurrentThreadId() != ui_thread); ++probes; probe_request = Json::parse(bytes);
    return real_probe ? theme_data::call(msime_client_cloud_clipboard_webdav_probe, probe_request).dump() : probe_answer.dump();
  };
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
  // WebDAV 网盘：应用密码遮罩；草稿不写盘；测试连接不通过就不保存；通过后写网盘并删掉中继；存中继时删掉网盘；清除网盘回到水杉账号。
  const auto read = [&](int target) { return control_text::read(control(target), 8192).value_or(""); };
  const auto message = [](std::wstring_view text) { return control_text::utf8(text).value_or(""); };
  const auto preferences = [&] { return store.load().snapshot->preferences(); };
  const auto set = [&](int target, const wchar_t* value) { assert(SetWindowTextW(control(target), value)); };
  assert(GetWindowLongPtrW(control(cloud_clipboard_webdav_password), GWL_STYLE) & ES_PASSWORD);
  state = "ready";
  set(cloud_clipboard_webdav_url, L"http://127.0.0.1:9/dav"); set(cloud_clipboard_webdav_username, L"synthetic@example.test");
  set(cloud_clipboard_webdav_password, L"synthetic-app-password");
  auto revision = store.load().snapshot->revision();
  assert(probes == 0 && preferences().contains("cloud_clipboard_relay") && !preferences().contains("cloud_clipboard_webdav"));
  // 真实 C ABI：本机 9 号端口拒绝连接，答 relay_unavailable，保存被拦下，什么都不写。
  real_probe = true; click(cloud_clipboard_webdav_save);
  assert(probes == 1 && store.load().snapshot->revision() == revision && !preferences().contains("cloud_clipboard_webdav"));
  assert(read(cloud_clipboard_notice) == message(msime::settings::webdav_error_message(L"relay_unavailable")));
  assert(probe_request.at("url") == "http://127.0.0.1:9/dav" && probe_request.at("username") == "synthetic@example.test" && probe_request.at("password") == "synthetic-app-password");
  assert(!probe_request.contains("directory") || std::filesystem::path(probe_request.at("directory").get<std::string>()).filename() == "account");
  real_probe = false; probe_answer = Json{{"ok", false}, {"error", "webdav_unsupported"}};
  click(cloud_clipboard_webdav_save);
  assert(probes == 2 && store.load().snapshot->revision() == revision && read(cloud_clipboard_notice) == message(msime::settings::webdav_error_message(L"webdav_unsupported")));
  assert(read(cloud_clipboard_webdav_url) == "http://127.0.0.1:9/dav" && read(cloud_clipboard_webdav_password) == "synthetic-app-password");
  // 只测试不保存，通过了也不写盘。
  probe_answer = Json{{"ok", true}, {"value", Json::object()}};
  click(cloud_clipboard_webdav_test);
  assert(probes == 3 && store.load().snapshot->revision() == revision && read(cloud_clipboard_notice) == message(msime::settings::webdav_probe_passed));
  click(cloud_clipboard_webdav_save);
  assert(probes == 4 && !preferences().contains("cloud_clipboard_relay"));
  assert(preferences().at("cloud_clipboard_webdav") == (Json{{"url", "http://127.0.0.1:9/dav"}, {"username", "synthetic@example.test"}, {"password", "synthetic-app-password"}}));
  assert(read(cloud_clipboard_source) == message(L"当前来源：WebDAV 网盘（请求失败不会改走水杉账号）") && read(cloud_clipboard_webdav_password).empty());
  // 应用密码框留空：沿用已存的应用密码去测试和保存。
  set(cloud_clipboard_webdav_username, L"synthetic-b@example.test"); click(cloud_clipboard_webdav_save);
  assert(probes == 5 && probe_request.at("password") == "synthetic-app-password");
  assert(preferences().at("cloud_clipboard_webdav").at("username") == "synthetic-b@example.test" && preferences().at("cloud_clipboard_webdav").at("password") == "synthetic-app-password");
  // 改存中继：网盘配置一起删掉，不经过测试连接。
  set(cloud_clipboard_relay_url, L"https://clipboard.example.invalid"); set(cloud_clipboard_relay_token, L"synthetic-token-a");
  click(cloud_clipboard_relay_save);
  assert(probes == 5 && preferences().at("cloud_clipboard_relay").at("token") == "synthetic-token-a" && !preferences().contains("cloud_clipboard_webdav"));
  // 再存回网盘（网盘配置被清掉后输入框也跟着清空了），然后清除网盘配置：确认后中继和网盘都不留，来源回到水杉账号。
  assert(read(cloud_clipboard_webdav_url).empty() && read(cloud_clipboard_webdav_username).empty());
  set(cloud_clipboard_webdav_url, L"http://127.0.0.1:9/dav"); set(cloud_clipboard_webdav_username, L"synthetic@example.test");
  set(cloud_clipboard_webdav_password, L"synthetic-app-password"); click(cloud_clipboard_webdav_save);
  assert(probes == 6 && preferences().contains("cloud_clipboard_webdav") && !preferences().contains("cloud_clipboard_relay"));
  const auto webdav_confirms = confirms; allowed = true; click(cloud_clipboard_webdav_clear);
  assert(confirms == webdav_confirms + 1 && probes == 6 && !preferences().contains("cloud_clipboard_webdav") && !preferences().contains("cloud_clipboard_relay"));
  assert(read(cloud_clipboard_source) == message(L"当前来源：水杉账号"));
  DestroyWindow(host);
  std::puts("Windows 7 cloud clipboard panel: shared source checks, literal copy/insert, confirmations, relay failure isolation, changed-source refusal, invalid-state protection and WebDAV probe-gated saving passed");
}
