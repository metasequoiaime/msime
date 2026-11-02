#pragma once
#include <cstdint>
#include <optional>

#include "../voice/VoiceHotkeyPolicy.h"

namespace msime::windows {
// The global maintenance shortcuts the shared settings page documents.
//
// These are advertised on the 快捷键 page for every platform, and Linux has
// implemented all four for some time. On Windows none of them did anything, so
// a user read four documented shortcuts and got silence from all of them.
enum class MaintenanceAction : uint8_t {
  DeleteCandidate, // Ctrl+Shift+Alt+1..8, index in `slot`
  ClearCache,      // Ctrl+Shift+Alt+C
  Restart,         // Ctrl+Shift+Alt+R
  Stop,            // Ctrl+Shift+Alt+T
  OpenScreenKeyboard, // Ctrl+Shift+Win+K
};
struct MaintenanceHotkey {
  MaintenanceAction action = MaintenanceAction::ClearCache;
  // 0-based candidate slot; meaningful only for DeleteCandidate.
  uint8_t slot = 0;
};
// Decide what a key press means. All four need Ctrl and Shift and Alt together;
// any other modifier combination belongs to the focused application.
//
// `vk` is a Win32 virtual-key code. Both the number row and the numeric keypad
// are accepted, because the digits are the shortcut's whole point and a user
// with a keypad should not be told the feature is missing.
inline std::optional<MaintenanceHotkey>
maintenance_hotkey(uint32_t vk, bool ctrl, bool shift, bool alt,
                   bool win = false) {
  // Ctrl+Shift+Win+K opens the on-screen keyboard. It uses Win rather than Alt,
  // so it is checked before the Alt-based group and must not be claimed by it.
  if (ctrl && shift && win && !alt && vk == 'K')
    return MaintenanceHotkey{MaintenanceAction::OpenScreenKeyboard, 0};
  if (!ctrl || !shift || !alt || win)
    return std::nullopt;
  // 1..8 only: the reference deletes candidates 1 through 8, and 9 and 0 are
  // deliberately not bound.
  if (vk >= '1' && vk <= '8')
    return MaintenanceHotkey{MaintenanceAction::DeleteCandidate,
                             static_cast<uint8_t>(vk - '1')};
  constexpr uint32_t numpad1 = 0x61; // VK_NUMPAD1
  if (vk >= numpad1 && vk <= numpad1 + 7)
    return MaintenanceHotkey{MaintenanceAction::DeleteCandidate,
                             static_cast<uint8_t>(vk - numpad1)};
  if (vk == 'C')
    return MaintenanceHotkey{MaintenanceAction::ClearCache, 0};
  if (vk == 'R')
    return MaintenanceHotkey{MaintenanceAction::Restart, 0};
  if (vk == 'T')
    return MaintenanceHotkey{MaintenanceAction::Stop, 0};
  return std::nullopt;
}

// Whether a key-down may trigger a maintenance shortcut. Strokes this process injected are skipped, or a handler that types something could drive itself. The voice hook's chord replay is the exception: with right Alt held for voice the hook swallows the physical right Alt, so the physical key of Ctrl+Shift+right Alt+R reaches this hook without Alt, and the replay that carries the Alt and the key is the user's own press standing in for it (voice_chord_replay_extra_info).
constexpr bool maintenance_stroke_may_trigger(bool injected,
                                              std::uintptr_t extra_info) {
  return !injected || extra_info == voice_chord_replay_extra_info;
}

// Server 记着的大写锁定状态，语言栏和悬浮工具栏的大写指示以它为准。低级钩子在按键生效之前被调用，读不到这一下之后的锁定状态，只能每按下一次 Caps Lock 翻转一次，所以每一次会改锁定的按下都要计入，注入的也算：TIP 轻点 Caps Lock 切换中英文后用 SendInput 再按一次撤回大写锁定，漏掉那一下，这里就和实际状态一直反着，直到 Server 重启。
struct CapsLockTracker {
  bool on = false;
  // 低级钩子没有自动重复标记：按着不放时一串按下只有第一次翻转。
  bool down = false;
  // Caps Lock 按下。返回 true 表示锁定状态变了，需要报告。
  bool press() {
    if (down)
      return false;
    down = true;
    on = !on;
    return true;
  }
  void release() { down = false; }
};
} // namespace msime::windows
