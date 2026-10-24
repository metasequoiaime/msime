#include "../../settings/win7/SettingsWindow.h"
#include "../../settings/win7/PluginEffectsForm.h"
#include "../../settings/win7/PluginLibraryForm.h"
#include "../../settings/win7/PluginMentionsForm.h"
#include "../../settings/win7/PluginScriptsForm.h"
#include <cassert>
#include <cstdio>
#include <fstream>

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
    assert(GetTempPathW(MAX_PATH, root) && GetTempFileNameW(root, L"m7p", 0, unique));
    assert(DeleteFileW(unique) && CreateDirectoryW(unique, nullptr)); path = unique;
  }
  ~TemporaryState() { std::error_code error; std::filesystem::remove_all(path, error); }
};
void write(const std::filesystem::path& path, const std::string& text) {
  std::ofstream output(path, std::ios::binary); output << text; assert(output.good());
}
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
  Environment state(L"MSIME_CLIENT_STATE_DIR", temporary.path);
  const auto options_path = temporary.path / L"runtime-options.json";
  Environment options(L"MSIME_CLIENT_HOST_OPTIONS", options_path);
  write(options_path, Json{{"plugin_packs", (temporary.path / L"builtin-packs").u8string()}}.dump());
  const auto source = temporary.path / L"source";
  std::filesystem::create_directory(source);
  const auto manifest = [&](const std::string& id, const std::string& kind, const std::string& body) {
    write(source / L"plugin.toml", "schema_version = 1\nkind = '" + kind + "'\nid = '" + id + "'\nname = '" + id + "'\nversion = '1'\nlicense = 'CC0-1.0'\n" + body);
  };
  PreferencesStore store(temporary.path.u8string());
  const auto initial = store.load(); assert(initial.snapshot);
  bool allow = false, cancel = false;
  unsigned picks = 0, confirms = 0;
  SettingsWindow window({}, [&](HWND, const wchar_t* title, const wchar_t* text) {
    assert(std::wstring_view(title) == L"插件" && text && *text); ++confirms; return allow;
  }, [&](HWND) -> std::optional<std::filesystem::path> { ++picks; return cancel ? std::nullopt : std::make_optional(source); });
  assert(window.create(GetModuleHandleW(nullptr)));
  const auto host = window.handle(), body = GetDlgItem(host, page_body), idle = GetDlgItem(body, language_switch);
  const auto wait = [&] { pump([&] { return IsWindowEnabled(idle) != FALSE; }); }; wait();
  assert(!GetDlgItem(body, PluginEffectsForm::key_enabled));
  const auto selector = GetDlgItem(host, page_selector);
  SendMessageW(selector, CB_SETCURSEL, 15, 0);
  SendMessageW(host, WM_COMMAND, MAKEWPARAM(page_selector, CBN_SELCHANGE), reinterpret_cast<LPARAM>(selector)); wait();
  const auto control = [&](int id) { const auto value = GetDlgItem(body, id); assert(value); return value; };
  const auto click = [&](int id) { SendMessageW(control(id), BM_CLICK, 0, 0); wait(); };
  const auto select = [&](int id, int index, int notification = CBN_SELCHANGE) {
    SendMessageW(control(id), (id == PluginLibraryForm::packages_list || id == PluginMentionsForm::entries || id == PluginScriptsForm::capabilities) ? LB_SETCURSEL : CB_SETCURSEL, index, 0);
    SendMessageW(host, WM_COMMAND, MAKEWPARAM(id, notification), reinterpret_cast<LPARAM>(control(id))); wait();
  };
  const auto prefs = [&] { const auto result = store.load(); assert(result.snapshot); return result.snapshot->preferences(); };
  const auto saved_json = [&](const wchar_t* name) { std::ifstream input(temporary.path / L"plugins" / name); assert(input.good()); Json document; input >> document; return document; };
  const auto revision = [&] { return store.load().snapshot->revision(); };
  assert(revision() == initial.snapshot->revision());
  assert(!IsWindowEnabled(control(PluginEffectsForm::music_enabled)));
  assert(!IsWindowEnabled(control(PluginEffectsForm::achievements_enabled)));
  // 默认省略的插件对象只补用户修改的字段，不把整个默认对象写回。
  click(PluginEffectsForm::key_enabled);
  assert(prefs().at("plugins").at("key_sound").at("enabled") == true);
  select(PluginEffectsForm::key_volume, 73);
  assert(prefs().at("plugins").at("key_sound").at("volume") == 73);
  assert(store.save(*store.load().snapshot, {{{"plugins", "unknown", "enabled"}, true}}).outcome == Outcome::invalid_change);
  const auto before_import = revision(); cancel = true; click(PluginLibraryForm::import_folder); assert(picks == 1);
  cancel = false;
  manifest("synthetic-effect", "effect", "[effect]\nstyle = 'flash'\n");
  click(PluginLibraryForm::import_folder);
  assert(revision() == before_import && std::filesystem::is_regular_file(temporary.path / L"plugins/effect/synthetic-effect/plugin.toml"));
  // 列表包含中文类型前缀，按已知唯一条目选中。
  select(PluginLibraryForm::packages_list, 0, LBN_SELCHANGE);
  click(PluginLibraryForm::use_pack); assert(prefs().at("plugins").at("effect_pack") == "synthetic-effect");
  assert(!IsWindowEnabled(control(PluginEffectsForm::intensity)));
  click(PluginLibraryForm::stop_pack); assert(prefs().at("plugins").value("effect_pack", std::string{}).empty());
  select(PluginEffectsForm::intensity, 64); assert(prefs().at("plugins").at("effect_intensity") == 64);
  click(PluginLibraryForm::use_pack);
  const auto selected_revision = revision(); click(PluginLibraryForm::remove_pack);
  assert(confirms == 1 && revision() == selected_revision && std::filesystem::exists(temporary.path / L"plugins/effect/synthetic-effect"));
  allow = true; click(PluginLibraryForm::remove_pack);
  assert(!std::filesystem::exists(temporary.path / L"plugins/effect/synthetic-effect") && prefs().at("plugins").value("effect_pack", std::string{}).empty());
  manifest("synthetic-one", "phrase_table", "[[phrases]]\nkey = 'one'\ntext = '合成🙂'\n"); click(PluginLibraryForm::import_folder);
  manifest("synthetic-two", "phrase_table", "[[phrases]]\nkey = 'two'\ntext = '合成二'\n"); click(PluginLibraryForm::import_folder);
  // 用筛选得到明确顺序，再从实际 HWND 启用并调整优先级。
  assert(SetWindowTextW(control(PluginLibraryForm::search), L"synthetic-one")); click(PluginLibraryForm::find_button);
  select(PluginLibraryForm::packages_list, 0, LBN_SELCHANGE); click(PluginLibraryForm::use_pack);
  assert(SetWindowTextW(control(PluginLibraryForm::search), L"synthetic-two")); click(PluginLibraryForm::find_button);
  select(PluginLibraryForm::packages_list, 0, LBN_SELCHANGE); click(PluginLibraryForm::use_pack); click(PluginLibraryForm::earlier);
  assert((prefs().at("plugins").at("phrase_tables") == Json::array({"synthetic-two", "synthetic-one"})));
  // 外部修订冲突不覆盖；窗口重读后可再次明确停用。
  const auto outside = store.load(); assert(store.save(*outside.snapshot, {{{"keybindings", "switch_language_caps_lock"}, true}}).outcome == Outcome::saved);
  click(PluginLibraryForm::stop_pack); assert(prefs().at("plugins").at("phrase_tables").size() == 2);
  click(PluginLibraryForm::stop_pack); assert(prefs().at("plugins").at("phrase_tables") == Json::array({"synthetic-one"}));
  const auto before_failure = revision(); write(source / L"plugin.toml", "invalid-synthetic"); click(PluginLibraryForm::import_folder);
  assert(revision() == before_failure && !control_text::read(control(PluginLibraryForm::notice)).value().empty());
  // 坏包不能让整个目录失去可用项，也不能重置偏好。
  write(temporary.path / L"plugins/phrase_table/synthetic-two/plugin.toml", "invalid-synthetic"); click(PluginLibraryForm::refresh_button);
  assert(revision() == before_failure && !control_text::read(control(PluginLibraryForm::notice)).value().empty());
  const auto current = store.load();
  assert(store.save(*current.snapshot, {{{"plugins", "effect_pack"}, "missing-synthetic"}}).outcome == Outcome::saved);
  click(PluginLibraryForm::refresh_button);
  const auto missing_index = SendMessageW(control(PluginLibraryForm::missing), CB_FINDSTRING, static_cast<WPARAM>(-1), reinterpret_cast<LPARAM>(L"特效包"));
  assert(missing_index >= 0); select(PluginLibraryForm::missing, static_cast<int>(missing_index)); click(PluginLibraryForm::clear_missing);
  assert(prefs().at("plugins").value("effect_pack", std::string{}).empty());
  // 刷新和切页不改默认值或现有选择。
  const auto final_revision = revision(); click(PluginLibraryForm::refresh_button); assert(revision() == final_revision);
  assert(prefs().at("plugins").at("key_sound").at("volume") == 73);
  // @ 名单编辑保留手机号；写入失败不丢草稿，坏文件不能被保存覆盖。
  const auto set = [&](int id, const wchar_t* value) { assert(SetWindowTextW(control(id), value)); };
  set(PluginMentionsForm::text, L"合成🙂"); set(PluginMentionsForm::key, L"he'cheng"); set(PluginMentionsForm::phone, L"synthetic-001");
  click(PluginMentionsForm::apply_entry); click(PluginMentionsForm::save);
  assert(saved_json(L"mentions.json").at("entries").at(0).at("phone") == "synthetic-001");
  select(PluginMentionsForm::entries, 0, LBN_SELCHANGE);
  set(PluginMentionsForm::text, L"合成改名🙂"); click(PluginMentionsForm::apply_entry); click(PluginMentionsForm::save);
  assert(saved_json(L"mentions.json").at("entries").at(0).at("phone") == "synthetic-001");
  set(PluginMentionsForm::text, L"合成改名🙂"); click(PluginMentionsForm::apply_entry);
  assert(!IsWindowEnabled(control(PluginMentionsForm::save)));
  const auto mentions_file = temporary.path / L"plugins/mentions.json";
  write(mentions_file, "{invalid-synthetic"); click(PluginMentionsForm::reload);
  assert(!IsWindowEnabled(control(PluginMentionsForm::apply_entry)) && !IsWindowEnabled(control(PluginMentionsForm::save)));
  write(mentions_file, Json{{"format_version", 1}, {"entries", Json::array()}}.dump()); click(PluginMentionsForm::reload);
  set(PluginMentionsForm::text, L"草稿"); click(PluginMentionsForm::apply_entry);
  allow = false; click(PluginMentionsForm::reload); assert(IsWindowEnabled(control(PluginMentionsForm::save)));
  allow = true; click(PluginMentionsForm::reload); assert(!IsWindowEnabled(control(PluginMentionsForm::save)));
  // ABI 模块只导入明确声明的读取和改写能力；导入不执行、不自动授权。
  const uint8_t module[] = {0, 97, 115, 109, 1, 0, 0, 0, 1, 21, 4, 96, 1, 127, 1, 126, 96, 2, 127, 127, 1, 127, 96, 0, 1, 127, 96, 1, 127, 1, 127, 2, 46, 2, 5, 109, 115, 105, 109, 101, 13, 99, 111, 109, 109, 105, 116, 95, 115, 99, 97, 108, 97, 114, 0, 0, 5, 109, 115, 105, 109, 101, 14, 114, 101, 119, 114, 105, 116, 101, 95, 99, 111, 109, 109, 105, 116, 0, 1, 3, 3, 2, 2, 3, 5, 3, 1, 0, 1, 7, 39, 3, 6, 109, 101, 109, 111, 114, 121, 2, 0, 9, 109, 115, 105, 109, 101, 95, 97, 98, 105, 0, 2, 14, 109, 115, 105, 109, 101, 95, 111, 110, 95, 101, 118, 101, 110, 116, 0, 3, 10, 11, 2, 4, 0, 65, 1, 11, 4, 0, 65, 0, 11};
  { std::ofstream output(source / L"main.wasm", std::ios::binary); output.write(reinterpret_cast<const char*>(module), sizeof(module)); assert(output.good()); }
  const auto script = [&](const std::string& id, bool updated = false) {
    write(source / L"plugin.toml", "schema_version = 2\nkind = 'script'\nid = '" + id + "'\nname = '" + id + "'\nversion = '1'\nlicense = 'MIT'\npermissions = ['read_commit','rewrite_commit']\n[script]\nabi = 1\nmodule = 'main.wasm'\nhooks = ['before_commit']\n" + (updated ? "# synthetic-update\n" : ""));
  };
  script("synthetic-script-one"); click(PluginLibraryForm::import_folder);
  assert(SendMessageW(control(PluginScriptsForm::scripts), CB_GETCOUNT, 0, 0) == 1);
  select(PluginScriptsForm::capabilities, 1, LBN_SELCHANGE); click(PluginScriptsForm::allow_capability);
  // 选改写时自动带上读取；撤销读取也撤销依赖它的改写。
  click(PluginScriptsForm::deny_capability);
  select(PluginScriptsForm::capabilities, 0, LBN_SELCHANGE); click(PluginScriptsForm::deny_capability);
  select(PluginScriptsForm::capabilities, 1, LBN_SELCHANGE); click(PluginScriptsForm::allow_capability);
  const auto before_grant = revision(); allow = false; click(PluginScriptsForm::authorize);
  assert(!std::filesystem::exists(temporary.path / L"plugins/script-grants.json"));
  allow = true; click(PluginScriptsForm::authorize);
  auto grants = saved_json(L"script-grants.json");
  assert(grants.at("plugins").at(0).at("enabled") == true && grants.at("plugins").at(0).at("granted").size() == 2);
  assert(revision() == before_grant);
  script("synthetic-script-two"); click(PluginLibraryForm::import_folder);
  select(PluginScriptsForm::scripts, 1);
  select(PluginScriptsForm::capabilities, 0, LBN_SELCHANGE); click(PluginScriptsForm::allow_capability); click(PluginScriptsForm::authorize);
  click(PluginScriptsForm::earlier);
  assert(saved_json(L"script-grants.json").at("plugins").at(0).at("id") == "synthetic-script-two");
  click(PluginScriptsForm::revoke);
  grants = saved_json(L"script-grants.json");
  assert(grants.at("plugins").at(0).at("enabled") == false && grants.at("plugins").at(0).at("granted").empty());
  // 页面显示之后包内容变化，旧哈希授权被拒绝并展示最新内容。
  select(PluginScriptsForm::scripts, 0);
  const auto installed_manifest = temporary.path / L"plugins/script/synthetic-script-one/plugin.toml";
  { std::ofstream output(installed_manifest, std::ios::binary | std::ios::app); output << "# synthetic-content-change\n"; assert(output.good()); }
  const auto previous_grants = saved_json(L"script-grants.json");
  click(PluginScriptsForm::authorize);
  assert(saved_json(L"script-grants.json") == previous_grants);
  assert(!control_text::read(control(PluginScriptsForm::notice)).value().empty());
  assert(revision() == before_grant);
  DestroyWindow(host);
  std::puts("Windows 7 plugins: real imports, selection, sparse preferences, priority, CAS conflict, removal, mentions, script grants and damaged-file preservation passed");
}
