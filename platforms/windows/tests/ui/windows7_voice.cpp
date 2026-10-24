#include "../../settings/win7/SettingsWindow.h"
#include "../../settings/win7/VoiceForm.h"
#include "../../settings/win7/VoiceModelsForm.h"
#include <atomic>
#include <cassert>
#include <cstdio>
#include <mutex>

namespace {
using namespace msime::settings::win7;
using Form = VoiceForm;
using Models = VoiceModelsForm;
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
struct CancelContext { std::string id; bool called = false; };
void cancel_registered(const uint8_t*, size_t, void* raw) {
  auto& context = *static_cast<CancelContext*>(raw); context.called = true;
  assert(theme_data::value(theme_data::call(msime_client_voice_local_model_cancel, {{"id",context.id}})));
}
}
int main() {
  wchar_t temp[MAX_PATH]{}, unique[MAX_PATH]{}; assert(GetTempPathW(MAX_PATH, temp) && GetTempFileNameW(temp, L"m7v", 0, unique));
  assert(DeleteFileW(unique) && CreateDirectoryW(unique, nullptr)); const std::filesystem::path directory(unique);
  const auto size = GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", nullptr, 0); std::wstring previous(size, L'\0');
  if (size) previous.resize(GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", previous.data(), size));
  assert(SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", directory.c_str()));
  {
    PreferencesStore store(directory.u8string()); const auto initial = store.load(); assert(initial.snapshot);
    // 保留旧文档同时打开的长按键和两个润色字段差异；打开、保存其他字段不能顺手规范化。
    const auto seed = store.save(*initial.snapshot, {{{"voice_input","hotkey_rctrl_ralt"},true},
      {{"voice_input","polish_enabled"},false}, {{"voice_input","polish_text"},true}}); assert(seed.snapshot);
    const auto before = *seed.snapshot;
    const auto providers = theme_data::value(theme_data::response(msime_client_voice_provider_catalog())).value();
    const auto provider_index = [&](const char* section, const char* id) { const auto& list = providers.at(section); for (size_t index = 0; index < list.size(); ++index) if (list[index].at("id") == id) return static_cast<int>(index); assert(false); return 0; };
    const auto catalog = theme_data::call(msime_client_voice_local_models, {{"root",(directory / L"models").u8string()}});
    assert(theme_data::value(catalog) && !catalog.at("value").at("models").empty());
    Json model = catalog.at("value").at("models").front(); model["path"] = (directory / L"models/synthetic").u8string();
    std::mutex mutex; std::vector<Json> requests; bool malformed = false, installed = false, allow = false, picker_cancel = false;
    std::atomic<bool> block_install{false}, install_started{false}, malformed_devices{false}; HANDLE gate = CreateEventW(nullptr, TRUE, FALSE, nullptr); assert(gate);
    const DWORD ui_thread = GetCurrentThreadId(); unsigned picks = 0;
    const auto transport = [&](const std::string& bytes) {
      const auto request = Json::parse(bytes); const auto& config = request.at("config");
      const auto service = request.at("service").get<std::string>(), operation = config.value("operation", std::string{});
      if (operation != "cancel") assert(GetCurrentThreadId() != ui_thread);
      { std::lock_guard<std::mutex> lock(mutex); requests.push_back(request); }
      if (operation == "cancel") { assert(SetEvent(gate)); return Json{{"ok",true},{"value",true}}.dump(); }
      if (service != "voice.settings") return Json{{"ok",true},{"value",{{"message","合成配置测试成功"}}}}.dump();
      if (operation == "capture_devices" && malformed_devices) return Json{{"ok",true},{"value",{Json{{"label",1}}}}}.dump();
      if (operation == "capture_devices") return Json{{"ok",true},{"value",{{{"backend","windows"},{"id","synthetic-device"},{"label","合成麦克风🙂"}}}}}.dump();
      if (operation == "install" && block_install) { install_started = true; assert(WaitForSingleObject(gate, 10000) == WAIT_OBJECT_0); return Json{{"ok",false},{"error","local_model_cancelled"}}.dump(); }
      std::lock_guard<std::mutex> lock(mutex);
      if (operation == "install") installed = true;
      if (operation == "remove") installed = false;
      model["installed"] = installed;
      const Json listing{{"ok",true},{"value",{{"models",{model}},{"default",model.at("id")}}}};
      if (malformed) return Json{{"ok",true},{"value",{{"models",1}}}}.dump();
      if (operation == "models") return listing.dump();
      return Json{{"ok",true},{"value",operation == "install" ? Json{{"path",model.at("path")}} : Json()}, {"catalog",listing}}.dump();
    };
    const auto last = [&] { std::lock_guard<std::mutex> lock(mutex); assert(!requests.empty()); return requests.back(); };
    const auto count = [&] { std::lock_guard<std::mutex> lock(mutex); return requests.size(); };
    SettingsWindow window([&](HWND, bool save) -> std::optional<std::filesystem::path> { assert(!save); ++picks; return picker_cancel ? std::nullopt : std::make_optional(directory / L"synthetic-model.tar.bz2"); },
      [&](HWND, const wchar_t* title, const wchar_t* message) { assert(std::wstring_view(title) == L"本地语音模型" && message && *message); return allow; },
      [&](HWND) { return std::make_optional(directory / L"models/custom"); }, {}, {}, {}, {}, {}, {}, {}, transport);
    assert(window.open_route("settings:voice") && window.create(GetModuleHandleW(nullptr)));
    const auto host = window.handle(), body = GetDlgItem(host,page_body), selector = GetDlgItem(host,page_selector);
    const auto control = [&](int id) { const auto value = GetDlgItem(body,id); assert(value); return value; };
    const auto wait = [&] { pump([&] { return SendMessageW(selector,CB_GETCURSEL,0,0) == 19 && IsWindowEnabled(GetDlgItem(body,language_switch)); }); }; wait();
    const auto click = [&](int id) { assert(IsWindowEnabled(control(id))); SendMessageW(control(id),BM_CLICK,0,0); wait(); };
    const auto choose = [&](int id, int index) { SendMessageW(control(id),CB_SETCURSEL,index,0); SendMessageW(body,WM_COMMAND,MAKEWPARAM(id,CBN_SELCHANGE),reinterpret_cast<LPARAM>(control(id))); wait(); };
    const auto set = [&](int id, const wchar_t* value) { assert(SetWindowTextW(control(id),value)); };
    const auto text = [&](int id) { return control_text::read(control(id)).value(); };
    const auto voice = [&] { return store.load().snapshot->preferences().at("voice_input"); };
    assert(store.load().snapshot->revision() == before.revision()); click(Form::save); assert(store.load().snapshot->revision() == before.revision());
    set(Form::language,L"en-US"); click(Form::save); assert(voice().at("language") == "en-US" && voice().at("hotkey_rctrl_ralt") == true && voice().at("polish_enabled") == false && voice().at("polish_text") == true);
    choose(Form::asr_provider,provider_index("asr","openai")); set(Form::asr_token,L"synthetic-openai-key"); click(Form::save);
    assert(voice().at("asr_tokens").at("openai") == "synthetic-openai-key");
    choose(Form::asr_provider,provider_index("asr","siliconflow")); assert(text(Form::asr_token).empty());
    set(Form::asr_token,L"synthetic-siliconflow-key"); click(Form::save);
    choose(Form::asr_provider,provider_index("asr","openai")); assert(text(Form::asr_token) == "synthetic-openai-key");
    click(Form::test_asr); assert(last().at("service") == "voice.asr" && last().at("config").at("token") == "synthetic-openai-key" && text(Form::test_result).find("成功") != std::string::npos);
    set(Form::asr_model,L"synthetic-model"); assert(text(Form::test_result).empty());
    const auto invalid_before = count(); const auto revision = store.load().snapshot->revision(); set(Form::asr_endpoint,L"http://invalid.example.test"); click(Form::test_asr); click(Form::save);
    assert(count() == invalid_before && store.load().snapshot->revision() == revision);
    set(Form::asr_endpoint,L"https://synthetic.example.test/audio"); click(Form::save);
    choose(Form::hold_key,2); click(Form::save); assert(voice().at("hotkey_ctrl_win") == true && voice().at("hotkey_ralt") == false && voice().at("hotkey_rctrl_ralt") == false);
    click(Form::device_refresh); assert(SendMessageW(control(Form::devices),CB_GETCOUNT,0,0) == 2); choose(Form::devices,1);
    assert(voice().at("capture_device") == "synthetic-device" && voice().at("capture_backend") == "windows");
    assert(text(Models::detail).find("语言：") != std::string::npos && text(Models::detail).find("流式识别：") != std::string::npos);
    malformed_devices = true; click(Form::device_refresh);
    assert(text(Form::device_status).find("失败") != std::string::npos && SendMessageW(control(Form::devices),CB_GETCOUNT,0,0) == 2 && voice().at("capture_device") == "synthetic-device");
    malformed_devices = false;
    const auto before_cancel = count(); click(Models::install); assert(count() == before_cancel);
    picker_cancel = true; click(Models::add_file); assert(picks == 1 && text(Models::files).empty());
    picker_cancel = false; click(Models::add_file); assert(picks == 2 && !text(Models::files).empty());
    allow = true; click(Models::import_model); assert(last().at("config").at("files").size() == 1 && voice().at("asr_model_path") == model.at("path"));
    click(Models::use); assert(voice().at("asr_provider") == "local" && !IsWindowEnabled(control(Form::test_asr)));
    allow = false; const auto before_remove = count(); click(Models::remove); assert(count() == before_remove);
    allow = true; click(Models::remove); assert(voice().at("asr_model_path") == "");
    block_install = true; assert(ResetEvent(gate)); SendMessageW(control(Models::install),BM_CLICK,0,0);
    pump([&] { return install_started.load(); }); assert(IsWindowEnabled(control(Models::cancel)) && !IsWindowEnabled(control(Models::install)));
    SendMessageW(control(Models::cancel),BM_CLICK,0,0); wait(); block_install = false;
    assert(!IsWindowEnabled(control(Models::install)) && text(Models::notice).find("取消") != std::string::npos);
    click(Models::refresh); click(Models::choose_directory); assert(voice().at("asr_model_path") == (directory / L"models/custom").u8string());
    { std::lock_guard<std::mutex> lock(mutex); malformed = true; } click(Models::refresh);
    assert(!IsWindowEnabled(control(Models::install)) && !IsWindowEnabled(control(Models::use)));
    // 直接用共享安装入口验证注册后的首次回调可取消，文件和网络都不必开始。
    const auto shared_request = Json{{"root",(directory / L"cancel-models").u8string()}, {"id",model.at("id")}, {"files",{(directory / L"missing-model.tar.bz2").u8string()}}}.dump();
    CancelContext context{model.at("id").get<std::string>(),false};
    const auto cancelled = theme_data::response(msime_client_voice_local_model_install(reinterpret_cast<const uint8_t*>(shared_request.data()),shared_request.size(),cancel_registered,&context));
    assert(context.called && cancelled.at("ok") == false && cancelled.at("error") == "local_model_cancelled");
    DestroyWindow(host); assert(CloseHandle(gate));
  }
  assert(SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR",size ? previous.c_str() : nullptr)); std::error_code error; std::filesystem::remove_all(directory,error);
  std::puts("Windows 7 voice: native settings, shared provider isolation and validation, preserved old settings, model requests and cancellation passed");
}
