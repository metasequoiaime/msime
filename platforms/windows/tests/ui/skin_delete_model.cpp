#include "../../settings/SkinDeleteModel.h"
#include <cstdio>
#include <string>

using namespace msime::settings::skin_delete;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}
bool contains(std::wstring const &text, std::wstring const &part) {
  return text.find(part) != std::wstring::npos;
}
} // namespace

int main() {
  // 确认框说清楚云端副本怎么办，与共享页的确认框同一段话。
  const auto asked = question(L"樱花");
  check(contains(asked, L"「樱花」"), "the question names the skin");
  check(contains(asked, L"下次同步时一起从云端删掉"), "the question says the private copy goes on the next sync");
  check(contains(asked, L"公开发布到社区的不受影响"), "the question says public publications stay");

  check(done(L"樱花", false) == L"已删除「樱花」。", "an unsynced skin is simply deleted");
  check(contains(done(L"樱花", true), L"下次同步时一起删掉"), "a synced skin mentions the cloud copy");

  // 选着的不能删：卡片上显示「使用中」时让用户关掉开关；槽位里留着但卡片没显示使用中时，说明要先打开再关掉。
  check(contains(failure("candidate_skin_in_use", L"樱花", true), L"先关掉它的开关"),
        "an in-use skin asks to turn it off first");
  check(contains(failure("candidate_skin_in_use", L"樱花", false), L"先打开再关掉"),
        "a skin left in a slot explains how to take it out");
  // 打开再关掉开关会把全局主题换成自定义，提示要说删完选回原来的主题。
  check(contains(failure("candidate_skin_in_use", L"樱花", false), L"选回原来的主题"),
        "taking a skin out of a slot says to pick the original theme again");
  check(!contains(failure("candidate_skin_in_use", L"樱花", true), L"选回原来的主题"),
        "an in-use skin is already on the custom theme");
  check(contains(failure("candidate_skin_not_installed", L"樱花", false), L"已经不在皮肤目录里"),
        "a missing skin says so");
  check(contains(failure("candidate_skin_package", L"樱花", false), L"手动删除"),
        "a non-folder points at the directory");
  check(failure("storage", L"樱花", false) == L"没能删除「樱花」，请稍后重试。",
        "other failures ask to retry");
  check(failure("", L"樱花", true) == L"没能删除「樱花」，请稍后重试。", "an unknown code asks to retry");
  check(rescan_after_failure("candidate_skin_not_installed"), "a missing skin rescans the directory");
  check(!rescan_after_failure("candidate_skin_in_use"), "an in-use refusal does not rescan");

  // 「回到当前」：停在在用的那张时不可点，翻走以后可点；越界的选中按最后一张算。
  check(!back_enabled(3, 3, 12), "already on the current card");
  check(back_enabled(5, 3, 12), "moved away from the current card");
  check(back_enabled(0, 3, 12), "moved to the first card");
  check(!back_enabled(11, 40, 12), "an out-of-range selection is the last card");
  check(!back_enabled(0, 0, 0), "an empty carousel has nothing to go back to");
  check(!back_enabled(-1, 0, 3), "no card shown yet");

  if (failures == 0)
    std::puts("skin delete model: ok");
  return failures == 0 ? 0 : 1;
}
