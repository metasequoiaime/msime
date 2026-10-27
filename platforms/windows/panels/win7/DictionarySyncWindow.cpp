#include "DictionarySyncWindow.h"
#include "PanelContext.h"
#include "../../common/DictionarySyncDirectory.h"
#include "../../settings/win7/DictionarySyncForm.h"
#include "../../compat/WindowsDpi.h"
#include <commctrl.h>
#include <windowsx.h>
#include <condition_variable>
#include <mutex>
#include <thread>
#include <utility>

namespace msime::windows::win7 {
namespace {
namespace shared = msime::settings::win7;
constexpr UINT completed_message = WM_APP + 79;
constexpr int notice_id = 401;
constexpr wchar_t window_class[] = L"MSIME.Windows7DictionarySync" MSIME_EDITION_NAME_SUFFIX;
constexpr wchar_t body_class[] = L"MSIME.Windows7DictionarySync.Body" MSIME_EDITION_NAME_SUFFIX;
// 一件事：request 是表单给的 {probe, save} 或 {action, devices?, status?}，changes 是要写进偏好的网盘配置。都为空时只读一次偏好。
struct Job { std::optional<Json> request; std::vector<shared::Change> changes; };
struct Completion { shared::Result preferences; std::optional<Json> response; bool unprepared = false; Job job; };
struct WorkerState {
  WorkerState(HWND window, DictionarySyncWindow::Services services) : window_(window), services_(std::move(services)) {}
  void stop() {
    { std::lock_guard<std::mutex> guard(mutex_); stopping_ = true; window_ = nullptr; pending_.reset(); }
    ready_.notify_one();
  }
  void submit(Job job) { { std::lock_guard<std::mutex> guard(mutex_); pending_ = std::move(job); } ready_.notify_one(); }
  std::optional<Completion> take() { std::lock_guard<std::mutex> guard(mutex_); return std::exchange(completed_, std::nullopt); }
  Completion execute(Job job) {
    const auto context = panel_context(); shared::PreferencesStore store(context.directory.u8string());
    // 本用户的同步目录，与 Server 和 WinUI 设置同一个（DictionarySyncDirectory.h）：一个安装只有一份设备身份。
    const auto directory = msime::windows::dictionary_sync_directory().u8string();
    const auto options = context.options.is_object() ? context.options : Json();
    // 「测试连接」与云剪贴板面板相同：先测试，通过了才写配置；不通过时丢掉要写的配置，只读回当前设置。
    std::optional<Json> probe;
    if (job.request && job.request->contains("probe")) {
      auto action = job.request->at("probe"); action["operation"] = "probe";
      const auto response = shared::call_dictionary_sync(action, options, directory, services_);
      if (!shared::theme_data::value(response)) job.changes.clear();
      probe = Json{{"probe", response}};
    }
    const bool write = !job.changes.empty();
    auto preferences = !write ? store.load() : snapshot_ ? store.save(*snapshot_, job.changes) : shared::Result{};
    if (!preferences.snapshot) preferences.snapshot = store.load().snapshot;
    snapshot_ = preferences.snapshot;
    if (probe) {
      // 配置和已存的相同时没有要写的，测试通过就算保存过了。
      (*probe)["saved"] = shared::theme_data::value(probe->at("probe")).has_value() && (!write || preferences.outcome == shared::Outcome::saved);
      return Completion{std::move(preferences), std::move(probe), false, std::move(job)};
    }
    Completion result{std::move(preferences), std::nullopt, false, std::move(job)};
    if (!result.job.request) return result;
    const auto action = result.job.request->value("action", Json::object());
    if (action.value("operation", std::string{}) == "probe" && action.value("trust_remote", false)) {
      Json saved = action;
      saved.erase("operation"); saved.erase("trust_remote");
      if (!snapshot_ || snapshot_->preferences().value("dictionary_sync_webdav", Json::object()) != saved) {
        result.response = Json{{"action", {{"ok", false}, {"error", "dictionary_sync_not_configured"}}}};
        return result;
      }
    }
    if (!options.is_object()) result.unprepared = true;
    else result.response = shared::run_dictionary_sync(*result.job.request, options, directory, services_);
    return result;
  }
  void run() {
    for (;;) {
      std::unique_lock<std::mutex> guard(mutex_); ready_.wait(guard, [this] { return stopping_ || pending_; });
      if (stopping_) return;
      auto job = std::move(*pending_); pending_.reset(); guard.unlock();
      Completion result;
      try { result = execute(std::move(job)); } catch (const std::exception&) { result.preferences = {}; }
      guard.lock(); if (!stopping_) { completed_ = std::move(result); PostMessageW(window_, completed_message, 0, 0); }
    }
  }
  HWND window_;
  DictionarySyncWindow::Services services_;
  std::mutex mutex_;
  std::condition_variable ready_;
  bool stopping_ = false;
  std::optional<Job> pending_;
  std::optional<Completion> completed_;
  std::optional<shared::Snapshot> snapshot_;
};
// 网络调用不能取消，也不能在 WM_DESTROY 上等待。共享状态持有服务和快照，窗口关闭后工作线程只收尾，不再投递消息。
class Worker {
public:
  Worker(HWND window, DictionarySyncWindow::Services services)
      : state_(std::make_shared<WorkerState>(window, std::move(services))),
        thread_([state = state_] { state->run(); }) {}
  ~Worker() { state_->stop(); thread_.detach(); }
  void submit(Job job) { state_->submit(std::move(job)); }
  std::optional<Completion> take() { return state_->take(); }
private:
  std::shared_ptr<WorkerState> state_;
  std::thread thread_;
};
} // namespace
struct DictionarySyncWindow::State {
  struct Item { HWND handle; int top, height; };
  HWND host = nullptr, body = nullptr, notice = nullptr;
  HINSTANCE instance = nullptr; HFONT font = nullptr; UINT dpi = USER_DEFAULT_SCREEN_DPI;
  int scroll = 0, height = 0;
  bool busy = true, quit = false, opened = false;
  std::vector<Item> items;
  shared::DictionarySyncForm form;
  Services services; Confirmation confirmation;
  std::unique_ptr<Worker> worker;
  State(Services api, Confirmation confirm) : services(std::move(api)), confirmation(std::move(confirm)) {}
  ~State() { worker.reset(); if (host && IsWindow(host)) DestroyWindow(host); if (font) DeleteObject(font); }
  int px(int value) const { return MulDiv(value, static_cast<int>(dpi), USER_DEFAULT_SCREEN_DPI); }
  void submit(Job job) { if (busy) return; busy = true; form.enable(false); SetWindowTextW(notice, L"正在处理…"); worker->submit(std::move(job)); }
  void complete() {
    auto result = worker->take(); if (!result) return; busy = false;
    const auto outcome = result->preferences.outcome;
    SetWindowTextW(notice, outcome == shared::Outcome::loaded ? L"" : outcome == shared::Outcome::saved ? L"已保存。" : outcome == shared::Outcome::conflict ? L"设置已在其他窗口更新，本次未覆盖，请重新填写。" : L"设置无法读取或保存，请重试。");
    if (result->preferences.snapshot) form.refresh(*result->preferences.snapshot);
    form.enable(result->preferences.snapshot.has_value());
    if (!result->preferences.snapshot) return;
    if (result->unprepared) { opened = true; form.unprepared(); return; }
    const bool probed = result->job.request && result->job.request->contains("probe");
    if (probed && result->response) form.probed(*result->job.request, *result->response);
    else if (result->response && result->job.request) form.completed(*result->job.request, *result->response);
    // 打开面板时，以及刚保存了网盘配置之后，存着网盘就以 opened 同步一次。
    const bool saved = probed && result->response && result->response->value("saved", false) && result->job.request->value("save", false);
    if ((!opened || saved) && form.configured()) {
      opened = true;
      if (const auto request = form.opened_request()) submit({request, {}});
    }
    opened = true;
  }
  void command(WPARAM event) {
    if (busy) return;
    auto action = form.command(event); if (!action.handled) return;
    if (!action.confirmation.empty()) {
      const bool allowed = confirmation ? confirmation(host, L"词库同步", action.confirmation.c_str()) : MessageBoxW(host, action.confirmation.c_str(), L"词库同步", MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2) == IDYES;
      if (!allowed) return;
    }
    if (action.request || !action.changes.empty()) submit({std::move(action.request), std::move(action.changes)});
  }
  void layout() {
    RECT client{}; GetClientRect(host, &client);
    MoveWindow(body, px(12), px(12), std::max(1L, client.right - px(24)), std::max(1L, client.bottom - px(64)), TRUE);
    MoveWindow(notice, px(12), client.bottom - px(42), std::max(1L, client.right - px(24)), px(36), TRUE);
    GetClientRect(body, &client);
    SCROLLINFO info{sizeof(info)}; info.fMask = SIF_RANGE | SIF_PAGE | SIF_POS; info.nMax = px(height) - 1; info.nPage = std::max(1L, client.bottom); info.nPos = scroll;
    SetScrollInfo(body, SB_VERT, &info, TRUE); info.fMask = SIF_POS; GetScrollInfo(body, SB_VERT, &info); scroll = info.nPos;
    GetClientRect(body, &client); for (const auto& item : items) MoveWindow(item.handle, 0, px(item.top) - scroll, std::max(1L, client.right - px(8)), px(item.height), TRUE);
  }
  static LRESULT CALLBACK body_proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    auto* state = reinterpret_cast<State*>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) { state = static_cast<State*>(reinterpret_cast<CREATESTRUCTW*>(lparam)->lpCreateParams); SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(state)); }
    if (!state) return DefWindowProcW(window, message, wparam, lparam);
    if (message == WM_COMMAND) {
      if (HIWORD(wparam) == EN_SETFOCUS || HIWORD(wparam) == BN_SETFOCUS || HIWORD(wparam) == LBN_SETFOCUS) {
        const auto control = reinterpret_cast<HWND>(lparam); RECT bounds{}, client{};
        if (control && GetWindowRect(control, &bounds)) {
          MapWindowPoints(nullptr, window, reinterpret_cast<POINT*>(&bounds), 2); GetClientRect(window, &client);
          if (bounds.top < 0) state->scroll = std::max(0, state->scroll + static_cast<int>(bounds.top));
          else if (bounds.bottom > client.bottom) state->scroll += bounds.bottom - client.bottom;
          state->layout();
        }
      }
      state->command(wparam); return 0;
    }
    if (message == WM_VSCROLL || message == WM_MOUSEWHEEL) {
      int delta = 0;
      if (message == WM_MOUSEWHEEL) delta = -GET_WHEEL_DELTA_WPARAM(wparam) / WHEEL_DELTA * state->px(72);
      else {
        SCROLLINFO info{sizeof(info)}; info.fMask = SIF_ALL;
        if (!GetScrollInfo(window, SB_VERT, &info)) return 0;
        switch (LOWORD(wparam)) {
        case SB_TOP: state->scroll = info.nMin; break;
        case SB_BOTTOM: state->scroll = info.nMax; break;
        case SB_LINEUP: delta = -state->px(36); break;
        case SB_LINEDOWN: delta = state->px(36); break;
        case SB_PAGEUP: delta = -static_cast<int>(info.nPage); break;
        case SB_PAGEDOWN: delta = static_cast<int>(info.nPage); break;
        case SB_THUMBPOSITION:
        case SB_THUMBTRACK: state->scroll = info.nTrackPos; break;
        default: return 0;
        }
      }
      state->scroll = std::max(0, state->scroll + delta); state->layout(); return 0;
    }
    return DefWindowProcW(window, message, wparam, lparam);
  }
  static LRESULT CALLBACK proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    auto* state = reinterpret_cast<State*>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) { state = static_cast<State*>(reinterpret_cast<CREATESTRUCTW*>(lparam)->lpCreateParams); SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(state)); state->host = window; }
    if (!state) return DefWindowProcW(window, message, wparam, lparam);
    switch (message) {
    case WM_SIZE: if (state->body) state->layout(); return 0;
    case WM_COMMAND: state->command(wparam); return 0;
    case completed_message: state->complete(); return 0;
    case WM_CLOSE: DestroyWindow(window); return 0;
    case WM_DESTROY: state->worker.reset(); if (state->quit) PostQuitMessage(0); return 0;
    case WM_NCDESTROY: state->host = nullptr; SetWindowLongPtrW(window, GWLP_USERDATA, 0); break;
    }
    return DefWindowProcW(window, message, wparam, lparam);
  }
};
DictionarySyncWindow::DictionarySyncWindow(Services services, Confirmation confirmation) : state_(std::make_unique<State>(std::move(services), std::move(confirmation))) {}
DictionarySyncWindow::~DictionarySyncWindow() = default;
HWND DictionarySyncWindow::handle() const { return state_->host; }
bool DictionarySyncWindow::create(HINSTANCE instance) {
  auto& state = *state_; state.instance = instance; state.dpi = compat::system_dpi();
  INITCOMMONCONTROLSEX common{sizeof(common), ICC_STANDARD_CLASSES}; if (!InitCommonControlsEx(&common)) return false;
  WNDCLASSEXW type{sizeof(type)}; type.hInstance = instance; type.lpfnWndProc = State::proc; type.lpszClassName = window_class; type.hCursor = LoadCursorW(nullptr, IDC_ARROW); type.hbrBackground = GetSysColorBrush(COLOR_BTNFACE);
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
  type.lpfnWndProc = State::body_proc; type.lpszClassName = body_class; if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
  const auto frame = panel_frame(640, 700, state.dpi);
  state.host = CreateWindowExW(WS_EX_TOOLWINDOW, window_class, L"词库同步", WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN, frame.left, frame.top, frame.right - frame.left, frame.bottom - frame.top, nullptr, nullptr, instance, &state);
  if (!state.host) return false;
  state.body = CreateWindowExW(WS_EX_CONTROLPARENT, body_class, L"词库同步内容", WS_CHILD | WS_VISIBLE | WS_VSCROLL | WS_CLIPCHILDREN, 0, 0, 1, 1, state.host, reinterpret_cast<HMENU>(dictionary_sync_panel_body), instance, &state);
  state.font = CreateFontW(-state.px(14), 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, DEFAULT_PITCH, L"Segoe UI");
  bool failed = !state.body;
  state.height = state.form.create([&](const wchar_t* kind, const wchar_t* title, DWORD style, int id, int top, int height) {
    if (lstrcmpiW(kind, L"BUTTON") == 0) style |= BS_NOTIFY;
    const auto control = CreateWindowExW(0, kind, title, WS_CHILD | WS_VISIBLE | style, 0, 0, 1, 1, state.body, reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)), instance, nullptr);
    if (!control) failed = true; else { SendMessageW(control, WM_SETFONT, reinterpret_cast<WPARAM>(state.font), FALSE); state.items.push_back({control, top, height}); } return control;
  });
  state.notice = CreateWindowExW(0, L"STATIC", L"", WS_CHILD | WS_VISIBLE | SS_LEFT, 0, 0, 1, 1, state.host, reinterpret_cast<HMENU>(static_cast<INT_PTR>(notice_id)), instance, nullptr);
  if (failed || !state.notice) return false;
  SendMessageW(state.notice, WM_SETFONT, reinterpret_cast<WPARAM>(state.font), FALSE);
  state.worker = std::make_unique<Worker>(state.host, state.services); state.busy = false; state.layout(); state.submit({}); return true;
}
int DictionarySyncWindow::run(int show) {
  state_->quit = true; ShowWindow(state_->host, show); MSG message{}; BOOL result;
  while ((result = GetMessageW(&message, nullptr, 0, 0)) > 0) { if (!IsDialogMessageW(state_->host, &message)) { TranslateMessage(&message); DispatchMessageW(&message); } }
  return result == -1 ? 1 : static_cast<int>(message.wParam);
}
} // namespace msime::windows::win7
