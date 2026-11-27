// 候选维护快捷键落到哪一行（iBus）：在本页渲染里找 highlighted 的那一行，围栏与「候选操作」属性菜单同一组（渲染页归属本会话且仍是当前代次、行 id 归本会话、candidate_dictionary_actions_available，删除另有可删除判定），通过时给出这一行的完整动作名，交给 property_activate 按身份核对后执行。高亮行不在、不可维护或围栏不通过时为空，调用方让按键照常往下走。
#pragma once

#include "../core/CandidateMaintenanceShortcut.h"
#include "CandidateActionName.h"
#include "CandidateActionPolicy.h"

#include <nlohmann/json.hpp>

#include <cstdint>
#include <optional>
#include <string>

namespace msime::linux_host {

inline std::optional<std::string> candidate_shortcut_action(
    const nlohmann::json &rendered_view, const nlohmann::json &view,
    const nlohmann::json &rendered_candidates, std::uint64_t session,
    std::uint64_t rendered_session, std::uint64_t scheme, CandidateShortcut shortcut) {
  if (shortcut.kind == CandidateShortcutKind::None || !session || rendered_session != session ||
      !rendered_view.is_object() || !view.is_object() || !rendered_candidates.is_array() ||
      rendered_view.value("generation", std::uint64_t{0}) !=
          view.value("generation", std::uint64_t{0}))
    return std::nullopt;
  for (const auto &candidate : rendered_candidates) {
    if (!candidate.is_object() || !candidate.value("highlighted", false))
      continue;
    const auto id = candidate.value("id", nlohmann::json::object());
    if (!id.is_object() || id.value("session", std::uint64_t{0}) != session)
      return std::nullopt;
    const auto source = candidate.value("source", 0);
    if (!candidate_dictionary_actions_available(scheme, source))
      return std::nullopt;
    if (shortcut.kind == CandidateShortcutKind::Remove &&
        !candidate_dictionary_removal_available(scheme, source,
                                                candidate.value("text", std::string{})))
      return std::nullopt;
    std::string action;
    switch (shortcut.kind) {
    case CandidateShortcutKind::Remove:
      action = "CandidateRemove";
      break;
    case CandidateShortcutKind::Pin:
      action = "CandidatePin";
      break;
    case CandidateShortcutKind::Clear:
      action = "CandidateClear";
      break;
    case CandidateShortcutKind::Fix:
      if (shortcut.position < 1 || shortcut.position > 5)
        return std::nullopt;
      action = "CandidateFix" + std::to_string(shortcut.position);
      break;
    case CandidateShortcutKind::None:
      return std::nullopt;
    }
    return candidate_action_name(action.c_str(), session,
                                 id.value("generation", std::uint64_t{0}),
                                 id.value("index", std::size_t{0}));
  }
  return std::nullopt;
}

}  // namespace msime::linux_host
