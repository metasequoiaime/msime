// 候选菜单到扩展的 wire 映射：条目来自 CandidateMenuItem（文案、顺序、可用性照
// CandidateActionPolicy.h），转成可直接回传 property_activate 的 action_name 与 flags。
#include "../../src/candidates/ShellCandidateMenu.h"

#include <cassert>

using namespace msime::linux_host;

int main() {
  // 四项命令映射到可直接回传 property_activate 的完整 action_name。
  const auto items = candidate_menu_items("甲乙");
  const auto wire = shell_menu_entries(items, 42, 7, 3);
  assert(wire.size() == items.size());
  assert(wire[0].action_name == "CandidatePin/42/7/3");
  assert(wire[1].action_name == "CandidateFix1/42/7/3");
  assert(wire[6].action_name.empty());  // 分隔线
  assert((wire[6].flags & kShellMenuFlagSeparator) != 0);
  assert(wire[8].action_name == "CandidateRemove/42/7/3");
  assert(wire[0].label == "置顶 (Ctrl+↑)");
  assert(wire[1].position == 1);

  // 单字候选无删除项。
  const auto single = shell_menu_entries(candidate_menu_items("字"), 42, 7, 0);
  for (const auto &e : single)
    assert(e.action_name.rfind("CandidateRemove", 0) != 0);

  // 整体不可用：所有非分隔线项 disabled。
  const auto off = shell_menu_entries(candidate_menu_items("甲乙", false, 0), 42, 7, 0);
  for (const auto &e : off)
    if (!e.action_name.empty())
      assert((e.flags & kShellMenuFlagDisabled) != 0);

  // 固定第 2 位：该项带 checked。
  const auto fixed = shell_menu_entries(candidate_menu_items("甲乙", true, 2), 42, 7, 0);
  assert((fixed[2].flags & kShellMenuFlagChecked) != 0);
}
