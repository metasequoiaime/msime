#include "../../src/candidate/CandidateWindowStyleSettings.h"
#include <cassert>
#include <thread>

int main() {
  using namespace msime::windows;
  // An untouched document is the shipped card: no scale, no fade, the skin's or theme's radius.
  auto defaults = candidate_window_style(nlohmann::json::object());
  assert(defaults && defaults->scale_percent == 100 &&
         defaults->opacity_percent == 100 && !defaults->corner_radius);
  assert(defaults->scale() == 1.0 && defaults->opacity() == 1.0f);
  auto configured = candidate_window_style(
      {{"candidate_scale_percent", 150},
       {"candidate_opacity_percent", 80},
       {"candidate_corner_radius", 12}});
  assert(configured && configured->scale_percent == 150 &&
         configured->opacity_percent == 80 && configured->corner_radius == 12u);
  assert(configured->scale() == 1.5);
  // 行高与行距是可选字段：缺字段与 null 都表示跟随皮肤与窗口布局，0 是一个真实的值（贴边、行挨着行）。
  auto rows = candidate_window_style(
      {{"candidate_item_padding_vertical", 3}, {"candidate_item_spacing", 0}});
  assert(rows && rows->item_padding_vertical == 3u && rows->item_spacing == 0u);
  auto neither = candidate_window_style(
      {{"candidate_item_padding_vertical", 16}, {"candidate_item_spacing", 16}});
  assert(neither && neither->valid() && *neither->item_padding_vertical == 16u &&
         *neither->item_spacing == 16u);
  auto followed_rows = candidate_window_style(
      {{"candidate_item_padding_vertical", nullptr}, {"candidate_item_spacing", nullptr}});
  assert(followed_rows && !followed_rows->item_padding_vertical &&
         !followed_rows->item_spacing);
  assert(*followed_rows == *defaults);
  // A null radius is how the settings window returns to following the skin.
  auto follow = candidate_window_style({{"candidate_corner_radius", nullptr}});
  assert(follow && !follow->corner_radius);
  auto bounds = candidate_window_style({{"candidate_scale_percent", 50},
                                        {"candidate_opacity_percent", 50},
                                        {"candidate_corner_radius", 0}});
  assert(bounds && bounds->corner_radius == 0u);
  assert(candidate_window_style({{"candidate_scale_percent", 200},
                                 {"candidate_corner_radius", 32}}));
  for (const auto &invalid :
       {nlohmann::json{{"candidate_scale_percent", 49}},
        nlohmann::json{{"candidate_scale_percent", 201}},
        nlohmann::json{{"candidate_scale_percent", -100}},
        nlohmann::json{{"candidate_scale_percent", 100.5}},
        nlohmann::json{{"candidate_scale_percent", "100"}},
        nlohmann::json{{"candidate_opacity_percent", 49}},
        nlohmann::json{{"candidate_opacity_percent", 101}},
        nlohmann::json{{"candidate_corner_radius", 33}},
        nlohmann::json{{"candidate_corner_radius", -1}},
        nlohmann::json{{"candidate_corner_radius", 4.5}},
        nlohmann::json{{"candidate_corner_radius", true}},
        nlohmann::json{{"candidate_item_padding_vertical", 17}},
        nlohmann::json{{"candidate_item_padding_vertical", -1}},
        nlohmann::json{{"candidate_item_padding_vertical", 4.5}},
        nlohmann::json{{"candidate_item_padding_vertical", "3"}},
        nlohmann::json{{"candidate_item_spacing", 17}},
        nlohmann::json{{"candidate_item_spacing", true}}})
    assert(!candidate_window_style(invalid));
  assert(!candidate_window_style(nlohmann::json::array()));

  // Precedence: the user's radius, else the package's, else the palette's.
  CandidateWindowStyle style;
  assert(candidate_card_radius(style, std::nullopt, 8.0f) == 8.0f);
  assert(candidate_card_radius(style, 14.0f, 8.0f) == 14.0f);
  style.corner_radius = 2;
  assert(candidate_card_radius(style, 14.0f, 8.0f) == 2.0f);
  style.corner_radius = 0;
  assert(candidate_card_radius(style, 14.0f, 8.0f) == 0.0f);
  // The row never rounds more than a card radius the user chose; a package's square card keeps the palette's row shape.
  assert(candidate_row_radius(style, 4.0f, 8.0f) == 4.0f);
  assert(candidate_row_radius(style, 4.0f, 2.0f) == 2.0f);
  assert(candidate_row_radius(style, 4.0f, 0.0f) == 0.0f);
  const CandidateWindowStyle unset;
  assert(candidate_row_radius(unset, 4.0f, 0.0f) == 4.0f);
  assert(candidate_row_radius(unset, 4.0f, 8.0f) == 4.0f);

  // Opacity scales alpha only, and only multiplies what the theme already chose.
  style.opacity_percent = 50;
  const auto faded = candidate_faded({0.2f, 0.4f, 0.6f, 0.8f}, style);
  assert(faded.r == 0.2f && faded.g == 0.4f && faded.b == 0.6f &&
         faded.a == 0.4f);
  style.opacity_percent = 100;
  assert(candidate_faded({0.2f, 0.4f, 0.6f, 0.8f}, style).a == 0.8f);

  CandidateWindowStyleMailbox mailbox;
  assert(!mailbox.take());
  assert(mailbox.publish(0, *defaults));
  assert(mailbox.publish(2, *configured));
  assert(!mailbox.publish(1, *defaults));
  assert(!mailbox.publish(2, *defaults));
  assert(mailbox.take() == configured);
  assert(!mailbox.take());
  auto invalid = *defaults;
  invalid.scale_percent = 300;
  assert(!mailbox.publish(100, invalid));
  assert(mailbox.publish(3, *follow));
  assert(mailbox.take() == follow);
  // Two writers can arrive out of order; only the largest revision survives.
  std::thread a([&] {
    for (uint64_t i = 4; i < 1000; i += 2)
      mailbox.publish(i, *defaults);
  });
  std::thread b([&] {
    for (uint64_t i = 5; i <= 1001; i += 2)
      mailbox.publish(i, *configured);
  });
  a.join();
  b.join();
  assert(mailbox.take() == configured);
  assert(!mailbox.publish(1000, *defaults));
}
