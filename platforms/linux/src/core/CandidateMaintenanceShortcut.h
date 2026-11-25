// 候选维护快捷键的解析（iBus）：Ctrl+Delete 删除候选、Ctrl+↑ 置顶、Ctrl+0 取消固定，
// 固定到第 1..5 位用 Ctrl+Shift+1..5（Ctrl+1..5 已让给别的快捷键，主键盘与小键盘等价）。
// 动作落到高亮渲染行（ClientEngine 遍历 rendered_candidates 里 highlighted 的行，
// 围栏与面板属性菜单同一组），本头只负责按键 → 动作的纯解析。
// 删除、置顶、取消固定的修饰键必须恰为 Control，固定排位恰为 Control+Shift（调用方已用
// canonical_modifiers 规范化并排除 release 事件）。美式布局的 Shift+1..5 打出 ! @ # $ %，
// 其它布局靠物理键码 2..6 认数字行；没有键码时按 keysym 认数字、小键盘与美式 Shift 符号。
// Ctrl+6..9、Ctrl+BackSpace、Ctrl+左右这些既有语义的键一律不动作。

#pragma once

#include <ibus.h>

#include <cstdint>

namespace msime::linux_host {

enum class CandidateShortcutKind {
  None,
  Remove,
  Pin,
  Fix,
  Clear,
};

struct CandidateShortcut {
  CandidateShortcutKind kind = CandidateShortcutKind::None;
  // Fix 时是目标槽位 1..5，其余为 0。
  std::uint8_t position = 0;
};

inline CandidateShortcut candidate_maintenance_shortcut(guint key, guint modifiers,
                                                         guint keycode = 0) {
  CandidateShortcut action;
  if (modifiers == IBUS_CONTROL_MASK) {
    if (key == IBUS_Delete || key == IBUS_KP_Delete)
      action.kind = CandidateShortcutKind::Remove;
    else if (key == IBUS_Up || key == IBUS_KP_Up)
      action.kind = CandidateShortcutKind::Pin;
    else if (key == IBUS_0 || key == IBUS_KP_0)
      action.kind = CandidateShortcutKind::Clear;
    return action;
  }
  if (modifiers != (IBUS_CONTROL_MASK | IBUS_SHIFT_MASK))
    return action;

  // 物理数字行 1..5 的 evdev 键码是 2..6，布局换过符号也照认；合成事件没有键码时
  // 退回 keysym：数字、小键盘数字，以及美式布局 Shift 出来的 ! @ # $ %。
  unsigned position = 0;
  if (keycode >= 2 && keycode <= 6) {
    position = keycode - 1;
  } else if (key >= IBUS_1 && key <= IBUS_5) {
    position = key - IBUS_0;
  } else if (key >= IBUS_KP_1 && key <= IBUS_KP_5) {
    position = key - IBUS_KP_0;
  } else {
    switch (key) {
    case '!': position = 1; break;
    case '@': position = 2; break;
    case '#': position = 3; break;
    case '$': position = 4; break;
    case '%': position = 5; break;
    default: break;
    }
  }
  if (position != 0) {
    action.kind = CandidateShortcutKind::Fix;
    action.position = static_cast<std::uint8_t>(position);
  }
  return action;
}

}  // namespace msime::linux_host
