#include "VoiceWindow.h"
#include "PanelContext.h"
#include "../../settings/win7/PreferencesStore.h"
#include "../../settings/win7/StatisticsData.h"
#include "../../settings/win7/SurfaceLauncher.h"
#include "../../src/input/NativeTextInput.h"
#include "../../tsf/IPC/PassthroughStatistics.h"
#include "../../compat/WindowsDpi.h"
#include <commctrl.h>
#include <windowsx.h>
#include <condition_variable>
#include <mutex>
#include <optional>
#include <thread>
#include <utility>

namespace msime::windows::win7 {
namespace {
namespace shared = msime::settings::win7;
constexpr UINT updated_message = WM_APP + 81;
constexpr wchar_t window_class[] = L"MSIME.Windows7Voice" MSIME_EDITION_NAME_SUFFIX;
Json parse(std::string_view bytes) {
  return bytes.size() <= 128 * 1024 ? Json::parse(bytes,nullptr,false) : Json();
}
std::string response(char* raw) {
  const auto owned = msime::host_api::own_string(raw);
  return owned ? owned.get() : "";
}
void progress_callback(const uint8_t* bytes, size_t length, void* context) noexcept {
  try {
    if (bytes && length <= 128 * 1024)
      (*static_cast<const VoiceSessionApi::Progress*>(context))(std::string_view(reinterpret_cast<const char*>(bytes),length));
  } catch (...) { /* C ABI 回调不能向 Rust 展开异常。 */ }
}
VoiceSessionApi default_api() {
  return {
    [] { return response(msime_client_windows_voice_session_create()); },
    [](uint64_t handle, unsigned operation) { return response(msime_client_windows_voice_session_control(handle,operation)); },
    [](uint64_t handle, const std::string& language, const VoiceSessionApi::Progress& progress) {
      return response(msime_client_windows_voice_session_recognize(handle,reinterpret_cast<const uint8_t*>(language.data()),language.size(),
        progress_callback,const_cast<VoiceSessionApi::Progress*>(&progress)));
    }
  };
}
struct Completion { Json answer; bool statistics = false; };
struct Job { uint64_t handle = 0; std::optional<std::string> text; };
class Worker {
public:
  Worker(HWND host, VoiceSessionApi api) : host_(host), api_(std::move(api)), thread_([this] { run(); }) {}
  ~Worker() {
    { std::lock_guard<std::mutex> lock(mutex_); closing_ = true; close(); }
    ready_.notify_one(); thread_.join();
  }
  bool start() {
    std::lock_guard<std::mutex> lock(mutex_);
    try {
      const auto answer = parse(api_.create());
      const auto value = shared::theme_data::value(answer);
      if (!value || !value->is_object() || !value->contains("handle") || !value->at("handle").is_number_unsigned()) return false;
      handle_ = value->at("handle").get<uint64_t>(); if (!handle_) return false;
      job_ = Job{handle_,std::nullopt}; progress_.reset(); completed_.reset(); ready_.notify_one(); return true;
    } catch (...) { close(); return false; }
  }
  bool control(unsigned operation) {
    std::lock_guard<std::mutex> lock(mutex_);
    try { return handle_ && shared::theme_data::value(parse(api_.control(handle_,operation))).has_value(); }
    catch (...) { return false; }
  }
  void record(std::string text) {
    { std::lock_guard<std::mutex> lock(mutex_); job_ = Job{0,std::move(text)}; }
    ready_.notify_one();
  }
  std::pair<std::optional<Json>,std::optional<Completion>> take() {
    std::lock_guard<std::mutex> lock(mutex_);
    return {std::exchange(progress_,std::nullopt),std::exchange(completed_,std::nullopt)};
  }
private:
  void close() noexcept {
    const auto handle = std::exchange(handle_,0);
    try { if (handle) api_.control(handle,2); } catch (...) { /* 关闭不能因协议边界异常跳过线程收尾。 */ }
  }
  void progress(std::string_view bytes) {
    auto value = parse(bytes);
    if (!value.is_object()) return;
    std::lock_guard<std::mutex> lock(mutex_);
    if (!closing_) { progress_ = std::move(value); PostMessageW(host_,updated_message,0,0); }
  }
  Completion execute(const Job& job) {
    const auto context = panel_context();
    if (job.text) {
      SYSTEMTIME now{}; GetLocalTime(&now);
      return {shared::theme_data::call(msime_client_typing_statistics,{{"directory",context.directory.u8string()},{"action",{
        {"operation","record"},{"text",*job.text},{"source","voice"},{"hour",now.wHour},
        {"day",shared::statistics_data::model::day_key(shared::statistics_data::local_day())}}}}),true};
    }
    const auto loaded = shared::PreferencesStore(context.directory.u8string()).load();
    if (!loaded.snapshot) return {Json{{"error","voice_preferences_unavailable"}},false};
    const auto& preferences = loaded.snapshot->preferences();
    const auto& voice = preferences.at("voice_input");
    const auto language = voice.value("language",std::string("zh-CN"));
    return {parse(api_.recognize(job.handle,language,[this](std::string_view bytes) { progress(bytes); })),false};
  }
  void run() {
    for (;;) {
      std::unique_lock<std::mutex> lock(mutex_); ready_.wait(lock,[this] { return closing_ || job_.has_value(); });
      if (closing_) return;
      const auto job = std::move(*job_); job_.reset(); lock.unlock(); Completion result;
      try { result = execute(job); } catch (...) { result = {Json{{"error","voice_unavailable"}},job.text.has_value()}; }
      lock.lock(); if (job.handle) close();
      if (!closing_) { completed_ = std::move(result); PostMessageW(host_,updated_message,0,0); }
    }
  }
  HWND host_; VoiceSessionApi api_; std::mutex mutex_; std::condition_variable ready_;
  bool closing_ = false; uint64_t handle_ = 0;
  std::optional<Job> job_; std::optional<Json> progress_; std::optional<Completion> completed_; std::thread thread_;
};
bool copy_text(HWND owner, std::wstring_view text) {
  const auto memory = GlobalAlloc(GMEM_MOVEABLE,(text.size() + 1) * sizeof(wchar_t)); if (!memory) return false;
  auto* bytes = static_cast<wchar_t*>(GlobalLock(memory)); if (!bytes) { GlobalFree(memory); return false; }
  std::copy(text.begin(),text.end(),bytes); bytes[text.size()] = L'\0'; GlobalUnlock(memory);
  if (!OpenClipboard(owner)) { GlobalFree(memory); return false; }
  const bool copied = EmptyClipboard() && SetClipboardData(CF_UNICODETEXT,memory); CloseClipboard();
  if (!copied) GlobalFree(memory); return copied;
}
const wchar_t* error_text(const Json& answer) {
  if (!answer.is_object()) return L"未收到有效的识别结果，请重试。";
  const auto error = answer.contains("error") && answer.at("error").is_string() ? answer.at("error").get<std::string>() : std::string{};
  if (error == "voice_cancelled") return L"已取消；没有输入或复制文字。";
  if (error == "voice_busy") return L"另一次语音正在进行，请结束后重试。";
  if (error == "voice_denied") return L"无法连接当前登录会话的语音服务。";
  if (error == "voice_preferences_unavailable") return L"无法读取语音设置，原设置文件保留。";
  return L"语音未能完成，请检查输入法服务、麦克风和语音设置后重试。";
}
}
struct VoiceWindow::State {
  HWND host = nullptr; HINSTANCE instance = nullptr; HFONT font = nullptr; UINT dpi = USER_DEFAULT_SCREEN_DPI;
  bool quit = false, busy = false, recognizing = false, cancelled = false, stopping = false;
  msimeui::NativeTextInputTarget target = msimeui::NativeTextInputTarget::Capture();
  InsertText insert; CopyText copy; VoiceSessionApi api; OpenSettings settings; std::unique_ptr<Worker> worker;
  State(InsertText sink, CopyText clipboard, VoiceSessionApi protocol, OpenSettings open) : insert(std::move(sink)),copy(std::move(clipboard)),api(std::move(protocol)),settings(std::move(open)) {}
  ~State() { worker.reset(); if (host && IsWindow(host)) DestroyWindow(host); if (font) DeleteObject(font); }
  int px(int value) const { return MulDiv(value,static_cast<int>(dpi),USER_DEFAULT_SCREEN_DPI); }
  HWND get(int id) const { return GetDlgItem(host,id); }
  void notice(const wchar_t* text) { SetWindowTextW(get(voice_panel_status),text); }
  void enable() {
    EnableWindow(get(voice_panel_start),!busy); EnableWindow(get(voice_panel_settings),!busy);
    EnableWindow(get(voice_panel_stop),recognizing && !stopping && !cancelled);
    EnableWindow(get(voice_panel_cancel),recognizing && !cancelled);
    EnableWindow(get(voice_panel_result),!busy); SendMessageW(get(voice_panel_result),EM_SETREADONLY,busy,0);
    const bool text = !busy && GetWindowTextLengthW(get(voice_panel_result)) > 0;
    EnableWindow(get(voice_panel_copy),text); EnableWindow(get(voice_panel_insert),text);
    auto style = GetWindowLongPtrW(host,GWL_EXSTYLE);
    SetWindowLongPtrW(host,GWL_EXSTYLE,recognizing ? style | WS_EX_NOACTIVATE : style & ~WS_EX_NOACTIVATE);
  }
  void update() {
    auto updates = worker->take();
    if (updates.first && recognizing && !cancelled) {
      const auto& value = *updates.first;
      if (value.contains("phase") && value.at("phase").is_string()) {
        const auto phase = value.at("phase").get<std::string>();
        if (phase == "recording") notice(stopping ? L"正在结束录音…" : L"正在录音，完成后可编辑识别结果。");
        else if (phase == "recognizing") { stopping = true; notice(L"正在识别…"); }
        else if (phase == "processing") { stopping = true; notice(L"正在润色…"); }
      }
      if (value.contains("level") && value.at("level").is_number()) {
        const auto level = value.at("level").get<double>();
        if (level >= 0 && level <= 1) SendMessageW(get(voice_panel_level),PBM_SETPOS,static_cast<WPARAM>(level * 100),0);
      }
      if (value.contains("text") && value.at("text").is_string()) {
        const auto text = value.at("text").get<std::string>();
        if (text.size() <= 65536 && text.find('\0') == std::string::npos) SetWindowTextW(get(voice_panel_result),shared::control_text::wide(text).c_str());
      }
    }
    if (updates.second) {
      busy = false; recognizing = false; SendMessageW(get(voice_panel_level),PBM_SETPOS,0,0);
      if (updates.second->statistics) {
        notice(shared::theme_data::value(updates.second->answer) ? L"已输入原编辑器。" : L"已输入原编辑器，但统计未能保存；不会重复输入。");
      } else {
        const auto value = shared::theme_data::value(updates.second->answer);
        if (!cancelled && value && value->is_object() && value->contains("text") && value->at("text").is_string()) {
          const auto text = value->at("text").get<std::string>(); const auto wide = shared::control_text::wide(text);
          if (text.size() <= 65536 && text.find('\0') == std::string::npos && (text.empty() || !wide.empty())) {
            SetWindowTextW(get(voice_panel_result),wide.c_str()); notice(text.empty() ? L"没有识别到文字，可以重新录音。" : L"识别完成；可修改文字后复制或输入原编辑器。");
          } else { SetWindowTextW(get(voice_panel_result),L""); notice(L"识别结果格式无效，未输入或复制文字。"); }
        } else {
          SetWindowTextW(get(voice_panel_result),L""); notice(cancelled ? L"已取消；没有输入或复制文字。" : error_text(updates.second->answer));
        }
      }
    }
    enable();
  }
  void command(int id, int notification) {
    if (id == voice_panel_result && notification == EN_CHANGE) { enable(); return; }
    if (notification != BN_CLICKED || !IsWindowEnabled(get(id))) return;
    if (id == voice_panel_start) {
      if (!worker->start()) { notice(L"无法创建语音会话，请关闭多余的语音面板后重试。"); return; }
      busy = true; recognizing = true; cancelled = false; stopping = false;
      SetWindowTextW(get(voice_panel_result),L""); notice(L"正在连接输入法语音服务…"); enable(); return;
    }
    if (id == voice_panel_stop || id == voice_panel_cancel) {
      const bool cancel = id == voice_panel_cancel;
      // 即使工作线程已经关闭句柄、完成消息尚未送达，用户的取消仍使迟到结果失效。
      if (cancel) cancelled = true;
      const bool accepted = worker->control(cancel ? 1 : 0);
      if (!accepted && !cancel) { notice(L"语音状态正在变化，请稍候。" ); return; }
      if (!cancel) stopping = true;
      notice(cancel ? L"正在取消…" : L"正在结束录音…"); enable(); return;
    }
    if (id == voice_panel_settings) {
      const bool opened = settings ? settings() : shared::launch_surface("settings:voice");
      notice(opened ? L"已打开语音设置。" : L"无法打开语音设置，请重试。"); return;
    }
    if (id != voice_panel_insert && id != voice_panel_copy) return;
    const auto text = shared::control_text::read(get(voice_panel_result),65536); if (!text || text->empty()) return;
    const auto wide = shared::control_text::wide(*text);
    if (id == voice_panel_copy) {
      notice((copy ? copy(host,wide) : copy_text(host,wide)) ? L"已复制。" : L"复制失败，请稍后重试。"); return;
    }
    if (wide.size() > 4000) { notice(L"文字超过单次输入上限，请分段输入或复制。" ); return; }
    if (!(insert ? insert(wide) : target.InsertFromPanel(wide,host,PanelTextSendInputExtraInfo))) {
      notice(L"未能输入：请保持原编辑器或本面板在前台后重试。" ); return;
    }
    busy = true; enable(); worker->record(*text);
  }
  void layout() {
    if (!get(voice_panel_start)) return;
    RECT area{}; GetClientRect(host,&area); const int width = std::max(px(100),static_cast<int>(area.right) - px(24));
    for (const auto& pair : std::vector<std::pair<int,int>>{{voice_panel_start,12},{voice_panel_stop,140},{voice_panel_cancel,268},{voice_panel_settings,396}})
      MoveWindow(get(pair.first),px(pair.second),px(12),px(116),px(32),TRUE);
    MoveWindow(get(voice_panel_level),px(12),px(56),width,px(14),TRUE);
    const auto height = std::max(px(60),static_cast<int>(area.bottom) - px(206));
    MoveWindow(get(voice_panel_result),px(12),px(82),width,height,TRUE);
    MoveWindow(get(voice_panel_insert),px(12),px(94) + height,px(148),px(32),TRUE);
    MoveWindow(get(voice_panel_copy),px(172),px(94) + height,px(116),px(32),TRUE);
    MoveWindow(get(voice_panel_status),px(12),px(138) + height,width,px(56),TRUE);
  }
  static LRESULT CALLBACK button_proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    const auto previous = reinterpret_cast<WNDPROC>(GetPropW(window,L"MSIME.Win7.Voice.ButtonProc"));
    // 鼠标路径保留前台编辑器；录音结束后仍可用键盘导航和编辑结果。
    switch (message) {
    case WM_MOUSEACTIVATE: return MA_NOACTIVATE;
    case WM_LBUTTONDOWN: case WM_LBUTTONDBLCLK: if (IsWindowEnabled(window)) { SetCapture(window); SendMessageW(window,BM_SETSTATE,TRUE,0); } return 0;
    case WM_LBUTTONUP:
      if (GetCapture() == window) {
        RECT area{}; GetClientRect(window,&area); const POINT point{GET_X_LPARAM(lparam),GET_Y_LPARAM(lparam)};
        ReleaseCapture(); SendMessageW(window,BM_SETSTATE,FALSE,0);
        if (PtInRect(&area,point) && IsWindowEnabled(window)) SendMessageW(GetParent(window),WM_COMMAND,MAKEWPARAM(GetDlgCtrlID(window),BN_CLICKED),reinterpret_cast<LPARAM>(window));
      } return 0;
    case WM_CANCELMODE: if (GetCapture() == window) ReleaseCapture(); SendMessageW(window,BM_SETSTATE,FALSE,0); return 0;
    case WM_CAPTURECHANGED: SendMessageW(window,BM_SETSTATE,FALSE,0); return 0;
    case WM_NCDESTROY: RemovePropW(window,L"MSIME.Win7.Voice.ButtonProc"); break;
    }
    return previous ? CallWindowProcW(previous,window,message,wparam,lparam) : DefWindowProcW(window,message,wparam,lparam);
  }
  static LRESULT CALLBACK proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    auto* state = reinterpret_cast<State*>(GetWindowLongPtrW(window,GWLP_USERDATA));
    if (message == WM_NCCREATE) { state = static_cast<State*>(reinterpret_cast<CREATESTRUCTW*>(lparam)->lpCreateParams); SetWindowLongPtrW(window,GWLP_USERDATA,reinterpret_cast<LONG_PTR>(state)); state->host = window; }
    if (!state) return DefWindowProcW(window,message,wparam,lparam);
    switch (message) {
    case WM_MOUSEACTIVATE: if (state->recognizing) return MA_NOACTIVATE; break;
    case WM_SIZE: state->layout(); return 0;
    case WM_COMMAND: state->command(LOWORD(wparam),HIWORD(wparam)); return 0;
    case updated_message: if (state->worker) state->update(); return 0;
    case WM_CLOSE: DestroyWindow(window); return 0;
    case WM_DESTROY: state->worker.reset(); if (state->quit) PostQuitMessage(0); return 0;
    case WM_NCDESTROY: state->host = nullptr; SetWindowLongPtrW(window,GWLP_USERDATA,0); break;
    }
    return DefWindowProcW(window,message,wparam,lparam);
  }
};
VoiceWindow::VoiceWindow(InsertText insert, CopyText copy, VoiceSessionApi api, OpenSettings settings) : state_(std::make_unique<State>(std::move(insert),std::move(copy),std::move(api),std::move(settings))) {}
VoiceWindow::~VoiceWindow() = default;
HWND VoiceWindow::handle() const { return state_->host; }
bool VoiceWindow::create(HINSTANCE instance) {
  auto& state = *state_; state.instance = instance; state.target = msimeui::NativeTextInputTarget::Capture(); state.dpi = compat::system_dpi();
  const bool any_api = state.api.create || state.api.control || state.api.recognize;
  if (any_api && !(state.api.create && state.api.control && state.api.recognize)) return false;
  if (!any_api) state.api = default_api();
  INITCOMMONCONTROLSEX common{sizeof(common),ICC_PROGRESS_CLASS}; if (!InitCommonControlsEx(&common)) return false;
  WNDCLASSEXW type{sizeof(type)}; type.hInstance = instance; type.lpfnWndProc = State::proc; type.lpszClassName = window_class;
  type.hCursor = LoadCursorW(nullptr,IDC_ARROW); type.hbrBackground = GetSysColorBrush(COLOR_BTNFACE);
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
  const auto frame = panel_frame(550,460,state.dpi);
  state.host = CreateWindowExW(WS_EX_TOOLWINDOW,window_class,L"语音输入",WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
    frame.left,frame.top,frame.right - frame.left,frame.bottom - frame.top,nullptr,nullptr,instance,&state);
  if (!state.host) return false;
  state.font = CreateFontW(-state.px(14),0,0,0,FW_NORMAL,FALSE,FALSE,FALSE,DEFAULT_CHARSET,OUT_DEFAULT_PRECIS,CLIP_DEFAULT_PRECIS,DEFAULT_QUALITY,DEFAULT_PITCH,L"Segoe UI");
  for (const auto& field : std::vector<std::pair<int,const wchar_t*>>{{voice_panel_start,L"开始录音"},{voice_panel_stop,L"完成录音"},{voice_panel_cancel,L"取消"},
      {voice_panel_settings,L"语音设置"},{voice_panel_result,L""},{voice_panel_level,L""},{voice_panel_status,L"点击开始录音，完成后可编辑结果。"},{voice_panel_insert,L"输入原编辑器"},{voice_panel_copy,L"复制"}}) {
    const auto kind = field.first == voice_panel_result ? L"EDIT" : field.first == voice_panel_level ? PROGRESS_CLASSW : field.first == voice_panel_status ? L"STATIC" : L"BUTTON";
    DWORD style = WS_CHILD | WS_VISIBLE;
    if (field.first == voice_panel_result) style |= WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN | WS_VSCROLL;
    else if (std::wstring_view(kind) == L"BUTTON") style |= WS_TABSTOP | BS_PUSHBUTTON;
    const auto control = CreateWindowExW(0,kind,field.second,style,0,0,1,1,state.host,reinterpret_cast<HMENU>(static_cast<INT_PTR>(field.first)),instance,nullptr);
    if (!control) return false;
    SendMessageW(control,WM_SETFONT,reinterpret_cast<WPARAM>(state.font),FALSE);
    if (std::wstring_view(kind) == L"BUTTON") {
      const auto previous = GetWindowLongPtrW(control,GWLP_WNDPROC);
      if (!previous || !SetPropW(control,L"MSIME.Win7.Voice.ButtonProc",reinterpret_cast<HANDLE>(previous))) return false;
      if (!SetWindowLongPtrW(control,GWLP_WNDPROC,reinterpret_cast<LONG_PTR>(State::button_proc))) { RemovePropW(control,L"MSIME.Win7.Voice.ButtonProc"); return false; }
    }
  }
  SendMessageW(state.get(voice_panel_result),EM_SETLIMITTEXT,65536,0);
  state.worker = std::make_unique<Worker>(state.host,std::move(state.api)); state.layout(); state.enable(); return true;
}
int VoiceWindow::run(int show) {
  state_->quit = true; ShowWindow(state_->host,show == SW_HIDE ? show : SW_SHOWNOACTIVATE); MSG message{}; BOOL result;
  while ((result = GetMessageW(&message,nullptr,0,0)) > 0) { if (!IsDialogMessageW(state_->host,&message)) { TranslateMessage(&message); DispatchMessageW(&message); } }
  return result == -1 ? 1 : static_cast<int>(message.wParam);
}
}
