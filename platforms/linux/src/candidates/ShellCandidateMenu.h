// 候选右键菜单发给 GNOME Shell 扩展的 wire 数据：条目文案、顺序、checked/disabled/
// separator 全部来自 CandidateMenuItem（其文案与可用性由 CandidateActionPolicy.h 决定），
// 扩展只按这些字段画菜单，不内置文案、不解析身份做信任判断。
#pragma once

#include "CandidateActionName.h"
#include "CandidateMenuItems.h"

#include <cstdint>
#include <string>
#include <vector>

namespace msime::linux_host {

// flags 位编码：扩展按位判断是否画勾选、置灰、分隔线。
inline constexpr std::uint32_t kShellMenuFlagChecked = 1u << 0;
inline constexpr std::uint32_t kShellMenuFlagDisabled = 1u << 1;
inline constexpr std::uint32_t kShellMenuFlagSeparator = 1u << 2;

struct ShellMenuEntry {
  // 可直接回传 property_activate 的完整动作名；分隔线为空。
  std::string action_name;
  std::string label;
  std::uint32_t flags = 0;
  // 固定到目标槽位 1..5，其余为 0。
  std::uint8_t position = 0;
};

inline std::vector<ShellMenuEntry> shell_menu_entries(
    const std::vector<CandidateMenuItem> &items, std::uint64_t session,
    std::uint64_t generation, std::size_t index) {
  std::vector<ShellMenuEntry> wire;
  wire.reserve(items.size());
  for (const auto &item : items) {
    ShellMenuEntry entry;
    entry.label = item.label;
    entry.position = item.position;
    if (item.separator) {
      entry.flags |= kShellMenuFlagSeparator;
    } else {
      const char *action = nullptr;
      switch (item.command) {
        case CandidateMenuCommand::PinToTop:
          action = "CandidatePin";
          break;
        case CandidateMenuCommand::FixAtPosition:
          entry.action_name = candidate_action_name(
              ("CandidateFix" + std::to_string(item.position)).c_str(), session,
              generation, index);
          break;
        case CandidateMenuCommand::ClearFixedPosition:
          action = "CandidateClear";
          break;
        case CandidateMenuCommand::Remove:
          action = "CandidateRemove";
          break;
      }
      if (action)
        entry.action_name = candidate_action_name(action, session, generation, index);
      if (item.checked) entry.flags |= kShellMenuFlagChecked;
      if (item.disabled) entry.flags |= kShellMenuFlagDisabled;
    }
    wire.push_back(std::move(entry));
  }
  return wire;
}

}  // namespace msime::linux_host
