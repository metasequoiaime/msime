#pragma once

#include <array>
#include <cstdint>

namespace msime::windows {
// The decisions the voice keyboard hook makes, as pure functions over plain
// values.
//
// The hook itself cannot be exercised off Windows - it is a low-level hook with
// atomic state and posted window messages - and the rules inside it are the sort
// that are wrong in one branch: a modifier that leaks to the application after
// being used as a shortcut, a hold that keeps recording after the user let go,
// an F9 key-up that reaches the editor because only the key-down was swallowed.
// Nothing covered any of it. Deciding here, in code with no Win32 in it, is what
// makes those rules testable on any machine.
//
// This header deliberately has no `#include <windows.h>`: the virtual-key codes
// below are named so the policy is readable, and `VoiceHotkey.cpp` asserts each
// one against the Win32 value it stands for, so a rename or a typo cannot pass.
inline constexpr unsigned voice_key_lcontrol = 0xA2;
inline constexpr unsigned voice_key_rcontrol = 0xA3;
inline constexpr unsigned voice_key_lwin = 0x5B;
inline constexpr unsigned voice_key_rwin = 0x5C;
inline constexpr unsigned voice_key_ralt = 0xA5;
inline constexpr unsigned voice_key_f9 = 0x78;
inline constexpr unsigned voice_key_space = 0x20;
inline constexpr unsigned voice_key_escape = 0x1B;

// 一个物理按键直到松开才结束；改绑、禁用、失焦与修饰键变化都不释放已消费的按键。
class VoiceToggleStrokes {
public:
  struct Event { bool repeat = false; bool consumed = false; };
  Event observe(unsigned key, bool down, bool up) {
    if (key >= pressed_.size()) return {};
    const Event event{down && pressed_[key], consumed_[key]};
    if (down) pressed_[key] = true;
    if (up) { pressed_[key] = false; consumed_[key] = false; }
    return event;
  }
  void claim(unsigned key) { if (key < consumed_.size()) consumed_[key] = true; }
private:
  std::array<bool, 256> pressed_{};
  std::array<bool, 256> consumed_{};
};

// 自定义切换键已由共享层解码，空主键不触发；固定 Ctrl+F9 的旧语义独立保留。
struct VoiceToggleBinding {
  unsigned key = 0;
  bool ctrl = false;
  bool alt = false;
  bool shift = false;
  bool meta = false;
};
constexpr bool voice_toggle_matches(const VoiceToggleBinding &binding, unsigned key,
                                     bool ctrl, bool alt, bool shift, bool meta) {
  return binding.key != 0 && binding.key == key && binding.ctrl == ctrl &&
         binding.alt == alt && binding.shift == shift && binding.meta == meta;
}

enum class VoiceHoldShortcut { None, RAlt, CtrlWin, RCtrlRAlt };

// The five voice shortcut switches, as the settings page writes them.
struct VoiceHotkeyBindings {
  bool ralt = true;
  bool ctrl_f9 = true;
  bool ctrl_win = false;
  bool rctrl_ralt = false;
  bool hold_space_lock = true;
};

// Which physical modifiers the hook has seen go down and not come back up.
struct VoiceModifierState {
  bool ralt = false;
  bool lctrl = false;
  bool rctrl = false;
  bool lwin = false;
  bool rwin = false;

  constexpr bool ctrl() const { return lctrl || rctrl; }
  constexpr bool win() const { return lwin || rwin; }
};

// Fold one key event into the modifier state. A key that is neither a down nor
// an up - the hook sees other messages - leaves the state alone rather than
// guessing, which is what keeps a modifier from being marked released by an
// event that said nothing about it.
constexpr VoiceModifierState voice_modifier_state_after(VoiceModifierState state,
                                                        unsigned key, bool down,
                                                        bool up) {
  if (!down && !up)
    return state;
  const bool pressed = down;
  if (key == voice_key_lcontrol)
    state.lctrl = pressed;
  else if (key == voice_key_rcontrol)
    state.rctrl = pressed;
  else if (key == voice_key_lwin)
    state.lwin = pressed;
  else if (key == voice_key_rwin)
    state.rwin = pressed;
  else if (key == voice_key_ralt)
    state.ralt = pressed;
  return state;
}

// Which hold shortcut this key-down starts, in the order the hook tries them.
//
// The order is the behaviour: with both RCtrl+RAlt and RAlt enabled, pressing
// RAlt while RCtrl is down is the two-key shortcut, not the one-key one. The
// asymmetry between them is also deliberate - RCtrl+RAlt wants the *right*
// control specifically, while Ctrl+Win takes either control.
constexpr VoiceHoldShortcut voice_hold_activation(unsigned key,
                                                  const VoiceHotkeyBindings &bindings,
                                                  const VoiceModifierState &state) {
  if (bindings.rctrl_ralt && key == voice_key_ralt && state.rctrl)
    return VoiceHoldShortcut::RCtrlRAlt;
  if (bindings.ctrl_win && (key == voice_key_lwin || key == voice_key_rwin) &&
      state.ctrl())
    return VoiceHoldShortcut::CtrlWin;
  if (bindings.ralt && key == voice_key_ralt)
    return VoiceHoldShortcut::RAlt;
  return VoiceHoldShortcut::None;
}

// Is an active hold still physically held? Releasing any part of it ends the
// hold; a two-key shortcut does not survive losing one of its keys.
constexpr bool voice_hold_held(VoiceHoldShortcut shortcut,
                               const VoiceModifierState &state) {
  switch (shortcut) {
  case VoiceHoldShortcut::RAlt:
    return state.ralt;
  case VoiceHoldShortcut::CtrlWin:
    return state.ctrl() && state.win();
  case VoiceHoldShortcut::RCtrlRAlt:
    return state.rctrl && state.ralt;
  case VoiceHoldShortcut::None:
    break;
  }
  return false;
}

// A modifier used to start a shortcut must not also reach the application, or
// RAlt opens a menu and Win opens the Start menu every time the user dictates.
// The latch is held until that key comes back up, because the application would
// otherwise see an unmatched release.
constexpr bool voice_hold_suppresses_ralt(VoiceHoldShortcut shortcut) {
  return shortcut == VoiceHoldShortcut::RAlt ||
         shortcut == VoiceHoldShortcut::RCtrlRAlt;
}

constexpr bool voice_hold_suppresses_win(VoiceHoldShortcut shortcut) {
  return shortcut == VoiceHoldShortcut::CtrlWin;
}

// Does this key carry a suppression latch right now?
constexpr bool voice_key_is_suppressed(unsigned key, bool ralt_latched,
                                       bool win_latched) {
  if (key == voice_key_ralt)
    return ralt_latched;
  if (key == voice_key_lwin || key == voice_key_rwin)
    return win_latched;
  return false;
}

// The key-up that clears a latch is the same key that set it.
constexpr bool voice_key_clears_ralt_latch(unsigned key, bool up) {
  return up && key == voice_key_ralt;
}

constexpr bool voice_key_clears_win_latch(unsigned key, bool up) {
  return up && (key == voice_key_lwin || key == voice_key_rwin);
}

// Space locks a hold so the user can stop holding and keep dictating, and is
// swallowed so the space never reaches the editor. Only while a hold is active
// and only when the setting is on; otherwise space is an ordinary space.
constexpr bool voice_space_locks_hold(unsigned key, VoiceHoldShortcut active,
                                      const VoiceHotkeyBindings &bindings) {
  return key == voice_key_space && active != VoiceHoldShortcut::None &&
         bindings.hold_space_lock;
}

// Escape cancels while recording, and is swallowed either way: an Escape that
// stopped dictation must not also close the dialog behind it.
constexpr bool voice_escape_cancels(unsigned key, bool recording) {
  return key == voice_key_escape && recording;
}
} // namespace msime::windows
