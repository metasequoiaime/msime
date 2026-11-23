// 候选维护快捷键的解析：Ctrl+Delete 删除候选、Ctrl+↑ 置顶、Ctrl+1..5 固定排位、
// Ctrl+0 取消固定（子线 C）。修饰键必须恰为 Control；其余键一律不动作。

#include "../../src/core/CandidateMaintenanceShortcut.h"

#include <cassert>

int main() {
  using msime::linux_host::candidate_maintenance_shortcut;
  using Kind = msime::linux_host::CandidateShortcutKind;
  constexpr auto control = IBUS_CONTROL_MASK;
  constexpr auto shift = IBUS_SHIFT_MASK;
  constexpr auto alt = IBUS_MOD1_MASK;

  // Ctrl+Delete 与小键盘 Ctrl+KP_Delete → 删除。
  assert(candidate_maintenance_shortcut(IBUS_Delete, control).kind == Kind::Remove);
  assert(candidate_maintenance_shortcut(IBUS_KP_Delete, control).kind == Kind::Remove);

  // Ctrl+↑ 与小键盘 Ctrl+KP_Up → 置顶。
  assert(candidate_maintenance_shortcut(IBUS_Up, control).kind == Kind::Pin);
  assert(candidate_maintenance_shortcut(IBUS_KP_Up, control).kind == Kind::Pin);

  // Ctrl+0 与 Ctrl+KP_0 → 取消固定。
  assert(candidate_maintenance_shortcut(IBUS_0, control).kind == Kind::Clear);
  assert(candidate_maintenance_shortcut(IBUS_KP_0, control).kind == Kind::Clear);

  // Ctrl+1..5 与 Ctrl+KP_1..KP_5 → 固定到对应槽位。
  for (std::uint32_t digit = 1; digit <= 5; ++digit) {
    const auto main_row = candidate_maintenance_shortcut(IBUS_0 + digit, control);
    assert(main_row.kind == Kind::Fix);
    assert(main_row.position == digit);
    const auto keypad = candidate_maintenance_shortcut(IBUS_KP_0 + digit, control);
    assert(keypad.kind == Kind::Fix);
    assert(keypad.position == digit);
  }

  // Ctrl+6..9 不是固定槽位，不动作（数字 6..9 留给别的语义）。
  for (std::uint32_t digit = 6; digit <= 9; ++digit) {
    assert(candidate_maintenance_shortcut(IBUS_0 + digit, control).kind == Kind::None);
    assert(candidate_maintenance_shortcut(IBUS_KP_0 + digit, control).kind == Kind::None);
  }

  // 其他修饰组合不算：Shift、Alt、Control+Shift、Control+Alt 都不动作。
  assert(candidate_maintenance_shortcut(IBUS_Delete, shift).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Delete, alt).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Delete, control | shift).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Delete, control | alt).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Up, control | shift).kind == Kind::None);

  // 不抢既有的组合键：无修饰的 Delete/Up 是光标与选词，Ctrl+BackSpace 和 Ctrl+左右是分段编辑。
  assert(candidate_maintenance_shortcut(IBUS_Delete, 0).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Up, 0).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_BackSpace, control).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Left, control).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Right, control).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Down, control).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_a, control).kind == Kind::None);
}
