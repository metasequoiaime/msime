// 候选右键自绘菜单的条目模型：条目文案与顺序照 Windows 的 CandidateMenuLayout.h
// （置顶 / 固定到第 1..5 位 / 分隔线 / 取消固定 / 删除），可用性与固定状态在这里算好，
// 浮层绘制与动作接线只消费条目表。

#include "../../src/candidates/CandidateMenuItems.h"

#include <cassert>

int main() {
  using msime::linux_host::candidate_menu_items;
  using Command = msime::linux_host::CandidateMenuCommand;

  // 多字词、可用、未固定：置顶、固定到第 1..5 位、分隔线、取消固定（禁用）、删除。
  {
    const auto items = candidate_menu_items("甲乙甲乙");
    assert(items.size() == 9);
    assert(items[0].command == Command::PinToTop);
    assert(items[0].label == "置顶");
    for (auto position = 1; position <= 5; ++position) {
      assert(items[position].command == Command::FixAtPosition);
      assert(items[position].position == position);
      assert(items[position].label == "固定到第 " + std::to_string(position) + " 位");
      assert(!items[position].checked);
    }
    assert(items[6].separator);
    assert(items[7].command == Command::ClearFixedPosition);
    assert(items[7].label == "取消固定");
    assert(items[7].disabled);
    assert(items[8].command == Command::Remove);
    assert(items[8].label == "删除");
  }
  // 单字行没有「删除」（CandidateActionPolicy.h 的可删除判定：单字不进）。
  {
    const auto items = candidate_menu_items("字");
    assert(items.size() == 8);
    assert(items[0].command == Command::PinToTop);
    assert(items[7].command == Command::ClearFixedPosition);
  }
  // 已固定第 2 位：第 2 位打勾，取消固定可用。
  {
    const auto items = candidate_menu_items("甲乙甲乙", true, 2);
    assert(items[2].checked);
    assert(!items[1].checked);
    assert(!items[7].disabled);
  }
  // 动作整体不可用时全部条目禁用，分隔线不变。
  {
    const auto items = candidate_menu_items("甲乙甲乙", false, 0);
    for (std::size_t index = 0; index < items.size(); ++index) {
      if (items[index].separator)
        continue;
      assert(items[index].disabled);
    }
  }
  // 已固定的行同样可置顶、固定到别的槽位（覆盖旧固定）。
  {
    const auto items = candidate_menu_items("甲乙甲乙", true, 4);
    assert(!items[0].disabled);
    assert(items[1].command == Command::FixAtPosition);
    assert(!items[1].disabled);
  }
}
