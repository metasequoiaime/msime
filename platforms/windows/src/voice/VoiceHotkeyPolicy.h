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
inline constexpr unsigned voice_key_shift = 0x10;
inline constexpr unsigned voice_key_control = 0x11;
inline constexpr unsigned voice_key_menu = 0x12;
inline constexpr unsigned voice_key_lshift = 0xA0;
inline constexpr unsigned voice_key_rshift = 0xA1;
inline constexpr unsigned voice_key_lalt = 0xA4;

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

// A modifier pressed during a hold is not the key that makes it a chord: the user is still building the combination, and the modifier reaches the application as it always has.
constexpr bool voice_key_is_modifier(unsigned key) {
  return key == voice_key_shift || key == voice_key_control ||
         key == voice_key_menu || key == voice_key_lshift ||
         key == voice_key_rshift || key == voice_key_lcontrol ||
         key == voice_key_rcontrol || key == voice_key_lalt ||
         key == voice_key_ralt || key == voice_key_lwin ||
         key == voice_key_rwin;
}

// A right-Alt hold that meets another key was a shortcut, not dictation (#10518): right Alt+F10 in PowerPoint reached the application as a bare F10 because the hold had swallowed the Alt. Such a key ends the hold, the recording that hold started is dropped (see voice_chord_cancels_recording), and the hook hands the application right Alt and then this key, in that order.
//
// Only a fresh physical press of a non-modifier key does it. A key-up, an auto-repeat of a key already down before the hold began, and input another program injected do not. Space while space-lock is on and Escape keep their own meanings, and a hold the user has locked with space is dictation they asked to keep. Ctrl+Win is not included: it swallows Win, not Alt, and is left as it is.
constexpr bool voice_hold_becomes_chord(unsigned key, bool down, bool repeat,
                                        bool injected, VoiceHoldShortcut active,
                                        const VoiceHotkeyBindings &bindings,
                                        bool locked) {
  if (!down || repeat || injected || locked)
    return false;
  if (!voice_hold_suppresses_ralt(active))
    return false;
  if (voice_key_is_modifier(key) || key == voice_key_escape)
    return false;
  return !voice_space_locks_hold(key, active, bindings);
}

// A chord drops only the recording its own hold started. `hold_session` is the session epoch that hold's start produced, 0 when it started nothing: a dictation was already running (Ctrl+F9, a custom toggle hotkey, the toolbar microphone, a settings review capture), the hold stopped a locked dictation, or the start failed. Any other recording keeps running, because the user pressed a shortcut and did not ask to end that dictation; before #10518 its right Alt release would have stopped and submitted it, and cancelling it instead would throw the audio away. A changed epoch means the hold's recording already ended and something else started a new one, and a locked recording is one the user asked to keep.
constexpr bool voice_chord_cancels_recording(std::uint64_t hold_session,
                                             std::uint64_t current_session,
                                             bool recording, bool locked) {
  return hold_session != 0 && hold_session == current_session && recording &&
         !locked;
}

// After a chord the user is still holding right Alt, and its auto-repeat keeps arriving as fresh key-downs. Those must not start a new hold, or the second repeat would begin recording and swallow the Alt the application now holds. The latch ends with right Alt's release.
constexpr bool voice_hold_may_activate(unsigned key, bool ralt_chord_latched) {
  return !(key == voice_key_ralt && ralt_chord_latched);
}
} // namespace msime::windows
