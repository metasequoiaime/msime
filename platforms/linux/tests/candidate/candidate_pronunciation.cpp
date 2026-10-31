// 候选释义的英文读音（CandidatePronunciation.h），和 macOS 的 CandidatePronunciationTest.mm、Windows 的 candidate_gloss_readings.cpp 逐条对应，只取英文这一半。所有文字都是合成值。
#include "../../src/candidates/CandidatePronunciation.h"

#include <cassert>
#include <map>

namespace host = msime::linux_host;
using Json = nlohmann::json;

int main() {
  // 第一个词：第一个分隔符之前的部分，去掉两端空白（包括全角空格）。
  assert(host::gloss_first_term("love; affection") == "love");
  assert(host::gloss_first_term("  hello ，hi") == "hello");
  assert(host::gloss_first_term("\xE3\x80\x80world\xE3\x80\x80") == "world");
  assert(host::gloss_first_term("cat/dog") == "cat");
  assert(host::gloss_first_term("") == "");
  assert(host::gloss_plain_english("ice-cream") && host::gloss_plain_english("don't"));
  assert(!host::gloss_plain_english("café") && !host::gloss_plain_english("你好") && !host::gloss_plain_english(" - "));

  const std::vector<std::string> english{"en"};
  const std::map<std::string, std::string> table{{"love; affection", "/lʌv/"}, {"hello", "/həˈləʊ/"}};
  const auto lookup = [&](const std::string &text) {
    const auto found = table.find(text);
    return found == table.end() ? std::string{} : found->second;
  };
  // 中文候选读它的英文释义行；英文候选读它自己，它那一行是中文释义。
  assert(host::gloss_pronunciation("爱", "love; affection", english, lookup) == "/lʌv/");
  assert(host::gloss_pronunciation("hello", "你好; 您好", english, lookup) == "/həˈləʊ/");
  // 查不到、没有释义、目标不是英文（法文的 chat 也是纯字母，不能按英文读）都不标。
  assert(host::gloss_pronunciation("天", "sky; heaven", english, lookup).empty());
  assert(host::gloss_pronunciation("天", "", english, lookup).empty());
  assert(host::gloss_pronunciation("猫", "chat", {"fr"}, [](const std::string &) { return std::string("/x/"); }).empty());
  assert(host::gloss_pronunciation("hello", "bonjour", {"fr"}, lookup).empty());
  // 没有打开「显示读音」时宿主不给目标语言，什么也不标。
  assert(host::gloss_pronunciation("爱", "love; affection", {}, lookup).empty());
  // 多行释义按行对应：只有英文目标那一行有读音，其他行留空。
  assert(host::gloss_pronunciation("爱", "love; affection\naimer", {"en", "fr"}, lookup) == "/lʌv/\n");
  assert(host::gloss_pronunciation("爱", "aimer\nlove; affection", {"fr", "en"}, lookup) == "\n/lʌv/");

  // 要问读音表的文本：中文候选是英文释义的整行，英文候选是它自己；去重、保持顺序，不是释义的项不问。
  assert((host::gloss_english_texts("爱", "love; affection", english) == std::vector<std::string>{"love; affection"}));
  assert((host::gloss_english_texts("hello", "你好", english) == std::vector<std::string>{"hello"}));
  assert(host::gloss_english_texts("hello", "", english).empty());
  assert(host::gloss_english_texts("猫", "chat", {"fr"}).empty());
  const auto translations = Json::array({Json{{"text", "爱"}, {"translation", "love; affection"}},
                                         Json{{"text", "愛"}, {"translation", "love; affection"}},
                                         Json{{"text", "hello"}, {"translation", "你好"}},
                                         Json{{"text", "天"}, {"translation", 7}}, Json("bad")});
  assert((host::translation_pronunciation_texts(translations, english) ==
          std::vector<std::string>{"love; affection", "hello"}));
  assert(host::translation_pronunciation_texts(Json::object(), english).empty());

  // 请求文档：每项都是英文；补充目录只在给了时出现。
  const auto request = Json::parse(host::pronunciation_request(9, {"hello"}, "/home/u/.local/state/msime/resource-packs"));
  assert(request == (Json{{"generation", 9},
                          {"items", Json::array({Json{{"text", "hello"}, {"language", "en"}}})},
                          {"supplementary_resources", "/home/u/.local/state/msime/resource-packs"}}));
  assert(!Json::parse(host::pronunciation_request(9, {"hello"}, "")).contains("supplementary_resources"));
  assert(host::pronunciation_supplementary_root("/home/u/state/") == "/home/u/state/resource-packs");
  assert(host::pronunciation_supplementary_root("/home/u/state") == "/home/u/state/resource-packs");
  assert(host::pronunciation_supplementary_root("relative/state").empty());
  assert(host::pronunciation_supplementary_root("").empty());

  // 回答：问过的每个文本都有一项，没有音标的记空串；回答里没问过的、格式不对的项不收。
  const auto answers = host::read_pronunciations(
      Json{{"generation", 9},
           {"pronunciations", Json::array({Json{{"text", "hello"}, {"language", "en"}, {"pronunciation", "/həˈləʊ/"}},
                                           Json{{"text", "intruder"}, {"pronunciation", "/x/"}},
                                           Json{{"text", "sky"}, {"pronunciation", 3}}})}},
      {"hello", "sky"});
  assert(answers == (Json{{"hello", "/həˈləʊ/"}, {"sky", ""}}));
  assert(host::read_pronunciations(Json(), {"hello"}) == (Json{{"hello", ""}}));

  // 工作线程上的一次查询：只在有要问的文本时才发请求，回答原样交回。
  int asked = 0;
  const auto looked = host::lookup_translation_pronunciations(
      translations, english, 3, "/state/resource-packs", [&](const std::string &document) {
        ++asked;
        const auto parsed = Json::parse(document);
        assert(parsed.at("generation") == 3 && parsed.at("items").size() == 2 &&
               parsed.at("supplementary_resources") == "/state/resource-packs");
        return Json{{"generation", 3},
                    {"pronunciations", Json::array({Json{{"text", "love; affection"}, {"pronunciation", "/lʌv/"}}})}};
      });
  assert(asked == 1 && looked == (Json{{"love; affection", "/lʌv/"}, {"hello", ""}}));
  assert(host::lookup_translation_pronunciations(Json::array({Json{{"text", "天"}, {"translation", "天空"}}}), english, 3, "",
                                                 [&](const std::string &) { ++asked; return Json(); })
             .empty() && asked == 1);
  // 读音表请求失败时（ask 交回 null），问过的文本都记空串，候选窗照旧只画释义。
  assert(host::lookup_translation_pronunciations(translations, english, 3, "", [](const std::string &) { return Json(); }) ==
         (Json{{"love; affection", ""}, {"hello", ""}}));

  // 宿主的缓存：按文本取读音，超过上限时整个清掉。
  host::CandidatePronunciations cache;
  cache.store(looked);
  assert(cache.size() == 2 && cache.find("love; affection") == "/lʌv/" && cache.find("unknown").empty());
  assert(cache.candidate("爱", "love; affection", english) == "/lʌv/");
  assert(cache.candidate("爱", "love; affection", {}).empty());
  assert(cache.candidate("hello", "你好", english).empty());
  cache.store(Json::array());
  assert(cache.size() == 2);
  auto many = Json::object();
  for (std::size_t index = 0; index < host::CandidatePronunciations::kCapacity; ++index) many["w" + std::to_string(index)] = "";
  cache.store(many);
  assert(cache.size() == host::CandidatePronunciations::kCapacity && cache.find("love; affection").empty());
  cache.clear();
  assert(cache.size() == 0);

  // 画出来的释义：读音在它那一行下面单独一行，位置记下来按释义色减淡画；释义里的区段按行换算到新位置。
  const auto drawn = host::candidate_gloss_readings("love; affection\naimer", "/lʌv/\n");
  assert(drawn.text == "love; affection\n/lʌv/\naimer");
  assert(drawn.readings.size() == 1 && drawn.text.substr(drawn.readings[0].first, drawn.readings[0].second) == "/lʌv/");
  assert(drawn.position(0) == 0 && drawn.position(6) == 6);
  // 原释义第二行从第 16 个字节开始，画出来跟在读音那一行后面。
  assert(drawn.text.substr(drawn.position(16), 5) == "aimer");
  const auto second = host::candidate_gloss_readings("aimer\nlove", "\n/lʌv/");
  assert(second.text == "aimer\nlove\n/lʌv/" && second.position(6) == 6);
  const auto none = host::candidate_gloss_readings("love", "");
  assert(none.text == "love" && none.readings.empty() && none.position(2) == 2);
  assert(host::candidate_gloss_readings("", "/x/").text.empty());
  // IBus 的候选行只有一行：读音隔两个空格接在释义后面。
  assert(host::candidate_gloss_inline_readings("love; affection", "/lʌv/") == "love; affection  /lʌv/");
  assert(host::candidate_gloss_inline_readings("love", "") == "love");
  return 0;
}
