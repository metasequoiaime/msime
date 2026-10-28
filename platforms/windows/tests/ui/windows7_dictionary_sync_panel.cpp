#include "../../panels/win7/DictionarySyncWindow.h"
#include "../../settings/DictionarySyncModel.h"
#include "../../settings/win7/DictionarySyncForm.h"
#include "../../settings/win7/PreferencesStore.h"
#include "../../settings/win7/ControlText.h"
#include "../../settings/win7/ThemeData.h"
#include <atomic>
#include <cassert>
#include <chrono>
#include <future>
#include <thread>
#include <cstdio>
#include <fstream>
#include <string>
#include <vector>

namespace {
using namespace msime::settings::win7;
using namespace msime::windows::win7;
namespace model = msime::settings::dictionary_sync;
using Form = DictionarySyncForm;
struct TemporaryState {
  std::filesystem::path path;
  std::wstring previous;
  bool present = false;
  TemporaryState() {
    wchar_t root[MAX_PATH]{}, file[MAX_PATH]{}; assert(GetTempPathW(MAX_PATH, root) && GetTempFileNameW(root, L"m7d", 0, file)); assert(DeleteFileW(file) && CreateDirectoryW(file, nullptr)); path = file;
    const auto size = GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", nullptr, 0); present = size != 0;
    if (present) { previous.resize(size); previous.resize(GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", previous.data(), size)); }
    assert(SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", path.c_str()));
  }
  ~TemporaryState() { SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", present ? previous.c_str() : nullptr); std::error_code error; std::filesystem::remove_all(path, error); }
};
void pump(const std::function<bool()>& condition) {
  const auto deadline = GetTickCount64() + 10000;
  for (;;) { MSG message{}; while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) { assert(message.message != WM_QUIT); TranslateMessage(&message); DispatchMessageW(&message); }
    if (condition()) return;
    assert(GetTickCount64() < deadline); MsgWaitForMultipleObjects(0, nullptr, FALSE, 20, QS_ALLINPUT); }
}
Json success(Json value) { return Json{{"ok", true}, {"value", std::move(value)}}; }
}

int main() {
  TemporaryState temporary; PreferencesStore store(temporary.path.u8string()); assert(store.load().snapshot);
  // 面板读状态目录里的运行时选项（HostOptions），合成回答不看它的内容，只要求它是对象、原样带上。
  { std::ofstream options(temporary.path / L"runtime-options.json", std::ios::binary); options << R"({"api_version":1,"synthetic":true})"; }
  const auto ui_thread = GetCurrentThreadId();
  const std::string own(32, 'a'), waiting(32, 'b'), other(32, 'c');
  std::vector<std::string> operations;
  Json probe_answer = Json{{"ok", false}, {"error", "dictionary_sync_unavailable"}}, last_probe, last_run;
  std::string last_device;
  uint64_t held = 0, pending = 0;
  unsigned conflicts = 0;
  const auto status = [&] {
    return Json{{"device_id", own}, {"code", "0123 4567 8901 2345"}, {"approved", true}, {"joined", true}, {"words", 12u},
        {"pending_upload", pending}, {"pending_apply", 0u}, {"held_deletions", held}, {"clock_skew", false}, {"last_success_ms", nullptr}, {"last_error", nullptr}};
  };
  // 同一台等待中的设备登记了两条（#10032 之前留下的旧登记），列表里只能有一条。
  const auto devices = [&] {
    return Json{{"enabled", true}, {"status", "ready"}, {"device_id", own}, {"code", "0123 4567 8901 2345"}, {"devices", Json::array({
        {{"device_id", own}, {"name", "本机"}, {"created_at", "2026-10-27T00:00:00Z"}, {"approved", true}, {"current", true}, {"code", "0123 4567 8901 2345"}},
        {{"device_id", waiting}, {"name", "合成笔记本"}, {"created_at", "2026-10-27T00:00:00Z"}, {"approved", false}, {"current", false}, {"code", "1111 2222 3333 4444"}},
        {{"device_id", other}, {"name", "合成手机"}, {"created_at", "2026-10-27T00:00:00Z"}, {"approved", true}, {"current", false}, {"code", "5555 6666 7777 8888"}},
        {{"device_id", waiting}, {"name", "合成笔记本"}, {"created_at", "2026-10-27T00:01:00Z"}, {"approved", false}, {"current", false}, {"code", "1111 2222 3333 4444"}}})}};
  };
  DictionarySyncWindow::Services services;
  services.sync = [&](const std::string& bytes) {
    assert(GetCurrentThreadId() != ui_thread);
    const auto request = Json::parse(bytes);
    // 同步目录是本用户的 dictionary-sync，与 Server 和 WinUI 设置同一个。
    assert(std::filesystem::u8path(request.at("directory").get<std::string>()).filename() == "dictionary-sync");
    const auto& action = request.at("action");
    const auto operation = action.at("operation").get<std::string>();
    operations.push_back(operation);
    if (operation == "probe") { last_probe = action; return probe_answer.dump(); }
    assert(request.at("options").at("synthetic") == true);
    // 密钥串身份冲突（同一安装的另一个进程刚存下身份）：宿主要重试，不能把它当成失败交给用户。
    if (conflicts) { --conflicts; return Json{{"ok", false}, {"error", "clipboard_keystore_conflict"}}.dump(); }
    if (operation == "run") {
      last_run = action;
      if (action.value("restore_deletions", false) || action.value("confirm_deletions", false)) held = 0;
      return success({{"network", true}, {"next_check_ms", 60000u}, {"status", status()}, {"report", {{"deferred", false}, {"pending_key", false}}}}).dump();
    }
    if (operation == "devices" || operation == "approve" || operation == "reject") {
      if (operation != "devices") last_device = action.at("device_id").get<std::string>();
      return success(devices()).dump();
    }
    if (operation == "status") return success(status()).dump();
    if (operation == "remove" || operation == "reset") {
      if (operation == "remove") last_device = action.at("device_id").get<std::string>();
      return success({{"devices", devices()}, {"run", {{"network", true}, {"next_check_ms", 60000u}, {"status", status()}, {"report", Json::object()}}}}).dump();
    }
    return success(status()).dump();
  };
  unsigned confirms = 0; bool allowed = false; bool change_during_confirmation = false;
  DictionarySyncWindow window(services, [&](HWND, const wchar_t* title, const wchar_t* text) { assert(std::wstring_view(title) == L"词库同步" && text && *text); ++confirms;
    if (change_during_confirmation) {
      change_during_confirmation = false;
      const auto current = store.load().snapshot;
      assert(current);
      const auto changed = store.save(*current, {{{"dictionary_sync_webdav", "password"}, "synthetic-other-password"}});
      assert(changed.outcome == Outcome::saved);
    }
    return allowed; });
  assert(window.create(GetModuleHandleW(nullptr))); const auto host = window.handle(), body = GetDlgItem(host, dictionary_sync_panel_body);
  const auto control = [&](int id) { const auto value = GetDlgItem(body, id); assert(value); return value; };
  const auto wait = [&] { pump([&] { return IsWindowEnabled(control(Form::test)) != FALSE; }); }; wait();
  const auto click = [&](int id) { SendMessageW(control(id), BM_CLICK, 0, 0); wait(); };
  const auto set = [&](int id, const wchar_t* value) { assert(SetWindowTextW(control(id), value)); };
  const auto read = [&](int id) { return control_text::read(control(id), 8192).value_or(""); };
  const auto message = [](std::wstring_view text) { return control_text::utf8(text).value_or(""); };
  const auto preferences = [&] { return store.load().snapshot->preferences(); };
  const auto select = [&](int index) { SendMessageW(control(Form::devices), LB_SETCURSEL, index, 0); SendMessageW(body, WM_COMMAND, MAKEWPARAM(Form::devices, LBN_SELCHANGE), reinterpret_cast<LPARAM>(control(Form::devices))); };

  // 没配网盘：打开面板不调用同步，立即同步和设备管理不可用；应用密码遮罩。
  assert(operations.empty() && !IsWindowEnabled(control(Form::sync_now)) && !IsWindowEnabled(control(Form::reset)));
  assert(GetWindowLongPtrW(control(Form::password), GWL_STYLE) & ES_PASSWORD);
  set(Form::url, L"http://127.0.0.1:9/dav"); set(Form::username, L"synthetic@example.test"); set(Form::password, L"synthetic-app-password");
  const auto revision = store.load().snapshot->revision();
  // 测试连接不通过：保存被拦下，什么都不写，输入留着。
  click(Form::save);
  assert(operations == std::vector<std::string>{"probe"} && store.load().snapshot->revision() == revision && !preferences().contains("dictionary_sync_webdav"));
  assert(read(Form::notice) == message(model::error_message(L"dictionary_sync_unavailable", true)));
  assert(last_probe.at("url") == "http://127.0.0.1:9/dav" && last_probe.at("username") == "synthetic@example.test" && last_probe.at("password") == "synthetic-app-password");
  assert(read(Form::url) == "http://127.0.0.1:9/dav" && read(Form::password) == "synthetic-app-password");
  // 只测试不保存，通过了也不写盘。
  probe_answer = success(Json::object());
  click(Form::test);
  assert(store.load().snapshot->revision() == revision && read(Form::notice) == message(model::probe_passed));
  // 测试并保存：通过后写偏好，密码框清空，然后以 opened 同步一次、取设备、读概况。
  operations.clear(); click(Form::save);
  assert(preferences().at("dictionary_sync_webdav") == (Json{{"url", "http://127.0.0.1:9/dav"}, {"username", "synthetic@example.test"}, {"password", "synthetic-app-password"}}));
  assert(read(Form::password).empty() && !preferences().contains("cloud_clipboard_webdav"));
  assert((operations == std::vector<std::string>{"probe", "run", "devices", "status"}) && last_run.at("trigger") == "opened");
  assert(read(Form::status).find("0123 4567 8901 2345") != std::string::npos);
  // 普通探测不恢复；恢复必须单独确认，且使用已保存配置，不用输入框里的草稿。
  assert(!last_probe.value("trust_remote", false));
  operations.clear(); click(Form::trust_remote); assert(confirms == 1 && operations.empty());
  allowed = true; set(Form::url, L"http://127.0.0.1:9/unsaved"); click(Form::trust_remote);
  assert(confirms == 2 && last_probe.value("trust_remote", false));
  assert(last_probe.at("url") == "http://127.0.0.1:9/dav");
  assert((operations == std::vector<std::string>{"probe", "status"}));
  allowed = false;
  confirms = 0;
  set(Form::url, L"http://127.0.0.1:9/dav");
  // 同一台设备两条登记只列一条：等待允许的笔记本在前，后面是本机和手机。
  assert(SendMessageW(control(Form::devices), LB_GETCOUNT, 0, 0) == 3);
  select(0); assert(IsWindowEnabled(control(Form::approve)) && !IsWindowEnabled(control(Form::remove)));
  // 允许之前要确认核对码；取消就不发请求。
  operations.clear(); click(Form::approve); assert(confirms == 1 && operations.empty());
  allowed = true; click(Form::approve); assert(confirms == 2 && last_device == waiting && (operations == std::vector<std::string>{"approve", "status"}));
  // 本机不能移除，其他已获准的设备可以。
  select(1); assert(!IsWindowEnabled(control(Form::remove)) && !IsWindowEnabled(control(Form::approve)));
  select(2); assert(IsWindowEnabled(control(Form::remove)));
  click(Form::remove); assert(last_device == other);
  // 身份冲突时重试：第一次答 clipboard_keystore_conflict，第二次照常，用户只看到「已同步」。
  conflicts = 1; operations.clear(); click(Form::sync_now);
  assert((operations == std::vector<std::string>{"run", "run", "devices"}) && last_run.at("trigger") == "manual");
  assert(read(Form::notice) == message(model::run_notice(true, false)));
  // Server 在后台记下了一个待发送的词：窗口重新获得焦点时只读一次不联网的概况，换上新的数量，结果说明和选中的设备都不动。
  select(2); pending = 1; operations.clear();
  SendMessageW(host, WM_ACTIVATE, WA_ACTIVE, 0);
  pump([&] { return read(Form::status).find(message(L"待发送 1 个")) != std::string::npos; });
  assert((operations == std::vector<std::string>{"status"}) && read(Form::notice) == message(model::run_notice(true, false)));
  assert(SendMessageW(control(Form::devices), LB_GETCURSEL, 0, 0) == 2 && IsWindowEnabled(control(Form::remove)));
  pending = 0;
  // 被挡下的大批删除：两个选择都可用，「从网盘恢复」带 restore_deletions。
  held = 60; click(Form::sync_now);
  assert(IsWindowEnabled(control(Form::confirm_deletions)) && IsWindowEnabled(control(Form::restore_deletions)));
  assert(read(Form::held).find("60") != std::string::npos);
  click(Form::restore_deletions); assert(last_run.value("restore_deletions", false) && !IsWindowEnabled(control(Form::restore_deletions)));
  // 应用密码框留空：沿用已存的应用密码去测试和保存。
  set(Form::username, L"synthetic-b@example.test"); click(Form::save);
  assert(last_probe.at("password") == "synthetic-app-password" && preferences().at("dictionary_sync_webdav").at("username") == "synthetic-b@example.test");
  // 确认期间另一窗口换了配置，旧恢复不得清掉当前待发改动。
  operations.clear(); change_during_confirmation = true; click(Form::trust_remote);
  assert(operations.empty());
  assert(preferences().at("dictionary_sync_webdav").at("password") == "synthetic-other-password");
  // 停止同步：确认后网盘配置清掉，立即同步不可用。
  const auto before = confirms; click(Form::clear);
  assert(confirms == before + 1 && !preferences().contains("dictionary_sync_webdav") && !IsWindowEnabled(control(Form::sync_now)));
  DestroyWindow(host);
  // 网盘请求没有返回时关闭面板：界面不能等待它，返回后也不能再访问窗口。
  auto entered = std::make_shared<std::atomic<bool>>(false);
  auto finished = std::make_shared<std::atomic<bool>>(false);
  auto lifetime = std::shared_ptr<int>(new int(0), [finished](int* value) { delete value; finished->store(true); });
  std::promise<void> release;
  const auto released = release.get_future().share();
  DictionarySyncWindow::Services blocked_services;
  blocked_services.sync = [entered, lifetime, released](const std::string&) {
    entered->store(true);
    released.wait();
    return success(Json::object()).dump();
  };
  auto blocked = std::make_unique<DictionarySyncWindow>(blocked_services);
  blocked_services.sync = {};
  lifetime.reset();
  assert(blocked->create(GetModuleHandleW(nullptr)));
  const auto blocked_host = blocked->handle();
  const auto blocked_body = GetDlgItem(blocked_host, dictionary_sync_panel_body);
  pump([&] { return IsWindowEnabled(GetDlgItem(blocked_body, Form::test)) != FALSE; });
  assert(SetWindowTextW(GetDlgItem(blocked_body, Form::url), L"http://127.0.0.1:9/dav"));
  assert(SetWindowTextW(GetDlgItem(blocked_body, Form::username), L"synthetic@example.test"));
  assert(SetWindowTextW(GetDlgItem(blocked_body, Form::password), L"synthetic-app-password"));
  SendMessageW(GetDlgItem(blocked_body, Form::test), BM_CLICK, 0, 0);
  pump([&] { return entered->load(); });
  // 即使旧实现阻塞，辅助线程也会释放请求，让失败断言有机会执行。
  std::thread unblock([&] { std::this_thread::sleep_for(std::chrono::milliseconds(1500)); release.set_value(); });
  const auto closing = GetTickCount64();
  assert(DestroyWindow(blocked_host));
  const auto close_duration = GetTickCount64() - closing;
  blocked.reset();
  unblock.join();
  pump([&] { return finished->load(); });
  assert(close_duration < 1000);
  std::puts("Windows 7 dictionary sync panel: probe-gated saving, blank password keeps the saved one, opened sync with devices and status, status reread on activation, one entry per device, confirmed approvals, keystore conflict retry, held deletions and stopping sync passed");
}
