#include "input/ModifierHotkeyPolicy.h"

#include <cassert>

int main() {
  using namespace msime::windows;

  // RDP 切回后残留的 Ctrl/Win 物理位不应阻止一次新的单按 Shift。
  assert(modifier_hotkey_allows_bare(
      kModifierHotkeyShift | kModifierHotkeyControl | kModifierHotkeyWin,
      kModifierHotkeyControl | kModifierHotkeyWin,
      kModifierHotkeyShift));

  // 焦点恢复后实际重新按下 Ctrl，它必须再次参与组合键判定。
  assert(!modifier_hotkey_allows_bare(
      kModifierHotkeyShift | kModifierHotkeyControl, 0,
      kModifierHotkeyShift));

  // 只保留当前修饰键时仍然允许单按切换。
  assert(modifier_hotkey_allows_bare(kModifierHotkeyShift, 0,
                                     kModifierHotkeyShift));
}
