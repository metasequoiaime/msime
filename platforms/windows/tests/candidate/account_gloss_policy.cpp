// 「水杉账号」候选释义的规则：哪些词发给账号、目标语言怎么写、回复怎么校验。和 macOS BackendCandidateGloss / BackendChatClient.translate 一致。
#include "AccountGlossPolicy.h"

#include <iostream>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

using namespace msime::windows;
namespace {
void require(bool value, int line) {
  if (!value)
    throw std::runtime_error("account gloss policy failed at line " + std::to_string(line));
}
#define REQUIRE(value) require((value), __LINE__)
using Words = std::vector<std::string>;
using Answered = std::vector<std::pair<std::string, std::string>>;
} // namespace

int main() {
  try {
    const auto none = [](const std::string &) { return false; };
    // 只有共享层标了 online_gloss 的候选才发：拼音缓冲、数字、英文、颜文字都不花账号额度。
    const std::vector<AccountGlossCandidate> page{
        {"你好", true}, {"nihao", false}, {"世界", true}, {"你好", true}, {"(*Φ皿Φ*)", false}};
    REQUIRE(account_gloss_words(page, {}, none) == (Words{"你好", "世界"}));
    // 本机词典已经答上的词不再问；答了空释义的不算答上。
    REQUIRE(account_gloss_words(page, Answered{{"你好", "hello"}}, none) == Words{"世界"});
    REQUIRE(account_gloss_words(page, Answered{{"你好", ""}}, none) == (Words{"你好", "世界"}));
    // 缓存里已有答案（含否定缓存）的词不再问。
    REQUIRE(account_gloss_words(page, {}, [](const std::string &word) { return word == "世界"; }) ==
            Words{"你好"});
    // 一次最多 32 个词。
    std::vector<AccountGlossCandidate> many;
    for (int index = 0; index < 40; ++index)
      many.push_back({"词" + std::to_string(index), true});
    REQUIRE(account_gloss_words(many, {}, none).size() == account_gloss_maximum_words);

    // 目标语言写成大写代码；中文目标和不合格的代码不发。
    REQUIRE(account_gloss_target("en") == std::optional<std::string>("EN"));
    REQUIRE(account_gloss_target("ja") == std::optional<std::string>("JA"));
    REQUIRE(!account_gloss_target("zh"));
    REQUIRE(!account_gloss_target(""));
    REQUIRE(!account_gloss_target("en\r\nX"));
    REQUIRE(!account_gloss_target("abcdefghijklmnopq"));

    // 回复条数必须和问的词一样多。
    const Words words{"你好", "世界"};
    REQUIRE(!account_gloss_values(words, {"hello"}));
    // 和词本身一样的释义等于没有，换成空；空表示账号对这个词没有释义。
    const auto values = account_gloss_values(words, {"hello", "世界"});
    REQUIRE(values && *values == (Words{"hello", ""}));
    // 制表符可以有，换成空格再交给会话：共享会话收释义时拒收一切控制字符，一条带制表符的释义会让整页释义都被拒掉。只有空白的释义等于没有。
    const auto tabbed = account_gloss_values(words, {"to like\tto love", "\t"});
    REQUIRE(tabbed && *tabbed == (Words{"to like to love", ""}));
    // 换行、回车、其他控制字符（含 DEL 和 U+0080 到 U+009F 的 C1 控制字符）和超长的释义让整批作废。
    REQUIRE(!account_gloss_values(words, {std::string("a\x7f" "b"), ""}));
    REQUIRE(!account_gloss_values(words, {std::string("a\xC2\x85" "b"), ""}));
    REQUIRE(!account_gloss_values(words, {std::string("a\xC2\x9F" "b"), ""}));
    // U+00A0（0xC2 0xA0）和 U+00E9 不是控制字符，照收。
    REQUIRE(account_gloss_values(words, {std::string("caf\xC3\xA9\xC2\xA0" "x"), ""}).has_value());
    REQUIRE(!account_gloss_values(words, {"a\nb", ""}));
    REQUIRE(!account_gloss_values(words, {"a\rb", ""}));
    REQUIRE(!account_gloss_values(words, {std::string("a\x01" "b"), ""}));
    REQUIRE(!account_gloss_values(words, {std::string(4097, 'a'), ""}));
    REQUIRE(account_gloss_values(words, {std::string(4096, 'a'), ""}).has_value());
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}
