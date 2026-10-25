#include "ClipboardWindow.h"
#include "PanelContext.h"
#include "../../src/input/NativeTextInput.h"
#include "../../compat/WindowsDpi.h"
#include "../../tsf/IPC/PassthroughStatistics.h"
#include "../../settings/HistoryPagination.h"
#include "../../settings/win7/ControlText.h"
#include "../../settings/win7/StatisticsData.h"
#include <commctrl.h>
#include <condition_variable>
#include <mutex>
#include <map>
#include <optional>
#include <thread>
#include <utility>

namespace msime::windows::win7 {
namespace {
namespace shared = msime::settings::win7;
constexpr UINT completed_message = WM_APP + 75;
constexpr wchar_t window_class[] = L"MSIME.Windows7Clipboard" MSIME_EDITION_NAME_SUFFIX;
struct Completion { Json response; std::optional<std::string> insert; std::string operation; };
class Worker {
public:
  Worker(HWND window, ClipboardWindow::Transport transport) : window_(window), transport_(std::move(transport)), thread_([this] { run(); }) {}
  ~Worker() { { std::lock_guard<std::mutex> lock(mutex_); stopping_ = true; } ready_.notify_one(); thread_.join(); }
  void submit(Json action) { { std::lock_guard<std::mutex> lock(mutex_); pending_ = std::move(action); } ready_.notify_one(); }
  std::optional<Completion> take() { std::lock_guard<std::mutex> lock(mutex_); return std::exchange(completed_, std::nullopt); }
private:
  Json history(const PanelContext& context, Json action) {
    const Json request{{"directory", context.directory.u8string()}, {"action", std::move(action)}};
    return transport_ ? Json::parse(transport_(request.dump()), nullptr, false) : shared::theme_data::call(msime_client_desktop_clipboard_history, request);
  }
  Completion execute(Json action) {
    const auto context = panel_context();
    const auto operation = action.value("operation", std::string{});
    if (operation == "record") {
      const auto day = shared::statistics_data::local_day();
      SYSTEMTIME now{}; GetLocalTime(&now);
      action["day"] = shared::statistics_data::model::day_key(day); action["hour"] = now.wHour; action["source"] = "unknown";
      shared::theme_data::call(msime_client_typing_statistics, {{"directory", context.directory.u8string()}, {"action", action}});
      return {history(context, {{"operation", "list"}}), std::nullopt, operation};
    }
    const bool insert = operation == "prepare_insert" || operation == "prepare_copy";
    auto response = history(context, insert ? Json{{"operation", "list"}} : action);
    Completion result{response, std::nullopt, operation};
    if (insert) {
      const auto value = shared::theme_data::value(response);
      if (value && value->is_object() && value->value("enabled", false) && value->contains("entries") && value->at("entries").is_array()) {
        const auto text = action.at("text").get<std::string>();
        for (const auto& entry : value->at("entries")) if (entry.is_object() && entry.contains("text") && entry.at("text") == text) { result.insert = text; break; }
      }
    }
    return result;
  }
  void run() {
    for (;;) {
      std::unique_lock<std::mutex> lock(mutex_); ready_.wait(lock, [this] { return stopping_ || pending_; });
      if (!pending_) return;
      auto action = std::move(*pending_); pending_.reset(); lock.unlock();
      Completion result;
      try { result = execute(std::move(action)); } catch (const std::exception&) { result.response = Json{{"ok", false}, {"error", "clipboard_unavailable"}}; }
      lock.lock();
      if (!stopping_) { completed_ = std::move(result); PostMessageW(window_, completed_message, 0, 0); }
    }
  }
  HWND window_;
  ClipboardWindow::Transport transport_;
  std::mutex mutex_;
  std::condition_variable ready_;
  bool stopping_ = false;
  std::optional<Json> pending_;
  std::optional<Completion> completed_;
  std::thread thread_;
};
bool copy_text(HWND owner, std::wstring_view text) {
  const auto memory = GlobalAlloc(GMEM_MOVEABLE, (text.size() + 1) * sizeof(wchar_t));
  if (!memory) return false;
  const auto bytes = static_cast<wchar_t*>(GlobalLock(memory));
  if (!bytes) { GlobalFree(memory); return false; }
  std::copy(text.begin(), text.end(), bytes); bytes[text.size()] = L'\0'; GlobalUnlock(memory);
  if (!OpenClipboard(owner)) { GlobalFree(memory); return false; }
  const bool copied = EmptyClipboard() && SetClipboardData(CF_UNICODETEXT, memory); CloseClipboard();
  if (!copied) GlobalFree(memory);
  return copied;
}
} // namespace
struct ClipboardWindow::State {
  struct Entry { std::string text; bool pinned; };
  HWND host = nullptr;
  HINSTANCE instance = nullptr;
  HFONT font = nullptr;
  UINT dpi = USER_DEFAULT_SCREEN_DPI;
  std::map<int, HWND> controls;
  std::vector<Entry> entries;
  size_t offset = 0;
  bool busy = true, loaded = false, enabled = false, quit = false;
  msimeui::NativeTextInputTarget target = msimeui::NativeTextInputTarget::Capture();
  InsertText insert;
  CopyText copy;
  Transport transport;
  std::unique_ptr<Worker> worker;
  State(InsertText sink, CopyText clipboard, Transport api) : insert(std::move(sink)), copy(std::move(clipboard)), transport(std::move(api)) {}
  ~State() { worker.reset(); if (host && IsWindow(host)) DestroyWindow(host); if (font) DeleteObject(font); }
  int px(int value) const { return MulDiv(value, static_cast<int>(dpi), USER_DEFAULT_SCREEN_DPI); }
  HWND get(int id) const { const auto found = controls.find(id); return found == controls.end() ? nullptr : found->second; }
  void notice(const wchar_t* text) { SetWindowTextW(get(clipboard_panel_status), text); }
  const Entry* selected() const {
    const auto index = SendMessageW(get(clipboard_panel_entries), LB_GETCURSEL, 0, 0);
    return loaded && enabled && index >= 0 && offset + static_cast<size_t>(index) < entries.size() ? &entries[offset + static_cast<size_t>(index)] : nullptr;
  }
  void update() {
    const auto* entry = selected();
    for (const auto id : {clipboard_panel_insert, clipboard_panel_copy, clipboard_panel_pin, clipboard_panel_remove}) EnableWindow(get(id), !busy && entry);
    EnableWindow(get(clipboard_panel_entries), !busy && loaded && enabled);
    EnableWindow(get(clipboard_panel_refresh), !busy);
    const auto page = msime::settings::history_page(entries.size(), offset);
    EnableWindow(get(clipboard_panel_previous), !busy && loaded && page.offset > 0);
    EnableWindow(get(clipboard_panel_next), !busy && loaded && page.offset < page.last_offset);
    SetWindowTextW(get(clipboard_panel_pin), entry && entry->pinned ? L"取消固定" : L"固定记录");
    SetWindowTextW(get(clipboard_panel_detail), entry ? shared::control_text::wide(entry->text).c_str() : L"");
  }
  void render(const std::string& previous = {}) {
    const auto page = msime::settings::history_page(entries.size(), offset); offset = page.offset;
    SendMessageW(get(clipboard_panel_entries), LB_RESETCONTENT, 0, 0);
    for (size_t index = 0; index < page.count; ++index) {
      const auto& entry = entries[offset + index];
      auto label = shared::control_text::wide(entry.text); std::replace(label.begin(), label.end(), L'\r', L' '); std::replace(label.begin(), label.end(), L'\n', L' ');
      if (label.size() > 100) label = label.substr(0, 100) + L"…";
      label = (entry.pinned ? L"★ " : L"") + label;
      SendMessageW(get(clipboard_panel_entries), LB_ADDSTRING, 0, reinterpret_cast<LPARAM>(label.c_str()));
      if (entry.text == previous) SendMessageW(get(clipboard_panel_entries), LB_SETCURSEL, index, 0);
    }
    const auto label = entries.empty() ? std::wstring(L"暂无记录") : std::to_wstring(offset / page.page_size + 1) + L" / " + std::to_wstring(page.last_offset / page.page_size + 1) + L" 页";
    SetWindowTextW(get(clipboard_panel_page), label.c_str()); update();
  }
  void submit(Json action) { if (busy) return; busy = true; update(); notice(L"正在读取…"); worker->submit(std::move(action)); }
  void complete() {
    auto result = worker->take(); if (!result) return;
    busy = false; const auto* previous = selected(); const auto previous_text = previous ? previous->text : std::string{};
    const auto value = shared::theme_data::value(result->response);
    loaded = value && value->is_object() && value->contains("enabled") && value->at("enabled").is_boolean() && value->contains("entries") && value->at("entries").is_array();
    std::vector<Entry> parsed;
    if (loaded) for (const auto& item : value->at("entries")) {
      if (!item.is_object() || !item.contains("text") || !item.at("text").is_string() || !item.contains("pinned") || !item.at("pinned").is_boolean()) { loaded = false; break; }
      parsed.push_back({item.at("text").get<std::string>(), item.at("pinned").get<bool>()});
    }
    if (loaded) { enabled = value->at("enabled").get<bool>(); entries = enabled ? std::move(parsed) : std::vector<Entry>{}; }
    else { enabled = false; entries.clear(); }
    render(previous_text);
    notice(!loaded ? L"历史记录无法读取，原文件不会自动重置。" : !enabled ? L"剪贴板历史未开启，请在设置中开启。" : entries.empty() ? L"暂无历史记录。" : L"选择记录可输入回打开面板时的编辑器。内容保持原样。");
    if ((result->operation == "prepare_insert" || result->operation == "prepare_copy") && !result->insert) {
      notice(L"历史开关或所选记录已经变化，请重新选择。"); return;
    }
    if (result->insert && loaded && enabled) {
      const auto text = shared::control_text::wide(*result->insert);
      if (result->operation == "prepare_copy") {
        const bool copied = copy ? copy(host, text) : copy_text(host, text);
        notice(copied ? L"已复制。" : L"复制失败，请稍后重试。"); return;
      }
      const bool sent = insert ? insert(text) : target.Insert(text, host, PanelTextSendInputExtraInfo);
      if (!sent) { notice(L"未能输入：请保持打开面板时的编辑器在前台后重试。"); return; }
      submit({{"operation", "record"}, {"text", *result->insert}});
    }
  }
  void command(int id, int notification) {
    if (busy || !IsWindowEnabled(get(id))) return;
    if (id == clipboard_panel_entries && notification == LBN_SELCHANGE) { update(); return; }
    if (id == clipboard_panel_entries && notification == LBN_DBLCLK) id = clipboard_panel_insert;
    else if (notification != BN_CLICKED) return;
    if (id == clipboard_panel_refresh) { submit({{"operation", "sync"}}); return; }
    if (id == clipboard_panel_previous || id == clipboard_panel_next) {
      const auto page = msime::settings::history_page(entries.size(), offset);
      offset = id == clipboard_panel_previous ? offset - page.page_size : offset + page.page_size; render(); return;
    }
    const auto* entry = selected(); if (!entry) return;
    if (id == clipboard_panel_insert) submit({{"operation", "prepare_insert"}, {"text", entry->text}});
    else if (id == clipboard_panel_copy) submit({{"operation", "prepare_copy"}, {"text", entry->text}});
    else if (id == clipboard_panel_pin) submit({{"operation", "set_pinned"}, {"text", entry->text}, {"pinned", !entry->pinned}});
    else if (id == clipboard_panel_remove) submit({{"operation", "remove"}, {"text", entry->text}});
  }
  void layout() {
    RECT bounds{}; GetClientRect(host, &bounds);
    const int width = std::max(1, static_cast<int>(bounds.right) - px(24));
    const int list_height = std::max(px(80), static_cast<int>(bounds.bottom) - px(300));
    MoveWindow(get(clipboard_panel_entries), px(12), px(12), width, list_height, TRUE);
    const int detail_top = px(24) + list_height;
    MoveWindow(get(clipboard_panel_detail), px(12), detail_top, width, px(100), TRUE);
    for (const auto& field : std::vector<std::pair<int, int>>{{clipboard_panel_previous, 12}, {clipboard_panel_next, 112}, {clipboard_panel_page, 222}, {clipboard_panel_refresh, 430}})
      MoveWindow(get(field.first), px(field.second), detail_top + px(112), px(94), px(30), TRUE);
    for (const auto& field : std::vector<std::pair<int, int>>{{clipboard_panel_insert, 12}, {clipboard_panel_copy, 142}, {clipboard_panel_pin, 272}, {clipboard_panel_remove, 402}})
      MoveWindow(get(field.first), px(field.second), detail_top + px(154), px(118), px(30), TRUE);
    MoveWindow(get(clipboard_panel_status), px(12), detail_top + px(196), width, px(70), TRUE);
  }
  static LRESULT CALLBACK proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    auto* state = reinterpret_cast<State*>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) { state = static_cast<State*>(reinterpret_cast<CREATESTRUCTW*>(lparam)->lpCreateParams); SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(state)); state->host = window; }
    if (!state) return DefWindowProcW(window, message, wparam, lparam);
    switch (message) {
    case WM_MOUSEACTIVATE: return MA_NOACTIVATE;
    case WM_SIZE: state->layout(); return 0;
    case WM_COMMAND: state->command(LOWORD(wparam), HIWORD(wparam)); return 0;
    case completed_message: state->complete(); return 0;
    case WM_CLOSE: DestroyWindow(window); return 0;
    case WM_DESTROY: state->worker.reset(); if (state->quit) PostQuitMessage(0); return 0;
    case WM_NCDESTROY: state->host = nullptr; SetWindowLongPtrW(window, GWLP_USERDATA, 0); break;
    }
    return DefWindowProcW(window, message, wparam, lparam);
  }
};
ClipboardWindow::ClipboardWindow(InsertText insert, CopyText copy, Transport transport) : state_(std::make_unique<State>(std::move(insert), std::move(copy), std::move(transport))) {}
ClipboardWindow::~ClipboardWindow() = default;
HWND ClipboardWindow::handle() const { return state_->host; }
bool ClipboardWindow::create(HINSTANCE instance) {
  auto& state = *state_; state.instance = instance; state.target = msimeui::NativeTextInputTarget::Capture(); state.dpi = compat::system_dpi();
  INITCOMMONCONTROLSEX common{sizeof(common), ICC_STANDARD_CLASSES}; if (!InitCommonControlsEx(&common)) return false;
  WNDCLASSEXW type{sizeof(type)}; type.hInstance = instance; type.lpfnWndProc = State::proc; type.lpszClassName = window_class; type.hCursor = LoadCursorW(nullptr, IDC_ARROW); type.hbrBackground = GetSysColorBrush(COLOR_BTNFACE);
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
  const auto frame = panel_frame(560, 600, state.dpi);
  state.host = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW, window_class, L"剪贴板历史", WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN,
      frame.left, frame.top, frame.right - frame.left, frame.bottom - frame.top, nullptr, nullptr, instance, &state);
  if (!state.host) return false;
  state.font = CreateFontW(-state.px(14), 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, DEFAULT_PITCH, L"Segoe UI");
  for (const auto& field : std::vector<std::pair<int, const wchar_t*>>{{clipboard_panel_entries, L""}, {clipboard_panel_detail, L""}, {clipboard_panel_refresh, L"刷新历史"}, {clipboard_panel_previous, L"上一页"}, {clipboard_panel_next, L"下一页"}, {clipboard_panel_page, L""}, {clipboard_panel_insert, L"输入原编辑器"}, {clipboard_panel_copy, L"重新复制"}, {clipboard_panel_pin, L"固定记录"}, {clipboard_panel_remove, L"删除记录"}, {clipboard_panel_status, L"正在读取…"}}) {
    const auto kind = field.first == clipboard_panel_entries ? L"LISTBOX" : field.first == clipboard_panel_detail ? L"EDIT" : field.first == clipboard_panel_page || field.first == clipboard_panel_status ? L"STATIC" : L"BUTTON";
    const DWORD flags = field.first == clipboard_panel_entries ? WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT : field.first == clipboard_panel_detail ? WS_BORDER | WS_VSCROLL | ES_MULTILINE | ES_READONLY : 0;
    const auto control = CreateWindowExW(0, kind, field.second, WS_CHILD | WS_VISIBLE | flags, 0, 0, 1, 1, state.host, reinterpret_cast<HMENU>(static_cast<INT_PTR>(field.first)), instance, nullptr);
    if (!control) return false; state.controls[field.first] = control; SendMessageW(control, WM_SETFONT, reinterpret_cast<WPARAM>(state.font), FALSE);
  }
  state.worker = std::make_unique<Worker>(state.host, state.transport); state.busy = false; state.layout(); state.submit({{"operation", "list"}}); return true;
}
int ClipboardWindow::run(int show) {
  state_->quit = true; ShowWindow(state_->host, show == SW_HIDE ? SW_HIDE : SW_SHOWNOACTIVATE);
  MSG message{}; BOOL result;
  while ((result = GetMessageW(&message, nullptr, 0, 0)) > 0) { TranslateMessage(&message); DispatchMessageW(&message); }
  return result == -1 ? 1 : static_cast<int>(message.wParam);
}
} // namespace msime::windows::win7
