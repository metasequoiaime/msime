#include "KeyboardWindow.h"
#include "PanelContext.h"
#include "../../settings/win7/KeyboardSkin.h"
#include "../../settings/win7/SurfaceLauncher.h"
#include "../../compat/WindowsDpi.h"
#include <commctrl.h>
#include <windowsx.h>
#include <condition_variable>
#include <mutex>
#include <stdexcept>
#include <thread>
#include <utility>

namespace msime::windows::win7 {
namespace {
namespace shared = msime::settings::win7;
constexpr UINT completed_message = WM_APP + 80;
constexpr wchar_t window_class[] = L"MSIME.Windows7Keyboard" MSIME_EDITION_NAME_SUFFIX;
struct Configuration {
  Json preferences, theme;
  shared::keyboard_skin::Look look;
  std::optional<std::map<WORD,std::wstring>> hints;
};
class Worker {
public:
  explicit Worker(HWND window) : window_(window), thread_([this] { run(); }) {}
  ~Worker() { { std::lock_guard<std::mutex> lock(mutex_); stopping_ = true; } ready_.notify_one(); thread_.join(); }
  void submit() { { std::lock_guard<std::mutex> lock(mutex_); pending_ = true; } ready_.notify_one(); }
  std::optional<Configuration> take() { std::lock_guard<std::mutex> lock(mutex_); return std::exchange(completed_,std::nullopt); }
private:
  void run() {
    for (;;) {
      std::unique_lock<std::mutex> lock(mutex_); ready_.wait(lock,[this] { return stopping_ || pending_; });
      if (stopping_) return;
      pending_ = false; lock.unlock(); Configuration result;
      try {
        const auto context = panel_context(); const auto loaded = shared::PreferencesStore(context.directory.u8string()).load();
        if (loaded.snapshot) {
          result.preferences = loaded.snapshot->preferences(); result.hints = keyboard::shuangpin_hints(result.preferences);
          const auto mode = result.preferences.value("theme",std::string("system"));
          const bool dark = mode == "dark" || (mode == "system" && shared::theme_data::system_dark());
          result.theme = shared::theme_data::value(shared::theme_data::call(msime_client_resolve_theme,
            {{"global_theme",result.preferences.value("global_theme",std::string("system"))},
             {"custom_theme",result.preferences.value("custom_theme",Json::object())}, {"dark",dark},
             {"skins_directory",context.directory.u8string()}, {"layout",result.preferences.value("candidate_layout",std::string("vertical"))}})).value_or(Json());
          result.look = shared::keyboard_skin::Look::resolved(result.preferences,result.theme);
        }
      } catch (const std::exception&) { result = {}; }
      lock.lock(); if (!stopping_) { completed_ = std::move(result); PostMessageW(window_,completed_message,0,0); }
    }
  }
  HWND window_; std::mutex mutex_; std::condition_variable ready_; bool stopping_ = false, pending_ = false;
  std::optional<Configuration> completed_; std::thread thread_;
};
COLORREF color(const Json& palette, const char* key, COLORREF fallback) {
  if (!palette.is_object() || !palette.contains(key) || !palette.at(key).is_string()) return fallback;
  const auto text = palette.at(key).get<std::string>(); if (text.size() != 7 || text[0] != '#') return fallback;
  unsigned value = 0;
  for (size_t index = 1; index < text.size(); ++index) {
    const char digit = text[index]; const int number = digit >= '0' && digit <= '9' ? digit - '0' : digit >= 'a' && digit <= 'f' ? digit - 'a' + 10 : digit >= 'A' && digit <= 'F' ? digit - 'A' + 10 : -1;
    if (number < 0) return fallback; value = value * 16 + static_cast<unsigned>(number);
  }
  return RGB((value >> 16) & 255,(value >> 8) & 255,value & 255);
}
}
struct KeyboardWindow::State {
  HWND host = nullptr; HINSTANCE instance = nullptr; HFONT font = nullptr; UINT dpi = USER_DEFAULT_SCREEN_DPI;
  bool quit = false, ready = false, initially_loaded = false;
  Foreground foreground; SendKeys send; OpenSurface open; std::unique_ptr<Worker> worker;
  keyboard::Rows rows; keyboard::Modifiers modifiers; Configuration configuration;
  std::string active_layout = "twenty_six_key";
  std::map<int,HWND> controls; std::map<int,keyboard::Key> keys;
  COLORREF background = RGB(228,230,234), key_color = RGB(255,255,255), function_color = RGB(200,204,211), text_color = RGB(26,26,26), accent = RGB(0,100,70);
  HBRUSH background_brush = nullptr;
  State(Foreground probe, SendKeys sink, OpenSurface opener) : foreground(std::move(probe)), send(std::move(sink)), open(std::move(opener)) {}
  ~State() { worker.reset(); if (host && IsWindow(host)) DestroyWindow(host); if (font) DeleteObject(font); if (background_brush) DeleteObject(background_brush); }
  int px(int value) const { return MulDiv(value,static_cast<int>(dpi),USER_DEFAULT_SCREEN_DPI); }
  HWND get(int id) const { const auto found = controls.find(id); return found == controls.end() ? nullptr : found->second; }
  void notice(const wchar_t* title) { SetWindowTextW(get(keyboard_panel_status),title); }
  HWND target() const {
    const auto current = foreground ? foreground() : GetForegroundWindow();
    if (!current || !IsWindow(current) || GetAncestor(current,GA_ROOT) == host) return nullptr;
    DWORD process = 0; if (!GetWindowThreadProcessId(current,&process) || (!foreground && process == GetCurrentProcessId())) return nullptr;
    return current;
  }
  void faces() {
    for (const auto& entry : keys) {
      auto label = keyboard::face(entry.second,modifiers); if (!entry.second.hint.empty()) label += L"\n" + entry.second.hint;
      SetWindowTextW(get(entry.first),label.c_str()); InvalidateRect(get(entry.first),nullptr,TRUE);
    }
  }
  void press(const keyboard::Key& key) {
    if (!ready) return;
    if (key.modifier) { keyboard::toggle(key.code,modifiers); faces(); return; }
    const auto destination = target(); DWORD process = 0; const auto thread = destination ? GetWindowThreadProcessId(destination,&process) : 0;
    const auto inputs = keyboard::sequence(key,modifiers); modifiers.shift = false; faces();
    DWORD current_process = 0;
    if (!destination || target() != destination || GetWindowThreadProcessId(destination,&current_process) != thread || current_process != process) {
      notice(L"未发送：请先点选要输入的编辑器，再点击键盘。"); return;
    }
    // 一次提交，失败不重放；系统返回部分发送时只释放尚未释放的按键，避免修饰键卡住。
    const UINT sent = send ? send(destination,inputs) : SendInput(static_cast<UINT>(inputs.size()),const_cast<INPUT*>(inputs.data()),sizeof(INPUT));
    if (sent != inputs.size()) {
      if (!send && sent > 0 && sent < inputs.size()) {
        std::vector<INPUT> releases; std::vector<INPUT> held;
        for (UINT index = 0; index < sent; ++index) {
          const auto& input = inputs[index];
          if ((input.ki.dwFlags & KEYEVENTF_KEYUP) == 0) held.push_back(input);
          else for (auto item = held.begin(); item != held.end(); ++item) if (item->ki.wVk == input.ki.wVk) { held.erase(item); break; }
        }
        for (auto item = held.rbegin(); item != held.rend(); ++item) { auto input = *item; input.ki.dwFlags |= KEYEVENTF_KEYUP; releases.push_back(input); }
        if (!releases.empty()) SendInput(static_cast<UINT>(releases.size()),releases.data(),sizeof(INPUT));
      }
      notice(L"按键未能完整发送，未重试；请检查目标窗口权限。"); return;
    }
    const auto title = L"已发送：" + keyboard::face(key,modifiers); notice(title.c_str());
  }
  static LRESULT CALLBACK key_proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    const auto previous = reinterpret_cast<WNDPROC>(GetPropW(window,L"MSIME.Win7.Keyboard.ButtonProc"));
    // 标准按钮的默认鼠标路径会 SetFocus；保留 HWND 可访问名称与按钮语义，鼠标路径只捕获和释放。
    switch (message) {
    case WM_MOUSEACTIVATE: return MA_NOACTIVATE;
    case WM_LBUTTONDOWN: case WM_LBUTTONDBLCLK: if (IsWindowEnabled(window)) { SetCapture(window); SendMessageW(window,BM_SETSTATE,TRUE,0); } return 0;
    case WM_MOUSEMOVE:
      if (GetCapture() == window) { RECT area{}; GetClientRect(window,&area); POINT point{GET_X_LPARAM(lparam),GET_Y_LPARAM(lparam)}; SendMessageW(window,BM_SETSTATE,PtInRect(&area,point),0); } return 0;
    case WM_LBUTTONUP:
      if (GetCapture() == window) {
        RECT area{}; GetClientRect(window,&area); POINT point{GET_X_LPARAM(lparam),GET_Y_LPARAM(lparam)};
        const bool inside = PtInRect(&area,point) != FALSE; ReleaseCapture(); SendMessageW(window,BM_SETSTATE,FALSE,0);
        if (inside && IsWindowEnabled(window)) SendMessageW(GetParent(window),WM_COMMAND,MAKEWPARAM(GetDlgCtrlID(window),BN_CLICKED),reinterpret_cast<LPARAM>(window));
      } return 0;
    case WM_CANCELMODE: if (GetCapture() == window) ReleaseCapture(); SendMessageW(window,BM_SETSTATE,FALSE,0); return 0;
    case WM_CAPTURECHANGED: SendMessageW(window,BM_SETSTATE,FALSE,0); return 0;
    case WM_NCDESTROY: RemovePropW(window,L"MSIME.Win7.Keyboard.ButtonProc"); break;
    }
    return previous ? CallWindowProcW(previous,window,message,wparam,lparam) : DefWindowProcW(window,message,wparam,lparam);
  }
  void button(int id, const wchar_t* title) {
    auto control = CreateWindowExW(0,L"BUTTON",title,WS_CHILD | WS_VISIBLE | BS_OWNERDRAW,0,0,1,1,host,reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)),instance,nullptr);
    if (!control) throw std::runtime_error("keyboard_control_create"); controls[id] = control;
    const auto previous = GetWindowLongPtrW(control,GWLP_WNDPROC);
    if (!previous || !SetPropW(control,L"MSIME.Win7.Keyboard.ButtonProc",reinterpret_cast<HANDLE>(previous))) throw std::runtime_error("keyboard_control_subclass");
    if (!SetWindowLongPtrW(control,GWLP_WNDPROC,reinterpret_cast<LONG_PTR>(key_proc))) { RemovePropW(control,L"MSIME.Win7.Keyboard.ButtonProc"); throw std::runtime_error("keyboard_control_subclass"); }
    SendMessageW(control,WM_SETFONT,reinterpret_cast<WPARAM>(font),FALSE);
  }
  void rebuild() {
    for (const auto& entry : keys) { DestroyWindow(get(entry.first)); controls.erase(entry.first); }
    keys.clear(); modifiers = {};
    if (active_layout == "twenty_six_key") rows = keyboard::full_rows();
    else rows = keyboard::nine_rows(active_layout == "nine_key_shuangpin" && configuration.hints ? *configuration.hints : std::map<WORD,std::wstring>{});
    int id = keyboard_panel_first_key;
    for (const auto& row : rows) for (const auto& key : row) { keys.emplace(id,key); button(id,L""); ++id; }
    SetWindowTextW(get(keyboard_panel_layout),active_layout == "twenty_six_key" ? L"切换九键" : L"切换全键盘"); faces(); layout(); enable();
  }
  void enable() {
    for (const auto& entry : keys) EnableWindow(get(entry.first),ready && (active_layout != "nine_key_shuangpin" || configuration.hints.has_value()));
    EnableWindow(get(keyboard_panel_layout),ready); EnableWindow(get(keyboard_panel_refresh),ready);
    const bool voice = configuration.preferences.is_object() && configuration.preferences.value("touch_voice_shortcut",false);
    EnableWindow(get(keyboard_panel_voice),ready && voice); ShowWindow(get(keyboard_panel_voice),voice ? SW_SHOWNOACTIVATE : SW_HIDE);
  }
  void refresh() { ready = false; enable(); notice(L"正在读取已应用的键盘设置…"); worker->submit(); }
  void complete() {
    auto result = worker->take(); if (!result) return;
    const auto previous_layout = configuration.preferences.is_object() ? configuration.preferences.value("touch_keyboard_layout",std::string{}) : std::string{};
    const auto previous_height = configuration.preferences.is_object() ? configuration.preferences.value("touch_keyboard_height_adjustment",0) : 0;
    configuration = std::move(*result);
    if (!configuration.preferences.is_object() || !configuration.theme.is_object()) {
      // 失败撤销旧键面，不用旧配置继续输入；刷新入口保持可用。
      ready = false; modifiers = {}; faces(); enable(); EnableWindow(get(keyboard_panel_refresh),TRUE);
      notice(L"键盘设置无法读取，按键已停用；请刷新重试。"); return;
    }
    ready = true;
    const auto preferred = configuration.preferences.value("touch_keyboard_layout",std::string("twenty_six_key"));
    if (!initially_loaded || preferred != previous_layout) active_layout = preferred;
    const auto palette = configuration.theme.value("keyboard",Json::object());
    background = color(palette,"background",RGB(228,230,234)); key_color = color(palette,"key",RGB(255,255,255));
    function_color = color(palette,"function_key",RGB(200,204,211)); text_color = color(palette,"text",RGB(26,26,26));
    accent = color(configuration.theme,"accent",RGB(0,100,70));
    if (background_brush) DeleteObject(background_brush); background_brush = CreateSolidBrush(background);
    if (!initially_loaded || previous_height != configuration.preferences.value("touch_keyboard_height_adjustment",0)) {
      UINT actual_dpi = dpi; const auto frame = panel_frame(1120,400 + configuration.preferences.value("touch_keyboard_height_adjustment",0),actual_dpi);
      if (actual_dpi != dpi) {
        dpi = actual_dpi; if (font) DeleteObject(font);
        font = CreateFontW(-px(12),0,0,0,FW_NORMAL,FALSE,FALSE,FALSE,DEFAULT_CHARSET,OUT_DEFAULT_PRECIS,CLIP_DEFAULT_PRECIS,DEFAULT_QUALITY,DEFAULT_PITCH,L"Segoe UI");
        for (const auto& control : controls) SendMessageW(control.second,WM_SETFONT,reinterpret_cast<WPARAM>(font),TRUE);
      }
      SetWindowPos(host,HWND_TOPMOST,frame.left,frame.top,frame.right - frame.left,frame.bottom - frame.top,SWP_NOACTIVATE);
    }
    if (font) DeleteObject(font);
    font = CreateFontW(-px(12),0,0,0,FW_NORMAL,FALSE,FALSE,FALSE,DEFAULT_CHARSET,OUT_DEFAULT_PRECIS,CLIP_DEFAULT_PRECIS,DEFAULT_QUALITY,DEFAULT_PITCH,configuration.look.mono() ? L"Consolas" : L"Segoe UI");
    for (const auto& control : controls) SendMessageW(control.second,WM_SETFONT,reinterpret_cast<WPARAM>(font),TRUE);
    initially_loaded = true; rebuild(); InvalidateRect(host,nullptr,TRUE);
    notice(active_layout == "nine_key_shuangpin" && !configuration.hints ? L"九键双拼键面无法读取，按键已停用；请刷新重试。" : L"先点选编辑器，再点击按键；拖动标题栏可移动键盘。");
  }
  void command(int id, int notification, HWND source) {
    if (notification != BN_CLICKED || source != get(id) || !IsWindowEnabled(source)) return;
    if (id == keyboard_panel_close) { DestroyWindow(host); return; }
    if (id == keyboard_panel_voice || id == keyboard_panel_settings) {
      const auto route = id == keyboard_panel_voice ? "voice" : "settings:screen-keyboard";
      const bool opened = open ? open(route) : shared::launch_surface(route); notice(opened ? L"已打开。" : L"无法打开，请稍后重试。"); return;
    }
    if (id == keyboard_panel_refresh) { refresh(); return; }
    if (!ready) return;
    if (id == keyboard_panel_layout) {
      const auto preferred = configuration.preferences.value("touch_keyboard_layout",std::string("twenty_six_key"));
      active_layout = active_layout == "twenty_six_key" ? preferred == "nine_key_shuangpin" ? preferred : "nine_key" : "twenty_six_key";
      rebuild(); notice(active_layout == "nine_key_shuangpin" && !configuration.hints ? L"九键双拼键面无法读取，按键已停用。" : L"已切换键面，输入方案保持原设置。"); return;
    }
    const auto found = keys.find(id); if (found != keys.end()) press(found->second);
  }
  void layout() {
    if (!host) return; RECT area{}; GetClientRect(host,&area); const auto width = static_cast<int>(area.right), height = static_cast<int>(area.bottom);
    MoveWindow(get(keyboard_panel_layout),px(10),px(6),px(120),px(28),TRUE);
    MoveWindow(get(keyboard_panel_refresh),px(138),px(6),px(80),px(28),TRUE);
    MoveWindow(get(keyboard_panel_settings),px(226),px(6),px(80),px(28),TRUE);
    MoveWindow(get(keyboard_panel_voice),px(314),px(6),px(80),px(28),TRUE);
    MoveWindow(get(keyboard_panel_close),std::max(0,width - px(65)),px(6),px(55),px(28),TRUE);
    MoveWindow(get(keyboard_panel_status),px(10),std::max(0,height - px(32)),std::max(1,width - px(20)),px(28),TRUE);
    if (rows.empty()) return;
    const auto& preferences = configuration.preferences;
    const int gap = px(preferences.is_object() ? preferences.value("touch_key_spacing_tenths",60) : 60);
    const int row_gap = px(preferences.is_object() ? preferences.value("touch_row_spacing_tenths",70) : 70);
    const int available_height = std::max(1,height - px(80));
    int id = keyboard_panel_first_key;
    for (size_t row_index = 0; row_index < rows.size(); ++row_index) {
      const auto& row = rows[row_index]; double total = 0; for (const auto& key : row) total += key.weight;
      const int left = px(10), right = std::max(left + 1,width - px(10));
      const int usable = std::max(1,right - left - static_cast<int>((row.size() - 1) * gap / 10));
      const int top = px(42) + static_cast<int>(row_index) * available_height / static_cast<int>(rows.size());
      const int bottom = px(42) + static_cast<int>(row_index + 1) * available_height / static_cast<int>(rows.size()) - row_gap / 10;
      double position = 0;
      for (size_t index = 0; index < row.size(); ++index) {
        const auto before = static_cast<int>(position * usable / total); position += row[index].weight;
        const auto after = static_cast<int>(position * usable / total);
        MoveWindow(get(id++),left + before + static_cast<int>(index * gap / 10),top,std::max(1,after - before),std::max(1,bottom - top),TRUE);
      }
    }
  }
  void draw(const DRAWITEMSTRUCT& item) {
    if (item.CtlType != ODT_BUTTON) return;
    const auto found = keys.find(static_cast<int>(item.CtlID));
    const bool function = found == keys.end() || found->second.modifier || found->second.label.size() > 1;
    const bool selected = (item.itemState & ODS_SELECTED) || (found != keys.end() && keyboard::pressed(found->second,modifiers));
    const auto& look = configuration.look;
    if (shared::keyboard_skin::start()) {
      Gdiplus::Graphics graphics(item.hDC); RECT client{}; GetClientRect(host,&client);
      POINT origin{}; MapWindowPoints(item.hwndItem,host,&origin,1);
      graphics.TranslateTransform(-static_cast<float>(origin.x),-static_cast<float>(origin.y));
      shared::keyboard_skin::background(graphics,Gdiplus::RectF(0,0,static_cast<float>(client.right),static_cast<float>(client.bottom)),look,dpi / 96.0f);
      graphics.ResetTransform();
      shared::keyboard_skin::key(graphics,Gdiplus::RectF(static_cast<float>(item.rcItem.left),static_cast<float>(item.rcItem.top),static_cast<float>(item.rcItem.right - item.rcItem.left),static_cast<float>(item.rcItem.bottom - item.rcItem.top)),look,function,selected,dpi / 96.0f);
    }
    const auto foreground_color = selected || function ? shared::keyboard_skin::readable(selected ? look.accent : look.action) : look.text;
    SetBkMode(item.hDC,TRANSPARENT); SetTextColor(item.hDC,(item.itemState & ODS_DISABLED) ? GetSysColor(COLOR_GRAYTEXT) : RGB((foreground_color >> 16) & 255,(foreground_color >> 8) & 255,foreground_color & 255));
    const auto old = SelectObject(item.hDC,font); auto bounds = item.rcItem; InflateRect(&bounds,-px(2),-px(2));
    const auto title = shared::control_text::wide(shared::control_text::read(item.hwndItem,4096).value_or(""));
    DrawTextW(item.hDC,title.c_str(),static_cast<int>(title.size()),&bounds,DT_CENTER | DT_VCENTER | (title.find(L'\n') == std::wstring::npos ? DT_SINGLELINE : DT_WORDBREAK) | DT_NOPREFIX);
    SelectObject(item.hDC,old);
  }
  static LRESULT CALLBACK proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    auto* state = reinterpret_cast<State*>(GetWindowLongPtrW(window,GWLP_USERDATA));
    if (message == WM_NCCREATE) { state = static_cast<State*>(reinterpret_cast<CREATESTRUCTW*>(lparam)->lpCreateParams); SetWindowLongPtrW(window,GWLP_USERDATA,reinterpret_cast<LONG_PTR>(state)); state->host = window; }
    if (!state) return DefWindowProcW(window,message,wparam,lparam);
    switch (message) {
    case WM_MOUSEACTIVATE: return MA_NOACTIVATE;
    case WM_GETMINMAXINFO: reinterpret_cast<MINMAXINFO*>(lparam)->ptMinTrackSize = {state->px(560),state->px(260)}; return 0;
    case WM_SIZE: state->layout(); return 0;
    case WM_COMMAND: state->command(LOWORD(wparam),HIWORD(wparam),reinterpret_cast<HWND>(lparam)); return 0;
    case WM_DRAWITEM: state->draw(*reinterpret_cast<DRAWITEMSTRUCT*>(lparam)); return TRUE;
    case WM_ERASEBKGND: { RECT area{}; GetClientRect(window,&area); if (shared::keyboard_skin::start()) { Gdiplus::Graphics graphics(reinterpret_cast<HDC>(wparam)); shared::keyboard_skin::background(graphics,Gdiplus::RectF(0,0,static_cast<float>(area.right),static_cast<float>(area.bottom)),state->configuration.look,state->dpi / 96.0f); } return TRUE; }
    case WM_CTLCOLORSTATIC: SetTextColor(reinterpret_cast<HDC>(wparam),state->text_color); SetBkColor(reinterpret_cast<HDC>(wparam),state->background); return reinterpret_cast<LRESULT>(state->background_brush ? state->background_brush : GetSysColorBrush(COLOR_BTNFACE));
    case completed_message: state->complete(); return 0;
    case WM_CLOSE: DestroyWindow(window); return 0;
    case WM_DESTROY: state->worker.reset(); if (state->quit) PostQuitMessage(0); return 0;
    case WM_NCDESTROY: state->host = nullptr; SetWindowLongPtrW(window,GWLP_USERDATA,0); break;
    }
    return DefWindowProcW(window,message,wparam,lparam);
  }
};
KeyboardWindow::KeyboardWindow(Foreground foreground, SendKeys send, OpenSurface open) : state_(std::make_unique<State>(std::move(foreground),std::move(send),std::move(open))) {}
KeyboardWindow::~KeyboardWindow() = default;
HWND KeyboardWindow::handle() const { return state_->host; }
bool KeyboardWindow::create(HINSTANCE instance) {
  auto& state = *state_; state.instance = instance; state.dpi = compat::system_dpi();
  INITCOMMONCONTROLSEX common{sizeof(common),ICC_STANDARD_CLASSES}; if (!InitCommonControlsEx(&common)) return false;
  WNDCLASSEXW type{sizeof(type)}; type.hInstance = instance; type.lpfnWndProc = State::proc; type.lpszClassName = window_class;
  type.hCursor = LoadCursorW(nullptr,IDC_ARROW); type.hbrBackground = nullptr;
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
  const auto frame = panel_frame(1120,400,state.dpi);
  state.host = CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,window_class,L"屏幕键盘",WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_CLIPCHILDREN,
    frame.left,frame.top,frame.right - frame.left,frame.bottom - frame.top,nullptr,nullptr,instance,&state);
  if (!state.host) return false;
  state.font = CreateFontW(-state.px(12),0,0,0,FW_NORMAL,FALSE,FALSE,FALSE,DEFAULT_CHARSET,OUT_DEFAULT_PRECIS,CLIP_DEFAULT_PRECIS,DEFAULT_QUALITY,DEFAULT_PITCH,L"Segoe UI");
  try {
    for (const auto& item : std::vector<std::pair<int,const wchar_t*>>{{keyboard_panel_layout,L"切换九键"},{keyboard_panel_refresh,L"刷新"},{keyboard_panel_voice,L"语音"},{keyboard_panel_settings,L"设置"},{keyboard_panel_close,L"关闭"}}) state.button(item.first,item.second);
    state.controls[keyboard_panel_status] = CreateWindowExW(0,L"STATIC",L"",WS_CHILD | WS_VISIBLE,0,0,1,1,state.host,reinterpret_cast<HMENU>(keyboard_panel_status),instance,nullptr);
    SendMessageW(state.get(keyboard_panel_status),WM_SETFONT,reinterpret_cast<WPARAM>(state.font),FALSE);
    state.rebuild(); state.worker = std::make_unique<Worker>(state.host); state.refresh();
  } catch (const std::exception&) { return false; }
  return true;
}
int KeyboardWindow::run(int show) {
  state_->quit = true; ShowWindow(state_->host,show == SW_HIDE ? SW_HIDE : SW_SHOWNOACTIVATE); MSG message{}; BOOL result;
  while ((result = GetMessageW(&message,nullptr,0,0)) > 0) { TranslateMessage(&message); DispatchMessageW(&message); }
  return result == -1 ? 1 : static_cast<int>(message.wParam);
}
}
