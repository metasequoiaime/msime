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
static_assert(voice_key_shift == VK_SHIFT);
static_assert(voice_key_control == VK_CONTROL);
static_assert(voice_key_menu == VK_MENU);
static_assert(voice_key_lshift == VK_LSHIFT);
static_assert(voice_key_rshift == VK_RSHIFT);
static_assert(voice_key_lalt == VK_LMENU);

namespace {
constexpr wchar_t kClassName[] = L"MSIMEClientVoiceHotkeyWindow" MSIME_EDITION_NAME_SUFFIX;
constexpr UINT kStartMessage = WM_APP + 200;
constexpr UINT kToggleMessage = WM_APP + 201;
constexpr UINT kStopMessage = WM_APP + 202;
constexpr UINT kLockMessage = WM_APP + 203;
constexpr UINT kCancelMessage = WM_APP + 204;
// A hold that became a chord drops the recording that hold started, and nothing else; see voice_chord_cancels_recording. Unlike kCancelMessage it leaves alone a dictation started another way (Ctrl+F9, the toolbar microphone) and, when the hold started nothing, an earlier dictation's recognition still in flight.
constexpr UINT kChordCancelMessage = WM_APP + 205;
// See voice_chord_replay_extra_info.
constexpr ULONG_PTR kChordReplayExtraInfo = voice_chord_replay_extra_info;

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
  // After a chord the application holds the right Alt this hook replayed, and the user is still holding the physical key. The latch is what keeps that key's auto-repeat from starting a new hold that would swallow its release, so it survives the reset: the release still reaches this hook (the hook stays installed) and the application, and clears the latch there.
  const bool ralt_chord = ralt_chord_until_up_.load();
  if (active_hold_.load() != VoiceHoldShortcut::None ||
      (!config.enabled && voice_.recording()))
    voice_.stop();
  reset_state();
  ralt_chord_until_up_ = ralt_chord;
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
    hold_session_ = 0;
    if (!voice_.recording()) {
      voice_.toggle();
      if (voice_.recording())
        hold_session_ = voice_.session();
    }
    return 0;
  case kToggleMessage:
    if (config_provider_().enabled && active_provider_()) voice_.toggle();
    return 0;
  case kStopMessage:
    voice_.stop();
    hold_session_ = 0;
    return 0;
  case kLockMessage:
    voice_.lock();
    return 0;
  case kCancelMessage:
    voice_.cancel();
    cancel_posted_ = false;
    return 0;
  case kChordCancelMessage:
    if (voice_chord_cancels_recording(hold_session_, voice_.session(),
                                      voice_.recording(), voice_.locked()))
      voice_.cancel();
    hold_session_ = 0;
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
  hold_locked_.store(false);
  if (voice_hold_suppresses_ralt(shortcut))
    suppress_ralt_until_up_.store(true);
  else if (voice_hold_suppresses_win(shortcut))
    suppress_win_until_up_.store(true);
  if (window_)
    PostMessageW(window_, voice_.locked() ? kStopMessage : kStartMessage, 0,
                 0);
}

// The hold swallowed right Alt's press, so the application has not seen it. Swallow this key too and replay both, right Alt first: letting this key through and injecting right Alt after it would put the Alt behind the key it modifies. The releases that follow are not swallowed and reach the application as they are.
void VoiceHotkeyController::replay_as_ralt_chord(const KBDLLHOOKSTRUCT &key) {
  active_hold_.store(VoiceHoldShortcut::None);
  suppress_ralt_until_up_.store(false);
  ralt_chord_until_up_.store(true);
  hold_locked_.store(false);
  if (window_)
    PostMessageW(window_, kChordCancelMessage, 0, 0);
  INPUT inputs[2]{};
  inputs[0].type = INPUT_KEYBOARD;
  inputs[0].ki.wVk = VK_RMENU;
  inputs[0].ki.wScan = static_cast<WORD>(MapVirtualKeyW(VK_RMENU, MAPVK_VK_TO_VSC));
  inputs[0].ki.dwFlags = KEYEVENTF_EXTENDEDKEY;
  inputs[0].ki.dwExtraInfo = kChordReplayExtraInfo;
  inputs[1].type = INPUT_KEYBOARD;
  inputs[1].ki.wVk = static_cast<WORD>(key.vkCode);
  inputs[1].ki.wScan = static_cast<WORD>(key.scanCode);
  inputs[1].ki.dwFlags = (key.flags & LLKHF_EXTENDED) ? KEYEVENTF_EXTENDEDKEY : 0;
  inputs[1].ki.dwExtraInfo = kChordReplayExtraInfo;
  (void)SendInput(2, inputs, sizeof(INPUT));
}

void VoiceHotkeyController::reset_state() {
  ralt_pressed_ = false;
  lctrl_pressed_ = false;
  rctrl_pressed_ = false;
  lwin_pressed_ = false;
  rwin_pressed_ = false;
  cancel_posted_ = false;
  suppress_ralt_until_up_ = false;
  suppress_win_until_up_ = false;
  active_hold_ = VoiceHoldShortcut::None;
  hold_locked_ = false;
  ralt_chord_until_up_ = false;
}

LRESULT CALLBACK VoiceHotkeyController::keyboard_proc(int code, WPARAM wparam,
                                                        LPARAM lparam) {
  if (code != HC_ACTION || !instance_)
    return CallNextHookEx(nullptr, code, wparam, lparam);
  auto &self = *instance_;
  const auto *key = reinterpret_cast<KBDLLHOOKSTRUCT *>(lparam);
  if (!key)
    return CallNextHookEx(self.hook_, code, wparam, lparam);
  if (key->dwExtraInfo == kChordReplayExtraInfo)
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
  if (up && key->vkCode == VK_RMENU)
    self.ralt_chord_until_up_ = false;

  const auto toggle_stroke = self.toggle_strokes_.observe(key->vkCode, down, up);
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
    return suppressed || toggle_stroke.consumed ? 1 : CallNextHookEx(self.hook_, code, wparam, lparam);
  }
  if (toggle_stroke.consumed) return 1;

  const bool custom_toggle = voice_toggle_matches(config.toggle_hotkey, key->vkCode,
      modifiers.ctrl(), (GetAsyncKeyState(VK_MENU) & 0x8000) != 0,
      (GetAsyncKeyState(VK_SHIFT) & 0x8000) != 0, modifiers.win());
  const bool fixed_toggle = key->vkCode == VK_F9 && config.hotkey_ctrl_f9 && modifiers.ctrl();
  if (down && !toggle_stroke.repeat && (custom_toggle || fixed_toggle)) {
    self.toggle_strokes_.claim(key->vkCode);
    PostMessageW(self.window_, kToggleMessage, 0, 0);
    return 1;
  }

  const bool injected = (key->flags & LLKHF_INJECTED) != 0;
  if (voice_hold_becomes_chord(key->vkCode, down, toggle_stroke.repeat, injected,
                               active_before, bindings, self.hold_locked_.load())) {
    self.replay_as_ralt_chord(*key);
    return 1;
  }

  if (active_before == VoiceHoldShortcut::None && down &&
      voice_hold_may_activate(key->vkCode, self.ralt_chord_until_up_.load())) {
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
    if (down && !self.voice_.locked()) {
      self.hold_locked_ = true;
      PostMessageW(self.window_, kLockMessage, 0, 0);
    }
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
