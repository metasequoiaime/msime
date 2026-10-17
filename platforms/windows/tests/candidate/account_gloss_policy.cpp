// 「水杉账号」候选释义的规则：哪些词发给账号、目标语言怎么写，以及 Rust 解读过的回复怎么换成每个词的释义。请求体和回复的解读经 msime_client_account_gloss，这里核对 Windows 用它的方式。
#include "AccountGlossPolicy.h"
#include "msime_client.h"
#include "../../../common/HostApiString.h"

#include <nlohmann/json.hpp>

#include <iostream>
#include <optional>
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
    REQUIRE(account_gloss_cache_scope("user-a") != account_gloss_cache_scope("user-b"));
    REQUIRE(account_gloss_cache_scope("") == account_gloss_cache_scope("anonymous"));

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

    // Rust 解读回复：一条坏值只让那一条为空，带换行的译文折成一行，与原文相同的等于没有。原来 Windows 自己解读时，一条带换行或控制字符的译文会让整页作废。
    const Words words{"你好", "世界", "水杉"};
    const auto parsed = [&](const nlohmann::json &data) {
      const auto request = nlohmann::json{{"operation", "parse"},
                                          {"texts", words},
                                          {"status", 200},
                                          {"body", nlohmann::json{{"code", 200}, {"data", data}}.dump()}}
                               .dump();
      const auto raw = msime::host_api::own_string(msime_client_account_gloss(
          reinterpret_cast<const uint8_t *>(request.data()), request.size()));
      REQUIRE(raw != nullptr);
      const auto document = nlohmann::json::parse(raw.get());
      std::optional<std::vector<std::string>> values;
      if (!document.value("ok", false))
        return values;
      std::vector<std::optional<std::string>> translations;
      for (const auto &item : document.at("value").at("translations"))
        translations.push_back(item.is_null() ? std::nullopt : std::optional<std::string>(item.get<std::string>()));
      return account_gloss_values(words, std::move(translations));
    };
    REQUIRE(parsed(nlohmann::json::array({"hello", "a\nb", std::string("x\x01" "y")})) == (Words{"hello", "a b", ""}));
    REQUIRE(parsed(nlohmann::json::array({"hello", "世界", std::string(4097, 'a')})) == (Words{"hello", "", ""}));
    // 条数和问的词对不上时整个回复不可用。
    REQUIRE(!parsed(nlohmann::json::array({"hello"})));
    // 回复条数必须和问的词一样多。
    REQUIRE(!account_gloss_values(words, {std::string("hello")}));
    REQUIRE(account_gloss_values(words, {std::string("hello"), std::nullopt, std::string("tree")}) ==
            (Words{"hello", "", "tree"}));
    // 请求体也由 Rust 给出：路径固定，target_lang 写成大写。
    const auto plan_request = nlohmann::json{{"operation", "request"},
                                             {"texts", words},
                                             {"source_lang", "zh"},
                                             {"target_lang", "en"}}
                                  .dump();
    const auto plan = msime::host_api::own_string(msime_client_account_gloss(
        reinterpret_cast<const uint8_t *>(plan_request.data()), plan_request.size()));
    REQUIRE(plan != nullptr);
    const auto value = nlohmann::json::parse(plan.get()).at("value");
    REQUIRE(value.at("path") == "/v1/translate");
    REQUIRE(nlohmann::json::parse(value.at("body").get<std::string>()).at("target_lang") == "EN");
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}
