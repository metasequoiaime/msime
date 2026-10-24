#include "EmojiWindow.h"
#include "EmojiRecent.h"
#include "../../common/Windows7EmojiFont.h"
#include "../../src/input/NativeTextInput.h"
#include "../../compat/WindowsDpi.h"
#include "../../tsf/IPC/PassthroughStatistics.h"
#include "../../settings/win7/ControlText.h"
#include "../../settings/win7/StatisticsData.h"
#include <commctrl.h>
#include <condition_variable>
#include <mutex>
#include <map>
#include <thread>
#include <utility>

namespace msime::windows::win7 {
namespace {
namespace shared = msime::settings::win7;
constexpr UINT completed_message = WM_APP + 79;
constexpr wchar_t window_class[] = L"MSIME.Windows7Emoji" MSIME_EDITION_NAME_SUFFIX;
struct Job { std::optional<emoji::Item> item; bool count = false; };
struct Completion { std::optional<emoji::Catalog> catalog; emoji::Recent recent; bool recorded = false; };
class Worker {
public:
  Worker(HWND window, emoji::Transport transport) : window_(window), transport_(std::move(transport)), thread_([this] { run(); }) {}
  ~Worker() { { std::lock_guard<std::mutex> lock(mutex_); stopping_ = true; } ready_.notify_one(); thread_.join(); }
  void submit(Job job) { { std::lock_guard<std::mutex> lock(mutex_); pending_ = std::move(job); } ready_.notify_one(); }
  std::optional<Completion> take() { std::lock_guard<std::mutex> lock(mutex_); return std::exchange(completed_,std::nullopt); }
private:
  Completion execute(const Job& job) {
    const auto context = panel_context(); Completion result; result.recorded = job.item.has_value();
    if (!job.item) { result.catalog = emoji::load(context,transport_); result.recent = emoji::read_recent(context.directory); return result; }
    if (job.count) {
      SYSTEMTIME now{}; GetLocalTime(&now);
      shared::theme_data::call(msime_client_typing_statistics, {{"directory",context.directory.u8string()}, {"action",{
        {"operation","record"},{"text",job.item->text},{"source","unknown"}, {"hour",now.wHour},
        {"day",shared::statistics_data::model::day_key(shared::statistics_data::local_day())}}}});
    }
    result.recent = emoji::remember(context.directory,*job.item); return result;
  }
  void run() {
    for (;;) {
      std::unique_lock<std::mutex> lock(mutex_); ready_.wait(lock,[this] { return stopping_ || pending_; });
      if (!pending_) return;
      auto job = std::move(*pending_); pending_.reset(); lock.unlock();
      Completion result;
      try { result = execute(job); }
      catch (const std::exception&) {
        result.recorded = job.item.has_value(); result.recent.readable = false;
        if (!job.item) { result.catalog = emoji::Catalog{}; result.catalog->errors.push_back(L"表情目录无法读取。"); }
      }
      lock.lock(); if (!stopping_) { completed_ = std::move(result); PostMessageW(window_,completed_message,0,0); }
    }
  }
  HWND window_; emoji::Transport transport_; std::mutex mutex_; std::condition_variable ready_;
  bool stopping_ = false; std::optional<Job> pending_; std::optional<Completion> completed_; std::thread thread_;
};
bool copy_text(HWND owner, std::wstring_view text) {
  const auto memory = GlobalAlloc(GMEM_MOVEABLE,(text.size() + 1) * sizeof(wchar_t)); if (!memory) return false;
  const auto bytes = static_cast<wchar_t*>(GlobalLock(memory)); if (!bytes) { GlobalFree(memory); return false; }
  std::copy(text.begin(),text.end(),bytes); bytes[text.size()] = L'\0'; GlobalUnlock(memory);
  if (!OpenClipboard(owner)) { GlobalFree(memory); return false; }
  const bool copied = EmptyClipboard() && SetClipboardData(CF_UNICODETEXT,memory); CloseClipboard(); if (!copied) GlobalFree(memory); return copied;
}
bool open_clipboard_panel() {
  std::wstring path(32768,L'\0'); const auto length = GetModuleFileNameW(nullptr,path.data(),static_cast<DWORD>(path.size()));
  if (!length || length >= path.size()) return false; path.resize(length);
  auto command = L"\"" + path + L"\" --route=clipboard";
  STARTUPINFOW startup{}; startup.cb = sizeof(startup); PROCESS_INFORMATION process{};
  if (!CreateProcessW(path.c_str(),command.data(),nullptr,nullptr,FALSE,0,nullptr,nullptr,&startup,&process)) return false;
  CloseHandle(process.hThread); CloseHandle(process.hProcess); return true;
}
bool contains(std::wstring_view haystack, std::wstring_view needle) {
  if (needle.empty()) return true;
  if (needle.size() > haystack.size()) return false;
  for (size_t index = 0; index + needle.size() <= haystack.size(); ++index)
    if (CompareStringOrdinal(haystack.data() + index,static_cast<int>(needle.size()),needle.data(),static_cast<int>(needle.size()),TRUE) == CSTR_EQUAL) return true;
  return false;
}
}
struct EmojiWindow::State {
  HWND host = nullptr; HINSTANCE instance = nullptr; HFONT font = nullptr; UINT dpi = USER_DEFAULT_SCREEN_DPI;
  std::map<int,HWND> controls; bool busy = true, quit = false, updating = false;
  emoji::Catalog catalog; emoji::Recent recent; std::vector<emoji::Item> filtered; std::vector<std::string> group_ids;
  size_t offset = 0; static constexpr size_t page_size = 48;
  msimeui::NativeTextInputTarget target = msimeui::NativeTextInputTarget::Capture();
  InsertText insert; CopyText copy; emoji::Transport transport; OpenClipboard clipboard;
  std::unique_ptr<Worker> worker;
  State(InsertText sink, CopyText receiver, emoji::Transport api, OpenClipboard open) : insert(std::move(sink)), copy(std::move(receiver)), transport(std::move(api)), clipboard(std::move(open)) {}
  ~State() { worker.reset(); if (host && IsWindow(host)) DestroyWindow(host); if (font) DeleteObject(font); }
  int px(int value) const { return MulDiv(value,static_cast<int>(dpi),USER_DEFAULT_SCREEN_DPI); }
  HWND get(int id) const { const auto it = controls.find(id); return it == controls.end() ? nullptr : it->second; }
  void notice(const wchar_t* text) { SetWindowTextW(get(emoji_panel_status),text); }
  std::string tab() const { const auto index = SendMessageW(get(emoji_panel_tab),CB_GETCURSEL,0,0); return index == 1 ? "kaomoji" : index == 2 ? "symbols" : "emoji"; }
  std::string group() const { const auto index = SendMessageW(get(emoji_panel_group),CB_GETCURSEL,0,0); return index >= 0 && static_cast<size_t>(index) < group_ids.size() ? group_ids[static_cast<size_t>(index)] : "all"; }
  const emoji::Item* selected() const {
    const auto index = SendMessageW(get(emoji_panel_entries),LB_GETCURSEL,0,0);
    return index >= 0 && offset + static_cast<size_t>(index) < filtered.size() ? &filtered[offset + static_cast<size_t>(index)] : nullptr;
  }
  void update() {
    for (const auto id : {emoji_panel_tab,emoji_panel_group,emoji_panel_search,emoji_panel_entries,emoji_panel_refresh,emoji_panel_clipboard}) EnableWindow(get(id),!busy);
    for (const auto id : {emoji_panel_copy,emoji_panel_insert}) EnableWindow(get(id),!busy && selected());
    EnableWindow(get(emoji_panel_previous),!busy && offset > 0);
    EnableWindow(get(emoji_panel_next),!busy && offset + page_size < filtered.size());
    const auto* item = selected(); const auto detail = item ? shared::control_text::wide(item->annotation) : std::wstring{};
    SetWindowTextW(get(emoji_panel_detail),detail.c_str());
    SetWindowTextW(get(emoji_panel_preview), item ? shared::control_text::wide(item->text).c_str() : L"");
    InvalidateRect(get(emoji_panel_preview),nullptr,TRUE);
  }
  void render_items() {
    SendMessageW(get(emoji_panel_entries),LB_RESETCONTENT,0,0);
    if (offset >= filtered.size()) offset = filtered.empty() ? 0 : ((filtered.size() - 1) / page_size) * page_size;
    for (size_t index = offset; index < std::min(filtered.size(),offset + page_size); ++index) {
      auto label = shared::control_text::wide(filtered[index].text + "  " + filtered[index].annotation);
      std::replace(label.begin(),label.end(),L'\r',L' '); std::replace(label.begin(),label.end(),L'\n',L' ');
      if (label.size() > 160) label.resize(160);
      SendMessageW(get(emoji_panel_entries),LB_ADDSTRING,0,reinterpret_cast<LPARAM>(label.c_str()));
    }
    const auto label = filtered.empty() ? std::wstring(L"没有匹配项") : std::to_wstring(offset / page_size + 1) + L" / " + std::to_wstring((filtered.size() - 1) / page_size + 1) + L" 页，共 " + std::to_wstring(filtered.size()) + L" 项";
    SetWindowTextW(get(emoji_panel_page),label.c_str()); update();
  }
  void filter() {
    filtered.clear(); offset = 0; const auto current_tab = tab(), current_group = group();
    const auto query = shared::control_text::wide(shared::control_text::read(get(emoji_panel_search),4096).value_or(""));
    const auto& source = current_group == "recent" ? recent.items : catalog.items;
    for (const auto& item : source) {
      const bool correct_tab = current_group == "recent" && current_tab != "symbols" ? item.tab != "symbols" : item.tab == current_tab;
      if (correct_tab && (current_group == "all" || current_group == "recent" || current_group == item.group) &&
          (contains(shared::control_text::wide(item.text),query) || contains(shared::control_text::wide(item.annotation),query))) filtered.push_back(item);
    }
    render_items();
  }
  void render_groups(const std::string& previous = "all") {
    updating = true; SendMessageW(get(emoji_panel_group),CB_RESETCONTENT,0,0); group_ids.clear();
    const auto add = [&](const std::string& id, const std::wstring& title) { group_ids.push_back(id); SendMessageW(get(emoji_panel_group),CB_ADDSTRING,0,reinterpret_cast<LPARAM>(title.c_str())); };
    add("all",L"全部分类"); add("recent",L"最近使用"); const auto current_tab = tab();
    for (const auto& entry : catalog.groups) if (entry.tab == current_tab) add(entry.id,shared::control_text::wide(entry.title));
    const auto found = std::find(group_ids.begin(),group_ids.end(),previous);
    SendMessageW(get(emoji_panel_group),CB_SETCURSEL,found == group_ids.end() ? 0 : static_cast<WPARAM>(found - group_ids.begin()),0);
    updating = false; filter();
  }
  void submit(Job job) { if (busy) return; busy = true; update(); worker->submit(std::move(job)); }
  void complete() {
    auto result = worker->take(); if (!result) return; busy = false;
    const auto previous = group(); if (result->catalog) catalog = std::move(*result->catalog); recent = std::move(result->recent);
    render_groups(previous);
    std::wstring message = result->recorded ? L"操作已完成。" : L"选择条目后可复制或输入原编辑器。";
    for (const auto& error : catalog.errors) message += L" " + error;
    if (!recent.readable) message += L" 最近使用记录无法读写，原文件保留。";
    notice(message.c_str());
  }
  void command(int id, int notification) {
    if (busy || updating || !IsWindowEnabled(get(id))) return;
    if (id == emoji_panel_tab && notification == CBN_SELCHANGE) { render_groups(); return; }
    if ((id == emoji_panel_group && notification == CBN_SELCHANGE) || (id == emoji_panel_search && notification == EN_CHANGE)) { filter(); return; }
    if (id == emoji_panel_entries && notification == LBN_SELCHANGE) { update(); return; }
    if (id == emoji_panel_entries && notification == LBN_DBLCLK) id = emoji_panel_insert; else if (notification != BN_CLICKED) return;
    if (id == emoji_panel_refresh) { submit({}); notice(L"正在读取目录…"); return; }
    if (id == emoji_panel_clipboard) { const bool opened = clipboard ? clipboard() : open_clipboard_panel(); notice(opened ? L"已打开剪贴板历史。" : L"剪贴板历史无法打开，请重试。"); return; }
    if (id == emoji_panel_previous || id == emoji_panel_next) { offset = id == emoji_panel_previous ? offset - page_size : offset + page_size; render_items(); return; }
    const auto* chosen = selected(); if (!chosen) return; const auto item = *chosen; const auto text = shared::control_text::wide(item.text);
    if (id == emoji_panel_copy) {
      if (!(copy ? copy(host,text) : copy_text(host,text))) { notice(L"复制失败，请稍后重试。"); return; }
      submit({item,false});
    } else if (id == emoji_panel_insert) {
      if (!(insert ? insert(text) : target.InsertFromPanel(text,host,PanelTextSendInputExtraInfo))) { notice(L"未能输入：请保持原编辑器或本面板在前台后重试。"); return; }
      submit({item,true});
    }
  }
  void layout() {
    RECT bounds{}; GetClientRect(host,&bounds); const int width = std::max(1,static_cast<int>(bounds.right) - px(24));
    MoveWindow(get(emoji_panel_tab),px(12),px(12),px(130),px(260),TRUE);
    MoveWindow(get(emoji_panel_group),px(154),px(12),std::max(1,width - px(142)),px(300),TRUE);
    MoveWindow(get(emoji_panel_search),px(12),px(54),width,px(30),TRUE);
    const int list_height = std::max(px(90),static_cast<int>(bounds.bottom) - px(360));
    MoveWindow(get(emoji_panel_entries),px(12),px(96),width,list_height,TRUE);
    const int top = px(108) + list_height;
    MoveWindow(get(emoji_panel_preview),px(12),top,px(80),px(74),TRUE);
    MoveWindow(get(emoji_panel_detail),px(104),top,std::max(1,width - px(92)),px(74),TRUE);
    MoveWindow(get(emoji_panel_previous),px(12),top + px(86),px(80),px(30),TRUE);
    MoveWindow(get(emoji_panel_next),px(104),top + px(86),px(80),px(30),TRUE);
    MoveWindow(get(emoji_panel_page),px(196),top + px(86),std::max(1,width - px(184)),px(30),TRUE);
    for (const auto& entry : std::vector<std::pair<int,int>>{{emoji_panel_insert,12},{emoji_panel_copy,150},{emoji_panel_refresh,270},{emoji_panel_clipboard,390}})
      MoveWindow(get(entry.first),px(entry.second),top + px(128),px(entry.first == emoji_panel_insert ? 126 : 108),px(32),TRUE);
    MoveWindow(get(emoji_panel_status),px(12),top + px(172),width,px(76),TRUE);
  }
  static LRESULT CALLBACK proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    auto* state = reinterpret_cast<State*>(GetWindowLongPtrW(window,GWLP_USERDATA));
    if (message == WM_NCCREATE) { state = static_cast<State*>(reinterpret_cast<CREATESTRUCTW*>(lparam)->lpCreateParams); SetWindowLongPtrW(window,GWLP_USERDATA,reinterpret_cast<LONG_PTR>(state)); state->host = window; }
    if (!state) return DefWindowProcW(window,message,wparam,lparam);
    switch (message) {
    case WM_MEASUREITEM: {
      auto* item = reinterpret_cast<MEASUREITEMSTRUCT*>(lparam);
      if (item && item->CtlID == emoji_panel_entries) { item->itemHeight = state->px(36); return TRUE; }
      break;
    }
    case WM_DRAWITEM: {
      const auto* item = reinterpret_cast<DRAWITEMSTRUCT*>(lparam);
      if (!item) break;
      if (item->CtlID == emoji_panel_entries && item->itemID != static_cast<UINT>(-1)) {
        const auto list = state->get(emoji_panel_entries);
        const auto length = SendMessageW(list,LB_GETTEXTLEN,item->itemID,0);
        if (length < 0 || length > 4096) return TRUE;
        std::wstring text(static_cast<size_t>(length) + 1,L'\0');
        SendMessageW(list,LB_GETTEXT,item->itemID,reinterpret_cast<LPARAM>(text.data())); text.resize(static_cast<size_t>(length));
        const bool highlighted = (item->itemState & ODS_SELECTED) != 0;
        compat::DrawWindows7EmojiToDC(item->hDC,item->rcItem,text,static_cast<float>(state->px(17)),GetSysColor(highlighted ? COLOR_HIGHLIGHTTEXT : COLOR_WINDOWTEXT),GetSysColor(highlighted ? COLOR_HIGHLIGHT : COLOR_WINDOW));
        if (item->itemState & ODS_FOCUS) DrawFocusRect(item->hDC,&item->rcItem);
        return TRUE;
      }
      if (item->CtlID == emoji_panel_preview) {
        const auto* chosen = state->selected();
        compat::DrawWindows7EmojiToDC(item->hDC,item->rcItem,chosen ? shared::control_text::wide(chosen->text) : L"",static_cast<float>(state->px(32)),GetSysColor(COLOR_WINDOWTEXT),GetSysColor(COLOR_WINDOW));
        return TRUE;
      }
      break;
    }
    case WM_SIZE: state->layout(); return 0;
    case WM_COMMAND: state->command(LOWORD(wparam),HIWORD(wparam)); return 0;
    case completed_message: state->complete(); return 0;
    case WM_CLOSE: DestroyWindow(window); return 0;
    case WM_DESTROY: state->worker.reset(); if (state->quit) PostQuitMessage(0); return 0;
    case WM_NCDESTROY: state->host = nullptr; SetWindowLongPtrW(window,GWLP_USERDATA,0); break;
    }
    return DefWindowProcW(window,message,wparam,lparam);
  }
};
EmojiWindow::EmojiWindow(InsertText insert, CopyText copy, emoji::Transport transport, OpenClipboard clipboard) : state_(std::make_unique<State>(std::move(insert),std::move(copy),std::move(transport),std::move(clipboard))) {}
EmojiWindow::~EmojiWindow() = default;
HWND EmojiWindow::handle() const { return state_->host; }
bool EmojiWindow::create(HINSTANCE instance) {
  auto& state = *state_; state.instance = instance; state.target = msimeui::NativeTextInputTarget::Capture(); state.dpi = compat::system_dpi();
  INITCOMMONCONTROLSEX common{sizeof(common),ICC_STANDARD_CLASSES}; if (!InitCommonControlsEx(&common)) return false;
  WNDCLASSEXW type{sizeof(type)}; type.hInstance = instance; type.lpfnWndProc = State::proc; type.lpszClassName = window_class;
  type.hCursor = LoadCursorW(nullptr,IDC_ARROW); type.hbrBackground = GetSysColorBrush(COLOR_BTNFACE);
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
  const auto frame = panel_frame(560,680,state.dpi);
  state.host = CreateWindowExW(WS_EX_TOOLWINDOW,window_class,L"表情、颜文字与符号",WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
    frame.left,frame.top,frame.right - frame.left,frame.bottom - frame.top,nullptr,nullptr,instance,&state);
  if (!state.host) return false;
  state.font = CreateFontW(-state.px(14),0,0,0,FW_NORMAL,FALSE,FALSE,FALSE,DEFAULT_CHARSET,OUT_DEFAULT_PRECIS,CLIP_DEFAULT_PRECIS,DEFAULT_QUALITY,DEFAULT_PITCH,L"Segoe UI");
  for (const auto& field : std::vector<std::pair<int,const wchar_t*>>{{emoji_panel_tab,L"目录类型"},{emoji_panel_group,L"分类"},{emoji_panel_search,L""},{emoji_panel_entries,L"目录条目"},{emoji_panel_detail,L""},
      {emoji_panel_previous,L"上一页"},{emoji_panel_next,L"下一页"},{emoji_panel_page,L""},{emoji_panel_insert,L"输入原编辑器"},{emoji_panel_copy,L"复制"},{emoji_panel_refresh,L"刷新目录"},{emoji_panel_clipboard,L"剪贴板历史"},{emoji_panel_status,L"正在读取目录…"},{emoji_panel_preview,L""}}) {
    const auto kind = field.first == emoji_panel_tab || field.first == emoji_panel_group ? WC_COMBOBOXW : field.first == emoji_panel_search || field.first == emoji_panel_detail ? L"EDIT" :
      field.first == emoji_panel_entries ? L"LISTBOX" : field.first == emoji_panel_page || field.first == emoji_panel_status || field.first == emoji_panel_preview ? L"STATIC" : L"BUTTON";
    DWORD style = WS_CHILD | WS_VISIBLE;
    if (std::wstring_view(kind) == WC_COMBOBOXW) style |= WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL;
    else if (field.first == emoji_panel_search) style |= WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL;
    else if (field.first == emoji_panel_detail) style |= WS_TABSTOP | WS_BORDER | ES_MULTILINE | ES_READONLY | WS_VSCROLL;
    else if (field.first == emoji_panel_entries) style |= WS_TABSTOP | WS_BORDER | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT | LBS_OWNERDRAWFIXED | LBS_HASSTRINGS;
    else if (field.first == emoji_panel_preview) style |= SS_OWNERDRAW;
    else if (std::wstring_view(kind) == L"BUTTON") style |= WS_TABSTOP | BS_PUSHBUTTON;
    const auto control = CreateWindowExW(0,kind,field.second,style,0,0,1,1,state.host,reinterpret_cast<HMENU>(static_cast<INT_PTR>(field.first)),instance,nullptr);
    if (!control) return false; state.controls[field.first] = control; SendMessageW(control,WM_SETFONT,reinterpret_cast<WPARAM>(state.font),FALSE);
  }
  for (const auto* title : {L"表情",L"颜文字",L"符号"}) SendMessageW(state.get(emoji_panel_tab),CB_ADDSTRING,0,reinterpret_cast<LPARAM>(title));
  SendMessageW(state.get(emoji_panel_tab),CB_SETCURSEL,0,0); SendMessageW(state.get(emoji_panel_search),EM_SETLIMITTEXT,1024,0);
  SendMessageW(state.get(emoji_panel_search),EM_SETCUEBANNER,TRUE,reinterpret_cast<LPARAM>(L"搜索文字或关键词"));
  state.worker = std::make_unique<Worker>(state.host,state.transport); state.busy = false; state.render_groups(); state.layout(); state.submit({}); return true;
}
int EmojiWindow::run(int show) {
  state_->quit = true; ShowWindow(state_->host,show); MSG message{}; BOOL result;
  while ((result = GetMessageW(&message,nullptr,0,0)) > 0) { if (!IsDialogMessageW(state_->host,&message)) { TranslateMessage(&message); DispatchMessageW(&message); } }
  return result == -1 ? 1 : static_cast<int>(message.wParam);
}
}
