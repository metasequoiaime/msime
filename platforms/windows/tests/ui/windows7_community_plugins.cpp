#include "../../settings/win7/SettingsWindow.h"
#include "../../settings/win7/CommunityPluginForm.h"
#include <cassert>
#include <cstdio>
#include <fstream>
#include <mutex>

namespace {
using namespace msime::settings::win7;
using Form = CommunityPluginForm;
void pump(const std::function<bool()>& ready) {
  const auto deadline = GetTickCount64() + 15000;
  for (;;) { MSG message{}; while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) { assert(message.message != WM_QUIT); TranslateMessage(&message); DispatchMessageW(&message); }
    if (ready()) return; assert(GetTickCount64() < deadline); MsgWaitForMultipleObjects(0, nullptr, FALSE, 10, QS_ALLINPUT); }
}
}
int main() {
  wchar_t temp[MAX_PATH]{}, unique[MAX_PATH]{}; assert(GetTempPathW(MAX_PATH, temp) && GetTempFileNameW(temp, L"m7c", 0, unique));
  assert(DeleteFileW(unique) && CreateDirectoryW(unique, nullptr)); const std::filesystem::path directory(unique);
  const auto size = GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", nullptr, 0); std::wstring previous(size, L'\0');
  if (size) previous.resize(GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", previous.data(), size));
  assert(SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", directory.c_str()));
  const auto package = directory / L"plugins/phrase_table/synthetic-share"; std::filesystem::create_directories(package);
  { std::ofstream output(package / L"plugin.toml", std::ios::binary); output << "schema_version = 1\nkind = 'phrase_table'\nid = 'synthetic-share'\nname = '合成🙂'\nversion = '1'\nlicense = 'CC0-1.0'\n[[phrases]]\nkey = 'test'\ntext = '合成内容'\n"; assert(output.good()); }
  PreferencesStore store(directory.u8string()); const auto initial = store.load(); assert(initial.snapshot);
  const DWORD main_thread = GetCurrentThreadId(); std::mutex mutex;
  std::vector<Json> actions; std::string failure; bool malformed = false, allow = false; unsigned confirmations = 0;
  Json publication{{"id", "00000000-0000-4000-8000-000000000123"}, {"kind", "phrase_table"}, {"plugin_id", "synthetic-share"},
    {"name", "合成社区🙂"}, {"description", "合成说明"}, {"author", "合成作者"}, {"version", "1"}, {"license", "CC0-1.0"}, {"owned", false}};
  const auto transport = [&](const std::string& bytes) {
    assert(GetCurrentThreadId() != main_thread); auto request = Json::parse(bytes); const auto action = request.at("action");
    assert(request.at("state_root") == directory.u8string() && !request.contains("client_agent"));
    std::lock_guard<std::mutex> lock(mutex); actions.push_back(action); const auto operation = action.at("operation").get<std::string>();
    if (!failure.empty()) return Json{{"ok", false}, {"error", failure}}.dump();
    if (malformed) return Json{{"ok", true}, {"value", {{"plugins", 1}}}}.dump();
    if (operation == "plugin_pack_preview") {
      // 只替换会话目录，实际包扫描、许可与打包检查仍走共享服务。
      request["directory"] = (directory / L"synthetic-account").u8string(); request.erase("legacy_directory");
      return theme_data::call(msime_client_community, request).dump();
    }
    Json value = Json::object();
    if (operation == "plugin_list") { auto item = publication; item["owned"] = action.value("mine", false); value = {{"plugins", {item}}, {"has_more", action.value("offset", size_t{0}) == 0}}; }
    if (operation == "plugin_detail") value = publication;
    return Json{{"ok", true}, {"value", value}}.dump();
  };
  const auto last = [&] { std::lock_guard<std::mutex> lock(mutex); assert(!actions.empty()); return actions.back(); };
  const auto count = [&] { std::lock_guard<std::mutex> lock(mutex); return actions.size(); };
  {
    SettingsWindow window({}, [&](HWND, const wchar_t* title, const wchar_t* text) { assert(std::wstring_view(title) == L"社区插件" && text && *text); ++confirmations; return allow; }, {}, transport);
    assert(window.open_route("settings:community-plugins") && window.create(GetModuleHandleW(nullptr)));
    const auto host = window.handle(), body = GetDlgItem(host, page_body), selector = GetDlgItem(host, page_selector);
    const auto control = [&](int id) { const auto value = GetDlgItem(body, id); assert(value); return value; };
    const auto wait = [&] { pump([&] { return SendMessageW(selector, CB_GETCURSEL, 0, 0) == 17 && IsWindowEnabled(GetDlgItem(body, language_switch)); }); }; wait();
    const auto click = [&](int id) { assert(IsWindowEnabled(control(id))); SendMessageW(control(id), BM_CLICK, 0, 0); wait(); };
    const auto text = [&](int id) { return control_text::read(control(id)).value(); };
    assert(SendMessageW(control(Form::list), LB_GETCOUNT, 0, 0) == 1 && text(Form::detail_text).find("合成社区🙂") != std::string::npos);
    assert(SendMessageW(control(Form::installed), CB_GETCOUNT, 0, 0) == 1);
    assert(store.load().snapshot->revision() == initial.snapshot->revision());
    click(Form::more); assert(last().at("offset") == 1 && SendMessageW(control(Form::list), LB_GETCOUNT, 0, 0) == 1);
    click(Form::detail); assert(last().at("operation") == "plugin_detail");
    const auto before_cancel = count(); click(Form::install); assert(count() == before_cancel);
    allow = true; click(Form::install); assert(last().at("operation") == "plugin_install" && last().at("plugin_id") == "synthetic-share");
    click(Form::rate); assert(last().at("operation") == "plugin_rate" && last().at("stars") == 5);
    assert(SetWindowTextW(control(Form::report_detail), L"合成举报说明")); click(Form::report); assert(last().at("kind") == "plugins" && text(Form::report_detail).empty());
    click(Form::pack_check); assert(text(Form::notice).find("检查通过") != std::string::npos && !text(Form::publish_name).empty());
    assert(SetWindowTextW(control(Form::publish_name), L"合成发布🙂") && SetWindowTextW(control(Form::publish_description), L"合成说明"));
    SendMessageW(control(Form::publish_agree), BM_SETCHECK, BST_CHECKED, 0);
    { std::lock_guard<std::mutex> lock(mutex); failure = "community_unavailable"; }
    click(Form::publish); const auto publication_id = last().at("id"); assert(last().at("name") == "合成发布🙂" && !publication_id.get<std::string>().empty());
    { std::lock_guard<std::mutex> lock(mutex); failure.clear(); }
    click(Form::publish); assert(last().at("id") == publication_id && text(Form::notice).find("已发布") != std::string::npos);
    SendMessageW(control(Form::mine), BM_SETCHECK, BST_CHECKED, 0); click(Form::reload);
    assert(IsWindowEnabled(control(Form::unpublish)) && !IsWindowEnabled(control(Form::rate)));
    allow = false; const auto before_delete = count(); click(Form::unpublish); assert(count() == before_delete);
    allow = true; click(Form::unpublish); assert(last().at("operation") == "plugin_delete" && std::filesystem::exists(package / L"plugin.toml"));
    assert(SetWindowTextW(control(Form::search), L"搜索🙂")); SendMessageW(control(Form::filter), CB_SETCURSEL, 5, 0); click(Form::reload);
    assert(last().at("search") == "搜索🙂" && last().at("kind") == "phrase_table");
    { std::lock_guard<std::mutex> lock(mutex); malformed = true; }
    click(Form::reload); assert(!IsWindowEnabled(control(Form::install)) && SendMessageW(control(Form::list), LB_GETCOUNT, 0, 0) == 0);
    assert(confirmations > 0 && store.load().snapshot->revision() == initial.snapshot->revision());
    DestroyWindow(host);
  }
  assert(SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", size ? previous.c_str() : nullptr));
  std::error_code error; std::filesystem::remove_all(directory, error);
  std::puts("Windows 7 community plugins: native requests, real local pack check, publication retry identity, ownership, confirmations and invalid-list refusal passed");
}
