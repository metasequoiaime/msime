#include "../VocabularyDueMenu.h"

#include <cstdio>
#include <cstdlib>
#include <string>

namespace {
int failures = 0;
void check(bool condition, const char *message) {
  if (!condition) {
    std::fprintf(stderr, "vocabulary_due_menu: %s\n", message);
    ++failures;
  }
}
} // namespace

int main() {
  using msime::vocabulary_due::local_day;
  using msime::vocabulary_due::menu_label;

  check(!menu_label(0, false), "nothing due hides the row");
  check(!menu_label(-1, false), "an unreadable count hides the row");
  check(!menu_label(12, true), "the row never appears while composing");
  const auto shown = menu_label(12, false);
  check(shown && *shown == "背单词（今日待复习 12）…", "the row names today's count");

  const auto day = local_day(std::time(nullptr));
  check(day.size() == 10 && day[4] == '-' && day[7] == '-', "the local day is YYYY-MM-DD");
  return failures == 0 ? EXIT_SUCCESS : EXIT_FAILURE;
}
