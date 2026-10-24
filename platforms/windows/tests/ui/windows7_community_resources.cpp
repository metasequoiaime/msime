#include "../../settings/win7/SettingsWindow.h"
#include "../../settings/win7/CommunityResourceForm.h"
#include <cassert>
#include <cstdio>
#include <fstream>
#include <mutex>

namespace {
using namespace msime::settings::win7;
using Form = CommunityResourceForm;
void pump(const std::function<bool()>& ready) {
  const auto deadline = GetTickCount64() + 15000;
  for (;;) {
    MSG message{}; while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
      assert(message.message != WM_QUIT); TranslateMessage(&message); DispatchMessageW(&message);
    }
    if (ready()) return;
    assert(GetTickCount64() < deadline); MsgWaitForMultipleObjects(0, nullptr, FALSE, 10, QS_ALLINPUT);
  }
}
struct Environment {
  const wchar_t* key; std::wstring previous; DWORD size;
  Environment(const wchar_t* name, const std::filesystem::path& path) : key(name), size(GetEnvironmentVariableW(name, nullptr, 0)) {
    if (size) { previous.resize(size); previous.resize(GetEnvironmentVariableW(key, previous.data(), size)); }
    assert(SetEnvironmentVariableW(key, path.c_str()));
  }
  ~Environment() { SetEnvironmentVariableW(key, size ? previous.c_str() : nullptr); }
};
}
int main(int argc, char** argv) {
  assert(argc == 2);
  wchar_t temp[MAX_PATH]{}, unique[MAX_PATH]{}; assert(GetTempPathW(MAX_PATH, temp) && GetTempFileNameW(temp, L"m7r", 0, unique));
  assert(DeleteFileW(unique) && CreateDirectoryW(unique, nullptr)); const std::filesystem::path directory(unique);
  {
    Environment state(L"MSIME_CLIENT_STATE_DIR", directory), host_options(L"MSIME_CLIENT_HOST_OPTIONS", directory / L"runtime-options.json");
    for (const auto* folder : {L"resources", L"dictionaries", L"user", L"cache"}) std::filesystem::create_directories(directory / folder);
    for (const auto* folder : {L"resources", L"dictionaries"})
      for (const auto* file : {L"msime-pinyin.db", L"msime-english.db"})
        std::filesystem::copy_file(std::filesystem::u8path(argv[1]) / file, directory / folder / file);
    PreferencesStore store(directory.u8string()); const auto initial = store.load(); assert(initial.snapshot);
    const Json options{{"api_version", 1}, {"resources", (directory / L"resources").u8string()},
      {"dictionaries", (directory / L"dictionaries").u8string()}, {"user_data", (directory / L"user").u8string()},
      {"cache", (directory / L"cache").u8string()}, {"preferences_directory", directory.u8string()}, {"preferences", initial.snapshot->preferences()}};
    { std::ofstream output(directory / L"runtime-options.json"); output << options.dump(); assert(output.good()); }
    Json publication{{"id", "00000000-0000-4000-8000-000000000456"}, {"kind", "dictionary"},
      {"name", "合成作品🙂"}, {"description", "合成说明"}, {"author", "合成作者"}, {"owned", false}, {"saved", false}, {"revision", 7},
      {"content", {{"entries", {{{"kind", "quick"}, {"code", "synthetic"}, {"word", "合成快捷语"}, {"weight", 42}},
        {{"kind", "english"}, {"code", "synthetic"}, {"word", "Synthetic"}, {"weight", 123}}}}}}};
    const DWORD ui_thread = GetCurrentThreadId(); std::mutex mutex; std::vector<Json> actions, imports;
    bool allow = false, malformed = false, reject_import = false; std::string failure;
    const auto transport = [&](const std::string& bytes) {
      assert(GetCurrentThreadId() != ui_thread); const auto request = Json::parse(bytes); const auto action = request.at("action");
      std::lock_guard<std::mutex> lock(mutex); actions.push_back(action);
      assert(!request.contains("client_agent") && request.at("state_root") == directory.u8string());
      if (!failure.empty()) return Json{{"ok", false}, {"error", failure}}.dump();
      if (malformed) return Json{{"ok", true}, {"value", {{"items", 1}, {"has_more", true}}}}.dump();
      const auto operation = action.at("operation").get<std::string>(); Json value;
      if (operation == "resource_list") {
        auto item = publication; item["owned"] = action.value("scope", std::string{}) == "mine";
        if (action.at("kind") == "reply") { item["kind"] = "reply"; item["content"] = {{"prompt", "合成回复提示词"}}; }
        value = {{"items", {item}}, {"has_more", action.value("offset", size_t{0}) == 0}};
      }
      if (operation == "resource_detail") value = publication;
      if (operation == "resource_publish") value = {{"id", action.at("id")}, {"revision", action.at("revision").get<uint32_t>() + 1}};
      return Json{{"ok", true}, {"value", value}}.dump();
    };
    const auto dictionary = [&](const std::string& bytes) {
      assert(GetCurrentThreadId() != ui_thread); const auto request = Json::parse(bytes);
      std::lock_guard<std::mutex> lock(mutex); imports.push_back(request.at("action")); assert(request.at("options") == options);
      if (reject_import && request.at("action").at("kind") == "english") return dictionary_data::failure("dictionary_busy").dump();
      // 使用真实临时词库，维护通知边界不联系已安装的输入法。
      return theme_data::call(msime_client_dictionary, request).dump();
    };
    const auto last = [&] { std::lock_guard<std::mutex> lock(mutex); assert(!actions.empty()); return actions.back(); };
    const auto count = [&] { std::lock_guard<std::mutex> lock(mutex); return actions.size(); };
    const auto imported = [&] { std::lock_guard<std::mutex> lock(mutex); return imports; };
    SettingsWindow window({}, [&](HWND, const wchar_t* title, const wchar_t* message) {
      assert(std::wstring_view(title) == L"社区作品" && message && *message); return allow;
    }, {}, transport, {}, {}, {}, {}, dictionary);
    assert(window.open_route("settings:community-resources") && window.create(GetModuleHandleW(nullptr)));
    const auto host = window.handle(), body = GetDlgItem(host, page_body), selector = GetDlgItem(host, page_selector);
    const auto control = [&](int id) { const auto value = GetDlgItem(body, id); assert(value); return value; };
    const auto wait = [&] { pump([&] { return SendMessageW(selector, CB_GETCURSEL, 0, 0) == 18 && IsWindowEnabled(GetDlgItem(body, language_switch)); }); }; wait();
    const auto click = [&](int id) { assert(IsWindowEnabled(control(id))); SendMessageW(control(id), BM_CLICK, 0, 0); wait(); };
    const auto text = [&](int id) { return control_text::read(control(id)).value(); };
    const auto set = [&](int id, const wchar_t* value) { assert(SetWindowTextW(control(id), value)); };
    assert(text(Form::detail_text).find("合成作品🙂") != std::string::npos);
    click(Form::more); assert(last().at("offset") == 1 && SendMessageW(control(Form::list), LB_GETCOUNT, 0, 0) == 1);
    click(Form::detail); assert(last().at("id") == publication.at("id"));
    click(Form::save_copy); assert(last().at("saved") == true); click(Form::save_copy); assert(last().at("saved") == false);
    const auto cancelled = count(); click(Form::apply_cloud); assert(count() == cancelled);
    allow = true; click(Form::apply_cloud); assert(last().at("resource_revision") == 7);
    set(Form::report_detail, L"合成举报"); click(Form::report); assert(last().at("kind") == "dictionaries" && text(Form::report_detail).empty());
    click(Form::rate); assert(last().at("stars") == 5);
    { std::lock_guard<std::mutex> lock(mutex); reject_import = true; }
    click(Form::apply_local); auto first = imported(); assert(first.size() == 2 && first[0].at("kind") == "quick_phrase");
    assert(text(Form::notice).find("已应用 1 条") != std::string::npos && IsWindowEnabled(control(Form::apply_local)));
    { std::lock_guard<std::mutex> lock(mutex); reject_import = false; }
    click(Form::apply_local); const auto retried = imported(); assert(retried.size() == 4);
    assert(first[0].at("request_id") == retried[2].at("request_id") && first[1].at("request_id") == retried[3].at("request_id"));
    assert(text(Form::notice).find("已导入 2") != std::string::npos);
    for (const auto* kind : {"quick_phrase", "english"}) {
      const auto listing = theme_data::value(theme_data::call(msime_client_dictionary,
        {{"options", options}, {"action", {{"operation", "list"}, {"kind", kind}, {"user_only", true}, {"offset", 0}, {"limit", 100}}}}));
      assert(listing && listing->at("entries").size() == 1);
    }
    click(Form::editor_new); set(Form::name, L"合成新作品🙂"); set(Form::description, L"合成发布说明");
    set(Form::entry_code, L"he'cheng"); set(Form::entry_word, L"合成"); click(Form::entry_add);
    assert(SendMessageW(control(Form::draft_entries), LB_GETCOUNT, 0, 0) == 1);
    click(Form::entry_add); assert(SendMessageW(control(Form::draft_entries), LB_GETCOUNT, 0, 0) == 1);
    const auto no_rights = count(); click(Form::publish); assert(count() == no_rights);
    SendMessageW(control(Form::agree), BM_SETCHECK, BST_CHECKED, 0);
    { std::lock_guard<std::mutex> lock(mutex); failure = "community_unavailable"; }
    click(Form::publish); const auto draft = last(); assert(draft.at("content").at("entries").size() == 1 && draft.at("revision") == 0);
    { std::lock_guard<std::mutex> lock(mutex); failure.clear(); }
    click(Form::publish); assert(last().at("id") == draft.at("id") && text(Form::notice).find("已发布") != std::string::npos);
    click(Form::publish); assert(last().at("revision") == 1);
    SendMessageW(control(Form::scope), CB_SETCURSEL, 2, 0); click(Form::reload);
    assert(IsWindowEnabled(control(Form::editor_existing)) && IsWindowEnabled(control(Form::unpublish)) && !IsWindowEnabled(control(Form::rate)));
    click(Form::editor_existing); click(Form::publish); assert(last().at("id") == publication.at("id") && last().at("revision") == 7);
    allow = false; const auto before_delete = count(); click(Form::unpublish); assert(count() == before_delete);
    allow = true; click(Form::unpublish); assert(last().at("operation") == "resource_unpublish");
    SendMessageW(control(Form::kind), CB_SETCURSEL, 1, 0); click(Form::reload);
    assert(!IsWindowEnabled(control(Form::apply_local)) && !IsWindowEnabled(control(Form::apply_cloud)));
    set(Form::report_detail, L"合成模板举报"); click(Form::report); assert(last().at("kind") == "replies");
    click(Form::editor_new); set(Form::name, L"合成模板"); set(Form::description, L"合成模板说明"); set(Form::prompt, L"合成提示词🙂");
    SendMessageW(control(Form::agree), BM_SETCHECK, BST_CHECKED, 0); click(Form::publish);
    assert(last().at("kind") == "reply" && last().at("content").at("prompt") == "合成提示词🙂" && !last().at("content").contains("entries"));
    { std::lock_guard<std::mutex> lock(mutex); malformed = true; }
    click(Form::reload); assert(!IsWindowEnabled(control(Form::detail)) && !IsWindowEnabled(control(Form::unpublish)));
    assert(store.load().snapshot->revision() == initial.snapshot->revision());
    auto bad_entries = publication.at("content").at("entries"); bad_entries[0]["word"] = "合成\n注入";
    const auto before_invalid = imported().size();
    assert(!theme_data::value(community_resource_data::import_local(options, {{"entries", bad_entries}, {"request_id", "synthetic"}}, dictionary)));
    assert(imported().size() == before_invalid);
    DestroyWindow(host);
  }
  std::error_code error; std::filesystem::remove_all(directory, error);
  std::puts("Windows 7 community resources: native requests, real grouped local import, partial retry receipts, revision checks, draft publication and invalid-state refusal passed");
}
