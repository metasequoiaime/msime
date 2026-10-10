#pragma once
#include "TypingEffectPolicy.h"
#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <vector>

namespace msime::windows {
// 光标处的打字特效浮层（TypingEffectOverlay）怎么画：火花、光标行闪光、连击徽标，以及 Power Mode 时候选卡片的抖动。数值都照搬 macOS 的 TypingEffectPanel.mm，长度单位是设备无关像素（macOS 的 pt），时间是毫秒。纯计算，不依赖 Windows 头文件，主机上的单测直接包含。

// 火花飞行的方块的半边长：一颗火花最多活 0.6 秒、初速最多 230，加上重力，落在这个范围里就淡出了。
constexpr float typing_spark_reach = 80.0f;
constexpr float typing_screen_margin = 4.0f;
constexpr float typing_badge_gap = 4.0f;
constexpr float typing_badge_font_size = 12.0f;
constexpr float typing_badge_inset = 7.0f;
constexpr float typing_badge_height = 20.0f;
// 一次按键、一次上屏的迸发时长：够短，打字的下一个键会另起一次迸发而不是连成一股。
constexpr uint32_t typing_key_burst_millis = 60u;
constexpr uint32_t typing_commit_burst_millis = 100u;
// 最后一次特效之后多久收起浮层：火花的寿命再留些余量；徽标显示着时更久，让人看得清数字。
constexpr uint32_t typing_spark_settle_millis = 700u;
constexpr uint32_t typing_badge_settle_millis = 1200u;
// 没有特效包给出时长时，按键和上屏的闪光长度。
constexpr uint32_t typing_key_flash_millis = 150u;
constexpr uint32_t typing_commit_flash_millis = 250u;
// 火花：寿命 0.45±0.15 秒，初速 120±60（Power Mode 170±60），向上 ±36° 喷出，重力 320 向下，透明度每秒降 1.8，大小是 16 的软圆点乘 0.45±0.2、每秒缩 0.6。
constexpr uint32_t typing_spark_lifetime_millis = 450u;
constexpr uint32_t typing_spark_lifetime_range_millis = 150u;
constexpr float typing_spark_velocity = 120.0f;
constexpr float typing_power_spark_velocity = 170.0f;
constexpr float typing_spark_velocity_range = 60.0f;
constexpr float typing_spark_spread = 0.2f * 3.14159265f;
constexpr float typing_spark_gravity = 320.0f;
constexpr float typing_spark_alpha_speed = 1.8f;
constexpr float typing_spark_image = 16.0f;
constexpr float typing_spark_scale = 0.45f;
constexpr float typing_spark_scale_range = 0.2f;
constexpr float typing_spark_scale_speed = 0.6f;
// 同时活着的火花上限：最多的一次（特效包 64 颗的上屏）是 128 颗，留出前一次还没落完的余量。浮层按这个数预留，画帧时不再分配。
constexpr size_t typing_max_sparks = 256u;
// Power Mode 的抖动：0.12 秒里横向 0、+a、-a、+a/2、0，a 随强度 0.5 到 2。
constexpr uint32_t typing_shake_millis = 120u;
// 浮层重画的间隔，约 60 帧；系统计时器的粒度会把它放宽到 16 毫秒左右。
constexpr uint32_t typing_overlay_frame_millis = 16u;

// effect_intensity 0-100 换成 0.4 到 1 的强度，50 是 0.7。
inline float typing_effect_strength(uint32_t intensity) {
  return 0.4f + 0.6f * static_cast<float>((std::min)(intensity, 100u)) / 100.0f;
}

// 实际画出来的样式。火花和 Power Mode 会在屏幕上动，开着节电模式时退回闪光，和 macOS 的低电量模式一样。Windows 的「显示动画」关掉时什么都不闪、不动，只留连击徽标，这是 Windows 候选卡片一直以来的规则（macOS 的「减弱动态效果」只去掉移动），这里把火花也退回闪光，再由调用方按「显示动画」决定闪不闪。
inline TypingEffectStyle typing_effect_drawn_style(TypingEffectStyle style, bool animations, bool power_saver) {
  if ((style == TypingEffectStyle::sparks || style == TypingEffectStyle::power_mode) && (!animations || power_saver))
    return TypingEffectStyle::flash;
  return style;
}

// 候选卡片闪不闪：闪光和 Power Mode 闪，火花样式由浮层在光标处画火花、卡片不闪。浮层不可用时退回从前的样子，每种样式都闪卡片。
inline bool typing_effect_card_flashes(TypingEffectStyle drawn, bool overlay_available) {
  switch (drawn) {
  case TypingEffectStyle::flash:
  case TypingEffectStyle::power_mode:
    return true;
  case TypingEffectStyle::sparks:
    return !overlay_available;
  case TypingEffectStyle::off:
    break;
  }
  return false;
}

// 连击徽标上的字，不到两键时为空：一键还不算连击。
inline std::wstring typing_effect_combo_text(uint32_t combo) {
  return combo >= 2 ? L"连击 ×" + std::to_wstring(combo) : std::wstring{};
}

// 一次迸发的火花数。特效包给了 particles 就是每个键这么多颗（上屏加倍），0 表示这个包的火花样式不画火花；没给时按 macOS 的发射率 220×强度×倍数每秒，乘以迸发时长，Power Mode 和上屏各加倍。
inline uint32_t typing_spark_count(bool power, bool commit, uint32_t intensity, std::optional<uint32_t> particles) {
  if (particles)
    return (std::min)(*particles, typing_effect_max_particles) * (commit ? 2u : 1u);
  const float burst = (power ? 2.0f : 1.0f) * (commit ? 2.0f : 1.0f);
  const float seconds = static_cast<float>(commit ? typing_commit_burst_millis : typing_key_burst_millis) / 1000.0f;
  return static_cast<uint32_t>(std::lround(220.0f * typing_effect_strength(intensity) * burst * seconds));
}

// 这一下的火花颜色：特效包的颜色按连击数一键换一个，没有特效包颜色时用强调色。
inline uint32_t typing_spark_color(const TypingEffectPalette &palette, uint32_t combo, uint32_t accent) {
  if (palette.color_count == 0)
    return accent & 0xFFFFFFu;
  return palette.colors[combo % (std::min)(palette.color_count, typing_effect_max_colors)] & 0xFFFFFFu;
}

// 闪光和徽标的颜色：特效包的第一个颜色，没有时用强调色。
inline uint32_t typing_flash_color(const TypingEffectPalette &palette, uint32_t accent) {
  return palette.color_count ? palette.colors[0] & 0xFFFFFFu : accent & 0xFFFFFFu;
}

// 闪光长度：特效包的 duration_ms；没给时按键 150 毫秒、上屏 250 毫秒。
inline uint32_t typing_flash_duration(uint32_t flash_millis, bool commit) {
  if (commit && flash_millis == typing_effect_flash_millis)
    return typing_commit_flash_millis;
  return flash_millis;
}

// 光标行闪光的起始不透明度，线性淡出。
inline float typing_caret_flash_peak(bool commit, uint32_t intensity) {
  return (commit ? 0.32f : 0.2f) * typing_effect_strength(intensity) * 1.5f;
}
inline float typing_caret_flash_alpha(float peak, uint32_t elapsed, uint32_t duration) {
  if (duration == 0 || elapsed >= duration)
    return 0.0f;
  return peak * (1.0f - static_cast<float>(elapsed) / static_cast<float>(duration));
}

// 关键帧之间线性插值，关键帧在时长里均匀分布，和 CAKeyframeAnimation 的默认计时一样。
template <size_t N> float typing_keyframes(const float (&values)[N], uint32_t elapsed, uint32_t duration) {
  if (duration == 0 || elapsed >= duration)
    return values[N - 1];
  const float position = static_cast<float>(elapsed) / static_cast<float>(duration) * static_cast<float>(N - 1);
  const size_t index = static_cast<size_t>(position);
  const float fraction = position - static_cast<float>(index);
  return values[index] + (values[index + 1] - values[index]) * fraction;
}

// Power Mode 时候选卡片横向的位移（设备无关像素），抖完回到 0。
inline float typing_shake_offset(uint32_t elapsed, uint32_t intensity) {
  const float amplitude = 0.5f + 1.5f * static_cast<float>((std::min)(intensity, 100u)) / 100.0f;
  const float values[] = {0.0f, amplitude, -amplitude, amplitude / 2.0f, 0.0f};
  return typing_keyframes(values, elapsed, typing_shake_millis);
}

// 徽标的弹跳：升档时 1、1.35、0.95、1 共 0.35 秒，Power Mode 每键 1、1.12、1 共 0.15 秒，其他时候不弹。
enum class TypingBadgeBounce : uint32_t { none = 0, power = 1, tier_up = 2 };
inline TypingBadgeBounce typing_badge_bounce(bool tier_up, bool power, bool animations) {
  if (!animations)
    return TypingBadgeBounce::none;
  return tier_up ? TypingBadgeBounce::tier_up : (power ? TypingBadgeBounce::power : TypingBadgeBounce::none);
}
inline uint32_t typing_badge_bounce_millis(TypingBadgeBounce bounce) {
  return bounce == TypingBadgeBounce::tier_up ? 350u : (bounce == TypingBadgeBounce::power ? 150u : 0u);
}
inline float typing_badge_bounce_scale(TypingBadgeBounce bounce, uint32_t elapsed) {
  if (bounce == TypingBadgeBounce::tier_up) {
    const float values[] = {1.0f, 1.35f, 0.95f, 1.0f};
    return typing_keyframes(values, elapsed, typing_badge_bounce_millis(bounce));
  }
  if (bounce == TypingBadgeBounce::power) {
    const float values[] = {1.0f, 1.12f, 1.0f};
    return typing_keyframes(values, elapsed, typing_badge_bounce_millis(bounce));
  }
  return 1.0f;
}

// 一颗火花：出生的位置（屏幕物理像素）、速度（物理像素每秒）、出生时刻和寿命（毫秒）、大小倍数和颜色。位置按屏幕坐标存，浮层换位置时已经飞出去的火花不会跟着跳。
struct TypingSpark {
  float x = 0.0f;
  float y = 0.0f;
  float vx = 0.0f;
  float vy = 0.0f;
  uint64_t born = 0;
  uint32_t life = 0;
  float scale = 0.0f;
  uint32_t color = 0;
};
// 一颗火花在某一刻的样子，相对同一个屏幕坐标系。
struct TypingSparkFrame {
  float x = 0.0f;
  float y = 0.0f;
  float radius = 0.0f;
  float alpha = 0.0f;
};

// 可复现的伪随机数（xorshift32），同一个种子得到同一批火花，单测才能检查它们的范围。返回 [0, 1)。
inline float typing_random(uint32_t &state) {
  if (state == 0)
    state = 0x9E3779B9u;
  state ^= state << 13;
  state ^= state >> 17;
  state ^= state << 5;
  return static_cast<float>(state >> 8) / 16777216.0f;
}

// 在 (x, y) 处迸发 `count` 颗火花，出生时刻在迸发时长里均匀铺开，像 macOS 的发射器那样是一小股而不是一下子全出来。`scale` 是每个设备无关像素的物理像素数。超过 typing_max_sparks 的部分不发，向量不会在这里扩容。
inline void emit_typing_sparks(std::vector<TypingSpark> &sparks, uint32_t count, float x, float y, bool power, bool commit,
                               uint64_t now, uint32_t color, float scale, uint32_t &seed) {
  const uint32_t burst = commit ? typing_commit_burst_millis : typing_key_burst_millis;
  const float base = power ? typing_power_spark_velocity : typing_spark_velocity;
  for (uint32_t index = 0; index < count && sparks.size() < typing_max_sparks; ++index) {
    TypingSpark spark;
    const float angle = -1.5707963f + (typing_random(seed) * 2.0f - 1.0f) * typing_spark_spread;
    const float speed = (base + (typing_random(seed) * 2.0f - 1.0f) * typing_spark_velocity_range) * scale;
    spark.x = x;
    spark.y = y;
    spark.vx = std::cos(angle) * speed;
    spark.vy = std::sin(angle) * speed;
    spark.born = now + static_cast<uint64_t>(count > 1 ? burst * index / count : 0u);
    const float life = static_cast<float>(typing_spark_lifetime_millis) +
                       (typing_random(seed) * 2.0f - 1.0f) * static_cast<float>(typing_spark_lifetime_range_millis);
    spark.life = static_cast<uint32_t>(life);
    spark.scale = typing_spark_scale + (typing_random(seed) * 2.0f - 1.0f) * typing_spark_scale_range;
    spark.color = color & 0xFFFFFFu;
    sparks.push_back(spark);
  }
}

// 火花在 `now` 时的位置、半径（物理像素）和不透明度；还没出生时为空，死了也为空。
inline std::optional<TypingSparkFrame> typing_spark_at(const TypingSpark &spark, uint64_t now, float scale) {
  if (now < spark.born)
    return std::nullopt;
  const uint64_t age = now - spark.born;
  if (age >= spark.life)
    return std::nullopt;
  const float t = static_cast<float>(age) / 1000.0f;
  const float alpha = 1.0f - typing_spark_alpha_speed * t;
  const float size = spark.scale - typing_spark_scale_speed * t;
  if (alpha <= 0.0f || size <= 0.0f)
    return std::nullopt;
  TypingSparkFrame frame;
  frame.x = spark.x + spark.vx * t;
  frame.y = spark.y + spark.vy * t + 0.5f * typing_spark_gravity * scale * t * t;
  frame.radius = typing_spark_image * size * scale / 2.0f;
  frame.alpha = (std::min)(1.0f, alpha);
  return frame;
}

// 是否还有火花要画：还没出生的也算。
inline bool typing_sparks_alive(const std::vector<TypingSpark> &sparks, uint64_t now) {
  return std::any_of(sparks.begin(), sparks.end(),
                     [now](const TypingSpark &spark) { return now < spark.born + spark.life; });
}
// 去掉寿命已尽的火花，原地进行，不分配内存。
inline void prune_typing_sparks(std::vector<TypingSpark> &sparks, uint64_t now) {
  sparks.erase(std::remove_if(sparks.begin(), sparks.end(),
                              [now](const TypingSpark &spark) { return now >= spark.born + spark.life; }),
               sparks.end());
}

// 屏幕上的矩形，物理像素。
struct TypingRect {
  float left = 0.0f;
  float top = 0.0f;
  float right = 0.0f;
  float bottom = 0.0f;
  float width() const { return right - left; }
  float height() const { return bottom - top; }
};
inline bool typing_rect_usable(const TypingRect &rect) {
  return std::isfinite(rect.left) && std::isfinite(rect.top) && std::isfinite(rect.right) &&
         std::isfinite(rect.bottom) && rect.bottom > rect.top && rect.right >= rect.left;
}
inline TypingRect typing_rect_union(const TypingRect &a, const TypingRect &b) {
  return {(std::min)(a.left, b.left), (std::min)(a.top, b.top), (std::max)(a.right, b.right),
          (std::max)(a.bottom, b.bottom)};
}

struct TypingOverlayPlacementInput {
  // 光标所在行的矩形；没有可用的光标时为空。
  std::optional<TypingRect> caret;
  // 候选卡片（不含阴影）在屏幕上的矩形；候选窗不在屏幕上时为空。
  std::optional<TypingRect> card;
  // 火花起点所在的显示器，徽标夹在它里面。
  TypingRect monitor;
  // 每个设备无关像素的物理像素数。
  float scale = 1.0f;
  // 徽标的宽度（设备无关像素），没有徽标时为 0。
  float badge_width = 0.0f;
};
struct TypingOverlayPlacement {
  // 浮层窗口的屏幕矩形，取整到像素。
  TypingRect frame;
  // 火花的起点，屏幕坐标。
  float origin_x = 0.0f;
  float origin_y = 0.0f;
  // 徽标的屏幕矩形。
  std::optional<TypingRect> badge;
  // 没有候选卡片时光标行的闪光，屏幕矩形。
  std::optional<TypingRect> caret_flash;
};

// 浮层放在哪里，和 macOS 的 presentEffect: 同一套规则：火花从光标行的左端、行高中间升起，没有可用的光标时从卡片左上角往里 12 处；窗口是起点周围 ±80 的方块，再并上徽标。徽标有卡片时贴在卡片右上角的上方，在候选窗外面不挡候选；没有卡片时在光标右上方；都夹在显示器里。光标和卡片都没有时为空，什么都不画。
inline std::optional<TypingOverlayPlacement> typing_overlay_placement(const TypingOverlayPlacementInput &input) {
  const bool caret = input.caret && typing_rect_usable(*input.caret);
  const bool card = input.card && typing_rect_usable(*input.card);
  if (!caret && !card)
    return std::nullopt;
  const float scale = input.scale > 0.0f ? input.scale : 1.0f;
  TypingOverlayPlacement placement;
  if (caret) {
    placement.origin_x = input.caret->left;
    placement.origin_y = (input.caret->top + input.caret->bottom) / 2.0f;
  } else {
    placement.origin_x = input.card->left + 12.0f * scale;
    placement.origin_y = input.card->top;
  }
  const float reach = typing_spark_reach * scale;
  TypingRect frame{placement.origin_x - reach, placement.origin_y - reach, placement.origin_x + reach,
                   placement.origin_y + reach};
  if (input.badge_width > 0.0f) {
    const float width = std::ceil(input.badge_width * scale);
    const float height = typing_badge_height * scale;
    const float gap = typing_badge_gap * scale;
    const float margin = typing_screen_margin * scale;
    float left = card ? input.card->right - width : input.caret->right + gap;
    float top = card ? input.card->top - gap - height : input.caret->top - gap - height;
    left = (std::max)(input.monitor.left + margin, (std::min)(left, input.monitor.right - margin - width));
    top = (std::max)(input.monitor.top + margin, (std::min)(top, input.monitor.bottom - margin - height));
    placement.badge = TypingRect{left, top, left + width, top + height};
    frame = typing_rect_union(frame, *placement.badge);
  }
  if (caret && !card) {
    const float pad = 2.0f * scale;
    placement.caret_flash = TypingRect{input.caret->left - pad, input.caret->top,
                                       input.caret->left - pad + 2.0f * pad + (std::max)(input.caret->width(), pad),
                                       input.caret->bottom};
    frame = typing_rect_union(frame, *placement.caret_flash);
  }
  placement.frame = {std::floor(frame.left), std::floor(frame.top), std::ceil(frame.right), std::ceil(frame.bottom)};
  return placement;
}

// 上一次的火花还在飞时，新窗口要把它们也框进去；但光标跳得太远时并起来的窗口会很大，每帧都要整块上传，这时放弃旧火花。上限是单个方块面积的四倍。
inline bool typing_overlay_keeps_previous(const TypingRect &previous, const TypingRect &next) {
  const TypingRect merged = typing_rect_union(previous, next);
  const float single = (std::max)(1.0f, next.width() * next.height());
  return merged.width() * merged.height() <= 4.0f * single;
}
} // namespace msime::windows
