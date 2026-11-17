#pragma once

#include <windows.h>
#include <commctrl.h>
#include <oleacc.h>
#include <algorithm>
#include <string>
#include <utility>

namespace msime::settings::win7 {

// 帮助按钮复用系统气泡：悬停或聚焦可读，点击固定，离开焦点或隐藏页面时收起。
class InfoTip {
 public:
  InfoTip(HWND parent, int id, HFONT font, std::wstring title, std::wstring text)
      : title_(std::move(title)), text_(std::move(text)) {
    button_ = CreateWindowExW(0, L"BUTTON", L"?", WS_CHILD | WS_TABSTOP | BS_PUSHBUTTON | BS_NOTIFY,
        0, 0, 1, 1, parent, reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)),
        reinterpret_cast<HINSTANCE>(GetWindowLongPtrW(parent, GWLP_HINSTANCE)), nullptr);
    if (!button_) return;
    if (SUCCEEDED(CoCreateInstance(CLSID_AccPropServices, nullptr, CLSCTX_INPROC_SERVER,
        IID_IAccPropServices, reinterpret_cast<void**>(&accessibility_)))) {
      const auto name = title_ + L"的说明：" + text_;
      accessibility_->SetHwndPropStr(button_, static_cast<DWORD>(OBJID_CLIENT), CHILDID_SELF, PROPID_ACC_NAME, name.c_str());
    }
    SendMessageW(button_, WM_SETFONT, reinterpret_cast<WPARAM>(font), FALSE);
    tooltip_ = CreateWindowExW(WS_EX_TOPMOST, TOOLTIPS_CLASSW, nullptr, WS_POPUP | TTS_ALWAYSTIP | TTS_NOPREFIX,
        CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, button_, nullptr,
        reinterpret_cast<HINSTANCE>(GetWindowLongPtrW(parent, GWLP_HINSTANCE)), nullptr);
    if (!tooltip_) return;
    tool_.cbSize = sizeof(tool_);
    tool_.uFlags = TTF_TRACK | TTF_ABSOLUTE | TTF_IDISHWND;
    tool_.hwnd = button_;
    tool_.uId = reinterpret_cast<UINT_PTR>(button_);
    tool_.lpszText = text_.data();
    ready_ = SendMessageW(tooltip_, TTM_ADDTOOLW, 0, reinterpret_cast<LPARAM>(&tool_)) != FALSE &&
        SetWindowSubclass(button_, button_proc, 1, reinterpret_cast<DWORD_PTR>(this)) != FALSE;
    SendMessageW(tooltip_, TTM_SETTITLEW, TTI_INFO, reinterpret_cast<LPARAM>(title_.c_str()));
  }

  ~InfoTip() {
    clear_accessibility();
    if (IsWindow(button_)) {
      RemoveWindowSubclass(button_, button_proc, 1);
      DestroyWindow(button_);
    }
    if (IsWindow(tooltip_)) DestroyWindow(tooltip_);
    if (accessibility_) accessibility_->Release();
  }
  InfoTip(const InfoTip&) = delete;
  InfoTip& operator=(const InfoTip&) = delete;

  bool ready() const { return ready_; }
  void hide() {
    pinned_ = false;
    if (IsWindow(tooltip_)) {
      SendMessageW(tooltip_, TTM_TRACKACTIVATE, FALSE, reinterpret_cast<LPARAM>(&tool_));
      SendMessageW(tooltip_, TTM_POP, 0, 0);
      // 跟踪气泡在键盘失焦时可能仍保持显示，显式同步可见状态。
      ShowWindow(tooltip_, SW_HIDE);
    }
  }
  void visible(bool value) {
    if (!value) hide();
    ShowWindow(button_, value ? SW_SHOWNA : SW_HIDE);
  }
  int layout(HWND label, int left, int top, int available, int height, UINT dpi) {
    if (accessibility_ && label_ != label) {
      label_ = label;
      accessibility_->SetHwndPropStr(label_, static_cast<DWORD>(OBJID_CLIENT), CHILDID_SELF, PROPID_ACC_DESCRIPTION, text_.c_str());
    }
    const auto dip = [dpi](int value) { return MulDiv(value, static_cast<int>(dpi), 96); };
    const auto dc = GetDC(label);
    const auto old = SelectObject(dc, reinterpret_cast<HFONT>(SendMessageW(label, WM_GETFONT, 0, 0)));
    SIZE size{};
    GetTextExtentPoint32W(dc, title_.c_str(), static_cast<int>(title_.size()), &size);
    SelectObject(dc, old);
    ReleaseDC(label, dc);
    const int width = std::max(1, std::min(static_cast<int>(size.cx) + dip(28), available - dip(32)));
    MoveWindow(button_, left + width + dip(4), top + std::max(0, (height - dip(24)) / 2), dip(24), dip(24), TRUE);
    SendMessageW(tooltip_, TTM_SETMAXTIPWIDTH, 0, dip(360));
    return width;
  }
  bool command(WPARAM value) {
    if (LOWORD(value) != GetDlgCtrlID(button_) || HIWORD(value) != BN_CLICKED) return false;
    pinned_ = !pinned_;
    if (pinned_) show(); else hide();
    return true;
  }

 private:
  void clear_accessibility() {
    if (!accessibility_) return;
    if (button_) accessibility_->ClearHwndProps(button_, static_cast<DWORD>(OBJID_CLIENT), CHILDID_SELF, &PROPID_ACC_NAME, 1);
    if (label_) accessibility_->ClearHwndProps(label_, static_cast<DWORD>(OBJID_CLIENT), CHILDID_SELF, &PROPID_ACC_DESCRIPTION, 1);
    label_ = nullptr;
  }
  void show() {
    if (!IsWindowVisible(button_)) return;
    RECT rect{};
    GetWindowRect(button_, &rect);
    SendMessageW(tooltip_, TTM_TRACKPOSITION, 0, MAKELPARAM(rect.left, rect.bottom));
    SendMessageW(tooltip_, TTM_TRACKACTIVATE, TRUE, reinterpret_cast<LPARAM>(&tool_));
  }
  static LRESULT CALLBACK button_proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam,
                                      UINT_PTR id, DWORD_PTR data) {
    auto* self = reinterpret_cast<InfoTip*>(data);
    if (message == WM_GETDLGCODE && lparam &&
        reinterpret_cast<const MSG*>(lparam)->wParam == VK_ESCAPE)
      return DefSubclassProc(window, message, wparam, lparam) | DLGC_WANTMESSAGE;
    if (message == WM_SETFOCUS) self->show();
    if (message == WM_KILLFOCUS) self->hide();
    if (message == WM_MOUSEMOVE) {
      TRACKMOUSEEVENT tracking{sizeof(tracking), TME_LEAVE, window, 0};
      TrackMouseEvent(&tracking);
      self->show();
    }
    if (message == WM_MOUSELEAVE && !self->pinned_ && GetFocus() != window) self->hide();
    if (message == WM_KEYDOWN && wparam == VK_ESCAPE) { self->hide(); return 0; }
    if (message == WM_NCDESTROY) {
      self->clear_accessibility();
      self->hide();
      RemoveWindowSubclass(window, button_proc, id);
      self->button_ = nullptr;
    }
    return DefSubclassProc(window, message, wparam, lparam);
  }
  HWND label_ = nullptr;
  IAccPropServices* accessibility_ = nullptr;
  HWND button_ = nullptr;
  HWND tooltip_ = nullptr;
  TOOLINFOW tool_{};
  std::wstring title_;
  std::wstring text_;
  bool ready_ = false;
  bool pinned_ = false;
};

}  // namespace msime::settings::win7
