#include "VoiceHotkey.h"

#include <windows.h>

#include <utility>
#include "../../../../shared/contracts/msime_edition.h"

namespace msime::windows {
// The policy header names these so it can be compiled and tested without
// <windows.h>. Here, where the real header is in scope, each one is held to the
// value it stands for: a rename or a typo cannot pass.
static_assert(voice_key_lcontrol == VK_LCONTROL);
static_assert(voice_key_rcontrol == VK_RCONTROL);
static_assert(voice_key_lwin == VK_LWIN);
static_assert(voice_key_rwin == VK_RWIN);
static_assert(voice_key_ralt == VK_RMENU);
static_assert(voice_key_f9 == VK_F9);
static_assert(voice_key_space == VK_SPACE);
static_assert(voice_key_escape == VK_ESCAPE);

namespace {
constexpr wchar_t kClassName[] = L"MSIMEClientVoiceHotkeyWindow" MSIME_EDITION_NAME_SUFFIX;
constexpr UINT kStartMessage = WM_APP + 200;
constexpr UINT kToggleMessage = WM_APP + 201;
constexpr UINT kStopMessage = WM_APP + 202;
constexpr UINT kLockMessage = WM_APP + 203;
constexpr UINT kCancelMessage = WM_APP + 204;

// The low-level hook swallows the RAlt key-up after using it as a voice
// shortcut. If the controller is torn down before that key-up arrives (for
// example while settings disable the hook or while Server exits), Windows
// would otherwise retain the modifier as logically pressed for the focused
// application. Match the native voice service's shutdown safety net.
void force_release_ralt() {
  INPUT input{};
  input.type = INPUT_KEYBOARD;
  input.ki.wVk = VK_RMENU;
  input.ki.dwFlags = KEYEVENTF_KEYUP;
  (void)SendInput(1, &input, sizeof(INPUT));
}
}

VoiceHotkeyController *VoiceHotkeyController::instance_ = nullptr;

VoiceHotkeyController::VoiceHotkeyController(VoiceInputSession &voice,
                                             ConfigProvider config,
                                             ActiveProvider active)
    : voice_(voice), config_provider_(std::move(config)),
      active_provider_(std::move(active)) {
  if (instance_)
    return;
  WNDCLASSW klass{};
  klass.lpfnWndProc = &VoiceHotkeyController::window_proc;
  klass.hInstance = GetModuleHandleW(nullptr);
  klass.lpszClassName = kClassName;
  RegisterClassW(&klass);
  window_ = CreateWindowExW(0, kClassName, L"", 0, 0, 0, 0, 0,
                            HWND_MESSAGE, nullptr, klass.hInstance, this);
  if (window_) {
    instance_ = this;
    hook_ = SetWindowsHookExW(WH_KEYBOARD_LL,
                              &VoiceHotkeyController::keyboard_proc,
                              klass.hInstance, 0);
  }
}

VoiceHotkeyController::~VoiceHotkeyController() {
  const bool release_ralt = suppress_ralt_until_up_.load();
  if (hook_)
    UnhookWindowsHookEx(hook_);
  hook_ = nullptr;
  if (instance_ == this)
    instance_ = nullptr;
  if (window_)
    DestroyWindow(window_);
  window_ = nullptr;
  reset_state();
  if (release_ralt)
    force_release_ralt();
}

void VoiceHotkeyController::refresh() {
  const auto config = config_provider_();
  const bool changed =
      !observed_config_ || observed_enabled_ != config.enabled ||
      observed_hotkey_ralt_ != config.hotkey_ralt ||
      observed_hotkey_ctrl_f9_ != config.hotkey_ctrl_f9 ||
      observed_hotkey_ctrl_win_ != config.hotkey_ctrl_win ||
      observed_hotkey_rctrl_ralt_ != config.hotkey_rctrl_ralt ||
      observed_hotkey_hold_space_lock_ != config.hotkey_hold_space_lock;
  if (!changed)
    return;
  observed_config_ = true;
  observed_enabled_ = config.enabled;
  observed_hotkey_ralt_ = config.hotkey_ralt;
  observed_hotkey_ctrl_f9_ = config.hotkey_ctrl_f9;
  observed_hotkey_ctrl_win_ = config.hotkey_ctrl_win;
  observed_hotkey_rctrl_ralt_ = config.hotkey_rctrl_ralt;
  observed_hotkey_hold_space_lock_ = config.hotkey_hold_space_lock;
  const bool release_ralt = suppress_ralt_until_up_.load();
  if (active_hold_.load() != VoiceHoldShortcut::None ||
      (!config.enabled && voice_.recording()))
    voice_.stop();
  reset_state();
  if (release_ralt)
    force_release_ralt();
}

LRESULT CALLBACK VoiceHotkeyController::window_proc(HWND hwnd, UINT message,
                                                      WPARAM wparam,
                                                      LPARAM lparam) {
  auto *self = reinterpret_cast<VoiceHotkeyController *>(
      GetWindowLongPtrW(hwnd, GWLP_USERDATA));
  if (message == WM_NCCREATE) {
    const auto *create = reinterpret_cast<CREATESTRUCTW *>(lparam);
    self = static_cast<VoiceHotkeyController *>(create->lpCreateParams);
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(self));
  }
  return self ? self->handle_window(hwnd, message, wparam, lparam)
              : DefWindowProcW(hwnd, message, wparam, lparam);
}

LRESULT VoiceHotkeyController::handle_window(HWND hwnd, UINT message,
                                               WPARAM wparam, LPARAM lparam) {
  (void)wparam;
  (void)lparam;
  switch (message) {
  case kStartMessage:
    if (!voice_.recording())
      voice_.toggle();
    return 0;
  case kToggleMessage:
    voice_.toggle();
    return 0;
  case kStopMessage:
    voice_.stop();
    return 0;
  case kLockMessage:
    voice_.lock();
    return 0;
  case kCancelMessage:
    voice_.cancel();
    cancel_posted_ = false;
    return 0;
  default:
    return DefWindowProcW(hwnd, message, wparam, lparam);
  }
}

VoiceModifierState VoiceHotkeyController::modifiers() const {
  return VoiceModifierState{ralt_pressed_.load(), lctrl_pressed_.load(),
                            rctrl_pressed_.load(), lwin_pressed_.load(),
                            rwin_pressed_.load()};
}

void VoiceHotkeyController::activate(VoiceHoldShortcut shortcut) {
  active_hold_.store(shortcut);
  cancel_posted_.store(false);
  if (voice_hold_suppresses_ralt(shortcut))
    suppress_ralt_until_up_.store(true);
  else if (voice_hold_suppresses_win(shortcut))
    suppress_win_until_up_.store(true);
  if (window_)
    PostMessageW(window_, voice_.locked() ? kStopMessage : kStartMessage, 0,
                 0);
}

void VoiceHotkeyController::reset_state() {
  ralt_pressed_ = false;
  lctrl_pressed_ = false;
  rctrl_pressed_ = false;
  lwin_pressed_ = false;
  rwin_pressed_ = false;
  f9_pressed_ = false;
  ctrl_f9_consumed_ = false;
  cancel_posted_ = false;
  suppress_ralt_until_up_ = false;
  suppress_win_until_up_ = false;
  active_hold_ = VoiceHoldShortcut::None;
}

LRESULT CALLBACK VoiceHotkeyController::keyboard_proc(int code, WPARAM wparam,
                                                        LPARAM lparam) {
  if (code != HC_ACTION || !instance_)
    return CallNextHookEx(nullptr, code, wparam, lparam);
  auto &self = *instance_;
  const auto *key = reinterpret_cast<KBDLLHOOKSTRUCT *>(lparam);
  if (!key)
    return CallNextHookEx(self.hook_, code, wparam, lparam);
  const bool down = wparam == WM_KEYDOWN || wparam == WM_SYSKEYDOWN;
  const bool up = wparam == WM_KEYUP || wparam == WM_SYSKEYUP;
  const auto config = self.config_provider_();
  const VoiceHotkeyBindings bindings{config.hotkey_ralt, config.hotkey_ctrl_f9,
                                     config.hotkey_ctrl_win,
                                     config.hotkey_rctrl_ralt,
                                     config.hotkey_hold_space_lock};
  const auto active_before = self.active_hold_.load();

  const auto modifiers =
      voice_modifier_state_after(self.modifiers(), key->vkCode, down, up);
  self.ralt_pressed_.store(modifiers.ralt);
  self.lctrl_pressed_.store(modifiers.lctrl);
  self.rctrl_pressed_.store(modifiers.rctrl);
  self.lwin_pressed_.store(modifiers.lwin);
  self.rwin_pressed_.store(modifiers.rwin);

  if (!config.enabled || !self.active_provider_()) {
    self.active_hold_ = VoiceHoldShortcut::None;
    if (self.voice_.recording() && !self.cancel_posted_.exchange(true))
      PostMessageW(self.window_, kCancelMessage, 0, 0);
    const bool suppressed =
        voice_key_is_suppressed(key->vkCode, self.suppress_ralt_until_up_.load(),
                                self.suppress_win_until_up_.load());
    if (voice_key_clears_ralt_latch(key->vkCode, up))
      self.suppress_ralt_until_up_ = false;
    if (voice_key_clears_win_latch(key->vkCode, up))
      self.suppress_win_until_up_ = false;
    return suppressed ? 1 : CallNextHookEx(self.hook_, code, wparam, lparam);
  }

  if (key->vkCode == VK_F9 && config.hotkey_ctrl_f9) {
    if (down && !self.f9_pressed_.exchange(true) && modifiers.ctrl()) {
      self.ctrl_f9_consumed_ = true;
      PostMessageW(self.window_, kToggleMessage, 0, 0);
      return 1;
    }
    if (up) {
      self.f9_pressed_ = false;
      if (self.ctrl_f9_consumed_.exchange(false))
        return 1;
    }
  } else if (key->vkCode == VK_F9 && up) {
    self.f9_pressed_ = false;
    self.ctrl_f9_consumed_ = false;
  }

  if (active_before == VoiceHoldShortcut::None && down) {
    const auto activated = voice_hold_activation(key->vkCode, bindings, modifiers);
    if (activated != VoiceHoldShortcut::None)
      self.activate(activated);
  } else if (active_before != VoiceHoldShortcut::None && up &&
             !voice_hold_held(active_before, modifiers)) {
    self.active_hold_ = VoiceHoldShortcut::None;
    if (!self.voice_.locked())
      PostMessageW(self.window_, kStopMessage, 0, 0);
  }

  const auto active_now = self.active_hold_.load();
  if (voice_space_locks_hold(key->vkCode, active_now, bindings)) {
    if (down && !self.voice_.locked())
      PostMessageW(self.window_, kLockMessage, 0, 0);
    return 1;
  }
  if (voice_escape_cancels(key->vkCode, self.voice_.recording())) {
    if (down)
      PostMessageW(self.window_, kCancelMessage, 0, 0);
    return 1;
  }

  const bool suppressed =
      voice_key_is_suppressed(key->vkCode, self.suppress_ralt_until_up_.load(),
                              self.suppress_win_until_up_.load());
  if (voice_key_clears_ralt_latch(key->vkCode, up))
    self.suppress_ralt_until_up_ = false;
  if (voice_key_clears_win_latch(key->vkCode, up))
    self.suppress_win_until_up_ = false;
  return suppressed ? 1 : CallNextHookEx(self.hook_, code, wparam, lparam);
}
} // namespace msime::windows
