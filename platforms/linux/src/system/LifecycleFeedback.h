#pragma once

#include "../core/JsonInteger.h"
#include "msime_client.h"

#include <cstdint>
#include <nlohmann/json.hpp>
#include <optional>

namespace msime::linux_host {

// IBus 和 Fcitx5 都让生命周期核心驱动按键音、上屏音、打字特效和背景音乐（msime_client.h 的 MSIME_LIFECYCLE_FEEDBACK）。这里是两个宿主共用、不依赖输入框架的那一半：字段属性怎么换算、什么时候报告，按键怎么编码，回复里的特效怎么读。

// 输入框的属性逐个标志换算成 msime_client_set_field_traits 的位，不复用宿主把"隐私"和"不做拼写检查"并成一个布尔值的那个判断：只带不做拼写检查的普通字段（终端、代码编辑器常这样标）照常出声，隐私字段（IBus 的 IBUS_INPUT_HINT_PRIVATE、Fcitx5 的 Sensitive）按最严处理成 SECURE 加 PRIVATE。核心只在 SECURE 下关掉声音、特效和音乐，PRIVATE 只挡将来的脚本，所以只报 PRIVATE 会让今天静音的隐私字段开始出声。密码、PIN、数字一类字段两个宿主都不建会话，走不到这里。两个宿主都确实知道这些属性，所以总带 KNOWN。
constexpr std::uint32_t lifecycle_field_traits(bool private_field, bool no_spellcheck) {
  return MSIME_TRAIT_KNOWN | (private_field ? MSIME_TRAIT_SECURE | MSIME_TRAIT_PRIVATE : 0u) |
         (no_spellcheck ? MSIME_TRAIT_NO_SPELLCHECK : 0u);
}

// msime_client_key 的参数：按键类别（key_sound_class 的 0-3），自动重复再带 MSIME_KEY_REPEAT。Linux 的自动重复照旧出声，只是画出来不计连击（设计待决问题 Q3：各宿主保留现状）。
constexpr std::uint32_t lifecycle_key(std::uint32_t key_class, bool repeat) {
  return key_class | (repeat ? MSIME_KEY_REPEAT : 0u);
}

// 宿主这一侧记着核心现在知道的字段属性。顺序契约要求宿主在每个键之前比较一次，变了才先报告、再调 msime_client_key；换了会话就从头报告，核心那边的新会话还什么都不知道。
class FieldTraitsReport {
public:
  template <class Call> void sync(std::uint64_t session, std::uint32_t traits, Call &&call) {
    if (session == 0 || (session == session_ && traits == reported_)) return;
    call(session, traits);
    session_ = session;
    reported_ = traits;
  }
  // 每次 msime_client_focus(true) 之前调用：不论属性变没变都报告一次。脚本运行时（#7253）在每次获得焦点之后，要等宿主确认过字段属性才把事件交给脚本，确认是 focus 之前的一次 set_field_traits（值相同也算）或之后的第一个键；sync 的去重会把同一个会话重新获得焦点时那次报告省掉，于是从获得焦点到第一个键之间脚本什么都收不到。
  template <class Call> void confirm(std::uint64_t session, std::uint32_t traits, Call &&call) {
    if (session == 0) return;
    call(session, traits);
    session_ = session;
    reported_ = traits;
  }
  // 会话销毁之前：下一个会话即使拿到同一个句柄，也要重新报告。
  void reset() {
    session_ = 0;
    reported_ = 0;
  }

private:
  std::uint64_t session_ = 0;
  std::uint32_t reported_ = 0;
};

// 派发类调用回复里的 "feedback":{"typing_effect":N}：上屏音和上屏的打字特效已经由核心在这次派发里出过，N 是上屏之后的特效，位布局同 msime_client_typing_effect，没有上屏时为 0。回复里没有这个键（focus、语音等不是派发的调用，或会话没开 FEEDBACK）或者它不是合法整数时返回空，宿主保留原来的连击数。
inline std::optional<std::uint32_t> reply_typing_effect(const nlohmann::json &reply) {
  if (!reply.is_object()) return std::nullopt;
  const auto feedback = reply.find("feedback");
  if (feedback == reply.end() || !feedback->is_object()) return std::nullopt;
  const auto effect = feedback->find("typing_effect");
  if (effect == feedback->end()) return std::nullopt;
  // 先按 64 位读再查范围：strict_json_integer_optional 对无符号目标类型不检查无符号输入是否越界，直接读成 uint32_t 会把 2^32 截成 0。
  const auto wide = strict_json_integer_optional<std::uint64_t>(*effect);
  if (!wide || *wide > UINT32_MAX) return std::nullopt;
  return static_cast<std::uint32_t>(*wide);
}

} // namespace msime::linux_host
