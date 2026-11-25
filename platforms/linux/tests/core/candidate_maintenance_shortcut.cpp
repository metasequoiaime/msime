// 候选维护快捷键的解析：Ctrl+Delete 删除候选、Ctrl+↑ 置顶、Ctrl+0 取消固定，Ctrl+Shift+1..5 固定到第 1..5 位（Ctrl+1..5 让给别的快捷键）。修饰键必须恰当；其余键一律不动作。以及按键落到哪一行（CandidateShortcutAction.h）：本页高亮行的动作名，围栏不通过时为空、按键照常往下走。

#include "../../src/candidates/CandidateShortcutAction.h"
#include "../../src/core/CandidateMaintenanceShortcut.h"

#include <cassert>

namespace {

using Json = nlohmann::json;
using msime::linux_host::candidate_shortcut_action;
using msime::linux_host::CandidateShortcut;
using msime::linux_host::CandidateShortcutKind;

Json row(const char *text, bool highlighted, int source = 0, std::uint64_t session = 42) {
  return {{"text", text}, {"highlighted", highlighted}, {"source", source},
          {"id", {{"session", session}, {"generation", 7}, {"index", highlighted ? 3 : 2}}}};
}

void shortcut_targets() {
  const Json view{{"generation", 7}};
  const Json page = Json::array({row("甲乙", false), row("丙丁", true)});
  const auto act = [&](CandidateShortcut shortcut, const Json &rows = Json(),
                       std::uint64_t scheme = 0, const Json &rendered = Json(),
                       std::uint64_t rendered_session = 42) {
    return candidate_shortcut_action(rendered.is_null() ? view : rendered, view,
                                     rows.is_null() ? page : rows, 42, rendered_session, scheme,
                                     shortcut);
  };
  // 动作落在高亮的那一行，编码成 property_activate 认得的完整动作名。
  assert(act({CandidateShortcutKind::Pin, 0}) == "CandidatePin/42/7/3");
  assert(act({CandidateShortcutKind::Remove, 0}) == "CandidateRemove/42/7/3");
  assert(act({CandidateShortcutKind::Clear, 0}) == "CandidateClear/42/7/3");
  assert(act({CandidateShortcutKind::Fix, 2}) == "CandidateFix2/42/7/3");
  // 不是快捷键、没有高亮行、代次过期、渲染页不归本会话、行 id 不归本会话：都为空，按键照常往下走。
  assert(!act({}));
  assert(!act({CandidateShortcutKind::Pin, 0}, Json::array({row("甲乙", false)})));
  assert(!act({CandidateShortcutKind::Pin, 0}, Json(), 0, Json{{"generation", 6}}));
  assert(!act({CandidateShortcutKind::Pin, 0}, Json(), 0, Json(), 41));
  assert(!act({CandidateShortcutKind::Pin, 0}, Json::array({row("丙丁", true, 0, 41)})));
  // 不学习的方案（日语等）和动态候选（云、AI、表情）不可维护。
  assert(!act({CandidateShortcutKind::Pin, 0}, Json(), 3));
  assert(!act({CandidateShortcutKind::Pin, 0}, Json::array({row("丙丁", true, 2)})));
  // 删除另有可删除判定：中文单字不删，英文词库的单字母行可删，其余动作不受它影响。
  assert(!act({CandidateShortcutKind::Remove, 0}, Json::array({row("丙", true)})));
  assert(act({CandidateShortcutKind::Pin, 0}, Json::array({row("丙", true)})) == "CandidatePin/42/7/3");
  assert(act({CandidateShortcutKind::Remove, 0}, Json::array({row("a", true, 4)})) ==
         "CandidateRemove/42/7/3");
}

}  // namespace

int main() {
  using msime::linux_host::candidate_maintenance_shortcut;
  using Kind = msime::linux_host::CandidateShortcutKind;
  constexpr auto control = IBUS_CONTROL_MASK;
  constexpr auto shift = IBUS_SHIFT_MASK;
  constexpr auto alt = IBUS_MOD1_MASK;
  constexpr auto fix_modifiers = control | shift;

  // Ctrl+Delete 与小键盘 Ctrl+KP_Delete → 删除。
  assert(candidate_maintenance_shortcut(IBUS_Delete, control).kind == Kind::Remove);
  assert(candidate_maintenance_shortcut(IBUS_KP_Delete, control).kind == Kind::Remove);

  // Ctrl+↑ 与小键盘 Ctrl+KP_Up → 置顶。
  assert(candidate_maintenance_shortcut(IBUS_Up, control).kind == Kind::Pin);
  assert(candidate_maintenance_shortcut(IBUS_KP_Up, control).kind == Kind::Pin);

  // Ctrl+0 与 Ctrl+KP_0 → 取消固定。
  assert(candidate_maintenance_shortcut(IBUS_0, control).kind == Kind::Clear);
  assert(candidate_maintenance_shortcut(IBUS_KP_0, control).kind == Kind::Clear);

  // Ctrl+1..5 与 Ctrl+KP_1..KP_5 已让给别的快捷键，不再固定排位。
  for (std::uint32_t digit = 1; digit <= 5; ++digit) {
    assert(candidate_maintenance_shortcut(IBUS_0 + digit, control).kind == Kind::None);
    assert(candidate_maintenance_shortcut(IBUS_KP_0 + digit, control).kind == Kind::None);
  }

  // Ctrl+Shift+1..5 与 Ctrl+Shift+KP_1..KP_5 → 固定到对应槽位。
  for (std::uint32_t digit = 1; digit <= 5; ++digit) {
    const auto main_row = candidate_maintenance_shortcut(IBUS_0 + digit, fix_modifiers);
    assert(main_row.kind == Kind::Fix);
    assert(main_row.position == digit);
    const auto keypad = candidate_maintenance_shortcut(IBUS_KP_0 + digit, fix_modifiers);
    assert(keypad.kind == Kind::Fix);
    assert(keypad.position == digit);
  }

  // 美式布局上 Shift+1..5 打出 ! @ # $ %；没有物理键码时也要认出对应槽位。
  const guint shifted_symbols[] = {'!', '@', '#', '$', '%'};
  for (std::uint32_t slot = 1; slot <= 5; ++slot) {
    const auto action = candidate_maintenance_shortcut(shifted_symbols[slot - 1], fix_modifiers);
    assert(action.kind == Kind::Fix);
    assert(action.position == slot);
  }

  // 非美式布局的 Shift+数字行打出的符号可能不在上表里；物理键码 2..6 仍按槽位认。
  for (guint keycode = 2; keycode <= 6; ++keycode) {
    const auto action = candidate_maintenance_shortcut('&', fix_modifiers, keycode);
    assert(action.kind == Kind::Fix);
    assert(action.position == keycode - 1);
  }

  // Ctrl+Shift+6..9、Ctrl+Shift+0 不是固定槽位，不动作。
  for (std::uint32_t digit = 6; digit <= 9; ++digit) {
    assert(candidate_maintenance_shortcut(IBUS_0 + digit, fix_modifiers).kind == Kind::None);
    assert(candidate_maintenance_shortcut(IBUS_KP_0 + digit, fix_modifiers).kind == Kind::None);
  }
  assert(candidate_maintenance_shortcut(IBUS_0, fix_modifiers).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_KP_0, fix_modifiers).kind == Kind::None);

  // 其余修饰组合不算；置顶、删除、取消固定仍只有 Control，Shift 版本不动作。
  assert(candidate_maintenance_shortcut(IBUS_Delete, shift).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Delete, alt).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Delete, control | alt).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Delete, fix_modifiers).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Up, control | alt).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Up, fix_modifiers).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_0, control | alt).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_0, fix_modifiers).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_1, control | shift | alt).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_1, shift).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_1, alt).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_1, 0).kind == Kind::None);

  // 不抢既有的组合键：无修饰的 Delete/Up 是光标与选词，Ctrl+BackSpace 和 Ctrl+左右是分段编辑。
  assert(candidate_maintenance_shortcut(IBUS_Delete, 0).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Up, 0).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_BackSpace, control).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Left, control).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Right, control).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_Down, control).kind == Kind::None);
  assert(candidate_maintenance_shortcut(IBUS_a, control).kind == Kind::None);

  // Ctrl+0 按物理键码认：AZERTY 上 0 键的按键符号是 agrave。
  assert(candidate_maintenance_shortcut(IBUS_agrave, control, 11).kind == Kind::Clear);
  assert(candidate_maintenance_shortcut(IBUS_agrave, control, 0).kind == Kind::None);

  shortcut_targets();
}
