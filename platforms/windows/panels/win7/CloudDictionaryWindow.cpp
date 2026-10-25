#include "CloudDictionaryWindow.h"
#include "PanelContext.h"
#include "../../settings/win7/CloudDictionaryForm.h"
#include "../../settings/win7/CloudDictionaryWorker.h"
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
constexpr UINT completed_message = WM_APP + 78;
constexpr wchar_t window_class[] = L"MSIME.Windows7CloudDictionary" MSIME_EDITION_NAME_SUFFIX;
constexpr wchar_t body_class[] = L"MSIME.Windows7CloudDictionary.Body" MSIME_EDITION_NAME_SUFFIX;
struct Job { Json action; std::filesystem::path file; };
struct Completion { Json response, request; };
class Worker {
public:
  explicit Worker(HWND window, shared::CloudDictionaryServices services) : window_(window), services_(std::move(services)), thread_([this] { run(); }) {}
  ~Worker() { { std::lock_guard<std::mutex> guard(mutex_); stopping_ = true; } ready_.notify_one(); thread_.join(); }
  void submit(Job job) { { std::lock_guard<std::mutex> guard(mutex_); pending_ = std::move(job); } ready_.notify_one(); }
  std::optional<Completion> take() { std::lock_guard<std::mutex> guard(mutex_); return std::exchange(completed_, std::nullopt); }
private:
  void run() {
    for (;;) {
      std::unique_lock<std::mutex> guard(mutex_); ready_.wait(guard, [this] { return stopping_ || pending_; });
      if (!pending_) return;
      auto job = std::move(*pending_); pending_.reset(); guard.unlock();
      Completion result; result.request = std::move(job.action);
      try {
        const auto context = panel_context(); DWORD session = 0;
        if (!ProcessIdToSessionId(GetCurrentProcessId(), &session)) throw std::runtime_error("session_unavailable");
        result.response = shared::run_cloud_dictionary(result.request, panel_account_request({{"operation", "status"}}), context.options,
            context.directory / (L"win7-cloud-snapshots-" + std::to_wstring(session)), job.file, services_);
      } catch (const std::exception&) { result.response = Json{{"response", {{"ok", false}, {"error", "dictionary_unavailable"}}}}; }
      guard.lock(); if (!stopping_) { completed_ = std::move(result); PostMessageW(window_, completed_message, 0, 0); }
    }
  }
  HWND window_;
  shared::CloudDictionaryServices services_;
  std::mutex mutex_;
  std::condition_variable ready_;
  bool stopping_ = false;
  std::optional<Job> pending_;
  std::optional<Completion> completed_;
  std::thread thread_;
};
} // namespace
struct CloudDictionaryWindow::State {
  struct Item { HWND handle; int top, height; };
  HWND host = nullptr, body = nullptr, notice = nullptr;
  HINSTANCE instance = nullptr; HFONT font = nullptr; UINT dpi = USER_DEFAULT_SCREEN_DPI;
  int scroll = 0, height = 0;
  bool busy = true, quit = false, com = false, modal = false;
  std::vector<Item> items;
  shared::CloudDictionaryForm form;
  FilePicker file_picker; Confirmation confirmation; shared::CloudDictionaryServices services; HANDLE singleton = nullptr;
  std::unique_ptr<Worker> worker;
  State(FilePicker picker, Confirmation confirm, shared::CloudDictionaryServices transport) : file_picker(std::move(picker)), confirmation(std::move(confirm)), services(std::move(transport)) {}
  ~State() { worker.reset(); if (host && IsWindow(host)) DestroyWindow(host); if (font) DeleteObject(font); if (com) CoUninitialize(); if (singleton) CloseHandle(singleton); }
  int px(int value) const { return MulDiv(value, static_cast<int>(dpi), USER_DEFAULT_SCREEN_DPI); }
  void submit(Job job) { if (busy) return; busy = true; form.enable(false); SetWindowTextW(notice, L"正在处理…"); worker->submit(std::move(job)); }
  void complete() {
    auto result = worker->take(); if (!result) return; busy = false;
    form.completed(result->request, result->response); form.enable(true); SetWindowTextW(notice, L"");
  }
  std::optional<std::filesystem::path> pick_file(shared::CloudDictionaryForm::File choice) {
    const bool save = choice == shared::CloudDictionaryForm::File::export_words || choice == shared::CloudDictionaryForm::File::export_snapshot;
    if (file_picker) return file_picker(host, save);
    IFileDialog* raw = nullptr;
    const auto failure = [this] { SetWindowTextW(notice, L"无法打开文件选择窗口，请重试。"); return std::optional<std::filesystem::path>{}; };
    if (FAILED(CoCreateInstance(save ? CLSID_FileSaveDialog : CLSID_FileOpenDialog, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&raw)))) return failure();
    const auto release = [](IFileDialog* value) { value->Release(); };
    const std::unique_ptr<IFileDialog, decltype(release)> dialog(raw, release);
    const bool snapshot = choice == shared::CloudDictionaryForm::File::export_snapshot || choice == shared::CloudDictionaryForm::File::restore_snapshot;
    const COMDLG_FILTERSPEC filter{snapshot ? L"完整词库快照 (*.ndjson)" : L"词表 (*.tsv;*.txt)", snapshot ? L"*.ndjson" : L"*.tsv;*.txt"}; FILEOPENDIALOGOPTIONS options = 0;
    if (FAILED(dialog->SetFileTypes(1, &filter)) || FAILED(dialog->SetTitle(save ? L"保存云词库" : L"选择云词库文件")) ||
        FAILED(dialog->GetOptions(&options)) || FAILED(dialog->SetOptions(options | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_DONTADDTORECENT | (save ? FOS_OVERWRITEPROMPT : FOS_FILEMUSTEXIST)))) return failure();
    if (save && (FAILED(dialog->SetDefaultExtension(snapshot ? L"ndjson" : L"tsv")) || FAILED(dialog->SetFileName(snapshot ? L"msime-dictionary-snapshot.ndjson" : L"msime-cloud-dictionary.tsv")))) return failure();
    const auto shown = dialog->Show(host); if (shown == HRESULT_FROM_WIN32(ERROR_CANCELLED)) return std::nullopt;
    if (FAILED(shown)) return failure();
    IShellItem* item = nullptr; if (FAILED(dialog->GetResult(&item))) return failure();
    PWSTR path = nullptr; const auto resolved = item->GetDisplayName(SIGDN_FILESYSPATH, &path); item->Release();
    if (FAILED(resolved) || !path) { CoTaskMemFree(path); return failure(); }
    const std::filesystem::path selected(path); CoTaskMemFree(path);
    return selected.is_absolute() ? std::make_optional(selected) : failure();
  }
  void command(WPARAM event) {
    if (busy || modal) return;
    auto action = form.command(event); if (!action.handled || !action.request) return;
    struct ModalGuard { bool& flag; explicit ModalGuard(bool& value) : flag(value) { flag = true; } ~ModalGuard() { flag = false; } } modal_guard(modal);
    Job job{std::move(*action.request), {}};
    if (action.file != shared::CloudDictionaryForm::File::none) { const auto path = pick_file(action.file); if (!path) return; job.file = *path; }
    if (!action.confirmation.empty()) {
      const bool allowed = confirmation ? confirmation(host, L"云词库", action.confirmation.c_str()) : MessageBoxW(host, action.confirmation.c_str(), L"云词库", MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2) == IDYES;
      if (!allowed) return;
    }
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
    case WM_TIMER: if (wparam == 1 && !state->busy && !state->modal && state->form.can_poll()) state->submit({state->form.status_request(), {}}); return 0;
    case WM_CLOSE: DestroyWindow(window); return 0;
    case WM_DESTROY: state->worker.reset(); if (state->quit) PostQuitMessage(0); return 0;
    case WM_NCDESTROY: state->host = nullptr; SetWindowLongPtrW(window, GWLP_USERDATA, 0); break;
    }
    return DefWindowProcW(window, message, wparam, lparam);
  }
};
CloudDictionaryWindow::CloudDictionaryWindow(FilePicker picker, Confirmation confirmation, shared::CloudDictionaryServices services) : state_(std::make_unique<State>(std::move(picker), std::move(confirmation), std::move(services))) {}
CloudDictionaryWindow::~CloudDictionaryWindow() = default;
HWND CloudDictionaryWindow::handle() const { return state_->host; }
bool CloudDictionaryWindow::create(HINSTANCE instance) {
  auto& state = *state_;
  state.singleton = CreateMutexW(nullptr, FALSE, L"Local\\MSIME.Windows7CloudDictionary" MSIME_EDITION_NAME_SUFFIX);
  if (!state.singleton || GetLastError() == ERROR_ALREADY_EXISTS) return false;
  state.instance = instance; state.dpi = compat::system_dpi();
  const auto initialized = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED); if (FAILED(initialized)) return false; state.com = true;
  INITCOMMONCONTROLSEX common{sizeof(common), ICC_STANDARD_CLASSES}; if (!InitCommonControlsEx(&common)) return false;
  WNDCLASSEXW type{sizeof(type)}; type.hInstance = instance; type.lpfnWndProc = State::proc; type.lpszClassName = window_class; type.hCursor = LoadCursorW(nullptr, IDC_ARROW); type.hbrBackground = GetSysColorBrush(COLOR_BTNFACE);
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
  type.lpfnWndProc = State::body_proc; type.lpszClassName = body_class; if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
  const auto frame = panel_frame(580, 700, state.dpi);
  state.host = CreateWindowExW(WS_EX_TOOLWINDOW, window_class, L"云词库", WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN, frame.left, frame.top, frame.right - frame.left, frame.bottom - frame.top, nullptr, nullptr, instance, &state);
  if (!state.host) return false;
  state.body = CreateWindowExW(WS_EX_CONTROLPARENT, body_class, L"云词库内容", WS_CHILD | WS_VISIBLE | WS_VSCROLL | WS_CLIPCHILDREN, 0, 0, 1, 1, state.host, reinterpret_cast<HMENU>(cloud_dictionary_panel_body), instance, &state);
  state.font = CreateFontW(-state.px(14), 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, DEFAULT_PITCH, L"Segoe UI");
  bool failed = !state.body;
  state.height = state.form.create([&](const wchar_t* kind, const wchar_t* title, DWORD style, int id, int top, int height) {
    if (lstrcmpiW(kind, L"BUTTON") == 0) style |= BS_NOTIFY;
    const auto control = CreateWindowExW(0, kind, title, WS_CHILD | WS_VISIBLE | style, 0, 0, 1, 1, state.body, reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)), instance, nullptr);
    if (!control) failed = true; else { SendMessageW(control, WM_SETFONT, reinterpret_cast<WPARAM>(state.font), FALSE); state.items.push_back({control, top, height}); } return control;
  });
  state.notice = CreateWindowExW(0, L"STATIC", L"", WS_CHILD | WS_VISIBLE | SS_LEFT, 0, 0, 1, 1, state.host, reinterpret_cast<HMENU>(cloud_dictionary_panel_notice), instance, nullptr);
  if (failed || !state.notice) return false;
  SendMessageW(state.notice, WM_SETFONT, reinterpret_cast<WPARAM>(state.font), FALSE);
  state.worker = std::make_unique<Worker>(state.host, state.services); state.busy = false; state.layout(); SetTimer(state.host, 1, 2000, nullptr); state.submit({state.form.query_request(), {}}); return true;
}
int CloudDictionaryWindow::run(int show) {
  state_->quit = true; ShowWindow(state_->host, show); MSG message{}; BOOL result;
  while ((result = GetMessageW(&message, nullptr, 0, 0)) > 0) { if (!IsDialogMessageW(state_->host, &message)) { TranslateMessage(&message); DispatchMessageW(&message); } }
  return result == -1 ? 1 : static_cast<int>(message.wParam);
}
} // namespace msime::windows::win7
