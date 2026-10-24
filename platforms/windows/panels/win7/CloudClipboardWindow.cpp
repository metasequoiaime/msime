#include "CloudClipboardWindow.h"
#include "PanelContext.h"
#include "../../settings/win7/CloudClipboardForm.h"
#include "../../settings/win7/CloudClipboardWorker.h"
#include "../../settings/win7/StatisticsData.h"
#include "../../src/input/NativeTextInput.h"
#include "../../compat/WindowsDpi.h"
#include "../../tsf/IPC/PassthroughStatistics.h"
#include <commctrl.h>
#include <windowsx.h>
#include <condition_variable>
#include <mutex>
#include <thread>
#include <utility>

namespace msime::windows::win7 {
namespace {
namespace shared = msime::settings::win7;
constexpr UINT completed_message = WM_APP + 76;
constexpr wchar_t window_class[] = L"MSIME.Windows7CloudClipboard" MSIME_EDITION_NAME_SUFFIX;
constexpr wchar_t body_class[] = L"MSIME.Windows7CloudClipboard.Body" MSIME_EDITION_NAME_SUFFIX;
struct Job { std::optional<Json> request; std::vector<shared::Change> changes; std::string transfer; };
struct Completion { shared::Result preferences; std::optional<Json> response; Job job; };
class Worker {
public:
  Worker(HWND window, CloudClipboardWindow::Services services, CloudClipboardWindow::Transport migration) : window_(window), services_(std::move(services)), migration_(std::move(migration)), thread_([this] { run(); }) {}
  ~Worker() { { std::lock_guard<std::mutex> guard(mutex_); stopping_ = true; } ready_.notify_one(); thread_.join(); }
  void submit(Job job) { { std::lock_guard<std::mutex> guard(mutex_); pending_ = std::move(job); } ready_.notify_one(); }
  std::optional<Completion> take() { std::lock_guard<std::mutex> guard(mutex_); return std::exchange(completed_, std::nullopt); }
private:
  Completion execute(Job job) {
    const auto context = panel_context(); shared::PreferencesStore store(context.directory.u8string());
    auto preferences = job.changes.empty() ? store.load() : snapshot_ ? store.save(*snapshot_, job.changes) : shared::Result{};
    if (!preferences.snapshot) preferences.snapshot = store.load().snapshot;
    snapshot_ = preferences.snapshot;
    Completion result{std::move(preferences), std::nullopt, std::move(job)};
    if (!result.preferences.snapshot || !result.job.request) return result;
    if (result.job.transfer == "record") {
      SYSTEMTIME now{}; GetLocalTime(&now); auto action = *result.job.request;
      action["day"] = shared::statistics_data::model::day_key(shared::statistics_data::local_day()); action["hour"] = now.wHour; action["source"] = "unknown";
      shared::theme_data::call(msime_client_typing_statistics, {{"directory", context.directory.u8string()}, {"action", action}});
      return result;
    }
    const auto account = panel_account_directory();
    if (account.empty()) result.response = Json{{"response", {{"ok", false}, {"error", "account_storage"}}}};
    else result.response = shared::run_cloud_clipboard(*result.job.request, context.directory.u8string(), account.u8string(), [&] {
      const auto request = panel_account_request({{"operation", "status"}});
      return migration_ ? Json::parse(migration_(request.dump()), nullptr, false) : shared::theme_data::call(msime_client_account, request);
    }, services_);
    auto fresh = store.load(); snapshot_ = fresh.snapshot; result.preferences.snapshot = fresh.snapshot;
    if (fresh.outcome != shared::Outcome::loaded) result.preferences.outcome = fresh.outcome;
    return result;
  }
  void run() {
    for (;;) {
      std::unique_lock<std::mutex> guard(mutex_); ready_.wait(guard, [this] { return stopping_ || pending_; });
      if (!pending_) return; auto job = std::move(*pending_); pending_.reset(); guard.unlock();
      Completion result;
      try { result = execute(std::move(job)); } catch (const std::exception&) { result.preferences = {}; }
      guard.lock(); if (!stopping_) { completed_ = std::move(result); PostMessageW(window_, completed_message, 0, 0); }
    }
  }
  HWND window_;
  CloudClipboardWindow::Services services_;
  CloudClipboardWindow::Transport migration_;
  std::mutex mutex_;
  std::condition_variable ready_;
  bool stopping_ = false;
  std::optional<Job> pending_;
  std::optional<Completion> completed_;
  std::optional<shared::Snapshot> snapshot_;
  std::thread thread_;
};
bool copy_text(HWND owner, std::wstring_view text) {
  const auto memory = GlobalAlloc(GMEM_MOVEABLE, (text.size() + 1) * sizeof(wchar_t)); if (!memory) return false;
  const auto bytes = static_cast<wchar_t*>(GlobalLock(memory)); if (!bytes) { GlobalFree(memory); return false; }
  std::copy(text.begin(), text.end(), bytes); bytes[text.size()] = L'\0'; GlobalUnlock(memory);
  if (!OpenClipboard(owner)) { GlobalFree(memory); return false; }
  const bool success = EmptyClipboard() && SetClipboardData(CF_UNICODETEXT, memory); CloseClipboard(); if (!success) GlobalFree(memory); return success;
}
} // namespace
struct CloudClipboardWindow::State {
  struct Item { HWND handle; int top, height; };
  HWND host = nullptr, body = nullptr, insert_button = nullptr, notice = nullptr;
  HINSTANCE instance = nullptr; HFONT font = nullptr; UINT dpi = USER_DEFAULT_SCREEN_DPI;
  int scroll = 0, height = 0;
  bool busy = true, quit = false;
  std::vector<Item> items;
  shared::CloudClipboardForm form;
  InsertText insert; CopyText copy; Services services; Transport migration; Confirmation confirmation;
  msimeui::NativeTextInputTarget target = msimeui::NativeTextInputTarget::Capture();
  std::unique_ptr<Worker> worker;
  State(InsertText sink, CopyText clipboard, Services api, Transport account, Confirmation confirm) : insert(std::move(sink)), copy(std::move(clipboard)), services(std::move(api)), migration(std::move(account)), confirmation(std::move(confirm)) {}
  ~State() { worker.reset(); if (host && IsWindow(host)) DestroyWindow(host); if (font) DeleteObject(font); }
  int px(int value) const { return MulDiv(value, static_cast<int>(dpi), USER_DEFAULT_SCREEN_DPI); }
  void enabled(bool enabled) { form.enable(enabled); EnableWindow(insert_button, enabled && form.transfer_request().has_value()); }
  void submit(Job job) { if (busy) return; busy = true; enabled(false); SetWindowTextW(notice, L"正在处理…"); worker->submit(std::move(job)); }
  void complete() {
    auto result = worker->take(); if (!result) return; busy = false;
    if (result->preferences.snapshot) form.refresh(*result->preferences.snapshot);
    if (result->response && result->job.request) form.completed(*result->job.request, *result->response);
    enabled(result->preferences.snapshot.has_value());
    const auto outcome = result->preferences.outcome;
    SetWindowTextW(notice, outcome == shared::Outcome::loaded ? L"" : outcome == shared::Outcome::saved ? L"已保存；请手动刷新确认来源。" : outcome == shared::Outcome::conflict ? L"设置已在其他窗口更新，本次未覆盖，请重新选择。" : L"设置无法读取或保存，请重试。");
    if (result->job.transfer == "record") { SetWindowTextW(notice, L"已输入原编辑器。"); return; }
    if (!result->job.transfer.empty() && result->job.request) {
      const auto selected = result->job.request->at("selected");
      if (!result->response || !form.contains_transfer(selected)) { SetWindowTextW(notice, L"来源或所选内容已经变化，请刷新后重新选择。"); return; }
      const auto text = shared::control_text::wide(selected.at("text").get<std::string>());
      if (result->job.transfer == "copy") { form.copy_result(copy ? copy(host, text) : copy_text(host, text)); return; }
      const bool sent = insert ? insert(text) : target.InsertFromPanel(text, host, PanelTextSendInputExtraInfo);
      if (!sent) { SetWindowTextW(notice, L"未能输入：原编辑器已关闭或前台已经切到其他窗口，请重新打开面板。"); return; }
      submit({Json{{"operation", "record"}, {"text", selected.at("text")}}, {}, "record"});
    }
  }
  void command(WPARAM event) {
    if (busy) return;
    if (LOWORD(event) == cloud_clipboard_panel_insert && HIWORD(event) == BN_CLICKED) { if (const auto request = form.transfer_request()) submit({request, {}, "insert"}); return; }
    auto action = form.command(event); if (!action.handled) return;
    if (!action.confirmation.empty()) {
      const bool allowed = confirmation ? confirmation(host, L"云剪贴板", action.confirmation.c_str()) : MessageBoxW(host, action.confirmation.c_str(), L"云剪贴板", MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2) == IDYES;
      if (!allowed) { form.cancelled(); return; }
    }
    if (action.copy) { if (const auto request = form.transfer_request()) submit({request, {}, "copy"}); return; }
    if (action.request || !action.changes.empty()) submit({std::move(action.request), std::move(action.changes), {}});
    else enabled(true);
  }
  void layout() {
    RECT client{}; GetClientRect(host, &client);
    MoveWindow(body, px(12), px(12), std::max(1L, client.right - px(24)), std::max(1L, client.bottom - px(104)), TRUE);
    MoveWindow(insert_button, px(12), client.bottom - px(80), px(240), px(32), TRUE);
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
      if (HIWORD(wparam) == EN_SETFOCUS || HIWORD(wparam) == BN_SETFOCUS || HIWORD(wparam) == CBN_SETFOCUS || HIWORD(wparam) == LBN_SETFOCUS) {
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
CloudClipboardWindow::CloudClipboardWindow(InsertText insert, CopyText copy, Services services, Transport migration, Confirmation confirmation) : state_(std::make_unique<State>(std::move(insert), std::move(copy), std::move(services), std::move(migration), std::move(confirmation))) {}
CloudClipboardWindow::~CloudClipboardWindow() = default;
HWND CloudClipboardWindow::handle() const { return state_->host; }
bool CloudClipboardWindow::create(HINSTANCE instance) {
  auto& state = *state_; state.instance = instance; state.dpi = compat::system_dpi(); state.target = msimeui::NativeTextInputTarget::Capture();
  INITCOMMONCONTROLSEX common{sizeof(common), ICC_STANDARD_CLASSES}; if (!InitCommonControlsEx(&common)) return false;
  WNDCLASSEXW type{sizeof(type)}; type.hInstance = instance; type.lpfnWndProc = State::proc; type.lpszClassName = window_class; type.hCursor = LoadCursorW(nullptr, IDC_ARROW); type.hbrBackground = GetSysColorBrush(COLOR_BTNFACE);
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
  type.lpfnWndProc = State::body_proc; type.lpszClassName = body_class; if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
  const auto frame = panel_frame(640, 700, state.dpi);
  state.host = CreateWindowExW(WS_EX_TOOLWINDOW, window_class, L"云剪贴板", WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN, frame.left, frame.top, frame.right - frame.left, frame.bottom - frame.top, nullptr, nullptr, instance, &state);
  if (!state.host) return false;
  state.body = CreateWindowExW(WS_EX_CONTROLPARENT, body_class, L"云剪贴板内容", WS_CHILD | WS_VISIBLE | WS_VSCROLL | WS_CLIPCHILDREN, 0, 0, 1, 1, state.host, reinterpret_cast<HMENU>(cloud_clipboard_panel_body), instance, &state);
  state.font = CreateFontW(-state.px(14), 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, DEFAULT_PITCH, L"Segoe UI");
  bool failed = !state.body;
  state.height = state.form.create([&](const wchar_t* kind, const wchar_t* title, DWORD style, int id, int top, int height) {
    if (lstrcmpiW(kind, L"BUTTON") == 0) style |= BS_NOTIFY;
    const auto control = CreateWindowExW(0, kind, title, WS_CHILD | WS_VISIBLE | style, 0, 0, 1, 1, state.body, reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)), instance, nullptr);
    if (!control) failed = true; else { SendMessageW(control, WM_SETFONT, reinterpret_cast<WPARAM>(state.font), FALSE); state.items.push_back({control, top, height}); } return control;
  });
  state.insert_button = CreateWindowExW(0, L"BUTTON", L"输入所选内容到原编辑器", WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON, 0, 0, 1, 1, state.host, reinterpret_cast<HMENU>(cloud_clipboard_panel_insert), instance, nullptr);
  state.notice = CreateWindowExW(0, L"STATIC", L"", WS_CHILD | WS_VISIBLE | SS_LEFT, 0, 0, 1, 1, state.host, reinterpret_cast<HMENU>(cloud_clipboard_panel_notice), instance, nullptr);
  if (failed || !state.insert_button || !state.notice) return false;
  for (const auto control : {state.insert_button, state.notice}) SendMessageW(control, WM_SETFONT, reinterpret_cast<WPARAM>(state.font), FALSE);
  state.worker = std::make_unique<Worker>(state.host, state.services, state.migration); state.busy = false; state.layout(); state.submit({state.form.refresh_request(), {}, {}}); return true;
}
int CloudClipboardWindow::run(int show) {
  state_->quit = true; ShowWindow(state_->host, show); MSG message{}; BOOL result;
  while ((result = GetMessageW(&message, nullptr, 0, 0)) > 0) { if (!IsDialogMessageW(state_->host, &message)) { TranslateMessage(&message); DispatchMessageW(&message); } }
  return result == -1 ? 1 : static_cast<int>(message.wParam);
}
} // namespace msime::windows::win7
