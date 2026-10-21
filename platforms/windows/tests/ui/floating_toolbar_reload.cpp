#include "../../src/candidate/FloatingToolbarSettings.h"
#include <cassert>
#include <thread>
using namespace msime::windows;
int main() {
  const auto defaults = floating_toolbar_settings(nlohmann::json::object());
  assert(defaults && defaults->scale_percent == 100 && defaults->font_size == 24);
  // 表情和屏幕键盘缺省关，偏好里缺这两个键时不画，与 macOS 和 client-core 一致。
  assert(!defaults->items[3] && !defaults->items[4]);
  nlohmann::json preferences = {{"floating_toolbar", {
    {"scale_percent", 125}, {"font_size", 28}, {"character_set", false},
    {"punctuation", false}, {"fullwidth", false}, {"emoji", false},
    {"screen_keyboard", true}, {"settings", false}}}};
  auto settings = floating_toolbar_settings(preferences);
  assert(settings && settings->scale_percent == 125 && settings->font_size == 28);
  assert((settings->items == std::array<bool, 6>{false, false, false, false, true, false}));
  // The shared `english_mode` item governs the 中/英 button; absent it stays, as in the reference.
  assert(defaults->language && settings->language);
  // 切换输入方案默认开，手写和语音默认关，logo 默认不画，与 client-core 的默认值一致。
  assert(defaults->input_scheme && !defaults->handwriting && !defaults->voice && !defaults->show_logo);
  const auto default_slots = floating_toolbar_slots(*defaults);
  assert((default_slots == std::vector<int>{0, 11, 2, 1, 3, 6}));
  assert(default_slots.capacity() >= 10);
  auto english_off = preferences;
  english_off["floating_toolbar"]["english_mode"] = false;
  english_off["floating_toolbar"]["input_scheme"] = false;
  const auto without_language = floating_toolbar_settings(english_off);
  assert(without_language && !without_language->language && !without_language->input_scheme);
  const auto no_language_slots = floating_toolbar_slots(*without_language);
  assert((no_language_slots == std::vector<int>{5}));
  // 十个按钮全开时的顺序与 macOS 工具栏相同：中/英、切换输入方案、标点、全角、简繁、表情、手写、屏幕键盘、语音、设置。
  nlohmann::json everything = {{"show_app_logo", true}, {"floating_toolbar", {
    {"english_mode", true}, {"input_scheme", true}, {"character_set", true},
    {"punctuation", true}, {"fullwidth", true}, {"emoji", true},
    {"handwriting", true}, {"screen_keyboard", true}, {"voice", true},
    {"settings", true}}}};
  const auto full = floating_toolbar_settings(everything);
  assert(full && full->handwriting && full->voice && full->show_logo);
  assert((floating_toolbar_slots(*full) == std::vector<int>{0, 11, 2, 1, 3, 4, 7, 5, 8, 6}));
  // 不提供手写的版本不画手写按钮，偏好里的开关也不算。
  assert((floating_toolbar_slots(*full, false) == std::vector<int>{0, 11, 2, 1, 3, 4, 5, 8, 6}));
  // 图片工具栏按共享键序请求按钮：全角在标点之前，没有图片键的切换输入方案、手写和语音不请求。
  assert((floating_toolbar_image_slots(floating_toolbar_slots(*full)) == std::vector<int>{0, 1, 2, 3, 4, 5, 6}));
  assert((floating_toolbar_image_slots(default_slots) == std::vector<int>{0, 1, 2, 3, 6}));
  // 设置不同，比较结果也不同，工具栏据此重新排版。
  auto without_voice = *full;
  without_voice.voice = false;
  assert(!(without_voice == *full) && *full == *full);
  auto without_logo = *full;
  without_logo.show_logo = false;
  assert(!(without_logo == *full));
  for (const char *key : {"input_scheme", "handwriting", "voice"}) {
    auto bad = everything;
    bad["floating_toolbar"][key] = "true";
    assert(!floating_toolbar_settings(bad));
  }
  auto bad_logo = everything;
  bad_logo["show_app_logo"] = 1;
  assert(!floating_toolbar_settings(bad_logo));
  // 中英文切换提示的 logo 只认布尔 true，与 macOS 相同；工具栏尺寸越界让工具栏设置作废时，提示照样画 logo。
  assert(hud_show_app_logo(everything) && !hud_show_app_logo(bad_logo));
  assert(!hud_show_app_logo(nlohmann::json::object()) && !hud_show_app_logo(nullptr));
  auto oversized = everything;
  oversized["floating_toolbar"]["scale_percent"] = 151;
  assert(!floating_toolbar_settings(oversized) && hud_show_app_logo(oversized));
  english_off["floating_toolbar"]["english_mode"] = "false";
  assert(!floating_toolbar_settings(english_off));
  for (const auto &invalid : {nlohmann::json(-1), nlohmann::json(74),
                            nlohmann::json(151), nlohmann::json(1ULL << 40),
                            nlohmann::json(100.5), nlohmann::json("100")}) {
    auto bad = preferences;
    bad["floating_toolbar"]["scale_percent"] = invalid;
    assert(!floating_toolbar_settings(bad));
  }
  for (int size : {15, 29}) {
    auto bad = preferences;
    bad["floating_toolbar"]["font_size"] = size;
    assert(!floating_toolbar_settings(bad));
  }
  for (int percent : {75, 150}) {
    preferences["floating_toolbar"]["scale_percent"] = percent;
    assert(floating_toolbar_settings(preferences));
  }
  preferences["floating_toolbar"]["emoji"] = "false";
  assert(!floating_toolbar_settings(preferences));
  assert(!floating_toolbar_settings(nullptr));
  FloatingToolbarMailbox mailbox;
  assert(!mailbox.take());
  assert(mailbox.publish(1, *defaults));
  std::thread publisher([&] { assert(mailbox.publish(3, *settings)); });
  publisher.join();
  assert(!mailbox.publish(2, *defaults));
  assert(mailbox.take()->scale_percent == 125);
  assert(!mailbox.take());
  assert(!mailbox.publish(3, *defaults));
  auto invalid = *settings;
  invalid.font_size = 0;
  assert(!mailbox.publish(4, invalid));
  assert(mailbox.publish(4, *defaults));
  assert(mailbox.take()->font_size == 24);
}
