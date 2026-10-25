#pragma once

#include "KeyEvent.h"
#include "SessionPump.h"
#include "ipc_negotiation.h"
#include <functional>
#include <utility>

namespace msime::windows {
// Production TSF clients receive their input policy from the validated
// preference snapshot owned by InputState.  Do not capture launch-time preview
// values here: the preference monitor can publish a newer snapshot while a
// client remains focused, and the next key must use that snapshot atomically.
// `persist_japanese_toggle` 在工作线程上从存储读出当前方案、算出目标并写回（server_main.cpp 的 toggle_japanese_scheme），返回是否接下了这次请求。
struct PreferenceChange {
  bool accepted = false;
  std::shared_future<std::optional<PreferenceSnapshot>> completion;
  PreferenceChange(bool value = false) : accepted(value) {}
  PreferenceChange(std::shared_future<std::optional<PreferenceSnapshot>> value)
      : accepted(true), completion(std::move(value)) {}
};
inline SessionPump::KeyHandler
production_key_handler(
    std::function<bool(bool)> persist_character_set = {},
    std::function<PreferenceChange()> persist_japanese_toggle = {}) {
  return [persist_character_set = std::move(persist_character_set),
          persist_japanese_toggle = std::move(persist_japanese_toggle)](
             InputState &state, const FocusLease &focus,
             const FanyImeNamedpipeData &packet) {
    if (FanyImeProtocol::IsCharacterSetShortcut(packet.keycode,
                                                packet.modifiers_down))
      return state.toggle_character_set(
          focus, packet, state.character_set_shortcut_enabled(),
          persist_character_set);
    if (FanyImeProtocol::IsJapaneseToggleShortcut(packet.keycode,
                                                  packet.modifiers_down)) {
      PreferenceChange change;
      auto reply = state.toggle_japanese(focus, packet,
          state.japanese_toggle_shortcut_enabled(), [&] {
            if (persist_japanese_toggle) change = persist_japanese_toggle();
            return change.accepted;
          });
      if (reply) reply->preference_change = std::move(change.completion);
      return reply;
    }
    return state.configured_key(
        focus, packet, state.tsf_preedit_style(), state.navigation_bindings(),
        local_commit_observation(packet), state.word_character_binding());
  };
}
} // namespace msime::windows
