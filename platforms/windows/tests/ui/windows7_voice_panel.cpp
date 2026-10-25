#include "../../panels/win7/VoiceWindow.h"
#include "../../settings/win7/PreferencesStore.h"
#include "../../settings/win7/ControlText.h"
#include "../../settings/win7/StatisticsData.h"
#include <commctrl.h>
#include <atomic>
#include <cassert>
#include <cstdio>
#include <map>
#include <mutex>
#include <thread>

namespace {
using namespace msime::windows::win7;
using namespace msime::settings::win7;
struct TemporaryState {
  std::filesystem::path path;
  TemporaryState() { wchar_t root[MAX_PATH]{}, file[MAX_PATH]{}; assert(GetTempPathW(MAX_PATH,root) && GetTempFileNameW(root,L"m7v",0,file)); assert(DeleteFileW(file) && CreateDirectoryW(file,nullptr)); path = file; }
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
    if (condition()) return; assert(GetTickCount64() < deadline); MsgWaitForMultipleObjects(0,nullptr,FALSE,10,QS_ALLINPUT);
  }
}
struct Session { std::atomic<bool> stopped{false},cancelled{false}; };
}
int main() {
  TemporaryState temporary;
  Environment state(L"MSIME_CLIENT_STATE_DIR",temporary.path), options(L"MSIME_CLIENT_HOST_OPTIONS",temporary.path / L"runtime-options.json");
  PreferencesStore store(temporary.path.u8string()); const auto initial = store.load(); assert(initial.snapshot);
  const auto saved = store.save(*initial.snapshot,{{{"voice_input","language"},"en-US"}}); assert(saved.snapshot);
  assert(theme_data::value(theme_data::call(msime_client_typing_statistics,{{"directory",temporary.path.u8string()},{"action",{{"operation","set_enabled"},{"enabled",true}}}})));
  const auto total = [&] { const auto value = theme_data::value(theme_data::call(msime_client_typing_statistics,{{"directory",temporary.path.u8string()},{"action",{{"operation","load"}}}})); assert(value); return value->value("total",0); };
  const auto ui_thread = std::this_thread::get_id(); std::mutex mutex; uint64_t next = 0;
  std::map<uint64_t,std::shared_ptr<Session>> sessions;
  std::atomic<unsigned> recognized{0},closed{0}; std::atomic<bool> malformed{false};
  const VoiceSessionApi api{
    [&] {
      std::lock_guard<std::mutex> lock(mutex); const auto id = ++next; sessions[id] = std::make_shared<Session>();
      return Json{{"ok",true},{"value",{{"handle",id}}}}.dump();
    },
    [&](uint64_t id, unsigned operation) {
      std::lock_guard<std::mutex> lock(mutex); const auto found = sessions.find(id);
      if (found == sessions.end()) return Json{{"ok",false},{"error","voice_session_invalid"}}.dump();
      if (operation == 0) found->second->stopped = true; else found->second->cancelled = true;
      if (operation == 2) { ++closed; sessions.erase(found); }
      return Json{{"ok",true},{"value",{{"accepted",true}}}}.dump();
    },
    [&](uint64_t id, const std::string& language, const VoiceSessionApi::Progress& progress) {
      assert(std::this_thread::get_id() != ui_thread && language == "en-US");
      std::shared_ptr<Session> session;
      { std::lock_guard<std::mutex> lock(mutex); const auto found = sessions.find(id); if (found == sessions.end()) return Json{{"error","voice_cancelled"}}.dump(); session = found->second; }
      progress(Json{{"phase","recording"},{"level",0.75},{"text","synthetic-stream"}}.dump()); ++recognized;
      const auto deadline = GetTickCount64() + 10000;
      while (!session->stopped && !session->cancelled) { assert(GetTickCount64() < deadline); std::this_thread::sleep_for(std::chrono::milliseconds(5)); }
      if (!session->cancelled) progress(Json{{"phase","processing"},{"level",0},{"text","synthetic-polish"}}.dump());
      // 故意在取消后也返回成功，验证 UI 不把迟到结果开放给复制或输入。
      return Json{{"ok",true},{"value",{{"text",malformed ? Json(7) : Json("合成语音🙂")}}}}.dump();
    }
  };
  bool insert_ok = false,copy_ok = true; unsigned inserts = 0,copies = 0,settings = 0; std::wstring received;
  VoiceWindow window([&](std::wstring_view text) { ++inserts; received = text; return insert_ok; },
    [&](HWND,std::wstring_view text) { ++copies; received = text; return copy_ok; },api,[&] { ++settings; return true; });
  assert(window.create(GetModuleHandleW(nullptr))); const auto host = window.handle();
  const auto control = [&](int id) { const auto child = GetDlgItem(host,id); assert(child); return child; };
  const auto click = [&](int id) { SendMessageW(control(id),BM_CLICK,0,0); };
  const auto done = [&] { pump([&] { return IsWindowEnabled(control(voice_panel_start)) != FALSE; }); };
  assert(recognized == 0 && total() == 0); click(voice_panel_settings); assert(settings == 1);
  assert(!IsWindowEnabled(control(voice_panel_insert))); click(voice_panel_insert); assert(inserts == 0);
  click(voice_panel_start); pump([&] { return recognized == 1 && SendMessageW(control(voice_panel_level),PBM_GETPOS,0,0) == 75; });
  assert(!IsWindowEnabled(control(voice_panel_result)) && !IsWindowEnabled(control(voice_panel_copy)));
  assert((GetWindowLongPtrW(host,GWL_EXSTYLE) & WS_EX_NOACTIVATE) != 0);
  assert(SendMessageW(host,WM_MOUSEACTIVATE,0,0) == MA_NOACTIVATE);
  click(voice_panel_insert); click(voice_panel_start); assert(inserts == 0 && recognized == 1);
  click(voice_panel_stop); done(); assert(closed == 1 && control_text::read(control(voice_panel_result)).value() == "合成语音🙂");
  assert(IsWindowEnabled(control(voice_panel_result)) && (GetWindowLongPtrW(host,GWL_EXSTYLE) & WS_EX_NOACTIVATE) == 0);
  click(voice_panel_copy); assert(copies == 1 && total() == 0 && received == L"合成语音🙂");
  copy_ok = false; click(voice_panel_copy); assert(copies == 2 && total() == 0);
  assert(SetWindowTextW(control(voice_panel_result),L"合成编辑")); click(voice_panel_insert); assert(inserts == 1 && total() == 0 && received == L"合成编辑");
  insert_ok = true; click(voice_panel_insert); done(); assert(inserts == 2 && total() > 0); const auto counted = total();
  click(voice_panel_start); pump([&] { return recognized == 2; }); click(voice_panel_cancel); done();
  assert(closed == 2 && !IsWindowEnabled(control(voice_panel_insert)) && GetWindowTextLengthW(control(voice_panel_result)) == 0);
  assert(total() == counted && inserts == 2);
  click(voice_panel_start); pump([&] { return recognized == 3; }); click(voice_panel_stop);
  // 不分发窗口消息，等工作线程关闭句柄后立即取消，覆盖完成消息尚在队列中的竞争。
  const auto race_deadline = GetTickCount64() + 10000;
  while (closed < 3) { assert(GetTickCount64() < race_deadline); std::this_thread::sleep_for(std::chrono::milliseconds(5)); }
  click(voice_panel_cancel); done(); assert(!IsWindowEnabled(control(voice_panel_insert)));
  malformed = true; click(voice_panel_start); pump([&] { return recognized == 4; }); click(voice_panel_stop); done();
  assert(closed == 4 && !IsWindowEnabled(control(voice_panel_insert)));
  assert(store.load().snapshot->revision() == saved.snapshot->revision());
  click(voice_panel_start); pump([&] { return recognized == 5; }); const auto before = GetTickCount64(); DestroyWindow(host);
  assert(GetTickCount64() - before < 2000 && closed == 5);
  { std::lock_guard<std::mutex> lock(mutex); assert(sessions.empty()); }
  // 只创建和关闭句柄：不执行真实录音，验证原生 ABI 的取消与重复关闭边界。
  const auto raw = msime::host_api::own_string(msime_client_windows_voice_session_create()); assert(raw);
  const auto created = Json::parse(raw.get()); const auto handle = created.at("value").at("handle").get<uint64_t>();
  const auto cancelled = msime::host_api::own_string(msime_client_windows_voice_session_control(handle,1)); assert(Json::parse(cancelled.get()).at("ok") == true);
  const auto answer = msime::host_api::own_string(msime_client_windows_voice_session_recognize(handle,reinterpret_cast<const uint8_t*>("en-US"),5,nullptr,nullptr));
  assert(Json::parse(answer.get()).at("error") == "voice_cancelled");
  const auto released = msime::host_api::own_string(msime_client_windows_voice_session_control(handle,2)); assert(Json::parse(released.get()).at("ok") == true);
  const auto repeated = msime::host_api::own_string(msime_client_windows_voice_session_control(handle,2)); assert(Json::parse(repeated.get()).at("error") == "voice_session_invalid");
  std::puts("Windows 7 voice: real HWND progress, read-only recording, edited review, copy/insert accounting, late cancellation, close/join and shared ABI boundaries passed");
}
