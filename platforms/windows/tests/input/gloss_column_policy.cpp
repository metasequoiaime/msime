// 释义列快捷键的规则（GlossColumnPolicy.h）：修饰键选哪一列、Tab 怎么在列之间循环、每一列取哪段释义。和 macOS InputController 的 commitCandidateGlossColumn / cycleArmedGlossColumnBackwards 一致。
#include "GlossColumnPolicy.h"

#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
void require(bool value, int line) {
  if (!value)
    throw std::runtime_error("gloss column policy failed at line " + std::to_string(line));
}
#define REQUIRE(value) require((value), __LINE__)
const std::string two_lines = std::string("hello") + std::string(translation_line_separator) + "こんにちは";
const std::string second_only = std::string(translation_line_separator) + "こんにちは";
} // namespace

int main() {
  try {
    // 只按 Alt 是第 1 列，只按 Ctrl 是第 2 列；AltGr（Ctrl+Alt）、带 Shift 的和不带修饰键的都不是。
    REQUIRE(gloss_column_for_digit_modifiers(gloss_modifier_alt) == 1);
    REQUIRE(gloss_column_for_digit_modifiers(gloss_modifier_control) == 2);
    REQUIRE(!gloss_column_for_digit_modifiers(gloss_modifier_alt | gloss_modifier_control));
    REQUIRE(!gloss_column_for_digit_modifiers(gloss_modifier_alt | gloss_modifier_shift));
    REQUIRE(!gloss_column_for_digit_modifiers(0));

    // 每一列是一种目标语言的那一行。
    REQUIRE(gloss_column_text(two_lines, 1) == "hello");
    REQUIRE(gloss_column_text(two_lines, 2) == "こんにちは");
    REQUIRE(gloss_column_text("hello", 2).empty());
    REQUIRE(gloss_column_text(second_only, 1).empty());
    REQUIRE(gloss_column_text(two_lines, 0).empty());
    REQUIRE(gloss_column_text(two_lines, 3).empty());

    // Tab 在 0 和有释义的列之间循环，Shift+Tab 反过来。
    REQUIRE(next_armed_gloss_column(0, two_lines, false) == 1);
    REQUIRE(next_armed_gloss_column(1, two_lines, false) == 2);
    REQUIRE(next_armed_gloss_column(2, two_lines, false) == 0);
    REQUIRE(next_armed_gloss_column(0, two_lines, true) == 2);
    REQUIRE(next_armed_gloss_column(2, two_lines, true) == 1);
    // 只有第 1 列时在 0 和 1 之间来回；只有第 2 列时跳过空的第 1 列。
    REQUIRE(next_armed_gloss_column(0, "hello", false) == 1);
    REQUIRE(next_armed_gloss_column(1, "hello", false) == 0);
    REQUIRE(next_armed_gloss_column(0, second_only, false) == 2);
    // 当前列已经没有释义时从 0 算起。
    REQUIRE(next_armed_gloss_column(2, "hello", false) == 1);
    // 高亮候选没有释义：Tab 留给翻页。
    REQUIRE(!next_armed_gloss_column(0, "", false));
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}
