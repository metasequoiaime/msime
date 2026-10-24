#include "../../panels/win7/CloudDictionaryWindow.h"
#include "../../panels/win7/PanelContext.h"
#include "../../settings/win7/PreferencesStore.h"
#include <cassert>
#include <cstdio>
#include <fstream>
#include <mutex>

namespace {
using namespace msime::settings::win7;
using Form = CloudDictionaryForm;
void pump(const std::function<bool()>& condition) {
  const auto deadline = GetTickCount64() + 15000;
  for (;;) {
    MSG event{}; while (PeekMessageW(&event, nullptr, 0, 0, PM_REMOVE)) { assert(event.message != WM_QUIT); TranslateMessage(&event); DispatchMessageW(&event); }
    if (condition()) return; assert(GetTickCount64() < deadline); MsgWaitForMultipleObjects(0, nullptr, FALSE, 10, QS_ALLINPUT);
  }
}
}
int main() {
  wchar_t root[MAX_PATH]{}, file[MAX_PATH]{}; assert(GetTempPathW(MAX_PATH, root) && GetTempFileNameW(root, L"m7d", 0, file));
  assert(DeleteFileW(file) && CreateDirectoryW(file, nullptr)); const std::filesystem::path directory(file);
  const auto length = GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", nullptr, 0); std::wstring previous(length, L'\0');
  if (length) previous.resize(GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", previous.data(), length));
  assert(SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", directory.c_str()));
  const auto options_length = GetEnvironmentVariableW(L"MSIME_CLIENT_HOST_OPTIONS", nullptr, 0); std::wstring previous_options(options_length, L'\0');
  if (options_length) previous_options.resize(GetEnvironmentVariableW(L"MSIME_CLIENT_HOST_OPTIONS", previous_options.data(), options_length));
  const auto options_path = directory / L"runtime-options.json";
  assert(SetEnvironmentVariableW(L"MSIME_CLIENT_HOST_OPTIONS", options_path.c_str()));
  const auto context = [] { return msime::windows::win7::panel_context(); };
  assert(context().options.is_null());
  for (const auto* bytes : {"{broken", "[]", "null", "7"}) {
    { std::ofstream output(options_path, std::ios::binary); output << bytes; }
    assert(context().options.is_null());
  }
  { std::ofstream output(options_path, std::ios::binary); output << "{\"synthetic\":true}"; }
  assert(context().options.at("synthetic") == true);
  assert(DeleteFileW(options_path.c_str()));
  PreferencesStore store(directory.u8string()); const auto initial = store.load(); assert(initial.snapshot);
  const auto main_thread = GetCurrentThreadId(); std::mutex mutex;
  std::string user = "synthetic-a", error; std::vector<Json> requests;
  bool malformed = false, change_during_request = false, allow = true;
  unsigned confirmations = 0, picks = 0;
  const auto path = directory / L"合成🙂.tsv"; { std::ofstream output(path, std::ios::binary); output << "ni\t合成\t100\n"; }
  std::optional<std::filesystem::path> selected_file = path;
  CloudDictionaryServices services;
  services.account = [&](const std::string& bytes) {
    assert(GetCurrentThreadId() != main_thread && Json::parse(bytes).at("action").at("operation") == "status");
    std::lock_guard<std::mutex> lock(mutex); return Json{{"ok", true}, {"value", {{"user", {{"id", user}}}}}}.dump();
  };
  services.dictionary = [&](const std::string& bytes) {
    assert(GetCurrentThreadId() != main_thread); const auto request = Json::parse(bytes), action = request.at("action");
    assert(request.at("snapshot_directory").get<std::string>().find("win7-cloud-snapshots-") != std::string::npos);
    assert(!request.contains("client_agent")); std::lock_guard<std::mutex> lock(mutex); requests.push_back(request);
    assert(request.at("options").is_null());
    if (change_during_request) { user = user == "synthetic-a" ? "synthetic-b" : "synthetic-a"; change_during_request = false; }
    if (!error.empty()) return Json{{"ok", false}, {"error", error}}.dump();
    if (malformed) return Json{{"ok", true}, {"value", {{"entries", 7}}}}.dump();
    const auto operation = action.at("operation").get<std::string>(); Json value = Json::object();
    if (operation == "list") value = {{"entries", {{{"id", "synthetic-entry"}, {"code", "ni"}, {"word", "合成🙂"}, {"weight", 100}, {"revision", 7}}}}, {"offset", action.at("offset")}, {"has_more", action.at("offset") == 0}};
    if (operation == "catalog") value = {{"catalog_entries", {{{"code", "ni"}, {"word", "合成"}, {"weight", 100}}}}, {"offset", 0u}, {"has_more", false}, {"revision", 41}, {"normalized", "ni"}};
    if (operation == "candidates") value = {{"candidates", {{{"code", "nihc"}, {"canonical_pinyin", "nihao"}, {"word", "合成"}, {"weight", 100}}}}, {"context", "server:synthetic-context"}, {"revision", 42}};
    if (operation == "fixed_positions") value = {{"positions", {{{"context", "server:synthetic-context"}, {"code", "nihao"}, {"word", "合成"}, {"position", 2}}}}, {"offset", 0u}, {"has_more", false}};
    if (operation == "snapshot_preview") value = {{"previewToken", "synthetic-preview"}, {"snapshot", {{"sha256", std::string(64, 'a')}, {"entries", 3}}}};
    if (operation == "snapshot_restore_preview_file") value = {{"snapshot", {{"sha256", std::string(64, 'b')}, {"entries", 4}}}, {"expectedRevision", 43}};
    if (operation == "snapshot_status" || operation == "snapshot_enqueue" || operation == "snapshot_cancel") value = {{"localVersion", "synthetic-local"}, {"request", {{"status", operation == "snapshot_cancel" ? "cancelled" : "queued"}}}};
    return Json{{"ok", true}, {"value", value}}.dump();
  };
  const auto last = [&] { std::lock_guard<std::mutex> lock(mutex); assert(!requests.empty()); return requests.back().at("action"); };
  const auto count = [&] { std::lock_guard<std::mutex> lock(mutex); return requests.size(); };
  {
    msime::windows::win7::CloudDictionaryWindow window([&](HWND host, bool) { ++picks; SendMessageW(host, WM_TIMER, 1, 0); return selected_file; },
      [&](HWND host, const wchar_t*, const wchar_t*) { ++confirmations; SendMessageW(host, WM_TIMER, 1, 0); return allow; }, services);
    assert(window.create(GetModuleHandleW(nullptr)));
    const auto host = window.handle(), body = GetDlgItem(host, msime::windows::win7::cloud_dictionary_panel_body);
    const auto control = [&](int id) { const auto result = GetDlgItem(body, id); assert(result); return result; };
    const auto wait = [&] { pump([&] { return IsWindowEnabled(control(Form::query)); }); }; wait();
    const auto click = [&](int id) { assert(IsWindowEnabled(control(id))); SendMessageW(control(id), BM_CLICK, 0, 0); wait(); };
    const auto choose = [&](int id, int index) { SendMessageW(control(id), CB_SETCURSEL, index, 0); SendMessageW(body, WM_COMMAND, MAKEWPARAM(id, CBN_SELCHANGE), reinterpret_cast<LPARAM>(control(id))); };
    const auto text = [&](int id) { return control_text::read(control(id)).value(); };
    const auto edit = [&](int id, const wchar_t* text) { assert(SetWindowTextW(control(id), text)); };
    assert(SendMessageW(control(Form::items), LB_GETCOUNT, 0, 0) == 1);
    assert(text(Form::word) == "合成🙂" && store.load().snapshot->revision() == initial.snapshot->revision());
    click(Form::next); assert(last().at("offset") == 100); click(Form::previous); assert(last().at("offset") == 0);
    click(Form::new_entry); edit(Form::code, L"hao"); edit(Form::word, L"合成新增🙂"); click(Form::save);
    assert(last().at("operation") == "add" && last().at("word") == "合成新增🙂");
    click(Form::query); edit(Form::weight, L"200"); click(Form::save); assert(last().at("operation") == "update" && last().at("revision") == 7);
    click(Form::query); allow = false; const auto before_cancel = count(); click(Form::remove); assert(count() == before_cancel);
    allow = true; click(Form::remove); assert(last().at("operation") == "delete");
    choose(Form::view, 1); edit(Form::search, L"ni"); click(Form::query); edit(Form::word, L"目录覆盖"); click(Form::save);
    assert(last().at("operation") == "edit_catalog" && last().at("revision") == 41 && last().at("replacement").at("word") == "目录覆盖");
    choose(Form::view, 2); click(Form::query); choose(Form::rank_mode, 4); click(Form::rank);
    assert(last().at("operation") == "rank" && last().at("mode") == "promote" && last().at("code") == "nihao" && last().at("revision") == 42);
    click(Form::query); edit(Form::search, L"changed-after-query"); click(Form::position_save);
    assert(last().at("context") == "server:synthetic-context" && last().at("code") == "nihao");
    click(Form::query); choose(Form::view, 3); click(Form::query); click(Form::position_clear);
    assert(last().at("operation") == "set_fixed_position" && last().at("position").is_null() && last().at("revision") == 42);
    choose(Form::view, 0); click(Form::query); const auto before_file_cancel = count(); selected_file.reset(); click(Form::import_file); assert(count() == before_file_cancel);
    selected_file = path; click(Form::import_file); assert(last().at("operation") == "import" && last().at("text") == "ni\t合成\t100\n");
    click(Form::query); click(Form::export_file); assert(last().at("operation") == "export_file" && last().at("path") == path.u8string());
    click(Form::snapshot_export); assert(last().at("operation") == "snapshot_export_file");
    click(Form::snapshot_preview); assert(IsWindowEnabled(control(Form::snapshot_apply)));
    allow = false; const auto before_apply_cancel = count(); click(Form::snapshot_apply); assert(count() == before_apply_cancel);
    allow = true; click(Form::snapshot_apply); assert(last().at("token") == "synthetic-preview" && !IsWindowEnabled(control(Form::snapshot_apply)));
    click(Form::restore_choose); assert(IsWindowEnabled(control(Form::restore_apply)));
    click(Form::restore_apply); assert(last().at("expected_sha256") == std::string(64, 'b') && last().at("revision") == 43 && last().at("path") == path.u8string());
    click(Form::restore_choose); { std::lock_guard<std::mutex> lock(mutex); error = "account_conflict"; }
    click(Form::restore_apply); assert(!IsWindowEnabled(control(Form::restore_apply)) && !IsWindowEnabled(control(Form::save)));
    { std::lock_guard<std::mutex> lock(mutex); error.clear(); }
    click(Form::query); click(Form::snapshot_preview); { std::lock_guard<std::mutex> lock(mutex); user = "synthetic-b"; }
    const auto before_changed = count(); click(Form::snapshot_apply); assert(count() == before_changed && !IsWindowEnabled(control(Form::snapshot_apply)));
    click(Form::query); { std::lock_guard<std::mutex> lock(mutex); change_during_request = true; }
    click(Form::query); assert(!IsWindowEnabled(control(Form::remove)));
    click(Form::query);
    { std::lock_guard<std::mutex> lock(mutex); malformed = true; }
    click(Form::query); assert(SendMessageW(control(Form::items), LB_GETCOUNT, 0, 0) == 0 && !IsWindowEnabled(control(Form::save)));
    assert(confirmations > 0 && picks > 0 && store.load().snapshot->revision() == initial.snapshot->revision());
    DestroyWindow(host);
  }
  assert(SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", length ? previous.c_str() : nullptr));
  assert(SetEnvironmentVariableW(L"MSIME_CLIENT_HOST_OPTIONS", options_length ? previous_options.c_str() : nullptr));
  std::error_code error_code; std::filesystem::remove_all(directory, error_code);
  std::puts("Windows 7 cloud dictionary: native queries, revisions, canonical candidates, file requests, snapshot confirmations, conflicts and account changes passed");
}
