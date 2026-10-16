#include "../../settings/PunctuationSettings.h"
#include <cstdio>
#include <string>

using namespace msime::settings::punctuation;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}
} // namespace

int main() {
  // 共享页 punctuationModeOptions 的四项，顺序和文字相同。
  check(mode_choices[0].value == L"chinese" && mode_choices[0].label == L"中文标点" &&
            mode_choices[1].value == L"english" && mode_choices[1].label == L"英文标点" &&
            mode_choices[2].value == L"always_chinese" &&
            mode_choices[2].label == L"始终中文标点（英文模式也用）" &&
            mode_choices[3].value == L"always_english" &&
            mode_choices[3].label == L"始终英文标点",
        "the four options of the shared page");

  // 四种取值的推导。
  check(mode(true, L"follow") == L"chinese", "Chinese start, following");
  check(mode(false, L"follow") == L"english", "English start, following");
  check(mode(true, L"chinese") == L"always_chinese", "locked to Chinese");
  check(mode(false, L"english") == L"always_english", "locked to English");
  // 矛盾的旧组合按锁定显示，起始标点不影响显示。
  check(mode(false, L"chinese") == L"always_chinese", "the Chinese lock wins over an English start");
  check(mode(true, L"english") == L"always_english", "the English lock wins over a Chinese start");
  check(mode(true, L"sideways") == L"chinese", "an unknown lock reads as following");

  // 工具栏预览的标点按钮：锁定时取锁定值，跟随时随中英文模式，与 TIP 的 ResolvePunctuationOpen 相同。
  check(toolbar_chinese_punctuation(true, L"follow"), "following a Chinese mode shows Chinese punctuation");
  check(!toolbar_chinese_punctuation(false, L"follow"), "following an English mode shows English punctuation");
  check(toolbar_chinese_punctuation(false, L"chinese"), "the Chinese lock shows Chinese even in English mode");
  check(!toolbar_chinese_punctuation(true, L"english"), "the English lock shows English even in Chinese mode");
  check(toolbar_chinese_punctuation(true, L"sideways"), "an unknown lock reads as following");

  // 前两项把两个键一起写，「始终」两项只写锁定，不碰会上云的 chinese_punctuation。
  const auto chinese = mode_patch(L"chinese");
  check(chinese && chinese->chinese_punctuation == true && chinese->punctuation_lock == L"follow",
        "中文标点 writes a Chinese start and unlocks");
  const auto english = mode_patch(L"english");
  check(english && english->chinese_punctuation == false && english->punctuation_lock == L"follow",
        "英文标点 writes an English start and unlocks");
  const auto always_chinese = mode_patch(L"always_chinese");
  check(always_chinese && !always_chinese->chinese_punctuation &&
            always_chinese->punctuation_lock == L"chinese",
        "始终中文标点 writes only the lock");
  const auto always_english = mode_patch(L"always_english");
  check(always_english && !always_english->chinese_punctuation &&
            always_english->punctuation_lock == L"english",
        "始终英文标点 writes only the lock");
  check(!mode_patch(L"follow"), "an unknown choice writes nothing");
  for (const auto &choice : mode_choices)
    check(mode_patch(choice.value).has_value(), "every option has a patch");

  // 智能标点总开关连同两半一起写。
  check(smart_punctuation_keys[0] == L"smart_punctuation" &&
            smart_punctuation_keys[1] == L"smart_punctuation_direct_digit" &&
            smart_punctuation_keys[2] == L"smart_punctuation_direct_letter",
        "the master switch writes both halves with it");

  // 总开关开、两半都关时说明里多一句；其余情况照常。
  check(smart_punctuation_inert(true, false, false), "master on, both halves off is inert");
  check(!smart_punctuation_inert(true, true, false), "the digit half is enough");
  check(!smart_punctuation_inert(true, false, true), "the letter half is enough");
  check(!smart_punctuation_inert(false, false, false), "master off says nothing extra");
  check(std::wstring(smart_punctuation_inert_description)
                .rfind(std::wstring(smart_punctuation_description), 0) == 0,
        "the inert description extends the normal one");

  if (failures == 0)
    std::puts("punctuation settings: ok");
  return failures == 0 ? 0 : 1;
}
