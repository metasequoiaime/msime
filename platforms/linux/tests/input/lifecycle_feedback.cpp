#include "../../src/system/KeySound.h"
#include "../../src/system/LifecycleFeedback.h"

#include <cassert>
#include <cstdint>
#include <initializer_list>
#include <utility>
#include <vector>

int main() {
  using msime::linux_host::FieldTraitsReport;
  using msime::linux_host::key_sound_class;
  using msime::linux_host::lifecycle_field_traits;
  using msime::linux_host::lifecycle_key;
  using msime::linux_host::reply_typing_effect;
  using Json = nlohmann::json;

  // 字段属性逐个标志映射：普通字段只有 KNOWN；隐私字段（IBus 的 HINT_PRIVATE、Fcitx5 的 Sensitive）是 SECURE 加 PRIVATE，核心据此静音；只带不做拼写检查的字段只多一个 NO_SPELLCHECK，不带 SECURE，照常出声。
  assert(lifecycle_field_traits(false, false) == MSIME_TRAIT_KNOWN);
  assert(lifecycle_field_traits(true, false) ==
         (MSIME_TRAIT_KNOWN | MSIME_TRAIT_SECURE | MSIME_TRAIT_PRIVATE));
  assert(lifecycle_field_traits(false, true) == (MSIME_TRAIT_KNOWN | MSIME_TRAIT_NO_SPELLCHECK));
  assert((lifecycle_field_traits(false, true) & MSIME_TRAIT_SECURE) == 0);
  assert(lifecycle_field_traits(true, true) ==
         (MSIME_TRAIT_KNOWN | MSIME_TRAIT_SECURE | MSIME_TRAIT_PRIVATE | MSIME_TRAIT_NO_SPELLCHECK));
  // 这两个宿主没有全屏、提权、宿主英文和语音这几种属性可报。
  for (bool private_field : {false, true})
    for (bool no_spellcheck : {false, true})
      assert((lifecycle_field_traits(private_field, no_spellcheck) &
              (MSIME_TRAIT_FULLSCREEN | MSIME_TRAIT_ELEVATED | MSIME_TRAIT_HOST_ENGLISH |
               MSIME_TRAIT_VOICE_ACTIVE)) == 0);
  // 头文件里的位与核心（crates/host-api/src/lifecycle.rs）一致。
  static_assert(MSIME_TRAIT_SECURE == 0x01u && MSIME_TRAIT_PRIVATE == 0x02u &&
                MSIME_TRAIT_NO_SPELLCHECK == 0x04u && MSIME_TRAIT_KNOWN == 0x80000000u);
  static_assert(MSIME_KEY_POINTER == 0x80u && MSIME_KEY_REPEAT == 0x100u &&
                MSIME_LIFECYCLE_FEEDBACK == 0x1u);

  // msime_client_key 的参数：按键类别同 key_sound，自动重复带 MSIME_KEY_REPEAT，Linux 的自动重复照旧出声。
  assert(lifecycle_key(key_sound_class('a'), false) == 0);
  assert(lifecycle_key(key_sound_class(0x0020), false) == 1);
  assert(lifecycle_key(key_sound_class(0xff0d), false) == 2);
  assert(lifecycle_key(key_sound_class(0xff08), false) == 3);
  assert(lifecycle_key(key_sound_class(0xff08), true) == (3 | MSIME_KEY_REPEAT));
  assert(lifecycle_key(key_sound_class('a'), true) == MSIME_KEY_REPEAT);
  assert((lifecycle_key(3, true) & MSIME_KEY_POINTER) == 0);

  // 字段属性只在变了时报告；新会话从头报告，没有会话时什么都不报。
  std::vector<std::pair<std::uint64_t, std::uint32_t>> reports;
  const auto report = [&](std::uint64_t session, std::uint32_t traits) {
    reports.emplace_back(session, traits);
    return traits;
  };
  FieldTraitsReport traits;
  const auto ordinary = lifecycle_field_traits(false, false);
  const auto secret = lifecycle_field_traits(true, false);
  traits.sync(0, ordinary, report);
  assert(reports.empty());
  traits.sync(7, ordinary, report);
  assert(reports.size() == 1 && reports.back() == std::make_pair(std::uint64_t{7}, ordinary));
  traits.sync(7, ordinary, report);
  assert(reports.size() == 1);
  traits.sync(7, secret, report);
  assert(reports.size() == 2 && reports.back() == std::make_pair(std::uint64_t{7}, secret));
  traits.sync(8, secret, report);
  assert(reports.size() == 3 && reports.back() == std::make_pair(std::uint64_t{8}, secret));
  // 会话销毁后，哪怕下一个会话拿到同一个句柄，也重新报告。
  traits.reset();
  traits.sync(8, secret, report);
  assert(reports.size() == 4 && reports.back() == std::make_pair(std::uint64_t{8}, secret));
  // 获得焦点之前的确认不去重：属性没变也报告，脚本运行时据此在 focus(true) 之后放行事件；没有会话时同样什么都不报。确认过的值照样记下，之后的按键不重复报告。
  traits.confirm(8, secret, report);
  assert(reports.size() == 5 && reports.back() == std::make_pair(std::uint64_t{8}, secret));
  traits.confirm(8, secret, report);
  assert(reports.size() == 6);
  traits.sync(8, secret, report);
  assert(reports.size() == 6);
  traits.confirm(0, secret, report);
  assert(reports.size() == 6);
  traits.reset();
  traits.confirm(9, ordinary, report);
  assert(reports.size() == 7 && reports.back() == std::make_pair(std::uint64_t{9}, ordinary));
  traits.sync(9, ordinary, report);
  assert(reports.size() == 7);

  // 派发回复里的 feedback：读出上屏之后的打字特效；没有这个键、不是对象或不是合法整数时为空，宿主保留原来的连击数。
  assert(reply_typing_effect(Json::parse(R"({"commit":"你好","feedback":{"typing_effect":0}})")) == 0u);
  assert(reply_typing_effect(Json::parse(R"({"feedback":{"typing_effect":786435}})")) == 786435u);
  assert(msime::linux_host::typing_effect_combo(
             *reply_typing_effect(Json::parse(R"({"feedback":{"typing_effect":786435}})"))) == 3);
  assert(!reply_typing_effect(Json::parse(R"({"commit":"你好"})")));
  assert(!reply_typing_effect(Json::parse(R"({"feedback":null})")));
  assert(!reply_typing_effect(Json::parse(R"({"feedback":{}})")));
  assert(!reply_typing_effect(Json::parse(R"({"feedback":{"typing_effect":-1}})")));
  assert(!reply_typing_effect(Json::parse(R"({"feedback":{"typing_effect":1.5}})")));
  assert(!reply_typing_effect(Json::parse(R"({"feedback":{"typing_effect":4294967296}})")));
  assert(!reply_typing_effect(Json::parse("null")));
}
