#include "KeySoundPolicy.h"
#include "LifecyclePolicy.h"
#include "TypingEffectPolicy.h"

#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
void require(bool value, int line) {
  if (!value)
    throw std::runtime_error("key sound policy failed at line " + std::to_string(line));
}
#define REQUIRE(value) require((value), __LINE__)

FanyImeNamedpipeData key(unsigned code, unsigned modifiers = 0) {
  FanyImeNamedpipeData packet{};
  packet.event_type = FanyImePipeEventType::KeyEvent;
  packet.keycode = code;
  packet.modifiers_down = modifiers;
  return packet;
}
} // namespace

int main() {
  try {
    // The classes msime_client_key_sound numbers: 1 space, 2 enter, 3 backspace, 0 any other key.
    REQUIRE(key_sound_class(key(0x20)) == 1u);
    REQUIRE(key_sound_class(key(0x0D)) == 2u);
    REQUIRE(key_sound_class(key(0x08)) == 3u);
    REQUIRE(key_sound_class(key('A')) == 0u);
    REQUIRE(key_sound_class(key('1')) == 0u);
    REQUIRE(key_sound_class(key(0xBC)) == 0u);
    REQUIRE(key_sound_class(key(0x1B)) == 0u);
    // Shift is typing: a capital, an operator, Shift+digit.
    REQUIRE(key_sound_class(key('V', 1)) == 0u);
    // The pipe's own flags are not modifiers.
    REQUIRE(key_sound_class(key(0x20, FanyImePipeFlags::UiLess | PipeMetadata::CandidateActive)) == 1u);
    REQUIRE(key_sound_class(key('A', PipeMetadata::AutoRepeat)) == 0u);
    REQUIRE(PipeMetadata::key_modifiers(1u | PipeMetadata::AutoRepeat) == 1u);
    // Chords with Ctrl or Alt are shortcuts, and a bare modifier the TIP forwards to cancel a composition is no key at all.
    REQUIRE(!key_sound_class(key('F', 3)));
    REQUIRE(!key_sound_class(key(0x08, 2)));
    REQUIRE(!key_sound_class(key('A', 4)));
    for (unsigned modifier : {0x10u, 0x11u, 0x12u, 0x14u, 0x5Bu, 0x5Cu, 0xA0u, 0xA1u, 0xA2u, 0xA3u, 0xA4u, 0xA5u, 0u})
      REQUIRE(!key_sound_class(key(modifier)));
    // Only key events sound.
    auto activated = key('A');
    activated.event_type = FanyImePipeEventType::ClientActivated;
    REQUIRE(!key_sound_class(activated));

    // The typing effect takes the same classes, flagged 0x100 for an auto-repeat, muted (0x200) in a full-screen application, and 4 for a commit.
    REQUIRE(typing_effect_key_event(2u, true, false) == 2u);
    REQUIRE(typing_effect_key_event(0u, false, false) == 0x200u);
    REQUIRE(typing_effect_key_event(0u, true, true) == 0x100u);
    REQUIRE(typing_effect_key_event(3u, false, true) == 0x303u);
    REQUIRE(typing_effect_commit(true) == 4u);
    REQUIRE(typing_effect_commit(false) == 0x204u);
    // Its packed answer: bits 0-15 the combo, bit 16 a new tier, bits 17-19 the style.
    const auto power = decode_typing_effect(25u | 0x10000u | (3u << 17) | 0x100000u);
    REQUIRE(power.combo == 25u && power.tier_up && power.style == TypingEffectStyle::power_mode);
    const auto flash = decode_typing_effect(7u | (1u << 17));
    REQUIRE(flash.combo == 7u && !flash.tier_up && flash.style == TypingEffectStyle::flash);
    REQUIRE(decode_typing_effect(0u).style == TypingEffectStyle::off);
    REQUIRE(decode_typing_effect(7u << 17).style == TypingEffectStyle::power_mode);
    // The flash fades out over 150 ms, scales with effect_intensity, and is nothing with the style off or the intensity at 0.
    REQUIRE(typing_effect_flash_alpha(flash, 50u, 0u) > typing_effect_flash_alpha(flash, 50u, 100u));
    REQUIRE(typing_effect_flash_alpha(flash, 50u, 150u) == 0.0f);
    REQUIRE(typing_effect_flash_alpha(flash, 0u, 0u) == 0.0f);
    REQUIRE(typing_effect_flash_alpha(flash, 100u, 0u) > typing_effect_flash_alpha(flash, 50u, 0u));
    REQUIRE(typing_effect_flash_alpha(power, 50u, 0u) > typing_effect_flash_alpha(flash, 50u, 0u));
    REQUIRE(typing_effect_flash_alpha(power, 100u, 0u) <= 1.0f);
    REQUIRE(typing_effect_flash_alpha(decode_typing_effect(9u), 100u, 0u) == 0.0f);
    // An effect pack's duration_ms is the flash length, clamped to 60-1500 ms.
    REQUIRE(typing_effect_flash_alpha(flash, 50u, 300u, 600u) > 0.0f);
    REQUIRE(typing_effect_flash_alpha(flash, 50u, 600u, 600u) == 0.0f);
    REQUIRE(resolve_typing_effect_settings(50u, 10u, std::nullopt).flash_millis == 60u);
    REQUIRE(resolve_typing_effect_settings(50u, 9000u, std::nullopt).flash_millis == 1500u);
    REQUIRE(resolve_typing_effect_settings(500u, std::nullopt, std::nullopt).intensity == 100u);
    REQUIRE(resolve_typing_effect_settings(50u, std::nullopt, std::nullopt).flash_millis == typing_effect_flash_millis);
    // Only "#RRGGBB" is a colour.
    REQUIRE(typing_effect_rgb("#FF8800") == 0xFF8800u);
    REQUIRE(typing_effect_rgb("#00aaff") == 0x00AAFFu);
    REQUIRE(!typing_effect_rgb("FF8800"));
    REQUIRE(!typing_effect_rgb("#FF880"));
    REQUIRE(!typing_effect_rgb("#GG8800"));
    // The word handed to the UI thread round-trips, and an unpublished word is nothing so the host's own intensity stands.
    const auto pack = resolve_typing_effect_settings(80u, 900u, 0x123456u);
    REQUIRE(unpack_typing_effect_settings(pack_typing_effect_settings(pack)) == pack);
    const TypingEffectSettings plain{};
    REQUIRE(unpack_typing_effect_settings(pack_typing_effect_settings(plain)) == plain);
    REQUIRE(!unpack_typing_effect_settings(0u));
    // The count shows from two keys on, until the library's 3 s idle window would have ended the combo.
    REQUIRE(!typing_effect_shows_combo(1u, 0u));
    REQUIRE(typing_effect_shows_combo(2u, 2999u));
    REQUIRE(!typing_effect_shows_combo(2u, 3000u));

    // 生命周期核心（FEEDBACK）接手之后，Server 不再自己调 key_sound 和 commit_sound：按键交给 msime_client_key，类别同上，自动重复带 MSIME_KEY_REPEAT；TIP 交给应用的键带 MSIME_KEY_PASSTHROUGH。类别占低 7 位，标志位不和它重叠。
    REQUIRE(lifecycle_key(1u, false) == 1u);
    REQUIRE(lifecycle_key(3u, true) == (3u | MSIME_KEY_REPEAT));
    REQUIRE(lifecycle_passthrough_key(2u) == (2u | MSIME_KEY_PASSTHROUGH));
    for (uint32_t key_class = 0; key_class <= 3u; ++key_class) {
      REQUIRE((lifecycle_key(key_class, true) & 0x7Fu) == key_class);
      REQUIRE((lifecycle_passthrough_key(key_class) & 0x7Fu) == key_class);
      REQUIRE((lifecycle_key(key_class, false) & (MSIME_KEY_POINTER | MSIME_KEY_PASSTHROUGH)) == 0u);
    }
    REQUIRE(lifecycle_flags() == (MSIME_LIFECYCLE_FEEDBACK | MSIME_LIFECYCLE_HOST_HOLDS_PIECES));
    REQUIRE(lifecycle_flags(true) == (lifecycle_flags() | MSIME_LIFECYCLE_COMMIT_OBSERVE_ONLY));
    // 整段上屏的那次选词只去掉 HOST_HOLDS_PIECES，FEEDBACK 和 COMMIT_OBSERVE_ONLY 照旧。
    REQUIRE(lifecycle_flags(false, false) == MSIME_LIFECYCLE_FEEDBACK);
    REQUIRE(lifecycle_flags(true, false) == (MSIME_LIFECYCLE_FEEDBACK | MSIME_LIFECYCLE_COMMIT_OBSERVE_ONLY));
    // 全屏前台不再由 Server 自己判断出不出声，而是作为 FULLSCREEN 属性交给核心：声音静音、特效照画。旧 TIP 不报字段属性，属性保持未知，不带 KNOWN。
    REQUIRE(server_field_traits(std::nullopt, false) == 0u);
    REQUIRE(server_field_traits(std::nullopt, true) == MSIME_TRAIT_FULLSCREEN);
    REQUIRE(server_field_traits(0u, false) == MSIME_TRAIT_KNOWN);
    REQUIRE(server_field_traits(FanyImeFieldTraits::Secure, true) ==
            (MSIME_TRAIT_SECURE | MSIME_TRAIT_FULLSCREEN | MSIME_TRAIT_KNOWN));
    REQUIRE(server_field_traits(FanyImeFieldTraits::Elevated, false) == (MSIME_TRAIT_ELEVATED | MSIME_TRAIT_KNOWN));
    // 核心的其余位只有核心自己或别的宿主会报，TIP 的帧里出现也不转交。
    REQUIRE(server_field_traits(0xFFFFFFFFu, false) ==
            (MSIME_TRAIT_SECURE | MSIME_TRAIT_ELEVATED | MSIME_TRAIT_KNOWN));
    // 上屏的特效只读带上屏的回复里核心写的 feedback，0 也是一个答案；没有上屏、没有 feedback（Server 合成的上屏）都没有特效。
    REQUIRE(commit_feedback(nlohmann::json{{"commit", "你好"}, {"feedback", {{"typing_effect", 70u}}}}) == 70u);
    REQUIRE(commit_feedback(nlohmann::json{{"commit", "你好"}, {"feedback", {{"typing_effect", 0u}}}}) == 0u);
    REQUIRE(!commit_feedback(nlohmann::json{{"commit", nullptr}, {"feedback", {{"typing_effect", 70u}}}}));
    REQUIRE(!commit_feedback(nlohmann::json{{"commit", "你好"}}));
    REQUIRE(!commit_feedback(nlohmann::json{{"commit", "你好"}, {"feedback", nullptr}}));
    REQUIRE(!commit_feedback(nlohmann::json::array()));
    // TIP 自己写进文档的上屏：韩文（4）、注音（6）、越南文（7）、藏文（8）这些方案，以及整句改字中的全拼、双拼。
    REQUIRE(!tip_writes_commits(nlohmann::json{{"scheme", 0u}, {"conversion", ""}}));
    REQUIRE(tip_writes_commits(nlohmann::json{{"scheme", 0u}, {"conversion", "你好"}}));
    for (uint32_t inline_scheme : {4u, 6u, 7u, 8u})
      REQUIRE(tip_writes_commits(nlohmann::json{{"scheme", inline_scheme}}));
    for (uint32_t ordinary : {0u, 1u, 2u, 3u, 5u, 9u})
      REQUIRE(!tip_writes_commits(nlohmann::json{{"scheme", ordinary}}));
    REQUIRE(!tip_writes_commits(nlohmann::json()));

    std::cout << "Windows key sound policy checks passed\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
