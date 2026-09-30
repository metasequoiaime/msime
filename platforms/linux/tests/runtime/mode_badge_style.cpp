#include "../src/overlay/ModeBadgeStyle.h"

#include <cassert>
#include <cmath>

int main() {
  using Json = nlohmann::json;
  namespace host = msime::linux_host;

  // The shared floating toolbar keys, accepted only at the values macOS accepts; anything else keeps 24 px at 100 %.
  const auto defaults = host::mode_badge_metrics(Json::object());
  assert(defaults.font_size == 24.0 && defaults.scale == 1.0);
  const auto set = host::mode_badge_metrics(Json{{"floating_toolbar", {{"font_size", 20}, {"scale_percent", 125}}}});
  assert(set.font_size == 20.0 && set.scale == 1.25);
  const auto whole = host::mode_badge_metrics(Json{{"floating_toolbar", {{"font_size", 28.0}, {"scale_percent", 150.0}}}});
  assert(whole.font_size == 28.0 && whole.scale == 1.5);
  const auto invalid = host::mode_badge_metrics(
      Json{{"floating_toolbar", {{"font_size", 23}, {"scale_percent", 110}}}});
  assert(invalid.font_size == 24.0 && invalid.scale == 1.0);
  const auto odd_types = host::mode_badge_metrics(
      Json{{"floating_toolbar", {{"font_size", "24"}, {"scale_percent", true}}}});
  assert(odd_types.font_size == 24.0 && odd_types.scale == 1.0);
  assert(host::mode_badge_metrics(Json{{"floating_toolbar", 3}}).scale == 1.0);
  assert(host::mode_badge_metrics(Json()).font_size == 24.0);

  // The default badge: (24 + 20) high, 12 + 22 + 6 + glyph + 12 wide, a 0.95 x 24 glyph, a 10 px radius.
  const auto badge = host::mode_badge_layout(defaults, 22.0, true);
  assert(badge.width == 74 && badge.height == 44);
  assert(std::abs(badge.glyph_size - 22.8) < 1e-9);
  assert(badge.logo_side == 22.0 && badge.spacing == 6.0 && badge.inset == 12.0 && badge.radius == 10.0);
  // A fractional glyph width rounds the plate outward, so the glyph is never clipped.
  assert(host::mode_badge_layout(defaults, 22.8, true).width == 75);

  // Every length scales: 125 % of 24 px is 55 high, and the logo, spacing, inset and radius grow with it.
  const auto larger = host::mode_badge_layout(host::ModeBadgeMetrics{24.0, 1.25}, 27.5, true);
  assert(larger.height == 55 && larger.width == 93);
  assert(larger.logo_side == 27.5 && larger.spacing == 7.5 && larger.inset == 15.0 && larger.radius == 12.5);
  assert(std::abs(larger.glyph_size - 28.5) < 1e-9);

  // Without a logo the glyph stands alone between the insets.
  const auto bare = host::mode_badge_layout(defaults, 22.0, false);
  assert(bare.width == 46 && bare.height == 44 && !bare.logo && bare.logo_side == 0.0 && bare.spacing == 0.0);

  // The smallest size macOS allows still fits its content.
  const auto smallest = host::mode_badge_layout(host::ModeBadgeMetrics{16.0, 0.75}, 11.4, true);
  assert(smallest.height == 27 && smallest.width == 51);
}
