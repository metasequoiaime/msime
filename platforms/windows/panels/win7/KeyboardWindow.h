#pragma once

#include "KeyboardModel.h"
#include <functional>
#include <memory>

namespace msime::windows::win7 {
enum KeyboardPanelControl {
  keyboard_panel_layout = 700, keyboard_panel_refresh, keyboard_panel_status,
  keyboard_panel_voice, keyboard_panel_settings, keyboard_panel_close,
  keyboard_panel_first_key = 1000,
};
class KeyboardWindow final {
public:
  // 只注入系统发送与前台边界，按键建模、真实控件和共享配置继续使用正式路径。
  using Foreground = std::function<HWND()>;
  using SendKeys = std::function<UINT(HWND, const std::vector<INPUT>&)>;
  using OpenSurface = std::function<bool(std::string_view)>;
  explicit KeyboardWindow(Foreground foreground = {}, SendKeys send = {}, OpenSurface open = {});
  ~KeyboardWindow();
  KeyboardWindow(const KeyboardWindow&) = delete;
  KeyboardWindow& operator=(const KeyboardWindow&) = delete;
  bool create(HINSTANCE instance);
  int run(int show);
  HWND handle() const;
private:
  struct State;
  std::unique_ptr<State> state_;
};
}
