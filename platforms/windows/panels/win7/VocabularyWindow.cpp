#include "VocabularyWindow.h"
#include "PanelContext.h"
#include "../../settings/win7/VocabularyForm.h"
#include "../../settings/win7/VocabularyData.h"
#include "../../compat/WindowsDpi.h"
#include <commctrl.h>
#include <shobjidl.h>
#include <windowsx.h>
#include <condition_variable>
#include <mutex>
#include <thread>
#include <utility>

namespace msime::windows::win7 {
namespace {
namespace shared = msime::settings::win7;
constexpr UINT completed_message = WM_APP + 77;
constexpr wchar_t window_class[] = L"MSIME.Windows7Vocabulary" MSIME_EDITION_NAME_SUFFIX;
constexpr wchar_t body_class[] = L"MSIME.Windows7Vocabulary.Body" MSIME_EDITION_NAME_SUFFIX;
struct Job { Json action; std::filesystem::path file; };
struct Completion { Json response; std::string operation; };
class Worker {
public:
  explicit Worker(HWND window) : window_(window), thread_([this] { run(); }) {}
  ~Worker() { { std::lock_guard<std::mutex> guard(mutex_); stopping_ = true; } ready_.notify_one(); thread_.join(); }
  void submit(Job job) { { std::lock_guard<std::mutex> guard(mutex_); pending_ = std::move(job); } ready_.notify_one(); }
  std::optional<Completion> take() { std::lock_guard<std::mutex> guard(mutex_); return std::exchange(completed_, std::nullopt); }
private:
  void run() {
    for (;;) {
      std::unique_lock<std::mutex> guard(mutex_); ready_.wait(guard, [this] { return stopping_ || pending_; });
      if (!pending_) return;
      auto job = std::move(*pending_); pending_.reset(); guard.unlock();
      Completion result;
      try {
        const auto context = panel_context(); result.operation = job.action.value("operation", std::string{});
        result.response = shared::vocabulary_data::call(context.directory, context.options, std::move(job.action), job.file);
      } catch (const std::exception&) { result.response = Json{{"ok", false}, {"error", "vocabulary_unavailable"}}; }
      guard.lock(); if (!stopping_) { completed_ = std::move(result); PostMessageW(window_, completed_message, 0, 0); }
    }
  }
  HWND window_;
  std::mutex mutex_;
  std::condition_variable ready_;
  bool stopping_ = false;
  std::optional<Job> pending_;
  std::optional<Completion> completed_;
  std::thread thread_;
};
} // namespace
struct VocabularyWindow::State {
  struct Item { HWND handle; int top, height; };
  HWND host = nullptr, body = nullptr, notice = nullptr;
  HINSTANCE instance = nullptr; HFONT font = nullptr; UINT dpi = USER_DEFAULT_SCREEN_DPI;
  int scroll = 0, height = 0;
  bool busy = true, quit = false, com = false;
  std::vector<Item> items;
  shared::VocabularyForm form;
  FilePicker file_picker; Confirmation confirmation;
  std::unique_ptr<Worker> worker;
  State(FilePicker picker, Confirmation confirm) : file_picker(std::move(picker)), confirmation(std::move(confirm)) {}
  ~State() { worker.reset(); if (host && IsWindow(host)) DestroyWindow(host); if (font) DeleteObject(font); if (com) CoUninitialize(); }
  int px(int value) const { return MulDiv(value, static_cast<int>(dpi), USER_DEFAULT_SCREEN_DPI); }
  void submit(Job job) { if (busy) return; busy = true; form.enable(false); SetWindowTextW(notice, L"正在处理…"); worker->submit(std::move(job)); }
  void complete() {
    auto result = worker->take(); if (!result) return; busy = false;
    form.completed(result->response, result->operation); form.enable(true); SetWindowTextW(notice, L"");
  }
  std::optional<std::filesystem::path> pick_file() {
    if (file_picker) return file_picker(host, false);
    IFileOpenDialog* raw = nullptr;
    const auto failure = [this] { SetWindowTextW(notice, L"无法打开词表选择窗口，请重试。"); return std::optional<std::filesystem::path>{}; };
    if (FAILED(CoCreateInstance(CLSID_FileOpenDialog, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&raw)))) return failure();
    const auto release = [](IFileOpenDialog* value) { value->Release(); };
    const std::unique_ptr<IFileOpenDialog, decltype(release)> dialog(raw, release);
    const COMDLG_FILTERSPEC filter{L"词表 (*.csv;*.txt)", L"*.csv;*.txt"}; FILEOPENDIALOGOPTIONS options = 0;
    if (FAILED(dialog->SetFileTypes(1, &filter)) || FAILED(dialog->SetTitle(L"导入背单词词表")) ||
        FAILED(dialog->GetOptions(&options)) || FAILED(dialog->SetOptions(options | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST | FOS_DONTADDTORECENT))) return failure();
    const auto shown = dialog->Show(host); if (shown == HRESULT_FROM_WIN32(ERROR_CANCELLED)) return std::nullopt;
    if (FAILED(shown)) return failure();
    IShellItem* item = nullptr; if (FAILED(dialog->GetResult(&item))) return failure();
    PWSTR path = nullptr; const auto resolved = item->GetDisplayName(SIGDN_FILESYSPATH, &path); item->Release();
    if (FAILED(resolved) || !path) { CoTaskMemFree(path); return failure(); }
    const std::filesystem::path selected(path); CoTaskMemFree(path);
    return selected.is_absolute() ? std::make_optional(selected) : failure();
  }
  void command(WPARAM event) {
    if (busy) return;
    auto action = form.command(event); if (!action.handled || !action.request) return;
    if (!action.confirmation.empty()) {
      const bool allowed = confirmation ? confirmation(host, L"背单词", action.confirmation.c_str()) : MessageBoxW(host, action.confirmation.c_str(), L"背单词", MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2) == IDYES;
      if (!allowed) return;
    }
    Job job{std::move(*action.request), {}};
    if (action.file) { const auto path = pick_file(); if (!path) return; job.file = *path; }
    submit(std::move(job));
  }
  void layout() {
    RECT client{}; GetClientRect(host, &client);
    MoveWindow(body, px(12), px(12), std::max(1L, client.right - px(24)), std::max(1L, client.bottom - px(64)), TRUE);
    MoveWindow(notice, px(12), client.bottom - px(42), std::max(1L, client.right - px(24)), px(36), TRUE);
    GetClientRect(body, &client); SCROLLINFO info{sizeof(info)};
    info.fMask = SIF_RANGE | SIF_PAGE | SIF_POS; info.nMax = px(height) - 1; info.nPage = std::max(1L, client.bottom); info.nPos = scroll;
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
      if (message == WM_MOUSEWHEEL) state->scroll -= MulDiv(GET_WHEEL_DELTA_WPARAM(wparam), state->px(72), WHEEL_DELTA);
      else {
        SCROLLINFO info{sizeof(info)}; info.fMask = SIF_ALL; if (!GetScrollInfo(window, SB_VERT, &info)) return 0;
        switch (LOWORD(wparam)) {
        case SB_TOP: state->scroll = info.nMin; break;
        case SB_BOTTOM: state->scroll = info.nMax; break;
        case SB_LINEUP: state->scroll -= state->px(36); break;
        case SB_LINEDOWN: state->scroll += state->px(36); break;
        case SB_PAGEUP: state->scroll -= static_cast<int>(info.nPage); break;
        case SB_PAGEDOWN: state->scroll += static_cast<int>(info.nPage); break;
        case SB_THUMBTRACK:
        case SB_THUMBPOSITION: state->scroll = info.nTrackPos; break;
        default: return 0;
        }
      }
      state->scroll = std::max(0, state->scroll); state->layout(); return 0;
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
VocabularyWindow::VocabularyWindow(FilePicker picker, Confirmation confirmation) : state_(std::make_unique<State>(std::move(picker), std::move(confirmation))) {}
VocabularyWindow::~VocabularyWindow() = default;
HWND VocabularyWindow::handle() const { return state_->host; }
bool VocabularyWindow::create(HINSTANCE instance) {
  auto& state = *state_; state.instance = instance; state.dpi = compat::system_dpi();
  const auto initialized = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED); if (FAILED(initialized)) return false; state.com = true;
  INITCOMMONCONTROLSEX common{sizeof(common), ICC_STANDARD_CLASSES}; if (!InitCommonControlsEx(&common)) return false;
  WNDCLASSEXW type{sizeof(type)}; type.hInstance = instance; type.lpfnWndProc = State::proc; type.lpszClassName = window_class; type.hCursor = LoadCursorW(nullptr, IDC_ARROW); type.hbrBackground = GetSysColorBrush(COLOR_BTNFACE);
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
  type.lpfnWndProc = State::body_proc; type.lpszClassName = body_class; if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
  const auto frame = panel_frame(580, 700, state.dpi);
  state.host = CreateWindowExW(WS_EX_TOOLWINDOW, window_class, L"背单词", WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN, frame.left, frame.top, frame.right - frame.left, frame.bottom - frame.top, nullptr, nullptr, instance, &state);
  if (!state.host) return false;
  state.body = CreateWindowExW(WS_EX_CONTROLPARENT, body_class, L"背单词内容", WS_CHILD | WS_VISIBLE | WS_VSCROLL | WS_CLIPCHILDREN, 0, 0, 1, 1, state.host, reinterpret_cast<HMENU>(vocabulary_panel_body), instance, &state);
  state.font = CreateFontW(-state.px(14), 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, DEFAULT_PITCH, L"Segoe UI");
  bool failed = !state.body;
  state.height = state.form.create([&](const wchar_t* kind, const wchar_t* title, DWORD style, int id, int top, int height) {
    if (lstrcmpiW(kind, L"BUTTON") == 0) style |= BS_NOTIFY;
    const auto control = CreateWindowExW(0, kind, title, WS_CHILD | WS_VISIBLE | style, 0, 0, 1, 1, state.body, reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)), instance, nullptr);
    if (!control) failed = true; else { SendMessageW(control, WM_SETFONT, reinterpret_cast<WPARAM>(state.font), FALSE); state.items.push_back({control, top, height}); } return control;
  });
  state.notice = CreateWindowExW(0, L"STATIC", L"", WS_CHILD | WS_VISIBLE | SS_LEFT, 0, 0, 1, 1, state.host, reinterpret_cast<HMENU>(vocabulary_panel_notice), instance, nullptr);
  if (failed || !state.notice) return false;
  SendMessageW(state.notice, WM_SETFONT, reinterpret_cast<WPARAM>(state.font), FALSE);
  state.worker = std::make_unique<Worker>(state.host); state.busy = false; state.layout(); state.submit({shared::VocabularyForm::load_request(), {}}); return true;
}
int VocabularyWindow::run(int show) {
  state_->quit = true; ShowWindow(state_->host, show); MSG message{}; BOOL result;
  while ((result = GetMessageW(&message, nullptr, 0, 0)) > 0) { if (!IsDialogMessageW(state_->host, &message)) { TranslateMessage(&message); DispatchMessageW(&message); } }
  return result == -1 ? 1 : static_cast<int>(message.wParam);
}
} // namespace msime::windows::win7
