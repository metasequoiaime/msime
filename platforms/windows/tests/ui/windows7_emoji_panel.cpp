#include "../../panels/win7/EmojiWindow.h"
#include "../../panels/win7/EmojiRecent.h"
#include "../../settings/win7/PreferencesStore.h"
#include "../../settings/win7/ControlText.h"
#include <cassert>
#include <cstdio>
#include <fstream>
#include <thread>

namespace {
using namespace msime::settings::win7;
using namespace msime::windows::win7;
struct TemporaryState {
  std::filesystem::path path;
  TemporaryState() { wchar_t root[MAX_PATH]{}, file[MAX_PATH]{}; assert(GetTempPathW(MAX_PATH,root) && GetTempFileNameW(root,L"m7e",0,file)); assert(DeleteFileW(file) && CreateDirectoryW(file,nullptr)); path = file; }
  ~TemporaryState() { std::error_code error; std::filesystem::remove_all(path,error); }
};
struct Environment {
  const wchar_t* name; std::wstring previous; bool present = false;
  Environment(const wchar_t* key, const std::filesystem::path& value) : name(key) {
    const auto size = GetEnvironmentVariableW(name,nullptr,0); present = size != 0;
    if (present) { previous.resize(size); previous.resize(GetEnvironmentVariableW(name,previous.data(),size)); }
    assert(SetEnvironmentVariableW(name,value.c_str()));
  }
  ~Environment() { SetEnvironmentVariableW(name,present ? previous.c_str() : nullptr); }
};
void pump(const std::function<bool()>& condition) {
  const auto deadline = GetTickCount64() + 10000;
  for (;;) {
    MSG message{}; while (PeekMessageW(&message,nullptr,0,0,PM_REMOVE)) { assert(message.message != WM_QUIT); TranslateMessage(&message); DispatchMessageW(&message); }
    if (condition()) return; assert(GetTickCount64() < deadline); MsgWaitForMultipleObjects(0,nullptr,FALSE,20,QS_ALLINPUT);
  }
}
}
int main() {
  TemporaryState temporary;
  Environment state(L"MSIME_CLIENT_STATE_DIR",temporary.path), options(L"MSIME_CLIENT_HOST_OPTIONS",temporary.path / L"runtime-options.json");
  PreferencesStore store(temporary.path.u8string()); const auto initial = store.load(); assert(initial.snapshot);
  const auto plugin = temporary.path / L"plugins/symbol_set/fixture"; std::filesystem::create_directories(plugin);
  { std::ofstream output(plugin / L"plugin.toml"); output << "schema_version=1\nkind=\"symbol_set\"\nid=\"fixture\"\nname=\"Fixture\"\nversion=\"1\"\nlicense=\"CC0-1.0\"\n[[groups]]\ntab=\"symbols\"\ntitle=\"arrows\"\nkeywords=\"synthetic-plugin\"\nitems=[\"-->\",\"<--\"]\n"; assert(output.good()); }
  assert(theme_data::value(theme_data::call(msime_client_typing_statistics,{{"directory",temporary.path.u8string()},{"action",{{"operation","set_enabled"},{"enabled",true}}}})));
  const auto total = [&] { const auto value = theme_data::value(theme_data::call(msime_client_typing_statistics,{{"directory",temporary.path.u8string()},{"action",{{"operation","load"}}}})); assert(value); return value->value("total",0); };
  const auto ui_thread = std::this_thread::get_id(); bool malformed = false, stalled = false, insert_ok = false, copy_ok = false;
  unsigned inserts = 0, copies = 0, clipboards = 0; std::wstring received;
  const auto transport = [&](const std::string& bytes, const std::string& resources) {
    assert(std::this_thread::get_id() != ui_thread); const auto query = Json::parse(bytes);
    if (query.value("list_plugin_symbol_groups",false)) return theme_data::response(msime_client_emoji_catalog_request(
      reinterpret_cast<const uint8_t*>(bytes.data()),bytes.size(),reinterpret_cast<const uint8_t*>(resources.data()),resources.size())).dump();
    if (malformed) return Json{{"ok",true},{"value",{{"items",3}}}}.dump();
    if (query.value("list_symbol_groups",false)) return Json{{"ok",true},{"value",{{"symbol_groups",{
      {{"parent","first"},{"title","same"}},{{"parent","second"},{"title","same"}}}}}}}.dump();
    const auto category = query.value("category",std::string{}); const auto offset = query.value("offset",size_t{0});
    Json items = Json::array(); size_t next = 0; bool complete = true;
    if (category == "symbols") {
      assert(query.at("group") == "same"); const auto parent = query.at("parent").get<std::string>();
      items.push_back({{"text","symbol-" + parent},{"annotation","synthetic"},{"group","same"}}); next = 1;
    } else if (category == "kaomoji") { items.push_back({{"text","(^_^)"},{"annotation","synthetic-happy"},{"group","All"}}); next = 1; }
    else if (offset == 0) { next = stalled ? 0 : 255; complete = false; }
    else if (offset == 255) {
      for (size_t index = 0; index < 255; ++index) items.push_back({{"text","synthetic-" + std::to_string(index)},{"annotation","searchable"},{"group","fixture"}});
      next = 510; complete = false;
    } else { assert(offset == 510); for (int index = 0; index < 2; ++index) items.push_back({{"text","合成🙂"},{"annotation","synthetic-tail"},{"group","fixture"}}); next = 512; }
    return Json{{"ok",true},{"value",{{"items",items},{"next_offset",next},{"complete",complete}}}}.dump();
  };
  EmojiWindow window([&](std::wstring_view text) { ++inserts; received = text; return insert_ok; },
    [&](HWND,std::wstring_view text) { ++copies; received = text; return copy_ok; },transport,[&] { ++clipboards; return true; });
  assert(window.create(GetModuleHandleW(nullptr))); const auto host = window.handle();
  const auto control = [&](int id) { const auto child = GetDlgItem(host,id); assert(child); return child; };
  const auto wait = [&] { pump([&] { return IsWindowEnabled(control(emoji_panel_refresh)) != FALSE; }); }; wait();
  const auto click = [&](int id) { SendMessageW(control(id),BM_CLICK,0,0); wait(); };
  const auto combo = [&](int id, int index) { SendMessageW(control(id),CB_SETCURSEL,index,0); SendMessageW(host,WM_COMMAND,MAKEWPARAM(id,CBN_SELCHANGE),reinterpret_cast<LPARAM>(control(id))); };
  const auto select = [&](int index) { SendMessageW(control(emoji_panel_entries),LB_SETCURSEL,index,0); SendMessageW(host,WM_COMMAND,MAKEWPARAM(emoji_panel_entries,LBN_SELCHANGE),reinterpret_cast<LPARAM>(control(emoji_panel_entries))); };
  const auto count = [&] { return SendMessageW(control(emoji_panel_entries),LB_GETCOUNT,0,0); };
  assert(count() == 48 && control_text::read(control(emoji_panel_page)).value().find("257") != std::string::npos);
  for (int page = 0; page < 5; ++page) click(emoji_panel_next); assert(count() == 17);
  SetWindowTextW(control(emoji_panel_search),L"SYNTHETIC-TAIL"); assert(count() == 2);
  select(0); click(emoji_panel_insert); assert(inserts == 1 && total() == 0 && received == L"合成🙂");
  assert(emoji::read_recent(temporary.path).items.empty());
  insert_ok = true; click(emoji_panel_insert); assert(inserts == 2 && total() > 0); const auto counted = total();
  select(0); click(emoji_panel_copy); assert(copies == 1 && total() == counted);
  copy_ok = true; click(emoji_panel_copy); assert(copies == 2 && received == L"合成🙂" && total() == counted);
  assert(emoji::read_recent(temporary.path).items.size() == 1);
  SetWindowTextW(control(emoji_panel_search),L""); combo(emoji_panel_group,1); assert(count() == 1);
  combo(emoji_panel_tab,1); assert(count() == 1); SetWindowTextW(control(emoji_panel_search),L"happy"); assert(count() == 1);
  SetWindowTextW(control(emoji_panel_search),L""); combo(emoji_panel_tab,2); assert(count() == 4);
  assert(SendMessageW(control(emoji_panel_group),CB_GETCOUNT,0,0) == 5);
  combo(emoji_panel_group,2); assert(count() == 1); select(0); click(emoji_panel_copy); assert(received == L"symbol-first");
  combo(emoji_panel_group,3); assert(count() == 1); select(0); click(emoji_panel_copy); assert(received == L"symbol-second");
  combo(emoji_panel_group,4); assert(count() == 2); click(emoji_panel_clipboard); assert(clipboards == 1);
  // 失败刷新必须撤销内置目录的旧选择；插件仍从真实共享服务独立读取。
  malformed = true; click(emoji_panel_refresh); assert(count() == 2 && !IsWindowEnabled(control(emoji_panel_insert)));
  combo(emoji_panel_group,0); assert(count() == 2); SetWindowTextW(control(emoji_panel_search),L"synthetic"); assert(count() == 2);
  combo(emoji_panel_tab,0); assert(count() == 0 && !IsWindowEnabled(control(emoji_panel_copy)));
  malformed = false; stalled = true; click(emoji_panel_refresh); assert(count() == 0);
  assert(control_text::read(control(emoji_panel_status)).value().find("内置表情") != std::string::npos);
  stalled = false; click(emoji_panel_refresh);
  // 损坏最近文件保留；成功复制照常，但不能把界面默认值写回原文件。
  const auto recent_file = temporary.path / L"emoji-panel-recent.json";
  { std::ofstream output(recent_file); output << "synthetic-invalid"; }
  SetWindowTextW(control(emoji_panel_search),L"tail"); select(0); click(emoji_panel_copy);
  { std::ifstream input(recent_file); const std::string bytes(std::istreambuf_iterator<char>(input),{}); assert(bytes == "synthetic-invalid"); }
  std::filesystem::remove(recent_file);
  // 两类最近使用分别最多 28 项；每次持锁重读，不依赖窗口的旧快照。
  for (unsigned index = 0; index < 31; ++index) {
    assert(emoji::remember(temporary.path,{"synthetic-" + std::to_string(index),"fixture","emoji",""}).readable);
    assert(emoji::remember(temporary.path,{"symbol-" + std::to_string(index),"fixture","symbols",""}).readable);
  }
  assert(emoji::read_recent(temporary.path).items.size() == 56);
  const auto lock_path = temporary.path / L"emoji-panel-recent.lock";
  {
    emoji::Handle lock(CreateFileW(lock_path.c_str(),GENERIC_READ | GENERIC_WRITE,0,nullptr,OPEN_EXISTING,0,nullptr)); assert(lock.value != INVALID_HANDLE_VALUE);
    assert(!emoji::remember(temporary.path,{"blocked","fixture","emoji",""}).readable);
    assert(emoji::read_recent(temporary.path).items.size() == 56);
  }
  assert(store.load().snapshot->revision() == initial.snapshot->revision());
  DestroyWindow(host);
  // 不注入目录传输时，内置资源缺失也不影响真实插件目录。
  EmojiWindow actual({}, {}, {}, {}); assert(actual.create(GetModuleHandleW(nullptr)));
  const auto real_host = actual.handle(); pump([&] { return IsWindowEnabled(GetDlgItem(real_host,emoji_panel_refresh)) != FALSE; });
  SendMessageW(GetDlgItem(real_host,emoji_panel_tab),CB_SETCURSEL,2,0);
  SendMessageW(real_host,WM_COMMAND,MAKEWPARAM(emoji_panel_tab,CBN_SELCHANGE),0);
  assert(SendMessageW(GetDlgItem(real_host,emoji_panel_entries),LB_GETCOUNT,0,0) == 2);
  DestroyWindow(real_host);
  std::puts("Windows 7 emoji panel: cursor paging, duplicate preservation, parent groups, real plugins, HWND search/transfers, statistics and recent-file preservation passed");
}
