// 候选维护快捷键的解析（iBus）：Ctrl+Delete 删除候选、Ctrl+↑ 置顶、Ctrl+1..5 固定到
// 第 1..5 位、Ctrl+0 取消固定。动作落到高亮渲染行（ClientEngine 遍历 rendered_candidates 里
// highlighted 的行，围栏与面板属性菜单同一组），本头只负责按键 → 动作的纯解析。
// 修饰键必须恰为 Control（调用方已用 canonical_modifiers 规范化并排除 release 事件）；
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

inline CandidateShortcut candidate_maintenance_shortcut(guint key, guint modifiers) {
  CandidateShortcut action;
  if (modifiers != IBUS_CONTROL_MASK)
    return action;
  if (key == IBUS_Delete || key == IBUS_KP_Delete)
    action.kind = CandidateShortcutKind::Remove;
  else if (key == IBUS_Up || key == IBUS_KP_Up)
    action.kind = CandidateShortcutKind::Pin;
  else if (key == IBUS_0 || key == IBUS_KP_0)
    action.kind = CandidateShortcutKind::Clear;
  else if (key >= IBUS_1 && key <= IBUS_5) {
    action.kind = CandidateShortcutKind::Fix;
    action.position = static_cast<std::uint8_t>(key - IBUS_0);
  } else if (key >= IBUS_KP_1 && key <= IBUS_KP_5) {
    action.kind = CandidateShortcutKind::Fix;
    action.position = static_cast<std::uint8_t>(key - IBUS_KP_0);
  }
  return action;
}

}  // namespace msime::linux_host
