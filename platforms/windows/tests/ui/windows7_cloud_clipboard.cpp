#include "../../settings/win7/SettingsWindow.h"
#include "../../settings/win7/PreferencesStore.h"
#include "../../settings/win7/ThemeData.h"
#include "../../settings/win7/ControlText.h"
#include "../../settings/win7/CloudClipboardWorker.h"
#include <cassert>
#include <cstdio>
#include <filesystem>
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
    assert(GetTempFileNameW(root, L"m7e", 0, unique));
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
Json success(const Json& value) { return Json{{"ok", true}, {"value", value}}; }
}

int main() {
  TemporaryState temporary;
  PreferencesStore store(temporary.path.u8string());
  unsigned migrations = 0, account_calls = 0, relay_calls = 0, copies = 0;
  bool allow = false, remote_enabled = true, pinned = false, approved = false;
  bool relay_fail = false, corrupt = false, change_during_request = false;
  std::string state = "ready", last_operation, uploaded;
  std::wstring confirmation, copied;
  const std::string own_id(32, 'a'), other_id(32, 'b'), item_id(64, 'c');
  const std::string record = "合成🙂\r\nline \t";
  int retention = 7;
  bool has_item = true;
  const DWORD ui_thread = GetCurrentThreadId();
  const auto save_relay = [&](const std::string& token) {
    auto current = store.load(); assert(current.snapshot);
    assert(store.save(*current.snapshot, {{{"cloud_clipboard_relay"}, {{"url", "https://clipboard.example.invalid"}, {"token", token}}}}).outcome == Outcome::saved);
  };
  SettingsWindow::CloudClipboardServices services;
  const auto backend = [&](const std::string& body, bool relay) {
    assert(GetCurrentThreadId() != ui_thread);
    const auto request = Json::parse(body);
    assert(request.at("preferences_directory") == temporary.path.u8string());
    assert(std::filesystem::u8path(request.at("directory").get<std::string>()).is_absolute());
    assert(!request.contains("legacy_directory") && !request.contains("client_agent"));
    if (relay) { ++relay_calls; assert(request.at("expected_revision") == store.load().snapshot->revision()); }
    else { ++account_calls; assert(!request.contains("expected_revision")); }
    const auto& action = request.at("action");
    const auto operation = action.at("operation").get<std::string>(); last_operation = operation;
    if (change_during_request) { change_during_request = false; save_relay("synthetic-token-b"); }
    if (relay && relay_fail) return Json{{"ok", false}, {"error", "relay_unauthorized"}}.dump();
    if (operation == "list") {
      Json items = Json::array();
      if (has_item && state == "ready" && remote_enabled) items.push_back({{"id", item_id}, {"text", record}, {"updated_at", "2026-10-24T00:00:00Z"}, {"pinned", pinned}, {"device", "合成设备"}});
      if (corrupt) return success({{"enabled", true}, {"items", "invalid"}}).dump();
      return success({{"enabled", remote_enabled}, {"retention_days", retention}, {"items", items}, {"e2e", {{"status", remote_enabled ? state : "disabled"}, {"code", "0123 4567 8901 2345"}, {"pending_devices", 1u}, {"undecryptable", 2u}}}}).dump();
    }
    if (operation == "devices") return success({{"enabled", remote_enabled}, {"status", remote_enabled ? state : "disabled"}, {"device_id", own_id}, {"code", "0123 4567 8901 2345"},
        {"devices", Json::array({{{"device_id", own_id}, {"name", "合成本机"}, {"created_at", "2026-10-24T00:00:00Z"}, {"approved", state == "ready"}, {"current", true}, {"code", "0123 4567 8901 2345"}},
            {{"device_id", other_id}, {"name", "合成待批准设备"}, {"created_at", "2026-10-24T00:00:01Z"}, {"approved", approved}, {"current", false}, {"code", "9876 5432 1098 7654"}}})}}).dump();
    if (operation == "add") { uploaded = action.at("text").get<std::string>(); has_item = true; }
    else if (operation == "pin") { assert(action.at("id") == item_id); pinned = action.at("pinned").get<bool>(); }
    else if (operation == "delete" || operation == "clear") has_item = false;
    else if (operation == "set_retention") retention = action.at("days").get<int>();
    else if (operation == "set_enabled") { remote_enabled = action.at("enabled").get<bool>(); if (!remote_enabled) has_item = false; }
    else if (operation == "approve") { assert(action.at("device_id") == other_id); approved = true; }
    else if (operation == "reject" || operation == "remove") { assert(action.at("device_id") == other_id); approved = false; }
    else if (operation == "reset") { has_item = false; state = "ready"; }
    else assert(false);
    return success(Json::object()).dump();
  };
  services.account = [&](const std::string& body) { return backend(body, false); };
  services.relay = [&](const std::string& body) { return backend(body, true); };
  SettingsWindow::ClipboardServices clipboard;
  clipboard.copy = [&](HWND owner, std::wstring_view text) { assert(IsWindow(owner) && GetCurrentThreadId() == ui_thread); ++copies; copied = text; return true; };
  SettingsWindow window({}, [&](HWND, const wchar_t* title, const wchar_t* text) {
    assert(std::wstring_view(title) == L"云剪贴板"); confirmation = text; return allow;
  }, {}, {}, {}, [&](const std::string& body) {
    assert(GetCurrentThreadId() != ui_thread);
    const auto request = Json::parse(body); assert(request.at("action").at("operation") == "status");
    ++migrations; return success({{"logged_in", true}}).dump();
  }, clipboard, services);
  assert(window.create(GetModuleHandleW(nullptr)));
  const auto host = window.handle(), body = GetDlgItem(host, page_body), idle = GetDlgItem(body, language_switch);
  const auto wait = [&] { pump([&] { return IsWindowEnabled(idle) != FALSE; }); };
  wait();
  const auto selector = GetDlgItem(host, page_selector);
  SendMessageW(selector, CB_SETCURSEL, 12, 0);
  SendMessageW(host, WM_COMMAND, MAKEWPARAM(page_selector, CBN_SELCHANGE), reinterpret_cast<LPARAM>(selector)); wait();
  assert(account_calls == 2 && relay_calls == 0 && migrations == 1);
  assert(GetWindowLongPtrW(GetDlgItem(body, cloud_clipboard_relay_token), GWL_STYLE) & ES_PASSWORD);
  const auto click = [&](int id) { SendMessageW(GetDlgItem(body, id), BM_CLICK, 0, 0); wait(); };
  const auto select = [&](int id, int index) {
    const auto list = GetDlgItem(body, id); SendMessageW(list, LB_SETCURSEL, index, 0);
    SendMessageW(body, WM_COMMAND, MAKEWPARAM(id, LBN_SELCHANGE), reinterpret_cast<LPARAM>(list));
  };
  const auto text = [&](int id) { return control_text::read(GetDlgItem(body, id)).value(); };
  const auto set = [&](int id, const wchar_t* value) { assert(SetWindowTextW(GetDlgItem(body, id), value)); };
  select(cloud_clipboard_items, 0);
  assert(text(cloud_clipboard_detail) == record);
  assert(text(cloud_clipboard_metadata).find("合成设备") != std::string::npos);
  click(cloud_clipboard_copy); assert(copies == 1 && copied == control_text::wide(record));
  click(cloud_clipboard_pin); assert(pinned);
  select(cloud_clipboard_items, 0);
  auto count = account_calls;
  click(cloud_clipboard_delete); assert(has_item && account_calls == count);
  allow = true; click(cloud_clipboard_delete); assert(!has_item);
  set(cloud_clipboard_text, L"合成上传🙂\r\n保留空白 \t");
  click(cloud_clipboard_add); assert(uploaded == "合成上传🙂\r\n保留空白 \t" && text(cloud_clipboard_text).empty());
  const auto retention_control = GetDlgItem(body, cloud_clipboard_retention);
  SendMessageW(retention_control, CB_SETCURSEL, 3, 0);
  SendMessageW(body, WM_COMMAND, MAKEWPARAM(cloud_clipboard_retention, CBN_SELCHANGE), reinterpret_cast<LPARAM>(retention_control)); wait(); assert(retention == 30);
  select(cloud_clipboard_devices, 0); assert(!IsWindowEnabled(GetDlgItem(body, cloud_clipboard_remove)));
  select(cloud_clipboard_devices, 1);
  assert(text(cloud_clipboard_device_detail).find("9876 5432 1098 7654") != std::string::npos);
  allow = false; count = account_calls; click(cloud_clipboard_approve); assert(account_calls == count && !approved);
  assert(confirmation.find(L"9876 5432 1098 7654") != std::wstring::npos);
  allow = true; click(cloud_clipboard_approve); assert(approved);
  select(cloud_clipboard_devices, 1); assert(IsWindowEnabled(GetDlgItem(body, cloud_clipboard_remove)));
  click(cloud_clipboard_remove); assert(!approved && confirmation.find(L"删除全部") != std::wstring::npos);
  select(cloud_clipboard_devices, 1); click(cloud_clipboard_reject);
  click(cloud_clipboard_clear); assert(!has_item);
  allow = false; click(cloud_clipboard_enabled); assert(remote_enabled && SendMessageW(GetDlgItem(body, cloud_clipboard_enabled), BM_GETCHECK, 0, 0) == BST_CHECKED);
  allow = true; click(cloud_clipboard_enabled); assert(!remote_enabled && !IsWindowEnabled(GetDlgItem(body, cloud_clipboard_add)));
  click(cloud_clipboard_enabled); assert(remote_enabled);
  state = "pending"; click(cloud_clipboard_refresh);
  assert(text(cloud_clipboard_status).find("0123 4567 8901 2345") != std::string::npos);
  assert(!IsWindowEnabled(GetDlgItem(body, cloud_clipboard_add)) && IsWindowEnabled(GetDlgItem(body, cloud_clipboard_reset)));
  click(cloud_clipboard_reset); assert(state == "ready" && confirmation.find(L"其他设备需要重新批准") != std::wstring::npos);
  // 使用真实 Rust 偏好校验保存中继；保存配置本身不触发任何云端请求。
  const auto revision = store.load().snapshot->revision();
  set(cloud_clipboard_relay_url, L"https://user:synthetic@clipboard.example.invalid"); set(cloud_clipboard_relay_token, L"synthetic-token-a");
  click(cloud_clipboard_relay_save);
  assert(store.load().snapshot->revision() == revision && text(cloud_clipboard_relay_token) == "synthetic-token-a");
  count = account_calls;
  set(cloud_clipboard_relay_url, L"https://clipboard.example.invalid"); set(cloud_clipboard_relay_token, L"synthetic-token-a");
  click(cloud_clipboard_relay_save); assert(account_calls == count && relay_calls == 0);
  assert(store.load().snapshot->preferences().at("cloud_clipboard_relay").at("token") == "synthetic-token-a");
  assert(text(cloud_clipboard_relay_token).empty());
  set(cloud_clipboard_relay_url, L"https://clipboard.example.invalid/path");
  click(cloud_clipboard_relay_save);
  assert(store.load().snapshot->preferences().at("cloud_clipboard_relay").at("token") == "synthetic-token-a" && text(cloud_clipboard_relay_token).empty());
  const auto before_migrations = migrations;
  relay_fail = true; click(cloud_clipboard_refresh);
  assert(relay_calls == 1 && account_calls == count && migrations == before_migrations);
  assert(text(cloud_clipboard_notice).find("未改走") != std::string::npos && !IsWindowEnabled(GetDlgItem(body, cloud_clipboard_add)));
  relay_fail = false; click(cloud_clipboard_refresh);
  assert(relay_calls == 3 && account_calls == count && migrations == before_migrations);
  // 窗口仍持有旧来源修订时修改配置，旧操作必须在任何网络请求之前拒绝。
  auto relay_count = relay_calls; save_relay("synthetic-token-b");
  set(cloud_clipboard_text, L"合成不可发送"); click(cloud_clipboard_add);
  assert(relay_calls == relay_count && account_calls == count);
  assert(text(cloud_clipboard_text) == "合成不可发送" && text(cloud_clipboard_notice).find("配置已更改") != std::string::npos);
  click(cloud_clipboard_refresh);
  save_relay("synthetic-token-a"); click(cloud_clipboard_refresh);
  change_during_request = true; relay_count = relay_calls; click(cloud_clipboard_refresh);
  assert(relay_calls == relay_count + 1 && SendMessageW(GetDlgItem(body, cloud_clipboard_items), LB_GETCOUNT, 0, 0) == 0);
  assert(!IsWindowEnabled(GetDlgItem(body, cloud_clipboard_add)));
  corrupt = true; click(cloud_clipboard_refresh); assert(!IsWindowEnabled(GetDlgItem(body, cloud_clipboard_enabled)));
  corrupt = false; click(cloud_clipboard_refresh);
  allow = false; relay_count = relay_calls; click(cloud_clipboard_relay_clear);
  assert(store.load().snapshot->preferences().contains("cloud_clipboard_relay") && relay_calls == relay_count);
  allow = true; click(cloud_clipboard_relay_clear);
  assert(!store.load().snapshot->preferences().contains("cloud_clipboard_relay") && relay_calls == relay_count && account_calls == count);
  click(cloud_clipboard_refresh); assert(account_calls == count + 2 && migrations == before_migrations + 1);
  DestroyWindow(host);
  std::puts("Windows 7 cloud clipboard: HWND actions, relay persistence, source binding, no fallback and device approval passed");
}
