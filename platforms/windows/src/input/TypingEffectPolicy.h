#pragma once
#include <algorithm>
#include <cmath>
#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string_view>

namespace msime::windows {
// The events msime_client_typing_effect takes beyond the key_sound classes 0-3, the flag for an auto-repeated key (drawn but not counted toward the combo), and the flag that keeps its tier-up sound quiet.
constexpr uint32_t typing_effect_commit_event = 4u;
constexpr uint32_t typing_effect_repeat_flag = 0x100u;
constexpr uint32_t typing_effect_muted_flag = 0x200u;
// 宿主自己加在答案上的标记：这次是上屏而不是按键。答案只用到第 20 位，第 31 位是 TypingEffectSignal 的等待位，这一位两边都不碰。上屏的火花多一倍、闪光长一些，和 macOS 一样。
constexpr uint32_t typing_effect_commit_mark = 1u << 24;
// How long one flash takes to fade out, and how long a combo count stays on the card after the last key: the library ends a combo after 3000 ms without a counted key, so the count it last reported is stale from then on.
constexpr uint32_t typing_effect_flash_millis = 150u;
constexpr uint32_t typing_effect_combo_millis = 3000u;

enum class TypingEffectStyle : uint32_t { off = 0, flash = 1, sparks = 2, power_mode = 3 };

struct TypingEffect {
  uint32_t combo = 0;
  bool tier_up = false;
  TypingEffectStyle style = TypingEffectStyle::off;
  // 上屏（typing_effect_commit_mark），不是一次按键。
  bool commit = false;
};

// The event for a key the Server handled, from its key_sound class. An auto-repeat of a held key is flagged so it does not count toward the combo or reach a new tier. Muted while sounds must stay quiet, so the library neither queues nor reports the tier-up sound.
inline uint32_t typing_effect_key_event(uint32_t key_class, bool sound_allowed, bool auto_repeat) {
  return key_class | (auto_repeat ? typing_effect_repeat_flag : 0u) | (sound_allowed ? 0u : typing_effect_muted_flag);
}
inline uint32_t typing_effect_commit(bool sound_allowed) {
  return typing_effect_key_event(typing_effect_commit_event, sound_allowed, false);
}
// 给上屏的答案打上标记。0 仍是 0：特效和连击都关着时不该为上屏唤醒界面线程。
inline uint32_t typing_effect_mark_commit(uint32_t packed) {
  return packed ? packed | typing_effect_commit_mark : 0u;
}

// Unpacks msime_client_typing_effect's return value: bits 0-15 the combo count, bit 16 a new tier, bits 17-19 the style. A style number this host does not know is drawn as the strongest one it does.
inline TypingEffect decode_typing_effect(uint32_t packed) {
  TypingEffect effect;
  effect.combo = packed & 0xFFFFu;
  effect.tier_up = (packed & 0x10000u) != 0;
  const uint32_t style = (packed >> 17) & 0x7u;
  effect.style = static_cast<TypingEffectStyle>((std::min)(style, 3u));
  effect.commit = (packed & typing_effect_commit_mark) != 0;
  return effect;
}

// 卡片闪光用到的 msime_client_typing_effect_settings 部分：强度、闪光时长（特效包的 duration_ms，没有时用宿主自己的 150 ms）和闪光颜色（特效包的第一个颜色，没有时用主题强调色）。特效包的整张颜色表和每键火花数另走 TypingEffectPalette，给光标浮层用。
struct TypingEffectSettings {
  uint32_t intensity = 50;
  uint32_t flash_millis = typing_effect_flash_millis;
  std::optional<uint32_t> color;
  friend bool operator==(const TypingEffectSettings &left, const TypingEffectSettings &right) {
    return left.intensity == right.intensity && left.flash_millis == right.flash_millis && left.color == right.color;
  }
};

// "#RRGGBB" as 0xRRGGBB, the only form the settings carry; anything else is no colour.
inline std::optional<uint32_t> typing_effect_rgb(std::string_view text) {
  if (text.size() != 7 || text.front() != '#')
    return std::nullopt;
  uint32_t value = 0;
  for (const char ch : text.substr(1)) {
    uint32_t digit = 0;
    if (ch >= '0' && ch <= '9')
      digit = static_cast<uint32_t>(ch - '0');
    else if (ch >= 'a' && ch <= 'f')
      digit = static_cast<uint32_t>(ch - 'a' + 10);
    else if (ch >= 'A' && ch <= 'F')
      digit = static_cast<uint32_t>(ch - 'A' + 10);
    else
      return std::nullopt;
    value = (value << 4) | digit;
  }
  return value;
}

// The settings clamped to the ranges the library documents (intensity 0-100, flash 60-1500 ms), so a value out of range from any producer still draws.
inline TypingEffectSettings resolve_typing_effect_settings(uint32_t intensity, std::optional<uint32_t> duration_ms, std::optional<uint32_t> color) {
  TypingEffectSettings settings;
  settings.intensity = (std::min)(intensity, 100u);
  if (duration_ms)
    settings.flash_millis = (std::max)(60u, (std::min)(*duration_ms, 1500u));
  if (color)
    settings.color = *color & 0xFFFFFFu;
  return settings;
}

// Packs the settings into one word the input thread hands the UI thread without a lock: bits 0-7 the intensity, 8-19 the flash length, 20 whether a colour is set, 32-55 the colour, 63 that settings were published at all.
inline uint64_t pack_typing_effect_settings(const TypingEffectSettings &settings) {
  return (1ull << 63) | (static_cast<uint64_t>(settings.color.value_or(0) & 0xFFFFFFu) << 32) |
         (settings.color ? (1ull << 20) : 0ull) | (static_cast<uint64_t>(settings.flash_millis & 0xFFFu) << 8) |
         static_cast<uint64_t>(settings.intensity & 0xFFu);
}
// Nothing for a word nobody published, which keeps the host's own preference-derived intensity.
inline std::optional<TypingEffectSettings> unpack_typing_effect_settings(uint64_t packed) {
  if ((packed & (1ull << 63)) == 0)
    return std::nullopt;
  std::optional<uint32_t> color;
  if (packed & (1ull << 20))
    color = static_cast<uint32_t>((packed >> 32) & 0xFFFFFFu);
  return resolve_typing_effect_settings(static_cast<uint32_t>(packed & 0xFFu), static_cast<uint32_t>((packed >> 8) & 0xFFFu), color);
}

// 按键后 `elapsed` 毫秒时卡片闪光的不透明度，淡完或不画时为 0。越强的样式和新的档位闪得越亮。某种样式到底闪不闪卡片由 typing_effect_card_flashes（TypingEffectOverlayPolicy.h）决定：火花样式只在画火花的光标浮层不可用时才闪卡片。effect_intensity 50 是标准强度，100 加倍。`flash_millis` 是闪光淡完的时长，即特效包的 duration_ms。
inline float typing_effect_flash_alpha(const TypingEffect &effect, uint32_t intensity, uint64_t elapsed,
                                       uint32_t flash_millis = typing_effect_flash_millis) {
  if (effect.style == TypingEffectStyle::off || intensity == 0 || flash_millis == 0 || elapsed >= flash_millis)
    return 0.0f;
  float base = 0.35f;
  if (effect.style == TypingEffectStyle::sparks)
    base = 0.5f;
  else if (effect.style == TypingEffectStyle::power_mode)
    base = 0.7f;
  if (effect.tier_up)
    base += 0.3f;
  const float strength = static_cast<float>((std::min)(intensity, 100u)) / 50.0f;
  const float remaining = 1.0f - static_cast<float>(elapsed) / static_cast<float>(flash_millis);
  return (std::min)(1.0f, base * strength) * remaining;
}

// 特效包的颜色（1-4 个）和每次按键迸出的火花数（0-64）。火花一键一色轮流取这些颜色，第一个颜色同时是闪光和连击徽标的颜色（TypingEffectSettings::color）。没有特效包，或包里没写时，颜色为空、火花数为空，用宿主自己的。
constexpr uint32_t typing_effect_max_colors = 4u;
constexpr uint32_t typing_effect_max_particles = 64u;
struct TypingEffectPalette {
  std::array<uint32_t, typing_effect_max_colors> colors{};
  uint32_t color_count = 0;
  std::optional<uint32_t> particles;
  friend bool operator==(const TypingEffectPalette &left, const TypingEffectPalette &right) {
    if (left.color_count != right.color_count || left.particles != right.particles)
      return false;
    for (uint32_t index = 0; index < left.color_count; ++index)
      if (left.colors[index] != right.colors[index])
        return false;
    return true;
  }
};
// 多出来的颜色丢掉、火花数夹到 0-64，任何来源的值都画得出来。
inline TypingEffectPalette resolve_typing_effect_palette(const uint32_t *colors, size_t count, std::optional<uint32_t> particles) {
  TypingEffectPalette palette;
  for (size_t index = 0; colors && index < count && palette.color_count < typing_effect_max_colors; ++index)
    palette.colors[palette.color_count++] = colors[index] & 0xFFFFFFu;
  if (particles)
    palette.particles = (std::min)(*particles, typing_effect_max_particles);
  return palette;
}
// 输入线程交给界面线程的两个字，不加锁：第一个是前两个颜色（各 24 位）、颜色个数（第 48-50 位）、火花数（第 51-57 位）和它给没给（第 58 位），第 63 位表示发布过；第二个是后两个颜色。
inline std::array<uint64_t, 2> pack_typing_effect_palette(const TypingEffectPalette &palette) {
  const uint32_t count = (std::min)(palette.color_count, typing_effect_max_colors);
  auto color = [&](uint32_t index) { return index < count ? static_cast<uint64_t>(palette.colors[index] & 0xFFFFFFu) : 0ull; };
  uint64_t first = (1ull << 63) | color(0) | (color(1) << 24) | (static_cast<uint64_t>(count) << 48);
  if (palette.particles)
    first |= (static_cast<uint64_t>((std::min)(*palette.particles, typing_effect_max_particles)) << 51) | (1ull << 58);
  return {first, color(2) | (color(3) << 24)};
}
// 没发布过的字是空调色板：没有颜色，火花数用宿主自己的。
inline TypingEffectPalette unpack_typing_effect_palette(uint64_t first, uint64_t second) {
  TypingEffectPalette palette;
  if ((first & (1ull << 63)) == 0)
    return palette;
  palette.color_count = (std::min)(static_cast<uint32_t>((first >> 48) & 0x7u), typing_effect_max_colors);
  const uint64_t words[4] = {first, first >> 24, second, second >> 24};
  for (uint32_t index = 0; index < palette.color_count; ++index)
    palette.colors[index] = static_cast<uint32_t>(words[index] & 0xFFFFFFu);
  if (first & (1ull << 58))
    palette.particles = (std::min)(static_cast<uint32_t>((first >> 51) & 0x7Fu), typing_effect_max_particles);
  return palette;
}

// 按帧变化的不透明度量化到 1/32：msimeui 按颜色缓存画刷，缓存只增不减，量化之后一次闪光最多多出几十把画刷，而不是每帧一把。
inline float typing_effect_quantized_alpha(float alpha) {
  const float clamped = (std::min)(1.0f, (std::max)(0.0f, alpha));
  return std::round(clamped * 32.0f) / 32.0f;
}

// Whether the card shows the combo count: a streak of at least two keys, reported within the library's idle window. A count of one is every first key, not a combo.
inline bool typing_effect_shows_combo(uint32_t combo, uint64_t elapsed) {
  return combo >= 2 && elapsed < typing_effect_combo_millis;
}
} // namespace msime::windows
