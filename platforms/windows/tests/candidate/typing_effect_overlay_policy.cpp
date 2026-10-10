#include "TypingEffectOverlayPolicy.h"

#include <cmath>
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
void require(bool value, int line) {
  if (!value)
    throw std::runtime_error("typing effect overlay policy failed at line " + std::to_string(line));
}
#define REQUIRE(value) require((value), __LINE__)
bool near(float left, float right) { return std::fabs(left - right) < 0.001f; }
} // namespace

int main() {
  try {
    // 火花和 Power Mode 在节电或关掉动画时退回闪光；闪光和关着的样式不变。
    REQUIRE(typing_effect_drawn_style(TypingEffectStyle::sparks, true, false) == TypingEffectStyle::sparks);
    REQUIRE(typing_effect_drawn_style(TypingEffectStyle::power_mode, true, true) == TypingEffectStyle::flash);
    REQUIRE(typing_effect_drawn_style(TypingEffectStyle::sparks, false, false) == TypingEffectStyle::flash);
    REQUIRE(typing_effect_drawn_style(TypingEffectStyle::flash, false, true) == TypingEffectStyle::flash);
    REQUIRE(typing_effect_drawn_style(TypingEffectStyle::off, true, false) == TypingEffectStyle::off);
    // 卡片只为闪光和 Power Mode 闪；火花样式的火花在光标处，浮层不可用时才退回闪卡片。
    REQUIRE(typing_effect_card_flashes(TypingEffectStyle::flash, true));
    REQUIRE(typing_effect_card_flashes(TypingEffectStyle::power_mode, true));
    REQUIRE(!typing_effect_card_flashes(TypingEffectStyle::sparks, true));
    REQUIRE(typing_effect_card_flashes(TypingEffectStyle::sparks, false));
    REQUIRE(!typing_effect_card_flashes(TypingEffectStyle::off, false));

    // 徽标的字与 macOS 相同，一键不算连击。
    REQUIRE(typing_effect_combo_text(1).empty());
    REQUIRE(typing_effect_combo_text(2) == L"连击 ×2");
    REQUIRE(typing_effect_combo_text(120) == L"连击 ×120");

    // 一次迸发的火花数：没有特效包时按 macOS 的发射率，强度 50 时一键 9 颗，Power Mode 和上屏各加倍。
    REQUIRE(typing_spark_count(false, false, 50, std::nullopt) == 9u);
    REQUIRE(typing_spark_count(true, false, 50, std::nullopt) == 18u);
    REQUIRE(typing_spark_count(false, true, 50, std::nullopt) == 31u);
    REQUIRE(typing_spark_count(true, true, 100, std::nullopt) == 88u);
    REQUIRE(typing_spark_count(false, false, 0, std::nullopt) > 0u);
    // 特效包的 particles 是每个键的颗数，上屏加倍，0 不画火花，超过 64 的按 64。
    REQUIRE(typing_spark_count(true, false, 50, 24u) == 24u);
    REQUIRE(typing_spark_count(false, true, 50, 24u) == 48u);
    REQUIRE(typing_spark_count(false, false, 50, 0u) == 0u);
    REQUIRE(typing_spark_count(false, true, 50, 500u) == 128u);
    REQUIRE(typing_spark_count(false, true, 50, 500u) <= typing_max_sparks);

    // 火花一键一色轮流取特效包的颜色，闪光和徽标用第一个；没有颜色时都用强调色。
    const uint32_t colors[] = {0xFF0000u, 0x00FF00u, 0x0000FFu};
    const auto pack = resolve_typing_effect_palette(colors, 3, std::nullopt);
    REQUIRE(typing_spark_color(pack, 0, 0x123456u) == 0xFF0000u);
    REQUIRE(typing_spark_color(pack, 4, 0x123456u) == 0x00FF00u);
    REQUIRE(typing_spark_color(pack, 5, 0x123456u) == 0x0000FFu);
    REQUIRE(typing_flash_color(pack, 0x123456u) == 0xFF0000u);
    REQUIRE(typing_spark_color(TypingEffectPalette{}, 7, 0x123456u) == 0x123456u);
    REQUIRE(typing_flash_color(TypingEffectPalette{}, 0x123456u) == 0x123456u);

    // 闪光长度：按键 150、上屏 250，特效包给了时长就用它。
    REQUIRE(typing_flash_duration(typing_effect_flash_millis, false) == 150u);
    REQUIRE(typing_flash_duration(typing_effect_flash_millis, true) == 250u);
    REQUIRE(typing_flash_duration(900u, true) == 900u);
    // 光标行的闪光：上屏比按键亮，随强度变亮，线性淡出。
    REQUIRE(typing_caret_flash_peak(true, 50) > typing_caret_flash_peak(false, 50));
    REQUIRE(typing_caret_flash_peak(false, 100) > typing_caret_flash_peak(false, 0));
    REQUIRE(near(typing_caret_flash_alpha(0.4f, 75, 150), 0.2f));
    REQUIRE(typing_caret_flash_alpha(0.4f, 150, 150) == 0.0f);

    // Power Mode 的抖动 0.12 秒，强度 50 时振幅 1.25，抖完回到原位。
    REQUIRE(typing_shake_offset(0, 50) == 0.0f);
    REQUIRE(near(typing_shake_offset(30, 50), 1.25f));
    REQUIRE(near(typing_shake_offset(60, 50), -1.25f));
    REQUIRE(near(typing_shake_offset(90, 50), 0.625f));
    REQUIRE(typing_shake_offset(120, 50) == 0.0f);
    REQUIRE(near(typing_shake_offset(30, 100), 2.0f));

    // 徽标升档时弹得大、Power Mode 每键轻弹一下，关掉动画时不弹。
    REQUIRE(typing_badge_bounce(true, true, true) == TypingBadgeBounce::tier_up);
    REQUIRE(typing_badge_bounce(false, true, true) == TypingBadgeBounce::power);
    REQUIRE(typing_badge_bounce(false, false, true) == TypingBadgeBounce::none);
    REQUIRE(typing_badge_bounce(true, true, false) == TypingBadgeBounce::none);
    REQUIRE(near(typing_badge_bounce_scale(TypingBadgeBounce::tier_up, 0), 1.0f));
    REQUIRE(typing_badge_bounce_scale(TypingBadgeBounce::tier_up, 116) > 1.3f);
    REQUIRE(near(typing_badge_bounce_scale(TypingBadgeBounce::tier_up, 350), 1.0f));
    REQUIRE(near(typing_badge_bounce_scale(TypingBadgeBounce::power, 75), 1.12f));
    REQUIRE(near(typing_badge_bounce_scale(TypingBadgeBounce::none, 75), 1.0f));

    // 火花：同一个种子得到同一批；出生时刻铺在迸发时长里；先向上飞再被重力拉下来；寿命里淡出、缩小，到期就没了。
    std::vector<TypingSpark> sparks;
    sparks.reserve(typing_max_sparks);
    uint32_t seed = 42;
    emit_typing_sparks(sparks, 9, 500.0f, 300.0f, false, false, 1000, 0xFF8800u, 2.0f, seed);
    REQUIRE(sparks.size() == 9u);
    std::vector<TypingSpark> again;
    uint32_t same_seed = 42;
    emit_typing_sparks(again, 9, 500.0f, 300.0f, false, false, 1000, 0xFF8800u, 2.0f, same_seed);
    REQUIRE(again[3].vx == sparks[3].vx && again[3].life == sparks[3].life);
    for (const auto &spark : sparks) {
      REQUIRE(spark.born >= 1000 && spark.born < 1000 + typing_key_burst_millis);
      REQUIRE(spark.life >= 300u && spark.life <= 600u);
      REQUIRE(spark.vy < 0.0f);
      REQUIRE(std::fabs(spark.vx) < -spark.vy);
      REQUIRE(spark.color == 0xFF8800u);
      REQUIRE(spark.scale >= 0.25f - 0.001f && spark.scale <= 0.65f + 0.001f);
    }
    REQUIRE(!typing_spark_at(sparks[8], 999, 2.0f));
    const auto first = typing_spark_at(sparks[0], 1000, 2.0f);
    REQUIRE(first && near(first->x, 500.0f) && near(first->y, 300.0f) && near(first->alpha, 1.0f));
    const auto later = typing_spark_at(sparks[0], 1100, 2.0f);
    REQUIRE(later && later->y < 300.0f && later->alpha < first->alpha && later->radius < first->radius);
    REQUIRE(!typing_spark_at(sparks[0], 1000 + sparks[0].life, 2.0f));
    // 每颗火花都飞不出起点周围的方块。
    for (const auto &spark : sparks)
      for (uint64_t now = spark.born; now < spark.born + spark.life; now += 10)
        if (const auto frame = typing_spark_at(spark, now, 2.0f))
          REQUIRE(std::fabs(frame->x - 500.0f) <= typing_spark_reach * 2.0f &&
                  std::fabs(frame->y - 300.0f) <= typing_spark_reach * 2.0f);
    REQUIRE(typing_sparks_alive(sparks, 1500));
    REQUIRE(!typing_sparks_alive(sparks, 1000 + typing_key_burst_millis + 600));
    prune_typing_sparks(sparks, 1000 + typing_key_burst_millis + 600);
    REQUIRE(sparks.empty());
    // 同时活着的火花有上限，满了就不再发，向量也不扩容。
    const auto capacity = sparks.capacity();
    for (int burst = 0; burst < 4; ++burst)
      emit_typing_sparks(sparks, 128, 0.0f, 0.0f, true, true, 0, 0, 1.0f, seed);
    REQUIRE(sparks.size() == typing_max_sparks && sparks.capacity() == capacity);

    // 摆放：有光标时火花从光标行左端的中间升起，窗口是 ±80 的方块，徽标在光标右上方。
    TypingOverlayPlacementInput input;
    input.monitor = {0.0f, 0.0f, 1920.0f, 1080.0f};
    input.scale = 1.5f;
    input.caret = TypingRect{400.0f, 500.0f, 401.0f, 530.0f};
    input.badge_width = 60.0f;
    auto placed = typing_overlay_placement(input);
    REQUIRE(placed && near(placed->origin_x, 400.0f) && near(placed->origin_y, 515.0f));
    REQUIRE(placed->badge && near(placed->badge->left, 407.0f) && near(placed->badge->bottom, 494.0f));
    REQUIRE(placed->caret_flash && near(placed->caret_flash->left, 397.0f) && near(placed->caret_flash->top, 500.0f));
    REQUIRE(placed->frame.left <= 280.0f && placed->frame.right >= 520.0f && placed->frame.top <= 395.0f);
    REQUIRE(placed->frame.left == std::floor(placed->frame.left));
    // 有候选卡片时徽标贴在卡片右上角上方，不画光标行闪光（卡片自己闪）。
    input.card = TypingRect{380.0f, 540.0f, 700.0f, 640.0f};
    placed = typing_overlay_placement(input);
    REQUIRE(placed && placed->badge && near(placed->badge->right, 700.0f) && near(placed->badge->bottom, 534.0f));
    REQUIRE(!placed->caret_flash);
    // 没有光标时火花从卡片左上角往里 12 处升起。
    input.caret.reset();
    placed = typing_overlay_placement(input);
    REQUIRE(placed && near(placed->origin_x, 398.0f) && near(placed->origin_y, 540.0f));
    // 徽标夹在显示器里。
    input.card = TypingRect{1800.0f, 4.0f, 1920.0f, 100.0f};
    placed = typing_overlay_placement(input);
    REQUIRE(placed && placed->badge && placed->badge->top >= 6.0f - 0.001f && placed->badge->right <= 1914.0f + 0.001f);
    // 光标和卡片都没有，或者光标不可用时，什么都不画。
    input.card.reset();
    REQUIRE(!typing_overlay_placement(input));
    input.caret = TypingRect{10.0f, 10.0f, 10.0f, 10.0f};
    REQUIRE(!typing_overlay_placement(input));
    // 旧火花只在合起来的窗口不太大时保留。
    REQUIRE(typing_overlay_keeps_previous({0, 0, 160, 160}, {20, 0, 180, 160}));
    REQUIRE(!typing_overlay_keeps_previous({0, 0, 160, 160}, {1500, 900, 1660, 1060}));

    // 上屏的标记跟着答案走，0 仍是 0；打包后的颜色和火花数往返不变。
    REQUIRE(typing_effect_mark_commit(0u) == 0u);
    const auto committed = decode_typing_effect(typing_effect_mark_commit(5u | (2u << 17)));
    REQUIRE(committed.commit && committed.combo == 5u && committed.style == TypingEffectStyle::sparks);
    REQUIRE(!decode_typing_effect(5u | (2u << 17)).commit);
    const uint32_t four[] = {0x112233u, 0x445566u, 0x778899u, 0xAABBCCu, 0xDDEEFFu};
    const auto full = resolve_typing_effect_palette(four, 5, 80u);
    REQUIRE(full.color_count == 4u && full.particles == 64u);
    const auto words = pack_typing_effect_palette(full);
    REQUIRE(unpack_typing_effect_palette(words[0], words[1]) == full);
    const auto none = resolve_typing_effect_palette(nullptr, 0, 0u);
    const auto none_words = pack_typing_effect_palette(none);
    REQUIRE(unpack_typing_effect_palette(none_words[0], none_words[1]) == none);
    REQUIRE(unpack_typing_effect_palette(0, 0) == TypingEffectPalette{});

    std::cout << "Windows typing effect overlay policy checks passed\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
