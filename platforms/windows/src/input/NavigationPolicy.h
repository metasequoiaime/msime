#pragma once
#include "ReplyCodec.h"
#include "PipeMetadata.h"
#include "msime_client.h"
#include <nlohmann/json.hpp>
#include <optional>

namespace msime::windows {
// Explicit snapshot supplied by the native dispatch owner. No implicit product
// defaults or preference persistence here. Only call after TSF context policy
// has excluded punctuation/word-to-character shortcuts.
struct NavigationBindings {
  bool minus_equal = false;
  bool comma_period = false;
  bool brackets = false;
  bool tab = false;
  bool page_up_down = false;
  bool arrows = false;
  bool mouse_wheel = false;
  // 组字时 ';' 和 '\'' 选第二、第三个候选（偏好 `second_third_candidate`）。和翻页键一样是候选列表上的键，所以随这份快照一起交给按键路由；规则见 SecondThirdCandidatePolicy.h。
  bool second_third_candidate = false;
};
struct NavigationAction {
  std::optional<uint32_t> command;
  NavigationReply reply;
};
// 共享偏好的 `second_third_candidate`：整个对象缺省（关闭时不写进文档）为关闭，缺字段时取 client-core 的默认值。键位名不认识时拒绝整份偏好，和以词定字一样，不会悄悄换成别的键。
inline bool preference_second_third_candidate(const nlohmann::json &preferences) {
  if (!preferences.contains("second_third_candidate"))
    return false;
  const auto &value = preferences.at("second_third_candidate");
  if (value.value("keys", std::string("semicolon_quote")) != "semicolon_quote")
    throw std::invalid_argument("Invalid shared second and third candidate keys");
  return value.value("enabled", false);
}
// Decode once per publication; no file reads or JSON parsing on the key path.
inline NavigationBindings
preference_navigation(const nlohmann::json &preferences) {
  NavigationBindings bindings{true, true, false, true, true, true, false};
  if (preferences.contains("navigation")) {
    const auto &value = preferences.at("navigation");
    bindings = {value.at("minus_equal").get<bool>(),
                value.at("comma_period").get<bool>(),
                value.at("brackets").get<bool>(),
                value.at("tab").get<bool>(),
                value.at("page_up_down").get<bool>(),
                value.at("arrows").get<bool>(),
                value.value("mouse_wheel", false)};
  }
  bindings.second_third_candidate = preference_second_third_candidate(preferences);
  return bindings;
}
inline std::optional<NavigationAction>
navigation_action(const FanyImeNamedpipeData &packet,
                  const NavigationBindings &bindings, bool unicode,
                  bool japanese = false) {
  const auto modifiers = PipeMetadata::key_modifiers(packet.modifiers_down);
  if (packet.event_type != FanyImePipeEventType::KeyEvent || (modifiers & ~1u))
    return std::nullopt;
  const auto key = packet.keycode;
  if (japanese && (key == 0xBD || key == 0xBB))
    return std::nullopt;
  // Unicode '+' extends the code sequence, even with equal-key paging enabled.
  if (unicode && packet.wch == '+')
    return std::nullopt;
  bool previous;
  if ((bindings.minus_equal && (key == 0xBD || key == 0xBB)) ||
      (bindings.comma_period && (key == 0xBC || key == 0xBE)) ||
      (bindings.brackets && (key == 0xDB || key == 0xDD)) ||
      (bindings.page_up_down && (key == 0x21 || key == 0x22)))
    previous = key == 0xBD || key == 0xBC || key == 0xDB || key == 0x21;
  else if (bindings.tab && key == 0x09)
    previous = (modifiers & 1u) != 0;
  else if (bindings.arrows && (key == 0x26 || key == 0x28))
    return NavigationAction{key == 0x26 ? MSIME_PREVIOUS_CANDIDATE
                                        : MSIME_NEXT_CANDIDATE,
                            key == 0x26 ? NavigationReply::PreviousCandidate
                                        : NavigationReply::NextCandidate};
  else {
    switch (key) {
    case 0xBD:
    case 0xBB:
    case 0xBC:
    case 0xBE:
    case 0xDB:
    case 0xDD:
    case 0x09:
    case 0x21:
    case 0x22:
    case 0x26:
    case 0x28:
      // TSF already consumed this navigation request. Disabled is a reply,
      // not permission to reinterpret the key as text or another command.
      return NavigationAction{std::nullopt, NavigationReply::Ignored};
    default:
      return std::nullopt;
    }
  }
  return NavigationAction{previous ? MSIME_PREVIOUS_PAGE : MSIME_NEXT_PAGE,
                          previous ? NavigationReply::PreviousPage
                                   : NavigationReply::NextPage};
}
} // namespace msime::windows
