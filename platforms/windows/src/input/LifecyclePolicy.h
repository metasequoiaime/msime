#pragma once
#include "InputSchemeTraits.h"
#include "msime_client.h"
#include "windows_ipc.h"
#include <cstdint>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>

namespace msime::windows {
// Server 的会话怎样接入生命周期核心（msime_client.h 的 msime_client_set_lifecycle 一段）。只有 Server 调这些 FFI：TIP 在应用进程里的宿主会话只给 Server 转述，从不选择加入（tsf/tests/input/lifecycle_boundary.cpp 守着这一条）。

// TIP 报的字段属性位和核心的同名位必须是同一个数，Server 才能原样转交。
static_assert(FanyImeFieldTraits::Secure == MSIME_TRAIT_SECURE);
static_assert(FanyImeFieldTraits::Elevated == MSIME_TRAIT_ELEVATED);

// 会话选择加入的位。按键音、上屏音、打字特效和背景音乐都由核心驱动（FEEDBACK）；部分选词由 Server 自己暂存在前缀里（HOST_HOLDS_PIECES）。`commit_observe_only` 只在 TIP 已经从自己的宿主会话把文字写进文档的那一次派发前后打开。`host_holds_pieces` 只在 Server 不暂存这次选中的一段、而是整段送进文档的那一次选词里关掉（ServerSession::select 里整段上屏的候选来源）。
constexpr uint32_t lifecycle_flags(bool commit_observe_only = false, bool host_holds_pieces = true) {
  return MSIME_LIFECYCLE_FEEDBACK | (host_holds_pieces ? MSIME_LIFECYCLE_HOST_HOLDS_PIECES : 0u) |
         (commit_observe_only ? MSIME_LIFECYCLE_COMMIT_OBSERVE_ONLY : 0u);
}

// Server 处理的一个键交给 msime_client_key 的值：按键类别 0-3（key_sound_class），按住不放的自动重复带 MSIME_KEY_REPEAT（画但不计连击）。
constexpr uint32_t lifecycle_key(uint32_t key_class, bool auto_repeat) {
  return key_class | (auto_repeat ? MSIME_KEY_REPEAT : 0u);
}

// TIP 交给应用、经 Aux 管道异步报来的键：带 MSIME_KEY_PASSTHROUGH，不开新的一轮，也不承诺先于应用处理。TIP 已经滤掉了自动重复。
constexpr uint32_t lifecycle_passthrough_key(uint32_t key_class) {
  return key_class | MSIME_KEY_PASSTHROUGH;
}

// 交给 msime_client_set_field_traits 的字段属性：TIP 报过的位（SECURE、ELEVATED）加 KNOWN，再并上 Server 自己看到的全屏前台。没协商到 FieldTraits 的旧 TIP 从不报，`tip` 为空，属性就保持未知、不带 KNOWN；FULLSCREEN 位本身仍让声音静音。
constexpr uint32_t server_field_traits(std::optional<uint32_t> tip, bool fullscreen) {
  return (fullscreen ? MSIME_TRAIT_FULLSCREEN : 0u) |
         (tip ? (*tip & FanyImeFieldTraits::Reported) | MSIME_TRAIT_KNOWN : 0u);
}

// 这次派发的上屏是不是 TIP 已经从自己的宿主会话写进文档的：韩文、注音、越南文和藏文（scheme::AlwaysInlinePreedit）都在 TIP 自己的会话里组字，全拼、双拼整句改字时（派发前 view 的 conversion 非空）空格和回车由 TIP 的会话钉住一段或上屏整句。Server 只在这种派发前后打开 COMMIT_OBSERVE_ONLY：这些上屏只能观察、不能改写。
inline bool tip_writes_commits(const nlohmann::json &view) {
  if (!view.is_object())
    return false;
  const auto scheme = view.find("scheme");
  if (scheme != view.end() && scheme->is_number_unsigned() &&
      scheme::AlwaysInlinePreedit(static_cast<int>(scheme->get<uint32_t>())))
    return true;
  const auto conversion = view.find("conversion");
  return conversion != view.end() && conversion->is_string() &&
         !conversion->get_ref<const std::string &>().empty();
}

// 一次派发回复里上屏的打字特效（位布局同 msime_client_typing_effect）：只有带上屏、而且核心在 FEEDBACK 下写了 "feedback":{"typing_effect":N} 的回复才有，N 可以是 0（特效和连击都关着）。没有上屏、或者是 Server 自己合成的上屏（译文、释义列、以词定字的回退）时为空：它们不经过核心的上屏事件，不出声也不画。
inline std::optional<uint32_t> commit_feedback(const nlohmann::json &transition) {
  if (!transition.is_object())
    return std::nullopt;
  const auto commit = transition.find("commit");
  const auto feedback = transition.find("feedback");
  if (commit == transition.end() || commit->is_null() || feedback == transition.end() ||
      !feedback->is_object())
    return std::nullopt;
  const auto effect = feedback->find("typing_effect");
  if (effect == feedback->end() || !effect->is_number_unsigned())
    return std::nullopt;
  return effect->get<uint32_t>();
}
} // namespace msime::windows
