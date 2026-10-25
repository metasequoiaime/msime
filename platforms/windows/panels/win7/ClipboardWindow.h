#pragma once

#include <windows.h>
#include <functional>
#include <memory>
#include <string>
#include <string_view>

namespace msime::windows::win7 {
enum ClipboardPanelControl {
  clipboard_panel_entries = 300, clipboard_panel_detail, clipboard_panel_refresh,
  clipboard_panel_previous, clipboard_panel_next, clipboard_panel_page, clipboard_panel_insert,
  clipboard_panel_copy, clipboard_panel_pin, clipboard_panel_remove, clipboard_panel_status,
};
class ClipboardWindow final {
public:
  using InsertText = std::function<bool(std::wstring_view)>;
  using CopyText = std::function<bool(HWND, std::wstring_view)>;
  // 测试可替换系统采集边界；真实历史文件、偏好、分页与 HWND 操作仍运行。
  using Transport = std::function<std::string(const std::string&)>;
  explicit ClipboardWindow(InsertText insert = {}, CopyText copy = {}, Transport transport = {});
  ~ClipboardWindow();
  ClipboardWindow(const ClipboardWindow&) = delete;
  ClipboardWindow& operator=(const ClipboardWindow&) = delete;
  bool create(HINSTANCE instance);
  int run(int show);
  HWND handle() const;
private:
  struct State;
  std::unique_ptr<State> state_;
};
} // namespace msime::windows::win7
