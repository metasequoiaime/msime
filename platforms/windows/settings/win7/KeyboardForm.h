#pragma once

#include "ThemeData.h"
#include "ControlText.h"
#include "../../panels/win7/KeyboardModel.h"
#include "../../compat/WindowsDpi.h"
#include "../../../../shared/contracts/msime_edition.h"
#include <commctrl.h>
#include <windowsx.h>
#include <cmath>
#include <functional>
#include <map>
#include <set>

namespace msime::settings::win7 {
class KeyboardForm {
public:
  enum Control { open = 4300, preview, height, height_value, key_gap, key_value, row_gap, row_value, voice,
    save, reset, themes, handwriting, notice, skin_editor };
  struct Command { bool handled = false, reset = false, theme = false; std::vector<Change> changes; std::optional<std::string> route; };
  int create(const std::function<HWND(const wchar_t*,const wchar_t*,DWORD,int,int,int)>& row) {
    int top = 0;
    const auto add = [&](int id, const wchar_t* kind, const wchar_t* title, DWORD style, int size = 28) {
      auto child = row(kind,title,style,id,top,size); if (id >= 0) controls_[id] = child; top += size + 12; return child;
    };
    add(-1,L"STATIC",L"使用鼠标或触控方式输入文字与快捷按键。",SS_LEFT,40);
    add(open,L"BUTTON",L"打开屏幕键盘",WS_TABSTOP | BS_PUSHBUTTON,32);
    add(preview,L"STATIC",L"屏幕键盘预览",SS_OWNERDRAW | SS_NOTIFY | WS_BORDER,240);
    auto sample = get(preview);
    if (!SetPropW(sample,L"MSIME.Win7.Keyboard.Form",this) || !SetPropW(sample,L"MSIME.Win7.Keyboard.PreviewProc",reinterpret_cast<HANDLE>(GetWindowLongPtrW(sample,GWLP_WNDPROC))) ||
        !SetWindowLongPtrW(sample,GWLP_WNDPROC,reinterpret_cast<LONG_PTR>(preview_proc))) return 0;
    add(-1,L"STATIC",L"尺寸只改变键位外观。左右拖动预览调键距，上下拖动调行距；修改后点击保存。",SS_LEFT,56);
    for (const auto& field : sliders()) {
      add(-1,L"STATIC",field.title,SS_LEFT);
      auto slider = add(field.id,TRACKBAR_CLASSW,field.title,WS_TABSTOP | TBS_AUTOTICKS | TBS_HORZ,40);
      SendMessageW(slider,TBM_SETRANGE,TRUE,MAKELPARAM(field.min,field.max));
      add(field.value,L"STATIC",L"",SS_LEFT);
    }
    add(voice,L"BUTTON",L"在键盘顶部显示语音入口",WS_TABSTOP | BS_AUTOCHECKBOX);
    add(save,L"BUTTON",L"保存屏幕键盘设置",WS_TABSTOP | BS_PUSHBUTTON,32);
    add(reset,L"BUTTON",L"恢复默认：高度、间距和顶部语音入口",WS_TABSTOP | BS_PUSHBUTTON,32);
    add(themes,L"BUTTON",L"外观跟随主题与颜色模式：前往主题",WS_TABSTOP | BS_PUSHBUTTON,32);
    add(skin_editor,L"BUTTON",L"制作与编辑键盘皮肤",WS_TABSTOP | BS_PUSHBUTTON,32);
    if constexpr (MSIME_EDITION_HANDWRITING != 0) add(handwriting,L"BUTTON",L"打开手写识别板",WS_TABSTOP | BS_PUSHBUTTON,32);
    add(notice,L"STATIC",L"",SS_LEFT,56); enable(false); return top;
  }
  void refresh(const Json& preferences) {
    Json current;
    for (const auto& field : sliders()) current[field.key] = preferences.at(field.key);
    current["touch_voice_shortcut"] = preferences.at("touch_voice_shortcut");
    if (loaded_ && current == previous_ && !reset_requested_) return;
    previous_ = std::move(current); loaded_ = true; dirty_.clear(); reset_requested_ = false;
    for (const auto& field : sliders()) SendMessageW(get(field.id),TBM_SETPOS,TRUE,previous_.at(field.key).get<int>());
    SendMessageW(get(voice),BM_SETCHECK,previous_.at("touch_voice_shortcut") == true ? BST_CHECKED : BST_UNCHECKED,0);
    labels(); InvalidateRect(get(preview),nullptr,TRUE); enable(enabled_);
  }
  void enable(bool enabled) { enabled_ = enabled; for (const auto& field : controls_) EnableWindow(field.second,enabled && loaded_); }
  void resetting() { reset_requested_ = true; }
  void scroll(HWND source) {
    if (!enabled_ || !loaded_) return;
    for (const auto& field : sliders()) if (get(field.id) == source) { dirty_.insert(field.id); labels(); InvalidateRect(get(preview),nullptr,TRUE); }
  }
  Command command(WPARAM event) {
    const int id = LOWORD(event); if (!controls_.count(id)) return {};
    Command result; result.handled = true;
    if (HIWORD(event) != BN_CLICKED || !enabled_ || !loaded_ || !IsWindowEnabled(get(id))) return result;
    if (id == open || id == handwriting) result.route = id == open ? "keyboard" : "handwriting";
    else if (id == skin_editor) result.route = "settings:keyboard-skin";
    else if (id == themes) result.theme = true;
    else if (id == reset) result.reset = true;
    else if (id == voice) dirty_.insert(id);
    else if (id == save) {
      for (const auto& field : sliders()) if (dirty_.count(field.id)) result.changes.push_back({{field.key},position(field.id)});
      if (dirty_.count(voice)) result.changes.push_back({{"touch_voice_shortcut"},SendMessageW(get(voice),BM_GETCHECK,0,0) == BST_CHECKED});
    }
    return result;
  }
  static std::vector<Change> reset_changes() {
    std::vector<Change> result; for (const auto* field : {"touch_key_spacing_tenths","touch_row_spacing_tenths","touch_keyboard_height_adjustment","touch_voice_shortcut","touch_toolbar"}) result.push_back({{field},nullptr,true}); return result;
  }
  bool draw(const DRAWITEMSTRUCT& draw) const {
    if (draw.CtlID != preview) return false;
    const auto dc = draw.hDC; const int saved = SaveDC(dc); FillRect(dc,&draw.rcItem,GetSysColorBrush(COLOR_BTNFACE));
    const auto rows = msime::windows::win7::keyboard::full_rows();
    const double scale = std::max(.05,std::min((draw.rcItem.right - draw.rcItem.left - 12) / 1100.0,(draw.rcItem.bottom - draw.rcItem.top - 12) / (360.0 + position(height))));
    const auto font = CreateFontW(-std::max(6,static_cast<int>(12 * scale)),0,0,0,FW_NORMAL,FALSE,FALSE,FALSE,DEFAULT_CHARSET,OUT_DEFAULT_PRECIS,CLIP_DEFAULT_PRECIS,DEFAULT_QUALITY,DEFAULT_PITCH,L"Segoe UI");
    const auto old = SelectObject(dc,font); SetBkMode(dc,TRANSPARENT); SetTextColor(dc,RGB(26,26,26));
    for (size_t index = 0; index < rows.size(); ++index) {
      const auto& row = rows[index]; double total = 0; for (const auto& key : row) total += key.weight;
      const double gap = position(key_gap) / 10.0, gap_y = position(row_gap) / 10.0;
      const double available = 1100 - (row.size() - 1) * gap; double left = 0;
      for (const auto& key : row) {
        const double width = available * key.weight / total;
        RECT area{draw.rcItem.left + 6 + static_cast<LONG>(left * scale),draw.rcItem.top + 6 + static_cast<LONG>(index * 48 * scale),
          draw.rcItem.left + 6 + static_cast<LONG>((left + width) * scale),draw.rcItem.top + 6 + static_cast<LONG>((index * 48 + 48 - gap_y) * scale)};
        FillRect(dc,&area,GetSysColorBrush(key.modifier || key.label.size() > 1 ? COLOR_3DLIGHT : COLOR_WINDOW));
        DrawTextW(dc,key.label.c_str(),static_cast<int>(key.label.size()),&area,DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX); left += width + gap;
      }
    }
    SelectObject(dc,old); DeleteObject(font); RestoreDC(dc,saved); return true;
  }
private:
  struct Slider { int id, value, min, max; const char* key; const wchar_t* title; };
  static const std::array<Slider,3>& sliders() {
    static const std::array<Slider,3> value{{{height,height_value,-12,48,"touch_keyboard_height_adjustment",L"键盘高度"},
      {key_gap,key_value,30,60,"touch_key_spacing_tenths",L"按键间距"},{row_gap,row_value,40,100,"touch_row_spacing_tenths",L"行间距"}}}; return value;
  }
  HWND get(int id) const { const auto found = controls_.find(id); return found == controls_.end() ? nullptr : found->second; }
  int position(int id) const { return static_cast<int>(SendMessageW(get(id),TBM_GETPOS,0,0)); }
  void labels() {
    for (const auto& field : sliders()) { const auto value = position(field.id); const auto text = field.id == height ? (value > 0 ? L"+" : L"") + std::to_wstring(value) + L" dp" : std::to_wstring(value / 10) + L"." + std::to_wstring(value % 10) + L" dp"; SetWindowTextW(get(field.value),text.c_str()); }
  }
  static LRESULT CALLBACK preview_proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    auto* form = static_cast<KeyboardForm*>(GetPropW(window,L"MSIME.Win7.Keyboard.Form"));
    const auto previous = reinterpret_cast<WNDPROC>(GetPropW(window,L"MSIME.Win7.Keyboard.PreviewProc"));
    if (form) switch (message) {
    case WM_LBUTTONDOWN:
      if (form->enabled_ && form->loaded_) { form->origin_ = {GET_X_LPARAM(lparam),GET_Y_LPARAM(lparam)}; form->start_key_ = form->position(key_gap); form->start_row_ = form->position(row_gap); form->axis_ = 0; SetCapture(window); } return 0;
    case WM_MOUSEMOVE:
      if (GetCapture() == window) {
        const auto dpi = msime::windows::compat::window_dpi(window);
        const double dx = (GET_X_LPARAM(lparam) - form->origin_.x) * 96.0 / dpi, dy = (GET_Y_LPARAM(lparam) - form->origin_.y) * 96.0 / dpi;
        if (form->axis_ == 0 && std::max(std::abs(dx),std::abs(dy)) >= 4) form->axis_ = std::abs(dx) >= std::abs(dy) ? 1 : 2;
        if (form->axis_) {
          const auto id = form->axis_ == 1 ? key_gap : row_gap; const auto before = form->axis_ == 1 ? form->start_key_ : form->start_row_;
          const auto value = std::clamp(before + static_cast<int>(std::floor((form->axis_ == 1 ? dx : dy) / 1.8 + .5)),id == key_gap ? 30 : 40,id == key_gap ? 60 : 100);
          SendMessageW(form->get(id),TBM_SETPOS,TRUE,value); form->scroll(form->get(id));
        }
      } return 0;
    case WM_LBUTTONUP: case WM_CANCELMODE: if (GetCapture() == window) ReleaseCapture(); form->axis_ = 0; return 0;
    case WM_NCDESTROY: RemovePropW(window,L"MSIME.Win7.Keyboard.Form"); RemovePropW(window,L"MSIME.Win7.Keyboard.PreviewProc"); break;
    }
    return previous ? CallWindowProcW(previous,window,message,wparam,lparam) : DefWindowProcW(window,message,wparam,lparam);
  }
  std::map<int,HWND> controls_; Json previous_; std::set<int> dirty_;
  bool enabled_ = false, loaded_ = false, reset_requested_ = false; POINT origin_{}; int start_key_ = 0, start_row_ = 0, axis_ = 0;
};
}
