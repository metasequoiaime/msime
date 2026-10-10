#pragma once
#include <optional>
#include <string_view>

namespace msime::windows {
// 焦点所在的输入框是不是密码框。macOS 在安全输入（密码框、终端的安全键盘输入）期间不打开云剪贴板，连列表都不显示；Windows 没有全局的安全输入状态，只能问焦点控件本身。
enum class FocusedFieldKind { Ordinary, Password, Unknown };
// 经典 Win32 编辑控件的 ES_PASSWORD 样式位（0x20）。这一位只对编辑类控件有这个意思，别的控件类（按钮的 BS_LEFTTEXT 等）用同一位表示别的东西，所以先看类名：Edit、RichEdit20W、RICHEDIT50W、WinForms 的 WindowsForms10.EDIT.* 都带 edit。
inline constexpr unsigned long win32_password_style = 0x20;
inline bool win32_password_edit(std::wstring_view class_name, unsigned long style) {
  if ((style & win32_password_style) == 0)
    return false;
  constexpr std::wstring_view edit = L"edit";
  if (class_name.size() < edit.size())
    return false;
  for (size_t start = 0; start + edit.size() <= class_name.size(); ++start) {
    bool matches = true;
    for (size_t index = 0; index < edit.size() && matches; ++index) {
      wchar_t c = class_name[start + index];
      if (c >= L'A' && c <= L'Z')
        c = static_cast<wchar_t>(c - L'A' + L'a');
      matches = c == edit[index];
    }
    if (matches)
      return true;
  }
  return false;
}
// 两路证据合起来：Win32 样式位说是密码框就是；否则听 UI Automation 的 IsPassword（浏览器、WinUI、WPF 的密码框都这样报告）；UI Automation 没在时限内回答时是未知。
inline FocusedFieldKind focused_field_kind(bool win32_password,
                                           std::optional<bool> automation_password) {
  if (win32_password)
    return FocusedFieldKind::Password;
  if (automation_password)
    return *automation_password ? FocusedFieldKind::Password
                                : FocusedFieldKind::Ordinary;
  return FocusedFieldKind::Unknown;
}
// 只在确知是密码框时拒绝，和 macOS 只在安全输入确实开着时拒绝一样。未知（焦点所在的程序没有及时回答）照常打开：面板打开时拿不到编辑器的话只能复制，不会把内容输入到别处。
inline bool cloud_clipboard_may_open(FocusedFieldKind kind) {
  return kind != FocusedFieldKind::Password;
}
} // namespace msime::windows
