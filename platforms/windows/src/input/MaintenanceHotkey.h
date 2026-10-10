#pragma once

#include "MaintenanceHotkeys.h"

#include <windows.h>

#include <functional>

namespace msime::windows {
// Installs the global maintenance shortcuts on a low-level keyboard hook.
//
// A hook is what the reference uses and what these shortcuts require: they must
// work while another application has focus, which a TSF key sink cannot see.
// The stroke is consumed so the focused application never receives a stray
// digit or letter.
class MaintenanceHotkeyController final {
public:
  // Return true when the action was handled; only then is the stroke consumed.
  using Handler = std::function<bool(MaintenanceHotkey)>;

  // Reports the new Caps Lock state on every press. The Server is the
  // authority for it; the TIP only sampled it at activation.
  using CapsSink = std::function<void(bool)>;
  // 每一次真实的按键按下（不含本进程注入的）都报告一次，悬浮工具栏用它判断用户是否还在打字。钩子回调跑在装钩子的 UI 线程上，所以这里可以直接改 UI 线程的状态，但不能做费时的事。
  using KeySink = std::function<void()>;
  explicit MaintenanceHotkeyController(Handler handler, CapsSink caps = {},
                                       KeySink key = {});
  ~MaintenanceHotkeyController();
  MaintenanceHotkeyController(const MaintenanceHotkeyController &) = delete;
  MaintenanceHotkeyController &operator=(const MaintenanceHotkeyController &) = delete;
  bool installed() const { return hook_ != nullptr; }

private:
  static LRESULT CALLBACK keyboard_proc(int code, WPARAM wparam, LPARAM lparam);
  Handler handler_;
  CapsSink caps_sink_;
  KeySink key_sink_;
  bool caps_ = false;
  // Low-level hooks carry no repeat bit; only the first down after an up toggles.
  bool caps_down_ = false;
  HHOOK hook_ = nullptr;
  static MaintenanceHotkeyController *instance_;
};
} // namespace msime::windows
