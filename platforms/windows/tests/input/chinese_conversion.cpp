// Pin simplified-to-traditional conversion at the Windows host boundary.
//
// The mapping is the shared OpenCC s2t table this repository ships (the same data the reference server loads), not an operating-system table, so concrete traditional forms are this code's own behaviour and are asserted here: in particular the one-to-many characters a character table gets wrong.
#include "ChineseTextConversion.h"

#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("Chinese conversion fixture failed at line " +
                             std::to_string(line));
}
#define require(...) require_at((__VA_ARGS__), __LINE__)
} // namespace

int main() {
  try {
    // The switch is off: nothing is converted, whatever the text.
    require(simplified_to_traditional("国家", false) == "国家");
    require(simplified_to_traditional("", false).empty());

    // Empty input stays empty rather than reaching the shared converter.
    require(simplified_to_traditional("", true).empty());

    // ASCII has no traditional form and has to survive byte for byte, including text that a
    // character-by-character mapper could mangle if it ran over it.
    for (const char *text : {"abc", "ASCII 123", "a,b.c", "\t\n"})
      require(simplified_to_traditional(text, true) == text);

    // Invalid UTF-8 cannot be converted. Returning the input is the documented fallback; the one
    // thing that must never happen is losing the text or throwing out of here.
    const std::string invalid("\xff\xfe invalid", 11);
    require(simplified_to_traditional(invalid, true) == invalid);

    // Phrase-level: 发 is 髮 in 头发 and 發 in 发展, which no character table can decide.
    require(simplified_to_traditional("头发", true) == "頭髮");
    require(simplified_to_traditional("发展", true) == "發展");
    require(simplified_to_traditional("汉语输入法", true) == "漢語輸入法");
    // Mixed text converts only the Chinese and keeps everything else in place.
    require(simplified_to_traditional("hello 国家 😀", true) == "hello 國家 😀");

    // 来源 18 已逐段转换，日文汉字不得整行转成中文繁体。
    require(candidate_output("hello網絡学校", true, 18) == "hello網絡学校");
    require(candidate_output("学校", true, 0) == "學校");

    std::cout << "Chinese conversion contract holds\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
