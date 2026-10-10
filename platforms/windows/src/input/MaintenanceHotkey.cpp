#include "MaintenanceHotkey.h"

namespace msime::windows {
MaintenanceHotkeyController *MaintenanceHotkeyController::instance_ = nullptr;

MaintenanceHotkeyController::MaintenanceHotkeyController(Handler handler,
                                                         CapsSink caps,
                                                         KeySink key)
    : handler_(std::move(handler)), caps_sink_(std::move(caps)),
      key_sink_(std::move(key)),
      // Seed from the OS so the first report is an actual change, not the
      // state the session already started in.
      caps_((GetKeyState(VK_CAPITAL) & 1) != 0) {
  if (!handler_)
    return;
  instance_ = this;
  hook_ = SetWindowsHookExW(WH_KEYBOARD_LL,
                            &MaintenanceHotkeyController::keyboard_proc,
                            GetModuleHandleW(nullptr), 0);
  // A hook that cannot be installed leaves the shortcuts inert. It is not
  // worth failing the Server over: every other input path still works.
  if (!hook_ && instance_ == this)
    instance_ = nullptr;
}

MaintenanceHotkeyController::~MaintenanceHotkeyController() {
  if (hook_)
    UnhookWindowsHookEx(hook_);
  hook_ = nullptr;
  if (instance_ == this)
    instance_ = nullptr;
}

LRESULT CALLBACK MaintenanceHotkeyController::keyboard_proc(int code,
                                                           WPARAM wparam,
                                                           LPARAM lparam) {
  auto *self = instance_;
  if (code != HC_ACTION || !self || !self->handler_)
    return CallNextHookEx(nullptr, code, wparam, lparam);
  if (wparam == WM_KEYUP || wparam == WM_SYSKEYUP) {
    const auto *up = reinterpret_cast<const KBDLLHOOKSTRUCT *>(lparam);
    if (up && up->vkCode == VK_CAPITAL)
      self->caps_down_ = false;
    return CallNextHookEx(nullptr, code, wparam, lparam);
  }
  if (wparam != WM_KEYDOWN && wparam != WM_SYSKEYDOWN)
    return CallNextHookEx(nullptr, code, wparam, lparam);
  const auto *event = reinterpret_cast<const KBDLLHOOKSTRUCT *>(lparam);
  if (!event)
    return CallNextHookEx(nullptr, code, wparam, lparam);
  // Do not act on strokes this process injected, or a handler that types
  // something could drive itself.
  if (event->flags & LLKHF_INJECTED)
    return CallNextHookEx(nullptr, code, wparam, lparam);
  if (self->key_sink_) {
    try {
      self->key_sink_();
    } catch (...) {
    }
  }
  // Caps Lock is observed, not claimed: the Server is the authority for the
  // indicator, and the TIP only sampled GetKeyState at activation, so pressing
  // Caps mid-session left the language-bar icon stale. The stroke is always
  // passed on - Caps Lock still has to work.
  // Holding Caps auto-repeats key-downs that do not toggle it again.
  if (event->vkCode == VK_CAPITAL && !(event->flags & LLKHF_UP) &&
      !self->caps_down_) {
    self->caps_down_ = true;
    self->caps_ = !self->caps_;
    if (self->caps_sink_) {
      try {
        self->caps_sink_(self->caps_);
      } catch (...) {
      }
    }
  }
  const bool ctrl = (GetAsyncKeyState(VK_CONTROL) & 0x8000) != 0;
  const bool shift = (GetAsyncKeyState(VK_SHIFT) & 0x8000) != 0;
  const bool alt = (GetAsyncKeyState(VK_MENU) & 0x8000) != 0;
  const bool win = (GetAsyncKeyState(VK_LWIN) & 0x8000) != 0 ||
                   (GetAsyncKeyState(VK_RWIN) & 0x8000) != 0;
  const auto hotkey = maintenance_hotkey(event->vkCode, ctrl, shift, alt, win);
  if (!hotkey)
    return CallNextHookEx(nullptr, code, wparam, lparam);
  bool handled = false;
  try {
    handled = self->handler_(*hotkey);
  } catch (...) {
    handled = false;
  }
  // Consume only what was actually handled. Swallowing a stroke the Server
  // ignored would delete a keypress from the focused application for nothing.
  if (handled)
    return 1;
  return CallNextHookEx(nullptr, code, wparam, lparam);
}
} // namespace msime::windows
