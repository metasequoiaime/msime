#include "../../settings/win7/SettingsWindow.h"
#include "../../settings/win7/PreferencesStore.h"
#include "../../settings/win7/ThemeData.h"
#include "../../settings/win7/ControlText.h"
#include "../../settings/win7/DictionaryData.h"
#include "../../settings/win7/DictionaryPersonalExport.h"
#include <cassert>
#include <atomic>
#include <mutex>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <functional>
#include <vector>

namespace {
using namespace msime::settings::win7;
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
struct TemporaryState {
  std::filesystem::path path;
  TemporaryState() {
    wchar_t root[MAX_PATH]{}, unique[MAX_PATH]{};
    assert(GetTempPathW(MAX_PATH, root) && GetTempFileNameW(root, L"m7d", 0, unique));
    assert(DeleteFileW(unique) && CreateDirectoryW(unique, nullptr)); path = unique;
  }
  ~TemporaryState() { std::error_code error; std::filesystem::remove_all(path, error); }
};
void pump(const std::function<bool()>& condition) {
  // 词库页首次创建会等待隔离的共享存储和后台线程；冷启动的 Windows CI 可能超过普通控件的 10 秒预算。
  const auto deadline = GetTickCount64() + 30000;
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

int main(int argc, char** argv) {
  assert(argc == 2);
  TemporaryState temporary;
  Environment state(L"MSIME_CLIENT_STATE_DIR", temporary.path);
  const auto options_path = temporary.path / L"runtime-options.json";
  Environment injected_options(L"MSIME_CLIENT_HOST_OPTIONS", options_path);
  for (const auto* name : {L"resources", L"dictionaries", L"user", L"cache"}) std::filesystem::create_directories(temporary.path / name);
  for (const auto* directory : {L"resources", L"dictionaries"})
    for (const auto* name : {L"msime-pinyin.db", L"msime-english.db"})
      std::filesystem::copy_file(std::filesystem::u8path(argv[1]) / name, temporary.path / directory / name);
  PreferencesStore store(temporary.path.u8string());
  auto snapshot = store.load(); assert(snapshot.snapshot);
  const auto options = Json{{"api_version", 1}, {"resources", (temporary.path / L"resources").u8string()},
      {"user_data", (temporary.path / L"user").u8string()}, {"cache", (temporary.path / L"cache").u8string()},
      {"dictionaries", (temporary.path / L"dictionaries").u8string()}, {"preferences_directory", temporary.path.u8string()}, {"preferences", snapshot.snapshot->preferences()}};
  { std::ofstream output(options_path, std::ios::binary); output << options.dump(); assert(output.good()); }
  auto export_path = temporary.path / L"synthetic-export.txt";
  auto import_path = temporary.path / L"synthetic-import.txt";
  bool allow = false, cancel_picker = false, reject = false, corrupt_list = false;
  unsigned picks = 0;
  std::vector<Json> calls;
  std::mutex calls_mutex;
  std::optional<Json> synthetic_updates;
  bool bad_updates = false, scheduler_test = false;
  unsigned automatic_calls = 0;
  HWND scheduler_host = nullptr;
  std::atomic<bool> block_updates{false}, update_entered{false};
  HANDLE update_gate = CreateEventW(nullptr, TRUE, FALSE, nullptr); assert(update_gate);
  const DWORD ui_thread = GetCurrentThreadId();
  const auto backend = [&](const std::string& body) {
    assert(GetCurrentThreadId() != ui_thread);
    auto request = Json::parse(body);
    { std::lock_guard<std::mutex> lock(calls_mutex); calls.push_back(request.at("action")); }
    if (request.at("action").at("operation") == "updates") {
      if (request.at("action").at("command") == "automatic") {
        assert(scheduler_test && scheduler_host); ++automatic_calls;
        assert(PostMessageW(scheduler_host, WM_CLOSE, 0, 0));
      }
      if (block_updates) { update_entered = true; assert(WaitForSingleObject(update_gate, 10000) == WAIT_OBJECT_0); }
      if (bad_updates) return Json{{"ok", true}, {"value", {{"enabled", "invalid"}}}}.dump();
      if (synthetic_updates) {
        const auto command = request.at("action").at("command");
        if (command == "check") (*synthetic_updates)["phase"] = "checking";
        if (command == "rollback") { (*synthetic_updates)["enabled"] = false; (*synthetic_updates)["pending_restart"] = true; }
        return Json{{"ok", true}, {"value", *synthetic_updates}}.dump();
      }
    }
    assert(request.at("options") == options);
    if (reject) return dictionary_data::failure("dictionary_busy").dump();
    if (corrupt_list && (request.at("action").at("operation") == "list" || request.at("action").at("operation") == "browse"))
      return Json{{"ok", true}, {"value", {{"entries", "bad"}, {"has_more", true}}}}.dump();
    // 隔离真实词库读写；只替换 Server 维护通知边界，不联系已安装输入法。
    return theme_data::call(msime_client_dictionary, request).dump();
  };
  SettingsWindow window([&](HWND, bool save) -> std::optional<std::filesystem::path> {
    ++picks; return cancel_picker ? std::nullopt : std::make_optional(save ? export_path : import_path);
  }, [&](HWND, const wchar_t* title, const wchar_t* message) { assert(std::wstring_view(title) == L"词库" && message && *message); return allow; }, {}, {}, {}, {}, {}, {}, backend);
  assert(window.create(GetModuleHandleW(nullptr)));
  const auto host = window.handle(), body = GetDlgItem(host, page_body), idle = GetDlgItem(body, language_switch);
  const auto wait = [&] { pump([&] { return IsWindowEnabled(idle) != FALSE; }); };
  wait();
  const auto selector = GetDlgItem(host, page_selector);
  SendMessageW(selector, CB_SETCURSEL, 13, 0);
  SendMessageW(host, WM_COMMAND, MAKEWPARAM(page_selector, CBN_SELCHANGE), reinterpret_cast<LPARAM>(selector)); wait();
  const auto wait_updates = [&] { pump([&] { return IsWindowEnabled(GetDlgItem(body, dictionary_updates_refresh)) != FALSE; }); };
  wait_updates();
  const auto click = [&](int id) { SendMessageW(GetDlgItem(body, id), BM_CLICK, 0, 0); wait(); };
  const auto set = [&](int id, const wchar_t* value) { assert(SetWindowTextW(GetDlgItem(body, id), value)); };
  const auto text = [&](int id) { return control_text::read(GetDlgItem(body, id)).value(); };
  const auto choose = [&](int id, int index) {
    const auto combo = GetDlgItem(body, id); SendMessageW(combo, CB_SETCURSEL, index, 0);
    SendMessageW(body, WM_COMMAND, MAKEWPARAM(id, CBN_SELCHANGE), reinterpret_cast<LPARAM>(combo)); wait();
  };
  const auto select = [&](int index) {
    const auto list = GetDlgItem(body, dictionary_entries); SendMessageW(list, LB_SETCURSEL, index, 0);
    SendMessageW(body, WM_COMMAND, MAKEWPARAM(dictionary_entries, LBN_SELCHANGE), reinterpret_cast<LPARAM>(list));
  };
  choose(dictionary_kind, 4);
  set(dictionary_key, L"synthetic"); set(dictionary_value, L"合成短语🙂"); set(dictionary_weight, L"1234");
  click(dictionary_save);
  assert(text(dictionary_notice).find("操作已完成") != std::string::npos);
  assert(SendMessageW(GetDlgItem(body, dictionary_entries), LB_GETCOUNT, 0, 0) == 1);
  select(0); assert(text(dictionary_value) == "合成短语🙂" && text(dictionary_weight) == "1234");
  auto count = calls.size(); click(dictionary_save); assert(calls.size() == count);
  set(dictionary_weight, L"2345"); click(dictionary_save); select(0); assert(text(dictionary_weight) == "2345");
  count = calls.size(); click(dictionary_delete); assert(calls.size() == count);
  allow = true; click(dictionary_delete); assert(SendMessageW(GetDlgItem(body, dictionary_entries), LB_GETCOUNT, 0, 0) == 0);
  // 有歧义的写入失败重试保留编号；修改内容以后生成新的编号。
  click(dictionary_new); set(dictionary_key, L"retry"); set(dictionary_value, L"合成重试");
  reject = true; click(dictionary_save);
  const auto first_id = calls.back().at("request_id");
  assert(text(dictionary_value) == "合成重试");
  click(dictionary_save); assert(calls.back().at("request_id") == first_id);
  set(dictionary_value, L"合成修改"); click(dictionary_save); assert(calls.back().at("request_id") != first_id);
  reject = false; click(dictionary_save);
  select(0); assert(text(dictionary_value) == "合成修改");
  cancel_picker = true; count = calls.size(); click(dictionary_export); assert(calls.size() == count && picks == 1 && !std::filesystem::exists(export_path));
  cancel_picker = false; click(dictionary_export);
  const auto exported = msime::settings::read_settings_document(export_path).value();
  assert(exported.find("合成修改\tretry\t10000") != std::string::npos && exported.substr(0, 3) == "\xEF\xBB\xBF");
  { std::ofstream output(import_path, std::ios::binary); output << "合成导入\timported\t3456\n"; assert(output.good()); }
  click(dictionary_import);
  assert(text(dictionary_notice).find("导入完成") != std::string::npos && SendMessageW(GetDlgItem(body, dictionary_entries), LB_GETCOUNT, 0, 0) == 2);
  // UTF-16 文件经正式格式模型解码，emoji 代理对与换行不丢失。
  { std::ofstream output(import_path, std::ios::binary); const std::wstring record = L"合成🙂\tunicode\t4567\r\n"; output.put(static_cast<char>(0xff)); output.put(static_cast<char>(0xfe)); output.write(reinterpret_cast<const char*>(record.data()), static_cast<std::streamsize>(record.size() * sizeof(wchar_t))); assert(output.good()); }
  click(dictionary_import); assert(SendMessageW(GetDlgItem(body, dictionary_entries), LB_GETCOUNT, 0, 0) == 3);
  set(dictionary_query, L"unicode"); click(dictionary_search); assert(SendMessageW(GetDlgItem(body, dictionary_entries), LB_GETCOUNT, 0, 0) == 1);
  select(0); assert(text(dictionary_value) == "合成🙂");
  { std::ofstream output(import_path, std::ios::binary); for (int index = 0; index < 55; ++index)
      output << "合成分页" << index << '\t' << 'p' << static_cast<char>('a' + index / 26) << static_cast<char>('a' + index % 26) << "\t10000\n"; assert(output.good()); }
  set(dictionary_query, L""); click(dictionary_import);
  assert(SendMessageW(GetDlgItem(body, dictionary_entries), LB_GETCOUNT, 0, 0) == 50 && IsWindowEnabled(GetDlgItem(body, dictionary_next)));
  click(dictionary_next); assert(SendMessageW(GetDlgItem(body, dictionary_entries), LB_GETCOUNT, 0, 0) == 8);
  click(dictionary_previous); assert(SendMessageW(GetDlgItem(body, dictionary_entries), LB_GETCOUNT, 0, 0) == 50);
  choose(dictionary_kind, 5);
  assert(!IsWindowEnabled(GetDlgItem(body, dictionary_save)) && !IsWindowEnabled(GetDlgItem(body, dictionary_import)));
  assert(text(dictionary_page).find("尚未安装") != std::string::npos);
  choose(dictionary_kind, 4); set(dictionary_query, L""); click(dictionary_search);
  corrupt_list = true; click(dictionary_search);
  assert(!IsWindowEnabled(GetDlgItem(body, dictionary_entries)) && !IsWindowEnabled(GetDlgItem(body, dictionary_delete)));
  corrupt_list = false; click(dictionary_search);
  allow = false; count = calls.size(); click(dictionary_reset); assert(calls.size() == count);
  allow = true; click(dictionary_reset);
  assert(SendMessageW(GetDlgItem(body, dictionary_entries), LB_GETCOUNT, 0, 0) == 0);
  choose(dictionary_kind, 0); click(dictionary_user_only); set(dictionary_query, L"he"); click(dictionary_search);
  assert(SendMessageW(GetDlgItem(body, dictionary_entries), LB_GETCOUNT, 0, 0) == 1);
  select(0); assert(text(dictionary_value) == "合成");
  assert(GetWindowLongPtrW(GetDlgItem(body, dictionary_key), GWL_STYLE) & ES_READONLY);
  set(dictionary_weight, L"654"); click(dictionary_save); select(0); assert(text(dictionary_weight) == "654");
  click(dictionary_delete); assert(SendMessageW(GetDlgItem(body, dictionary_entries), LB_GETCOUNT, 0, 0) == 0);
  click(dictionary_reset); assert(SendMessageW(GetDlgItem(body, dictionary_entries), LB_GETCOUNT, 0, 0) == 1);
  select(0); assert(text(dictionary_weight) == "321");
  choose(dictionary_kind, 3); click(dictionary_new);
  set(dictionary_key, L"syntheticword"); set(dictionary_value, L"SyntheticWord"); click(dictionary_save);
  choose(dictionary_kind, 4); click(dictionary_new);
  set(dictionary_key, L"allwords"); set(dictionary_value, L"合成全部导出🙂"); click(dictionary_save);
  click(dictionary_export_all);
  const auto all_export = msime::settings::read_settings_document(export_path).value();
  assert(all_export.find("# 类别\t编码\t词条\t权重\n") == 0);
  assert(all_export.find("英文\tsyntheticword\tSyntheticWord\t10000") != std::string::npos);
  assert(all_export.find("快捷短语\tallwords\t合成全部导出🙂\t10000") != std::string::npos);
  assert(all_export.find("合成\t321") == std::string::npos);
  dictionary_data::Requests export_requests;
  const auto malformed = [&](const std::string&) { return Json{{"ok", true}, {"value", {{"entries", Json::array()}, {"has_more", true}}}}.dump(); };
  assert(dictionary_data::export_all_file(export_requests, options, export_path, malformed).at("ok") == false);
  assert(msime::settings::read_settings_document(export_path).value() == all_export);
  click(dictionary_reset);
  // 配置开关走真实隔离共享存储；检查与回滚只注入状态，测试不下载真实资源。
  const auto update_click = [&](int id) { SendMessageW(GetDlgItem(body, id), BM_CLICK, 0, 0); wait_updates(); };
  assert(SendMessageW(GetDlgItem(body, dictionary_updates_enabled), BM_GETCHECK, 0, 0) == BST_CHECKED);
  update_click(dictionary_updates_enabled);
  assert(SendMessageW(GetDlgItem(body, dictionary_updates_enabled), BM_GETCHECK, 0, 0) == BST_UNCHECKED);
  update_click(dictionary_updates_refresh);
  assert(SendMessageW(GetDlgItem(body, dictionary_updates_enabled), BM_GETCHECK, 0, 0) == BST_UNCHECKED);
  synthetic_updates = Json{{"enabled", false}, {"version", "synthetic-v1"}, {"phase", "current"}, {"checked_at", 0},
      {"downloaded_bytes", 0}, {"total_bytes", 0}, {"error", nullptr}, {"can_rollback", true}, {"pending_restart", false}};
  update_click(dictionary_updates_refresh);
  assert(text(dictionary_updates_version).find("synthetic-v1") != std::string::npos);
  update_click(dictionary_updates_check);
  assert(!IsWindowEnabled(GetDlgItem(body, dictionary_updates_check)) && text(dictionary_updates_notice).find("正在检查") != std::string::npos);
  (*synthetic_updates)["phase"] = "downloading"; (*synthetic_updates)["downloaded_bytes"] = 1048577; (*synthetic_updates)["total_bytes"] = 2097153;
  update_click(dictionary_updates_refresh); assert(text(dictionary_updates_notice).find("1 / 3 MiB") != std::string::npos);
  (*synthetic_updates)["phase"] = "current"; (*synthetic_updates)["pending_restart"] = true;
  update_click(dictionary_updates_refresh); assert(text(dictionary_updates_notice).find("下次启动") != std::string::npos);
  (*synthetic_updates)["phase"] = "failed"; (*synthetic_updates)["error"] = "synthetic_failure";
  update_click(dictionary_updates_refresh); assert(text(dictionary_updates_notice).find("继续使用原词库") != std::string::npos);
  (*synthetic_updates)["phase"] = "current"; (*synthetic_updates)["error"] = nullptr; (*synthetic_updates)["pending_restart"] = false;
  update_click(dictionary_updates_refresh);
  allow = false; count = calls.size(); update_click(dictionary_updates_rollback); assert(calls.size() == count);
  allow = true; update_click(dictionary_updates_rollback);
  assert(calls.back().at("command") == "rollback" && text(dictionary_updates_notice).find("下次启动") != std::string::npos);
  bad_updates = true; update_click(dictionary_updates_refresh);
  assert(!IsWindowEnabled(GetDlgItem(body, dictionary_updates_enabled)) && !IsWindowEnabled(GetDlgItem(body, dictionary_updates_check)));
  bad_updates = false; SendMessageW(host, WM_TIMER, 74, 0); wait_updates();
  assert(calls.back().at("command") == "status");
  // 更新查询阻塞时，词条草稿、焦点与真正保存都不能被进度轮询打断。
  choose(dictionary_kind, 4); click(dictionary_new);
  set(dictionary_key, L"parallel"); set(dictionary_value, L"合成并行草稿");
  block_updates = true;
  SendMessageW(GetDlgItem(body, dictionary_updates_refresh), BM_CLICK, 0, 0);
  pump([&] { return update_entered.load(); });
  assert(IsWindowEnabled(GetDlgItem(body, dictionary_save)) && text(dictionary_value) == "合成并行草稿");
  click(dictionary_save);
  assert(text(dictionary_notice).find("操作已完成") != std::string::npos);
  assert(SetEvent(update_gate)); wait_updates(); block_updates = false;
  set(dictionary_query, L"parallel"); click(dictionary_search);
  select(0); assert(text(dictionary_value) == "合成并行草稿");
  assert(CloseHandle(update_gate));
  // 退出前校验测试全过程没有改变偏好默认值。
  assert(store.load().snapshot->revision() == snapshot.snapshot->revision());
  scheduler_test = true; scheduler_host = host;
  assert(window.run(SW_HIDE) == 0 && automatic_calls == 1);
  std::puts("Windows 7 dictionary: actual shared store, HWND edits, imports, export, receipts and read-only resources passed");
}
