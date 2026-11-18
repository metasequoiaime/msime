// 候选右键自绘菜单的条目模型（iBus）。条目文案与顺序照 Windows 的
// CandidateMenuLayout.h：置顶；固定到第 1..5 位；分隔线；取消固定；删除。
// 单字行没有「删除」（CandidateActionPolicy.h 的可删除判定）；动作整体不可用时
// （视图代次过期、会话已换等）全部条目画成禁用、点了不做事。浮层绘制与动作接线
// 只消费这里的条目表，布局与命中在浮层 surface 里做。

#pragma once

#include "CandidateActionPolicy.h"

#include <cstdint>
#include <string>
#include <vector>

namespace msime::linux_host {

enum class CandidateMenuCommand {
  PinToTop,
  FixAtPosition,
  ClearFixedPosition,
  Remove,
};

struct CandidateMenuItem {
  CandidateMenuCommand command = CandidateMenuCommand::PinToTop;
  std::string label;
  // FixAtPosition 时是目标槽位 1..5，其余为 0。
  std::uint8_t position = 0;
  bool checked = false;
  bool separator = false;
  bool disabled = false;
};

inline std::vector<CandidateMenuItem> candidate_menu_items(
    const std::string &text, bool actionable = true,
    std::uint8_t fixed_position = 0) {
  std::vector<CandidateMenuItem> items;
  const bool removal = candidate_removal_available(text);
  items.reserve(removal ? 9 : 8);
  items.push_back(
      {CandidateMenuCommand::PinToTop, candidate_pin_label, 0, false, false, !actionable});
  for (std::uint8_t position = 1; position <= 5; ++position) {
    items.push_back(
        {CandidateMenuCommand::FixAtPosition, candidate_fix_label(position), position,
         fixed_position == position, false, !actionable});
  }
  items.push_back({CandidateMenuCommand::ClearFixedPosition, "", 0, false, true, false});
  items.push_back({CandidateMenuCommand::ClearFixedPosition, candidate_clear_fix_label, 0,
                   false, false, !actionable || fixed_position == 0});
  if (removal)
    items.push_back({CandidateMenuCommand::Remove, candidate_remove_label, 0, false, false,
                     !actionable});
  return items;
}

}  // namespace msime::linux_host
