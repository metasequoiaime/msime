#include "../../src/system/TranslationDisplay.h"
#include <cassert>

using namespace msime::windows;

int main() {
  const std::string separator(translation_line_separator);
  // 两种目标语言各占一行，行间是 U+2028，交给会话的释义里没有控制字符。
  const auto joined = join_translation_lines({"synthetic primary", "合成次译"});
  assert(joined == "synthetic primary" + separator + "合成次译");
  for (unsigned char ch : joined)
    assert(ch >= 32 && ch != 127);
  assert(translation_lines(joined) ==
         (std::vector<std::string>{"synthetic primary", "合成次译"}));
  // 第一种语言没有释义时留空行，第二种语言仍在第二行；末尾的空行去掉，全空时为空。
  assert(join_translation_lines({"", "合成次译"}) == separator + "合成次译");
  assert(translation_lines(separator + "合成次译") ==
         (std::vector<std::string>{"", "合成次译"}));
  assert(join_translation_lines({"synthetic primary", ""}) == "synthetic primary");
  assert(join_translation_lines({"", ""}).empty());
  assert(join_translation_lines({}).empty());
  assert(translation_lines("one line") == std::vector<std::string>{"one line"});
  // 来源自带的 U+2028、U+2029 折成空格，不会多出一行。
  assert(flatten_translation_line("a" + separator + "b\xE2\x80\xA9" "c") == "a b c");
  assert(join_translation_lines({"a" + separator + "b", "c"}) == "a b" + separator + "c");
  // 4096 字节放不下的行整行丢掉，第一行留着。
  const std::string primary(4091, 'a');
  assert(join_translation_lines({primary, "中"}) == primary);
  assert(join_translation_lines({std::string(4093, 'a'), "b"}).size() == 4093);
  assert(join_translation_lines({std::string(4092, 'a'), "b"}).size() == 4096);
  // Ctrl+Enter 上屏取第一条非空的行。
  assert(primary_translation_line(joined) == "synthetic primary");
  assert(primary_translation_line(separator + "合成次译") == "合成次译");
  assert(primary_translation_line("").empty());
}
